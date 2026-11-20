"""⏱️ tools/ci_art_changed.py: 바뀐 파일 목록 → 그림 테스트를 돌릴지(run) 건너뛸지(skip)."""
import re
import shutil
import subprocess
from pathlib import Path

import pytest

import ci_art_changed as cac

ROOT = Path(__file__).resolve().parent.parent
SHA_A = "a" * 40
SHA_B = "b" * 40


@pytest.mark.parametrize("path", [
    "tools/draw_pets.py", "tools/draw_house.py", "tools/sprite_spec.py", "tools/conftest.py",
    "tools/test_draw_frog.py", "tools/requirements.txt", "tools/requirements-pack.txt", "tools/pack_webp.py",
    "ui/pets/cat/sheet_baby.png", "ui/pets/cat/pet.json", "ui/effects/snow_cloud.png", "ui/objects/dropping.png",
    "ui/house/house.json", "ui/house_xmas/a.png", "ui/sleigh/sleigh.png", "ui/vehicles/car/sheet.png",
    "ui/main.js", "ui/index.html", "ui/style.css", "src-tauri/src/prop.rs", "crates/screenpet-core/src/food.rs",
    ".github/workflows/build.yml", "tools/ci_art_changed.py",
])
def test_art_files(path):
    assert cac.is_art(path)


@pytest.mark.parametrize("path", [
    "crates/screenpet-core/src/update.rs", "crates/screenpet-net/src/lib.rs", "src-tauri/src/update.rs",
    "ui/house.html", "ui/house.js", "ui/board.js", "tools/test_board_ui.py", "tools/board_harness.js",
    "deploy/publish-release.sh", "docs/releases/v0.7.1.md", "README.md", "Cargo.lock",
])
def test_code_files(path):
    assert not cac.is_art(path)


def test_code_only_push_skips_the_art_tests():
    mode, why = cac.decide("push", "refs/heads/main", SHA_A, ["crates/screenpet-core/src/update.rs", "ui/board.js"])
    assert mode == "skip" and "test_draw_" in why and "2개" in why


def test_any_art_file_runs_everything():
    mode, why = cac.decide("push", "refs/heads/main", SHA_A, ["README.md", "ui/pets/cat/sheet_baby.png"])
    assert mode == "run" and "ui/pets/cat/sheet_baby.png" in why


def test_nothing_changed_skips():
    assert cac.decide("push", "refs/heads/main", SHA_A, [])[0] == "skip"


@pytest.mark.parametrize("event,ref,before,changed", [
    ("pull_request", "refs/pull/3/merge", SHA_A, ["README.md"]),
    ("workflow_dispatch", "refs/heads/main", SHA_A, ["README.md"]),
    ("push", "refs/tags/v0.7.1", SHA_A, ["README.md"]),
    ("push", "refs/heads/new", "0" * 40, ["README.md"]),
    ("push", "refs/heads/new", "", ["README.md"]),
    ("push", "refs/heads/main", SHA_A, None),  # before가 기록에 없음 (강제 push 등)
])
def test_anything_unknown_runs_everything(event, ref, before, changed):
    assert cac.decide(event, ref, before, changed)[0] == "run"


def test_every_path_the_art_tests_read_is_art():
    """그림 테스트·생성기가 읽는 저장소 파일이 바뀌면 그림 테스트가 돌아야 한다: 새 경로를 적으면 ART_PATTERNS도 고친다."""
    chain = re.compile(r'(?:ROOT|parent\.parent|root)((?:\s*/\s*"[^"{}]+")+)')
    found = set()
    for f in [*ROOT.glob("tools/test_draw_*.py"), *ROOT.glob("tools/draw_*.py")]:
        for m in chain.finditer(f.read_text(encoding="utf-8")):
            parts = re.findall(r'"([^"]+)"', m.group(1))
            path = "/".join(parts)
            if path == "ui":
                continue  # ui/ 자체는 "빠진 폴더(gifts·pets_pixel…)가 없다" 확인용으로만 씀
            if parts[0] in {"ui", "src-tauri", "crates", "tools"} and (ROOT / path).is_file():
                found.add(path)
            elif parts[0] in {"ui", "src-tauri", "crates", "tools"} and (ROOT / path).is_dir():
                found.add(path + "/x.png")
    assert len(found) > 10, found
    assert not [p for p in found if not cac.is_art(p)]


needs_git = pytest.mark.skipif(not shutil.which("git"), reason="git 없음")


def git(cwd, *args):
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


@needs_git
def test_changed_files_from_git(tmp_path):
    git(tmp_path, "init", "-q")
    git(tmp_path, "config", "user.email", "t@example.com")
    git(tmp_path, "config", "user.name", "t")
    (tmp_path / "a.txt").write_text("1")
    git(tmp_path, "add", ".")
    git(tmp_path, "commit", "-qm", "1")
    before = git(tmp_path, "rev-parse", "HEAD")
    (tmp_path / "ui" / "pets").mkdir(parents=True)
    (tmp_path / "ui" / "pets" / "x.png").write_bytes(b"x")
    (tmp_path / "a.txt").write_text("2")
    git(tmp_path, "add", ".")
    git(tmp_path, "commit", "-qm", "2")
    sha = git(tmp_path, "rev-parse", "HEAD")
    assert sorted(cac.changed_files(before, sha, tmp_path)) == ["a.txt", "ui/pets/x.png"]
    # 기록에 없는 커밋이나 0만 있는 before는 모름 → None
    assert cac.changed_files("c" * 40, sha, tmp_path) is None
    assert cac.changed_files("0" * 40, sha, tmp_path) is None
    assert cac.changed_files("", sha, tmp_path) is None


def test_main_prints_one_word(capsys):
    assert cac.main(["--event", "pull_request", "--ref", "refs/pull/1/merge"]) == 0
    out, err = capsys.readouterr()
    assert out == "run\n" and err.strip()
