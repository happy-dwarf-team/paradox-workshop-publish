//! `client` mode: publishing through the logged-in Steam client with
//! `ISteamUGC` (spec section 10, ADR 0002). It is the only mode that sets
//! tags, and `init` creates items with it.
//!
//! The Steam calls sit behind the [`Workshop`] trait: [`steam::SteamClient`]
//! is the real one, and the tests use a fake, because CI has no Steam client.

pub mod drift;
pub mod steam;

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::change_note::ChangeNoteInput;
use crate::config::{Config, Visibility};
use crate::copy::{CopyError, PublishedCopy};
use crate::exit;
use crate::game::Game;
use crate::item_id::ItemId;
use crate::plan::{ModPlan, Plan};
use crate::write_back::{self, WriteBackError};

pub use crate::steamcmd::Published;

/// Where Steam asks a user to accept the Workshop legal agreement; an item
/// stays hidden until they do.
pub const LEGAL_AGREEMENT_URL: &str =
    "https://steamcommunity.com/sharedfiles/workshoplegalagreement";

/// The `ISteamUGC` calls `client` mode needs.
pub trait Workshop {
    /// `CreateItem` for `app_id`.
    fn create_item(&mut self, app_id: u32) -> Result<Created, Error>;

    /// `StartItemUpdate`, the setters for `update`, and `SubmitItemUpdate`.
    /// `events` gets the `GetItemUpdateProgress` stages while it uploads.
    fn submit_update(
        &mut self,
        app_id: u32,
        item: ItemId,
        update: &ItemUpdate,
        events: &mut dyn FnMut(Event),
    ) -> Result<Submitted, Error>;

    /// The game's install folder, when it is installed.
    fn install_dir(&self, app_id: u32) -> Option<PathBuf>;
}

/// The result of `CreateItem`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Created {
    pub item_id: ItemId,
    pub needs_agreement: bool,
}

/// The result of `SubmitItemUpdate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Submitted {
    pub needs_agreement: bool,
}

/// What one `SubmitItemUpdate` sets. `None` leaves that field as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemUpdate {
    pub title: String,
    pub description: Option<String>,
    /// The published copy, as an absolute path.
    pub content: PathBuf,
    /// An absolute path.
    pub preview: Option<PathBuf>,
    pub visibility: Option<Visibility>,
    /// Set verbatim, also when empty: the descriptor is the only source.
    pub tags: Vec<String>,
    pub change_note: Option<String>,
}

/// A `GetItemUpdateProgress` stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    PreparingConfig,
    PreparingContent,
    UploadingContent,
    UploadingPreview,
    Committing,
}

/// What `client` mode reports while it works.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Progress { stage: Stage, done: u64, total: u64 },
    Notice(String),
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::Notice(text) => f.write_str(text),
            Event::Progress { stage, done, total } => {
                let stage = match stage {
                    Stage::PreparingConfig => "preparing the update",
                    Stage::PreparingContent => "preparing the content",
                    Stage::UploadingContent => "uploading the content",
                    Stage::UploadingPreview => "uploading the preview",
                    Stage::Committing => "committing the update",
                };
                if *total > 0 {
                    write!(
                        f,
                        "{stage}: {done} of {total} bytes ({}%)",
                        done * 100 / total
                    )
                } else {
                    f.write_str(stage)
                }
            }
        }
    }
}

/// A new item made by [`init`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Initialized {
    pub item_id: ItemId,
    /// The file the ID was written into, to commit.
    pub write_back: String,
    pub needs_agreement: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    MissingSteamApi(String),
    #[error("cannot connect to Steam for {game}: {reason}")]
    Init { game: &'static str, reason: String },
    #[error("the logged-in Steam account does not own {0}; client mode needs the game")]
    NoLicence(&'static str),
    #[error(
        "client mode publishes the mods of one game per run, and this selection has {0}; \
         pick mods with --mod"
    )]
    SeveralGames(String),
    #[error("the Workshop item does not exist yet; create it with `pdx-ws init`")]
    NoItemId,
    #[error("the mod already has Workshop item {0}; update it with `pdx-ws publish --mode client`")]
    AlreadyExists(ItemId),
    #[error("Steam could not {action}: {reason}")]
    Steam {
        action: &'static str,
        reason: String,
    },
    #[error("Steam did not answer within {seconds} s while trying to {action}")]
    Timeout { action: &'static str, seconds: u64 },
    #[error(transparent)]
    Plan(#[from] crate::Error),
    #[error(transparent)]
    Copy(#[from] CopyError),
    #[error("{action}: {source}")]
    Io { action: String, source: io::Error },
    #[error("{source}; Workshop item {item_id} was created, so add its ID by hand")]
    WriteBack {
        item_id: ItemId,
        source: WriteBackError,
    },
    #[error(
        "{source} (Workshop item {item_id} exists and its ID is in {file}; finish with \
         `pdx-ws publish --mode client`)"
    )]
    AfterCreate {
        item_id: ItemId,
        file: String,
        source: Box<Error>,
    },
}

impl Error {
    /// The `pdx-ws` exit code (spec section 3).
    pub fn exit_code(&self) -> u8 {
        match self {
            Error::MissingSteamApi(_)
            | Error::SeveralGames(_)
            | Error::NoItemId
            | Error::AlreadyExists(_) => exit::VALIDATION,
            Error::Plan(error) => error.exit_code(),
            Error::AfterCreate { source, .. } => source.exit_code(),
            _ => exit::PUBLISH_FAILED,
        }
    }
}

/// The one game of the selected mods; `client` mode connects to Steam as
/// that game.
pub fn single_game(plan: &Plan) -> Result<Option<Game>, Error> {
    let mut games: Vec<Game> = plan.mods.iter().map(|module| module.game).collect();
    games.sort_by_key(|game| game.key());
    games.dedup();
    match games[..] {
        [] => Ok(None),
        [game] => Ok(Some(game)),
        _ => Err(Error::SeveralGames(
            games
                .iter()
                .map(|game| game.key())
                .collect::<Vec<_>>()
                .join(", "),
        )),
    }
}

/// What the plan sets on the item. `creating` makes an unset visibility
/// `private`, so a new item never goes public by accident.
pub fn item_update(plan: &ModPlan, content: &Path, creating: bool) -> Result<ItemUpdate, Error> {
    let absolute = |path: &Path| {
        std::path::absolute(path).map_err(|source| Error::Io {
            action: format!("cannot resolve {}", path.display()),
            source,
        })
    };
    let metadata = &plan.metadata;
    Ok(ItemUpdate {
        title: metadata.title.clone(),
        description: metadata.description.clone(),
        content: absolute(content)?,
        preview: match &metadata.preview {
            Some(preview) => Some(absolute(&plan.dir.join(&preview.path))?),
            None => None,
        },
        visibility: metadata
            .visibility
            .or(creating.then_some(Visibility::Private)),
        tags: metadata.tags.clone(),
        change_note: metadata.change_note.clone(),
    })
}

/// Updates the mod's existing item, tags included.
pub fn publish(
    workshop: &mut dyn Workshop,
    plan: &ModPlan,
    events: &mut dyn FnMut(Event),
) -> Result<Published, Error> {
    let item_id = plan.item.id.ok_or(Error::NoItemId)?;
    let copy = PublishedCopy::create(plan)?;
    let update = item_update(plan, copy.path(), false)?;
    let submitted = workshop.submit_update(plan.app_id, item_id, &update, events)?;
    if submitted.needs_agreement {
        events(Event::Notice(agreement_notice()));
    }
    Ok(Published {
        item_id,
        created: false,
        unchanged: false,
    })
}

/// Creates the mod's item, writes its ID back into the repository, and
/// uploads the published copy with title, tags, preview, visibility and
/// description. The ID is written back right after `CreateItem`, so a
/// failed upload does not lose it.
pub fn init(
    workshop: &mut dyn Workshop,
    config: &Config,
    plan: &ModPlan,
    change_note: &ChangeNoteInput,
    events: &mut dyn FnMut(Event),
) -> Result<Initialized, Error> {
    if let Some(item_id) = plan.item.id {
        return Err(Error::AlreadyExists(item_id));
    }
    let created = workshop.create_item(plan.app_id)?;
    let item_id = created.item_id;
    events(Event::Notice(format!(
        "created Workshop item {item_id} ({})",
        item_id.url()
    )));
    let file = write_back::item_id(config, plan, item_id)
        .map_err(|source| Error::WriteBack { item_id, source })?;
    events(Event::Notice(format!("wrote the item ID into {file}")));

    let mut upload = || -> Result<bool, Error> {
        // The repository holds the ID now; plan again, so the published
        // copy's descriptor carries it.
        let config = Config::load(&config.file)?;
        let selection: Vec<String> = plan.id.iter().cloned().collect();
        let mut replanned = Plan::build_with(&config, &selection, change_note)?;
        let plan = replanned.mods.remove(0);
        let copy = PublishedCopy::create(&plan)?;
        let update = item_update(&plan, copy.path(), true)?;
        let submitted = workshop.submit_update(plan.app_id, item_id, &update, events)?;
        Ok(submitted.needs_agreement)
    };
    let submitted_needs_agreement = upload().map_err(|source| Error::AfterCreate {
        item_id,
        file: file.clone(),
        source: Box::new(source),
    })?;
    let needs_agreement = created.needs_agreement || submitted_needs_agreement;
    if needs_agreement {
        events(Event::Notice(agreement_notice()));
    }
    Ok(Initialized {
        item_id,
        write_back: file,
        needs_agreement,
    })
}

fn agreement_notice() -> String {
    format!(
        "Steam shows the item only after you accept the Workshop legal agreement: \
         {LEGAL_AGREEMENT_URL}"
    )
}
