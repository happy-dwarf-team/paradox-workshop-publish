//! Checksum rules: which mod files a game counts in its checksum, and
//! whether a published copy changes it (spec section 8).
//!
//! The rules are data in pdx-ws's own format (`checksum/<game>.toml`),
//! embedded in the binary. A game gets rules only once they are verified on
//! a real install; EU4 1.37.5 is the only one so far.

use std::cmp::Ordering;
use std::sync::LazyLock;

use serde::Deserialize;

use crate::game::Game;

/// The checksum rules of one game, one table per game version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameRules {
    /// How messages name the game, e.g. `EU4`.
    pub name: String,
    /// What a changed checksum means for players.
    pub effect: String,
    /// Oldest first.
    #[serde(rename = "table")]
    pub tables: Vec<RuleTable>,
}

/// The checksum rules of one game version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleTable {
    /// The versions the table applies to, as a prefix such as `1.37`.
    pub game_version: String,
    /// The exact version the rules were checked on.
    pub verified_on: String,
    /// Where the rules come from.
    pub source: String,
    #[serde(rename = "rule")]
    pub rules: Vec<Rule>,
}

/// One rule: the files with one of `extensions` in `folder`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// Relative to the mod root, with `/` separators.
    pub folder: String,
    /// Whether the files of subfolders count too.
    pub recursive: bool,
    /// Including the dot, e.g. `.txt`.
    pub extensions: Vec<String>,
    /// Subfolders of `folder` that do not count.
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// The table chosen for a mod.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection<'a> {
    pub table: &'a RuleTable,
    /// Whether no table covered the mod's `supported_version`, so the newest
    /// was taken.
    pub fallback: bool,
}

/// What in a published copy changes the checksum.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Findings {
    /// The files that a rule matches, in the given order.
    pub files: Vec<String>,
    /// The `replace_path` values that cover checksummed scope, as written.
    pub replace_paths: Vec<String>,
}

impl Findings {
    /// Whether the published copy changes the checksum.
    pub fn changes(&self) -> bool {
        !(self.files.is_empty() && self.replace_paths.is_empty())
    }
}

static EU4: LazyLock<GameRules> = LazyLock::new(|| parse(include_str!("checksum/eu4.toml")));

/// The checksum rules of `game`, or `None` when none are verified yet.
pub fn rules(game: Game) -> Option<&'static GameRules> {
    match game {
        Game::Eu4 => Some(&EU4),
        _ => None,
    }
}

/// Parses and checks embedded rules; they ship with the binary, so a
/// mistake is a bug that the unit tests catch.
fn parse(text: &str) -> GameRules {
    let rules: GameRules = toml::from_str(text).expect("the embedded checksum rules are valid");
    if let Err(problem) = rules.check() {
        panic!("the embedded checksum rules are invalid: {problem}");
    }
    rules
}

impl GameRules {
    fn check(&self) -> Result<(), String> {
        if self.tables.is_empty() {
            return Err("no tables".to_owned());
        }
        for pair in self.tables.windows(2) {
            if compare_versions(&pair[0].game_version, &pair[1].game_version) != Ordering::Less {
                return Err(format!(
                    "tables are not oldest first: {} before {}",
                    pair[0].game_version, pair[1].game_version
                ));
            }
        }
        for table in &self.tables {
            if !covers(&table.game_version, &table.verified_on) {
                return Err(format!(
                    "table {} is verified on {}, which it does not cover",
                    table.game_version, table.verified_on
                ));
            }
            for rule in &table.rules {
                rule.check()
                    .map_err(|problem| format!("table {}: {problem}", table.game_version))?;
            }
        }
        Ok(())
    }

    /// The table for a mod's `supported_version`: the newest table that
    /// covers it, or the newest table of all when none does.
    pub fn select(&self, supported_version: Option<&str>) -> Selection<'_> {
        let newest = self.tables.last().expect("rules have at least one table");
        match supported_version.and_then(|version| {
            self.tables
                .iter()
                .rev()
                .find(|table| covers(&table.game_version, version))
        }) {
            Some(table) => Selection {
                table,
                fallback: false,
            },
            None => Selection {
                table: newest,
                fallback: true,
            },
        }
    }
}

impl RuleTable {
    /// Checks the files of a published copy (relative to the mod root, with
    /// `/` separators) and the descriptor's `replace_path` values.
    pub fn check<'a>(
        &self,
        files: impl IntoIterator<Item = &'a str>,
        replace_paths: &[String],
    ) -> Findings {
        Findings {
            files: files
                .into_iter()
                .filter(|path| self.matches_file(path))
                .map(str::to_owned)
                .collect(),
            replace_paths: replace_paths
                .iter()
                .filter(|path| self.covered_by_replace_path(path))
                .cloned()
                .collect(),
        }
    }

    /// Whether the game counts the file at `path` in its checksum.
    pub fn matches_file(&self, path: &str) -> bool {
        let path = path.to_lowercase();
        self.rules.iter().any(|rule| rule.matches_file(&path))
    }

    /// Whether a `replace_path` removes checksummed files from the game's
    /// own set. E5 showed this changes the EU4 checksum even when the mod
    /// ships nothing there.
    pub fn covered_by_replace_path(&self, replace_path: &str) -> bool {
        let path = normalize(replace_path);
        self.rules.iter().any(|rule| rule.covered_by(&path))
    }
}

impl Rule {
    fn check(&self) -> Result<(), String> {
        let lowercase = |text: &str| text == text.to_lowercase();
        if self.folder.is_empty() || normalize(&self.folder) != self.folder {
            return Err(format!(
                "folder `{}` is not a plain relative path",
                self.folder
            ));
        }
        if self.extensions.is_empty() {
            return Err(format!("the rule for `{}` has no extensions", self.folder));
        }
        for extension in &self.extensions {
            if !extension.starts_with('.') || extension.contains('/') || !lowercase(extension) {
                return Err(format!("extension `{extension}` is not like `.txt`"));
            }
        }
        for exclude in &self.exclude {
            if exclude.is_empty() || normalize(exclude) != *exclude {
                return Err(format!("exclude `{exclude}` is not a plain relative path"));
            }
        }
        Ok(())
    }

    /// `path` is lowercase.
    fn matches_file(&self, path: &str) -> bool {
        let Some(rest) = strip_folder(path, &self.folder) else {
            return false;
        };
        let (dir, name) = match rest.rsplit_once('/') {
            Some((dir, name)) => (Some(dir), name),
            None => (None, rest),
        };
        if let Some(dir) = dir
            && (!self.recursive || self.excluded(dir))
        {
            return false;
        }
        self.extensions
            .iter()
            .any(|extension| name.ends_with(extension.as_str()))
    }

    /// Whether replacing the folder `path` (normalized) drops files this
    /// rule counts.
    fn covered_by(&self, path: &str) -> bool {
        if within(&self.folder, path) {
            // The replaced folder is the rule's folder or holds it.
            return true;
        }
        match strip_folder(path, &self.folder) {
            Some(rest) => self.recursive && !self.excluded(rest),
            None => false,
        }
    }

    /// `dir` is relative to the rule's folder.
    fn excluded(&self, dir: &str) -> bool {
        self.exclude.iter().any(|exclude| within(dir, exclude))
    }
}

/// `path` without the leading `folder/`, if it is inside `folder`.
fn strip_folder<'a>(path: &'a str, folder: &str) -> Option<&'a str> {
    path.strip_prefix(folder)?
        .strip_prefix('/')
        .filter(|rest| !rest.is_empty())
}

/// Whether `path` is `folder` or inside it; every path is inside the root
/// (an empty `folder`).
fn within(path: &str, folder: &str) -> bool {
    folder.is_empty() || path == folder || strip_folder(path, folder).is_some()
}

/// A descriptor path as the rules compare it: lowercase, `/` separators,
/// without leading `./` and without leading or trailing slashes.
fn normalize(path: &str) -> String {
    let path = path.trim().replace('\\', "/").to_lowercase();
    let mut path = path.as_str().trim_matches('/');
    while let Some(rest) = path.strip_prefix("./") {
        path = rest.trim_start_matches('/');
    }
    if path == "." {
        path = "";
    }
    path.to_owned()
}

/// Whether the table version `prefix` (e.g. `1.37`) covers a descriptor's
/// `supported_version` (e.g. `v1.37.*`, `1.37.5`). A `*` covers the rest.
fn covers(prefix: &str, version: &str) -> bool {
    let version = version.trim();
    let version = version
        .strip_prefix(['v', 'V'])
        .unwrap_or(version)
        .split('.')
        .collect::<Vec<_>>();
    for (index, part) in prefix.split('.').enumerate() {
        match version.get(index).map(|part| part.trim()) {
            Some("*") => return true,
            Some(given) if given == part => {}
            _ => return false,
        }
    }
    true
}

/// Orders table versions such as `1.37` by their numeric parts.
fn compare_versions(a: &str, b: &str) -> Ordering {
    let parts = |version: &str| -> Vec<u64> {
        version
            .split('.')
            .map(|part| part.parse().unwrap_or(0))
            .collect()
    };
    parts(a).cmp(&parts(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eu4() -> &'static RuleTable {
        rules(Game::Eu4).unwrap().tables.last().unwrap()
    }

    #[test]
    fn only_eu4_has_rules() {
        for game in Game::ALL {
            assert_eq!(rules(game).is_some(), game == Game::Eu4, "{game}");
        }
    }

    #[test]
    fn the_eu4_table_encodes_the_1_37_5_manifest() {
        let eu4 = rules(Game::Eu4).unwrap();
        assert_eq!(eu4.name, "EU4");
        assert!(!eu4.effect.contains("kept"));
        let table = eu4.tables.last().unwrap();
        assert_eq!(
            (table.game_version.as_str(), table.verified_on.as_str()),
            ("1.37", "1.37.5")
        );
        // The 13 entries of checksum_manifest.txt in EU4 1.37.5, in order:
        // folder, sub_directories, file_extension.
        let manifest = [
            ("common", true, ".txt"),
            ("common", true, ".lua"),
            ("events", true, ".txt"),
            ("missions", true, ".txt"),
            ("decisions", true, ".txt"),
            ("history", true, ".txt"),
            ("map", false, ".txt"),
            ("map", false, ".map"),
            ("map", false, ".bmp"),
            ("map", false, ".csv"),
            ("map/random", false, ".lua"),
            ("map/random", true, ".txt"),
            ("map/random", true, ".bmp"),
        ];
        let encoded: Vec<_> = table
            .rules
            .iter()
            .flat_map(|rule| {
                assert!(rule.exclude.is_empty());
                rule.extensions
                    .iter()
                    .map(|extension| (rule.folder.as_str(), rule.recursive, extension.as_str()))
            })
            .collect();
        assert_eq!(encoded, manifest);
    }

    #[test]
    fn files_match_by_folder_recursion_and_extension() {
        let table = eu4();
        for path in [
            "common/ideas/fixture_ideas.txt",
            "common/defines.lua",
            "common/a/b/c/deep.txt",
            "events/flavor.txt",
            "missions/x/y.txt",
            "decisions/d.txt",
            "history/provinces/1 - Uppland.txt",
            "map/default.map",
            "map/definition.csv",
            "map/provinces.bmp",
            "map/adjacencies.txt",
            "map/random/tweaks.lua",
            "map/random/tiles/tile.bmp",
            "map/random/names/n.txt",
            // Matching ignores case, so a mod packed on Windows is not missed.
            "Common/Ideas/IDEAS.TXT",
        ] {
            assert!(table.matches_file(path), "{path} should match");
        }
        for path in [
            // Extensions and sub_directories = no.
            "map/terrain/colormap.dds",
            "map/terrain/atlas.txt",
            "map/terrain/heightmap.bmp",
            "map/random/sub/script.lua",
            "map/rivers.dds",
            "common/readme.md",
            "common/ideas/ideas.yml",
            "events/events.lua",
            // Folders outside the rules.
            "interface/x.gui",
            "gfx/flags/SWE.tga",
            "localisation/x_l_english.yml",
            "music/songs.txt",
            "anvil/plugin.txt",
            "descriptor.mod",
            "commons/x.txt",
            "mapx/a.txt",
            "common.txt",
        ] {
            assert!(!table.matches_file(path), "{path} should not match");
        }
    }

    #[test]
    fn replace_paths_cover_checksummed_scope() {
        let table = eu4();
        for path in [
            // E5: changes the EU4 1.37.5 checksum.
            "common/client_states",
            "common",
            "history/provinces",
            "common/bookmarks/",
            "./events",
            "map",
            "map/random",
            "map/random/tiles",
            "Common\\Ideas",
            "",
        ] {
            assert!(
                table.covered_by_replace_path(path),
                "{path:?} should be covered"
            );
        }
        for path in [
            // E5: does not change it.
            "music",
            "gfx/interface",
            "localisation",
            // map is not recursive, and map/random has no rule for this folder.
            "map/terrain",
            "commons",
        ] {
            assert!(
                !table.covered_by_replace_path(path),
                "{path:?} should not be covered"
            );
        }
    }

    #[test]
    fn excludes_leave_subfolders_out() {
        let table = RuleTable {
            game_version: "1.0".to_owned(),
            verified_on: "1.0.0".to_owned(),
            source: "test".to_owned(),
            rules: vec![Rule {
                folder: "common".to_owned(),
                recursive: true,
                extensions: vec![".txt".to_owned()],
                exclude: vec!["dlc".to_owned()],
            }],
        };
        assert!(table.matches_file("common/a.txt"));
        assert!(table.matches_file("common/dlcs/a.txt"));
        assert!(!table.matches_file("common/dlc/a.txt"));
        assert!(!table.matches_file("common/dlc/x/a.txt"));
        assert!(table.covered_by_replace_path("common"));
        assert!(!table.covered_by_replace_path("common/dlc"));
        assert!(!table.covered_by_replace_path("common/dlc/x"));
        assert!(table.covered_by_replace_path("common/other"));
    }

    #[test]
    fn findings_keep_the_order_and_spelling() {
        let replace_paths = ["music".to_owned(), "Common/Bookmarks/".to_owned()];
        let findings = eu4().check(
            [
                "descriptor.mod",
                "map/terrain/x.dds",
                "events/b.txt",
                "common/a.txt",
            ],
            &replace_paths,
        );
        assert_eq!(findings.files, ["events/b.txt", "common/a.txt"]);
        assert_eq!(findings.replace_paths, ["Common/Bookmarks/"]);
        assert!(findings.changes());
        assert!(!eu4().check(["gfx/a.dds"], &[]).changes());
    }

    fn table(version: &str) -> RuleTable {
        RuleTable {
            game_version: version.to_owned(),
            verified_on: format!("{version}.0"),
            source: "test".to_owned(),
            rules: vec![Rule {
                folder: "common".to_owned(),
                recursive: true,
                extensions: vec![".txt".to_owned()],
                exclude: Vec::new(),
            }],
        }
    }

    #[test]
    fn tables_are_selected_by_supported_version() {
        let rules = GameRules {
            name: "Test".to_owned(),
            effect: "test".to_owned(),
            tables: vec![table("1.36"), table("1.37")],
        };
        assert_eq!(rules.check(), Ok(()));
        let select = |version| {
            let selection = rules.select(version);
            (selection.table.game_version.as_str(), selection.fallback)
        };
        assert_eq!(select(Some("v1.36.*")), ("1.36", false));
        assert_eq!(select(Some("1.36.2.0")), ("1.36", false));
        assert_eq!(select(Some("v1.37.*")), ("1.37", false));
        assert_eq!(select(Some(" V1.37.5 ")), ("1.37", false));
        // A wider wildcard takes the newest table it covers.
        assert_eq!(select(Some("1.*")), ("1.37", false));
        assert_eq!(select(Some("*")), ("1.37", false));
        // No match: the newest table, flagged.
        assert_eq!(select(Some("v1.35.*")), ("1.37", true));
        assert_eq!(select(Some("1.3")), ("1.37", true));
        assert_eq!(select(Some("1.370")), ("1.37", true));
        assert_eq!(select(Some("")), ("1.37", true));
        assert_eq!(select(None), ("1.37", true));
    }

    #[test]
    fn invalid_rules_are_caught() {
        let mut rules = GameRules {
            name: "Test".to_owned(),
            effect: "test".to_owned(),
            tables: vec![table("1.37"), table("1.36")],
        };
        assert!(rules.check().unwrap_err().contains("oldest first"));
        rules.tables = vec![table("1.9"), table("1.10")];
        assert_eq!(rules.check(), Ok(()));

        let mut bad = table("1.37");
        bad.verified_on = "1.36.1".to_owned();
        rules.tables = vec![bad];
        assert!(rules.check().unwrap_err().contains("does not cover"));

        for (folder, extension) in [
            ("/common", ".txt"),
            ("common/", ".txt"),
            ("Common", ".txt"),
            ("common", "txt"),
            ("common", ".TXT"),
        ] {
            let mut bad = table("1.37");
            bad.rules[0].folder = folder.to_owned();
            bad.rules[0].extensions = vec![extension.to_owned()];
            rules.tables = vec![bad];
            assert!(rules.check().is_err(), "{folder} {extension}");
        }
        rules.tables.clear();
        assert!(rules.check().is_err());
    }
}
