//! The change note of one publish (spec section 7): `--change-note`, else
//! the changelog section for the descriptor's version, else
//! `Version <version>` with the commit link in CI.

use std::path::Path;

use pulldown_cmark::{Event, Parser, Tag};
use serde::Serialize;

use crate::bbcode::{self, LineIndex};
use crate::description::{check_limit, read_text};
use crate::plan::PlanError;

/// What the change note can come from besides the repository.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeNoteInput {
    /// `--change-note`, in Markdown; blank counts as not given.
    pub explicit: Option<String>,
    /// The commit the fallback links to.
    pub commit_url: Option<String>,
}

impl ChangeNoteInput {
    /// `--change-note`, and the commit from the GitHub Actions variables
    /// `GITHUB_SERVER_URL`, `GITHUB_REPOSITORY` and `GITHUB_SHA`.
    pub fn from_env(explicit: Option<String>) -> ChangeNoteInput {
        ChangeNoteInput {
            explicit,
            commit_url: commit_url(|name| std::env::var(name).ok()),
        }
    }
}

/// The commit URL when all three variables are set.
fn commit_url(var: impl Fn(&str) -> Option<String>) -> Option<String> {
    let value = |name| var(name).filter(|value| !value.trim().is_empty());
    let server = value("GITHUB_SERVER_URL")?;
    let repository = value("GITHUB_REPOSITORY")?;
    let sha = value("GITHUB_SHA")?;
    Some(format!(
        "{}/{}/commit/{}",
        server.trim().trim_end_matches('/'),
        repository.trim(),
        sha.trim()
    ))
}

/// A change note in BBCode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeNote {
    pub bbcode: String,
    pub source: ChangeNoteSource,
}

/// Where a change note came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChangeNoteSource {
    /// `--change-note`.
    Explicit,
    /// The changelog section for the descriptor's version.
    Changelog { file: String, version: String },
    /// `Version <version>`.
    Fallback,
}

/// The configured changelog: the file on disk and how messages show it.
#[derive(Debug, Clone, Copy)]
pub struct Changelog<'a> {
    pub file: &'a Path,
    pub path: &'a str,
}

/// Resolves the change note by the precedence of spec section 7. `None`:
/// nothing gives one, because the descriptor has no version.
pub fn resolve(
    input: &ChangeNoteInput,
    changelog: Option<Changelog>,
    version: Option<&str>,
) -> Result<Option<ChangeNote>, PlanError> {
    if let Some(explicit) = input
        .explicit
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    {
        let bbcode =
            bbcode::to_bbcode(explicit).map_err(|source| PlanError::ChangeNoteOption { source })?;
        return note(bbcode, ChangeNoteSource::Explicit).map(Some);
    }

    if let Some(changelog) = changelog {
        let Some(version) = version else {
            return Err(PlanError::NoVersion {
                path: changelog.path.to_owned(),
            });
        };
        let text = read_text(changelog.file, changelog.path, "changelog")?;
        let section = section(&text, version).ok_or_else(|| PlanError::NoChangelogSection {
            path: changelog.path.to_owned(),
            version: version.to_owned(),
        })?;
        if section.body.is_empty() {
            return Err(PlanError::EmptyChangelogSection {
                path: changelog.path.to_owned(),
                line: section.heading_line,
            });
        }
        let bbcode = bbcode::to_bbcode(section.body).map_err(|source| PlanError::Markdown {
            path: changelog.path.to_owned(),
            source: source.starting_at(section.first_line),
        })?;
        let source = ChangeNoteSource::Changelog {
            file: changelog.path.to_owned(),
            version: version.to_owned(),
        };
        return note(bbcode, source).map(Some);
    }

    let Some(version) = version else {
        return Ok(None);
    };
    let markdown = match &input.commit_url {
        Some(url) => format!("Version {version}\n\n<{url}>"),
        None => format!("Version {version}"),
    };
    let bbcode = bbcode::to_bbcode(&markdown).map_err(|source| PlanError::Markdown {
        path: "the fallback change note".to_owned(),
        source,
    })?;
    note(bbcode, ChangeNoteSource::Fallback).map(Some)
}

fn note(bbcode: String, source: ChangeNoteSource) -> Result<ChangeNote, PlanError> {
    check_limit("change note", &bbcode)?;
    Ok(ChangeNote { bbcode, source })
}

/// A changelog section: what follows its heading up to the next heading of
/// the same or a higher level.
#[derive(Debug, PartialEq, Eq)]
struct Section<'a> {
    heading_line: usize,
    /// Without the blank lines around it.
    body: &'a str,
    /// The line `body` starts on.
    first_line: usize,
}

/// The section whose heading names `version`, as release-please
/// (`## [1.2.0](…) (date)`, `## 1.2.0 (date)`) and Keep a Changelog
/// (`## [1.2.0] - date`) write them. A leading `v` is ignored on both sides.
fn section<'a>(text: &'a str, version: &str) -> Option<Section<'a>> {
    let wanted = bare(version);
    let headings: Vec<_> = Parser::new(text)
        .into_offset_iter()
        .filter_map(|(event, range)| match event {
            Event::Start(Tag::Heading { level, .. }) => Some((level, range)),
            _ => None,
        })
        .collect();
    let index = headings
        .iter()
        .position(|(_, range)| heading_version(&text[range.clone()]) == Some(wanted))?;
    let (level, heading) = &headings[index];
    let end = headings[index + 1..]
        .iter()
        .find(|(other, _)| other <= level)
        .map_or(text.len(), |(_, range)| range.start);

    let body = &text[heading.end..end];
    // Skip whole blank lines only, so an indented first line keeps its indent.
    let leading = body.len() - body.trim_start().len();
    let skip = body[..leading].rfind('\n').map_or(0, |index| index + 1);
    let lines = LineIndex::new(text);
    Some(Section {
        heading_line: lines.line(heading.start),
        body: body[skip..].trim_end(),
        first_line: lines.line(heading.end + skip),
    })
}

fn heading_version(heading: &str) -> Option<&str> {
    let first_line = heading.lines().next()?;
    let title = first_line.trim_start_matches('#').trim();
    let version = match title.strip_prefix('[') {
        Some(rest) => rest.split(']').next()?,
        None => title.split_whitespace().next()?,
    };
    Some(bare(version))
}

fn bare(version: &str) -> &str {
    let version = version.trim();
    version
        .strip_prefix(['v', 'V'])
        .filter(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        .unwrap_or(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELEASE_PLEASE: &str = "\
# Changelog

## [1.3.0](https://github.com/o/r/compare/v1.2.0...v1.3.0) (2026-10-20)


### Features

* **missions:** add the Baltic tree ([abc1234](https://github.com/o/r/commit/abc1234))

## [1.2.0](https://github.com/o/r/compare/v1.1.0...v1.2.0) (2026-09-01)

### Bug Fixes

* fix the [Sweden] event

## 1.0.0 (2026-08-01)

* first release
";

    const KEEP_A_CHANGELOG: &str = "\
# Changelog

## [Unreleased]

## [1.2.0] - 2026-10-20

### Added

- Ideas for Hansa.

## [1.1.0] - 2026-09-01

- Older.

[unreleased]: https://github.com/o/r/compare/v1.2.0...HEAD
[1.2.0]: https://github.com/o/r/compare/v1.1.0...v1.2.0
";

    #[test]
    fn release_please_sections() {
        let section = section(RELEASE_PLEASE, "1.3.0").unwrap();
        assert_eq!(section.heading_line, 3);
        assert_eq!(section.first_line, 6);
        assert_eq!(
            section.body,
            "### Features\n\n* **missions:** add the Baltic tree \
             ([abc1234](https://github.com/o/r/commit/abc1234))"
        );
        assert_eq!(
            super::section(RELEASE_PLEASE, "v1.2.0").unwrap().body,
            "### Bug Fixes\n\n* fix the [Sweden] event"
        );
        assert_eq!(
            super::section(RELEASE_PLEASE, "1.0.0").unwrap().body,
            "* first release"
        );
        assert_eq!(super::section(RELEASE_PLEASE, "1.1.0"), None);
    }

    #[test]
    fn keep_a_changelog_sections() {
        assert_eq!(
            section(KEEP_A_CHANGELOG, "1.2.0").unwrap().body,
            "### Added\n\n- Ideas for Hansa."
        );
        let unreleased = section(KEEP_A_CHANGELOG, "Unreleased").unwrap();
        assert_eq!(unreleased.body, "");
    }

    #[test]
    fn headings_in_code_blocks_are_not_sections() {
        let text = "## 1.0.0\n\n```\n## 2.0.0\n```\n";
        assert_eq!(section(text, "2.0.0"), None);
        assert_eq!(section(text, "1.0.0").unwrap().body, "```\n## 2.0.0\n```");
    }

    fn changelog_file(text: &str) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), text).unwrap();
        file
    }

    fn resolve_with(
        input: &ChangeNoteInput,
        changelog: Option<&tempfile::NamedTempFile>,
        version: Option<&str>,
    ) -> Result<Option<ChangeNote>, PlanError> {
        let changelog = changelog.map(|file| Changelog {
            file: file.path(),
            path: "CHANGELOG.md",
        });
        resolve(input, changelog, version)
    }

    #[test]
    fn the_option_comes_first() {
        let file = changelog_file(RELEASE_PLEASE);
        let input = ChangeNoteInput {
            explicit: Some("Fixed **everything**".to_owned()),
            commit_url: Some("https://github.com/o/r/commit/abc".to_owned()),
        };
        let note = resolve_with(&input, Some(&file), Some("1.3.0"))
            .unwrap()
            .unwrap();
        assert_eq!(note.bbcode, "Fixed [b]everything[/b]");
        assert_eq!(note.source, ChangeNoteSource::Explicit);
    }

    #[test]
    fn the_changelog_comes_next() {
        let file = changelog_file(RELEASE_PLEASE);
        let input = ChangeNoteInput {
            explicit: Some("  \n".to_owned()),
            commit_url: None,
        };
        let note = resolve_with(&input, Some(&file), Some("1.3.0"))
            .unwrap()
            .unwrap();
        assert_eq!(
            note.bbcode,
            "[h3]Features[/h3]\n\n[list]\n[*][b]missions:[/b] add the Baltic tree \
             ([url=https://github.com/o/r/commit/abc1234]abc1234[/url])\n[/list]"
        );
        assert_eq!(
            note.source,
            ChangeNoteSource::Changelog {
                file: "CHANGELOG.md".to_owned(),
                version: "1.3.0".to_owned()
            }
        );
    }

    #[test]
    fn changelog_errors() {
        let file = changelog_file(KEEP_A_CHANGELOG);
        let input = ChangeNoteInput::default();
        let error = |version| {
            resolve_with(&input, Some(&file), version)
                .unwrap_err()
                .to_string()
        };
        assert_eq!(
            error(Some("2.0.0")),
            "CHANGELOG.md has no section for version 2.0.0; add a `## [2.0.0]` heading \
             or pass --change-note"
        );
        assert_eq!(
            error(Some("Unreleased")),
            "CHANGELOG.md: the section on line 3 is empty"
        );
        assert_eq!(
            error(None),
            "changelog CHANGELOG.md is configured but the descriptor has no `version`"
        );

        let file = changelog_file("## 1.0.0\n\nIntro\n\n<details>x</details>\n");
        assert_eq!(
            resolve_with(&input, Some(&file), Some("1.0.0"))
                .unwrap_err()
                .to_string(),
            "CHANGELOG.md, line 5: raw HTML is not supported; use Markdown only"
        );
    }

    #[test]
    fn the_fallback_links_the_commit() {
        let input = ChangeNoteInput {
            explicit: None,
            commit_url: Some("https://github.com/o/r/commit/abc".to_owned()),
        };
        let note = resolve_with(&input, None, Some("1.4.2")).unwrap().unwrap();
        assert_eq!(
            note.bbcode,
            "Version 1.4.2\n\n[url=https://github.com/o/r/commit/abc]\
             https://github.com/o/r/commit/abc[/url]"
        );
        assert_eq!(note.source, ChangeNoteSource::Fallback);

        let plain = resolve_with(&ChangeNoteInput::default(), None, Some("1.4.2"))
            .unwrap()
            .unwrap();
        assert_eq!(plain.bbcode, "Version 1.4.2");
        assert_eq!(
            resolve_with(&ChangeNoteInput::default(), None, None).unwrap(),
            None
        );
    }

    #[test]
    fn explicit_errors_name_the_option() {
        let input = ChangeNoteInput {
            explicit: Some("ok\n\n<b>no</b>".to_owned()),
            commit_url: None,
        };
        assert_eq!(
            resolve_with(&input, None, None).unwrap_err().to_string(),
            "--change-note, line 3: raw HTML is not supported; use Markdown only"
        );
    }

    #[test]
    fn the_commit_url_needs_all_three_variables() {
        let vars = |sha: &'static str| {
            move |name: &str| {
                Some(
                    match name {
                        "GITHUB_SERVER_URL" => "https://github.com/",
                        "GITHUB_REPOSITORY" => "o/r",
                        _ => sha,
                    }
                    .to_owned(),
                )
            }
        };
        assert_eq!(
            commit_url(vars("abc")).as_deref(),
            Some("https://github.com/o/r/commit/abc")
        );
        assert_eq!(commit_url(vars("")), None);
        assert_eq!(commit_url(|_| None), None);
    }
}
