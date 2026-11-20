"""⏱️ CI: 이번 push에서 그림 관련 파일이 바뀌었는지 보고 그림 테스트(test_draw_*.py)를 돌릴지 정함.

    python tools/ci_art_changed.py --event push --ref refs/heads/main --before <sha> --sha <sha>
    → stdout에 run 또는 skip 한 줄, 까닭은 stderr

그림 테스트는 생성기로 시트를 다시 그려 번들과 견주는 무거운 것들이라, 코드만 바뀐 push에서는 건너뜀.
조금이라도 모르면 (PR·수동 실행·태그·새 브랜치·강제 push로 before가 없음·git 실패) 다 돌림."""

import argparse
import fnmatch
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# 그림 테스트가 만들거나 읽는 파일 (저장소 기준 경로, fnmatch: *는 /도 넘음).
# test_ci_art_changed.py가 test_draw_*.py·draw_*.py에 적힌 경로가 다 여기 걸리는지 지킴
ART_PATTERNS = (
    # 생성기와 그 도우미, 테스트 자체, 그림 도구 패키지
    "tools/draw_*.py",
    "tools/sprite_spec.py",
    "tools/conftest.py",
    "tools/test_draw_*.py",
    "tools/requirements*.txt",
    "tools/pack_webp.py",
    "tools/preview/*",
    # 번들에 든 그림 (생성기 출력과 견줌)
    "ui/pets/*",
    "ui/effects/*",
    "ui/objects/*",
    "ui/house*/*",
    "ui/sleigh/*",
    "ui/vehicles/*",
    # 그림 테스트가 같이 읽는 코드 (그림과 맞물린 상수·이름)
    "ui/index.html",
    "ui/main.js",
    "ui/style.css",
    "ui/object.js",
    "src-tauri/src/network.rs",
    "src-tauri/src/objects.rs",
    "src-tauri/src/prop.rs",
    "src-tauri/src/weather.rs",
    "crates/screenpet-core/src/food.rs",
    "crates/screenpet-core/src/object.rs",
    "crates/screenpet-core/src/tumble.rs",
    # 이 결정 자체가 바뀌면 한 번은 다 돌려 봄
    "tools/ci_art_changed.py",
    "tools/test_ci_art_changed.py",
    ".github/workflows/build.yml",
)

ZERO_SHA = "0" * 40


def is_art(path: str) -> bool:
    """이 파일이 바뀌면 그림 테스트를 돌려야 하는지."""
    return any(fnmatch.fnmatchcase(path, p) for p in ART_PATTERNS)


def decide(event: str, ref: str, before: str, changed: list[str] | None) -> tuple[str, str]:
    """("run"|"skip", 까닭). changed가 None이면 바뀐 파일을 모르는 것 → 다 돌림."""
    if ref.startswith("refs/tags/"):
        return "run", "태그 실행이라 다 돌림"
    if event != "push":
        return "run", f"{event or '알 수 없는'} 실행이라 다 돌림"
    if not before or set(before) == {"0"}:
        return "run", "이전 커밋이 없어서 (새 브랜치) 다 돌림"
    if changed is None:
        return "run", f"{before[:12]}부터 바뀐 파일을 알 수 없어서 다 돌림"
    art = [p for p in changed if is_art(p)]
    if art:
        shown = ", ".join(art[:5]) + (f" 외 {len(art) - 5}개" if len(art) > 5 else "")
        return "run", f"그림 관련 파일이 바뀌어서 다 돌림: {shown}"
    return "skip", f"{before[:12]} 이후 바뀐 {len(changed)}개 파일 중 그림 관련이 없어서 그림 테스트(test_draw_*)를 건너뜀"


def changed_files(before: str, sha: str, cwd: Path = ROOT) -> list[str] | None:
    """before..sha 사이에 바뀐 파일. before가 이 저장소 기록에 없거나 git이 실패하면 None."""

    def git(*args):
        return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)

    if not before or not sha or set(before) == {"0"}:
        return None
    for rev in (before, sha):
        if git("cat-file", "-e", f"{rev}^{{commit}}").returncode != 0:
            return None
    diff = git("diff", "--name-only", "--no-renames", "-z", before, sha)
    if diff.returncode != 0:
        return None
    return [p for p in diff.stdout.split("\0") if p]


def main(argv=None) -> int:
    """결정을 stdout에, 까닭을 stderr에 씀."""
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--event", default="")
    ap.add_argument("--ref", default="")
    ap.add_argument("--before", default="")
    ap.add_argument("--sha", default="")
    a = ap.parse_args(argv)
    needs_diff = a.event == "push" and not a.ref.startswith("refs/tags/")
    changed = changed_files(a.before, a.sha) if needs_diff else None
    mode, why = decide(a.event, a.ref, a.before, changed)
    print(mode)
    print(why, file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
