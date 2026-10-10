# Validating an implementation change

This guide is for contributors checking a change to zetesis. Start with the
behavior the change must preserve: answer sets, objective ordering, resource
limits or a representation invariant. Choose tests that can detect a violation.

For routine use, see the installed [test commands](commands.md#check-conformance).
For timing and memory comparisons, start with the
[benchmark guide](performance.md). Those commands serve different purposes from
the repository checks below.

## Run the independent checks

Run from the checkout root, after installing the tools in the next section.

| Command | Checks |
| --- | --- |
| `scripts/check.sh portable` | Rust tests, formatting, strict lint, documentation and benchmark correctness, including the maintained standalone packages. |
| `scripts/check.sh oracle` | External clingo comparisons. |
| `scripts/check.sh proofs` | Lean build, axiom audit and proof-record consistency. |
| `scripts/check.sh coverage` | Separate workspace and CPU-only line-coverage floors. |
| `scripts/check.sh book` | Manual build and checked Rust examples. |
| `scripts/check.sh hardware` | The named physical-device tests on the host backend. |

The [contributor guide](https://github.com/GregoryGelfond/zetesis/blob/main/CONTRIBUTING.md#verification-and-review)
defines the required checks. Run them against the exact source you intend to
qualify. Keep each checkout's build directory separate, and retain the commands,
logs and source identity. Passing tests from another build does not qualify a
new executable.

CPU tests do not qualify a GPU. The installed `test backend` command checks three
known answer families; it does not replace the full hardware gate. Likewise,
a proof build establishes its stated mathematical results, not correctness of
the Rust implementation or shaders.

## Prepare verification tools

The shell checks target macOS and Linux. Install Git and
[Rust's native prerequisites](https://doc.rust-lang.org/book/ch01-01-installation.html),
including a linker. The [installation guide](https://github.com/GregoryGelfond/zetesis/blob/main/INSTALL.md)
covers source access and the installed tools.

### Rust, coverage and documentation

Install the pinned tools once:

```sh
rustup toolchain install 1.97.1 --profile minimal --component rustfmt,clippy,llvm-tools-preview
cargo +1.97.1 install --locked --version '=0.8.7' cargo-llvm-cov
cargo +1.97.1 install --locked --version '=0.5.4' mdbook
```

Put the Cargo binary directory, normally `~/.cargo/bin`, on `PATH`.
Coverage requires cargo-llvm-cov 0.8.7 and the book requires mdBook 0.5.4.
The `llvm-tools-preview` component supplies the matching `llvm-cov` and
`llvm-profdata`; rustup may list the component as `llvm-tools`.

By default, coverage finds those executables under
`$(rustc +1.97.1 --print sysroot)/lib/rustlib/<host>/bin`, deriving the host from
`rustc +1.97.1 -vV`. Tools installed for another toolchain are not used.
To supply external LLVM tools, set **both** `LLVM_COV` and `LLVM_PROFDATA` to
a compatible pair. Setting only one is refused. The report records the selected
paths and versions; an arbitrary system LLVM installation is not a compatible
profile reader.

### Lean

Install [elan](https://github.com/leanprover/elan#installation), put its
`elan`, `lean` and `lake` commands on `PATH`, and fetch the version in
`proofs/lean-toolchain`:

```sh
elan toolchain install leanprover/lean4:v4.33.1
```

The proof gate runs Lake from `proofs`, where the checked-in toolchain applies.

### Clingo

Oracle checks require clingo 5.8.2. Set its absolute executable path in
`CLINGO` and select the same installation through `PATH`:

```sh
export CLINGO=/absolute/path/to/clingo
export PATH="$(dirname "$CLINGO"):$PATH"
```

The gate verifies the version and that both names identify the same executable.
Its 15 independent Cargo campaigns retain arguments, exit statuses and the
harness's report of the tests each ran under a fresh `target/oracle-checks/run.*`
directory. A test failure does not skip later campaigns; setup or receipt-write
failures stop the run. After the campaigns, the gate requires each to have run
at least one test, and exactly the ignored tests its filters select among the
sources. The gate returns the first failed campaign's status, or that check's.
Capture stdout and stderr with the command log.

The script lists each campaign's selection explicitly: its test targets and,
within a crate's `integration` target, test-name filters naming the modules it
runs. zetesis-maintenance's portable `ignored_tests` check refuses an ignore
whose reason names no resource, a `requires clingo:` test that no campaign
selects, and a campaign whose selection contains no such test.

### Check tool selection

```sh
rustc +1.97.1 -vV
cargo +1.97.1 llvm-cov --version
mdbook --version
(cd proofs && lake --version)
clingo --version
```

Initial tool and dependency installation needs network access.

## Keep semantic expectations independent

For a language change, write a small ASP fixture with an independently derived
answer family. Include relevant boundary cases: empty extensions, shared atoms
or tuples, self-support and nested negation. External comparisons should use
the original source. Comparing a rewritten program cannot by itself establish
support for the original construct.

A lowering test may need to check satisfaction both in the candidate and in a
subset of its frozen reduct. A fast path needs an applicability test and an
assertion that the intended route ran. Keep those implementation checks separate
from an already-proved formula identity.

Native admission policy need not match an external parser's diagnostics.
Zetesis rejects distinct lexical include paths resolving to the same source,
but accepts a repeated identical path. Clingo 5.8.2 has rejected a lexical-alias
fixture on macOS and accepted it with an already-included warning on Linux.
The native test therefore checks the typed refusal and provenance; oracle tests
compare admitted include graphs. A version number alone does not establish a
portable diagnostic contract.

The maintained source collections provide broader regressions. `zetesis-corpus`
is a developer tool and is not installed; build it with
`cargo build --locked --release -p zetesis-validation` and run it from
`target/release`:

```sh
zetesis-corpus verify-examples examples/correctness
zetesis-corpus verify validation/upstream/clingo-5.8.2/curated
zetesis test corpus --repo . --report target/corpus-parity.json
zetesis-corpus compare validation/upstream/clingo-5.8.2/curated \
  --clingo /path/to/clingo --zetesis /path/to/zetesis \
  --report target/selected-parity.json
```

The two `verify` commands check retained sources and contracts without running
a solver. Curated upstream fixtures retain assertion excerpts, immutable source
references and licenses. Their loader checks decoded ASP and expected output
against those excerpts; it does not fetch or reauthenticate the complete upstream
C++ files.

| Evidence | What it establishes |
| --- | --- |
| correctness comparison | Agreement of completed displayed-model multisets, costs, optimum ties and declared corpus contracts. Hidden atoms cannot be reconstructed from `#show`. |
| Selected upstream comparison | Full-model agreement for curated fixtures whose contracts exclude projection and objectives. |
| Lean build and axiom audit | Kernel acceptance of the stated laws under the recorded axiom boundary. |
| Proof-record check | Consistency of retained sources, theorem locations, audit output, command records and hashes. It does not establish that commands executed. |
| Physical backend tests | Results and accounting for the named cases on the observed adapter. |

Refused input, incomplete enumeration, resource exhaustion, malformed output
and semantic disagreement are different outcomes. Inspect the typed decision
or versioned report; a successful process alone does not establish agreement.

The reusable tools also have distinct roles: `zetesis_validation` owns
reported-answer comparison, corpus contracts and bounded process capture;
`zetesis_maintenance` owns repository policy and proof-record checks.
Neither participates in production answer-set search.

## Read coverage and hardware reports

The hardware gate selects Metal on macOS and Vulkan elsewhere. Override the
selection with `scripts/check.sh hardware --metal` or `--vulkan`.
Each backend has a reviewed selection of 63 exact tests in 14 groups.
This includes complete terminal-definition reconstruction over device-verified
base answers, compared with eager CPU answer sets and the original output queries.
Logs and status files are retained under `target/hardware`.

To include a backend's physical tests in coverage, run:

```sh
scripts/check.sh coverage --metal
scripts/check.sh coverage --vulkan
```

Each requires an available adapter of its backend, and each reads the reviewed
selection the hardware gate reads. The selection includes static-oracle
construction and closure against an independent ordered-set reference, tight and
general formula checking, resource refusal, reusable sessions, and completed
table joins composed with GPU checking. Formula tests do not replace the
[static shader tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/tests/integration/hardware.rs).
The [0.5.0 coverage snapshot](coverage-0.5.0.md) records the selected Metal tests
within workspace coverage. The [0.3.0 snapshot](coverage-0.3.0.md) retains its
earlier source and separate physical qualification.

Coverage always has two separately instrumented populations:

| Population | Report directory |
| --- | --- |
| Workspace, portable tests; also the named physical tests with `--metal` or `--vulkan` | `target/coverage/workspace` |
| CPU-only `zetesis-solve`, `zetesis-engine`, `zetesis` and `zetesis-cli` | `target/coverage/cli-cpu` |

The historical directory name `cli-cpu` includes all four crates. The public
Rust backend and facade therefore receive the same independent CPU-only check
as the native solver and command-line interface. A portable-only
workspace report and a combined Metal report have different test populations;
their instrumented lines may differ too. Compare like populations on the same
source. Neither replaces the CPU-only check.

After both reports are written, the gate checks both independent 91% floors.
It records their statuses in `target/coverage/floors.tsv`; a failed workspace
floor does not skip the CPU-only check. `target/coverage/status.txt` stays
`incomplete` unless both pass. Setup, test or report-generation failures stop
before those floor checks. Keep the command log to distinguish these outcomes.

The coverage command clears each instrumented build directory before rebuilding
it, including obsolete test executables. It retains the ordinary build directory.
Use fresh instrumentation for the source under review. Matching executable
names do not prove matching builds. Coverage does not measure assertion strength
or replace review.

## Validate performance separately

A performance claim needs matched programs, answer requests, binaries, execution
settings and limits. Keep failed and incomplete runs in the comparison. Report
work, elapsed time and memory separately; less work does not by itself mean a
faster solve.

Start with [Benchmarks and comparisons](performance.md). The
[detailed measurement protocols](measurement-protocols.md) cover parameterized
workloads, fixed series and reproduction of early observations. Keep durable
fixtures, source attribution and runnable checks with the code; private
development records should not be needed to understand a public claim.

## Coverage

The README badge reports the **portable workspace** line coverage from the most
recently qualified source identified below. It is a recorded local measurement,
not a live hosted-CI status. It does not use the combined portable-plus-Metal
percentage. A release version does not change the source identity or test
population of that measurement.
The [coverage and Metal qualification record](coverage-0.5.0.md) holds the exact
counts, source-content manifest, qualification scope and reproduction command.

Update the badge and the snapshot record together only after qualification
completes. Line coverage identifies executed Rust lines; it does not establish
assertion strength, WGSL instruction coverage or formal correctness.
