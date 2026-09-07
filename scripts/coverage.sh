#!/bin/sh
# Separate, freshly instrumented profiles; each must pass the committed floor.
set -eu

mode=${1:-gate}
if [ "$#" -gt 1 ]; then
    printf '%s\n' 'Usage: scripts/coverage.sh [gate|baseline]' >&2
    exit 2
fi
case "$mode" in gate|baseline) ;; *)
    printf '%s\n' 'Usage: scripts/coverage.sh [gate|baseline]' >&2
    exit 2 ;;
esac

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
python3 - "$coverage_dir/toolchain.json" "$mode" "$floor" <<'PY'
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
Path(sys.argv[1]).write_text(json.dumps({
    "mode": sys.argv[2], "committed_floor": sys.argv[3], "rustc": rust,
    "cargo_llvm_cov": "0.8.7", "llvm_tools": tools,
    "primary": "workspace --all-features", "supplemental": "zetesis-cli --no-default-features",
    "floor_profiles": ["workspace", "cli-cpu"],
    "default_filename_filters": "cargo-llvm-cov 0.8.7 src/report.rs::ignore_filename_regex",
    "project_added_filename_filters": [],
    "profiles_merged": False,
}, indent=2) + "\n")
PY

run_profile() {
    profile=$1
    shift
    report_dir="$coverage_dir/$profile"
    export CARGO_LLVM_COV_TARGET_DIR="$coverage_dir/build-$profile"
    mkdir -p -- "$report_dir"
    cargo +1.97.1 llvm-cov clean --workspace --locked
    if [ "$profile" = workspace ]; then
        cargo +1.97.1 llvm-cov --workspace "$@" --locked --no-report
        set --
    else
        cargo +1.97.1 llvm-cov "$@" --locked --no-report
        set -- --package zetesis-cli
    fi
    # In 0.8.7, report rejects build-feature flags despite listing them in help.
    # The separate instrumented directories retain each feature configuration.
    cargo +1.97.1 llvm-cov report "$@" --locked --json \
        --output-path "$report_dir/coverage.json"
    cargo +1.97.1 llvm-cov report "$@" --locked --html --output-dir "$report_dir"
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
