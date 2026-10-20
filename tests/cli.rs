use std::process::{Command, Output};

fn pdx_ws(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdx-ws"))
        .args(args)
        .output()
        .expect("pdx-ws runs")
}

#[test]
fn unimplemented_commands_exit_with_code_2() {
    for args in [&["publish"][..], &["init"], &["session", "export"]] {
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
