//! The real [`Workshop`]: `ISteamUGC` through `steamworks-rs` and the
//! running Steam client.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use steamworks::{
    AppId, Client, FileType, PublishedFileId, PublishedFileVisibility, SteamAPIInitError,
    UpdateStatus,
};

use super::{Created, Error, Event, ItemUpdate, Stage, Submitted, Workshop};
use crate::config::Visibility;
use crate::game::Game;
use crate::item_id::ItemId;

/// The Steamworks library that release archives ship next to pdx-ws.
#[cfg(windows)]
pub const STEAM_API: &str = "steam_api64.dll";
#[cfg(target_os = "macos")]
pub const STEAM_API: &str = "libsteam_api.dylib";
#[cfg(not(any(windows, target_os = "macos")))]
pub const STEAM_API: &str = "libsteam_api.so";

/// How long `CreateItem` may take.
const CREATE_TIMEOUT: Duration = Duration::from_secs(2 * 60);
/// How long one upload may take.
const SUBMIT_TIMEOUT: Duration = Duration::from_secs(60 * 60);
/// How often progress is reported at most.
const PROGRESS_EVERY: Duration = Duration::from_millis(500);

/// Looks for [`STEAM_API`] next to the executable, then on `PATH` (the
/// folders Windows searches for a delay-loaded DLL).
pub fn find_steam_api(exe_dir: Option<&Path>, path: Option<&OsStr>) -> Option<PathBuf> {
    exe_dir
        .map(Path::to_owned)
        .into_iter()
        .chain(path.into_iter().flat_map(std::env::split_paths))
        .map(|dir| dir.join(STEAM_API))
        .find(|candidate| candidate.is_file())
}

/// On Windows `steam_api64.dll` is delay-loaded (see `build.rs`): without
/// it the first Steam call would end the process, so check first. Elsewhere
/// pdx-ws does not start without the library.
fn check_steam_api() -> Result<(), Error> {
    if !cfg!(windows) {
        return Ok(());
    }
    let exe = std::env::current_exe().ok();
    let exe_dir = exe.as_deref().and_then(Path::parent);
    match find_steam_api(exe_dir, std::env::var_os("PATH").as_deref()) {
        Some(_) => Ok(()),
        None => Err(Error::MissingSteamApi(format!(
            "client mode needs {STEAM_API} next to pdx-ws{}; it comes in the pdx-ws release \
             archive",
            exe_dir
                .map(|dir| format!(" (in {})", dir.display()))
                .unwrap_or_default()
        ))),
    }
}

/// A connection to the running Steam client as one game.
pub struct SteamClient {
    client: Client,
}

impl SteamClient {
    /// `SteamAPI_Init` with the game's app ID. Fails when Steam is not
    /// running, runs as another OS user, or the account lacks the game.
    pub fn connect(game: Game) -> Result<SteamClient, Error> {
        check_steam_api()?;
        let profile = game.profile();
        let client = Client::init_app(profile.app_id).map_err(|error| Error::Init {
            game: profile.name,
            reason: init_reason(&error),
        })?;
        if !client.apps().is_subscribed_app(AppId(profile.app_id)) {
            return Err(Error::NoLicence(profile.name));
        }
        Ok(SteamClient { client })
    }

    /// Runs Steam callbacks until `receiver` gets the call result.
    fn wait<T>(
        &self,
        action: &'static str,
        receiver: &mpsc::Receiver<T>,
        timeout: Duration,
        mut tick: impl FnMut(),
    ) -> Result<T, Error> {
        let started = Instant::now();
        loop {
            self.client.run_callbacks();
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(value) => return Ok(value),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(Error::Steam {
                        action,
                        reason: "the call result never arrived".to_owned(),
                    });
                }
            }
            tick();
            if started.elapsed() > timeout {
                return Err(Error::Timeout {
                    action,
                    seconds: timeout.as_secs(),
                });
            }
        }
    }
}

fn init_reason(error: &SteamAPIInitError) -> String {
    match error {
        SteamAPIInitError::NoSteamClient(message) => format!(
            "Steam is not running, or it runs as another OS user ({})",
            message.trim()
        ),
        SteamAPIInitError::VersionMismatch(message) => {
            format!("the Steam client is out of date ({})", message.trim())
        }
        SteamAPIInitError::FailedGeneric(message) => message.trim().to_owned(),
    }
}

fn visibility(visibility: Visibility) -> PublishedFileVisibility {
    match visibility {
        Visibility::Public => PublishedFileVisibility::Public,
        Visibility::Friends => PublishedFileVisibility::FriendsOnly,
        Visibility::Private => PublishedFileVisibility::Private,
        Visibility::Unlisted => PublishedFileVisibility::Unlisted,
    }
}

fn stage(status: UpdateStatus) -> Option<Stage> {
    match status {
        UpdateStatus::Invalid => None,
        UpdateStatus::PreparingConfig => Some(Stage::PreparingConfig),
        UpdateStatus::PreparingContent => Some(Stage::PreparingContent),
        UpdateStatus::UploadingContent => Some(Stage::UploadingContent),
        UpdateStatus::UploadingPreviewFile => Some(Stage::UploadingPreview),
        UpdateStatus::CommittingChanges => Some(Stage::Committing),
    }
}

impl Workshop for SteamClient {
    fn create_item(&mut self, app_id: u32) -> Result<Created, Error> {
        let (sender, receiver) = mpsc::channel();
        self.client
            .ugc()
            .create_item(AppId(app_id), FileType::Community, move |result| {
                let _ = sender.send(result);
            });
        let (id, needs_agreement) = self
            .wait("create the Workshop item", &receiver, CREATE_TIMEOUT, || {})?
            .map_err(|error| Error::Steam {
                action: "create the Workshop item",
                reason: error.to_string(),
            })?;
        let item_id = ItemId::try_from(id.0).map_err(|reason| Error::Steam {
            action: "create the Workshop item",
            reason: reason.to_owned(),
        })?;
        Ok(Created {
            item_id,
            needs_agreement,
        })
    }

    fn submit_update(
        &mut self,
        app_id: u32,
        item: ItemId,
        update: &ItemUpdate,
        events: &mut dyn FnMut(Event),
    ) -> Result<Submitted, Error> {
        let mut handle = self
            .client
            .ugc()
            .start_item_update(AppId(app_id), PublishedFileId(item.get()))
            .title(&update.title)
            .content_path(&update.content)
            .tags(update.tags.clone(), false);
        if let Some(description) = &update.description {
            handle = handle.description(description);
        }
        if let Some(preview) = &update.preview {
            handle = handle.preview_path(preview);
        }
        if let Some(chosen) = update.visibility {
            handle = handle.visibility(visibility(chosen));
        }

        let (sender, receiver) = mpsc::channel();
        let watch = handle.submit(update.change_note.as_deref(), move |result| {
            let _ = sender.send(result);
        });
        let mut last: Option<(Stage, u64)> = None;
        let mut reported = Instant::now()
            .checked_sub(PROGRESS_EVERY)
            .unwrap_or_else(Instant::now);
        let result = self.wait("upload the item", &receiver, SUBMIT_TIMEOUT, || {
            let (status, done, total) = watch.progress();
            let Some(stage) = stage(status) else { return };
            if last != Some((stage, done)) && reported.elapsed() >= PROGRESS_EVERY {
                last = Some((stage, done));
                reported = Instant::now();
                events(Event::Progress { stage, done, total });
            }
        })?;
        let (_, needs_agreement) = result.map_err(|error| Error::Steam {
            action: "upload the item",
            reason: error.to_string(),
        })?;
        Ok(Submitted { needs_agreement })
    }

    fn install_dir(&self, app_id: u32) -> Option<PathBuf> {
        let apps = self.client.apps();
        if !apps.is_app_installed(AppId(app_id)) {
            return None;
        }
        let dir = apps.app_install_dir(AppId(app_id));
        (!dir.is_empty()).then(|| PathBuf::from(dir))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_steam_api_next_to_the_exe_or_on_path() {
        let exe_dir = tempfile::tempdir().unwrap();
        let on_path = tempfile::tempdir().unwrap();
        let path = std::env::join_paths([on_path.path()]).unwrap();
        assert_eq!(find_steam_api(Some(exe_dir.path()), Some(&path)), None);

        std::fs::write(on_path.path().join(STEAM_API), b"").unwrap();
        assert_eq!(
            find_steam_api(Some(exe_dir.path()), Some(&path)),
            Some(on_path.path().join(STEAM_API))
        );
        std::fs::write(exe_dir.path().join(STEAM_API), b"").unwrap();
        assert_eq!(
            find_steam_api(Some(exe_dir.path()), Some(&path)),
            Some(exe_dir.path().join(STEAM_API))
        );
    }

    #[test]
    fn maps_visibility_and_stages() {
        assert!(matches!(
            visibility(Visibility::Friends),
            PublishedFileVisibility::FriendsOnly
        ));
        assert!(matches!(
            visibility(Visibility::Unlisted),
            PublishedFileVisibility::Unlisted
        ));
        assert_eq!(stage(UpdateStatus::Invalid), None);
        assert_eq!(
            stage(UpdateStatus::UploadingPreviewFile),
            Some(Stage::UploadingPreview)
        );
    }
}
