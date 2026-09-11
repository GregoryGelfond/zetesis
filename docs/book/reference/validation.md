# Validating an implementation change

Begin with a proposition about the original program: its answer sets, its
objective ordering, or a preserved relationship between representations. A
passing process, a matching display and a verified reduct establish different
things. Choose checks that observe the proposition you intend to claim.

The tools share Rust library APIs. `zetesis_validation` provides
reported-answer comparison, corpus contracts and bounded process capture.
`zetesis_maintenance` owns repository assurance policy and proof-record
consistency. Neither participates in production answer-set search.

## Prepare verification tools

The maintained shell checks target macOS and Linux. Begin with
[Rust's installation prerequisites](https://doc.rust-lang.org/book/ch01-01-installation.html),
including a native linker, and Git. The
[checkout installation guide](https://github.com/GregoryGelfond/zetesis/blob/main/README.md#install-and-run)
covers source access and the installed zetesis commands used below.

The coverage and book gates require additional pinned tools. From the checkout
root, install them once:

```sh
rustup toolchain install 1.97.1 --profile minimal --component rustfmt,clippy,llvm-tools-preview
cargo +1.97.1 install --locked --version '=0.8.7' cargo-llvm-cov
cargo +1.97.1 install --locked --version '=0.5.4' mdbook
```

Ensure the Cargo binary directory is on `PATH` (normally `~/.cargo/bin`).
`scripts/coverage.sh` requires cargo-llvm-cov 0.8.7; the book gate requires mdBook
0.5.4. The `llvm-tools-preview` component supplies `llvm-cov` and `llvm-profdata`
for the pinned Rust compiler; rustup may display it as `llvm-tools` in its
component listing. Installing those tools for a different Rust toolchain does
not populate this toolchain's directory.

By default, the coverage script discovers both LLVM executables under
`$(rustc +1.97.1 --print sysroot)/lib/rustlib/<host>/bin`, deriving `<host>` from
`rustc +1.97.1 -vV`. If you intentionally supply external tools, set **both**
`LLVM_COV` and `LLVM_PROFDATA` to a compatible pair. Setting only one is refused;
an arbitrary system LLVM installation is not an interchangeable profile reader.
The report records the selected tool paths and their versions.

For proofs, install [elan using its upstream instructions](https://github.com/leanprover/elan#installation)
and make its `elan`, `lean` and `lake` commands available on `PATH`. Fetch the
Lean version declared by `proofs/lean-toolchain`:

```sh
elan toolchain install leanprover/lean4:v4.33.1
```

The proof gate enters `proofs` before invoking Lake, so the checked-in toolchain
selection applies. External oracle checks additionally require **clingo 5.8.2**
as `clingo` on `PATH`; setting `CLINGO` alone does not configure every test.
The comparison commands also accept explicit executable paths.

Confirm the tools selected by your shell before running the gates:

```sh
rustc +1.97.1 -vV
cargo +1.97.1 llvm-cov --version
mdbook --version
(cd proofs && lake --version)
clingo --version
```

Initial tool and dependency installation needs network access. After installation,
run the checks below against the exact checkout you intend to qualify.

## Keep semantic expectations independent

For a new language feature, retain a small ASP fixture with an independently
derived expected answer family. Include the cases that distinguish its meaning:
empty extensions, shared atoms or tuples, self-support, and nested negation where
applicable. Use the original source in external clingo comparisons. A rewritten
input can test a proposed equivalence, but cannot by itself establish support
for the original construct.

Tests of lowering should inspect satisfaction in an interpretation and in a
subset of the frozen candidate where the claimed law requires both. Keep source
correspondence separate from an already-proved formula identity. A new fast path
also needs an applicability check and evidence that the intended route executed.

The maintained collections provide broader regressions:

```sh
zetesis-corpus verify-examples examples/kr-domains
zetesis-corpus verify validation/upstream/clingo-5.8.2/curated
zetesis-validate --repo . --report target/corpus-parity.json
zetesis-corpus compare validation/upstream/clingo-5.8.2/curated \
  --clingo /path/to/clingo --zetesis /path/to/zetesis \
  --report target/selected-parity.json
```

The two `verify` operations check retained source and contract integrity without
running a solver. The curated upstream fixtures retain assertion excerpts,
immutable source references and licenses. Their loader checks the excerpts
against decoded ASP and expected output; it does not fetch or reauthenticate
the complete upstream C++ files.

| Evidence | What it can establish |
| --- | --- |
| kr-domains comparison | Agreement of completed displayed-model multisets, costs, optimum ties and the declared corpus contracts. Atoms hidden by `#show` cannot be reconstructed. |
| Selected upstream comparison | Full-model agreement for the curated fixtures whose contracts exclude projection and objectives. |
| Lean build and axiom audit | Kernel acceptance of the stated mathematical laws under the recorded axiom boundary. |
| Proof-record check | Consistency of retained sources, theorem locations, audit output, command records and hashes. It does not establish that commands executed. |
| Physical backend tests | Correctness and accounting for the named cases on the observed adapter. CPU tests do not qualify a GPU. |

Refused input, incomplete enumeration, exhausted resource allowances, malformed
output and semantic disagreement must remain distinguishable. Report formats
are versioned; compare like fields within their documented contracts rather
than interpreting every successful command as a semantic pass.

## Measure the relevant work

Use `zetesis-perf` for matched end-to-end comparisons and the
[comparison guide](https://github.com/GregoryGelfond/zetesis/blob/main/scripts/README-comparison.md) for its schedules,
capture bounds and report formats. Direct wall time includes process startup,
source loading, grounding, solving and captured output. The CPU baseline keeps
instrumented phase observations in separate samples. The explicit profile matrix
includes JSON/statistics in its timed native runs and forms a separate
population. Keep the selected inputs, binaries, backend,
worker counts, limits and warmup schedule with each result.

Repeated `--case` arguments select unchanged cases from the sealed corpus.
`--memory-runs` adds a separate population of fresh-child resource observations
on macOS or Linux; it does not add samples to the wall-time distribution. Its
reported child peak RSS excludes the measuring helper and is neither simultaneous
process-tree memory nor GPU memory. These selected/resource campaigns use their
own versioned report view.

For example, compare three different encodings with complete CPU/eager solves:

```sh
zetesis-perf examples/kr-domains \
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

### Compare a parameterized workload

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

target/debug/examples/book-workload examples/kr-domains \
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

### Performance evidence

Storage contracts and elapsed time are separate results. Formula joins reuse
one cleared expression workspace across prefix checks, generators and final
filters. Packed GPU support reduces the support buffer to one bit per atom.
Neither change establishes a general latency improvement or lower process RSS.

The following measurements compare
[`f1c6365a`](https://github.com/GregoryGelfond/zetesis/tree/f1c6365af66a56902d985fb3f61f584c860527de)
with [`e7e5e410`](https://github.com/GregoryGelfond/zetesis/tree/e7e5e410d4072457eee4a469b600981d713f5d15),
using uninstrumented Rust 1.97.1 release builds on Apple M4 Pro on 10 September
2026. Both sources already use packed Atomic support. The order is
before/current/current/before; unchanged-executable drift remains part of the
result. These are descriptive screens, not statistical confidence estimates.

#### Ordinary CPU/eager solving

The 13 selected kr-domains cases include all six queens encodings, SEND and
shortest-path/task-allocation cases. Each block uses one closure worker, one
completion worker, `--oracle auto` and `--models 0`. Each case has one
qualification pair, one warmup pair, five timed native/clingo pairs and a
separate native statistics observation. Completed displayed results, costs and
optimum ties agree with clingo 5.8.2. This comparison does not reconstruct hidden
atoms from displayed output.

The table reports the median of ten whole-process native observations per
executable and case. Before drift is the last before-block median relative to
the first. Positive change means more elapsed time.

| Case | Before, ms | Current, ms | Change | Before drift |
| --- | ---: | ---: | ---: | ---: |
| [Queens 1](../../../examples/kr-domains/standalone/n-queens/variant-01.lp) | 9.194 | 9.321 | +1.38% | −0.15% |
| [Queens 2](../../../examples/kr-domains/standalone/n-queens/variant-02.lp) | 91.299 | 91.346 | +0.05% | +3.03% |
| [Queens 3](../../../examples/kr-domains/standalone/n-queens/variant-03.lp) | 9.144 | 9.120 | −0.26% | −0.48% |
| [Queens 4](../../../examples/kr-domains/standalone/n-queens/variant-04.lp) | 7.866 | 7.840 | −0.33% | −0.08% |
| [Queens 5](../../../examples/kr-domains/standalone/n-queens/variant-05.lp) | 11.592 | 11.612 | +0.17% | −0.35% |
| [Queens 6](../../../examples/kr-domains/standalone/n-queens/variant-06.lp) | 11.630 | 11.601 | −0.25% | +0.11% |
| [SEND + MORE = MONEY](../../../examples/kr-domains/standalone/send-money/send-money.lp) | 38.584 | 39.255 | +1.74% | +3.76% |
| [Shortest path: cycles](../../../examples/kr-domains/scenarios/shortest-path/variant-01/07-cycles.lp) | 5.366 | 5.364 | −0.03% | −0.19% |
| [Shortest path: ordering and cap](../../../examples/kr-domains/scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp) | 9.081 | 9.043 | −0.42% | +13.49% |
| [Shortest path: unsatisfiable budget](../../../examples/kr-domains/scenarios/shortest-path/variant-03/05-budget-unsat.lp) | 5.358 | 5.379 | +0.41% | +1.73% |
| [Task allocation: larger mix](../../../examples/kr-domains/scenarios/task-allocation/variant-01/05-larger-mix.lp) | 5.311 | 5.314 | +0.07% | −0.26% |
| [Task allocation: makespan tie](../../../examples/kr-domains/scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp) | 5.323 | 5.322 | −0.02% | −1.08% |
| [Task allocation: scheduling](../../../examples/kr-domains/scenarios/task-allocation/variant-04/05-larger-mix.lp) | 127.127 | 127.651 | +0.41% | +2.66% |

Per-case changes range from −0.42% to +1.74%, with larger unchanged-baseline
drift in several cases. The measurements do not isolate a timing benefit from
workspace reuse. Reproduce the selected population with `zetesis-perf`, passing
the linked case paths relative to `examples/kr-domains` with `--case`,
`--warmups 1`, `--repetitions 5`,
`--memory-runs 0`, `--timeout-seconds 10` and `--campaign-seconds 120`.
Run the two frozen executables in the stated four-block order and retain each
report separately. Statistics observations are outside the timed population.

#### Tight GPU membership

The [tight-oracle benchmark](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-experiments/README.md)
compares identical candidates through scalar, Rayon, fresh-device and
resident-device routes. Classification includes transfer and result decoding;
whole-query time also includes exact CPU completion of residuals. Source
grounding and outer candidate search are outside this experiment. The
[execution chapter](../architecture/execution.md) states the support-buffer
size independently of elapsed time and total device memory.

The Atomic comparison uses the same normal/choice fixtures, with 4, 32, 33 and
256 atoms and batches of 3 or 128 candidates:

```sh
zetesis-bench tight --backend metal --atoms 4,32,33,256 --batches 3,128 \
  --families normal,choices --workers 4 --warmups 2 --repetitions 8 \
  --max-work 100000000
```

Each of the four blocks retains 704 observations, including 512 timed
observations. Complete ordered subjects, certificate witnesses, final decisions
and device work/storage accounting agree across blocks. All 1,408 GPU batches
complete, accounting for 92,224 candidate occurrences. Exact residual checking
remains on the CPU.

For each case, compare the average of the two current block medians with the
average of the two before block medians. The table summarizes those 16 ratios
with a geometric mean; values below one mean less elapsed time.

| GPU route | Classification ratio | Whole-call ratio | Whole-call case range | Before final/initial drift range |
| --- | ---: | ---: | --- | --- |
| Fresh resources | 1.008 | 1.007 | 0.811–1.528 | 0.514–1.443 |
| Resident resources | 1.011 | 1.002 | 0.821–1.313 | 0.490–1.381 |

Substantial same-executable drift prevents a firm speedup or regression
conclusion from this short screen. These intervals include host/device transfer
and waiting; they are not shader-only timings or ordinary solve times.

A separate 33-atom, three-candidate check uses
`--families support-uniform,support-skewed`, four workers, one warmup and four
repetitions with each of `--support atomic` and `--support grouped`. Both
policies complete 48 observations and agree with the full CPU reference,
including exact residual completion. Each theory retains 512 producer
occurrences. Grouped storage adds 12 graph bytes and eight temporary cursor
bytes, with unchanged transport and readback payloads. All producers occupy one
support word, so this check establishes neither useful occupancy nor a timing
advantage. Atomic remains the tight library default.

Neither the selected CPU cases nor these device primitives establish performance
across the complete corpus, general lazy grounding or every backend. Keep
complete distributions, unchanged-baseline drift and separate memory
observations with every comparison.

#### Typed relation selection

The shared relation primitive at
[`d871e91b`](https://github.com/GregoryGelfond/zetesis/tree/d871e91b56406c20b312e63f9d3437e6352803e2)
was measured separately on Apple M4 Pro using an uninstrumented Rust 1.97.1
release build on 10 September 2026. Each case used four Rayon workers, one
warmup and three timed repetitions. These operation medians include equality
selection and typed row reconstruction; source/view preparation is separate.

The table compares routes within each physical Metal invocation. CPU routes
retain the uploaded columns, and all routes use the same typed input and masks.

| Relation case | Rows / queries | Scalar (µs) | Rayon (µs) | Metal (µs) |
| --- | ---: | ---: | ---: | ---: |
| Independent, numeric values | 256 / 8 | 4.667 | 18.999 | 235.833 |
| Correlated, tuple values | 256 / 8 | 4.959 | 21.167 | 477.375 |
| Independent, numeric values | 4,096 / 32 | 206.375 | 139.376 | 732.584 |
| Skewed, tuple values | 4,096 / 32 | 252.457 | 152.166 | 663.666 |

All routes produced the same complete masks and reconstructed typed rows. Across
the four cases, 20 Metal batches completed all 400 queries. Rayon reduced the
operation median in the two larger cases; Metal did not beat the CPU routes in
this pilot. The fixed scalar/Rayon/Metal order and three repetitions make these
descriptive observations, not confidence estimates or evidence of faster program
grounding. The numeric 4,096-row Metal case includes a retained 2.806-ms sample;
the 256-row tuple Metal samples range from 0.222 to 0.521 ms.

Device and pipeline preparation took 9.549–44.942 ms, column upload
17.875–58.958 µs, and initial GPU operations 3.014–3.361 ms. Repeated operations
reused the immutable columns. In the two 4,096-row cases, common typed
reconstruction remained about 76–85 µs. These costs identify further work on
batching, transport and materialization; they do not establish a GPU speedup.
Separate CPU-only invocations are a distinct population and are not pooled into
this table.

Reproduce a row with the corresponding family, payload, row and query counts:

```sh
zetesis-bench relation --backend metal --family independent --payload numeric \
  --rows 4096 --queries 32 --workers 4 --warmups 1 --repetitions 3
```

Use `--family correlated --payload tuple --rows 256 --queries 8` or
`--family skewed --payload tuple --rows 4096 --queries 32` for the tuple rows.
The small numeric case uses `--family independent --payload numeric --rows 256
--queries 8` with the same worker and repetition settings. Use `--backend cpu`
for a separate scalar/Rayon invocation. The
[measurement contract](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-experiments/README.md#retained-relation-selection)
defines the preparation, transfer, reconstruction and authored-storage fields.
Eager formula support also uses this column view, with its own unchanged
shortest-posting selection and complete matcher. These isolated selection
measurements do not establish ordinary grounding performance or GPU grounding.

## Run the independent checks

The [contributing guide](https://github.com/GregoryGelfond/zetesis/blob/main/CONTRIBUTING.md#verification-and-review) defines
the maintained checks. From the repository root:

```sh
scripts/check.sh portable
scripts/check.sh oracle
scripts/check.sh proofs
scripts/check.sh coverage
scripts/check.sh book
```

Physical Metal qualification adds the named device tests with
`scripts/check.sh coverage --metal` on a machine exposing a Metal adapter.
Use fresh instrumentation for the source being qualified; matching executable
filenames do not establish matching builds. Coverage has independent workspace
and CPU-only populations and does not replace assertion review.

Keep durable fixtures, source attribution and runnable checks in the repository.
The manual describes current contracts. Private development history and
temporary campaign records are not prerequisites for reproducing a public claim.

## Coverage

The README badge reports workspace line coverage from the most recently
qualified source below. It is a recorded local measurement, not a live hosted-CI
status. A newer source remains unqualified until its own checks complete.

| Population | Covered / instrumented lines | Coverage |
| --- | ---: | ---: |
| Workspace, all features, portable tests plus 30 physical Metal tests | 46,198 / 49,420 | 93.48% |
| CPU-only CLI, separate instrumentation | 4,312 / 4,626 | 93.21% |

This snapshot was qualified on 10 September 2026 for
[`d871e91b`](https://github.com/GregoryGelfond/zetesis/tree/d871e91b56406c20b312e63f9d3437e6352803e2),
using Rust 1.97.1, cargo-llvm-cov 0.8.7 and LLVM 22.1.6 on macOS with Apple M4 Pro
Metal. Both populations passed their independent 91% floor. The workspace
combines its portable and physical profiles; the CPU-only population remains
separate. Each of the four tight-oracle physical tests exercises both Atomic
and Grouped support construction. The relation tests cover typed equality masks,
prepared-view refusals and matched scalar/Rayon/Metal measurement results.
Vulkan and other untested devices are outside this measurement.

Reproduce the populations with `scripts/check.sh coverage --metal` using the
[verification tools](#prepare-verification-tools). Retain the generated JSON and
HTML reports under `target/coverage/workspace` and `target/coverage/cli-cpu`.
Update the badge and this table together only after qualification completes.
Line coverage identifies executed Rust lines; it does not establish assertion
strength, WGSL instruction coverage or formal correctness.
