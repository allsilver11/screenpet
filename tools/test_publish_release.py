"""🔄 deploy/publish-release.sh: GitHub 릴리스를 릴레이의 업데이트 폴더(~/.local/share/screenpet-relay/downloads)로 옮긴다.
네트워크 없이: GITHUB_API를 file:// 가짜 API 폴더로 돌려 실제로 돌려 본다."""
import hashlib
import json
import os
import re
import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "deploy" / "publish-release.sh"
TEXT = SCRIPT.read_text(encoding="utf-8")
README = (ROOT / "deploy" / "relay" / "README.md").read_text(encoding="utf-8")

needs_tools = pytest.mark.skipif(
    not all(shutil.which(t) for t in ("bash", "curl", "python3", "sha256sum")), reason="bash/curl/python3/sha256sum 없음"
)

REPO = "allsilver11/screenpet"


def fake_api(root: Path, tag: str, files: dict, digests: dict | None = None) -> Path:
    """api.github.com처럼 보이는 폴더: repos/<repo>/releases/tags/<tag> 와 releases/assets/<id>."""
    api = root / "api"
    base = api / "repos" / REPO / "releases"
    (base / "tags").mkdir(parents=True, exist_ok=True)
    (base / "assets").mkdir(parents=True, exist_ok=True)
    assets = []
    for i, (name, data) in enumerate(files.items(), start=100):
        (base / "assets" / str(i)).write_bytes(data)
        digest = (digests or {}).get(name, "sha256:" + hashlib.sha256(data).hexdigest())
        asset = {"id": i, "name": name, "size": len(data)}
        if digest is not None:
            asset["digest"] = digest
        assets.append(asset)
    (base / "tags" / tag).write_text(json.dumps({"tag_name": tag, "assets": assets}), encoding="utf-8")
    return api


def run(tmp_path: Path, *args, api: Path | None = None, env_extra=None):
    env = dict(os.environ)
    env.pop("GITHUB_TOKEN", None)
    env["SCREENPET_DOWNLOADS_DIR"] = str(tmp_path / "downloads")
    env["GITHUB_API"] = (api or tmp_path / "no-api").as_uri()
    env["HOME"] = str(tmp_path / "home")
    env.update(env_extra or {})
    return subprocess.run(["bash", str(SCRIPT), *args], capture_output=True, text=True, env=env, timeout=60)


def release_files(version: str, salt: bytes = b""):
    return {
        f"ScreenPet_{version}_x64-setup.exe": b"MZ windows " + version.encode() + salt + os.urandom(3000),
        f"ScreenPet_{version}_universal.dmg": b"koly mac " + version.encode() + salt + os.urandom(5000),
    }


def test_script_is_strict_bash_and_validates_the_version():
    assert TEXT.startswith("#!/usr/bin/env bash\n")
    assert "set -euo pipefail" in TEXT
    assert os.access(SCRIPT, os.X_OK)
    assert 'REPO="allsilver11/screenpet"' in TEXT
    assert "^v[0-9]{1,9}\\.[0-9]{1,9}\\.[0-9]{1,9}$" in TEXT
    # 토큰이 있으면 쓴다 (저장소가 비공개가 돼도 받을 수 있게)
    assert 'Authorization: Bearer $GITHUB_TOKEN' in TEXT and "Accept: application/octet-stream" in TEXT
    assert ".local/share/screenpet-relay/downloads" in TEXT


def test_order_is_files_then_latest_json_then_delete_old():
    body = TEXT[TEXT.index("SHA-256 $got does not match") :]
    win = body.index('mv -f "$WORK/$WIN.part" "$DEST/$WIN"')
    mac = body.index('mv -f "$WORK/$MAC.part" "$DEST/$MAC"')
    latest = body.index('mv -f "$WORK/latest.json" "$DEST/latest.json"')
    delete = body.index('rm -rf -- "$old"')
    assert win < latest and mac < latest < delete
    # 임시 폴더는 DEST 안 (같은 파일 시스템이라 mv가 원자적), 실패해도 지운다
    assert 'mktemp -d "$DEST/.publish-XXXXXX"' in TEXT
    assert "trap 'rm -rf \"$WORK\"' EXIT" in TEXT


@needs_tools
@pytest.mark.parametrize("arg", ["", "0.6.3", "v0.6", "v0.6.3.1", "v0.6.3-beta", "v0.6.x", "v0.6.3; rm -rf ~", "../v0.6.3", "v0.6.3\n"])
def test_bad_versions_are_refused_before_anything_happens(tmp_path, arg):
    r = run(tmp_path, arg)
    assert r.returncode == 2, r.stderr
    assert not (tmp_path / "downloads").exists()


@needs_tools
def test_needs_exactly_one_argument(tmp_path):
    assert run(tmp_path).returncode == 2
    assert run(tmp_path, "v0.6.3", "v0.6.4").returncode == 2
    assert "usage" in run(tmp_path).stderr


@needs_tools
def test_publishes_and_keeps_only_the_new_release(tmp_path):
    dl = tmp_path / "downloads"
    dl.mkdir()
    for old in ("ScreenPet_0.6.1_x64-setup.exe", "ScreenPet_0.6.1_universal.dmg", "ScreenPet_0.6.2_x64-setup.exe", "ScreenPet_0.6.2_universal.dmg"):
        (dl / old).write_bytes(b"old")
    (dl / "latest.json").write_text('{"version":"0.6.2"}', encoding="utf-8")
    (dl / "README.txt").write_text("keep me", encoding="utf-8")
    files = release_files("0.6.3")
    api = fake_api(tmp_path, "v0.6.3", files)
    r = run(tmp_path, "v0.6.3", api=api)
    assert r.returncode == 0, r.stderr + r.stdout
    latest = json.loads((dl / "latest.json").read_text(encoding="utf-8"))
    assert latest == {
        "version": "0.6.3",
        "files": {
            "windows": {
                "name": "ScreenPet_0.6.3_x64-setup.exe",
                "sha256": hashlib.sha256(files["ScreenPet_0.6.3_x64-setup.exe"]).hexdigest(),
                "size": len(files["ScreenPet_0.6.3_x64-setup.exe"]),
            },
            "macos": {
                "name": "ScreenPet_0.6.3_universal.dmg",
                "sha256": hashlib.sha256(files["ScreenPet_0.6.3_universal.dmg"]).hexdigest(),
                "size": len(files["ScreenPet_0.6.3_universal.dmg"]),
            },
        },
    }
    for name, data in files.items():
        assert (dl / name).read_bytes() == data
    # 새 릴리스만 남는다 (ScreenPet_* 아닌 파일은 건드리지 않는다), 임시 파일도 없다
    assert sorted(p.name for p in dl.iterdir()) == sorted([*files, "latest.json", "README.txt"])
    assert "removed 4 older file(s)" in r.stdout
    assert "ScreenPet_0.6.3_x64-setup.exe" in r.stdout and "sha256" in r.stdout
    # 릴레이(같은 사용자)가 읽을 수 있게
    assert (dl / "latest.json").stat().st_mode & 0o444 == 0o444


@needs_tools
@pytest.mark.parametrize(
    "case",
    ["bad_digest", "no_digest", "missing_asset", "no_release"],
)
def test_a_failed_run_changes_nothing_and_leaves_no_temp_files(tmp_path, case):
    dl = tmp_path / "downloads"
    dl.mkdir()
    old = {"ScreenPet_0.6.2_x64-setup.exe": b"old win", "ScreenPet_0.6.2_universal.dmg": b"old mac"}
    for n, d in old.items():
        (dl / n).write_bytes(d)
    old_latest = '{"version":"0.6.2"}'
    (dl / "latest.json").write_text(old_latest, encoding="utf-8")
    files = release_files("0.6.3")
    mac = "ScreenPet_0.6.3_universal.dmg"
    if case == "bad_digest":
        api = fake_api(tmp_path, "v0.6.3", files, {mac: "sha256:" + "0" * 64})
    elif case == "no_digest":
        api = fake_api(tmp_path, "v0.6.3", files, {mac: None})
    elif case == "missing_asset":
        api = fake_api(tmp_path, "v0.6.3", {k: v for k, v in files.items() if k != mac})
    else:
        api = fake_api(tmp_path, "v0.6.2", files)
    r = run(tmp_path, "v0.6.3", api=api)
    assert r.returncode != 0
    assert "publish-release:" in r.stderr
    assert (dl / "latest.json").read_text(encoding="utf-8") == old_latest
    assert sorted(p.name for p in dl.iterdir()) == sorted([*old, "latest.json"]), "temp files or new files left behind"


@needs_tools
def test_leftovers_of_a_killed_run_are_cleaned(tmp_path):
    dl = tmp_path / "downloads"
    (dl / ".publish-abc123").mkdir(parents=True)
    (dl / ".publish-abc123" / "ScreenPet_0.6.3_universal.dmg.part").write_bytes(b"half")
    (dl / "ScreenPet_0.6.3_universal.dmg.part").write_bytes(b"half")
    (dl / "latest.json.tmp").write_text("{", encoding="utf-8")
    files = release_files("0.6.3")
    r = run(tmp_path, "v0.6.3", api=fake_api(tmp_path, "v0.6.3", files))
    assert r.returncode == 0, r.stderr
    assert sorted(p.name for p in dl.iterdir()) == sorted([*files, "latest.json"])


@needs_tools
def test_republishing_the_same_version_keeps_its_files(tmp_path):
    files = release_files("0.6.3")
    api = fake_api(tmp_path, "v0.6.3", files)
    assert run(tmp_path, "v0.6.3", api=api).returncode == 0
    r = run(tmp_path, "v0.6.3", api=api)
    assert r.returncode == 0, r.stderr
    dl = tmp_path / "downloads"
    assert sorted(p.name for p in dl.iterdir()) == sorted([*files, "latest.json"])
    assert "removed 0 older file(s)" in r.stdout


def test_latest_json_matches_what_the_app_parses():
    """앱(screenpet-core::update::check)이 읽는 열쇠 이름 그대로 쓴다."""
    core = (ROOT / "crates" / "screenpet-core" / "src" / "update.rs").read_text(encoding="utf-8")
    for key in ('"windows"', '"macos"'):
        assert key in core and key in TEXT
    for field in ('"version"', '"files"', '"name"', '"sha256"', '"size"'):
        assert field in TEXT, field
        assert f"[{field}]" in core or f"{field}" in core
    assert re.search(r"MAX_SIZE=\$\(\(300 \* 1024 \* 1024\)\)", TEXT)
    assert "pub const MAX_DOWNLOAD: u64 = 300 * 1024 * 1024;" in core


def test_readme_documents_publishing():
    assert "deploy/publish-release.sh v" in README
    assert "GITHUB_TOKEN" in README
    assert "/downloads/latest.json" in README
