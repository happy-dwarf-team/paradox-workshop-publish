//! Installing steamcmd from Valve's CDN into a temporary folder
//! (spec section 9).

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Valve's steamcmd download for this platform.
#[cfg(windows)]
pub const DOWNLOAD_URL: &str = "https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip";
#[cfg(not(windows))]
pub const DOWNLOAD_URL: &str =
    "https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz";

/// The program to run inside the steamcmd folder.
#[cfg(windows)]
pub const EXECUTABLE: &str = "steamcmd.exe";
#[cfg(not(windows))]
pub const EXECUTABLE: &str = "steamcmd.sh";

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("cannot download {DOWNLOAD_URL}: {0}")]
    Download(String),
    #[error("cannot unpack steamcmd into {}: {source}", path.display())]
    Unpack { path: PathBuf, source: io::Error },
    #[error("the steamcmd download has no {EXECUTABLE}")]
    MissingExecutable,
    #[error(
        "steamcmd needs the 32-bit package lib32gcc-s1, and installing it failed ({0}); \
         install it with `sudo apt-get install lib32gcc-s1`"
    )]
    Lib32Gcc(String),
}

/// Downloads and unpacks steamcmd into `dir` and returns its executable.
pub fn download(dir: &Path) -> Result<PathBuf, InstallError> {
    #[cfg(target_os = "linux")]
    ensure_lib32gcc()?;

    fs::create_dir_all(dir).map_err(|source| InstallError::Unpack {
        path: dir.to_owned(),
        source,
    })?;
    let archive = dir.join("steamcmd-download");
    fetch(&archive)?;
    unpack(&archive, dir).map_err(|source| InstallError::Unpack {
        path: dir.to_owned(),
        source,
    })?;
    let _ = fs::remove_file(&archive);
    let executable = dir.join(EXECUTABLE);
    if executable.is_file() {
        Ok(executable)
    } else {
        Err(InstallError::MissingExecutable)
    }
}

fn fetch(target: &Path) -> Result<(), InstallError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(300)))
        .build()
        .into();
    let mut response = agent
        .get(DOWNLOAD_URL)
        .call()
        .map_err(|error| InstallError::Download(error.to_string()))?;
    let mut file =
        File::create(target).map_err(|error| InstallError::Download(error.to_string()))?;
    io::copy(&mut response.body_mut().as_reader(), &mut file)
        .map_err(|error| InstallError::Download(error.to_string()))?;
    Ok(())
}

#[cfg(windows)]
fn unpack(archive: &Path, dir: &Path) -> io::Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(archive)?).map_err(io::Error::other)?;
    zip.extract(dir).map_err(io::Error::other)
}

#[cfg(not(windows))]
fn unpack(archive: &Path, dir: &Path) -> io::Result<()> {
    let gz = flate2::read::GzDecoder::new(File::open(archive)?);
    tar::Archive::new(gz).unpack(dir)
}

/// steamcmd is a 32-bit program; Ubuntu runners need `lib32gcc-s1`.
#[cfg(target_os = "linux")]
fn ensure_lib32gcc() -> Result<(), InstallError> {
    use std::process::{Command, Stdio};

    let installed = Command::new("dpkg-query")
        .args(["-W", "-f=${Status}", "lib32gcc-s1"])
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|output| {
            String::from_utf8_lossy(&output.stdout).contains("install ok installed")
        });
    if installed {
        return Ok(());
    }
    let root = Command::new("id")
        .arg("-u")
        .output()
        .is_ok_and(|output| output.stdout.trim_ascii() == b"0");
    let mut command = if root {
        Command::new("apt-get")
    } else {
        let mut sudo = Command::new("sudo");
        sudo.args(["-n", "apt-get"]);
        sudo
    };
    let status = command
        .args(["install", "-y", "-qq", "lib32gcc-s1"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .map_err(|error| InstallError::Lib32Gcc(error.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(InstallError::Lib32Gcc(format!(
            "apt-get exited with {status}"
        )))
    }
}
