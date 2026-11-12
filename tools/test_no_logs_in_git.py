"""로그는 git에 안 들어감: 추적 중인 파일 중 `*.log`나 `logs/` 폴더 아래 것이 있으면 실패. `.gitignore`도 둘을 막는지 봄."""
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent


def _tracked():
    """git이 추적하는 파일 목록 (git 저장소가 아니면 건너뜀)"""
    try:
        out = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, capture_output=True, check=True)
    except (OSError, subprocess.CalledProcessError):
        pytest.skip("not a git checkout")
    return [p for p in out.stdout.decode("utf-8", "replace").split("\0") if p]


def _is_log(path):
    """로그 파일 모양인지: `.log`로 끝나거나 경로에 `logs` 폴더가 있음"""
    parts = path.split("/")
    return parts[-1].lower().endswith(".log") or any(p.lower() == "logs" for p in parts[:-1])


def test_the_rule_matches_log_files():
    for bad in ("network.log", "a/b/crash.log", "logs/x.txt", "deploy/logs/relay.json", "X.LOG"):
        assert _is_log(bad), bad
    for ok in ("docs/architecture/logging.md", "crates/screenpet-log/src/lib.rs", "catalog.json", "logs.md"):
        assert not _is_log(ok), ok


def test_no_tracked_file_is_a_log():
    tracked = _tracked()
    assert tracked, "git ls-files returned nothing"
    logs = [p for p in tracked if _is_log(p)]
    assert logs == [], logs


def test_gitignore_keeps_logs_out():
    lines = {l.strip() for l in (ROOT / ".gitignore").read_text(encoding="utf-8").splitlines()}
    assert "*.log" in lines and "logs/" in lines
