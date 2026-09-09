#!/bin/sh
# Separate, freshly instrumented profiles; each must pass the committed floor.
set -eu

mode=${1:-gate}
metal=${2:-}
if [ "$#" -gt 2 ] || { [ "$#" -eq 2 ] && [ "$metal" != --metal ]; }; then
    printf '%s\n' 'Usage: scripts/coverage.sh [gate|baseline] [--metal]' >&2
    exit 2
fi
case "$mode" in gate|baseline) ;; *)
    printf '%s\n' 'Usage: scripts/coverage.sh [gate|baseline] [--metal]' >&2
    exit 2 ;;
esac
# Each row names a report group, Cargo target, required count and exact tests.
# All invocations keep workspace feature unification, including the library
# selection: narrowing to one package can change instrumented dependency builds.
metal_groups='wgpu-lib|lib|5|aggregate::device::tests::metal_aggregate_readback_failure_retains_submitted_work lazy::transport_tests::metal_lazy_transport_reuse_preserves_round_truth lazy::transport_tests::metal_input_slack_preserves_exact_admission lazy::transport_tests::metal_lazy_transport_refusal_preserves_reuse lazy::transport_tests::metal_lazy_transport_cancelled_read_discards_capacity
tight|hardware_tight|4|metal_support_matches_exact_reduct_semantics metal_support_preserves_batch_isolation metal_support_refusals_preserve_reusable_residency metal_support_residency_tracks_theory_identity
formula|hardware_formula|2|metal_formula_limits_resize_identity_and_word_boundaries_remain_explicit metal_formula_queries_preserve_exact_frozen_semantics_and_residency
aggregate|hardware_aggregate|3|metal_aggregate_reductions_match_native_occurrences metal_aggregate_guards_preserve_numeric_boundaries metal_aggregate_exact_admission_preserves_cache_lifecycle
lazy|hardware_lazy|4|metal_lazy_worlds_match_exact_frozen_cpu_closures metal_lazy_growth_preserves_previous_round_truth metal_lazy_catalog_fits_when_static_carrier_refuses metal_source_selections_preserve_each_frozen_closure
cli-lazy|lazy_gpu|4|physical::ordinary_lazy_metal_preserves_complete_cpu_models physical::requested_model_limit_retains_completed_lazy_candidates physical::lazy_source_stop_preserves_unfinished_candidate_counts physical::lazy_writer_failure_preserves_completed_device_work
cli-formula|formula_gpu|2|physical::ordinary_metal_formula_batches_match_complete_cpu_models_costs_and_displays physical::ordinary_metal_formula_limits_preserve_partial_coverage_and_writer_errors
world-views|world_views_gpu|2|metal_world_view_preserves_nonoptimal_answers metal_collection_limit_retains_checked_accounting
aggregate-measurement|aggregate_measurement|1|metal_aggregate_measurements_require_actual_submissions'

repo_dir=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd -- "$repo_dir"
coverage_dir="$repo_dir/target/coverage"
mkdir -p -- "$coverage_dir"
if ! mkdir -- "$coverage_dir/.lock" 2>/dev/null; then
    printf '%s\n' 'Another coverage run owns target/coverage/.lock; reports cannot be shared concurrently.' >&2
    exit 2
fi
trap 'rmdir -- "$coverage_dir/.lock"' 0
trap 'exit 130' INT
trap 'exit 143' TERM
# A previous complete report never certifies an interrupted new run.
printf '%s\n' incomplete > "$coverage_dir/status.txt"
floor=$(python3 - "$mode" scripts/coverage-floor.txt <<'PY'
from pathlib import Path
import sys

mode, path = sys.argv[1:]
value = Path(path).read_text().strip()
if value == "UNMEASURED":
    if mode == "gate":
        sys.exit("Coverage floor is unmeasured; run baseline and review its evidence first.")
elif not value.isascii() or not value.isdecimal() or not 0 <= int(value) <= 100:
    sys.exit("Coverage floor must be UNMEASURED or an integer percentage from 0 to 100.")
print(value)
PY
)

tool_version=$(cargo +1.97.1 llvm-cov --version)
if [ "$tool_version" != 'cargo-llvm-cov 0.8.7' ]; then
    printf 'Expected cargo-llvm-cov 0.8.7; found %s\n' "$tool_version" >&2
    exit 2
fi
if { [ -n "${LLVM_COV:-}" ] && [ -z "${LLVM_PROFDATA:-}" ]; } ||
   { [ -z "${LLVM_COV:-}" ] && [ -n "${LLVM_PROFDATA:-}" ]; }; then
    printf '%s\n' 'Set both LLVM_COV and LLVM_PROFDATA, or neither.' >&2
    exit 2
fi
if [ -z "${LLVM_COV:-}" ]; then
    sysroot=$(rustc +1.97.1 --print sysroot)
    host=$(rustc +1.97.1 -vV | sed -n 's/^host: //p')
    LLVM_COV="$sysroot/lib/rustlib/$host/bin/llvm-cov"
    LLVM_PROFDATA="$sysroot/lib/rustlib/$host/bin/llvm-profdata"
fi
export LLVM_COV LLVM_PROFDATA
python3 - "$coverage_dir/toolchain.json" "$mode" "$floor" "$metal" "$metal_groups" <<'PY'
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess
import sys

def output(command):
    return subprocess.check_output(command, text=True).strip()

rust = output(["rustc", "+1.97.1", "-vV"])
match = re.search(r"^LLVM version: ([0-9]+\.[0-9]+\.[0-9]+)$", rust, re.M)
if not match:
    sys.exit("Pinned rustc did not report its LLVM version.")
accepted_versions = {match.group(1), match.group(1) + "-rust-1.97.1-stable"}
tools = {}
for name in ["LLVM_COV", "LLVM_PROFDATA"]:
    path = Path(os.environ[name]).resolve(strict=True)
    version = output([str(path), "--version"])
    llvm = re.findall(r"^[ \t]*LLVM version ([^\r\n]+)$", version, re.M)
    if len(llvm) != 1 or llvm[0].strip() not in accepted_versions:
        sys.exit(f"{name} does not match rustc LLVM {match.group(1)}: {version}")
    tools[name] = {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "version": version}
physical_groups = []
if sys.argv[4] == "--metal":
    for row in sys.argv[5].splitlines():
        group, target, expected, names = row.split("|")
        tests = names.split()
        if len(tests) != int(expected) or len(set(tests)) != len(tests):
            sys.exit(f"Invalid physical coverage selection: {group}")
        physical_groups.append({
            "group": group, "target_kind": "lib" if target == "lib" else "test",
            "target": "workspace libraries" if target == "lib" else target,
            "tests": tests, "expected_tests": int(expected),
        })
    # These totals describe this reviewed scope, independently of each row.
    if (len(physical_groups) != 9 or
            len({group["group"] for group in physical_groups}) != 9 or
            sum(group["expected_tests"] for group in physical_groups) != 27):
        sys.exit("Physical coverage requires all nine groups and 27 named tests.")
physical_tests = [test for group in physical_groups for test in group["tests"]]
Path(sys.argv[1]).write_text(json.dumps({
    "mode": sys.argv[2], "committed_floor": sys.argv[3], "rustc": rust,
    "cargo_llvm_cov": "0.8.7", "llvm_tools": tools,
    "primary": "workspace --all-features", "supplemental": "zetesis-cli --no-default-features",
    "floor_profiles": ["workspace", "cli-cpu"],
    "default_filename_filters": "cargo-llvm-cov 0.8.7 src/report.rs::ignore_filename_regex",
    "project_added_filename_filters": [],
    "profiles_merged": False,
    "profile_merge_scope": (
        "profiles_merged describes floor profiles; raw execution profiles combine "
        "only within their own floor profile"
    ),
    "workspace_execution": "portable+metal" if physical_tests else "portable",
    "workspace_stages": ["portable", "metal"] if physical_tests else ["portable"],
    "physical_test_groups": physical_groups,
    "physical_tests": physical_tests,
    "expected_physical_tests": len(physical_tests),
    "physical_scope": (
        "27 exact Metal tests: native aggregate reduction and measurement, lazy "
        "transport and source closure, tight and formula oracles, and ordinary "
        "lazy/formula CLI paths and complete-world-view collection. "
        "Unlisted tests and Vulkan are not selected."
    ) if physical_tests else None,
}, indent=2) + "\n")
PY

write_report() {
    destination=$1
    shift
    mkdir -p -- "$destination"
    # In 0.8.7, report rejects build-feature flags despite listing them in help.
    # The separate instrumented directories retain each feature configuration.
    cargo +1.97.1 llvm-cov report "$@" --locked --json \
        --output-path "$destination/coverage.json"
    cargo +1.97.1 llvm-cov report "$@" --locked --html --output-dir "$destination"
}

run_metal_group() {
    group=$1
    target=$2
    expected=$3
    names=$4
    # Splitting is intentional: these are fixed libtest identifiers, not input.
    set -- $names
    if [ "$target" = lib ]; then
        set -- --lib --locked --no-report --no-clean -- --ignored --nocapture \
            --test-threads=1 --exact "$@"
    else
        set -- --test "$target" --locked --no-report --no-clean -- --ignored \
            --nocapture --test-threads=1 --exact "$@"
    fi
    physical_log="$coverage_dir/workspace/metal-$group.log"
    physical_status="$coverage_dir/workspace/metal-$group-status.txt"
    # Keep workspace feature unification and the existing instrumented target.
    # --no-clean retains the portable profile; no CLI-CPU data enters this stage.
    if CARGO_TERM_COLOR=never cargo +1.97.1 llvm-cov --workspace --all-features \
        "$@" > "$physical_log" 2>&1; then
        cat "$physical_log"
    else
        physical_exit=$?
        cat "$physical_log" >&2 || :
        return "$physical_exit"
    fi
    # Cargo permits a successful zero-match selection. Check both named test
    # records and pinned libtest summaries; output within a test may share its
    # first line under --nocapture. Other workspace libraries may report zero.
    if ! python3 - "$physical_log" "$group" "$target" "$expected" "$names" <<'PY'
from collections import Counter
from pathlib import Path
import re
import sys

path, group, target, expected, names = sys.argv[1:]
output = Path(path).read_text()
records = re.findall(r"^test (\S+) \.\.\.[ \t]*(.*?)(?=^test |\Z)", output, re.M | re.S)
selected = [name for name, _ in records]
# With serial --nocapture output, each test block ends in its own outcome.
# A summary cannot substitute for a missing individual outcome record.
passed_records = all(re.search(r"(?:^|\n)ok[ \t\r\n]*\Z", body) for _, body in records)
summaries = re.findall(r"^test result: .*$", output, re.M)
counts = []
for summary in summaries:
    match = re.fullmatch(
        r"test result: ok\. ([0-9]+) passed; 0 failed; 0 ignored; 0 measured; "
        r"[0-9]+ filtered out; finished in [^\r\n]+", summary)
    if not match:
        break
    counts.append(int(match.group(1)))
else:
    positive = [count for count in counts if count > 0]
    if (Counter(selected) == Counter(names.split()) and passed_records and
            positive == [int(expected)] and
            (target == "lib" or len(counts) == 1)):
        sys.exit(0)
sys.exit(f"Physical coverage group {group} requires exactly {expected} "
         "named passing tests and complete libtest summaries.")
PY
    then
        return 1
    fi
    printf '%s\n' passed > "$physical_status"
}

run_metal() {
    printf '%s\n' incomplete > "$coverage_dir/workspace/metal-status.txt"
    # Reset every group before any execution, including groups an early failure
    # would leave unvisited. Old logs and status markers cannot certify this run.
    while IFS='|' read -r group target expected names; do
        printf '%s\n' incomplete > "$coverage_dir/workspace/metal-$group-status.txt"
        : > "$coverage_dir/workspace/metal-$group.log"
    done <<EOF
$metal_groups
EOF
    while IFS='|' read -r group target expected names; do
        run_metal_group "$group" "$target" "$expected" "$names"
    done <<EOF
$metal_groups
EOF
    printf '%s\n' passed > "$coverage_dir/workspace/metal-status.txt"
}

run_profile() {
    profile=$1
    shift
    report_dir="$coverage_dir/$profile"
    export CARGO_LLVM_COV_TARGET_DIR="$coverage_dir/build-$profile"
    mkdir -p -- "$report_dir"
    cargo +1.97.1 llvm-cov clean --workspace --locked
    if [ "$profile" = workspace ]; then
        cargo +1.97.1 llvm-cov --workspace "$@" --locked --no-report
        if [ "$metal" = --metal ]; then
            write_report "$report_dir/portable"
            run_metal
        fi
        set --
    else
        cargo +1.97.1 llvm-cov "$@" --locked --no-report
        set -- --package zetesis-cli
    fi
    write_report "$report_dir" "$@"
}

run_profile workspace --all-features
run_profile cli-cpu --package zetesis-cli --no-default-features

export CARGO_LLVM_COV_TARGET_DIR="$coverage_dir/build-workspace"
if [ "$mode" = gate ]; then
    cargo +1.97.1 llvm-cov report --locked --fail-under-lines "$floor"
    export CARGO_LLVM_COV_TARGET_DIR="$coverage_dir/build-cli-cpu"
    cargo +1.97.1 llvm-cov report --package zetesis-cli --locked --fail-under-lines "$floor"
    printf '%s\n' gate-passed > "$coverage_dir/status.txt"
else
    printf '%s\n' 'baseline-complete (nongating)' > "$coverage_dir/status.txt"
fi
printf 'Coverage %s completed; reports: %s\n' "$mode" "$coverage_dir"
