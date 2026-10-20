//! `workshop.toml`: which mods a repository publishes and how (spec section 4).

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::Error;
use crate::game::Game;
use crate::item_id::ItemId;

/// The only `schema` this version of `pdx-ws` reads.
pub const SCHEMA: i64 = 1;

/// The default config file name, in the repository root.
pub const FILE_NAME: &str = "workshop.toml";

/// A validated `workshop.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The folder that holds `workshop.toml`; mod paths are relative to it.
    pub base_dir: PathBuf,
    /// The `[[mod]]` tables in file order, with `[defaults]` applied.
    pub mods: Vec<ModConfig>,
}

/// One `[[mod]]` table with `[defaults]` applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModConfig {
    /// The mod's slug; `None` only when the file has a single mod.
    pub id: Option<String>,
    pub game: Game,
    /// The mod root, relative to [`Config::base_dir`].
    pub path: PathBuf,
    /// `None`: publishing never changes the item's visibility.
    pub visibility: Option<Visibility>,
    /// The defaults' globs followed by the mod's own.
    pub exclude: Vec<String>,
    pub description: Option<PathBuf>,
    pub changelog: Option<PathBuf>,
    pub checksum: ChecksumSetting,
    pub item_id: Option<ItemId>,
}

impl ModConfig {
    /// How messages name this mod.
    pub fn label(&self) -> String {
        label(self.id.as_deref(), &self.path)
    }
}

fn label(id: Option<&str>, path: &Path) -> String {
    match id {
        Some(id) => format!("mod `{id}`"),
        None => format!("the mod at `{}`", path.display()),
    }
}

/// The Workshop item's visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Friends,
    Private,
    Unlisted,
}

/// What a checksum change does to a publish (spec section 8).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChecksumSetting {
    #[default]
    Warn,
    Keep,
    Ignore,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
    #[error("the file is not valid UTF-8")]
    NotUtf8,
    #[error("`schema` is missing; add `schema = {SCHEMA}` at the top")]
    MissingSchema,
    #[error("schema {0} is not supported; this pdx-ws reads schema {SCHEMA}")]
    UnsupportedSchema(i64),
    #[error("there is no [[mod]] table")]
    NoMods,
    #[error("[[mod]] #{0} has no `id`, which is required when the file has more than one mod")]
    MissingId(usize),
    #[error("mod id `{0}` is not a slug; use lowercase letters, digits and hyphens, e.g. `my-mod`")]
    InvalidId(String),
    #[error("mod id `{0}` is used more than once")]
    DuplicateId(String),
    #[error("{0}: `game` is required, in [[mod]] or [defaults]")]
    MissingGame(String),
    #[error("{label}: `path` must be relative to the folder of workshop.toml, not `{path}`")]
    AbsolutePath { label: String, path: String },
    #[error("{label}: `description` must be a .md or .bbcode file, not `{path}`")]
    DescriptionType { label: String, path: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    schema: Option<i64>,
    #[serde(default)]
    defaults: RawDefaults,
    #[serde(default, rename = "mod")]
    mods: Vec<RawMod>,
}

/// Any `[[mod]]` key except `id`, `path` and `item_id`.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDefaults {
    game: Option<Game>,
    visibility: Option<Visibility>,
    exclude: Option<Vec<String>>,
    description: Option<PathBuf>,
    changelog: Option<PathBuf>,
    checksum: Option<ChecksumSetting>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMod {
    id: Option<String>,
    game: Option<Game>,
    path: Option<PathBuf>,
    visibility: Option<Visibility>,
    exclude: Option<Vec<String>>,
    description: Option<PathBuf>,
    changelog: Option<PathBuf>,
    checksum: Option<ChecksumSetting>,
    item_id: Option<ItemId>,
}

impl Config {
    /// Reads and validates the config file at `path`.
    pub fn load(path: &Path) -> Result<Config, Error> {
        let bytes = std::fs::read(path).map_err(|source| Error::Read {
            path: path.to_owned(),
            source,
        })?;
        let base_dir = path.parent().map(Path::to_owned).unwrap_or_default();
        std::str::from_utf8(&bytes)
            .map_err(|_| ConfigError::NotUtf8)
            .and_then(|text| Config::parse(text, base_dir))
            .map_err(|source| Error::Config {
                path: path.to_owned(),
                source,
            })
    }

    /// Validates `workshop.toml` text; `base_dir` is the folder it came from.
    pub fn parse(text: &str, base_dir: PathBuf) -> Result<Config, ConfigError> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let raw: RawConfig = toml::from_str(text)?;
        match raw.schema {
            None => return Err(ConfigError::MissingSchema),
            Some(SCHEMA) => {}
            Some(other) => return Err(ConfigError::UnsupportedSchema(other)),
        }
        if raw.mods.is_empty() {
            return Err(ConfigError::NoMods);
        }

        let several = raw.mods.len() > 1;
        let mut ids = HashSet::new();
        let mut mods = Vec::with_capacity(raw.mods.len());
        for (index, raw_mod) in raw.mods.into_iter().enumerate() {
            if let Some(id) = &raw_mod.id {
                if !is_slug(id) {
                    return Err(ConfigError::InvalidId(id.clone()));
                }
                if !ids.insert(id.clone()) {
                    return Err(ConfigError::DuplicateId(id.clone()));
                }
            } else if several {
                return Err(ConfigError::MissingId(index + 1));
            }
            mods.push(apply_defaults(&raw.defaults, raw_mod)?);
        }
        Ok(Config { base_dir, mods })
    }
}

fn apply_defaults(defaults: &RawDefaults, raw: RawMod) -> Result<ModConfig, ConfigError> {
    let path = raw.path.unwrap_or_else(|| PathBuf::from("."));
    let label = label(raw.id.as_deref(), &path);
    // A root or a drive (`C:mods` on Windows) would leave the repository.
    if matches!(
        path.components().next(),
        Some(Component::RootDir | Component::Prefix(_))
    ) {
        return Err(ConfigError::AbsolutePath {
            label,
            path: path.display().to_string(),
        });
    }
    let game = raw
        .game
        .or(defaults.game)
        .ok_or_else(|| ConfigError::MissingGame(label.clone()))?;
    let description = raw.description.or_else(|| defaults.description.clone());
    if let Some(description) = &description {
        let extension = description
            .extension()
            .and_then(|extension| extension.to_str());
        if !matches!(extension, Some("md" | "bbcode")) {
            return Err(ConfigError::DescriptionType {
                label,
                path: description.display().to_string(),
            });
        }
    }
    let exclude = defaults
        .exclude
        .iter()
        .flatten()
        .chain(raw.exclude.iter().flatten())
        .cloned()
        .collect();
    Ok(ModConfig {
        id: raw.id,
        game,
        path,
        visibility: raw.visibility.or(defaults.visibility),
        exclude,
        description,
        changelog: raw.changelog.or_else(|| defaults.changelog.clone()),
        checksum: raw.checksum.or(defaults.checksum).unwrap_or_default(),
        item_id: raw.item_id,
    })
}

/// Lowercase letters, digits and inner hyphens, as in `anbennar-2k-ui`.
fn is_slug(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Config, ConfigError> {
        Config::parse(text, PathBuf::from("repo"))
    }

    fn error(text: &str) -> String {
        parse(text).unwrap_err().to_string()
    }

    #[test]
    fn minimal_file_uses_the_defaults() {
        let config = parse("schema = 1\n[[mod]]\ngame = \"eu4\"\n").unwrap();
        assert_eq!(config.base_dir, PathBuf::from("repo"));
        assert_eq!(
            config.mods,
            [ModConfig {
                id: None,
                game: Game::Eu4,
                path: PathBuf::from("."),
                visibility: None,
                exclude: vec![],
                description: None,
                changelog: None,
                checksum: ChecksumSetting::Warn,
                item_id: None,
            }]
        );
    }

    #[test]
    fn every_mod_key_is_read() {
        let config = parse(
            r#"
            schema = 1
            [[mod]]
            id = "my-mod"
            game = "vic3"
            path = "mods/my-mod"
            visibility = "unlisted"
            exclude = ["docs/**"]
            description = "workshop/description.bbcode"
            changelog = "CHANGELOG.md"
            checksum = "keep"
            item_id = 3123456789
            "#,
        )
        .unwrap();
        let module = &config.mods[0];
        assert_eq!(module.id.as_deref(), Some("my-mod"));
        assert_eq!(module.game, Game::Vic3);
        assert_eq!(module.path, PathBuf::from("mods/my-mod"));
        assert_eq!(module.visibility, Some(Visibility::Unlisted));
        assert_eq!(module.exclude, ["docs/**"]);
        assert_eq!(
            module.description,
            Some(PathBuf::from("workshop/description.bbcode"))
        );
        assert_eq!(module.changelog, Some(PathBuf::from("CHANGELOG.md")));
        assert_eq!(module.checksum, ChecksumSetting::Keep);
        assert_eq!(module.item_id.map(ItemId::get), Some(3123456789));
    }

    #[test]
    fn mod_values_override_defaults_and_excludes_combine() {
        let config = parse(
            r#"
            schema = 1
            [defaults]
            game = "eu4"
            visibility = "private"
            checksum = "ignore"
            exclude = ["docs/**"]
            changelog = "CHANGELOG.md"
            [[mod]]
            id = "a"
            [[mod]]
            id = "b"
            game = "hoi4"
            visibility = "public"
            exclude = ["*.psd"]
            "#,
        )
        .unwrap();
        let [a, b] = &config.mods[..] else {
            panic!("two mods expected");
        };
        assert_eq!(a.game, Game::Eu4);
        assert_eq!(a.visibility, Some(Visibility::Private));
        assert_eq!(a.checksum, ChecksumSetting::Ignore);
        assert_eq!(a.exclude, ["docs/**"]);
        assert_eq!(a.changelog, Some(PathBuf::from("CHANGELOG.md")));
        assert_eq!(b.game, Game::Hoi4);
        assert_eq!(b.visibility, Some(Visibility::Public));
        assert_eq!(b.exclude, ["docs/**", "*.psd"]);
    }

    #[test]
    fn schema_is_required_and_must_be_1() {
        assert_eq!(
            error("[[mod]]\ngame = \"eu4\"\n"),
            "`schema` is missing; add `schema = 1` at the top"
        );
        assert_eq!(
            error("schema = 2\n[[mod]]\ngame = \"eu4\"\n"),
            "schema 2 is not supported; this pdx-ws reads schema 1"
        );
    }

    #[test]
    fn unknown_keys_are_errors() {
        for text in [
            "schema = 1\nscheme = 1\n[[mod]]\ngame = \"eu4\"\n",
            "schema = 1\n[[mod]]\ngame = \"eu4\"\ntitle = \"x\"\n",
            "schema = 1\n[defaults]\nid = \"x\"\n[[mod]]\ngame = \"eu4\"\n",
            "schema = 1\n[defaults]\npath = \"x\"\n[[mod]]\ngame = \"eu4\"\n",
            "schema = 1\n[defaults]\nitem_id = 1\n[[mod]]\ngame = \"eu4\"\n",
        ] {
            assert!(error(text).contains("unknown field"), "{text}");
        }
    }

    #[test]
    fn toml_errors_name_the_line() {
        let message = error("schema = 1\n[[mod]]\ngame = \"eu4\"\nvisibility = \"hidden\"\n");
        assert!(message.contains("line 4"), "{message}");
        assert!(message.contains("unknown variant `hidden`"), "{message}");
    }

    #[test]
    fn games_are_validated() {
        let message = error("schema = 1\n[[mod]]\ngame = \"ck2\"\n");
        assert!(
            message.contains(
                "CK2 uses legacy single-file Workshop items, which pdx-ws does not support"
            ),
            "{message}"
        );
        assert!(error("schema = 1\n[[mod]]\ngame = \"eu3\"\n").contains("unknown game `eu3`"));
        assert_eq!(
            error("schema = 1\n[[mod]]\npath = \"a\"\n"),
            "the mod at `a`: `game` is required, in [[mod]] or [defaults]"
        );
    }

    #[test]
    fn ids_are_required_with_several_mods_and_unique() {
        let two = "schema = 1\n[defaults]\ngame = \"eu4\"\n[[mod]]\nid = \"a\"\n[[mod]]\n";
        assert_eq!(
            error(two),
            "[[mod]] #2 has no `id`, which is required when the file has more than one mod"
        );
        let twice =
            "schema = 1\n[defaults]\ngame = \"eu4\"\n[[mod]]\nid = \"a\"\n[[mod]]\nid = \"a\"\n";
        assert_eq!(error(twice), "mod id `a` is used more than once");
        for bad in ["My Mod", "my_mod", "-a", "a-", ""] {
            let text = format!("schema = 1\n[[mod]]\nid = \"{bad}\"\ngame = \"eu4\"\n");
            assert!(error(&text).contains("is not a slug"), "{bad:?}");
        }
    }

    #[test]
    fn at_least_one_mod_is_required() {
        assert_eq!(error("schema = 1\n"), "there is no [[mod]] table");
    }

    #[test]
    fn paths_and_description_types_are_checked() {
        let mut absolute = vec!["/mods/a"];
        if cfg!(windows) {
            absolute.extend(["C:/mods/a", "C:mods"]);
        }
        for path in absolute {
            let text = format!("schema = 1\n[[mod]]\ngame = \"eu4\"\npath = \"{path}\"\n");
            assert!(error(&text).contains("`path` must be relative"), "{path}");
        }
        let text = "schema = 1\n[[mod]]\ngame = \"eu4\"\ndescription = \"README.txt\"\n";
        assert_eq!(
            error(text),
            "the mod at `.`: `description` must be a .md or .bbcode file, not `README.txt`"
        );
    }

    #[test]
    fn item_id_must_be_a_positive_integer() {
        for value in ["0", "-5", "\"123\""] {
            let text = format!("schema = 1\n[[mod]]\ngame = \"vic3\"\nitem_id = {value}\n");
            assert!(parse(&text).is_err(), "{value}");
        }
    }

    #[test]
    fn a_bom_is_tolerated() {
        assert!(parse("\u{feff}schema = 1\n[[mod]]\ngame = \"eu4\"\n").is_ok());
    }
}
