//! `steamcmd` mode: publishing a plan through steamcmd with a Steam session
//! from the environment (spec section 9).
//!
//! Everything lives in one temporary folder: steamcmd itself, its home on
//! Linux, the session, the VDF and the logs. The folder is deleted when the
//! [`Steamcmd`] is dropped, and the session file right after each run.

pub mod install;
pub mod output;
pub mod process;
pub mod session;
pub mod vdf;

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tempfile::TempDir;

use crate::copy::CopyError;
use crate::exit;
use crate::item_id::ItemId;
use crate::plan::ModPlan;
use install::InstallError;
use output::Signal;
use session::{Credentials, SessionError};
use vdf::ItemVdf;

/// How long the self-update after installing may take.
pub const SELF_UPDATE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// How long one publish may take by default.
pub const PUBLISH_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// How many lines of each steamcmd log a failure shows.
const LOG_TAIL: usize = 15;

/// A steamcmd installation in a temporary folder.
#[derive(Debug)]
pub struct Steamcmd {
    root: TempDir,
    executable: PathBuf,
}

/// Options of one publish.
#[derive(Debug, Clone)]
pub struct PublishOptions {
    /// Create the item when the plan has no item ID (`--allow-create`).
    pub allow_create: bool,
    pub timeout: Duration,
}

impl Default for PublishOptions {
    fn default() -> Self {
        PublishOptions {
            allow_create: false,
            timeout: PUBLISH_TIMEOUT,
        }
    }
}

/// A successful publish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Published {
    pub item_id: ItemId,
    /// The item was created by this publish (`--allow-create`).
    pub created: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("steamcmd mode runs on Windows and Linux, not {0}")]
    UnsupportedPlatform(&'static str),
    #[error(
        "the Workshop item does not exist yet; run `pdx-ws init` in client mode first, or \
         pass --allow-create to create it without tags"
    )]
    NoItemId,
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(
        "Steam did not accept the Steam session ({reason}): it is missing or expired; create \
         a new one with `pdx-ws session export`"
    )]
    SessionRejected { reason: String },
    #[error(transparent)]
    Install(#[from] InstallError),
    #[error("{action}: {source}")]
    Io { action: String, source: io::Error },
    #[error("steamcmd did not finish within {} s and was stopped", .0.as_secs())]
    Timeout(Duration),
    /// steamcmd ran but did not publish; `details` holds its error lines
    /// and the end of its logs, masked.
    #[error("{summary}")]
    Failed {
        code: Option<i32>,
        summary: String,
        details: Vec<String>,
    },
    #[error(transparent)]
    Copy(#[from] CopyError),
}

impl Error {
    /// The `pdx-ws` exit code (spec section 3).
    pub fn exit_code(&self) -> u8 {
        match self {
            Error::UnsupportedPlatform(_) | Error::NoItemId => exit::VALIDATION,
            Error::Session(_) | Error::SessionRejected { .. } => exit::SESSION,
            _ => exit::PUBLISH_FAILED,
        }
    }

    /// Masked lines from steamcmd and its logs that explain a failure.
    pub fn details(&self) -> &[String] {
        match self {
            Error::Failed { details, .. } => details,
            _ => &[],
        }
    }
}

fn io_error(action: impl Into<String>) -> impl FnOnce(io::Error) -> Error {
    let action = action.into();
    move |source| Error::Io { action, source }
}

impl Steamcmd {
    /// Downloads steamcmd into a new temporary folder and lets it update
    /// itself. `log` receives steamcmd's output, masked.
    pub fn install(log: &mut dyn FnMut(&str)) -> Result<Steamcmd, Error> {
        if !cfg!(any(windows, target_os = "linux")) {
            return Err(Error::UnsupportedPlatform(std::env::consts::OS));
        }
        let root = tempfile::Builder::new()
            .prefix("pdx-ws-steamcmd-")
            .tempdir()
            .map_err(io_error("cannot create a temporary folder"))?;
        let executable = install::download(&root.path().join("steamcmd"))?;
        let steamcmd = Steamcmd::from_installed(root, executable);
        steamcmd.self_update(log)?;
        Ok(steamcmd)
    }

    /// Uses a steamcmd already unpacked inside `root`. `root` must be a
    /// scratch folder: the session is written into it, and it is deleted
    /// with this value.
    pub fn from_installed(root: TempDir, executable: PathBuf) -> Steamcmd {
        Steamcmd { root, executable }
    }

    /// The temporary folder that holds everything.
    pub fn root(&self) -> &Path {
        self.root.path()
    }

    fn folder(&self) -> &Path {
        self.executable.parent().unwrap_or(self.root.path())
    }

    /// steamcmd's home on Linux, where it keeps `~/Steam`. It is inside the
    /// temporary folder, so the user's own `~/Steam` is never touched.
    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    /// Where steamcmd looks for the cached login: `config/config.vdf` in its
    /// folder, or `~/Steam/config/config.vdf` on Linux.
    pub fn session_path(&self) -> PathBuf {
        if cfg!(target_os = "linux") {
            self.home().join("Steam/config/config.vdf")
        } else {
            self.folder().join("config/config.vdf")
        }
    }

    fn env(&self) -> Vec<(&'static str, OsString)> {
        if cfg!(target_os = "linux") {
            vec![("HOME", self.home().into_os_string())]
        } else {
            Vec::new()
        }
    }

    fn run(
        &self,
        args: &[&std::ffi::OsStr],
        timeout: Duration,
        user: &str,
        log: &mut dyn FnMut(&str),
    ) -> Result<process::Run, Error> {
        fs::create_dir_all(self.home()).map_err(io_error("cannot create the steamcmd home"))?;
        let env = self.env();
        let env: Vec<_> = env
            .iter()
            .map(|(key, value)| (*key, value.as_os_str()))
            .collect();
        let run = process::run(
            &self.executable,
            args,
            self.folder(),
            &env,
            timeout,
            &mut |line| log(&output::mask(&output::strip_ansi(line), user)),
        )
        .map_err(io_error(format!(
            "cannot run {}",
            self.executable.display()
        )))?;
        if run.timed_out {
            return Err(Error::Timeout(timeout));
        }
        Ok(run)
    }

    /// Runs steamcmd once without logging in, which updates it. Windows
    /// exits with 7 after the first update, which counts as success.
    pub fn self_update(&self, log: &mut dyn FnMut(&str)) -> Result<(), Error> {
        let run = self.run(&["+quit".as_ref()], SELF_UPDATE_TIMEOUT, "", log)?;
        match run.code {
            Some(0 | 7) => Ok(()),
            code => Err(Error::Failed {
                code,
                summary: format!("the steamcmd self-update failed ({})", describe(code)),
                details: output::error_lines(&output::strip_ansi(&run.lines.join("\n"))),
            }),
        }
    }

    /// Publishes `plan` with the published copy in `content`.
    pub fn publish(
        &self,
        credentials: &Credentials,
        plan: &ModPlan,
        content: &Path,
        options: &PublishOptions,
        log: &mut dyn FnMut(&str),
    ) -> Result<Published, Error> {
        let item = match (plan.item.id, options.allow_create) {
            (Some(id), _) => Some(id),
            (None, true) => None,
            (None, false) => return Err(Error::NoItemId),
        };
        let absolute = |path: &Path| {
            std::path::absolute(path)
                .map_err(io_error(format!("cannot resolve {}", path.display())))
        };
        let item_vdf = ItemVdf {
            app_id: plan.app_id,
            published_file_id: item.map_or(0, ItemId::get),
            content_folder: absolute(content)?,
            preview_file: match &plan.metadata.preview {
                Some(preview) => Some(absolute(&plan.dir.join(&preview.path))?),
                None => None,
            },
            title: Some(plan.metadata.title.clone()),
            description: plan.metadata.description.clone(),
            visibility: plan.metadata.visibility,
            change_note: plan.metadata.change_note.clone(),
        };
        // Outside the published copy, so it is never uploaded.
        let vdf_path = self.root.path().join("item.vdf");
        fs::write(&vdf_path, item_vdf.render()).map_err(io_error("cannot write the VDF"))?;
        self.clear_logs(plan.app_id);

        let user = credentials.user();
        let session = SessionFile::write(self.session_path(), credentials.session())?;
        let args = [
            "+login".as_ref(),
            user.as_ref(),
            "+workshop_build_item".as_ref(),
            vdf_path.as_os_str(),
            "+quit".as_ref(),
        ];
        let run = self.run(&args, options.timeout, user, log);
        drop(session);
        let run = run?;

        let text = output::strip_ansi(&run.lines.join("\n"));
        match output::classify(run.code, &text) {
            Signal::Success { .. } => {
                let written = fs::read_to_string(&vdf_path)
                    .ok()
                    .and_then(|vdf| vdf::published_file_id(&vdf))
                    .and_then(|id| ItemId::try_from(id).ok());
                match (written, item) {
                    (Some(written), Some(item)) if written != item => Err(self.failure(
                        run.code,
                        format!("steamcmd updated item {written} instead of {item}"),
                        &text,
                        user,
                        plan.app_id,
                    )),
                    (Some(item_id), _) => Ok(Published {
                        item_id,
                        created: item.is_none(),
                    }),
                    (None, _) => Err(self.failure(
                        run.code,
                        "steamcmd reported success but wrote no publishedfileid back".to_owned(),
                        &text,
                        user,
                        plan.app_id,
                    )),
                }
            }
            Signal::SessionRejected { reason } => Err(Error::SessionRejected { reason }),
            Signal::BuildFailed => Err(self.failure(
                run.code,
                "steamcmd could not build the Workshop item (exit 9)".to_owned(),
                &text,
                user,
                plan.app_id,
            )),
            Signal::Failed => Err(self.failure(
                run.code,
                format!("steamcmd did not publish the item ({})", describe(run.code)),
                &text,
                user,
                plan.app_id,
            )),
        }
    }

    fn failure(
        &self,
        code: Option<i32>,
        summary: String,
        output: &str,
        user: &str,
        app_id: u32,
    ) -> Error {
        let mut details: Vec<String> = output::error_lines(output)
            .iter()
            .map(|line| output::mask(line, user))
            .collect();
        for path in self.logs(app_id) {
            let Ok(text) = fs::read(&path) else { continue };
            let text = String::from_utf8_lossy(&text);
            let lines: Vec<_> = text
                .lines()
                .filter(|line| !line.trim().is_empty())
                .collect();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            for line in &lines[lines.len().saturating_sub(LOG_TAIL)..] {
                details.push(format!("{name}: {}", output::mask(line.trim(), user)));
            }
        }
        Error::Failed {
            code,
            summary,
            details,
        }
    }

    /// `depot_build_<appid>.log` and `workshop_log.txt` anywhere in the
    /// temporary folder.
    fn logs(&self, app_id: u32) -> Vec<PathBuf> {
        let names = [
            format!("depot_build_{app_id}.log"),
            "workshop_log.txt".to_owned(),
        ];
        let mut found = Vec::new();
        find_files(self.root.path(), &names, &mut found);
        found.sort();
        found
    }

    /// Deletes the logs of an earlier run, so a failure shows its own.
    fn clear_logs(&self, app_id: u32) {
        for path in self.logs(app_id) {
            let _ = fs::remove_file(path);
        }
    }
}

fn describe(code: Option<i32>) -> String {
    match code {
        Some(code) => format!("exit {code}"),
        None => "no exit code".to_owned(),
    }
}

fn find_files(dir: &Path, names: &[String], found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            find_files(&entry.path(), names, found);
        } else if file_type.is_file()
            && names
                .iter()
                .any(|name| entry.file_name().to_str() == Some(name))
        {
            found.push(entry.path());
        }
    }
}

/// The session written for one run; deleted when dropped, also on errors.
struct SessionFile(PathBuf);

impl SessionFile {
    fn write(path: PathBuf, session: &[u8]) -> Result<SessionFile, Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_error("cannot create the session folder"))?;
        }
        let file = SessionFile(path);
        write_private(&file.0, session).map_err(io_error("cannot write the Steam session"))?;
        Ok(file)
    }
}

impl Drop for SessionFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(unix)]
fn write_private(path: &Path, contents: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?
        .write_all(contents)
}

#[cfg(not(unix))]
fn write_private(path: &Path, contents: &[u8]) -> io::Result<()> {
    fs::write(path, contents)
}
