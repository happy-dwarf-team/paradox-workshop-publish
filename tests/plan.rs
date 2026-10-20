//! The publish plan and the published copy of the fixture mods.

use std::fs;
use std::path::{Path, PathBuf};

use pdx_ws::Error;
use pdx_ws::config::Config;
use pdx_ws::copy::PublishedCopy;
use pdx_ws::plan::{ChecksumReport, ModPlan, Plan, PlanError, WriteBack};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn plan_dir(dir: &Path, selection: &[&str]) -> Result<Plan, Error> {
    let config = Config::load(&dir.join("workshop.toml"))?;
    let selection: Vec<String> = selection.iter().map(|id| id.to_string()).collect();
    Plan::build(&config, &selection)
}

fn plan_one(name: &str) -> ModPlan {
    let mut plan = plan_dir(&fixture(name), &[]).unwrap();
    assert_eq!(plan.mods.len(), 1);
    plan.mods.remove(0)
}

fn paths(plan: &ModPlan) -> Vec<&str> {
    plan.files.iter().map(|file| file.path.as_str()).collect()
}

fn codes(plan: &ModPlan) -> Vec<&str> {
    plan.warnings.iter().map(|warning| warning.code).collect()
}

#[test]
fn eu4_plan_lists_the_published_copy() {
    let plan = plan_one("eu4");
    assert_eq!(
        paths(&plan),
        [
            "common/ideas/fixture_ideas.txt",
            "descriptor.mod",
            "thumbnail.png"
        ]
    );
    assert_eq!(
        plan.excluded,
        [".github/", "docs/", "upload.vdf", "workshop.toml"]
    );
    let descriptor = &plan.files[1];
    let rewrite = descriptor.rewrite.as_ref().unwrap();
    assert_eq!(rewrite.removed, ["path"]);
    assert!(rewrite.added.is_empty());
    let text = String::from_utf8(descriptor.contents.clone().unwrap()).unwrap();
    assert!(
        !text.lines().any(|line| line.starts_with("path=")),
        "{text}"
    );
    assert!(text.contains("remote_file_id=\"2948217283\""), "{text}");
    assert_eq!(descriptor.size, text.len() as u64);

    assert_eq!(plan.item.id.map(|id| id.get()), Some(2948217283));
    assert_eq!(
        plan.item.url.as_deref(),
        Some("https://steamcommunity.com/sharedfiles/filedetails/?id=2948217283")
    );
    assert_eq!(plan.metadata.title, "Fixture Mod");
    assert_eq!(plan.metadata.tags, ["Gameplay", "Missions and Decisions"]);
    let preview = plan.metadata.preview.as_ref().unwrap();
    assert_eq!((preview.path.as_str(), preview.size), ("thumbnail.png", 69));
    assert!(plan.write_backs.is_empty());
    assert!(matches!(plan.checksum, ChecksumReport::Changes { .. }));
    assert_eq!(codes(&plan), ["not-implemented", "checksum"]);
}

#[test]
fn a_new_eu4_item_plans_the_write_back() {
    let plan = plan_one("eu4-unpublished");
    assert_eq!(plan.item.id, None);
    assert_eq!(
        plan.write_backs,
        [WriteBack {
            file: "descriptor.mod".to_owned(),
            key: "remote_file_id",
        }]
    );
    assert_eq!(codes(&plan), ["no-preview", "no-item-id"]);
    // Nothing to change, so the descriptor is copied as it is.
    assert_eq!(plan.files[0].rewrite, None);
}

#[test]
fn metadata_json_games_keep_the_descriptor_and_write_back_item_id() {
    let vic3 = plan_one("vic3");
    assert_eq!(paths(&vic3), [".metadata/metadata.json"]);
    assert_eq!(vic3.files[0].rewrite, None);
    assert_eq!(vic3.item.id.map(|id| id.get()), Some(3123456789));
    assert!(matches!(vic3.checksum, ChecksumReport::Unavailable { .. }));

    let eu5 = plan_one("eu5");
    assert_eq!(
        eu5.write_backs,
        [WriteBack {
            file: "workshop.toml".to_owned(),
            key: "item_id",
        }]
    );
}

#[test]
fn mods_are_selected_by_id_in_file_order() {
    let dir = fixture("multi");
    let all = plan_dir(&dir, &[]).unwrap();
    let ids: Vec<_> = all.mods.iter().map(|m| m.id.as_deref().unwrap()).collect();
    assert_eq!(ids, ["alpha", "beta"]);
    assert_eq!(all.mods[1].path, "mods/beta");
    assert_eq!(all.mods[1].write_backs[0].file, "mods/beta/descriptor.mod");

    let selected = plan_dir(&dir, &["beta", "alpha", "beta"]).unwrap();
    let ids: Vec<_> = selected
        .mods
        .iter()
        .map(|m| m.id.as_deref().unwrap())
        .collect();
    assert_eq!(ids, ["alpha", "beta"]);

    let error = plan_dir(&dir, &["gamma"]).unwrap_err();
    assert_eq!(
        error.to_string(),
        "no mod has the id `gamma`; workshop.toml has `alpha`, `beta`"
    );
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn the_published_copy_matches_the_plan() {
    let plan = plan_one("eu4");
    let copy = PublishedCopy::create(&plan).unwrap();
    let root = copy.path().to_owned();
    for file in &plan.files {
        let written = fs::read(root.join(&file.path)).unwrap();
        assert_eq!(written.len() as u64, file.size, "{}", file.path);
    }
    assert!(!root.join("docs").exists());
    assert!(!root.join("workshop.toml").exists());
    let descriptor = fs::read_to_string(root.join("descriptor.mod")).unwrap();
    assert!(!descriptor.lines().any(|line| line.starts_with("path=")));
    drop(copy);
    assert!(!root.exists(), "the temporary folder is deleted");
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

fn plan_error(dir: &Path) -> PlanError {
    match plan_dir(dir, &[]).unwrap_err() {
        Error::Plan { source, .. } => source,
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn a_preview_of_1_mb_is_an_error() {
    let big = vec![0u8; 1024 * 1024];
    let dir = scratch(
        "schema = 1\n[[mod]]\ngame = \"eu4\"\n",
        &[("descriptor.mod", b"name=\"A\"\n"), ("thumbnail.png", &big)],
    );
    let error = plan_error(dir.path());
    assert!(matches!(
        error,
        PlanError::PreviewTooLarge { size: 1048576, .. }
    ));

    let just_under = vec![0u8; 1024 * 1024 - 1];
    fs::write(dir.path().join("thumbnail.png"), just_under).unwrap();
    assert!(plan_dir(dir.path(), &[]).is_ok());
}

#[test]
fn excluding_the_descriptor_is_an_error() {
    let dir = scratch(
        "schema = 1\n[[mod]]\ngame = \"eu4\"\nexclude = [\"*.mod\"]\n",
        &[("descriptor.mod", b"name=\"A\"\n")],
    );
    assert!(matches!(
        plan_error(dir.path()),
        PlanError::DescriptorExcluded("descriptor.mod")
    ));
}

#[test]
fn invalid_globs_are_validation_errors() {
    let dir = scratch(
        "schema = 1\n[[mod]]\ngame = \"eu4\"\nexclude = [\"gfx/[\"]\n",
        &[("descriptor.mod", b"name=\"A\"\n")],
    );
    let error = plan_dir(dir.path(), &[]).unwrap_err();
    assert!(
        error.to_string().contains("`gfx/[` is not a valid glob"),
        "{error}"
    );
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn tag_limits_are_warnings() {
    let dir = scratch(
        "schema = 1\n[[mod]]\ngame = \"vic3\"\n",
        &[(
            ".metadata/metadata.json",
            br#"{"name": "A", "tags": ["a", "b", "c", "d", "e", "f"]}"#,
        )],
    );
    let plan = plan_dir(dir.path(), &[]).unwrap();
    let warning = &plan.mods[0].warnings[1];
    assert_eq!(warning.code, "tag-limit");
    assert_eq!(
        warning.message,
        "the descriptor has 6 tags; the Victoria 3 launcher allows at most 5"
    );
}
