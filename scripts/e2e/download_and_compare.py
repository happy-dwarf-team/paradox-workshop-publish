"""E2E check (docs/spec/v1.md, section 13; experiment E4): download a
Workshop item with steamcmd and compare its files with the published copy of
a mod folder.

The published copy is the mod folder without the built-in excludes
(.git*, .github/, workshop.toml, *.vdf); the fixture has no other excludes and
no local-only descriptor keys, so its files must come back byte for byte.

STEAM_USERNAME and STEAM_SESSION (gzip + base64 of steamcmd's
config/config.vdf) come from the environment. The session is never printed,
SteamIDs and the user name are masked in steamcmd's output, and the whole
steamcmd folder is deleted at the end, also on failure.
"""

import argparse
import base64
import gzip
import hashlib
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request
import zipfile
from pathlib import Path

WINDOWS = platform.system() == "Windows"
URL = (
    "https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip"
    if WINDOWS
    else "https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz"
)


def excluded(relative: Path) -> bool:
    """pdx-ws's built-in excludes, at any depth."""
    for part in relative.parts:
        if part.startswith(".git"):
            return True
    return relative.name == "workshop.toml" or relative.suffix == ".vdf"


def tree(root: Path, skip_excludes: bool) -> dict:
    files = {}
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if path.is_file() and not (skip_excludes and excluded(relative)):
            files[relative.as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    return files


class Steamcmd:
    def __init__(self, work: Path, user: str):
        self.folder = work / "steamcmd"
        # Linux steamcmd keeps ~/Steam in its home; give it one of its own.
        self.home = work / "home"
        self.user = user
        self.exe = self.folder / ("steamcmd.exe" if WINDOWS else "steamcmd.sh")

    def env(self):
        env = dict(os.environ)
        env.pop("STEAM_SESSION", None)
        if not WINDOWS:
            env["HOME"] = str(self.home)
        return env

    def session_path(self) -> Path:
        if WINDOWS:
            return self.folder / "config" / "config.vdf"
        return self.home / "Steam" / "config" / "config.vdf"

    def content_roots(self):
        return [self.folder / "steamapps", self.home / "Steam" / "steamapps"]

    def install(self):
        self.folder.mkdir(parents=True)
        self.home.mkdir(parents=True)
        archive = self.folder.parent / "steamcmd-download"
        urllib.request.urlretrieve(URL, archive)
        if WINDOWS:
            with zipfile.ZipFile(archive) as zipped:
                zipped.extractall(self.folder)
        else:
            with tarfile.open(archive) as tarred:
                tarred.extractall(self.folder)
        archive.unlink()
        code, _ = self.run([], timeout=600)
        if code not in (0, 7):
            sys.exit(f"steamcmd self-update failed with exit {code}")

    def restore_session(self, encoded: str):
        session = gzip.decompress(base64.b64decode("".join(encoded.split())))
        path = self.session_path()
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(session)

    def mask(self, line: str) -> str:
        line = re.sub(r"\[U:1:\d+\]", "[U:1:***]", line)
        if self.user:
            line = line.replace(f"'{self.user}'", "'***'")
        return re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", line)

    def run(self, args, timeout):
        login = ["+login", self.user] if args else []
        command = [str(self.exe), *login, *args, "+quit"]
        try:
            result = subprocess.run(
                command,
                cwd=self.folder,
                env=self.env(),
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                timeout=timeout,
            )
            code, output = result.returncode, result.stdout
        except subprocess.TimeoutExpired as error:
            code, output = "timeout", error.stdout or b""
        text = output.decode(errors="replace")
        for line in text.splitlines():
            if not re.search(r"\[\s*\d+%\]|Downloading update|Extracting package|Installing update", line):
                print(f"steamcmd | {self.mask(line)}", flush=True)
        return code, text

    def download(self, app: str, item: str):
        for root in self.content_roots():
            shutil.rmtree(root / "workshop" / "content" / app / item, ignore_errors=True)
        code, _ = self.run(["+workshop_download_item", app, item, "validate"], timeout=900)
        for root in self.content_roots():
            folder = root / "workshop" / "content" / app / item
            if folder.is_dir():
                return code, folder
        return code, None


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--app", required=True)
    parser.add_argument("--item", required=True)
    parser.add_argument("--mod", required=True, type=Path)
    parser.add_argument("--attempts", type=int, default=3)
    args = parser.parse_args()

    user = os.environ.get("STEAM_USERNAME", "").strip()
    session = os.environ.get("STEAM_SESSION", "")
    if not user or not session:
        sys.exit("STEAM_USERNAME and STEAM_SESSION must be set")

    expected = tree(args.mod, skip_excludes=True)
    print(f"expected {len(expected)} files from {args.mod}:")
    for path, digest in expected.items():
        print(f"  {digest[:12]}  {path}")

    work = Path(tempfile.mkdtemp(prefix="pdx-ws-e2e-"))
    try:
        steamcmd = Steamcmd(work, user)
        steamcmd.install()
        steamcmd.restore_session(session)
        for attempt in range(1, args.attempts + 1):
            code, folder = steamcmd.download(args.app, args.item)
            if folder is None:
                print(f"attempt {attempt}: steamcmd exited with {code} and downloaded nothing")
            else:
                actual = tree(folder, skip_excludes=False)
                if actual == expected:
                    print(f"attempt {attempt}: the downloaded item matches the published copy")
                    return
                print(f"attempt {attempt}: the downloaded item differs:")
                for path in sorted(set(actual) | set(expected)):
                    if actual.get(path) != expected.get(path):
                        print(
                            f"  {path}: published {str(expected.get(path))[:12]}, "
                            f"downloaded {str(actual.get(path))[:12]}"
                        )
            if attempt < args.attempts:
                time.sleep(30)
        sys.exit("the downloaded item does not match the published copy")
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
