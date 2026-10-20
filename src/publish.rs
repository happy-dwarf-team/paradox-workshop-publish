//! Publishing the selected mods one after another (spec section 3, issue
//! #20): a failed mod does not stop the others unless `--fail-fast` is
//! given, and every mod gets a result.

use std::fmt;

use serde::Serialize;

use crate::copy::PublishedCopy;
use crate::exit;
use crate::item_id::ItemId;
use crate::plan::{ModPlan, Plan, Warning, WriteBack};
use crate::steamcmd::session::Credentials;
use crate::steamcmd::{self, PublishOptions, Published, Steamcmd};

/// The version of the `publish --json` format.
pub const JSON_SCHEMA: u32 = 1;

/// The results of one `publish` run, in plan order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub schema: u32,
    pub mods: Vec<ModResult>,
}

/// What happened to one mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModResult {
    pub id: Option<String>,
    /// The mod root relative to the folder of `workshop.toml`.
    pub path: String,
    /// The item published to; `None` when no item exists.
    pub item_id: Option<ItemId>,
    pub url: Option<String>,
    pub outcome: Outcome,
    /// Why the mod failed or was skipped.
    pub error: Option<String>,
    /// Masked lines from steamcmd and its logs that explain a failure.
    pub details: Vec<String>,
    /// The plan's warnings for this mod.
    pub warnings: Vec<Warning>,
    /// What to commit now that the item exists (`created` only).
    pub write_backs: Vec<WriteBack>,
    /// The exit code a failure stands for.
    #[serde(skip)]
    pub exit_code: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The existing item was updated.
    Published,
    /// A new item was created and published (`--allow-create`).
    Created,
    Failed,
    /// Not attempted after an earlier failure.
    Skipped,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Published => "published",
            Outcome::Created => "created",
            Outcome::Failed => "failed",
            Outcome::Skipped => "skipped",
        }
    }
}

impl Report {
    /// 0 when every mod was published, 3 when Steam rejected the session,
    /// else 1 when any mod failed.
    pub fn exit_code(&self) -> u8 {
        let failed = self
            .mods
            .iter()
            .filter(|result| result.outcome == Outcome::Failed);
        let mut code = exit::SUCCESS;
        for result in failed {
            if result.exit_code == exit::SESSION {
                return exit::SESSION;
            }
            code = exit::PUBLISH_FAILED;
        }
        code
    }

    /// How many mods had `outcome`.
    pub fn count(&self, outcome: Outcome) -> usize {
        self.mods
            .iter()
            .filter(|result| result.outcome == outcome)
            .count()
    }
}

/// What [`run`] needs from a mode's error.
pub trait PublishError: fmt::Display {
    /// The exit code the failure stands for.
    fn exit_code(&self) -> u8;
    /// Lines that explain the failure.
    fn details(&self) -> &[String] {
        &[]
    }
}

impl PublishError for steamcmd::Error {
    fn exit_code(&self) -> u8 {
        steamcmd::Error::exit_code(self)
    }

    fn details(&self) -> &[String] {
        steamcmd::Error::details(self)
    }
}

impl PublishError for crate::client::Error {
    fn exit_code(&self) -> u8 {
        crate::client::Error::exit_code(self)
    }
}

/// Publishes the plan's mods in order with `publish`. After a failure the
/// remaining mods are still published, unless `fail_fast` is set or Steam
/// rejected the session, which no later mod could get past.
pub fn run<E: PublishError>(
    plan: &Plan,
    fail_fast: bool,
    mut publish: impl FnMut(&ModPlan) -> Result<Published, E>,
) -> Report {
    let mut mods = Vec::with_capacity(plan.mods.len());
    let mut stop: Option<String> = None;
    for module in &plan.mods {
        if let Some(reason) = &stop {
            mods.push(result(module, Outcome::Skipped, Some(reason.clone())));
            continue;
        }
        match publish(module) {
            Ok(published) => {
                let outcome = if published.created {
                    Outcome::Created
                } else {
                    Outcome::Published
                };
                let mut result = result(module, outcome, None);
                result.item_id = Some(published.item_id);
                result.url = Some(published.item_id.url());
                if published.created {
                    result.write_backs = module.write_backs.clone();
                }
                mods.push(result);
            }
            Err(error) => {
                let exit_code = error.exit_code();
                if exit_code == exit::SESSION {
                    stop = Some(format!(
                        "not attempted: Steam rejected the session for {}",
                        module.label()
                    ));
                } else if fail_fast {
                    stop = Some(format!(
                        "not attempted: --fail-fast stopped after {} failed",
                        module.label()
                    ));
                }
                let mut result = result(module, Outcome::Failed, Some(error.to_string()));
                result.details = error.details().to_vec();
                result.exit_code = exit_code;
                mods.push(result);
            }
        }
    }
    Report {
        schema: JSON_SCHEMA,
        mods,
    }
}

fn result(module: &ModPlan, outcome: Outcome, error: Option<String>) -> ModResult {
    ModResult {
        id: module.id.clone(),
        path: module.path.clone(),
        item_id: module.item.id,
        url: module.item.url.clone(),
        outcome,
        error,
        details: Vec::new(),
        warnings: module.warnings.clone(),
        write_backs: Vec::new(),
        exit_code: exit::SUCCESS,
    }
}

/// Publishes one mod's published copy through `steamcmd`; the copy is
/// deleted afterwards.
pub fn through_steamcmd(
    steamcmd: &Steamcmd,
    credentials: &Credentials,
    module: &ModPlan,
    options: &PublishOptions,
    log: &mut dyn FnMut(&str),
) -> Result<Published, steamcmd::Error> {
    let copy = PublishedCopy::create(module)?;
    steamcmd.publish(credentials, module, copy.path(), options, log)
}

impl ModResult {
    /// How the results table names this mod.
    fn name(&self) -> &str {
        self.id.as_deref().unwrap_or(&self.path)
    }
}

impl fmt::Display for Report {
    /// One row per mod, then the error, details and write-backs under the
    /// rows that have them.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let width = self
            .mods
            .iter()
            .map(|result| result.name().chars().count())
            .chain(std::iter::once("Mod".len()))
            .max()
            .unwrap_or(0);
        writeln!(f, "{:width$}  {:9}  Workshop item", "Mod", "Result")?;
        for result in &self.mods {
            let item = result.url.as_deref().unwrap_or("none");
            writeln!(
                f,
                "{:width$}  {:9}  {item}",
                result.name(),
                result.outcome.as_str()
            )?;
            let indent = " ".repeat(width + 2);
            if let Some(error) = &result.error {
                writeln!(f, "{indent}{error}")?;
            }
            for detail in &result.details {
                writeln!(f, "{indent}  {detail}")?;
            }
            if let Some(item_id) = result.item_id {
                for write_back in &result.write_backs {
                    writeln!(
                        f,
                        "{indent}set {} to {item_id} in {} and commit it",
                        write_back.key, write_back.file
                    )?;
                }
            }
        }
        writeln!(
            f,
            "{} published, {} created, {} failed, {} skipped",
            self.count(Outcome::Published),
            self.count(Outcome::Created),
            self.count(Outcome::Failed),
            self.count(Outcome::Skipped)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    /// Three EU4 mods with items 101, 102 and 103 in a scratch folder.
    fn three_mods() -> (tempfile::TempDir, Plan) {
        let dir = tempfile::tempdir().unwrap();
        let mut toml = "schema = 1\n[defaults]\ngame = \"eu4\"\n".to_owned();
        for (index, id) in ["one", "two", "three"].iter().enumerate() {
            toml.push_str(&format!("[[mod]]\nid = \"{id}\"\npath = \"{id}\"\n"));
            let folder = dir.path().join(id);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(
                folder.join("descriptor.mod"),
                format!("name=\"{id}\"\nremote_file_id=\"{}\"\n", 101 + index),
            )
            .unwrap();
        }
        std::fs::write(dir.path().join("workshop.toml"), toml).unwrap();
        let config = Config::load(&dir.path().join("workshop.toml")).unwrap();
        let plan = Plan::build(&config, &[]).unwrap();
        (dir, plan)
    }

    fn published(module: &ModPlan) -> Result<Published, steamcmd::Error> {
        Ok(Published {
            item_id: module.item.id.unwrap(),
            created: false,
        })
    }

    fn failing(
        name: &'static str,
        error: fn() -> steamcmd::Error,
    ) -> impl FnMut(&ModPlan) -> Result<Published, steamcmd::Error> {
        move |module| {
            if module.id.as_deref() == Some(name) {
                Err(error())
            } else {
                published(module)
            }
        }
    }

    fn build_failure() -> steamcmd::Error {
        steamcmd::Error::Failed {
            code: Some(9),
            summary: "steamcmd could not build the Workshop item (exit 9)".to_owned(),
            details: vec!["ERROR! Build for workshop item has no content".to_owned()],
        }
    }

    fn rejected() -> steamcmd::Error {
        steamcmd::Error::SessionRejected {
            reason: "Cached credentials not found".to_owned(),
        }
    }

    fn outcomes(report: &Report) -> Vec<Outcome> {
        report.mods.iter().map(|result| result.outcome).collect()
    }

    #[test]
    fn every_mod_is_published_in_order() {
        let (_dir, plan) = three_mods();
        let mut order = Vec::new();
        let report = run(&plan, false, |module| {
            order.push(module.id.clone().unwrap());
            published(module)
        });
        assert_eq!(order, ["one", "two", "three"]);
        assert_eq!(outcomes(&report), [Outcome::Published; 3]);
        assert_eq!(report.exit_code(), 0);
        assert_eq!(
            report.mods[1].url.as_deref(),
            Some("https://steamcommunity.com/sharedfiles/filedetails/?id=102")
        );
    }

    #[test]
    fn a_failure_does_not_stop_the_others() {
        let (_dir, plan) = three_mods();
        let report = run(&plan, false, failing("two", build_failure));
        assert_eq!(
            outcomes(&report),
            [Outcome::Published, Outcome::Failed, Outcome::Published]
        );
        assert_eq!(report.exit_code(), 1);
        let two = &report.mods[1];
        assert_eq!(
            two.error.as_deref(),
            Some("steamcmd could not build the Workshop item (exit 9)")
        );
        assert_eq!(
            two.details,
            ["ERROR! Build for workshop item has no content"]
        );
        assert_eq!(two.item_id.map(ItemId::get), Some(102));
    }

    #[test]
    fn fail_fast_skips_the_rest() {
        let (_dir, plan) = three_mods();
        let report = run(&plan, true, failing("two", build_failure));
        assert_eq!(
            outcomes(&report),
            [Outcome::Published, Outcome::Failed, Outcome::Skipped]
        );
        assert_eq!(
            report.mods[2].error.as_deref(),
            Some("not attempted: --fail-fast stopped after mod `two` failed")
        );
        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn a_rejected_session_stops_the_run_with_3() {
        let (_dir, plan) = three_mods();
        let report = run(&plan, false, failing("one", rejected));
        assert_eq!(
            outcomes(&report),
            [Outcome::Failed, Outcome::Skipped, Outcome::Skipped]
        );
        assert_eq!(report.exit_code(), 3);
    }

    #[test]
    fn created_items_carry_their_write_backs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("workshop.toml"),
            "schema = 1\n[[mod]]\ngame = \"eu4\"\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("descriptor.mod"), "name=\"A\"\n").unwrap();
        let config = Config::load(&dir.path().join("workshop.toml")).unwrap();
        let plan = Plan::build(&config, &[]).unwrap();
        let report = run(&plan, false, |_| {
            Ok::<_, steamcmd::Error>(Published {
                item_id: ItemId::try_from(555).unwrap(),
                created: true,
            })
        });
        let result = &report.mods[0];
        assert_eq!(result.outcome, Outcome::Created);
        assert_eq!(result.item_id.map(ItemId::get), Some(555));
        assert_eq!(result.write_backs[0].key, "remote_file_id");
        assert_eq!(
            report.to_string(),
            "Mod  Result     Workshop item\n\
             .    created    https://steamcommunity.com/sharedfiles/filedetails/?id=555\n     \
             set remote_file_id to 555 in descriptor.mod and commit it\n\
             0 published, 1 created, 0 failed, 0 skipped\n"
        );
    }

    #[test]
    fn the_table_and_json_show_every_mod() {
        let (_dir, plan) = three_mods();
        let report = run(&plan, true, failing("two", build_failure));
        let url = |id| format!("https://steamcommunity.com/sharedfiles/filedetails/?id={id}");
        assert_eq!(
            report.to_string(),
            format!(
                "Mod    Result     Workshop item\n\
                 one    published  {}\n\
                 two    failed     {}\n       \
                 steamcmd could not build the Workshop item (exit 9)\n         \
                 ERROR! Build for workshop item has no content\n\
                 three  skipped    {}\n       \
                 not attempted: --fail-fast stopped after mod `two` failed\n\
                 1 published, 0 created, 1 failed, 1 skipped\n",
                url(101),
                url(102),
                url(103)
            )
        );

        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["schema"], 1);
        let two = &json["mods"][1];
        assert_eq!(two["id"], "two");
        assert_eq!(two["item_id"], 102);
        assert_eq!(two["url"], url(102));
        assert_eq!(two["outcome"], "failed");
        assert_eq!(
            two["error"],
            "steamcmd could not build the Workshop item (exit 9)"
        );
        assert_eq!(two["warnings"][0]["code"], "no-preview");
        assert!(two.get("exit_code").is_none());
        assert_eq!(json["mods"][0]["error"], serde_json::Value::Null);
    }
}
