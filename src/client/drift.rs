//! The checksum cross-check of `client` mode (spec section 8): the installed
//! game's `checksum_manifest.txt` against the bundled checksum rules.

use std::collections::BTreeSet;
use std::fs;

use super::Workshop;
use crate::checksum::{self, RuleTable};
use crate::plan::{ChecksumReport, ModPlan};

/// The game's own list of what it hashes.
pub const MANIFEST: &str = "checksum_manifest.txt";

/// One `directory = { ... }` entry of `checksum_manifest.txt`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ManifestEntry {
    /// Lowercase, with `/` separators.
    pub folder: String,
    pub recursive: bool,
    /// Lowercase, including the dot.
    pub extension: String,
}

/// Parses `checksum_manifest.txt`: `directory = { name = "common"
/// sub_directories = yes file_extension = ".txt" }` entries. Other keys are
/// ignored.
pub fn parse_manifest(text: &str) -> Result<Vec<ManifestEntry>, String> {
    let tokens = tokenize(text)?;
    let mut entries = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let key = &tokens[index];
        if tokens.get(index + 1).map(String::as_str) != Some("=") {
            return Err(format!("expected `=` after `{key}`"));
        }
        index += 2;
        if tokens.get(index).map(String::as_str) != Some("{") {
            // A plain `key = value` at the top level.
            index += 1;
            continue;
        }
        index += 1;
        let mut fields = Vec::new();
        loop {
            match tokens.get(index).map(String::as_str) {
                None => return Err(format!("the block of `{key}` is not closed")),
                Some("}") => {
                    index += 1;
                    break;
                }
                Some(name) => {
                    if tokens.get(index + 1).map(String::as_str) != Some("=") {
                        return Err(format!("expected `=` after `{name}`"));
                    }
                    let value = tokens
                        .get(index + 2)
                        .ok_or_else(|| format!("`{name}` has no value"))?;
                    fields.push((name.to_owned(), value.clone()));
                    index += 3;
                }
            }
        }
        if key != "directory" {
            continue;
        }
        let field = |name: &str| {
            fields
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
                .ok_or_else(|| format!("a `directory` entry has no `{name}`"))
        };
        entries.push(ManifestEntry {
            folder: field("name")?
                .replace('\\', "/")
                .trim_matches('/')
                .to_lowercase(),
            recursive: match field("sub_directories")? {
                "yes" => true,
                "no" => false,
                other => return Err(format!("`sub_directories = {other}` is not yes or no")),
            },
            extension: field("file_extension")?.to_lowercase(),
        });
    }
    Ok(entries)
}

/// Words, quoted strings without the quotes, `=`, `{` and `}`; `#` starts a
/// comment.
fn tokenize(text: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '#' => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '=' | '{' | '}' => tokens.push(c.to_string()),
            '"' => {
                let mut value = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some(c) => value.push(c),
                        None => return Err("a string is not closed".to_owned()),
                    }
                }
                tokens.push(value);
            }
            c if c.is_whitespace() => {}
            c => {
                let mut word = c.to_string();
                while let Some(&next) = chars.peek() {
                    if next.is_whitespace() || matches!(next, '=' | '{' | '}' | '"' | '#') {
                        break;
                    }
                    word.push(next);
                    chars.next();
                }
                tokens.push(word);
            }
        }
    }
    Ok(tokens)
}

/// What the installed manifest and a bundled rule table disagree on, one
/// message per difference.
pub fn drift(game: &str, table: &RuleTable, entries: &[ManifestEntry]) -> Vec<String> {
    let bundled: BTreeSet<ManifestEntry> = table
        .rules
        .iter()
        .flat_map(|rule| {
            rule.extensions.iter().map(|extension| ManifestEntry {
                folder: rule.folder.to_lowercase(),
                recursive: rule.recursive,
                extension: extension.to_lowercase(),
            })
        })
        .collect();
    let installed: BTreeSet<ManifestEntry> = entries.iter().cloned().collect();
    let describe = |entry: &ManifestEntry| {
        format!(
            "`{}` files in `{}`{}",
            entry.extension,
            entry.folder,
            if entry.recursive {
                " and its subfolders"
            } else {
                ""
            }
        )
    };
    let version = &table.game_version;
    installed
        .difference(&bundled)
        .map(|entry| {
            format!(
                "the installed {MANIFEST} counts {}, which the bundled {game} {version} rules \
                 do not",
                describe(entry)
            )
        })
        .chain(bundled.difference(&installed).map(|entry| {
            format!(
                "the bundled {game} {version} rules count {}, which the installed {MANIFEST} \
                 does not",
                describe(entry)
            )
        }))
        .collect()
}

/// Compares the installed game's manifest with the rule table the plan
/// used. Returns warnings; nothing when the game has no rules.
pub fn cross_check(workshop: &dyn Workshop, plan: &ModPlan) -> Vec<String> {
    let Some(rules) = checksum::rules(plan.game) else {
        return Vec::new();
    };
    let used = match &plan.checksum {
        ChecksumReport::Unchanged { rules } | ChecksumReport::Changes { rules, .. } => {
            Some(rules.game_version.as_str())
        }
        _ => None,
    };
    let table = used
        .and_then(|version| {
            rules
                .tables
                .iter()
                .find(|table| table.game_version == version)
        })
        .unwrap_or_else(|| rules.select(None).table);
    let Some(dir) = workshop.install_dir(plan.app_id) else {
        return vec![format!(
            "{} is not installed, so its {MANIFEST} was not compared with the checksum rules",
            plan.game.profile().name
        )];
    };
    let path = dir.join(MANIFEST);
    let entries = match fs::read_to_string(&path) {
        Ok(text) => parse_manifest(&text),
        Err(error) => Err(error.to_string()),
    };
    match entries {
        Ok(entries) => drift(&rules.name, table, &entries),
        Err(reason) => vec![format!("cannot read {}: {reason}", path.display())],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    /// The shape of EU4 1.37.5's manifest (13 entries).
    const EU4_MANIFEST: &str = r#"
directory = {
	name = "common"
	sub_directories = yes
	file_extension = ".txt"
}
directory = {
	name = "common"
	sub_directories = yes
	file_extension = ".lua"
}
directory = {
	name = "events"
	sub_directories = yes
	file_extension = ".txt"
}
directory = {
	name = "missions"
	sub_directories = yes
	file_extension = ".txt"
}
directory = {
	name = "decisions"
	sub_directories = yes
	file_extension = ".txt"
}
directory = {
	name = "history"
	sub_directories = yes
	file_extension = ".txt"
}
directory = {
	name = "map"
	sub_directories = no
	file_extension = ".txt"
}
directory = {
	name = "map"
	sub_directories = no
	file_extension = ".map"
}
directory = {
	name = "map"
	sub_directories = no
	file_extension = ".bmp"
}
directory = {
	name = "map"
	sub_directories = no
	file_extension = ".csv"
}
directory = {
	name = "map/random"
	sub_directories = no
	file_extension = ".lua"
}
directory = {
	name = "map/random"
	sub_directories = yes
	file_extension = ".txt"
}
directory = {
	name = "map/random"
	sub_directories = yes
	file_extension = ".bmp"
}
"#;

    fn eu4_table() -> &'static RuleTable {
        checksum::rules(Game::Eu4)
            .unwrap()
            .select(Some("1.37.5"))
            .table
    }

    #[test]
    fn parses_the_manifest() {
        let entries = parse_manifest(EU4_MANIFEST).unwrap();
        assert_eq!(entries.len(), 13);
        assert_eq!(
            entries[10],
            ManifestEntry {
                folder: "map/random".to_owned(),
                recursive: false,
                extension: ".lua".to_owned(),
            }
        );
    }

    #[test]
    fn the_bundled_eu4_rules_match_the_1_37_5_manifest() {
        let entries = parse_manifest(EU4_MANIFEST).unwrap();
        assert_eq!(drift("EU4", eu4_table(), &entries), Vec::<String>::new());
    }

    #[test]
    fn reports_drift_both_ways() {
        let changed = EU4_MANIFEST
            .replace("\"missions\"", "\"tweakergui\"")
            .replace("\".csv\"", "\".CSV\"");
        let entries = parse_manifest(&changed).unwrap();
        assert_eq!(
            drift("EU4", eu4_table(), &entries),
            [
                "the installed checksum_manifest.txt counts `.txt` files in `tweakergui` and \
                 its subfolders, which the bundled EU4 1.37 rules do not",
                "the bundled EU4 1.37 rules count `.txt` files in `missions` and its \
                 subfolders, which the installed checksum_manifest.txt does not",
            ]
        );
    }

    #[test]
    fn malformed_manifests_are_errors() {
        assert!(parse_manifest("directory = { name = \"common\" }").is_err());
        assert!(parse_manifest("directory = { name = \"common\"").is_err());
        assert!(
            parse_manifest(
                "directory = { name = \"a\" sub_directories = maybe file_extension = \".txt\" }"
            )
            .is_err()
        );
    }
}
