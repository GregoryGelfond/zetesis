# Complete optimization-aware solver comparisons

`compare-optimal.py` compares complete executions of the original manifest cases.
It supports optimized and objective-free inputs. It leaves the historical
`compare-clingo.py` runner and its SEND+MORE measurement record unchanged.
No timings are claimed by this tooling change; its automated tests use recorded
outputs and synthetic Python executables.

Run from the repository after building the native binary and completing other
checks. Do not run performance campaigns alongside builds or tests.

```sh
python3 scripts/compare-optimal.py \
  --case scenarios/task-allocation/variant-04/05-larger-mix.lp \
  --zetesis target/release/zetesis \
  --report work/task-allocation-comparison.json

python3 scripts/compare-optimal.py \
  --case standalone/n-queens/variant-01.lp \
  --zetesis target/release/zetesis \
  --report work/queens-comparison.json
```

Defaults are 21 timed pairs, three warmup pairs, one separately labelled
first-observed pair, five separate memory pairs, and a 30-second limit per
process. `--memory-runs 0` omits memory measurements. Native execution explicitly
uses CPU, one worker, automatic oracle/materialization, and all displayed models;
clingo uses one thread and `--opt-mode=optN`. Limit defaults and versions are
captured from each measured executable, and the complete commands and native
phase/profile diagnostics are retained. Neither process receives input rewrites.

Every pair checks satisfiability, the complete objective vector and the
**displayed-model multiset**, including empty or repeated displays. Duplicate
symbols inside a display also survive. The parser validates clingo's raw witness
count and `Models.Optimal`, then removes exactly the first final-cost incumbent
replay emitted by `optN`. It does not deduplicate the remaining optimal displays.
Native output must report one exhausted coverage record, consistent status and
count, and the same objective vector on every optimal model. Quoted atom strings
retain spaces, escaping and literal newlines. These are display/count contracts;
hidden atoms cannot be reconstructed from `#show`.

The byte-pinned manifest supplies every transitive original source file and its
original `@expect`, `@cost`, `@count`, `@model`, `@optimal`, and `@cautious optimal`
contracts. Both solvers must satisfy those original contracts, in addition to
agreeing with each other, before a timing result is accepted. `@note` remains
metadata; unknown contract tags are refused. Up to 256 source files and 16 MiB of
source bytes are admitted. The supplied corpus directory must contain the exact
manifest hashes. Source, dependency, manifest, runner, parser, memory-wrapper and
binary/Python-interpreter hashes are retained before and after the campaign. The manifest's old
historical status labels are not interpreted as current solver results.

Timed pairs alternate which solver runs first, keeping the imbalance at most
one pair. Wall time covers direct process creation, parsing, grounding, solving,
and bounded stdout/stderr collection. Hashing and solver-configuration queries
precede the first-observed pair, so that phase is **not a cold-cache claim**.
Caches are not flushed. Native and clingo can select different internal
algorithms; this is an end-to-end CPU comparison, not an isolated kernel test.

RSS uses a separate fresh Python parent for each memory run and
`resource.getrusage(RUSAGE_CHILDREN).ru_maxrss`. The Python wrapper's own RSS is
excluded. macOS reports bytes; Linux reports KiB, converted explicitly to bytes.
The raw value, units and wrapper command are recorded. Timing summaries exclude
all warmup, first-observed and memory runs. Reported quartiles use the inclusive
method; ratios compare median timed wall duration.

Capture is capped at 4 MiB per process and 64 MiB of cumulative retained
stdout/stderr by default. `--max-output-bytes` and `--max-captured-bytes` expose
those allowances. They bound raw capture payload, not exact JSON/allocator
storage, which also includes parsed answers and escaping. Timeout/output failures
kill only the process group created by that call. Source changes, contract
mismatches, partial enumeration, unsupported RSS units and other incomplete runs
produce no accepted comparison summary. Schema-2 reports retain failure evidence
and before/after integrity results. Reports cannot overwrite tracked inputs or
measurement executables/scripts.

The portable checks need only Python's standard library:

```sh
python3 -m unittest discover -s scripts/tests -v
```

They cover the `optN` replay protocol, hidden equal displays, objective vectors,
original contracts, malformed/incomplete outputs, quoted strings, the historical
SEND+MORE record, timeout/output ceilings, source include hashes, child-only RSS
units and a complete report round trip using synthetic binaries. They never
launch zetesis or clingo and establish no performance result.
