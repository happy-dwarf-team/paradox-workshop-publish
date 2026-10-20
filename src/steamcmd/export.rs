//! `pdx-ws session export`: creating a Steam session with an interactive
//! steamcmd login on the modder's computer (spec section 3, issue #9).
//!
//! steamcmd is kept in a per-user folder, never in the repository. The
//! cached login is read once, printed as gzip + base64 to stdout by the
//! caller, and deleted, so the secret is the only copy.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::{Error, Steamcmd, io_error, output, session};

/// How long the check login without a password may take.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(2 * 60);

/// The GitHub environment the guidance suggests (spec section 11).
pub const SECRET_ENVIRONMENT: &str = "steam-workshop";

/// The per-user folder for pdx-ws: `%LOCALAPPDATA%\pdx-ws` on Windows,
/// `$XDG_DATA_HOME/pdx-ws` or `~/.local/share/pdx-ws` elsewhere.
pub fn data_dir() -> Option<PathBuf> {
    data_dir_from(cfg!(windows), |key| std::env::var_os(key))
}

/// [`data_dir`] with the environment passed in.
pub fn data_dir_from(windows: bool, var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let set = |key: &str| {
        var(key)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let base = if windows {
        set("LOCALAPPDATA")?
    } else {
        set("XDG_DATA_HOME").or_else(|| set("HOME").map(|home| home.join(".local/share")))?
    };
    Some(base.join("pdx-ws"))
}

/// The session goes to stdout only when stdout is not a terminal (a pipe
/// or a redirect), or when `--stdout` allows a terminal.
pub fn may_write_session(stdout_is_terminal: bool, forced: bool) -> bool {
    !stdout_is_terminal || forced
}

/// How to store the session as GitHub environment secrets.
pub fn guidance(user: &str) -> String {
    let env = SECRET_ENVIRONMENT;
    format!(
        "Store the Steam session as secrets of a GitHub environment, for example:\n\
         \n  pdx-ws session export --user {user} | gh secret set STEAM_SESSION --env {env}\
         \n  gh secret set STEAM_USERNAME --env {env} --body {user}\n\
         \nPass them to the action as `steam-session` and `steam-username`. The session lets \
         steamcmd log in as {user} to publish, update and download; it does not reveal the \
         password or the Steam Guard code. Revoke it with \"Deauthorize all devices\" in Steam \
         Guard or by changing the password."
    )
}

/// Checks the output of a login without a password: it must use the cached
/// login and must not ask for anything.
pub fn check_login_output(code: Option<i32>, output: &str) -> Result<(), Error> {
    let reject = |reason: &str| Err(Error::LoginCheck(reason.to_owned()));
    for prompt in ["password:", "Steam Guard code:", "Two-factor code:"] {
        if output.contains(prompt) {
            return reject(&format!(
                "steamcmd asked for `{}`",
                prompt.trim_end_matches(':')
            ));
        }
    }
    for message in ["Cached credentials not found", "License expired"] {
        if output.contains(message) {
            return reject(message);
        }
    }
    if code != Some(0) {
        return reject(&match code {
            Some(code) => format!("steamcmd exited with {code}"),
            None => "steamcmd ended without an exit code".to_owned(),
        });
    }
    if !output.contains("Logging in using cached credentials") {
        return reject("steamcmd did not use the cached login");
    }
    Ok(())
}

impl Steamcmd {
    /// Creates a Steam session for `user` and returns it as gzip + base64:
    /// an interactive login, a second login that must not prompt, then the
    /// cached login is read and deleted. It is deleted on errors too.
    pub fn export_session(&self, user: &str, log: &mut dyn FnMut(&str)) -> Result<String, Error> {
        self.forget_session()?;
        let result = self
            .login_interactively(user)
            .and_then(|()| self.check_cached_login(user, log))
            .and_then(|()| self.read_session());
        let forgotten = self.forget_session();
        let session = result?;
        forgotten?;
        Ok(session)
    }

    /// Deletes a cached login, so the next login asks for the password.
    pub fn forget_session(&self) -> Result<(), Error> {
        match fs::remove_file(self.session_path()) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                Err(io_error("cannot delete the cached Steam login")(error))
            }
            _ => Ok(()),
        }
    }

    /// `+login <user> +quit` on the modder's terminal, where they type the
    /// password and the Steam Guard code. steamcmd's output goes to stderr,
    /// because stdout carries the session.
    pub fn login_interactively(&self, user: &str) -> Result<(), Error> {
        fs::create_dir_all(self.home()).map_err(io_error("cannot create the steamcmd home"))?;
        let status = Command::new(&self.executable)
            .args(["+login", user, "+quit"])
            .current_dir(self.folder())
            .envs(self.env())
            .stdin(Stdio::inherit())
            .stdout(Stdio::from(io::stderr()))
            .stderr(Stdio::inherit())
            .status()
            .map_err(io_error(format!(
                "cannot run {}",
                self.executable.display()
            )))?;
        match status.code() {
            Some(0) => Ok(()),
            code => Err(Error::Failed {
                code,
                summary: format!(
                    "the steamcmd login failed ({}); check the user name, password and code",
                    super::describe(code)
                ),
                details: Vec::new(),
            }),
        }
    }

    /// Logs in again without a password, as CI will, with stdin closed.
    pub fn check_cached_login(&self, user: &str, log: &mut dyn FnMut(&str)) -> Result<(), Error> {
        let args = ["+login".as_ref(), user.as_ref(), "+quit".as_ref()];
        let run = self.run(&args, CHECK_TIMEOUT, user, log)?;
        check_login_output(run.code, &output::strip_ansi(&run.lines.join("\n")))
    }

    /// The cached login as a Steam session (gzip + base64).
    fn read_session(&self) -> Result<String, Error> {
        match fs::read(self.session_path()) {
            Ok(bytes) if !bytes.is_empty() => Ok(session::encode(&bytes)),
            Ok(_) => Err(Error::LoginCheck("the cached login is empty".to_owned())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Err(Error::LoginCheck(
                "steamcmd wrote no cached login (config/config.vdf)".to_owned(),
            )),
            Err(error) => Err(io_error("cannot read the cached Steam login")(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_follows_the_platform() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |key: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| OsString::from(v))
            }
        };
        assert_eq!(
            data_dir_from(true, env(&[("LOCALAPPDATA", r"C:\Users\m\AppData\Local")])),
            Some(PathBuf::from(r"C:\Users\m\AppData\Local").join("pdx-ws"))
        );
        assert_eq!(
            data_dir_from(
                false,
                env(&[("XDG_DATA_HOME", "/data"), ("HOME", "/home/m")])
            ),
            Some(PathBuf::from("/data/pdx-ws"))
        );
        assert_eq!(
            data_dir_from(false, env(&[("XDG_DATA_HOME", ""), ("HOME", "/home/m")])),
            Some(PathBuf::from("/home/m/.local/share").join("pdx-ws"))
        );
        assert_eq!(data_dir_from(false, env(&[])), None);
    }

    #[test]
    fn the_session_never_goes_to_a_terminal_unasked() {
        assert!(may_write_session(false, false));
        assert!(!may_write_session(true, false));
        assert!(may_write_session(true, true));
    }

    #[test]
    fn guidance_shows_the_gh_pipe() {
        let text = guidance("modder");
        assert!(
            text.contains(
                "pdx-ws session export --user modder | gh secret set STEAM_SESSION --env steam-workshop"
            ),
            "{text}"
        );
        assert!(text.contains("gh secret set STEAM_USERNAME --env steam-workshop --body modder"));
        assert!(text.contains("Deauthorize all devices"));
    }

    #[test]
    fn the_check_login_must_use_the_cache_without_prompts() {
        let cached = "Logging in using cached credentials.\nLogging in user '***' [U:1:***] to Steam Public...OK\nWaiting for user info...OK";
        assert!(check_login_output(Some(0), cached).is_ok());
        for (code, output) in [
            (
                Some(5),
                "Cached credentials not found.\npassword: \nERROR (Invalid Password)",
            ),
            (
                Some(0),
                "Logging in using cached credentials.\nSteam Guard code:",
            ),
            (Some(5), "ERROR (License expired)"),
            (Some(0), "Logging in user '***' to Steam Public...OK"),
            (Some(7), cached),
        ] {
            let error = check_login_output(code, output).unwrap_err();
            assert!(matches!(error, Error::LoginCheck(_)), "{output}: {error}");
        }
    }
}
