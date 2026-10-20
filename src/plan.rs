//! The publish plan: everything one publish will do, computed from the
//! repository before anything is sent to Steam (spec sections 2, 5, 6 and 8).

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::Error;
use crate::checksum;
use crate::config::{ChecksumSetting, Config, ModConfig, Visibility};
use crate::copy;
use crate::game::{DescriptorFormat, Game};
use crate::item_id::{ItemId, ItemIdSource};
use crate::mods::{Descriptor, Mod};

/// The version of the `--json` format.
pub const JSON_SCHEMA: u32 = 1;

/// Steam rejects a preview image of 1 MB or more.
pub const PREVIEW_LIMIT: u64 = 1024 * 1024;

/// The publish plan of the selected mods, in `workshop.toml` order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub schema: u32,
    pub mods: Vec<ModPlan>,
}

/// The publish plan of one mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModPlan {
    pub id: Option<String>,
    pub game: Game,
    pub app_id: u32,
    /// The mod root relative to the folder of `workshop.toml`.
    pub path: String,
    /// The mod root on disk.
    #[serde(skip)]
    pub dir: PathBuf,
    pub item: ItemPlan,
    pub metadata: WorkshopMetadata,
    /// The files of the published copy, sorted by path.
    pub files: Vec<PlannedFile>,
    /// What the excludes left out (folders end with `/`).
    pub excluded: Vec<String>,
    pub checksum: ChecksumReport,
    pub warnings: Vec<Warning>,
    /// What `init` writes into the repository once the item exists.
    pub write_backs: Vec<WriteBack>,
}

/// The Workshop item the plan targets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemPlan {
    /// `None`: the item does not exist yet.
    pub id: Option<ItemId>,
    pub source: Option<ItemIdSource>,
    pub url: Option<String>,
}

/// The Workshop metadata the plan sets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkshopMetadata {
    pub title: String,
    /// Sent verbatim; applied in `client` mode only.
    pub tags: Vec<String>,
    /// `None`: the preview is left as it is.
    pub preview: Option<Preview>,
    /// `None`: the visibility is left as it is.
    pub visibility: Option<Visibility>,
    /// The description in BBCode; `None` leaves it as it is.
    pub description: Option<String>,
    /// The change note in BBCode.
    pub change_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Preview {
    /// Relative to the mod root.
    pub path: String,
    pub size: u64,
}

/// One file of the published copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedFile {
    /// Relative to the mod root, with `/` separators.
    pub path: String,
    pub size: u64,
    /// How the copy differs from the repository's file, if it does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewrite: Option<Rewrite>,
    /// The copy's contents when they differ from the repository's file.
    #[serde(skip)]
    pub contents: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rewrite {
    pub removed: Vec<String>,
    pub added: Vec<String>,
}

/// The checksum check of the published copy (spec section 8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ChecksumReport {
    /// `checksum = "ignore"`.
    Ignored,
    /// No checksum rules for this game yet.
    Unavailable { reason: String },
    /// Nothing in the published copy or the descriptor touches checksummed
    /// files.
    Unchanged { rules: RulesUsed },
    /// The published copy changes the checksum (`checksum = "warn"`; with
    /// `"keep"` the plan fails instead).
    Changes {
        rules: RulesUsed,
        /// The files that a rule matches.
        files: Vec<String>,
        /// The `replace_path` values that cover checksummed scope.
        replace_paths: Vec<String>,
    },
}

/// The checksum rule table a check used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RulesUsed {
    /// The game versions the table applies to, e.g. `1.37`.
    pub game_version: String,
    pub verified_on: String,
    pub source: String,
    /// Why the newest table was taken, when none covers the descriptor's
    /// supported version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Warning {
    /// A stable identifier, e.g. `no-item-id`.
    pub code: &'static str,
    pub message: String,
}

/// A value `init` writes into the repository once the item exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WriteBack {
    /// Relative to the folder of `workshop.toml`.
    pub file: String,
    pub key: &'static str,
}

#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("no mod has the id `{id}`; workshop.toml has {known}")]
    UnknownMod { id: String, known: String },
    #[error("exclude `{pattern}` is not a valid glob: {message}")]
    InvalidGlob { pattern: String, message: String },
    #[error("cannot read the folder {}: {source}", path.display())]
    Walk { path: PathBuf, source: io::Error },
    #[error("{} has a name that is not valid UTF-8", .0.display())]
    NonUtf8Name(PathBuf),
    #[error("`{0}` is a symbolic link; the published copy takes regular files and folders only")]
    Symlink(String),
    #[error("the excludes leave out {0}, which the game needs")]
    DescriptorExcluded(&'static str),
    #[error("cannot read the preview {path}: {source}")]
    Preview { path: String, source: io::Error },
    #[error(
        "the preview {path} is {size} bytes; Steam takes less than 1 MB ({PREVIEW_LIMIT} bytes)"
    )]
    PreviewTooLarge { path: String, size: u64 },
    #[error("`checksum = \"keep\"`, but the published copy changes the {game} checksum: {matches}")]
    ChecksumKept { game: String, matches: String },
}

impl Plan {
    /// Plans the mods named in `selection` (by `id`), or every mod when it
    /// is empty. Reads the repository; sends nothing.
    pub fn build(config: &Config, selection: &[String]) -> Result<Plan, Error> {
        let selected = select(config, selection)?;
        let mods = selected
            .into_iter()
            .map(|mod_config| {
                let module = Mod::load(&config.base_dir, mod_config)?;
                ModPlan::build(config, &module).map_err(|source| Error::Plan {
                    label: mod_config.label(),
                    source,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Plan {
            schema: JSON_SCHEMA,
            mods,
        })
    }
}

fn select<'a>(config: &'a Config, selection: &[String]) -> Result<Vec<&'a ModConfig>, Error> {
    if selection.is_empty() {
        return Ok(config.mods.iter().collect());
    }
    for id in selection {
        if !config.mods.iter().any(|m| m.id.as_deref() == Some(id)) {
            let ids: Vec<_> = config
                .mods
                .iter()
                .filter_map(|m| m.id.as_deref())
                .map(|id| format!("`{id}`"))
                .collect();
            let known = if ids.is_empty() {
                "no mod ids".to_owned()
            } else {
                ids.join(", ")
            };
            return Err(Error::Selection(PlanError::UnknownMod {
                id: id.clone(),
                known,
            }));
        }
    }
    Ok(config
        .mods
        .iter()
        .filter(|m| m.id.as_ref().is_some_and(|id| selection.contains(id)))
        .collect())
}

impl ModPlan {
    fn build(config: &Config, module: &Mod) -> Result<ModPlan, PlanError> {
        let profile = module.profile;
        let format = profile.descriptor;
        let mut warnings = Vec::new();

        let excludes = copy::excludes(&module.config.exclude)?;
        let selection = copy::select(&module.dir, &excludes)?;
        let descriptor_path = format.descriptor_path();
        if !selection
            .files
            .iter()
            .any(|(path, _)| path == descriptor_path)
        {
            return Err(PlanError::DescriptorExcluded(descriptor_path));
        }

        let item_id = module.item_id.map(|resolved| resolved.id);
        let published_descriptor = match &module.descriptor {
            Descriptor::DescriptorMod(descriptor) => Some(descriptor.for_published_copy(item_id)),
            Descriptor::MetadataJson(_) => None,
        };
        let files: Vec<PlannedFile> = selection
            .files
            .into_iter()
            .map(|(path, size)| match &published_descriptor {
                Some(published)
                    if path == descriptor_path
                        && !(published.removed.is_empty() && published.added.is_empty()) =>
                {
                    PlannedFile {
                        path,
                        size: published.text.len() as u64,
                        rewrite: Some(Rewrite {
                            removed: published.removed.clone(),
                            added: published.added.clone(),
                        }),
                        contents: Some(published.text.clone().into_bytes()),
                    }
                }
                _ => PlannedFile {
                    path,
                    size,
                    rewrite: None,
                    contents: None,
                },
            })
            .collect();

        let preview = preview(&module.dir, format.thumbnail_path())?;
        if preview.is_none() {
            warnings.push(Warning {
                code: "no-preview",
                message: format!(
                    "there is no {}; the Workshop preview is left as it is",
                    format.thumbnail_path()
                ),
            });
        }

        let tags = module.descriptor.tags().to_vec();
        if let Some(limit) = profile.tag_limit
            && tags.len() > limit
        {
            warnings.push(Warning {
                code: "tag-limit",
                message: format!(
                    "the descriptor has {} tags; the {} launcher allows at most {limit}",
                    tags.len(),
                    profile.name
                ),
            });
        }

        let mut write_backs = Vec::new();
        if item_id.is_none() {
            warnings.push(Warning {
                code: "no-item-id",
                message: "the Workshop item does not exist yet; create it with `pdx-ws init` \
                          in client mode first"
                    .to_owned(),
            });
            write_backs.push(match format {
                DescriptorFormat::DescriptorMod => WriteBack {
                    file: relative(&module.config.path.join(descriptor_path)),
                    key: "remote_file_id",
                },
                DescriptorFormat::MetadataJson => WriteBack {
                    file: config
                        .file
                        .strip_prefix(&config.base_dir)
                        .map_or_else(|_| relative(&config.file), relative),
                    key: "item_id",
                },
            });
        }

        // Step 4 of the implementation sequence fills this in.
        if module.config.description.is_some() {
            warnings.push(Warning {
                code: "not-implemented",
                message: "`description` is not converted yet; the Workshop description is \
                          left as it is"
                    .to_owned(),
            });
        }
        let checksum = check_checksum(module, &files, &mut warnings)?;

        Ok(ModPlan {
            id: module.config.id.clone(),
            game: module.config.game,
            app_id: profile.app_id,
            path: relative(&module.config.path),
            dir: module.dir.clone(),
            item: ItemPlan {
                id: item_id,
                source: module.item_id.map(|resolved| resolved.source),
                url: item_id.map(ItemId::url),
            },
            metadata: WorkshopMetadata {
                title: module.descriptor.name().to_owned(),
                tags,
                preview,
                visibility: module.config.visibility,
                description: None,
                change_note: None,
            },
            files,
            excluded: selection.excluded,
            checksum,
            warnings,
            write_backs,
        })
    }

    /// How messages name this mod.
    pub fn label(&self) -> String {
        match &self.id {
            Some(id) => format!("mod `{id}`"),
            None => format!("the mod at `{}`", self.path),
        }
    }
}

/// The checksum check of the published copy (spec section 8).
fn check_checksum(
    module: &Mod,
    files: &[PlannedFile],
    warnings: &mut Vec<Warning>,
) -> Result<ChecksumReport, PlanError> {
    let setting = module.config.checksum;
    if setting == ChecksumSetting::Ignore {
        return Ok(ChecksumReport::Ignored);
    }
    let Some(rules) = checksum::rules(module.config.game) else {
        let reason = format!(
            "the checksum check is not available for {} yet",
            module.profile.name
        );
        if setting == ChecksumSetting::Keep {
            warnings.push(Warning {
                code: "checksum-unavailable",
                message: format!("`checksum = \"keep\"` is not enforced: {reason}"),
            });
        }
        return Ok(ChecksumReport::Unavailable { reason });
    };

    let supported_version = module.descriptor.supported_version();
    let selection = rules.select(supported_version);
    let table = selection.table;
    let used = RulesUsed {
        game_version: table.game_version.clone(),
        verified_on: table.verified_on.clone(),
        source: table.source.clone(),
        fallback: selection.fallback.then(|| match supported_version {
            Some(version) => format!(
                "no {} rules cover supported_version \"{version}\"; the newest are used",
                rules.name
            ),
            None => format!(
                "the descriptor has no supported_version; the newest {} rules are used",
                rules.name
            ),
        }),
    };
    let replace_paths: &[String] = match &module.descriptor {
        Descriptor::DescriptorMod(descriptor) => &descriptor.replace_paths,
        Descriptor::MetadataJson(_) => &[],
    };
    let findings = table.check(files.iter().map(|file| file.path.as_str()), replace_paths);
    if !findings.changes() {
        return Ok(ChecksumReport::Unchanged { rules: used });
    }
    if setting == ChecksumSetting::Keep {
        let matches = checksum_matches(&findings.files, &findings.replace_paths);
        let mut listed = matches
            .iter()
            .take(LISTED_FILES)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        if matches.len() > LISTED_FILES {
            listed.push_str(&format!(" and {} more", matches.len() - LISTED_FILES));
        }
        return Err(PlanError::ChecksumKept {
            game: rules.name.clone(),
            matches: listed,
        });
    }
    warnings.push(Warning {
        code: "checksum",
        message: format!(
            "the published copy changes the {} checksum: {}",
            rules.name, rules.effect
        ),
    });
    Ok(ChecksumReport::Changes {
        rules: used,
        files: findings.files,
        replace_paths: findings.replace_paths,
    })
}

/// What changes the checksum, `replace_path` values first.
fn checksum_matches(files: &[String], replace_paths: &[String]) -> Vec<String> {
    replace_paths
        .iter()
        .map(|path| format!("replace_path=\"{path}\""))
        .chain(files.iter().cloned())
        .collect()
}

fn preview(dir: &Path, path: &str) -> Result<Option<Preview>, PlanError> {
    let size = match fs::metadata(dir.join(path)) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(PlanError::Preview {
                path: path.to_owned(),
                source,
            });
        }
    };
    if size >= PREVIEW_LIMIT {
        return Err(PlanError::PreviewTooLarge {
            path: path.to_owned(),
            size,
        });
    }
    Ok(Some(Preview {
        path: path.to_owned(),
        size,
    }))
}

/// A relative path with `/` separators.
fn relative(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    match text.strip_prefix("./") {
        Some(rest) if !rest.is_empty() => rest.to_owned(),
        _ => text,
    }
}

/// How many files the text output lists before "and N more".
const LISTED_FILES: usize = 15;

impl fmt::Display for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, module) in self.mods.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{module}")?;
        }
        Ok(())
    }
}

impl fmt::Display for ModPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let profile = self.game.profile();
        writeln!(
            f,
            "{}: {} (app {})",
            capitalize(&self.label()),
            profile.name,
            self.app_id
        )?;
        match (&self.item.id, self.item.source) {
            (Some(id), source) => {
                let source = match source {
                    Some(ItemIdSource::WorkshopToml) => "item_id in workshop.toml",
                    _ => "remote_file_id in descriptor.mod",
                };
                writeln!(f, "  Workshop item:  {id} ({source})")?;
                writeln!(f, "                  {}", id.url())?;
            }
            (None, _) => writeln!(f, "  Workshop item:  none yet")?,
        }
        let metadata = &self.metadata;
        writeln!(f, "  Title:          {}", metadata.title)?;
        if metadata.tags.is_empty() {
            writeln!(f, "  Tags:           none (client mode only)")?;
        } else {
            writeln!(
                f,
                "  Tags:           {} (client mode only)",
                metadata.tags.join(", ")
            )?;
        }
        match &metadata.preview {
            Some(preview) => writeln!(
                f,
                "  Preview:        {} ({})",
                preview.path,
                size(preview.size)
            )?,
            None => writeln!(f, "  Preview:        unchanged")?,
        }
        match metadata.visibility {
            Some(visibility) => writeln!(f, "  Visibility:     {}", visibility.as_str())?,
            None => writeln!(f, "  Visibility:     unchanged")?,
        }
        match &metadata.description {
            Some(description) => writeln!(f, "  Description:    {} characters", description.len())?,
            None => writeln!(f, "  Description:    unchanged")?,
        }
        match &metadata.change_note {
            Some(note) => writeln!(f, "  Change note:    {note}")?,
            None => writeln!(f, "  Change note:    none")?,
        }
        let game = checksum::rules(self.game).map_or(profile.name, |rules| rules.name.as_str());
        match &self.checksum {
            ChecksumReport::Ignored => writeln!(f, "  Checksum:       ignored")?,
            ChecksumReport::Unavailable { reason } => {
                writeln!(f, "  Checksum:       not checked ({reason})")?
            }
            ChecksumReport::Unchanged { rules } => {
                writeln!(
                    f,
                    "  Checksum:       does not touch {game} checksummed files ({})",
                    describe_rules(rules)
                )?;
                if let Some(fallback) = &rules.fallback {
                    writeln!(f, "                  {fallback}")?;
                }
            }
            ChecksumReport::Changes {
                rules,
                files,
                replace_paths,
            } => {
                writeln!(
                    f,
                    "  Checksum:       changes the {game} checksum ({})",
                    describe_rules(rules)
                )?;
                if let Some(fallback) = &rules.fallback {
                    writeln!(f, "                  {fallback}")?;
                }
                let matches = checksum_matches(files, replace_paths);
                for path in matches.iter().take(LISTED_FILES) {
                    writeln!(f, "    {path}")?;
                }
                if matches.len() > LISTED_FILES {
                    writeln!(f, "    and {} more", matches.len() - LISTED_FILES)?;
                }
            }
        }

        let total: u64 = self.files.iter().map(|file| file.size).sum();
        let count = self.files.len();
        writeln!(
            f,
            "  Published copy: {count} {}, {} (from {})",
            if count == 1 { "file" } else { "files" },
            size(total),
            self.path
        )?;
        for file in self.files.iter().take(LISTED_FILES) {
            match &file.rewrite {
                Some(rewrite) => writeln!(f, "    {} ({})", file.path, describe(rewrite))?,
                None => writeln!(f, "    {}", file.path)?,
            }
        }
        if self.files.len() > LISTED_FILES {
            writeln!(f, "    and {} more", self.files.len() - LISTED_FILES)?;
        }
        if !self.excluded.is_empty() {
            writeln!(f, "  Excluded:       {}", self.excluded.join(", "))?;
        }
        for write_back in &self.write_backs {
            writeln!(
                f,
                "  Write-back:     {} in {} once the item exists",
                write_back.key, write_back.file
            )?;
        }
        for warning in &self.warnings {
            writeln!(f, "  Warning:        {}", warning.message)?;
        }
        Ok(())
    }
}

fn describe_rules(rules: &RulesUsed) -> String {
    format!(
        "rules for {}, verified on {}",
        rules.game_version, rules.verified_on
    )
}

fn describe(rewrite: &Rewrite) -> String {
    let mut parts = Vec::new();
    if !rewrite.removed.is_empty() {
        parts.push(format!("without {}", rewrite.removed.join(", ")));
    }
    if !rewrite.added.is_empty() {
        parts.push(format!("with {}", rewrite.added.join(", ")));
    }
    parts.join("; ")
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let value = bytes as f64;
    if value < KB {
        format!("{bytes} B")
    } else if value < KB * KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{:.1} MB", value / (KB * KB))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_use_slashes() {
        assert_eq!(relative(Path::new(".")), ".");
        assert_eq!(relative(Path::new("./mods/a")), "mods/a");
        assert_eq!(relative(&Path::new("mods").join("a")), "mods/a");
    }

    #[test]
    fn sizes_are_readable() {
        assert_eq!(size(512), "512 B");
        assert_eq!(size(1536), "1.5 KB");
        assert_eq!(size(3 * 1024 * 1024), "3.0 MB");
    }
}
