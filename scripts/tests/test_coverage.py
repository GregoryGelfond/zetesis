"""Coverage workflow contracts; no compiler, LLVM instrumentation or solver runs."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest


ROOT = Path(__file__).resolve().parents[2]
TIMEOUT = 20

FAKE_CARGO = '''from pathlib import Path
import json
import os
import sys

args = sys.argv[1:]
if args == ["+1.97.1", "llvm-cov", "--version"]:
    print(os.environ.get("COVERAGE_TEST_CARGO_VERSION", "cargo-llvm-cov 0.8.7"))
    sys.exit(0)
if args[:2] != ["+1.97.1", "llvm-cov"]:
    sys.exit("unexpected mock Cargo invocation")
if args[2:3] == ["report"]:
    # The verified subset of the real 0.8.7 parser, not its broader help text.
    flags = {"--locked", "--json", "--html"}
    values = {"--package", "--output-path", "--output-dir", "--fail-under-lines"}
    index = 3
    while index < len(args):
        if args[index] in flags:
            index += 1
        elif args[index] in values and index + 1 < len(args):
            index += 2
        else:
            sys.exit("invalid mock report option: " + args[index])
profile = Path(os.environ["CARGO_LLVM_COV_TARGET_DIR"]).name
with open(os.environ["COVERAGE_TEST_LOG"], "a") as log:
    log.write(json.dumps({"args": args, "profile": profile}) + "\\n")
phase = ("gate" if "--fail-under-lines" in args else
         "html" if "--html" in args else
         "json" if "--json" in args else
         "clean" if "clean" in args else "test")
if os.environ.get("COVERAGE_TEST_FAIL") in [phase, profile + ":" + phase]:
    print("simulated failure " + phase, file=sys.stderr)
    sys.exit(37)
if "--output-path" in args:
    Path(args[args.index("--output-path") + 1]).write_text("{}\\n")
if "--output-dir" in args:
    output = Path(args[args.index("--output-dir") + 1]) / "html"
    output.mkdir(exist_ok=True)
    (output / "index.html").write_text("mock HTML")
'''

FAKE_RUSTC = '''import os
import sys

if sys.argv[1:] == ["+1.97.1", "--print", "sysroot"]:
    print(os.environ["COVERAGE_TEST_SYSROOT"])
elif sys.argv[1:] == ["+1.97.1", "-vV"]:
    print("rustc 1.97.1\\nhost: fake-host\\nLLVM version: 22.1.6")
else:
    sys.exit("unexpected mock rustc invocation")
'''

FAKE_LLVM = '''#!/bin/sh
printf 'LLVM (http://llvm.org/):\\n  LLVM version %s\\n  Optimized build.\\n' \\
    "${COVERAGE_TEST_LLVM_VERSION:-22.1.6}"
'''


def clean_environment():
    """Do not inherit tool overrides, Git hooks/configuration or test controls."""
    excluded = {"LLVM_COV", "LLVM_PROFDATA", "CARGO_LLVM_COV_TARGET_DIR"}
    return {key: value for key, value in os.environ.items()
            if key not in excluded and not key.startswith(("GIT_", "COVERAGE_TEST_"))}


def ratchet_script():
    """Extract the actual literal run block; fail if its YAML structure changes."""
    lines = (ROOT / ".github/workflows/checks.yml").read_text().splitlines()
    matches = [index for index, line in enumerate(lines)
               if line.strip() == "- name: Preserve the committed floor"]
    if len(matches) != 1:
        raise AssertionError("Expected exactly one committed coverage-floor step")
    index = matches[0] + 1
    if lines[index].strip() != "run: |":
        raise AssertionError("Coverage-floor step must have a literal run block")
    indentation = len(lines[index]) - len(lines[index].lstrip())
    block = []
    for line in lines[index + 1:]:
        if line.strip() and len(line) - len(line.lstrip()) <= indentation:
            break
        block.append(line)
    result = textwrap.dedent("\n".join(block)) + "\n"
    if not result.strip():
        raise AssertionError("Coverage-floor run block is empty")
    return result


class CoverageScriptTests(unittest.TestCase):
    def run_case(self, *, mode="baseline", floor="UNMEASURED", overrides="neither",
                 changes=None, error=None, locked=False):
        with tempfile.TemporaryDirectory(prefix="zetesis-coverage-") as temporary:
            repo = Path(temporary).resolve()
            (repo / "scripts").mkdir()
            shutil.copyfile(ROOT / "scripts/coverage.sh", repo / "scripts/coverage.sh")
            floor_path = repo / "scripts/coverage-floor.txt"
            floor_path.write_text(floor + "\n")
            binaries = repo / "bin"
            binaries.mkdir()
            tools = repo / "sysroot/lib/rustlib/fake-host/bin"
            tools.mkdir(parents=True)
            for path, contents in [
                (binaries / "cargo", f"#!{sys.executable}\n" + FAKE_CARGO),
                (binaries / "rustc", f"#!{sys.executable}\n" + FAKE_RUSTC),
                (tools / "llvm-cov", FAKE_LLVM),
                (tools / "llvm-profdata", FAKE_LLVM),
            ]:
                path.write_text(contents)
                path.chmod(0o700)
            reports = repo / "target/coverage"
            reports.mkdir(parents=True)
            # Every refusal must invalidate this old marker except lock contention,
            # which must leave the other owner's reports and lock untouched.
            (reports / "status.txt").write_text("gate-passed\n")
            if locked:
                (reports / ".lock").mkdir()
            log = repo / "calls.jsonl"
            env = clean_environment()
            env.update(PATH=str(binaries) + os.pathsep + env["PATH"],
                       COVERAGE_TEST_SYSROOT=str(repo / "sysroot"),
                       COVERAGE_TEST_LOG=str(log))
            if overrides in ("cov", "both"):
                env["LLVM_COV"] = str(tools / "llvm-cov")
            if overrides in ("profdata", "both"):
                env["LLVM_PROFDATA"] = str(tools / "llvm-profdata")
            env.update(changes or {})
            completed = subprocess.run(["sh", "scripts/coverage.sh", mode], cwd=repo,
                                       env=env, text=True, capture_output=True,
                                       timeout=TIMEOUT, check=False)
            if error is None:
                self.assertEqual(completed.returncode, 0, completed.stderr)
            else:
                self.assertNotEqual(completed.returncode, 0)
                self.assertIn(error, completed.stderr)
            expected_status = ("gate-passed" if locked else "incomplete" if error else
                               "baseline-complete (nongating)" if mode == "baseline" else
                               "gate-passed")
            self.assertEqual((reports / "status.txt").read_text().strip(), expected_status)
            self.assertEqual((reports / ".lock").exists(), locked)
            self.assertEqual(floor_path.read_text(), floor + "\n")
            calls = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
            if error is None:
                self.assert_profile_contract(repo, reports, calls, mode, floor)
            elif not changes or "COVERAGE_TEST_FAIL" not in changes:
                self.assertEqual(calls, [], "preflight failure must not start instrumentation")

    def assert_profile_contract(self, repo, reports, calls, mode, floor):
        expected = []
        for profile, features, workspace in [
            ("workspace", ["--all-features"], ["--workspace"]),
            ("cli-cpu", ["--package", "zetesis-cli", "--no-default-features"], []),
        ]:
            directory = reports / profile
            selection = [] if profile == "workspace" else ["--package", "zetesis-cli"]
            commands = [
                ["clean", "--workspace", "--locked"],
                [*workspace, *features, "--locked", "--no-report"],
                ["report", *selection, "--locked", "--json", "--output-path",
                 str(directory / "coverage.json")],
                ["report", *selection, "--locked", "--html", "--output-dir", str(directory)],
            ]
            expected.extend({"args": ["+1.97.1", "llvm-cov", *command],
                             "profile": "build-" + profile} for command in commands)
            for path in ["coverage.json", "html/index.html"]:
                self.assertTrue((directory / path).is_file())
        if mode == "gate":
            expected.append({"args": ["+1.97.1", "llvm-cov", "report", "--locked",
                                      "--fail-under-lines", floor],
                             "profile": "build-workspace"})
            expected.append({"args": ["+1.97.1", "llvm-cov", "report", "--package",
                                      "zetesis-cli", "--locked", "--fail-under-lines", floor],
                             "profile": "build-cli-cpu"})
        # This catches shared profile paths, missing fresh cleanup, project-added
        # exclusions, ignored test failures, and gates applied to the wrong profile.
        self.assertEqual(calls, expected)
        metadata = json.loads((reports / "toolchain.json").read_text())
        self.assertFalse(metadata["profiles_merged"])
        self.assertEqual(metadata["floor_profiles"], ["workspace", "cli-cpu"])
        self.assertEqual(metadata["mode"], mode)
        self.assertEqual(metadata["committed_floor"], floor)
        self.assertEqual(metadata["cargo_llvm_cov"], "0.8.7")
        self.assertEqual(metadata["default_filename_filters"],
                         "cargo-llvm-cov 0.8.7 src/report.rs::ignore_filename_regex")
        self.assertEqual(metadata["project_added_filename_filters"], [])
        for tool in metadata["llvm_tools"].values():
            self.assertTrue(Path(tool["path"]).is_relative_to(repo))
            self.assertRegex(tool["sha256"], r"^[0-9a-f]{64}$")
            self.assertTrue(tool["version"].startswith("LLVM (http://llvm.org/):\n  LLVM version "))
            self.assertTrue(tool["version"].endswith("\n  Optimized build."))

    def test_baseline_profiles_are_fresh_separate_and_nongating(self):
        for overrides in ("neither", "both"):
            with self.subTest(overrides=overrides):
                self.run_case(overrides=overrides)

    def test_measured_floor_gates_both_profiles_independently(self):
        self.run_case(mode="gate", floor="75", overrides="both")

    def test_report_mock_rejects_build_flags_advertised_by_tool_help(self):
        with tempfile.TemporaryDirectory(prefix="zetesis-report-parser-") as temporary:
            script = Path(temporary) / "cargo.py"
            script.write_text(FAKE_CARGO)
            for arguments in (["--all-features"], ["--no-default-features"],
                              ["--features", "gpu"], ["--workspace"]):
                with self.subTest(arguments=arguments):
                    completed = subprocess.run(
                        [sys.executable, str(script), "+1.97.1", "llvm-cov", "report", *arguments],
                        env=clean_environment(), text=True, capture_output=True,
                        timeout=TIMEOUT, check=False)
                    self.assertNotEqual(completed.returncode, 0)
                    self.assertIn("invalid mock report option", completed.stderr)

    def test_matching_rust_bundled_llvm_build_format_is_accepted(self):
        self.run_case(overrides="both", changes={
            "COVERAGE_TEST_LLVM_VERSION": "22.1.6-rust-1.97.1-stable",
        })

    def test_invalid_floor_cannot_reuse_a_previous_success(self):
        for mode, floor, error in [
            ("gate", "UNMEASURED", "Coverage floor is unmeasured"),
            ("baseline", "NaN", "Coverage floor must be"),
        ]:
            with self.subTest(mode=mode, floor=floor):
                self.run_case(mode=mode, floor=floor, error=error)

    def test_llvm_overrides_must_be_a_complete_pair(self):
        for overrides in ("cov", "profdata"):
            with self.subTest(overrides=overrides):
                self.run_case(overrides=overrides, error="Set both LLVM_COV and LLVM_PROFDATA")

    def test_wrong_tool_versions_cannot_reuse_a_previous_success(self):
        for changes, error in [
            ({"COVERAGE_TEST_CARGO_VERSION": "cargo-llvm-cov 0.8.6"}, "Expected cargo-llvm-cov 0.8.7"),
            ({"COVERAGE_TEST_LLVM_VERSION": "22.1.5"}, "does not match rustc LLVM 22.1.6"),
        ]:
            with self.subTest(changes=changes):
                self.run_case(changes=changes, error=error)

    def test_wrong_llvm_build_or_unrecognized_suffix_is_refused(self):
        for version in (
            "22.1.5-rust-1.97.1-stable",
            "22.1.6-rust-1.96.0-stable",
            "22.1.6-rust-1.97.1-nightly",
            "22.1.6-custom",
            "22.1.6-rust-1.97.1-stable-extra",
            "22.1.6 trailing text",
        ):
            with self.subTest(version=version):
                self.run_case(changes={"COVERAGE_TEST_LLVM_VERSION": version},
                              error="does not match rustc LLVM 22.1.6")

    def test_concurrent_run_preserves_the_existing_owner(self):
        self.run_case(locked=True, error="Another coverage run owns")

    def test_every_command_failure_keeps_the_run_incomplete(self):
        for failure in ("clean", "test", "json", "html", "build-cli-cpu:test", "gate",
                        "build-cli-cpu:gate"):
            with self.subTest(failure=failure):
                self.run_case(mode="gate", floor="75", changes={"COVERAGE_TEST_FAIL": failure},
                              error="simulated failure " + failure.split(":")[-1])


class FloorRatchetTests(unittest.TestCase):
    def test_actual_workflow_retains_or_raises_the_committed_floor(self):
        script = ratchet_script()
        for label, before, after, success in [
            ("initial adoption", None, "75", True),
            ("measured adoption", "UNMEASURED", "75", True),
            ("same floor", "75", "75", True),
            ("raised floor", "75", "80", True),
            ("lower floor", "75", "70", False),
            ("erase measurement", "75", "UNMEASURED", False),
            ("invalid current", "75", "garbage", False),
        ]:
            with self.subTest(case=label), tempfile.TemporaryDirectory(prefix="zetesis-floor-") as temporary:
                repo = Path(temporary).resolve()
                (repo / "scripts").mkdir()
                hooks = repo / "empty-hooks"
                hooks.mkdir()
                template = repo / "empty-template"
                template.mkdir()
                env = clean_environment()
                env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)

                def git(*arguments):
                    return subprocess.run(
                        ["git", "-c", f"core.hooksPath={hooks}", "-c", "commit.gpgsign=false",
                         "-c", "user.name=Coverage Test", "-c", "user.email=coverage@example.invalid",
                         *arguments], cwd=repo, env=env, text=True, capture_output=True,
                        timeout=TIMEOUT, check=True)

                git("init", "-q", f"--template={template}")
                (repo / "initial").write_text("local test fixture\n")
                floor_path = repo / "scripts/coverage-floor.txt"
                if before is not None:
                    floor_path.write_text(before + "\n")
                git("add", "initial", "scripts")
                git("commit", "-qm", "local coverage-floor fixture")
                commit = git("rev-parse", "HEAD").stdout.strip()
                floor_path.write_text(after + "\n")
                event_path = repo / "event.json"
                for kind, event in [
                    ("push", {"before": commit}),
                    ("pull request", {"pull_request": {"base": {"sha": commit}}}),
                ]:
                    with self.subTest(event=kind):
                        event_path.write_text(json.dumps(event))
                        completed = subprocess.run(
                            ["sh", "-c", script], cwd=repo,
                            env=dict(env, GITHUB_EVENT_PATH=str(event_path)), text=True,
                            capture_output=True, timeout=TIMEOUT, check=False)
                        self.assertEqual(completed.returncode == 0, success, completed.stderr)
                        self.assertEqual(floor_path.read_text(), after + "\n")


if __name__ == "__main__":
    unittest.main()
