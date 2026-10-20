//! `steamcmd` mode against a fake steamcmd that replays E4's real outputs.
//!
//! This test binary is its own fake: copied into a scratch steamcmd folder
//! under steamcmd's file name, it reads `scenario` next to itself (one
//! scenario, or one per item ID), records what it was given in `seen/`,
//! prints the transcript and exits with its code. Nothing here talks to
//! Steam.

use std::fs;
use std::io::Read;
use std::panic;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use pdx_ws::change_note::ChangeNoteInput;
use pdx_ws::config::Config;
use pdx_ws::copy::PublishedCopy;
use pdx_ws::plan::{ModPlan, Plan};
use pdx_ws::publish::{self, Outcome as ModOutcome, Report};
use pdx_ws::steamcmd::install::EXECUTABLE;
use pdx_ws::steamcmd::session::{self, Credentials};
use pdx_ws::steamcmd::{Error, PublishOptions, Published, Steamcmd};

const USER: &str = "pdx-ws-test-user";
const SESSION: &[u8] = b"\"InstallConfigStore\"\n{\n\t\"fake\" \"session\"\n}\n";
const CREATED_ID: &str = "3823257732";

fn main() -> ExitCode {
    let exe = std::env::current_exe().expect("current exe");
    if exe.file_name().and_then(|name| name.to_str()) == Some(EXECUTABLE) {
        return fake_steamcmd(&exe);
    }
    run_tests(&[
        (
            "updates_an_item_and_cleans_up",
            updates_an_item_and_cleans_up,
        ),
        (
            "creates_an_item_only_when_allowed",
            creates_an_item_only_when_allowed,
        ),
        (
            "a_build_failure_shows_the_logs",
            a_build_failure_shows_the_logs,
        ),
        (
            "a_missing_session_exits_with_3",
            a_missing_session_exits_with_3,
        ),
        (
            "exit_0_without_success_is_a_failure",
            exit_0_without_success_is_a_failure,
        ),
        (
            "a_hanging_steamcmd_is_stopped",
            a_hanging_steamcmd_is_stopped,
        ),
        (
            "the_self_update_accepts_exit_7",
            the_self_update_accepts_exit_7,
        ),
        ("every_mod_is_published", every_mod_is_published),
        (
            "a_failed_mod_does_not_stop_the_others",
            a_failed_mod_does_not_stop_the_others,
        ),
        (
            "fail_fast_stops_at_the_first_failure",
            fail_fast_stops_at_the_first_failure,
        ),
    ])
}

/// A tiny runner, because `harness = false` has no libtest.
fn run_tests(tests: &[(&str, fn())]) -> ExitCode {
    let filter = std::env::args().skip(1).find(|arg| !arg.starts_with('-'));
    let selected: Vec<_> = tests
        .iter()
        .filter(|(name, _)| filter.as_deref().is_none_or(|filter| name.contains(filter)))
        .collect();
    println!("\nrunning {} tests", selected.len());
    let mut failed = Vec::new();
    for (name, test) in &selected {
        let ok = panic::catch_unwind(test).is_ok();
        println!("test {name} ... {}", if ok { "ok" } else { "FAILED" });
        if !ok {
            failed.push(*name);
        }
    }
    let result = if failed.is_empty() { "ok" } else { "FAILED" };
    println!(
        "\ntest result: {result}. {} passed; {} failed\n",
        selected.len() - failed.len(),
        failed.len()
    );
    if failed.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

// ---------------------------------------------------------------------------
// The fake

/// Where real steamcmd reads the cached login.
fn fake_session_path(folder: &Path) -> PathBuf {
    if cfg!(target_os = "linux") {
        PathBuf::from(std::env::var_os("HOME").expect("HOME is set"))
            .join("Steam/config/config.vdf")
    } else {
        folder.join("config/config.vdf")
    }
}

fn fake_steamcmd(exe: &Path) -> ExitCode {
    let folder = exe.parent().unwrap();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["+quit"] {
        // The first self-update on Windows exits with 7.
        println!("Steam Console Client (c) Valve Corporation - version 1791918946");
        return ExitCode::from(7);
    }
    let [login, user, build, vdf, quit] = &args[..] else {
        eprintln!("fake steamcmd: unexpected arguments {args:?}");
        return ExitCode::from(100);
    };
    assert_eq!(
        (login.as_str(), build.as_str(), quit.as_str()),
        ("+login", "+workshop_build_item", "+quit")
    );

    let seen = folder.join("seen");
    fs::create_dir_all(&seen).unwrap();
    let mut stdin = Vec::new();
    let stdin_closed = std::io::stdin()
        .read_to_end(&mut stdin)
        .is_ok_and(|n| n == 0);
    fs::write(
        seen.join("stdin"),
        if stdin_closed { "closed" } else { "open" },
    )
    .unwrap();
    if let Ok(session) = fs::read(fake_session_path(folder)) {
        fs::write(seen.join("session"), session).unwrap();
    }
    let vdf_text = fs::read_to_string(vdf).unwrap();
    fs::write(seen.join("item.vdf"), &vdf_text).unwrap();

    let scenario = fs::read_to_string(folder.join("scenario")).unwrap();
    let scenario = pick_scenario(&scenario, &vdf_text);
    let scenario = scenario.as_str();
    let attempted = seen.join("attempted");
    let mut list = fs::read_to_string(&attempted).unwrap_or_default();
    list.push_str(&format!("{}\n", published_file_id(&vdf_text)));
    fs::write(&attempted, list).unwrap();
    if scenario == "hang" {
        std::thread::sleep(Duration::from_secs(60));
        return ExitCode::from(0);
    }
    let transcript = fs::read_to_string(fixtures().join(format!("{scenario}.txt"))).unwrap();
    let (first, body) = transcript.split_once('\n').unwrap();
    let code: u8 = first.trim().strip_prefix("exit=").unwrap().parse().unwrap();
    print!("{}", body.replace("{user}", user).replace("^[", "\u{1b}"));

    match scenario {
        "create" => {
            let updated = vdf_text.replace(
                "\"publishedfileid\" \"0\"",
                &format!("\"publishedfileid\" \"{CREATED_ID}\""),
            );
            fs::write(vdf, updated).unwrap();
        }
        "no-content" => {
            let logs = folder.join("logs");
            fs::create_dir_all(&logs).unwrap();
            fs::copy(
                fixtures().join("depot_build_236850.log"),
                logs.join("depot_build_236850.log"),
            )
            .unwrap();
        }
        _ => {}
    }
    ExitCode::from(code)
}

/// `scenario` is one scenario name, or lines of `<publishedfileid>=<name>`
/// that pick one per item.
fn pick_scenario(scenario: &str, vdf: &str) -> String {
    let scenario = scenario.trim();
    if !scenario.contains('=') {
        return scenario.to_owned();
    }
    let id = published_file_id(vdf);
    scenario
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{id}=")))
        .unwrap_or_else(|| panic!("fake steamcmd: no scenario for item {id}"))
        .trim()
        .to_owned()
}

fn published_file_id(vdf: &str) -> String {
    vdf.lines()
        .find_map(|line| line.trim().strip_prefix("\"publishedfileid\""))
        .map(|value| value.trim().trim_matches('"').to_owned())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Helpers

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/steamcmd")
}

fn mod_plan(fixture: &str) -> ModPlan {
    mod_plan_with(fixture, &ChangeNoteInput::default())
}

fn mod_plan_with(fixture: &str, change_note: &ChangeNoteInput) -> ModPlan {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture)
        .join("workshop.toml");
    let config = Config::load(&path).unwrap();
    Plan::build_with(&config, &[], change_note)
        .unwrap()
        .mods
        .remove(0)
}

/// A scratch steamcmd folder holding a copy of this binary as steamcmd.
fn fake_install(scenario: &str) -> Steamcmd {
    let root = tempfile::Builder::new()
        .prefix("pdx-ws-fake-steamcmd-")
        .tempdir()
        .unwrap();
    let folder = root.path().join("steamcmd");
    fs::create_dir_all(&folder).unwrap();
    let executable = folder.join(EXECUTABLE);
    fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
    fs::write(folder.join("scenario"), scenario).unwrap();
    Steamcmd::from_installed(root, executable)
}

fn seen(steamcmd: &Steamcmd, name: &str) -> Option<String> {
    fs::read_to_string(steamcmd.root().join("steamcmd/seen").join(name)).ok()
}

fn credentials() -> Credentials {
    Credentials::new(USER, &session::encode(SESSION)).unwrap()
}

struct Outcome {
    result: Result<Published, Error>,
    log: Vec<String>,
}

fn publish(steamcmd: &Steamcmd, plan: &ModPlan, options: &PublishOptions) -> Outcome {
    let copy = PublishedCopy::create(plan).unwrap();
    let mut log = Vec::new();
    let result = steamcmd.publish(&credentials(), plan, copy.path(), options, &mut |line| {
        log.push(line.to_owned())
    });
    Outcome { result, log }
}

fn vdf_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn assert_masked(log: &[String]) {
    for line in log {
        assert!(!line.contains("12345678"), "SteamID not masked: {line}");
        assert!(!line.contains(USER), "user name not masked: {line}");
        assert!(!line.contains('\u{1b}'), "ANSI code left: {line:?}");
    }
    assert!(
        log.iter().any(|line| line.contains("'***' [U:1:***]")),
        "{log:?}"
    );
}

// ---------------------------------------------------------------------------
// Tests

fn updates_an_item_and_cleans_up() {
    let steamcmd = fake_install("update");
    let change_note = ChangeNoteInput {
        explicit: Some("Fixed the \"Fixture\" event".to_owned()),
        commit_url: None,
    };
    let plan = mod_plan_with("eu4", &change_note);
    let outcome = publish(&steamcmd, &plan, &PublishOptions::default());
    let published = outcome.result.unwrap();
    assert_eq!(published.item_id.to_string(), "2948217283");
    assert!(!published.created);
    assert_masked(&outcome.log);

    assert_eq!(seen(&steamcmd, "stdin").as_deref(), Some("closed"));
    assert_eq!(
        seen(&steamcmd, "session").as_deref().map(str::as_bytes),
        Some(SESSION)
    );
    let vdf = seen(&steamcmd, "item.vdf").unwrap();
    for line in [
        "\t\"appid\" \"236850\"",
        "\t\"publishedfileid\" \"2948217283\"",
        "\t\"title\" \"Fixture Mod\"",
        "\t\"visibility\" \"0\"",
        "\t\"changenote\" \"Fixed the \\\"Fixture\\\" event\"",
    ] {
        assert!(vdf.lines().any(|l| l == line), "{line} missing in\n{vdf}");
    }
    assert!(vdf.contains("\"contentfolder\""), "{vdf}");
    assert!(vdf.contains("thumbnail.png"), "{vdf}");
    // The description, when the plan has one, is the plan's BBCode.
    match &plan.metadata.description {
        Some(description) => assert!(
            vdf.contains(&format!(
                "\t\"description\" \"{}\"\n",
                vdf_escape(description)
            )),
            "{vdf}"
        ),
        None => assert!(!vdf.contains("\"description\""), "{vdf}"),
    }
    assert!(!vdf.contains("\"tags\""), "{vdf}");
    // The VDF is written outside the published copy.
    assert!(steamcmd.root().join("item.vdf").is_file());

    assert!(!steamcmd.session_path().exists(), "the session is deleted");
    let root = steamcmd.root().to_owned();
    drop(steamcmd);
    assert!(!root.exists(), "the steamcmd folder is deleted");
}

fn creates_an_item_only_when_allowed() {
    let plan = mod_plan("eu4-unpublished");
    let steamcmd = fake_install("create");
    let refused = publish(&steamcmd, &plan, &PublishOptions::default());
    let error = refused.result.unwrap_err();
    assert!(matches!(error, Error::NoItemId), "{error}");
    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("run `pdx-ws init` in client mode first")
    );
    assert_eq!(seen(&steamcmd, "item.vdf"), None, "steamcmd did not run");

    let options = PublishOptions {
        allow_create: true,
        ..PublishOptions::default()
    };
    let outcome = publish(&steamcmd, &plan, &options);
    let published = outcome.result.unwrap();
    assert_eq!(published.item_id.to_string(), CREATED_ID);
    assert!(published.created);
    let vdf = seen(&steamcmd, "item.vdf").unwrap();
    assert!(vdf.contains("\t\"publishedfileid\" \"0\"\n"), "{vdf}");
    assert!(!vdf.contains("\"visibility\""), "{vdf}");
    assert!(!vdf.contains("\"previewfile\""), "{vdf}");
    assert!(!steamcmd.session_path().exists());
}

fn a_build_failure_shows_the_logs() {
    let steamcmd = fake_install("no-content");
    let outcome = publish(&steamcmd, &mod_plan("eu4"), &PublishOptions::default());
    let error = outcome.result.unwrap_err();
    assert_eq!(error.exit_code(), 1);
    assert_eq!(
        error.to_string(),
        "steamcmd could not build the Workshop item (exit 9)"
    );
    let details = error.details();
    assert!(
        details
            .iter()
            .any(|line| line.ends_with("ERROR! Build for workshop item has no content")),
        "{details:?}"
    );
    assert!(
        details
            .iter()
            .any(|line| line.starts_with("depot_build_236850.log: ")),
        "{details:?}"
    );
    assert_masked(&outcome.log);
    assert!(!steamcmd.session_path().exists());
}

fn a_missing_session_exits_with_3() {
    let steamcmd = fake_install("no-session");
    let outcome = publish(&steamcmd, &mod_plan("eu4"), &PublishOptions::default());
    let error = outcome.result.unwrap_err();
    assert!(matches!(error, Error::SessionRejected { .. }), "{error}");
    assert_eq!(error.exit_code(), 3);
    assert!(error.to_string().contains("pdx-ws session export"));
    assert_masked(&outcome.log);
    assert!(!steamcmd.session_path().exists());
}

fn exit_0_without_success_is_a_failure() {
    let steamcmd = fake_install("no-success-line");
    let outcome = publish(&steamcmd, &mod_plan("eu4"), &PublishOptions::default());
    let error = outcome.result.unwrap_err();
    assert_eq!(error.exit_code(), 1);
    assert_eq!(
        error.to_string(),
        "steamcmd did not publish the item (exit 0)"
    );
}

fn a_hanging_steamcmd_is_stopped() {
    let steamcmd = fake_install("hang");
    let options = PublishOptions {
        timeout: Duration::from_secs(2),
        ..PublishOptions::default()
    };
    let started = Instant::now();
    let outcome = publish(&steamcmd, &mod_plan("eu4"), &options);
    let error = outcome.result.unwrap_err();
    assert!(matches!(error, Error::Timeout(_)), "{error}");
    assert_eq!(error.exit_code(), 1);
    assert!(started.elapsed() < Duration::from_secs(30));
    assert!(!steamcmd.session_path().exists());
}

fn the_self_update_accepts_exit_7() {
    let steamcmd = fake_install("update");
    let mut log = Vec::new();
    steamcmd
        .self_update(&mut |line| log.push(line.to_owned()))
        .unwrap();
    assert!(
        log.iter()
            .any(|line| line.starts_with("Steam Console Client"))
    );
}

// ---------------------------------------------------------------------------
// Several mods

/// Three EU4 mods with items 101, 102 and 103, in a scratch repository.
fn three_mods() -> (tempfile::TempDir, Plan) {
    let dir = tempfile::tempdir().unwrap();
    let mut toml = "schema = 1\n[defaults]\ngame = \"eu4\"\n".to_owned();
    for (index, id) in ["one", "two", "three"].iter().enumerate() {
        toml.push_str(&format!("[[mod]]\nid = \"{id}\"\npath = \"{id}\"\n"));
        let folder = dir.path().join(id);
        fs::create_dir_all(folder.join("common")).unwrap();
        fs::write(folder.join("common/notes.txt"), id).unwrap();
        fs::write(
            folder.join("descriptor.mod"),
            format!(
                "name=\"{id}\"\nversion=\"1.0.{index}\"\nremote_file_id=\"{}\"\n",
                101 + index
            ),
        )
        .unwrap();
    }
    fs::write(dir.path().join("workshop.toml"), toml).unwrap();
    let config = Config::load(&dir.path().join("workshop.toml")).unwrap();
    let plan = Plan::build(&config, &[]).unwrap();
    (dir, plan)
}

fn publish_all(scenario: &str, fail_fast: bool) -> (Report, Vec<String>) {
    let (_dir, plan) = three_mods();
    let steamcmd = fake_install(scenario);
    let options = PublishOptions::default();
    let mut log = |_: &str| {};
    let report = publish::run(&plan, fail_fast, |module| {
        publish::through_steamcmd(&steamcmd, &credentials(), module, &options, &mut log)
    });
    let attempted = seen(&steamcmd, "attempted")
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect();
    assert!(!steamcmd.session_path().exists(), "the session is deleted");
    (report, attempted)
}

fn outcomes(report: &Report) -> Vec<ModOutcome> {
    report.mods.iter().map(|result| result.outcome).collect()
}

fn every_mod_is_published() {
    let (report, attempted) = publish_all("update", false);
    assert_eq!(attempted, ["101", "102", "103"]);
    assert_eq!(outcomes(&report), [ModOutcome::Published; 3]);
    assert_eq!(report.exit_code(), 0);
}

fn a_failed_mod_does_not_stop_the_others() {
    let (report, attempted) = publish_all("101=update\n102=no-content\n103=update", false);
    assert_eq!(attempted, ["101", "102", "103"]);
    assert_eq!(
        outcomes(&report),
        [
            ModOutcome::Published,
            ModOutcome::Failed,
            ModOutcome::Published
        ]
    );
    assert_eq!(report.exit_code(), 1);
    let two = &report.mods[1];
    assert_eq!(
        two.error.as_deref(),
        Some("steamcmd could not build the Workshop item (exit 9)")
    );
    assert!(
        two.details
            .iter()
            .any(|line| line.ends_with("ERROR! Build for workshop item has no content")),
        "{:?}",
        two.details
    );
}

fn fail_fast_stops_at_the_first_failure() {
    let (report, attempted) = publish_all("101=update\n102=no-content\n103=update", true);
    assert_eq!(attempted, ["101", "102"]);
    assert_eq!(
        outcomes(&report),
        [
            ModOutcome::Published,
            ModOutcome::Failed,
            ModOutcome::Skipped
        ]
    );
    assert_eq!(report.exit_code(), 1);

    // A rejected session stops the run even without --fail-fast.
    let (report, attempted) = publish_all("101=update\n102=no-session\n103=update", false);
    assert_eq!(attempted, ["101", "102"]);
    assert_eq!(
        outcomes(&report),
        [
            ModOutcome::Published,
            ModOutcome::Failed,
            ModOutcome::Skipped
        ]
    );
    assert_eq!(report.exit_code(), 3);
}
