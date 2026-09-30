# Measurement protocols and early results

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

For everyday benchmarking and current results, start with
[Benchmarks and comparisons](performance.md). This reference is for maintainers
reproducing a particular measurement or deriving a workload through the library.

The recipes retain the compatibility command names used by the recorded runs.
They do not qualify a newer executable. The results below identify their own
sources, dates, settings and measurement limits.

| Task | Section |
| --- | --- |
| Compare unchanged source cases | [Measure the relevant work](#measure-the-relevant-work) |
| Change a workload constant without editing the corpus | [Parameterized workloads](#compare-a-parameterized-workload) |
| Compare successive executables on fixed cells | [Fixed series](#measure-the-series) |
| Read early CPU and Metal observations | [Performance evidence](#performance-evidence) |

## Measure the relevant work

Use `zetesis-bench corpus` for the installed explicit-profile matrix interface.
`zetesis-bench perf`, formerly the separate `zetesis-perf` executable, retains
the additional protocols and argument spellings used by the recipes and measured
records below, which keep the `zetesis-perf` name of the binaries they record.
See the
[comparison guide](https://github.com/GregoryGelfond/zetesis/blob/main/scripts/README-comparison.md) for its schedules,
capture bounds and report formats. Direct wall time includes process startup,
source loading, grounding, solving and captured output. The CPU baseline keeps
instrumented phase observations in separate samples. The explicit profile matrix
includes JSON/statistics in its timed native runs and forms a separate
population. Keep the selected inputs, binaries, backend,
worker counts, limits and warmup schedule with each result.

Repeated `--case` arguments select unchanged cases from the sealed corpus.
`--memory-runs` adds a separate population of fresh-child resource observations
on macOS or Linux, to the ordinary campaign and to the matrix and series
campaigns alike; it does not add samples to the wall-time distribution. Its
reported child peak RSS excludes the measuring helper and is neither simultaneous
process-tree memory nor GPU memory. These selected/resource campaigns use their
own versioned report view; the series view reports the median peak RSS of each
solver beside its timing.

For example, compare three different encodings with complete CPU/eager solves:

```sh
zetesis-perf examples/correctness \
  --case standalone/n-queens/variant-02.lp \
  --case standalone/send-money/send-money.lp \
  --case scenarios/task-allocation/variant-04/05-larger-mix.lp \
  --zetesis /path/to/zetesis --clingo /path/to/clingo \
  --warmups 2 --repetitions 9 --memory-runs 0 \
  --timeout-seconds 10 --campaign-seconds 120 \
  --report target/cpu-eager.json
```

Replace the executable paths and choose an unused report path. This example
uses one native closure worker, one native completion worker and one clingo
worker. Each case has qualification, two warmup pairs, nine timed pairs and a
separate native statistics observation. The report retains those populations
independently. These three cases are a selection, not the complete corpus.

Stop competing builds, tests and measurements before a timing campaign. A small
fixture can establish semantic agreement while being too small to demonstrate
useful parallel speedup. Conversely, a shorter run is not an improvement if it
searched fewer candidates, omitted optimum ties or stopped early.

## Compare a parameterized workload

Use `performance::matrix::run_workloads` to compare a changed constant without
editing the curated examples. The following library client derives N=4 from
the first queens encoding's N=8 declaration. themelios-syntax locates the numeric
token; the admitted include closure remains fixed. The resulting workload
retains the original and derived source identities. Its complete clingo result
is the comparison expectation, rather than the original N=8 model count.

```rust,no_run
# extern crate zetesis_validation;
{{#include ../examples/workload.rs:example}}
```

Build the client from the checkout root, then run it with explicit paths:

```sh
cargo build --locked -p zetesis-validation --example book-workload

target/debug/examples/book-workload examples/correctness \
  /absolute/path/to/zetesis /absolute/path/to/clingo \
  target/queens-n4-comparison.json
```

Choose an unused report path in a directory you control; existing evidence is
never replaced. The executable paths must be absolute. This example uses one
native eager CPU profile and one clingo worker, with qualification followed by
one timed pair and no warmups. Each child has a 30-second deadline and a 4-MiB
combined output ceiling; the campaign has a 180-second scheduling deadline.
These bounds may stop a larger workload. The report is saved before the client
returns failure for any refused, incomplete or disagreeing cell.

The manual only compiles this acquisition block. The registered Cargo example
test checks N=4 source derivation and identity without launching solvers:

```sh
cargo test --locked -p zetesis-validation --example book-workload
```

This instrumented full-enumeration comparison includes JSON/statistics output
and retains the native model records. It checks selected displays, multiplicities
and costs against clingo; hidden clingo atoms are unavailable. It adds neither
time-to-first-answer nor RSS observations. A single pair does not establish a
performance trend.

## Measure the series

A sequence of solver changes is measured on one fixed cell set, so that each
change's effect and the sequence's cumulative effect rest on the same
observations. `performance::series::workloads` names twenty-two cells:
seventeen generated programs from `performance::families` (one shape and one
size each, byte-exact, with closed-form complete families as their
contracts), three amended queens boards and two unchanged entries. The
generated programs reach routes the corpus does not: the closure route, deep
derivation, cyclic and stratified negation, refused admissions, a Latin
square in the shape of Sudoku and a line walked under frame rules.
`zetesis-bench perf --suite series` runs them through the instrumented matrix; `--profile cpu-auto` requests the shipped
defaults and the observation retains the grounding mode each cell took;
`--time-limit` adds a cooperative deadline to every native profile.

`zetesis-bench series` derives one comparison from published reports of the same
cells: exact medians, later-over-earlier ratios, each report's native median
over the reference solver's median on the same cell, the retained counters and
each report's native seal, with cells that did not pass listed by decision,
and a scoreboard against the reference solver per report and profile, whose
parts the validation crate's README describes. The
[comparison guide](https://github.com/GregoryGelfond/zetesis/blob/main/scripts/README-comparison.md#the-fixed-series)
gives the commands. Retained series comparisons live beside the other
[recorded observations](https://github.com/GregoryGelfond/zetesis/blob/main/docs/book/reference/observations/README.md).

## Performance evidence

The [grounding comparison](grounding-measurements.md) reports ordinary CPU wall time and
child peak RSS for sources `ca10a5e7`, `f56a5a24` and `679ca856`, alongside
larger queens screens. The [matched CPU/Metal comparison](prepared-metal.md) adds
810 complete eager positions, with exact timed data and the actual device-work
scope. The measured executables report version `0.1.0`.
The grounding comparison also retains the earlier
`6bebb980` → `1e5b78ce` release, Table, Metal and LTO comparisons under their
original source identities. Those historical Metal measurements retain two
acquisition windows separately and include no RSS observations. Physical
regression qualification does not supply new GPU timing measurements.

The measurements below apply to their explicitly named revisions. No timing or
peak-RSS measurements were collected for
[`74c0627f`](https://github.com/GregoryGelfond/zetesis/tree/74c0627f3aab89ae466a1a33cc61e352264b8af1).

Storage contracts and elapsed time are separate results. Formula joins reuse
one cleared expression workspace across prefix checks, generators and final
filters. Packed GPU support reduces the support buffer to one bit per atom.
Neither change establishes a general latency improvement or lower process RSS.

### Ordinary CPU/eager solving

These measurements compare the previous executable built from
[`d871e91b`](https://github.com/GregoryGelfond/zetesis/tree/d871e91b56406c20b312e63f9d3437e6352803e2),
qualified at
[`e69890b2`](https://github.com/GregoryGelfond/zetesis/tree/e69890b234c12f7565923b4a2f7bc312e92eae51),
with [`55f5aa73`](https://github.com/GregoryGelfond/zetesis/tree/55f5aa739ec3fc941f27359dcb4d0c8608b82284).
Both are Rust 1.97.1 release builds measured on Apple M4 Pro on 10 September
2026, with clingo 5.8.2 as the reference.

| Executable | SHA-256 |
| --- | --- |
| Previous zetesis | `dc9224f4df6e1809180a7cb4566110c920e94aecdd43b15124a58058de170907` |
| Current zetesis | `197e49c3f81bd5be18fb19441351b1727a2a3814f5f7ecad66281f271bca66f8` |
| clingo 5.8.2 | `31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015` |

All four previous/current/current/previous blocks passed their displayed-model,
cost and optimum-tie checks: 279 observations per block, 1,116 total. Each case
has one qualification pair, two warmup pairs, nine timed pairs, one separate
native statistics observation and three separate memory pairs. Each native
revision therefore has 18 timed observations and six child peak-RSS observations
per case. Execution uses eager CPU, one closure worker, one completion worker,
`--oracle auto` and complete enumeration.

The table gives the range of the two native block medians for each revision
and the four clingo block medians, in milliseconds. These are ranges of
observations, not confidence intervals. Timed native runs use human output
without statistics; clingo uses JSON. Process startup, grounding, solving,
rendering and capture remain included.

| Case | Previous zetesis, ms | Current zetesis, ms | clingo, ms |
| --- | ---: | ---: | ---: |
| [Queens 1, N=8](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/n-queens/variant-01.lp) | 9.224–9.237 | 9.225–9.239 | 6.160–6.187 |
| [Queens 2, N=8](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/n-queens/variant-02.lp) | 93.551–93.741 | 92.266–93.555 | 123.634–125.123 |
| [Queens 3, N=8](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/n-queens/variant-03.lp) | 9.325–9.338 | 9.323–9.336 | 6.192–6.227 |
| [Queens 4, N=8](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/n-queens/variant-04.lp) | 7.726–7.751 | 7.732–7.753 | 6.157–6.164 |
| [Queens 5, N=8](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/n-queens/variant-05.lp) | 10.733–10.743 | 10.735–10.737 | 6.153–6.169 |
| [Queens 6, N=8](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/n-queens/variant-06.lp) | 12.236–12.256 | 12.251–12.253 | 6.158–6.173 |
| [SEND + MORE = MONEY](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/send-money/send-money.lp) | 40.658–40.839 | 39.393–40.830 | 12.230–13.759 |
| [Task allocation: scheduling](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-04/05-larger-mix.lp) | 123.917–126.926 | 123.913–131.220 | 185.434–194.311 |
| [Shortest path: layered DAG](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-01/06-layered-dag.lp) | 7.884–7.888 | 7.842–7.852 | 6.188–6.241 |

These results do not demonstrate a broad application speedup from the columnar
integration. Most differences are small; one current task-allocation block is
slower than the other three blocks. Zetesis is faster than clingo on queens-02
and this task-allocation case; clingo is faster on the other selected cases.
This selection does not establish a general solver ranking.

Separate peak-memory observations cover the same nine cases:

| Case | Prior zetesis (MiB) | Current zetesis (MiB) | clingo (MiB) |
|---|---:|---:|---:|
| Queens 01 (N=8) | 12.031–12.094 | 12.078–12.156 | 5.422 |
| Queens 02 (N=8) | 12.969–12.984 | 12.906–12.938 | 7.969–8.844 |
| Queens 03 (N=8) | 12.172–12.188 | 12.234–12.297 | 5.438 |
| Queens 04 (N=8) | 12.047–12.125 | 12.125 | 5.469 |
| Queens 05 (N=8) | 12.891–12.922 | 12.828–12.875 | 5.531 |
| Queens 06 (N=8) | 13.094–13.141 | 13.078–13.094 | 5.563 |
| SEND + MORE = MONEY | 26.188–26.203 | 26.094 | 8.281 |
| Task allocation 04/05 | 40.234–40.266 | 40.188–40.234 | 19.359–20.938 |
| Shortest path 01/06 | 12.891–12.906 | 12.875–12.938 | 5.641 |

All four ordinary ABBA reports passed. Each contains three memory observations
per solver per case: six per case for each zetesis build and twelve per case for
clingo, totaling 216 memory observations across the nine cases. The table gives
ranges of block medians (two blocks per zetesis build, four for clingo), rounded
to three decimals; these are descriptive ranges, not confidence intervals.

These are separate fresh-helper memory observations, not the timed samples. On
macOS, the retained raw `ru_maxrss` values are bytes; MiB means 1,048,576 bytes.
The helper waits for the solver and reads `RUSAGE_CHILDREN`, excluding its own
memory. The operating system may propagate usage from descendants reaped by the
solver. This is neither simultaneous process-tree RSS nor GPU/device-memory
usage.

Clingo used less reported peak memory in all nine cases. Current and prior
zetesis values are close and mixed; these observations do not establish a broad
peak-memory reduction from the columnar changes.

These eager CPU runs used one zetesis worker and one completion worker. The
accepted comparisons cover selected output and costs, including optimum ties;
clingo’s hidden interpretations are unavailable.

Reproduce each block with the same nine cases and an unused report path:

```sh
zetesis-perf examples/correctness \
  --case standalone/n-queens/variant-01.lp \
  --case standalone/n-queens/variant-02.lp \
  --case standalone/n-queens/variant-03.lp \
  --case standalone/n-queens/variant-04.lp \
  --case standalone/n-queens/variant-05.lp \
  --case standalone/n-queens/variant-06.lp \
  --case standalone/send-money/send-money.lp \
  --case scenarios/task-allocation/variant-04/05-larger-mix.lp \
  --case scenarios/shortest-path/variant-01/06-layered-dag.lp \
  --zetesis /path/to/zetesis --clingo /path/to/clingo \
  --warmups 2 --repetitions 9 --memory-runs 3 \
  --timeout-seconds 30 --campaign-seconds 180 \
  --sample-bytes 4194304 --capture-bytes 134217728 --report-bytes 536870912 \
  --report target/ordinary-cpu.json
```

Run the frozen executables in previous/current/current/previous order, retaining
four reports. Compare block medians without pooling away unchanged-executable
drift. The source manifest is
`b43df1adf17ae0c035f1e310a5c15345c26cbcad8b59596932627c46fd1c6958`.
The 4 MiB per-process capture ceiling, 128 MiB cumulative capture allowance and
512 MiB serialized-report ceiling apply independently.

### Four-case CPU comparison, 11 September 2026

A separate comparison of four unchanged corpus cases used the builds from
[`3afaf719`](https://github.com/GregoryGelfond/zetesis/tree/3afaf719949de0e8c2162e6ff2b107fb26675c24),
qualified at [`dca674c0`](https://github.com/GregoryGelfond/zetesis/tree/dca674c067e08286d91cd8426bacb5932b42e1ac),
and
[`0d287734`](https://github.com/GregoryGelfond/zetesis/tree/0d2877346b4d5822b74c655a34e3daa3c1938913).
These are Rust 1.97.1 release builds for macOS arm64, measured on Apple M4 Pro
running macOS 26.6.2 (build 25G83). One fixed `zetesis-perf` executable served
both revisions, and clingo 5.8.2 was invoked directly.
These observations are separate from the September 10 nine-case comparison.

| Executable | SHA-256 |
| --- | --- |
| zetesis `3afaf719` | `7be291273d37c9814411b3165cfddca0c09967eae8bf9b411d45eba77f03e69b` |
| zetesis `0d287734` | `20406e3936563b17e006d659c29baa1b1c0dd208f5e4afa020add000d368b3f7` |
| Shared zetesis-perf runner | `2fd427ec77ec91faa038fbeddf10e686f2f539cc6c2222c16635a8084a637469` |
| Direct clingo 5.8.2 | `31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015` |

Four sequential `3afaf719 / 0d287734 / 0d287734 / 3afaf719` blocks used eager CPU,
`--oracle auto`, one search worker, one completion worker and complete
enumeration. Each case had qualification, two warmup pairs, five timed pairs,
one separate native statistics observation and three separate memory pairs.
All 368 solve observations passed: queens-02 retained all 92 displayed models,
SEND retained its unique model, task allocation retained all 1,176 optimum ties
at cost `[5]`, and shortest path retained its unique optimum at cost `[4,4]`.
The comparison preserves displayed-model and symbol multiplicities; clingo's
hidden interpretations remain unavailable.

Wall-time ranges below span the two block medians for each zetesis revision
and the four clingo block medians. Each median uses five observations. They
include startup, grounding, solving, output and capture; native runs use human
output without statistics, while clingo uses JSON. These descriptive ranges
are not confidence intervals.

| Case | zetesis `3afaf719`, ms | zetesis `0d287734`, ms | Direct clingo, ms |
| --- | ---: | ---: | ---: |
| [Queens 2, N=8](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/n-queens/variant-02.lp) | 93.173–93.467 | 94.042–94.846 | 126.297–127.296 |
| [SEND + MORE = MONEY](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/send-money/send-money.lp) | 40.301–40.653 | 40.719–41.735 | 13.050–14.131 |
| [Task allocation: scheduling](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-04/05-larger-mix.lp) | 125.165–126.981 | 128.198–129.724 | 184.024–192.129 |
| [Shortest path: layered DAG](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-01/06-layered-dag.lp) | 7.933–7.936 | 7.939–8.003 | 6.567–6.598 |

Separate child peak-RSS observations use three fresh runs per block, giving
six observations per native revision/case and twelve for clingo. The ranges
again span block medians. The `RUSAGE_CHILDREN` scope described above applies:
these are neither simultaneous process-tree RSS nor GPU memory.

| Case | zetesis `3afaf719`, MiB | zetesis `0d287734`, MiB | Direct clingo, MiB |
| --- | ---: | ---: | ---: |
| Queens 2, N=8 | 12.969–13.031 | 13.266–13.281 | 9.328–10.406 |
| SEND + MORE = MONEY | 26.031–26.203 | 26.297–26.391 | 8.469–8.734 |
| Task allocation: scheduling | 40.281–40.359 | 40.484–40.562 | 21.625–22.188 |
| Shortest path: layered DAG | 12.875 | 13.188–13.203 | 5.641–5.766 |

The `0d287734` wall and peak-RSS medians are slightly higher in all four cases.
Task allocation is a possible regression signal, but the unchanged clingo
executable also took longer in the middle blocks: 189.656–192.129 ms versus
184.024–184.225 ms at the endpoints. The two `0d287734` zetesis diagnostic solving
intervals for task allocation were 107.064 and 113.013 ms, versus 105.010 and
104.685 ms for `3afaf719`; those separate statistics runs do not provide a
phase-time distribution for the timed samples. Shared block variation prevents
attributing the whole difference to code changes. This sample establishes
neither a broad speedup nor a general absence of regressions, and does not
measure lazy or GPU solving.

Reproduce each block with the same manifest and a new report path:

```sh
zetesis-perf examples/correctness \
  --case standalone/n-queens/variant-02.lp \
  --case standalone/send-money/send-money.lp \
  --case scenarios/task-allocation/variant-04/05-larger-mix.lp \
  --case scenarios/shortest-path/variant-01/06-layered-dag.lp \
  --zetesis /path/to/zetesis --clingo /path/to/direct/clingo \
  --warmups 2 --repetitions 5 --memory-runs 3 \
  --timeout-seconds 30 --campaign-seconds 180 \
  --sample-bytes 4194304 --capture-bytes 134217728 --report-bytes 536870912 \
  --report target/four-case-cpu.json
```

Use the same runner and direct clingo executable for all four blocks, in
`3afaf719 / 0d287734 / 0d287734 / 3afaf719` order. Finish and check each invocation
before starting the next, with competing builds and measurements stopped.
The manifest SHA-256 remains
`b43df1adf17ae0c035f1e310a5c15345c26cbcad8b59596932627c46fd1c6958`.
The following instrumented queens and complete Metal results retain their
September 10 builds and separate measurement populations.

### Instrumented queens limits

The same executables were compared on all six queens encodings at N=8 and N=10
using the [parameterized workload API](#compare-a-parameterized-workload).
Each report has 60 positions: four eager/lazy × one/four-worker native profiles
and one clingo profile, qualification followed by one timed round, with no
warmups. The profiles use `--oracle auto`, batch size 64, 256 MiB completion
scratch and one clingo worker. Input derivation preserves the source closure
and records each workload's identity.

| N | Native passes | Reference passes | Native refusals | Native incomplete | Capture stops | Skipped positions |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 8 | 24 | 12 | 12 | 0 | 0 | 12 |
| 10 | 12 | 12 | 12 | 2 | 4 | 18 |

The two revisions have the same disposition populations. At N=8, every eager
profile completes 92 models. The 24 paired passing native positions agree in
complete typed atom families, model multiplicities, shown records and
priority/cost vectors; 24 native positions remain unavailable for comparison.
At N=10, eager variants 01, 03 and 04 complete 724 models, with equal complete
native records in all 12 paired passing positions. The other 36 native
positions remain unavailable. Comparisons with clingo concern displayed-model
multisets, costs and optimum ties; hidden clingo interpretations are unavailable.

Explicit lazy execution was unsupported for these six formula encodings in the
measured revisions; these records do not qualify the later hybrid profile.
At N=10, variant 02 reaches the native countermodel work ceiling before
publishing an answer. Variants 05/06 reach the measurement harness's 4 MiB
capture ceiling; that does not establish a solver scalability limit. A larger
capture allowance also needs an appropriate independent native-decoder budget.
No failed or skipped sample is replaced.

Native JSON retains hidden atoms and includes statistics. At N=8, variants
05/06 produce about 2.04 MB per invocation, versus about 224 KB for the other
variants. Output accounts for about 40 ms of their approximately 52 ms
single-worker wall interval. These instrumented times belong to a different
population from the ordinary CLI table. One timed sample per cell does not
establish a performance trend, and no RSS observation is inferred from it.

### Complete Metal solves

The same old/current source and executable identities listed above were compared
on Apple M4 Pro using all six N=8 queens encodings. Four blocks ran in
old/current/current/old order. Each used four host and completion workers,
automatic oracle selection, batches of at most 64 candidates and one clingo
worker. Qualification and one warmup preceded five timed rounds per block.

All 504 positions were accounted for: 168 complete eager Metal solves, 168
passing references, 24 explicit lazy refusals and 144 subsequent skipped
positions. All 84 old/current pairs of complete native results agree on full
typed atom families, costs, shown values and multiplicities; each enumerates
92 answer sets. Reference agreement covers displayed results, not hidden
clingo atoms. Lazy requests select an unsupported countermodel/grounder
combination for these encodings; they are not completed solves or evidence
about the separate lazy closure path.

Every complete native solve reports the Apple M4 Pro Metal adapter, two GPU
batches, 92 GPU-decided candidates and no CPU residuals or pending results.
GPU work and accounted execution bytes are identical between revisions for
each encoding. This establishes device use, not GPU execution of grounding or
candidate generation.

Ranges span the two block medians per native revision and four clingo block
medians, each based on five timed observations. They are not confidence
intervals. Native runs include full typed JSON and statistics; clingo emits its
displayed-model JSON. These timings therefore differ from the ordinary
human-output CLI measurements above.

| Queens variant | Old Metal, ms | Current Metal, ms | clingo, ms |
| --- | ---: | ---: | ---: |
| 01 | 31.788–31.899 | 31.844–31.971 | 6.514–6.560 |
| 02 | 120.935–122.279 | 119.837–121.020 | 120.690–122.121 |
| 03 | 31.686–31.753 | 31.693–31.703 | 6.493–6.538 |
| 04 | 29.165–29.186 | 29.171–29.201 | 6.509–6.550 |
| 05 | 68.769–69.196 | 69.181–69.192 | 6.512–6.545 |
| 06 | 71.945–72.093 | 72.120–72.177 | 6.585–6.616 |

Whole-invocation times are broadly unchanged. Grounding medians for variants
05/06 rise by approximately 0.08–0.11 ms; no grounding-speed gain is established.
Current execution setup costs 8.334–8.491 ms. Variants 05/06 emit approximately
2.04 MB of JSON and spend 35.690–36.083 ms in output. Host GPU-oracle intervals
include work and waits; they are not kernel timings. These small workloads do
not establish a general GPU speedup or a lower memory footprint.

Reproduce the workload and execution configuration with the maintained matrix
CLI. Run once per executable in the stated four-block order, choosing a new
report path for each invocation:

```sh
zetesis-perf examples/correctness --suite queens \
  --profile metal-eager --profile metal-lazy \
  --workers 4 --completion-workers 4 --clingo-workers 1 --batch-size 64 \
  --zetesis /path/to/zetesis --clingo /path/to/clingo \
  --warmups 1 --repetitions 5 \
  --timeout-seconds 30 --campaign-seconds 180 \
  --sample-bytes 4194304 --native-report-bytes 8388608 \
  --capture-bytes 134217728 --report-bytes 536870912 \
  --report target/queens-metal.json
```

The default completion scratch ceiling is 256 MiB. Preserve refused and skipped
cells when reviewing the report. The measurements used the maintained
`performance::matrix::run_workloads` API with unchanged N=8 sources; the command
above selects the same inputs, profiles and schedule through the base-corpus
report view. Raw report schemas need not be identical.
