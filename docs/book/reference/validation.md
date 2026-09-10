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
