//! What the GitHub Action reports (spec section 11): the job summary,
//! warning annotations and the `item-id` / `item-url` outputs, rendered from
//! the `plan --json` and `publish --json` files of one run.
//!
//! The files are read into small views of their own, so the report only
//! depends on the JSON fields it shows.

use std::fmt::Write as _;

use serde::Deserialize;

/// How many checksum matches get a file annotation of their own; GitHub
/// shows at most 10 warning annotations per step.
pub const ANNOTATED_FILES: usize = 5;

/// How many checksum matches the summary lists before "and N more".
const LISTED_MATCHES: usize = 15;

/// The part of `plan --json` the report shows.
#[derive(Debug, Clone, Deserialize)]
pub struct PlanView {
    pub mods: Vec<ModView>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModView {
    pub id: Option<String>,
    pub game: String,
    pub path: String,
    pub item: ItemView,
    pub metadata: MetadataView,
    pub files: Vec<FileView>,
    pub checksum: ChecksumView,
    pub warnings: Vec<WarningView>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemView {
    pub id: Option<u64>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetadataView {
    pub title: String,
    pub tags: Vec<String>,
    pub visibility: Option<String>,
    pub description: Option<String>,
    pub description_source: Option<String>,
    pub change_note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileView {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ChecksumView {
    Ignored,
    Unavailable {
        reason: String,
    },
    Unchanged {
        rules: RulesView,
    },
    Changes {
        rules: RulesView,
        files: Vec<String>,
        replace_paths: Vec<String>,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct RulesView {
    pub game_version: String,
    pub verified_on: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WarningView {
    pub code: String,
    pub message: String,
}

/// The part of `publish --json` the report shows.
#[derive(Debug, Clone, Deserialize)]
pub struct ResultsView {
    pub mods: Vec<ResultView>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResultView {
    pub id: Option<String>,
    pub path: String,
    pub item_id: Option<u64>,
    pub url: Option<String>,
    pub outcome: String,
    pub error: Option<String>,
    pub details: Vec<String>,
}

/// How the run went, beyond the two files.
#[derive(Debug, Clone, Default)]
pub struct Run {
    /// Nothing was sent to Steam.
    pub dry_run: bool,
    /// Why the run is a dry run when the workflow did not ask for one.
    pub dry_run_reason: Option<String>,
    /// The exit code of `publish`, when it ran.
    pub publish_exit: Option<u8>,
    /// `workshop.toml` as the workflow named it, relative to the repository
    /// root; annotation paths start from its folder.
    pub config: String,
}

impl ModView {
    fn name(&self) -> &str {
        self.id.as_deref().unwrap_or(&self.path)
    }

    fn label(&self) -> String {
        match &self.id {
            Some(id) => format!("mod `{id}`"),
            None => format!("the mod at `{}`", self.path),
        }
    }
}

impl ResultsView {
    fn find(&self, module: &ModView) -> Option<&ResultView> {
        self.mods
            .iter()
            .find(|result| result.id == module.id && result.path == module.path)
    }
}

/// The job summary in GitHub Markdown.
pub fn summary(plan: &PlanView, results: Option<&ResultsView>, run: &Run) -> String {
    let mut out = String::new();
    let heading = if run.dry_run {
        "pdx-ws plan (dry run)"
    } else {
        "pdx-ws publish"
    };
    let _ = writeln!(out, "## {heading}\n");
    if run.dry_run {
        let reason = run
            .dry_run_reason
            .as_deref()
            .map(|reason| format!(" ({reason})"))
            .unwrap_or_default();
        let _ = writeln!(out, "Dry run{reason}: nothing was sent to Steam.\n");
    } else if results.is_none() {
        let code = run
            .publish_exit
            .map(|code| format!(" with exit code {code}"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "`pdx-ws publish` stopped{code} before it published any mod; the job log has \
             the error.\n"
        );
    }

    let _ = writeln!(
        out,
        "| Mod | Game | Workshop item | Files | Checksum | Result |"
    );
    let _ = writeln!(out, "| --- | --- | --- | --- | --- | --- |");
    for module in &plan.mods {
        let result = results.and_then(|results| results.find(module));
        let item = item_link(module, result);
        let outcome = match (result, run.dry_run) {
            (Some(result), _) => result.outcome.clone(),
            (None, true) => "planned".to_owned(),
            (None, false) => "not published".to_owned(),
        };
        let _ = writeln!(
            out,
            "| `{}` | {} | {item} | {} | {} | {outcome} |",
            cell(module.name()),
            cell(&module.game),
            module.files.len(),
            checksum_cell(&module.checksum)
        );
    }

    for module in &plan.mods {
        let result = results.and_then(|results| results.find(module));
        let _ = writeln!(
            out,
            "\n### `{}`: {}\n",
            cell(module.name()),
            text(&module.metadata.title)
        );
        let _ = writeln!(out, "- Workshop item: {}", item_link(module, result));
        let metadata = &module.metadata;
        let tags = if metadata.tags.is_empty() {
            "none".to_owned()
        } else {
            text(&metadata.tags.join(", "))
        };
        let _ = writeln!(out, "- Tags: {tags} (applied in client mode only)");
        let _ = writeln!(
            out,
            "- Visibility: {}",
            metadata.visibility.as_deref().unwrap_or("unchanged")
        );
        if let Some(result) = result {
            match &result.error {
                Some(error) => {
                    let _ = writeln!(out, "- Result: **{}**: {}", result.outcome, text(error));
                }
                None => {
                    let _ = writeln!(out, "- Result: **{}**", result.outcome);
                }
            }
        }
        for warning in &module.warnings {
            let _ = writeln!(out, "- :warning: {}", text(&warning.message));
        }
        if let ChecksumView::Changes {
            rules,
            files,
            replace_paths,
        } = &module.checksum
        {
            let _ = writeln!(
                out,
                "- Checksum (rules for {}, verified on {}): changed by",
                rules.game_version, rules.verified_on
            );
            let matches: Vec<String> = replace_paths
                .iter()
                .map(|path| format!("`replace_path=\"{}\"`", cell(path)))
                .chain(files.iter().map(|file| format!("`{}`", cell(file))))
                .collect();
            for line in matches.iter().take(LISTED_MATCHES) {
                let _ = writeln!(out, "  - {line}");
            }
            if matches.len() > LISTED_MATCHES {
                let _ = writeln!(out, "  - and {} more", matches.len() - LISTED_MATCHES);
            }
        }
        if let Some(result) = result
            && !result.details.is_empty()
        {
            details_block(&mut out, "steamcmd output", &result.details.join("\n"));
        }
        match &metadata.description {
            Some(description) => {
                let source = metadata
                    .description_source
                    .as_deref()
                    .map(|source| format!(" from {}", text(source)))
                    .unwrap_or_default();
                details_block(
                    &mut out,
                    &format!("Description (BBCode{source}; replaces the Workshop description)"),
                    description,
                );
            }
            None => {
                let _ = writeln!(out, "- Description: unchanged");
            }
        }
        match &metadata.change_note {
            Some(note) => details_block(&mut out, "Change note (BBCode)", note),
            None => {
                let _ = writeln!(out, "- Change note: none");
            }
        }
    }
    out
}

fn item_link(module: &ModView, result: Option<&ResultView>) -> String {
    let id = result.and_then(|result| result.item_id).or(module.item.id);
    let url = result
        .and_then(|result| result.url.clone())
        .or_else(|| module.item.url.clone());
    match (id, url) {
        (Some(id), Some(url)) => format!("[{id}]({url})"),
        (Some(id), None) => id.to_string(),
        _ => "none yet".to_owned(),
    }
}

fn checksum_cell(checksum: &ChecksumView) -> String {
    match checksum {
        ChecksumView::Ignored => "ignored".to_owned(),
        ChecksumView::Unavailable { .. } => "not checked".to_owned(),
        ChecksumView::Unchanged { .. } => "unchanged".to_owned(),
        ChecksumView::Changes {
            files,
            replace_paths,
            ..
        } => format!(
            ":warning: changes ({} match{})",
            files.len() + replace_paths.len(),
            if files.len() + replace_paths.len() == 1 {
                ""
            } else {
                "es"
            }
        ),
    }
}

/// A collapsed block with `content` verbatim in a code fence.
fn details_block(out: &mut String, title: &str, content: &str) {
    let fence = fence(content);
    let _ = writeln!(
        out,
        "\n<details><summary>{}</summary>\n\n{fence}text\n{content}\n{fence}\n\n</details>\n",
        html(title)
    );
}

/// A backtick fence longer than any run of backticks in `content`.
fn fence(content: &str) -> String {
    let longest = content.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

/// Text for a Markdown table cell or a code span.
fn cell(text: &str) -> String {
    text.replace('|', "\\|")
        .replace('`', "'")
        .replace(['\n', '\r'], " ")
}

/// Plain text in Markdown.
fn text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(
            c,
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '|' | '#'
        ) {
            out.push('\\');
        }
        out.push(if c == '\n' { ' ' } else { c });
    }
    out
}

fn html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The workflow commands that annotate the run: a warning per plan
/// warning (file-level for checksum matches, up to [`ANNOTATED_FILES`] per
/// mod) and an error per failed mod.
pub fn annotations(plan: &PlanView, results: Option<&ResultsView>, run: &Run) -> Vec<String> {
    let mut out = Vec::new();
    for module in &plan.mods {
        let label = module.label();
        for warning in &module.warnings {
            if warning.code == "checksum"
                && let ChecksumView::Changes {
                    files,
                    replace_paths,
                    ..
                } = &module.checksum
            {
                let descriptor = module
                    .files
                    .iter()
                    .map(|file| file.path.as_str())
                    .find(|path| *path == "descriptor.mod" || *path == ".metadata/metadata.json");
                // One annotation on the descriptor stands for every replace_path.
                let on_descriptor = descriptor.filter(|_| !replace_paths.is_empty());
                if let Some(descriptor) = on_descriptor {
                    out.push(command(
                        "warning",
                        Some(&repo_path(&run.config, &module.path, descriptor)),
                        "Checksum change",
                        &format!(
                            "{label}: replace_path {} covers checksummed files; {}",
                            replace_paths.join(", "),
                            warning.message
                        ),
                    ));
                }
                let budget = ANNOTATED_FILES - usize::from(on_descriptor.is_some());
                for file in files.iter().take(budget) {
                    out.push(command(
                        "warning",
                        Some(&repo_path(&run.config, &module.path, file)),
                        "Checksum change",
                        &format!("{label}: {file} is a checksummed file; {}", warning.message),
                    ));
                }
                let unannotated = files.len().saturating_sub(budget)
                    + if on_descriptor.is_some() {
                        0
                    } else {
                        replace_paths.len()
                    };
                if unannotated > 0 {
                    out.push(command(
                        "warning",
                        None,
                        "Checksum change",
                        &format!(
                            "{label}: {} ({unannotated} more {} listed in the job summary)",
                            warning.message,
                            if unannotated == 1 {
                                "match is"
                            } else {
                                "matches are"
                            }
                        ),
                    ));
                }
                continue;
            }
            out.push(command(
                "warning",
                None,
                &format!("pdx-ws: {}", warning.code),
                &format!("{label}: {}", warning.message),
            ));
        }
    }
    if let Some(results) = results {
        for result in results
            .mods
            .iter()
            .filter(|result| result.outcome == "failed")
        {
            let label = match &result.id {
                Some(id) => format!("mod `{id}`"),
                None => format!("the mod at `{}`", result.path),
            };
            out.push(command(
                "error",
                None,
                "pdx-ws publish failed",
                &format!(
                    "{label}: {}",
                    result.error.as_deref().unwrap_or("the publish failed")
                ),
            ));
        }
    }
    out
}

/// `path` inside the mod at `mod_path`, relative to the repository root.
fn repo_path(config: &str, mod_path: &str, path: &str) -> String {
    let config = config.replace('\\', "/");
    let base = match config.rfind('/') {
        Some(index) => &config[..index],
        None => "",
    };
    [base, mod_path, path]
        .iter()
        .map(|part| part.trim_start_matches("./").trim_matches('/'))
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
}

/// A workflow command such as `::warning file=a.txt,title=T::message`.
fn command(kind: &str, file: Option<&str>, title: &str, message: &str) -> String {
    let mut properties = Vec::new();
    if let Some(file) = file {
        properties.push(format!("file={}", escape_property(file)));
    }
    properties.push(format!("title={}", escape_property(title)));
    format!(
        "::{kind} {}::{}",
        properties.join(","),
        escape_data(message)
    )
}

fn escape_data(text: &str) -> String {
    text.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn escape_property(text: &str) -> String {
    escape_data(text).replace(':', "%3A").replace(',', "%2C")
}

/// The `item-id` and `item-url` outputs: the selected mod's item when the
/// run has exactly one mod, else empty.
pub fn outputs(plan: &PlanView, results: Option<&ResultsView>) -> [(&'static str, String); 2] {
    let (id, url) = match &plan.mods[..] {
        [module] => {
            let result = results.and_then(|results| results.find(module));
            let id = result.and_then(|result| result.item_id).or(module.item.id);
            let url = result
                .and_then(|result| result.url.clone())
                .or_else(|| module.item.url.clone());
            (
                id.map(|id| id.to_string()).unwrap_or_default(),
                url.unwrap_or_default(),
            )
        }
        _ => (String::new(), String::new()),
    };
    [("item-id", id), ("item-url", url)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> PlanView {
        serde_json::from_str(PLAN).unwrap()
    }

    const PLAN: &str = r#"{
      "schema": 1,
      "mods": [{
        "id": "alpha", "game": "eu4", "app_id": 236850, "path": "mods/alpha",
        "item": {"id": 111, "source": "remote_file_id",
                 "url": "https://steamcommunity.com/sharedfiles/filedetails/?id=111"},
        "metadata": {"title": "Alpha | One", "tags": ["Gameplay"], "preview": null,
                     "visibility": null, "description": "[h1]A[/h1]\n```",
                     "description_source": "docs/a.md", "change_note": "Version 1.0.0",
                     "change_note_source": {"kind": "fallback"}},
        "files": [{"path": "common/ideas/a.txt", "size": 1},
                  {"path": "descriptor.mod", "size": 2}],
        "excluded": [],
        "checksum": {"status": "changes",
                     "rules": {"game_version": "1.37", "verified_on": "1.37.5", "source": "s"},
                     "files": ["common/ideas/a.txt"], "replace_paths": ["history/provinces"]},
        "warnings": [{"code": "checksum", "message": "the published copy changes the EU4 checksum: players lose achievements"},
                     {"code": "no-preview", "message": "there is no thumbnail.png, 100%"}],
        "write_backs": []
      }]
    }"#;

    const RESULTS: &str = r#"{
      "schema": 1,
      "mods": [{"id": "alpha", "path": "mods/alpha", "item_id": 111,
                "url": "https://steamcommunity.com/sharedfiles/filedetails/?id=111",
                "outcome": "failed", "error": "steamcmd could not build the Workshop item (exit 9)",
                "details": ["ERROR! Build for workshop item has no content"],
                "warnings": [], "write_backs": []}]
    }"#;

    fn run() -> Run {
        Run {
            config: "repo/workshop.toml".to_owned(),
            ..Run::default()
        }
    }

    #[test]
    fn annotations_point_at_the_checksum_matches() {
        let annotations = annotations(&plan(), None, &run());
        assert_eq!(
            annotations,
            [
                "::warning file=repo/mods/alpha/descriptor.mod,title=Checksum change::mod \
                 `alpha`: replace_path history/provinces covers checksummed files; the \
                 published copy changes the EU4 checksum: players lose achievements",
                "::warning file=repo/mods/alpha/common/ideas/a.txt,title=Checksum change::mod \
                 `alpha`: common/ideas/a.txt is a checksummed file; the published copy changes \
                 the EU4 checksum: players lose achievements",
                "::warning title=pdx-ws%3A no-preview::mod `alpha`: there is no thumbnail.png, \
                 100%25",
            ]
        );
    }

    #[test]
    fn many_matches_end_with_a_count() {
        let mut plan = plan();
        if let ChecksumView::Changes { files, .. } = &mut plan.mods[0].checksum {
            *files = (0..8)
                .map(|index| format!("common/ideas/{index}.txt"))
                .collect();
        }
        let annotations = annotations(&plan, None, &run());
        let files = annotations
            .iter()
            .filter(|line| line.starts_with("::warning file="))
            .count();
        assert_eq!(files, ANNOTATED_FILES);
        assert!(
            annotations[ANNOTATED_FILES]
                .ends_with("(4 more matches are listed in the job summary)"),
            "{annotations:?}"
        );
    }

    #[test]
    fn failed_mods_are_errors() {
        let results: ResultsView = serde_json::from_str(RESULTS).unwrap();
        let annotations = annotations(&plan(), Some(&results), &run());
        assert_eq!(
            annotations.last().unwrap(),
            "::error title=pdx-ws publish failed::mod `alpha`: steamcmd could not build the \
             Workshop item (exit 9)"
        );
    }

    #[test]
    fn paths_start_from_the_config_folder() {
        assert_eq!(repo_path("workshop.toml", ".", "a.txt"), "a.txt");
        assert_eq!(
            repo_path("./workshop.toml", "./mods/a", "b/c.txt"),
            "mods/a/b/c.txt"
        );
        assert_eq!(repo_path("x\\workshop.toml", ".", "a.txt"), "x/a.txt");
    }

    #[test]
    fn the_summary_shows_the_plan_and_the_results() {
        let results: ResultsView = serde_json::from_str(RESULTS).unwrap();
        let summary = summary(&plan(), Some(&results), &run());
        for expected in [
            "## pdx-ws publish\n",
            "| `alpha` | eu4 | [111](https://steamcommunity.com/sharedfiles/filedetails/?id=111) \
             | 2 | :warning: changes (2 matches) | failed |",
            "### `alpha`: Alpha \\| One\n",
            "- Result: **failed**: steamcmd could not build the Workshop item (exit 9)\n",
            "  - `replace_path=\"history/provinces\"`\n  - `common/ideas/a.txt`\n",
            "<summary>steamcmd output</summary>\n\n```text\nERROR! Build for workshop item \
             has no content\n```",
            "<summary>Description (BBCode from docs/a.md; replaces the Workshop \
             description)</summary>\n\n````text\n[h1]A[/h1]\n```\n````",
            "<summary>Change note (BBCode)</summary>\n\n```text\nVersion 1.0.0\n```",
        ] {
            assert!(summary.contains(expected), "{expected}\n---\n{summary}");
        }
    }

    #[test]
    fn a_dry_run_says_so() {
        let summary = summary(
            &plan(),
            None,
            &Run {
                dry_run: true,
                dry_run_reason: Some("pull_request events never publish".to_owned()),
                ..run()
            },
        );
        assert!(summary.starts_with(
            "## pdx-ws plan (dry run)\n\nDry run (pull_request events never publish): nothing \
             was sent to Steam.\n"
        ));
        assert!(summary.contains("| planned |"), "{summary}");

        let stopped = super::summary(
            &plan(),
            None,
            &Run {
                publish_exit: Some(3),
                ..run()
            },
        );
        assert!(
            stopped.contains("`pdx-ws publish` stopped with exit code 3 before it published"),
            "{stopped}"
        );
        assert!(stopped.contains("| not published |"), "{stopped}");
    }

    #[test]
    fn outputs_name_a_single_mod_only() {
        let [(id_key, id), (url_key, url)] = outputs(&plan(), None);
        assert_eq!((id_key, id.as_str()), ("item-id", "111"));
        assert_eq!(
            (url_key, url.as_str()),
            (
                "item-url",
                "https://steamcommunity.com/sharedfiles/filedetails/?id=111"
            )
        );

        let mut two = plan();
        two.mods.push(two.mods[0].clone());
        let [(_, id), (_, url)] = outputs(&two, None);
        assert_eq!((id.as_str(), url.as_str()), ("", ""));
    }
}
