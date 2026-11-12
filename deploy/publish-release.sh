#!/usr/bin/env bash
# 🔄 Publish a GitHub release to the relay's update folder, so the apps can
# update from https://pet.danni.day/downloads/ (see deploy/relay/README.md).
#
#   deploy/publish-release.sh vX.Y.Z
#
# Run on the relay server after CI has finished the release. It downloads the
# Windows and macOS installers through the GitHub API (with $GITHUB_TOKEN if
# set, so it keeps working when the repository is private), checks each one
# against the size and SHA-256 GitHub reports, then:
#   1. downloads to a temporary folder inside the downloads folder,
#   2. verifies the files,
#   3. renames the installers into place,
#   4. atomically replaces latest.json,
#   5. deletes every other ScreenPet_* installer (only the new release stays).
# latest.json never names a file that isn't there. A failed run leaves the
# published files as they were and removes its temporary files.
#
# Environment: GITHUB_TOKEN (optional), SCREENPET_DOWNLOADS_DIR (default
# ~/.local/share/screenpet-relay/downloads), GITHUB_API (default
# https://api.github.com; the tests point it at file://).
set -euo pipefail

REPO="allsilver11/screenpet"
API="${GITHUB_API:-https://api.github.com}"
DEST="${SCREENPET_DOWNLOADS_DIR:-$HOME/.local/share/screenpet-relay/downloads}"
# The app refuses anything bigger (screenpet_core::update::MAX_DOWNLOAD).
MAX_SIZE=$((300 * 1024 * 1024))

die() {
    echo "publish-release: $*" >&2
    exit 1
}

if [ "$#" -ne 1 ]; then
    echo "usage: $0 vX.Y.Z" >&2
    exit 2
fi
TAG="$1"
if ! [[ "$TAG" =~ ^v[0-9]{1,9}\.[0-9]{1,9}\.[0-9]{1,9}$ ]]; then
    echo "publish-release: the version must look like v0.6.3, not '$TAG'" >&2
    exit 2
fi
VERSION="${TAG#v}"
WIN="ScreenPet_${VERSION}_x64-setup.exe"
MAC="ScreenPet_${VERSION}_universal.dmg"

for tool in curl python3 sha256sum; do
    command -v "$tool" >/dev/null || die "needs $tool"
done

mkdir -p "$DEST"
chmod 755 "$DEST"
# Leftovers of an earlier run that was killed before it could clean up.
find "$DEST" -maxdepth 1 -name '.publish-*' -exec rm -rf {} +
find "$DEST" -maxdepth 1 -type f \( -name '*.part' -o -name 'latest.json.tmp*' \) -delete

# Inside DEST, so the renames below stay on one file system (atomic).
WORK="$(mktemp -d "$DEST/.publish-XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
trap 'exit 130' INT TERM

AUTH=()
if [ -n "${GITHUB_TOKEN:-}" ]; then
    AUTH=(-H "Authorization: Bearer $GITHUB_TOKEN")
fi
CURL=(curl -fsSL --proto '=https,file' --retry 3 --connect-timeout 20 "${AUTH[@]}")

echo "publish-release: reading $REPO release $TAG"
"${CURL[@]}" -H "Accept: application/vnd.github+json" -o "$WORK/release.json" \
    "$API/repos/$REPO/releases/tags/$TAG" || die "no release $TAG on GitHub (or no access)"

# "<id> <size> <sha256>" of one asset, as GitHub reports it.
asset_info() {
    python3 - "$WORK/release.json" "$1" <<'PY'
import json, re, sys
release = json.load(open(sys.argv[1], encoding="utf-8"))
for a in release.get("assets", []):
    if a.get("name") == sys.argv[2]:
        digest = a.get("digest") or ""
        m = re.fullmatch(r"sha256:([0-9a-fA-F]{64})", digest)
        if not m:
            sys.exit(f"{sys.argv[2]}: GitHub reports no sha256 digest")
        print(a["id"], int(a["size"]), m.group(1).lower())
        sys.exit(0)
sys.exit(f"{sys.argv[2]}: not in the release")
PY
}

declare -A SIZE SHA
for name in "$WIN" "$MAC"; do
    read -r id size want < <(asset_info "$name") || die "$name: cannot use this release"
    [ "$size" -gt 0 ] && [ "$size" -le "$MAX_SIZE" ] || die "$name: size $size is out of range"
    echo "publish-release: downloading $name ($size bytes)"
    # Through the API with Accept: octet-stream (works for private repositories).
    # curl does not pass the token on to the storage host it is redirected to.
    "${CURL[@]}" -H "Accept: application/octet-stream" -o "$WORK/$name.part" \
        "$API/repos/$REPO/releases/assets/$id" || die "$name: download failed"
    got_size=$(stat -c %s "$WORK/$name.part")
    [ "$got_size" -eq "$size" ] || die "$name: got $got_size bytes, expected $size"
    got=$(sha256sum "$WORK/$name.part" | cut -d' ' -f1)
    [ "$got" = "$want" ] || die "$name: SHA-256 $got does not match GitHub's $want"
    chmod 644 "$WORK/$name.part"
    SIZE[$name]=$size
    SHA[$name]=$got
done

python3 - "$WORK/latest.json" "$VERSION" "$WIN" "${SHA[$WIN]}" "${SIZE[$WIN]}" "$MAC" "${SHA[$MAC]}" "${SIZE[$MAC]}" <<'PY'
import json, sys
out, version, wn, ws, wz, mn, ms, mz = sys.argv[1:]
latest = {
    "version": version,
    "files": {
        "windows": {"name": wn, "sha256": ws, "size": int(wz)},
        "macos": {"name": mn, "sha256": ms, "size": int(mz)},
    },
}
with open(out, "w", encoding="utf-8") as f:
    json.dump(latest, f, separators=(",", ":"))
    f.write("\n")
PY
chmod 644 "$WORK/latest.json"

# Installers first, then latest.json: it never names a missing file.
mv -f "$WORK/$WIN.part" "$DEST/$WIN"
mv -f "$WORK/$MAC.part" "$DEST/$MAC"
mv -f "$WORK/latest.json" "$DEST/latest.json"

# Only the new release stays.
removed=0
for old in "$DEST"/ScreenPet_*; do
    [ -e "$old" ] || continue
    case "$(basename "$old")" in
        "$WIN" | "$MAC") ;;
        *)
            rm -rf -- "$old"
            echo "publish-release: removed $(basename "$old")"
            removed=$((removed + 1))
            ;;
    esac
done

echo "publish-release: published $TAG to $DEST"
echo "  $WIN  ${SIZE[$WIN]} bytes  sha256 ${SHA[$WIN]}"
echo "  $MAC  ${SIZE[$MAC]} bytes  sha256 ${SHA[$MAC]}"
echo "  removed $removed older file(s)"
echo "  check: curl -s https://pet.danni.day/downloads/latest.json"
