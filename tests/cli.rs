use std::process::{Command, Output};

fn pdx_ws(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdx-ws"))
        .args(args)
        .output()
        .expect("pdx-ws runs")
}

#[test]
fn unimplemented_commands_exit_with_code_2() {
    for args in [
        &["plan"][..],
        &["publish"],
        &["init"],
        &["session", "export"],
    ] {
        let output = pdx_ws(args);
        assert_eq!(output.status.code(), Some(2), "pdx-ws {args:?}");
        assert!(output.stdout.is_empty(), "pdx-ws {args:?} wrote to stdout");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let expected = format!("pdx-ws {}: not implemented yet", args.join(" "));
        assert_eq!(stderr.trim_end(), expected, "pdx-ws {args:?}");
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
