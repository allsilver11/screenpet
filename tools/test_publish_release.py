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


FAKE_SIG = "5a" * 64


def fake_signer(tmp_path: Path, verify_ok: bool = True) -> Path:
    """screenpet-update-sign 대신: sign은 고정 서명, verify는 정한 대로 (진짜 서명은 Rust 시험 update_sign.rs가 본다)."""
    p = tmp_path / "fake-sign"
    p.write_text(
        "#!/bin/sh\n"
        f'echo "$*" >> "{tmp_path}/sign.log"\n'
        'case "$1" in\n'
        f'  sign) [ -r "$2" ] && [ -r "$3" ] && printf %s {FAKE_SIG} ;;\n'
        f"  verify) exit {0 if verify_ok else 1} ;;\n"
        "  *) exit 2 ;;\n"
        "esac\n",
        encoding="utf-8",
    )
    p.chmod(0o755)
    return p


def run(tmp_path: Path, *args, api: Path | None = None, env_extra=None, verify_ok=True):
    env = dict(os.environ)
    env.pop("GITHUB_TOKEN", None)
    key = tmp_path / "update-key.txt"
    key.write_text("00" * 32 + "\n", encoding="utf-8")
    env["SCREENPET_UPDATE_KEY"] = str(key)
    env["SCREENPET_UPDATE_SIGN"] = str(fake_signer(tmp_path, verify_ok))
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
    sig = body.index('mv -f "$WORK/latest.json.sig" "$DEST/latest.json.sig"')
    delete = body.index('rm -rf -- "$old"')
    assert win < latest and mac < latest < sig < delete
    # 서명은 옮기기 전에 만들고, 앱에 든 키로 확인한다
    signing = TEXT.index('"$SIGN" sign "$KEY" "$WORK/latest.json"')
    checking = TEXT.index('"$SIGN" verify "$WORK/latest.json" "$WORK/latest.json.sig"')
    assert signing < checking < TEXT.index('mv -f "$WORK/$WIN.part"')
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
    assert sorted(p.name for p in dl.iterdir()) == sorted([*files, "latest.json", "latest.json.sig", "README.txt"])
    assert (dl / "latest.json.sig").read_text(encoding="utf-8") == FAKE_SIG
    log = (tmp_path / "sign.log").read_text(encoding="utf-8")
    assert "sign " in log and "verify " in log
    assert "removed 4 older file(s)" in r.stdout
    assert "ScreenPet_0.6.3_x64-setup.exe" in r.stdout and "sha256" in r.stdout
    # 릴레이(같은 사용자)가 읽을 수 있게
    assert (dl / "latest.json").stat().st_mode & 0o444 == 0o444


@needs_tools
@pytest.mark.parametrize(
    "case",
    ["bad_digest", "no_digest", "missing_asset", "no_release", "wrong_signing_key"],
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
    elif case == "no_release":
        api = fake_api(tmp_path, "v0.6.2", files)
    else:
        api = fake_api(tmp_path, "v0.6.3", files)
    r = run(tmp_path, "v0.6.3", api=api, verify_ok=case != "wrong_signing_key")
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
    assert sorted(p.name for p in dl.iterdir()) == sorted([*files, "latest.json", "latest.json.sig"])


@needs_tools
def test_republishing_the_same_version_keeps_its_files(tmp_path):
    files = release_files("0.6.3")
    api = fake_api(tmp_path, "v0.6.3", files)
    assert run(tmp_path, "v0.6.3", api=api).returncode == 0
    r = run(tmp_path, "v0.6.3", api=api)
    assert r.returncode == 0, r.stderr
    dl = tmp_path / "downloads"
    assert sorted(p.name for p in dl.iterdir()) == sorted([*files, "latest.json", "latest.json.sig"])
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


@needs_tools
def test_no_signing_key_or_signer_means_nothing_is_published(tmp_path):
    files = release_files("0.6.3")
    api = fake_api(tmp_path, "v0.6.3", files)
    r = run(tmp_path, "v0.6.3", api=api, env_extra={"SCREENPET_UPDATE_KEY": str(tmp_path / "missing-key")})
    assert r.returncode != 0 and "signing key" in r.stderr
    r = run(tmp_path, "v0.6.3", api=api, env_extra={"SCREENPET_UPDATE_SIGN": str(tmp_path / "missing-signer")})
    assert r.returncode != 0 and "screenpet-update-sign" in r.stderr
    dl = tmp_path / "downloads"
    assert not dl.exists() or not any(dl.iterdir())


def test_the_signing_key_never_reaches_the_relay():
    """비밀 키는 발행 단계(스크립트 + screenpet-update-sign)만 읽는다: 릴레이 코드는 키 파일·서명 함수를 모른다."""
    relay = ROOT / "crates" / "screenpet-relay"
    texts = [p.read_text(encoding="utf-8") for p in relay.rglob("*") if p.is_file() and p.suffix in (".rs", ".toml")]
    texts.append((ROOT / "deploy" / "relay" / "screenpet-relay.service").read_text(encoding="utf-8"))
    for t in texts:
        for word in ("screenpet-update-key", "SCREENPET_UPDATE_KEY", "sign_latest", "SigningKey", "screenpet-update-sign"):
            assert word not in t, word
    # 릴레이는 screenpet-core를 시험에서만 쓴다 (서명 도우미가 릴레이 바이너리에 들어가지 않는다)
    cargo = (relay / "Cargo.toml").read_text(encoding="utf-8")
    deps = cargo.split("[dependencies]")[1].split("[dev-dependencies]")[0]
    assert "screenpet-core" not in deps
    # 키는 저장소 밖에 두고, 혹시 안에 놓여도 올라가지 않게
    assert "$HOME/personal/screenpet-update-key.txt" in TEXT
    assert "screenpet-update-key*.txt" in (ROOT / ".gitignore").read_text(encoding="utf-8").splitlines()
    tracked = subprocess.run(["git", "ls-files"], cwd=ROOT, capture_output=True, text=True).stdout
    assert "screenpet-update-key" not in tracked


def test_readme_documents_the_signing_key():
    assert "screenpet-update-key.txt" in README and "back" in README.lower()
    assert "screenpet-update-sign" in README


def test_the_relay_unit_cannot_read_the_signing_key_folder():
    unit = (ROOT / "deploy" / "relay" / "screenpet-relay.service").read_text(encoding="utf-8")
    lines = [l.strip() for l in unit.splitlines()]
    assert "InaccessiblePaths=-%h/personal" in lines
    # 릴레이가 쓰는 곳은 그 밖이다
    assert "ExecStart=%h/.local/bin/screenpet-relay --bind 0.0.0.0:9411 --data %h/.local/share/screenpet-relay" in lines
    assert "ReadWritePaths=%h/.local/share/screenpet-relay" in lines
    assert "~/.config/systemd/user/screenpet-relay.service" in README and "InaccessiblePaths" in README
