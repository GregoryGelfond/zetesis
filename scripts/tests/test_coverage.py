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
# An independent expectation for each Cargo target and its exact libtest names.
# In particular, three lazy CLI tests do not contain "metal" in their names.
METAL_GROUPS = [
    ("wgpu-lib", "lib", [
        "aggregate::device::tests::metal_aggregate_readback_failure_retains_submitted_work",
        "lazy::transport_tests::metal_lazy_transport_reuse_preserves_round_truth",
        "lazy::transport_tests::metal_input_slack_preserves_exact_admission",
        "lazy::transport_tests::metal_lazy_transport_refusal_preserves_reuse",
        "lazy::transport_tests::metal_lazy_transport_cancelled_read_discards_capacity",
    ]),
    ("tight", "hardware_tight", [
        "metal_support_matches_exact_reduct_semantics",
        "metal_support_preserves_batch_isolation",
        "metal_support_refusals_preserve_reusable_residency",
        "metal_support_residency_tracks_theory_identity",
    ]),
    ("formula", "hardware_formula", [
        "metal_formula_limits_resize_identity_and_word_boundaries_remain_explicit",
        "metal_formula_queries_preserve_exact_frozen_semantics_and_residency",
    ]),
    ("aggregate", "hardware_aggregate", [
        "metal_aggregate_reductions_match_native_occurrences",
        "metal_aggregate_guards_preserve_numeric_boundaries",
        "metal_aggregate_exact_admission_preserves_cache_lifecycle",
    ]),
    ("lazy", "hardware_lazy", [
        "metal_lazy_worlds_match_exact_frozen_cpu_closures",
        "metal_lazy_growth_preserves_previous_round_truth",
        "metal_lazy_catalog_fits_when_static_carrier_refuses",
        "metal_source_selections_preserve_each_frozen_closure",
    ]),
    ("cli-lazy", "lazy_gpu", [
        "physical::ordinary_lazy_metal_preserves_complete_cpu_models",
        "physical::requested_model_limit_retains_completed_lazy_candidates",
        "physical::lazy_source_stop_preserves_unfinished_candidate_counts",
        "physical::lazy_writer_failure_preserves_completed_device_work",
    ]),
    ("cli-formula", "formula_gpu", [
        "physical::ordinary_metal_formula_batches_match_complete_cpu_models_costs_and_displays",
        "physical::ordinary_metal_formula_limits_preserve_partial_coverage_and_writer_errors",
    ]),
    ("world-views", "world_views_gpu", [
        "metal_world_view_preserves_nonoptimal_answers",
        "metal_collection_limit_retains_checked_accounting",
    ]),
    ("aggregate-measurement", "aggregate_measurement", [
        "metal_aggregate_measurements_require_actual_submissions",
    ]),
]
METAL_TESTS = [test for _, _, tests in METAL_GROUPS for test in tests]

FAKE_CARGO = '''from pathlib import Path
import json
import os
import sys

physical_groups = PHYSICAL_GROUP_FIXTURE
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
if "--ignored" in args:
    target = "lib" if "--lib" in args else args[args.index("--test") + 1]
    groups = [entry for entry in physical_groups if entry[1] == target]
    if len(groups) != 1:
        sys.exit("unexpected physical test target")
    group, _, available = groups[0]
    physical = os.environ.get("COVERAGE_TEST_PHYSICAL", "passed")
    if os.environ.get("COVERAGE_TEST_PHYSICAL_GROUP", "wgpu-lib") != group:
        physical = "passed"
    if physical == "failed":
        print("simulated physical test failure", file=sys.stderr)
        sys.exit(37)
    filters = args[args.index("--exact") + 1:]
    selected = [name for name in available if name in filters]
    if physical == "missing":
        selected = selected[:-1]
    elif physical == "empty":
        selected = []
    elif physical == "wrong-name":
        selected[-1] = "unrelated_physical_test"
    elif physical == "duplicate-name":
        selected[-1] = selected[0]
    for name in selected:
        # --nocapture allows test output before the same test's final "ok".
        print(f"test {name} ... adapter=qualified-fixture")
        if physical != "missing-outcome" or name != selected[-1]:
            print("FAILED" if physical == "failed-outcome" and name == selected[-1] else "ok")
    if target == "lib":
        # --workspace --lib legitimately visits other libraries with no matches.
        print("test result: ok. 0 passed; 0 failed; 0 ignored; "
              "0 measured; 10 filtered out; finished in 0.00s")
    if physical != "unreported":
        passed = len(selected)
        ignored = 1 if physical == "ignored" else 0
        # The pinned libtest wire format, independently observed in retained
        # coverage output, includes the measured count even for ordinary tests.
        print(f"test result: ok. {passed} passed; 0 failed; {ignored} ignored; "
              "0 measured; 1 filtered out; finished in 0.01s")
    if physical == "extra-summary":
        print("test result: ok. 1 passed; 0 failed; 0 ignored; "
              "0 measured; 0 filtered out; finished in 0.01s")
    if physical == "failed-summary":
        print("test result: FAILED. 0 passed; 1 failed; 0 ignored; "
              "0 measured; 0 filtered out; finished in 0.01s")
if "--output-path" in args:
    Path(args[args.index("--output-path") + 1]).write_text("{}\\n")
if "--output-dir" in args:
    output = Path(args[args.index("--output-dir") + 1]) / "html"
    output.mkdir(exist_ok=True)
    (output / "index.html").write_text("mock HTML")
'''.replace("PHYSICAL_GROUP_FIXTURE", repr(METAL_GROUPS))

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
                 changes=None, error=None, locked=False, metal=False, exit_code=None,
                 omitted_group=None):
        with tempfile.TemporaryDirectory(prefix="zetesis-coverage-") as temporary:
            repo = Path(temporary).resolve()
            (repo / "scripts").mkdir()
            script = repo / "scripts/coverage.sh"
            shutil.copyfile(ROOT / "scripts/coverage.sh", script)
            if omitted_group:
                # Model an accidentally dropped target row without corrupting
                # shell syntax or substituting another workflow implementation.
                lines = script.read_text().splitlines(keepends=True)
                removed = [line for line in lines if line.startswith(omitted_group + "|")]
                self.assertEqual(len(removed), 1)
                script.write_text("".join(line for line in lines if line not in removed))
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
            if metal:
                directory = reports / "workspace"
                directory.mkdir()
                (directory / "metal-status.txt").write_text("passed\n")
                for group, _, _ in METAL_GROUPS:
                    (directory / f"metal-{group}-status.txt").write_text("passed\n")
                    (directory / f"metal-{group}.log").write_text("old physical log\n")
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
            arguments = ["sh", "scripts/coverage.sh", mode]
            if metal:
                arguments.append("--metal")
            completed = subprocess.run(arguments, cwd=repo,
                                       env=env, text=True, capture_output=True,
                                       timeout=TIMEOUT, check=False)
            if error is None:
                self.assertEqual(completed.returncode, 0, completed.stderr)
            else:
                self.assertNotEqual(completed.returncode, 0)
                self.assertIn(error, completed.stderr)
            if exit_code is not None:
                self.assertEqual(completed.returncode, exit_code)
            expected_status = ("gate-passed" if locked else "incomplete" if error else
                               "baseline-complete (nongating)" if mode == "baseline" else
                               "gate-passed")
            self.assertEqual((reports / "status.txt").read_text().strip(), expected_status)
            self.assertEqual((reports / ".lock").exists(), locked)
            self.assertEqual(floor_path.read_text(), floor + "\n")
            calls = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
            if error is None:
                self.assert_profile_contract(repo, reports, calls, mode, floor, metal)
            elif metal and changes and "COVERAGE_TEST_PHYSICAL" in changes:
                self.assert_physical_failure(reports, calls,
                                             changes.get("COVERAGE_TEST_PHYSICAL_GROUP", "wgpu-lib"))
            elif not changes or "COVERAGE_TEST_FAIL" not in changes:
                self.assertEqual(calls, [], "preflight failure must not start instrumentation")

    def assert_profile_contract(self, repo, reports, calls, mode, floor, metal):
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
            ]
            if profile == "workspace" and metal:
                commands.extend([
                    ["report", "--locked", "--json", "--output-path",
                     str(directory / "portable/coverage.json")],
                    ["report", "--locked", "--html", "--output-dir", str(directory / "portable")],
                ])
                for group, target, tests in METAL_GROUPS:
                    commands.append(self.physical_command(target, tests))
                    self.assertEqual((directory / f"metal-{group}-status.txt").read_text(), "passed\n")
                    self.assertIn(f"{len(tests)} passed;",
                                  (directory / f"metal-{group}.log").read_text())
                self.assertEqual((directory / "metal-status.txt").read_text(), "passed\n")
                for path in ["coverage.json", "html/index.html"]:
                    self.assertTrue((directory / "portable" / path).is_file())
            commands.extend([
                ["report", *selection, "--locked", "--json", "--output-path",
                 str(directory / "coverage.json")],
                ["report", *selection, "--locked", "--html", "--output-dir", str(directory)],
            ])
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
        self.assertEqual(metadata["profile_merge_scope"],
                         "profiles_merged describes floor profiles; raw execution profiles combine "
                         "only within their own floor profile")
        self.assertEqual(metadata["workspace_execution"], "portable+metal" if metal else "portable")
        self.assertEqual(metadata["workspace_stages"], ["portable", "metal"] if metal else ["portable"])
        self.assertEqual(metadata["physical_tests"], METAL_TESTS if metal else [])
        self.assertEqual(metadata["expected_physical_tests"], len(METAL_TESTS) if metal else 0)
        expected_groups = [{
            "group": group, "target_kind": "lib" if target == "lib" else "test",
            "target": "workspace libraries" if target == "lib" else target,
            "tests": tests, "expected_tests": len(tests),
        } for group, target, tests in METAL_GROUPS] if metal else []
        self.assertEqual(metadata["physical_test_groups"], expected_groups)
        if metal:
            self.assertEqual(metadata["expected_physical_tests"], 27)
            self.assertIn("Unlisted tests and Vulkan are not selected.", metadata["physical_scope"])
        else:
            self.assertIsNone(metadata["physical_scope"])
        for tool in metadata["llvm_tools"].values():
            self.assertTrue(Path(tool["path"]).is_relative_to(repo))
            self.assertRegex(tool["sha256"], r"^[0-9a-f]{64}$")
            self.assertTrue(tool["version"].startswith("LLVM (http://llvm.org/):\n  LLVM version "))
            self.assertTrue(tool["version"].endswith("\n  Optimized build."))

    @staticmethod
    def physical_command(target, tests):
        """Require the same workspace features, retained data and exact group."""
        selection = ["--lib"] if target == "lib" else ["--test", target]
        return ["--workspace", "--all-features", *selection,
                "--locked", "--no-report", "--no-clean", "--", "--ignored",
                "--nocapture", "--test-threads=1", "--exact", *tests]

    def assert_physical_failure(self, reports, calls, failed_group):
        """Only a retained portable report may precede physical-stage refusal."""
        directory = reports / "workspace"
        expected = [
            ["clean", "--workspace", "--locked"],
            ["--workspace", "--all-features", "--locked", "--no-report"],
            ["report", "--locked", "--json", "--output-path", str(directory / "portable/coverage.json")],
            ["report", "--locked", "--html", "--output-dir", str(directory / "portable")],
        ]
        failed_index = [group for group, _, _ in METAL_GROUPS].index(failed_group)
        for _, target, tests in METAL_GROUPS[:failed_index + 1]:
            expected.append(self.physical_command(target, tests))
        self.assertEqual(calls, [{"args": ["+1.97.1", "llvm-cov", *command],
                                 "profile": "build-workspace"} for command in expected])
        self.assertTrue((directory / "portable/coverage.json").is_file())
        self.assertTrue((directory / "portable/html/index.html").is_file())
        self.assertFalse((directory / "coverage.json").exists())
        self.assertEqual((directory / "metal-status.txt").read_text(), "incomplete\n")
        for index, (group, _, _) in enumerate(METAL_GROUPS):
            status = "passed\n" if index < failed_index else "incomplete\n"
            self.assertEqual((directory / f"metal-{group}-status.txt").read_text(), status)
            output = (directory / f"metal-{group}.log").read_text()
            self.assertNotIn("old physical log", output)
            if index > failed_index:
                self.assertEqual(output, "")

    def test_metal_stage_precedes_separate_profile_gates(self):
        self.run_case(mode="gate", floor="91", metal=True)

    def test_metal_baseline_does_not_apply_a_floor(self):
        self.run_case(metal=True)

    def test_omitted_group_prevents_instrumentation(self):
        self.run_case(mode="gate", floor="91", metal=True, omitted_group="lazy",
                      error="requires all nine groups and 27 named tests")

    def test_failed_physical_tests_prevent_completion(self):
        self.run_case(mode="gate", floor="91", metal=True,
                      changes={"COVERAGE_TEST_PHYSICAL": "failed"},
                      error="simulated physical test failure", exit_code=37)

    def test_missing_physical_tests_prevent_completion(self):
        for result in ["missing", "wrong-name", "duplicate-name"]:
            with self.subTest(result=result):
                self.run_case(mode="gate", floor="91", metal=True,
                              changes={"COVERAGE_TEST_PHYSICAL": result},
                              error="group wgpu-lib requires exactly 5 named passing tests")

    def test_zero_matches_cannot_certify_any_group(self):
        for group, _, tests in METAL_GROUPS:
            with self.subTest(group=group):
                self.run_case(mode="gate", floor="91", metal=True,
                              changes={"COVERAGE_TEST_PHYSICAL": "empty",
                                       "COVERAGE_TEST_PHYSICAL_GROUP": group},
                              error=f"group {group} requires exactly {len(tests)} named passing tests")

    def test_last_group_failure_prevents_publication(self):
        self.run_case(mode="gate", floor="91", metal=True,
                      changes={"COVERAGE_TEST_PHYSICAL": "failed",
                               "COVERAGE_TEST_PHYSICAL_GROUP": "aggregate-measurement"},
                      error="simulated physical test failure", exit_code=37)

    def test_ignored_physical_tests_prevent_completion(self):
        self.run_case(mode="gate", floor="91", metal=True,
                      changes={"COVERAGE_TEST_PHYSICAL": "ignored"},
                      error="group wgpu-lib requires exactly 5 named passing tests")

    def test_unreported_physical_tests_prevent_completion(self):
        self.run_case(mode="gate", floor="91", metal=True,
                      changes={"COVERAGE_TEST_PHYSICAL": "unreported"},
                      error="group wgpu-lib requires exactly 5 named passing tests")

    def test_summaries_cannot_replace_individual_outcomes(self):
        for result in ["missing-outcome", "failed-outcome"]:
            with self.subTest(result=result):
                self.run_case(mode="gate", floor="91", metal=True,
                              changes={"COVERAGE_TEST_PHYSICAL": result},
                              error="group wgpu-lib requires exactly 5 named passing tests")

    def test_additional_harness_results_prevent_completion(self):
        for result in ["extra-summary", "failed-summary"]:
            with self.subTest(result=result):
                self.run_case(mode="gate", floor="91", metal=True,
                              changes={"COVERAGE_TEST_PHYSICAL": result},
                              error="group wgpu-lib requires exactly 5 named passing tests")

    def test_optional_gate_failure_is_forwarded(self):
        with tempfile.TemporaryDirectory(prefix="zetesis-check-forward-") as temporary:
            repo = Path(temporary)
            (repo / "scripts").mkdir()
            shutil.copyfile(ROOT / "scripts/check.sh", repo / "scripts/check.sh")
            gate = repo / "scripts/coverage.sh"
            gate.write_text('#!/bin/sh\nprintf "<%s>\\n" "$@"\nexit 37\n')
            gate.chmod(0o700)
            result = subprocess.run(["sh", "scripts/check.sh", "coverage", "--metal"],
                                    cwd=repo, env=clean_environment(), text=True,
                                    capture_output=True, timeout=TIMEOUT, check=False)
            self.assertEqual(result.stdout, "<gate>\n<--metal>\n")
            self.assertEqual(result.returncode, 37)

    def test_invalid_coverage_arguments_are_refused(self):
        for arguments in [["bad"], ["gate", "--bad"], ["gate", ""],
                          ["gate", "--metal", "extra"]]:
            with self.subTest(arguments=arguments):
                result = subprocess.run(["sh", str(ROOT / "scripts/coverage.sh"), *arguments],
                                        env=clean_environment(), text=True, capture_output=True,
                                        timeout=TIMEOUT, check=False)
                self.assertEqual(result.returncode, 2)
                self.assertIn("Usage:", result.stderr)

    def test_invalid_check_arguments_are_refused(self):
        for arguments in [["portable", "--metal"], ["full", "--metal"],
                          ["coverage", "--bad"], ["coverage", ""],
                          ["coverage", "--metal", "extra"]]:
            with self.subTest(arguments=arguments):
                result = subprocess.run(["sh", str(ROOT / "scripts/check.sh"), *arguments],
                                        env=clean_environment(), text=True, capture_output=True,
                                        timeout=TIMEOUT, check=False)
                self.assertEqual(result.returncode, 2)
                self.assertIn("Usage:", result.stderr)

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
