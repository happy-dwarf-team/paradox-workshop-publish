"""Experiment E4 (issue #6): publish a private EU4 Workshop item with steamcmd from a GitHub-hosted runner.

Throwaway code on the e4-run branch; never merged. Reads e4/run.json, installs steamcmd, restores the cached
session from STEAM_CONFIG_VDF (gzip + base64 of steamcmd's config/config.vdf), publishes e4/content, downloads
the item again and compares hashes, then asks the Steam Web API for the item's details. It never prints the
session or the password; it reports only whether the session file changed during the run.
"""
import base64, gzip, hashlib, json, os, platform, re, shutil, subprocess, sys, tarfile, time
import urllib.parse, urllib.request, zipfile
from pathlib import Path

APP = 236850
ROOT = Path(__file__).resolve().parent
RUN = json.loads((ROOT / "run.json").read_text())
TMP = Path(os.environ.get("RUNNER_TEMP", "/tmp")) / "e4"
WIN = platform.system() == "Windows"
STEAMCMD_DIR = TMP / "steamcmd"
SUMMARY = []


def log(msg=""):
    print(msg, flush=True)


def summary(line):
    SUMMARY.append(line)
    log(line)


def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()


def tree_hashes(root):
    root = Path(root)
    return {p.relative_to(root).as_posix(): sha(p) for p in sorted(root.rglob("*")) if p.is_file()}


def steamcmd(*args, timeout=900, user=None):
    exe = STEAMCMD_DIR / ("steamcmd.exe" if WIN else "steamcmd.sh")
    cmd = [str(exe)] + (["+login", user] if user else []) + list(args) + ["+quit"]
    started = time.time()
    try:
        p = subprocess.run(cmd, stdin=subprocess.DEVNULL, capture_output=True, text=True, errors="replace",
                           timeout=timeout)
        out, code = p.stdout + p.stderr, p.returncode
    except subprocess.TimeoutExpired as e:
        out, code = (e.stdout or b"").decode(errors="replace") if isinstance(e.stdout, bytes) else (e.stdout or ""), "timeout"
    return out, code, round(time.time() - started, 1)


def show(title, out, code, secs):
    log(f"::group::{title} (exit {code}, {secs} s)")
    # Drop the self-update progress noise; keep everything else (the user name is masked by Actions).
    for line in out.splitlines():
        if not re.search(r"\[\s*\d+%\]|Downloading update|Extracting package|Installing update|Verifying installation", line):
            log(line)
    log("::endgroup::")


def install():
    STEAMCMD_DIR.mkdir(parents=True, exist_ok=True)
    if WIN:
        url, archive = "https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip", TMP / "steamcmd.zip"
        urllib.request.urlretrieve(url, archive)
        zipfile.ZipFile(archive).extractall(STEAMCMD_DIR)
    else:
        subprocess.run(["sudo", "apt-get", "install", "-y", "-qq", "lib32gcc-s1"], check=True, stdout=subprocess.DEVNULL)
        url, archive = "https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz", TMP / "steamcmd.tgz"
        urllib.request.urlretrieve(url, archive)
        tarfile.open(archive).extractall(STEAMCMD_DIR)
    out, code, secs = steamcmd(timeout=600)  # first run updates steamcmd itself (Windows exits with 7)
    summary(f"- steamcmd installed and self-updated: exit {code}, {secs} s")


def config_paths():
    homes = [STEAMCMD_DIR, Path.home() / "Steam", Path.home() / ".steam", Path.home() / ".local/share/Steam"]
    found = []
    for h in homes:
        if h.exists():
            found += [p for p in h.rglob("config.vdf") if p.parent.name == "config" and "workshop" not in p.parts]
    return sorted(set(p.resolve() for p in found))


def restore_session():
    blob = gzip.decompress(base64.b64decode(os.environ["STEAM_CONFIG_VDF"]))
    paths = config_paths() or [STEAMCMD_DIR / "config" / "config.vdf"]
    for p in paths:
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(blob)
    summary(f"- session restored to {len(paths)} location(s): "
            + ", ".join(str(p).replace(str(Path.home()), "~").replace(str(TMP), "$RUNNER_TEMP/e4") for p in paths))
    summary(f"- secret size: {len(os.environ['STEAM_CONFIG_VDF'])} characters (GitHub limit 48 KB)")
    return paths, hashlib.sha256(blob).hexdigest()


def build_content():
    content = TMP / "content"
    shutil.copytree(ROOT / "content", content)
    # A deterministic 256 KiB binary stands in for a plugin DLL (no binaries in the repository).
    block, data = b"paradox-workshop-publish E4", bytearray()
    while len(data) < 256 * 1024:
        block = hashlib.sha256(block).digest()
        data += block
    (content / "anvil/plugins/probe/probe.bin").write_bytes(bytes(data))
    (content / "README.txt").write_text(
        "Private test item for happy-dwarf-team/paradox-workshop-publish#6 (experiment E4).\n"
        "Nothing in this mod changes the game.\n"
        f"Revision {RUN['revision']} ({RUN['label']}).\n", newline="\n")
    if RUN["publishedfileid"] != "0":
        with open(content / "descriptor.mod", "a", newline="\n") as f:
            f.write(f'remote_file_id="{RUN["publishedfileid"]}"\n')
    return content


def vdf_escape(s):
    return str(s).replace("\\", "\\\\").replace('"', '\\"')


def write_vdf(path, fields):
    body = "".join(f'\t"{k}" "{vdf_escape(v)}"\n' for k, v in fields.items())
    path.write_text('"workshopitem"\n{\n' + body + "}\n", newline="\n")


def read_published_id(vdf):
    m = re.search(r'"publishedfileid"\s+"(\d+)"', vdf.read_text())
    return m.group(1) if m else None


def item_details(item):
    data = urllib.parse.urlencode({"itemcount": 1, "publishedfileids[0]": item}).encode()
    with urllib.request.urlopen("https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/",
                                data, timeout=30) as r:
        d = json.load(r)["response"]["publishedfiledetails"][0]
    keep = ["result", "visibility", "title", "file_size", "time_created", "time_updated", "consumer_app_id",
            "banned", "tags"]
    return {k: d.get(k) for k in keep}


def grep_logs():
    hits = []
    for p in list(STEAMCMD_DIR.rglob("*.log")) + list(STEAMCMD_DIR.rglob("workshop_log.txt")):
        for line in p.read_text(errors="replace").splitlines():
            if re.search(r"(?i)workshop|publishedfile|upload|error|fail", line):
                hits.append(f"{p.name}: {line.strip()}")
    return hits[-60:]


def main():
    user = os.environ["STEAM_USERNAME"]
    summary(f"## E4 {RUN['label']} on {platform.system()} ({os.environ.get('ImageOS', '?')})")
    install()
    paths, before = restore_session()
    content = build_content()
    source = tree_hashes(content)
    summary("- content: " + ", ".join(f"`{k}` {v[:12]}" for k, v in source.items()))

    vdf = TMP / "item.vdf"
    fields = {"appid": APP, "publishedfileid": RUN["publishedfileid"], "contentfolder": str(content)}
    if "previewfile" in RUN["keys"]:
        fields["previewfile"] = str(ROOT / "preview.png")
    if "title" in RUN["keys"]:
        fields["title"] = "paradox-workshop-publish E4 probe (private test, do not subscribe)"
    if "description" in RUN["keys"]:
        fields["description"] = "Private test item for experiment E4 (steamcmd from GitHub Actions). Nothing here changes the game."
    if "visibility" in RUN["keys"]:
        fields["visibility"] = 2
    fields.update(RUN.get("extra_vdf", {}))
    fields["changenote"] = RUN["change_note"]
    write_vdf(vdf, fields)
    summary("- VDF keys: " + ", ".join(fields))

    out, code, secs = steamcmd("+workshop_build_item", str(vdf), user=user)
    show("steamcmd workshop_build_item", out, code, secs)
    item = read_published_id(vdf)
    ok_line = [l.strip() for l in out.splitlines() if re.search(r"(?i)publishedfileid|success|ERROR|FAILED|Steam Guard", l)]
    summary(f"- publish: exit {code}, {secs} s, publishedfileid in VDF after run: {item}")
    for l in ok_line[-10:]:
        summary(f"  - `{l}`")
    for l in grep_logs():
        log(f"log | {l}")

    if item and item != "0":
        time.sleep(5)
        out, code, secs = steamcmd("+workshop_download_item", str(APP), item, "validate", user=user)
        show("steamcmd workshop_download_item", out, code, secs)
        dirs = [d for base in [STEAMCMD_DIR, Path.home() / "Steam", Path.home() / ".local/share/Steam"] if base.exists()
                for d in base.rglob(item) if d.is_dir() and d.parent.name == str(APP)]
        if dirs:
            got = tree_hashes(dirs[0])
            same = got == source
            summary(f"- download: exit {code}, {secs} s, files byte-identical to source: {same}")
            if not same:
                for k in sorted(set(got) | set(source)):
                    if got.get(k) != source.get(k):
                        summary(f"  - differs: `{k}` source {str(source.get(k))[:12]} downloaded {str(got.get(k))[:12]}")
        else:
            summary(f"- download: exit {code}, {secs} s, downloaded folder not found")
        try:
            summary(f"- GetPublishedFileDetails: `{json.dumps(item_details(item))}`")
        except Exception as e:  # noqa: BLE001 - recorded, not fatal
            summary(f"- GetPublishedFileDetails failed: {e}")

    if RUN.get("fail_tests"):
        bad = TMP / "bad.vdf"
        write_vdf(bad, {"appid": APP, "publishedfileid": item or "0", "contentfolder": str(TMP / "missing-folder"),
                        "changenote": "E4 deliberate failure"})
        out, code, secs = steamcmd("+workshop_build_item", str(bad), user=user)
        show("deliberate failure: missing content folder", out, code, secs)
        summary(f"- failure test, missing content folder: exit {code}; "
                + " | ".join(l.strip() for l in out.splitlines() if re.search(r"(?i)error|fail", l))[-400:])
        out, code, secs = steamcmd(timeout=120, user="pwp-e4-no-such-user-for-testing")
        show("deliberate failure: login without a cached session", out, code, secs)
        summary(f"- failure test, unknown user without session: exit {code}, {secs} s; "
                + " | ".join(l.strip() for l in out.splitlines() if re.search(r"(?i)login|password|fail|guard|cached", l))[-400:])

    after = {sha(p) for p in paths if p.exists()}
    summary(f"- session file changed during the run: {after != {before}} (not written back to the secret)")
    for p in paths:
        p.unlink(missing_ok=True)
    Path(os.environ.get("GITHUB_STEP_SUMMARY", os.devnull)).open("a").write("\n".join(SUMMARY) + "\n")
    if not item or item == "0":
        sys.exit(1)


if __name__ == "__main__":
    main()
