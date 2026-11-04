"""GitHub Actions 설정: 실제 실행은 푸시해야 보이니, 깨지기 쉬운 약속을 여기서 지킨다."""
import json
import os
import re
import shutil
import subprocess
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS = sorted((ROOT / ".github" / "workflows").glob("*.yml"))
BUILD = ROOT / ".github" / "workflows" / "build.yml"


def load(path):
    return yaml.safe_load(path.read_text(encoding="utf-8"))


def build():
    return load(BUILD)


def steps(job):
    return build()["jobs"][job]["steps"]


def all_steps():
    for path in WORKFLOWS:
        for job in load(path)["jobs"].values():
            yield from job["steps"]


def all_runs():
    return [line for s in all_steps() for line in s.get("run", "").splitlines()]


def test_the_token_is_read_only_unless_a_job_says_otherwise():
    assert build()["permissions"] == {"contents": "read"}


def test_every_job_has_a_timeout():
    for path in WORKFLOWS:
        for name, job in load(path)["jobs"].items():
            assert job.get("timeout-minutes"), (path.name, name)


def test_third_party_actions_are_pinned_to_a_commit_with_a_version_comment():
    for path in WORKFLOWS:
        for ref in re.findall(r"uses: (\S+.*)", path.read_text(encoding="utf-8")):
            action = ref.split()[0]
            if action.startswith("actions/"):
                assert re.fullmatch(r"actions/[\w-]+@v\d+", action), ref
            else:
                assert re.fullmatch(r"[\w-]+/[\w-]+@[0-9a-f]{40}\s+# \S+.*", ref), ref


def test_a_pinned_rust_toolchain_still_names_its_toolchain():
    """@stable라는 이름에서 판을 읽던 액션이라, 커밋으로 고정하면 직접 적어야 한다."""
    toolchains = [s for s in all_steps() if "rust-toolchain" in s.get("uses", "")]
    assert toolchains
    assert all(s["with"]["toolchain"] == "stable" for s in toolchains)


def test_checkouts_leave_no_credentials_behind():
    checkouts = [s for s in all_steps() if s.get("uses", "").startswith("actions/checkout")]
    assert checkouts
    assert all(s["with"]["persist-credentials"] is False for s in checkouts)


def test_every_build_respects_the_lock_file():
    cargo = [r for r in all_runs() if re.search(r"\bcargo (test|check|build)\b", r)]
    assert cargo
    assert all("--locked" in r for r in cargo), cargo
    tauri = [r for r in all_runs() if "tauri build" in r]
    assert tauri
    # npm이 첫 --를, tauri가 다음 --를 먹고 나머지가 cargo로 간다
    assert all(re.search(r"npm run tauri build -- .*-- --locked$", r) for r in tauri), tauri


def test_the_test_job_runs_every_net_test():
    """릴리즈 전에 확인하는 것이 test 결과라, 앱에 들어가는 net도 test에서 돈다."""
    runs = [s.get("run", "") for s in steps("test")]
    assert "cargo test --locked -p screenpet-net" in runs
    assert not [r for r in all_runs() if "--test relay" in r]


def test_the_ui_harness_tests_cannot_be_skipped_for_lack_of_node():
    uses = [s.get("uses", "") for s in steps("test")]
    runs = [s.get("run", "") for s in steps("test")]
    node = next(i for i, u in enumerate(uses) if u.startswith("actions/setup-node"))
    pytest_step = next(i for i, r in enumerate(runs) if "python -m pytest" in r)
    assert node < pytest_step


def test_tool_packages_install_only_by_hash():
    req = (ROOT / "tools" / "requirements.txt").read_text(encoding="utf-8")
    specs = re.findall(r"^(\S+)( \\\n(?:    --hash=sha256:[0-9a-f]{64}(?: \\\n)?)+)?", req, re.M)
    pins = [(name, hashes) for name, hashes in specs if not name.startswith("#")]
    assert {"pillow==12.3.0", "numpy==2.4.6", "pytest==9.0.3"} <= {n for n, _ in pins}
    assert all("==" in name and hashes for name, hashes in pins), pins
    runs = " ".join(s.get("run", "") for s in steps("test"))
    assert "pip install --require-hashes --only-binary :all: -r tools/requirements.txt" in runs


def test_the_test_check_keeps_the_name_the_release_looks_for():
    assert "name" not in build()["jobs"]["test"]
    assert "strategy" not in build()["jobs"]["test"]


def test_a_branch_push_with_an_open_pr_runs_once():
    """같은 저장소의 PR은 push가 이미 돌린다. 포크 PR과 수동 실행은 그대로."""
    wf = build()
    on = wf[True]  # YAML에서 on은 true로 읽힌다
    assert on["push"]["branches"] == ["**"] and on["push"]["tags"] == ["v*"]
    assert "pull_request" in on and "workflow_dispatch" in on
    for job in ("test", "relay"):
        assert "github.event.pull_request.head.repo.full_name != github.repository" in wf["jobs"][job]["if"]
    for name, job in wf["jobs"].items():
        if "tauri build" in " ".join(s.get("run", "") for s in job["steps"]):
            # test를 건너뛴 것이 괜찮은 건 태그뿐: 같은 저장소의 PR이면 빌드도 건너뛴다
            assert "needs.test.result == 'success'" in job["if"], name
            assert "startsWith(github.ref, 'refs/tags/v') && needs.test.result == 'skipped'" in job["if"], name


# ---- 릴리즈 --------------------------------------------------------------------

def release():
    return build()["jobs"]["release"]


def script(job, name):
    return next(s for s in job["steps"] if s.get("name") == name)


def fake_gh(tmp_path, json_reply):
    """--jq는 진짜 jq로 돌리고 받은 인자를 적어 두는 가짜 gh."""
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    (tmp_path / "reply.json").write_text(json_reply, encoding="utf-8")
    gh = bin_dir / "gh"
    gh.write_text(
        '#!/bin/bash\n'
        'echo "$*" >> "$GH_LOG"\n'
        'if [ -n "${GH_FAIL_ON:-}" ] && [[ "$*" == *"$GH_FAIL_ON"* ]]; then echo boom >&2; exit 1; fi\n'
        'while [ $# -gt 0 ]; do if [ "$1" = --jq ]; then shift; jq -r "$1" "$GH_REPLY"; fi; shift; done\n',
        encoding="utf-8",
    )
    gh.chmod(0o755)
    return bin_dir


def run_script(tmp_path, step, reply, cwd=None, **env):
    bin_dir = fake_gh(tmp_path, reply)
    full = {
        "PATH": f"{bin_dir}:{os.environ['PATH']}",
        "GH_LOG": str(tmp_path / "gh.log"),
        "GH_REPLY": str(tmp_path / "reply.json"),
        "GITHUB_REPOSITORY": "me/pets",
        "GITHUB_SHA": "abc123",
        **{k: str(v) for k, v in step.get("env", {}).items() if "${{" not in str(v)},
        **{k: str(v) for k, v in env.items()},
    }
    result = subprocess.run(["bash", "-c", step["run"]], env=full, cwd=cwd or tmp_path, capture_output=True, text=True, timeout=30)
    log = tmp_path / "gh.log"
    return result, log.read_text(encoding="utf-8").splitlines() if log.exists() else []


def check_runs(*runs):
    return json.dumps({"check_runs": [{"app": {"slug": slug}, "status": status, "conclusion": conclusion} for slug, status, conclusion in runs]})


needs_jq = pytest.mark.skipif(not shutil.which("jq"), reason="jq 없음")
VERIFY = "Verify the tagged commit passed the test job"


def test_only_the_release_job_can_write():
    jobs = build()["jobs"]
    assert release()["permissions"] == {"contents": "write", "checks": "read"}
    for name, job in jobs.items():
        if name != "release":
            assert "permissions" not in job, name


def test_builds_only_upload_artifacts_and_fail_when_they_made_nothing():
    upload = [s for s in steps("build") if s.get("uses", "").startswith("actions/upload-artifact")]
    assert len(upload) == 1 and upload[0]["with"]["if-no-files-found"] == "error"
    assert not [s for s in steps("build") if "action-gh-release" in s.get("uses", "")]


def test_builds_cover_both_platforms_and_one_failure_does_not_cancel_the_other():
    strategy = build()["jobs"]["build"]["strategy"]
    assert strategy["fail-fast"] is False
    assert {(r["os"], r["artifact"]) for r in strategy["matrix"]["include"]} == {
        ("windows-latest", "screenpet-windows"), ("macos-latest", "screenpet-macos"),
    }


def test_release_needs_a_fully_successful_build_on_a_tag():
    """test가 건너뛰어지면 그 뒤 작업도 조용히 건너뛰니, 결과를 직접 물어야 태그에서 돈다."""
    job = release()
    assert job["needs"] == "build"
    assert "!cancelled()" in job["if"]
    assert "needs.build.result == 'success'" in job["if"]
    assert "startsWith(github.ref, 'refs/tags/v')" in job["if"]


def test_release_runs_no_repo_code():
    """쓰기 토큰을 든 작업: 저장소 파일은 읽기만 한다."""
    for step in release()["steps"]:
        assert not re.search(r"\b(npm|npx|cargo|python|pip|make|node)\b|\./", step.get("run", "")), step
        if "uses" in step:
            assert step["uses"].split("@")[0] in {"actions/checkout", "actions/download-artifact", "softprops/action-gh-release"}, step


def test_release_checks_come_before_anything_is_published():
    order = [s.get("name") or s.get("uses", "") for s in release()["steps"]]
    publish = next(i for i, s in enumerate(release()["steps"]) if "action-gh-release" in s.get("uses", ""))
    assert order.index(VERIFY) < publish
    assert order.index("Require the release notes") < publish
    assert publish == len(order) - 1


def test_release_publishes_notes_and_every_installer_or_fails():
    step = next(s for s in release()["steps"] if "action-gh-release" in s.get("uses", ""))
    assert step["with"]["body_path"] == "docs/releases/${{ github.ref_name }}.md"
    assert step["with"]["fail_on_unmatched_files"] is True
    files = step["with"]["files"]
    for ext in ("msi", "exe", "dmg"):
        assert f"dist/**/*.{ext}" in files


def test_release_downloads_what_the_builds_upload():
    names = {s["with"]["name"] for s in release()["steps"] if s.get("uses", "").startswith("actions/download-artifact")}
    assert names == {r["artifact"] for r in build()["jobs"]["build"]["strategy"]["matrix"]["include"]}


@pytest.mark.parametrize("present", [True, False])
def test_release_notes_guard(tmp_path, present):
    if present:
        (tmp_path / "docs" / "releases").mkdir(parents=True)
        (tmp_path / "docs" / "releases" / "v1.2.3.md").write_text("안녕\n", encoding="utf-8")
    step = script(release(), "Require the release notes")
    result, _ = run_script(tmp_path, step, "{}", TAG="v1.2.3")
    assert (result.returncode == 0) == present, result.stderr
    if not present:
        assert "docs/releases/v1.2.3.md" in result.stdout


@needs_jq
def test_verify_accepts_a_passed_test_even_beside_the_tag_runs_own_skipped_one(tmp_path):
    """태그 실행의 test는 skipped로 같은 커밋에 남고, 가장 최근 것이기도 하다."""
    reply = check_runs(("github-actions", "completed", "success"), ("github-actions", "completed", "skipped"))
    result, log = run_script(tmp_path, script(release(), VERIFY), reply)
    assert result.returncode == 0, result.stdout + result.stderr
    assert "repos/me/pets/commits/abc123/check-runs" in log[0]
    assert "check_name=test" in log[0] and "filter=all" in log[0]


@needs_jq
@pytest.mark.parametrize("runs", [
    [],
    [("github-actions", "completed", "skipped")],
    [("github-actions", "completed", "failure")],
    [("someone-else", "completed", "success")],
])
def test_verify_refuses_a_commit_without_a_passed_test(tmp_path, runs):
    result, _ = run_script(tmp_path, script(release(), VERIFY), check_runs(*runs))
    assert result.returncode == 1
    assert "::error::" in result.stdout and "abc123" in result.stdout


@needs_jq
def test_verify_waits_for_a_running_test_but_not_forever(tmp_path):
    """main과 태그를 같이 푸시하면 빌드가 30분 걸리는 test보다 먼저 끝난다."""
    reply = check_runs(("github-actions", "in_progress", None))
    result, log = run_script(tmp_path, script(release(), VERIFY), reply, WAIT_MINUTES=0, POLL_SECONDS=0)
    assert result.returncode == 1
    assert "::error::" in result.stdout
    # 기다림의 끝이 작업 시간 제한보다 먼저 와야 위 메시지가 보인다
    step = script(release(), VERIFY)
    assert step["env"]["WAIT_MINUTES"] >= build()["jobs"]["test"]["timeout-minutes"]
    assert step["env"]["WAIT_MINUTES"] + 5 <= release()["timeout-minutes"]


@needs_jq
def test_verify_fails_when_github_cannot_answer(tmp_path):
    result, _ = run_script(tmp_path, script(release(), VERIFY), "{}", GH_FAIL_ON="check-runs")
    assert result.returncode != 0
