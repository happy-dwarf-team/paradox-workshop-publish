//! The checksum check in the publish plan (spec section 8).

use std::fs;
use std::path::{Path, PathBuf};

use pdx_ws::Error;
use pdx_ws::config::Config;
use pdx_ws::plan::{ChecksumReport, ModPlan, Plan, PlanError, RulesUsed};

const EU4_EFFECT: &str = "players lose achievements while this mod is enabled, and multiplayer \
                          partners need the same mods; ironman itself still works";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn plan_dir(dir: &Path) -> Result<Plan, Error> {
    let config = Config::load(&dir.join("workshop.toml"))?;
    Plan::build(&config, &[])
}

/// A scratch mod in a temporary folder.
fn scratch(workshop_toml: &str, files: &[(&str, &[u8])]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("workshop.toml"), workshop_toml).unwrap();
    for (path, contents) in files {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    dir
}

/// Plans a one-mod EU4 scratch repository.
fn eu4(checksum: &str, descriptor: &str, files: &[(&str, &[u8])]) -> Result<ModPlan, Error> {
    let toml = format!("schema = 1\n[[mod]]\ngame = \"eu4\"\nchecksum = \"{checksum}\"\n");
    let mut all = vec![("descriptor.mod", descriptor.as_bytes())];
    all.extend_from_slice(files);
    let dir = scratch(&toml, &all);
    plan_dir(dir.path()).map(|mut plan| plan.mods.remove(0))
}

const DESCRIPTOR: &str = "name=\"A\"\nsupported_version=\"v1.37.*\"\n";

fn eu4_rules(fallback: Option<&str>) -> RulesUsed {
    RulesUsed {
        game_version: "1.37".to_owned(),
        verified_on: "1.37.5".to_owned(),
        source: "checksum_manifest.txt of an EU4 v1.37.5.0 Inca (491d) install (13 entries, \
                 SHA-256 2d7c64965363d0543ec61c512c4079ebbd0521acdfce9fde2590b2db1b6b7ff1), \
                 read on 2026-10-20; lab experiments E1 (files) and E5 (replace_path, issue #21)"
            .to_owned(),
        fallback: fallback.map(str::to_owned),
    }
}

#[test]
fn the_eu4_fixture_changes_the_checksum() {
    let config = Config::load(&fixture("eu4").join("workshop.toml")).unwrap();
    let plan = Plan::build(&config, &[]).unwrap();
    let module = &plan.mods[0];
    assert_eq!(
        module.checksum,
        ChecksumReport::Changes {
            rules: eu4_rules(None),
            files: vec!["common/ideas/fixture_ideas.txt".to_owned()],
            replace_paths: vec![
                "history/provinces".to_owned(),
                "common/bookmarks".to_owned()
            ],
        }
    );
    let warning = module
        .warnings
        .iter()
        .find(|warning| warning.code == "checksum")
        .unwrap();
    assert_eq!(
        warning.message,
        format!("the published copy changes the EU4 checksum: {EU4_EFFECT}")
    );

    let text = plan.to_string();
    assert!(
        text.contains(
            "  Checksum:       changes the EU4 checksum (rules for 1.37, verified on 1.37.5)\n\
             \x20   replace_path=\"history/provinces\"\n\
             \x20   replace_path=\"common/bookmarks\"\n\
             \x20   common/ideas/fixture_ideas.txt\n"
        ),
        "{text}"
    );
    assert!(text.contains(EU4_EFFECT), "{text}");
    assert!(!text.contains("achievements are kept"), "{text}");
}

#[test]
fn a_replace_path_alone_changes_the_checksum() {
    // E5: replace_path="common/client_states" changed the EU4 1.37.5
    // checksum although the mod shipped no file there.
    let plan = eu4(
        "warn",
        "name=\"A\"\nreplace_path=\"common/client_states\"\nsupported_version=\"v1.37.*\"\n",
        &[],
    )
    .unwrap();
    assert_eq!(
        plan.checksum,
        ChecksumReport::Changes {
            rules: eu4_rules(None),
            files: Vec::new(),
            replace_paths: vec!["common/client_states".to_owned()],
        }
    );

    // E5: replace_path="music" did not.
    let plan = eu4(
        "warn",
        "name=\"A\"\nreplace_path=\"music\"\nsupported_version=\"v1.37.*\"\n",
        &[],
    )
    .unwrap();
    assert_eq!(
        plan.checksum,
        ChecksumReport::Unchanged {
            rules: eu4_rules(None)
        }
    );
    assert!(
        plan.warnings
            .iter()
            .all(|warning| warning.code != "checksum")
    );
}

#[test]
fn files_outside_the_rules_keep_the_checksum() {
    let plan = eu4(
        "keep",
        DESCRIPTOR,
        &[
            // map/ counts its own files only, and not .dds.
            ("map/terrain/colormap.dds", b""),
            ("map/terrain/atlas.txt", b""),
            ("map/rivers.dds", b""),
            ("common/readme.md", b""),
            ("gfx/flags/SWE.tga", b""),
            ("interface/x.gui", b""),
            ("localisation/x_l_english.yml", b""),
            ("anvil/plugin.txt", b""),
        ],
    )
    .unwrap();
    assert_eq!(
        plan.checksum,
        ChecksumReport::Unchanged {
            rules: eu4_rules(None)
        }
    );
    let text = plan.to_string();
    assert!(
        text.contains(
            "  Checksum:       does not touch EU4 checksummed files (rules for 1.37, verified on 1.37.5)\n"
        ),
        "{text}"
    );
    assert!(!text.contains("achievements are kept"), "{text}");
}

#[test]
fn excluded_files_do_not_count() {
    let dir = scratch(
        "schema = 1\n[[mod]]\ngame = \"eu4\"\nchecksum = \"keep\"\nexclude = [\"common/**\"]\n",
        &[
            ("descriptor.mod", DESCRIPTOR.as_bytes()),
            ("common/ideas/a.txt", b""),
        ],
    );
    let plan = plan_dir(dir.path()).unwrap();
    assert!(matches!(
        plan.mods[0].checksum,
        ChecksumReport::Unchanged { .. }
    ));
}

#[test]
fn keep_fails_the_plan() {
    let error = eu4(
        "keep",
        "name=\"A\"\nreplace_path=\"history\"\n",
        &[("common/defines.lua", b""), ("map/default.map", b"")],
    )
    .unwrap_err();
    assert_eq!(error.exit_code(), 2);
    let Error::Plan { source, .. } = &error else {
        panic!("unexpected error: {error}");
    };
    assert!(matches!(source, PlanError::ChecksumKept { .. }));
    assert_eq!(
        error.to_string(),
        "the mod at `.`: `checksum = \"keep\"`, but the published copy changes the EU4 \
         checksum: replace_path=\"history\", common/defines.lua, map/default.map"
    );
}

#[test]
fn ignore_is_silent() {
    let plan = eu4("ignore", DESCRIPTOR, &[("common/defines.lua", b"")]).unwrap();
    assert_eq!(plan.checksum, ChecksumReport::Ignored);
    assert!(
        plan.warnings
            .iter()
            .all(|w| !w.code.starts_with("checksum"))
    );
    assert!(plan.to_string().contains("  Checksum:       ignored\n"));
}

#[test]
fn long_lists_end_with_and_n_more() {
    let names: Vec<String> = (0..20).map(|n| format!("events/e{n:02}.txt")).collect();
    let files: Vec<(&str, &[u8])> = names.iter().map(|name| (name.as_str(), &b""[..])).collect();

    let plan = eu4("warn", DESCRIPTOR, &files).unwrap();
    let text = plan.to_string();
    assert!(
        text.contains("    events/e14.txt\n    and 5 more\n"),
        "{text}"
    );
    assert!(!text.contains("events/e15.txt"), "{text}");

    let error = eu4("keep", DESCRIPTOR, &files).unwrap_err().to_string();
    assert!(
        error.ends_with("events/e13.txt, events/e14.txt and 5 more"),
        "{error}"
    );
}

#[test]
fn without_a_matching_table_the_newest_is_used() {
    let fallback = "no EU4 rules cover supported_version \"v1.36.*\"; the newest are used";
    let plan = eu4(
        "warn",
        "name=\"A\"\nsupported_version=\"v1.36.*\"\n",
        &[("common/a.txt", b"")],
    )
    .unwrap();
    assert_eq!(
        plan.checksum,
        ChecksumReport::Changes {
            rules: eu4_rules(Some(fallback)),
            files: vec!["common/a.txt".to_owned()],
            replace_paths: Vec::new(),
        }
    );
    let text = plan.to_string();
    assert!(
        text.contains(&format!("\n                  {fallback}\n")),
        "{text}"
    );

    let plan = eu4("warn", "name=\"A\"\n", &[]).unwrap();
    assert_eq!(
        plan.checksum,
        ChecksumReport::Unchanged {
            rules: eu4_rules(Some(
                "the descriptor has no supported_version; the newest EU4 rules are used"
            ))
        }
    );
}

#[test]
fn other_games_are_not_checked_yet() {
    let dir = scratch(
        "schema = 1\n[[mod]]\ngame = \"vic3\"\nchecksum = \"keep\"\n",
        &[
            (".metadata/metadata.json", br#"{"name": "A"}"#),
            ("common/a.txt", b""),
        ],
    );
    let plan = plan_dir(dir.path()).unwrap();
    let module = &plan.mods[0];
    let reason = "the checksum check is not available for Victoria 3 yet";
    assert_eq!(
        module.checksum,
        ChecksumReport::Unavailable {
            reason: reason.to_owned()
        }
    );
    let warning = module
        .warnings
        .iter()
        .find(|warning| warning.code == "checksum-unavailable")
        .unwrap();
    assert_eq!(
        warning.message,
        format!("`checksum = \"keep\"` is not enforced: {reason}")
    );
    assert!(
        plan.to_string()
            .contains(&format!("  Checksum:       not checked ({reason})\n"))
    );

    // With the default `warn` there is nothing to enforce, so no warning.
    let vic3 = plan_dir(&fixture("vic3")).unwrap();
    assert!(matches!(
        vic3.mods[0].checksum,
        ChecksumReport::Unavailable { .. }
    ));
    assert!(
        vic3.mods[0]
            .warnings
            .iter()
            .all(|warning| warning.code != "checksum-unavailable")
    );
}

#[test]
fn the_json_report_is_tagged_by_status() {
    let plan = plan_dir(&fixture("eu4")).unwrap();
    let json = serde_json::to_value(&plan).unwrap();
    let checksum = &json["mods"][0]["checksum"];
    assert_eq!(checksum["status"], "changes");
    assert_eq!(checksum["rules"]["game_version"], "1.37");
    assert_eq!(checksum["rules"]["verified_on"], "1.37.5");
    assert!(checksum["rules"].get("fallback").is_none());
    assert_eq!(checksum["files"][0], "common/ideas/fixture_ideas.txt");
    assert_eq!(checksum["replace_paths"][1], "common/bookmarks");

    // `alpha` keeps the checksum and has no supported_version.
    let multi = plan_dir(&fixture("multi")).unwrap();
    let json = serde_json::to_value(&multi).unwrap();
    let checksum = &json["mods"][0]["checksum"];
    assert_eq!(checksum["status"], "unchanged");
    assert_eq!(
        checksum["rules"]["fallback"],
        "the descriptor has no supported_version; the newest EU4 rules are used"
    );
}
