"""배포용 WebP 변환: 그림이 픽셀까지 같고, 앱이 webp를 읽게 바뀌고, 저장소의 앱은 PNG를 읽는다."""
import re
import shutil
from pathlib import Path

import numpy as np
import pytest
import yaml
from PIL import Image

import pack_webp

ROOT = Path(__file__).resolve().parent.parent


@pytest.fixture
def ui(tmp_path):
    (tmp_path / "pets" / "ghost").mkdir(parents=True)
    src = ROOT / "ui" / "pets" / "ghost"
    shutil.copy(src / "sheet_baby.png", tmp_path / "pets" / "ghost")
    shutil.copy(src / "pet.json", tmp_path / "pets" / "ghost")
    shutil.copy(ROOT / "ui" / "common.js", tmp_path)
    return tmp_path


def test_pack_is_lossless_and_switches_the_app_to_webp(ui):
    png = ui / "pets" / "ghost" / "sheet_baby.png"
    want = np.asarray(Image.open(png).convert("RGBA"))
    count, before, after = pack_webp.pack(ui)
    assert count == 1 and after < before
    assert not png.exists()
    assert (ui / "pets" / "ghost" / "pet.json").exists()
    assert np.array_equal(np.asarray(Image.open(png.with_suffix(".webp")).convert("RGBA")), want)
    assert 'const PET_ART_EXT = "webp";' in (ui / "common.js").read_text(encoding="utf-8")


def test_pack_refuses_a_tree_it_already_packed(ui):
    pack_webp.pack(ui)
    with pytest.raises(SystemExit):
        pack_webp.pack(ui)


def test_the_repo_app_reads_png_and_every_pet_sheet_url_uses_the_constant():
    common = (ROOT / "ui" / "common.js").read_text(encoding="utf-8")
    assert common.count(pack_webp.MARK) == 1
    main = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
    # 동글 시트·목은 상수로, 픽셀 그림은 PNG 그대로
    assert not re.findall(r"`pets/[^`]*\.png`", common + main)
    assert not re.findall(r"\$\{dir\}neck[^`]*\.png", main)
    assert 'dir.startsWith("pets_pixel") ? "png" : PET_ART_EXT' in main
    assert "`pets_pixel/${id}/sheet_${stageName}.png`" in common


def packed_platforms(ci):
    """설치 파일을 만드는 작업마다 그 빌드 앞에서 그림을 꼭 변환하는지 보고, 만드는 플랫폼을 돌려준다."""
    platforms = set()
    for job in ci["jobs"].values():
        runs = [step.get("run", "") for step in job["steps"]]
        builds = [i for i, run in enumerate(runs) if "tauri build" in run]
        if not builds:
            continue
        packs = [i for i, run in enumerate(runs) if "python tools/pack_webp.py" in run]
        assert len(builds) == 1 and len(packs) == 1
        assert packs[0] < builds[0], "installer built before packing"
        assert "if" not in job["steps"][packs[0]], "packing must not be conditional"
        matrix = job.get("strategy", {}).get("matrix", {}).get("include")
        platforms |= {row["os"] for row in matrix} if matrix else {job["runs-on"]}
    return platforms


def test_the_build_packs_before_every_installer():
    ci = yaml.safe_load((ROOT / ".github" / "workflows" / "build.yml").read_text(encoding="utf-8"))
    assert packed_platforms(ci) == {"windows-latest", "macos-latest"}


def test_the_packing_order_check_catches_a_late_pack():
    ci = yaml.safe_load((ROOT / ".github" / "workflows" / "build.yml").read_text(encoding="utf-8"))
    for job in ci["jobs"].values():
        job["steps"] = [s for s in job["steps"] if "pack_webp" not in s.get("run", "")] + [
            s for s in job["steps"] if "pack_webp" in s.get("run", "")
        ]
    with pytest.raises(AssertionError, match="before packing"):
        packed_platforms(ci)
