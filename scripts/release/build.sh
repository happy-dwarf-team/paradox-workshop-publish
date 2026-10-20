#!/usr/bin/env bash
# Builds the release archive of one target with cargo-dist (spec section 13):
# the pdx-ws binary, the platform's Steamworks steam_api library next to it,
# LICENSE, README.md and THIRD_PARTY_NOTICES, plus the archive's .sha256.
#
# steam_api comes from the steamworks-sys build and must be byte-identical to
# the Steamworks SDK release pinned in steam-api-sdk-1.64.sha256; any other
# file fails the build. cargo-dist's `include` is the same for every target,
# so this script adds the one library of this target to it for this build.
#
# Usage: scripts/release/build.sh <target-triple>
# Output: target/distrib/pdx-ws-<target>.{zip,tar.xz} and its .sha256.
set -euo pipefail

target=${1:?usage: build.sh <target-triple>}
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"

case "$target" in
  *-windows-*) lib=steam_api64.dll sdk_path=win64/steam_api64.dll ext=zip exe=pdx-ws.exe ;;
  *-linux-*) lib=libsteam_api.so sdk_path=linux64/libsteam_api.so ext=tar.xz exe=pdx-ws ;;
  *-apple-*) lib=libsteam_api.dylib sdk_path=osx/libsteam_api.dylib ext=tar.xz exe=pdx-ws ;;
  *) echo "::error::no steam_api for target $target"; exit 1 ;;
esac

sha256() {
  if command -v sha256sum > /dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

# 1. Build, so steamworks-sys has copied its steam_api into its OUT_DIR.
cargo build --profile dist --locked --target "$target"
built=$(find "target/$target/dist/build" -path "*/steamworks-sys-*/out/$lib" | head -n 1)
[ -n "$built" ] || { echo "::error::the steamworks-sys build has no $lib"; exit 1; }

# 2. Check it against the pinned SDK hashes.
expected=$(awk -v path="$sdk_path" '$2 == path {print $1}' scripts/release/steam-api-sdk-1.64.sha256)
actual=$(sha256 "$built")
if [ "$actual" != "$expected" ]; then
  echo "::error::$lib from steamworks-sys has SHA-256 $actual, but Steamworks SDK 1.64's $sdk_path has $expected. Update scripts/release/ for the new SDK only after checking it against the official SDK."
  exit 1
fi
echo "$lib matches Steamworks SDK 1.64 ($actual)"
mkdir -p target/steam-api
cp "$built" "target/steam-api/$lib"

# 3. Add it to cargo-dist's include list for this build only.
config=dist-workspace.toml
cp "$config" "target/steam-api/$config.orig"
trap 'cp "target/steam-api/$config.orig" "$config"' EXIT
sed -i.bak "s|^include = \[\"THIRD_PARTY_NOTICES\"\]$|include = [\"THIRD_PARTY_NOTICES\", \"target/steam-api/$lib\"]|" "$config"
rm -f "$config.bak"
grep -q "target/steam-api/$lib" "$config" || { echo "::error::cannot add $lib to the include list of $config"; exit 1; }

# 4. Build the archive.
dist build --artifacts=local --target "$target"
archive="target/distrib/pdx-ws-$target.$ext"
[ -f "$archive" ] && [ -f "$archive.sha256" ] || { echo "::error::cargo-dist made no $archive"; exit 1; }

# 5. Check what is inside, and that the binary starts from the unpacked
#    archive with nothing but its own folder to load steam_api from.
check=$(mktemp -d)
if [ "$ext" = zip ]; then
  unzip -q "$archive" -d "$check"
else
  tar -xJf "$archive" -C "$check"
fi
dir=$(dirname "$(find "$check" -type f -name "$exe" | head -n 1)")
for file in "$exe" "$lib" LICENSE README.md THIRD_PARTY_NOTICES; do
  [ -f "$dir/$file" ] || { echo "::error::$archive has no $file next to the binary"; exit 1; }
done
[ "$(sha256 "$dir/$lib")" = "$expected" ] || { echo "::error::$archive holds a different $lib"; exit 1; }
runs=yes
if [ "$target" = x86_64-apple-darwin ] && [ "$(uname -m)" = arm64 ]; then
  arch -x86_64 /usr/bin/true 2> /dev/null || runs=no
fi
if [ "$runs" = yes ]; then
  env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH -u DYLD_FALLBACK_LIBRARY_PATH \
    "$dir/$exe" --version
  env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH -u DYLD_FALLBACK_LIBRARY_PATH \
    "$dir/$exe" plan --config tests/fixtures/eu4/workshop.toml > /dev/null
else
  echo "::notice::$target cannot run on this $(uname -m) runner; only its contents were checked"
fi
rm -rf "$check"
echo "built $archive"
