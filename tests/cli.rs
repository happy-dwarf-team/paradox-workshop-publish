use std::process::{Command, Output};

fn pdx_ws(args: &[&str]) -> Output {
    // Without credentials, `publish` stops before it would install steamcmd.
    Command::new(env!("CARGO_BIN_EXE_pdx-ws"))
        .args(args)
        .env_remove("PDX_WS_STEAM_USERNAME")
        .env_remove("PDX_WS_STEAM_SESSION")
        .output()
        .expect("pdx-ws runs")
}

#[test]
fn unimplemented_commands_exit_with_code_2() {
    for args in [&["init"][..], &["session", "export"]] {
        let output = pdx_ws(args);
        assert_eq!(output.status.code(), Some(2), "pdx-ws {args:?}");
        assert!(output.stdout.is_empty(), "pdx-ws {args:?} wrote to stdout");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let expected = format!("pdx-ws {}: not implemented yet", args.join(" "));
        assert_eq!(stderr.trim_end(), expected, "pdx-ws {args:?}");
    }
}

fn fixture_config(name: &str) -> String {
    format!(
        "{}/tests/fixtures/{name}/workshop.toml",
        env!("CARGO_MANIFEST_DIR")
    )
}

#[test]
fn plan_prints_text() {
    let output = pdx_ws(&["plan", "--config", &fixture_config("eu4")]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Title:          Fixture Mod"), "{stdout}");
    assert!(stdout.contains("descriptor.mod (without path)"), "{stdout}");
    assert!(stdout.contains("changes the EU4 checksum"), "{stdout}");
    assert!(
        stdout.contains(
            "  Description:    replaces the Workshop description (docs/description.md, \
             217 of 8000 bytes)\n    | [h1]Fixture Mod[/h1]\n    |\n"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "  Change note:    from docs/CHANGELOG.md, version 1.4.2\n    | [h3]Bug Fixes[/h3]\n"
        ),
        "{stdout}"
    );
}

#[test]
fn plan_takes_the_change_note_option() {
    let config = fixture_config("eu4");
    let output = pdx_ws(&[
        "plan",
        "--config",
        &config,
        "--change-note",
        "Hotfix for **Riga**",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(0));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let metadata = &json["mods"][0]["metadata"];
    assert_eq!(metadata["change_note"], "Hotfix for [b]Riga[/b]");
    assert_eq!(metadata["change_note_source"]["kind"], "explicit");
    assert_eq!(metadata["description_source"], "docs/description.md");
    let description = metadata["description"].as_str().unwrap();
    assert!(
        description.starts_with("[h1]Fixture Mod[/h1]"),
        "{description}"
    );
}

#[test]
fn plan_prints_json() {
    let config = fixture_config("multi");
    let output = pdx_ws(&["plan", "--config", &config, "--mod", "beta", "--json"]);
    assert_eq!(output.status.code(), Some(0));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["schema"], 1);
    assert_eq!(json["mods"].as_array().unwrap().len(), 1);
    assert_eq!(json["mods"][0]["id"], "beta");
    assert_eq!(json["mods"][0]["game"], "hoi4");
    assert_eq!(json["mods"][0]["item"]["id"], serde_json::Value::Null);
    assert_eq!(json["mods"][0]["checksum"]["status"], "ignored");
    let metadata = &json["mods"][0]["metadata"];
    assert_eq!(metadata["description"], serde_json::Value::Null);
    assert_eq!(metadata["change_note_source"]["kind"], "fallback");
    // In CI the fallback also links the commit.
    let note = metadata["change_note"].as_str().unwrap();
    assert!(note.starts_with("Version 2.0.0"), "{note}");
}

#[test]
fn plan_errors_exit_with_code_2() {
    let missing = format!("{}/no-such-workshop.toml", env!("CARGO_MANIFEST_DIR"));
    let config = fixture_config("multi");
    for args in [
        &["plan", "--config", missing.as_str()][..],
        &["plan", "--config", config.as_str(), "--mod", "gamma"],
    ] {
        let output = pdx_ws(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with("error: "), "{stderr}");
    }
}

#[test]
fn publish_refuses_a_missing_item_before_anything_else() {
    let config = fixture_config("eu4-unpublished");
    let output = pdx_ws(&["publish", "--config", &config]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("run `pdx-ws init` in client mode first"),
        "{stderr}"
    );
}

#[test]
fn publish_without_a_session_exits_with_3() {
    let config = fixture_config("eu4");
    let output = pdx_ws(&["publish", "--config", &config]);
    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("PDX_WS_STEAM_USERNAME is not set"),
        "{stderr}"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_pdx-ws"))
        .args(["publish", "--config", &config])
        .env("PDX_WS_STEAM_USERNAME", "modder")
        .env_remove("PDX_WS_STEAM_SESSION")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("pdx-ws session export"), "{stderr}");
}

#[test]
fn client_mode_is_not_implemented_yet() {
    let config = fixture_config("eu4");
    let output = pdx_ws(&["publish", "--config", &config, "--mode", "client"]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn help_lists_every_command() {
    let output = pdx_ws(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in ["plan", "publish", "init", "session"] {
        assert!(stdout.contains(command), "--help does not list {command}");
    }
}

#[test]
fn version_matches_the_crate() {
    let output = pdx_ws(&["--version"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim_end(),
        format!("pdx-ws {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn missing_command_is_a_usage_error() {
    let output = pdx_ws(&[]);
    assert_eq!(output.status.code(), Some(2));
}
