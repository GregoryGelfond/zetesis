#!/bin/sh
# Qualify general Ferraris propagation on a physical Metal device.
set -eu
if [ "$#" -gt 1 ]; then
    printf '%s\n' 'Usage: scripts/qualify-metal-formula.sh [new-result-directory]' >&2
    exit 2
fi
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
result_dir=${1:-"$repo_dir/target/metal-formula-$(date -u +%Y%m%dT%H%M%SZ)"}
mkdir -p -- "$(dirname -- "$result_dir")"
mkdir -- "$result_dir"
# A failed or interrupted invocation retains its partial evidence and status.
trap 'result=$?; if [ "$result" -ne 0 ]; then printf "FAIL: exit %s; inspect retained diagnostics.\n" "$result" > "$result_dir/status.txt"; fi' EXIT
printf '%s\n' 'INCOMPLETE: qualification has not finished.' > "$result_dir/status.txt"
command -v zetesis > "$result_dir/binary-paths.txt"
command -v zetesis-bench >> "$result_dir/binary-paths.txt"
zetesis --version > "$result_dir/version.txt"
shasum -a 256 "$(command -v zetesis)" "$(command -v zetesis-bench)" \
    > "$result_dir/binary-sha256.txt"
python3 - "$repo_dir" > "$result_dir/source-sha256.json" <<'PY'
import hashlib
import json
from pathlib import Path
import sys

root = Path(sys.argv[1])
paths = [root / name for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml")]
paths.extend(p for p in (root / "crates").rglob("*")
             if p.is_file() and (p.suffix in (".rs", ".wgsl") or p.name == "Cargo.toml"))
print(json.dumps({str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
                  for p in sorted(paths)}, indent=2))
PY
uname -a > "$result_dir/system.txt"
system_profiler SPDisplaysDataType -json > "$result_dir/graphics.json"
zetesis devices > "$result_dir/devices.txt" 2> "$result_dir/device-diagnostics.txt"
printf '%s\n' 'zetesis-bench formula --backend metal --atoms 64,256 --batches 1,64,256 --repetitions 5 --cpu-workers 4' \
    > "$result_dir/command.txt"
zetesis-bench formula --backend metal --atoms 64,256 --batches 1,64,256 --repetitions 5 --cpu-workers 4 \
    > "$result_dir/formula.tsv" 2> "$result_dir/benchmark-diagnostics.txt"
printf '%s\n' 'PASS: synthetic formula membership matched native CPU; warm theory and transport reused. GPU residuals were checked on CPU. Source grounding and outer search excluded.' \
    > "$result_dir/status.txt"
printf 'Formula Metal qualification complete: %s\n' "$result_dir"
