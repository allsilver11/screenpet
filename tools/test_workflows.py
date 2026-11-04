"""GitHub Actions 설정: 실제 실행은 푸시해야 보이니, 깨지기 쉬운 약속을 여기서 지킨다."""
import re
from pathlib import Path

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
