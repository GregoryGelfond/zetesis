# Prepared reduct execution

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

Retaining a prepared reduct removes repeated construction, but does not guarantee
faster membership checks. In this comparison, task allocation's Metal route uses
**21.87% less named completion capacity** while taking **10.25% more process
wall time** than the baseline. Its CPU route changes by +0.15%. The result does
not establish lower process memory or a general speedup.

The comparison ran on an Apple M4 Pro using Metal, macOS 26.6.2 (25G83), on
15 September 2026, 00:18:15–00:24:15 UTC. Both native executables use the locked
Rust 1.97.1 release recipe for `aarch64-apple-darwin`. The measured sources are
`9b8cf74c818b884b2a7510ec6d98b0ee0873d6cb` and
`d8a4a964e4db8eef535f0d8ff950572df61ba515`. Subsequent documentation changes
are outside those executable identities.

| Measured executable | SHA-256 |
| --- | --- |
| Baseline `zetesis`, source `9b8cf74c` | `2e19a2e4d5bdb2d7e58edede4e828b31b59166eda0959f403c7b4588ef19418b` |
| Prepared `zetesis`, source `d8a4a964` | `caf84a0785a190f9a12b0a3b232f7c931f92b3d251ab52c8bf9252c01ceeebb3` |
| Fixed `zetesis-perf` | `a5499a49dc29de1b5120c5dd0fdd2b4b325d1da6d86bfd23a12b92ca090c69a9` |
| clingo 5.8.2 | `31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015` |

Eight matrices retain baseline/prepared/prepared/baseline order for each suite.
The `baseline` suite contains SEND, queens 02 and task allocation 04/05; the
`queens` suite contains all six original N=8 encodings. Queens 02's two contexts
remain separate. All source files and includes are unchanged between versions.

All **540 planned positions** completed: 180 CPU, 180 Metal and 180 clingo.
Each producer has 36 qualification, 36 warmup and 108 timed observations.
The eight runners and 24 metadata captures completed successfully, with no
faults, unresolved children or missing positions. Native solves exited 0 and
clingo exited 30. Before/after executable and input seals agreed. Every Metal
observation records the Apple M4 Pro adapter and actual formula-device work.

Full native typed atom/cost families and multiplicities agree across versions,
backends and phases: one SEND model, 92 for each queens encoding and 1,176 task
optima with cost `[[0,5]]`. Independent qualification comparisons passed for
all corresponding native families. The clingo comparison covers complete
selected displays, multiplicities and final costs; it does not reconstruct
hidden reference atoms. Task optN output has 1,178 raw witnesses: one is
nonoptimal, and removing the first incumbent replay from the 1,177 best-cost
witnesses leaves exactly 1,176 final ties.

The table gives process-wall medians in milliseconds. Each native version has
six timed observations, grouped into two blocks of three. The clingo column
uses all twelve timed observations. Qualification and warmup are excluded;
percentage changes compare prepared execution with the baseline.

| Workload | CPU baseline → prepared | Change | Metal baseline → prepared | Change | Clingo |
| --- | ---: | ---: | ---: | ---: | ---: |
| SEND | 39.858 → 38.044 | -4.55% | 66.924 → 67.010 | +0.13% | 11.572 |
| Queens 02, baseline suite | 97.149 → 96.976 | -0.18% | 111.611 → 110.968 | -0.58% | 124.483 |
| Task allocation 04/05 | 323.222 → 323.711 | +0.15% | 564.760 → 622.673 | +10.25% | 181.757 |
| Queens 01 | 19.199 → 19.225 | +0.13% | 34.260 → 34.319 | +0.17% | 6.540 |
| Queens 02, queens suite | 96.488 → 95.833 | -0.68% | 111.574 → 111.573 | -0.00% | 123.262 |
| Queens 03 | 20.495 → 19.752 | -3.63% | 34.832 → 34.293 | -1.55% | 6.520 |
| Queens 04 | 11.655 → 11.649 | -0.05% | 25.455 → 26.113 | +2.59% | 6.513 |
| Queens 05 | 48.992 → 49.211 | +0.45% | 65.773 → 65.703 | -0.11% | 6.575 |
| Queens 06 | 49.296 → 49.399 | +0.21% | 67.096 → 67.139 | +0.06% | 6.577 |

Task Metal block medians are 565.154 / 623.350 / 621.238 / 562.684 ms in
acquisition order. Its timed ranges do not overlap: baseline 562.448–566.380 ms,
prepared 619.093–626.428 ms. The closing baseline therefore does not explain
this regression. Smaller changes are less decisive. Queens 03 Metal closes at
34.309 ms, essentially the two prepared medians, after opening at 35.598 ms.
Queens 04's +2.59% is concentrated in one prepared block. SEND CPU has two lower
prepared medians, but also baseline drift and unchanged search work. These
observations do not establish a broad improvement across programs or GPUs.

Task phase receipts locate the remaining increase in CPU residual completion:

| Task measurement | Baseline | Prepared |
| --- | ---: | ---: |
| Completion coordinator wall, ms | 83.891 | 142.297 |
| Summed worker reduct time, ms | 317.404 | 547.880 |
| Summed worker original validation, ms | 9.407 | 9.671 |
| Solving stage, ms | 330.339 | 388.051 |
| GPU host phase, ms | 155.784 | 154.912 |
| Output stage, ms | 218.385 | 217.867 |
| Grounding stage, ms | 3.183 | 3.189 |
| Search work, observed range | 60,313,761–60,332,346 | 70,781,247–70,803,794 |
| Named completion peak, bytes | 6,092,476 | 4,759,876 |
| Named GPU capacity, bytes | 2,387,368 | 2,387,368 |

The approximately 58 ms coordinator increase closely matches the process-wall
increase. These are separate medians, not an additive decomposition. Worker
times overlap across four workers. Both versions submit 1,208 candidates in
19 GPU batches; 1,194–1,195 require CPU residual checks and 13–14 are decided by
the GPU. All completion attempts finish. Device work stays near 3.024 billion
units, with small sweep variation.

Prepared setup takes 0.336 ms median. It records 83,017 work units, 6,087
variables, 14,868 clauses and 34,087 literals, with 466,616 retained and 767,976
peak bytes. Query workspace peaks at 1,041,158 bytes. Baseline preparation
receipts are unavailable, not zero. The remaining structural difference is
candidate-dependent propagation and undo in a fixed expanded encoding, versus
candidate-specific substitution and folding; the measurements do not isolate
those operations' individual timing costs.

The completion-capacity saving is 1,332,600 bytes. Other measured completion
peaks increase: SEND 513 → 993 bytes, and queens plus task CPU 32,832 → 63,552
bytes. Their search work is unchanged, as is non-task Metal device work.
These counters describe named owners and operations; they are neither RSS nor
an end-to-end allocation bound. Passing comparisons and related semantic Lean
laws do not constitute a proof of the complete executable or GPU implementation.

To reproduce the protocol, build the named sources in separate checkouts with
separate build directories. In each checkout, use the same release configuration:

```sh
cargo +1.97.1 build --locked --offline --release \
    --target aarch64-apple-darwin \
    -p zetesis-cli -p zetesis-experiments -p zetesis-validation \
    --bins --all-features
```

Dependencies must already be available for the offline build. Preserve actual
build arguments, compiler/SDK identity and executable hashes; a rebuild need
not reproduce the recorded binary bytes. Use one fixed `zetesis-perf` built
from the prepared source for both solvers, so its telemetry decoder supports
both records. Stop other builds and measurements during acquisition.

From the prepared checkout root, replace the executable paths below and create
a fresh output directory. The maintained runner snapshots the selected corpus
closure and retains exact plans, captures, dispositions and input identities.

```sh
perf_command=/absolute/path/to/prepared/zetesis-perf
baseline_solver=/absolute/path/to/baseline/zetesis
prepared_solver=/absolute/path/to/prepared/zetesis
clingo_command=/absolute/path/to/clingo
results_dir=$(mktemp -d "${TMPDIR:-/tmp}/zetesis-reduct.XXXXXX")

matrix() {
    comparison_label=$1
    comparison_solver=$2
    comparison_suite=$3
    if /usr/bin/env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin \
        LC_ALL=C TMPDIR=/private/tmp \
        "$perf_command" examples/correctness \
        --zetesis "$comparison_solver" --clingo "$clingo_command" \
        --report "$results_dir/$comparison_label.json" \
        --suite "$comparison_suite" \
        --profile cpu-eager --profile metal-eager --formula-joins indexed \
        --workers 4 --completion-workers 4 --clingo-workers 1 --batch-size 64 \
        --warmups 1 --repetitions 3 --timeout-seconds 10 --campaign-seconds 90 \
        --sample-bytes 33554432 --native-report-bytes 33554432 \
        --capture-bytes 134217728 --report-bytes 268435456 \
        > "$results_dir/$comparison_label.stdout" \
        2> "$results_dir/$comparison_label.stderr"; then
        comparison_exit=0
    else
        comparison_exit=$?
    fi
    printf '%s\n' "$comparison_exit" > "$results_dir/$comparison_label.exit" || return 2
    return "$comparison_exit"
}

matrix baseline-1-baseline "$baseline_solver" baseline || exit $?
matrix baseline-1-queens "$baseline_solver" queens || exit $?
matrix prepared-1-baseline "$prepared_solver" baseline || exit $?
matrix prepared-1-queens "$prepared_solver" queens || exit $?
matrix prepared-2-baseline "$prepared_solver" baseline || exit $?
matrix prepared-2-queens "$prepared_solver" queens || exit $?
matrix baseline-2-baseline "$baseline_solver" baseline || exit $?
matrix baseline-2-queens "$baseline_solver" queens || exit $?
```

Inspect each retained report before continuing. Preserve failures, partial
captures and unattempted positions; do not replace them with successful retries.
The unchanged bounds are 10 seconds per child, 90 seconds per matrix, 32 MiB
per captured child/native decoder input, 128 MiB total capture and 256 MiB
per report. Retain the decoder's other structural limits. Deadlines are polling
boundaries; cleanup and publication can extend process wall time.

Both native profiles use eager Indexed grounding and four requested execution
and completion workers; clingo uses one. The CPU route actually selects tight
support, while Metal uses formula countermodel checking. Task residuals use
four CPU completion workers; other Metal cases require none. Requested worker
counts alone do not establish parallel activity.

Fresh-process wall includes loading, grounding, GPU context preparation,
solving and JSON/statistics output. Comparison and hashing are excluded; no
cold-cache condition is claimed. These matrices collect neither RSS nor kernel
clock time. Their instrumented CPU counterparts are the appropriate comparison
within this protocol; the separate [ordinary CPU measurements](performance.md)
have a different timing boundary and must not be pooled with them.
