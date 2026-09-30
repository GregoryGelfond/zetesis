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
Its 15 independent Cargo campaigns retain arguments and exit statuses under a
fresh `target/oracle-checks/run.*` directory. A test failure does not skip later
campaigns; setup or receipt-write failures stop the run. The gate returns the
first failed campaign's status. Capture stdout and stderr with the command log.

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
Each backend has a reviewed selection of 58 exact tests in 14 groups.
This includes complete terminal-definition reconstruction over device-verified
base answers, compared with eager CPU answer sets and the original output queries.
Logs and status files are retained under `target/hardware`.

To include the physical Metal tests in coverage, run:

```sh
scripts/check.sh coverage --metal
```

This requires an available Metal adapter. The selection includes static-oracle
construction and closure against an independent ordered-set reference, tight and
general formula checking, resource refusal, reusable sessions, and completed
table joins composed with GPU checking. Formula tests do not replace the
[static shader tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/tests/integration/hardware.rs).
The coverage snapshot below describes the tests qualified on its stated source;
today's required selection does not update that snapshot.

Coverage always has two separately instrumented populations:

| Population | Report directory |
| --- | --- |
| Workspace, portable tests; also the named physical tests with `--metal` | `target/coverage/workspace` |
| CPU-only `zetesis-solve` and `zetesis-cli` | `target/coverage/cli-cpu` |

The historical directory name `cli-cpu` includes both crates. A portable-only
workspace report and a combined Metal report have different test populations;
their instrumented lines may differ too. Compare like populations on the same
source. Neither replaces the CPU-only check.

After both reports are written, the gate checks both independent 91% floors.
It records their statuses in `target/coverage/floors.tsv`; a failed workspace
floor does not skip the CPU-only check. `target/coverage/status.txt` stays
`incomplete` unless both pass. Setup, test or report-generation failures stop
before those floor checks. Keep the command log to distinguish these outcomes.

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

The README badge reports workspace line coverage from the most recently
qualified source below. It is a recorded local measurement, not a live hosted-CI
status. A newer source remains unqualified until its own checks complete.

| Population | Covered / instrumented lines | Coverage |
| --- | ---: | ---: |
| Workspace, all features, portable tests | 108,128 / 117,927 | 91.69% |
| CPU-only solver library and CLI, separate instrumentation | 9,666 / 10,549 | 91.63% |

This version `0.1.5` snapshot measures compiled source
[`318c8238`](https://github.com/GregoryGelfond/zetesis/tree/318c8238ad72719deee63f9b5250d3e1bbce565b)
using Rust 1.97.1, cargo-llvm-cov 0.8.7 and LLVM 22.1.6 on macOS 26.6.2.
The [verification receipt](observations/coverage-318c8238.json) retains exact
counts, source identities, report hashes and qualification scope. Documentation
updates do not change the measured source. The
[canonical-storage comparison](canonical-storage.md) records performance
separately; its measured executable is byte-identical to the qualified release.

Both populations passed their independent 91% floor. The workspace contains
2,433 portable profiles; the CPU-only population contains 342 profiles.
No physical profiles were imported into either population.

Sixty tests in 16 groups passed separately on Apple M4 Pro Metal at
[`3b06e479`](https://github.com/GregoryGelfond/zetesis/tree/3b06e4795b3a00cd0fbf11d3e0f4f2c32750cf2c).
That revision changes only an ignored table-join test to request eager grounding
explicitly; automatic grounding can instead reconstruct terminal definitions.
All assertions remain, and production sources and the release executable are
unchanged. The ordinary `zetesis test backend --device metal` command also
passed its three complete-family checks with actual device work. These runs
were not instrumented and do not contribute to the coverage badge.

The portable, external-oracle, manual and coverage gates passed at `318c8238`.
The corrected test also passed formatting, its related CPU tests and strict
Clippy before device qualification. The Lean 4.33.1 build, axiom audit and
source-record checks cover 147 semantic modules and 1,348 audited theorems.
Their source hashes are recorded in the
[verification record](https://github.com/GregoryGelfond/zetesis/blob/3b06e4795b3a00cd0fbf11d3e0f4f2c32750cf2c/proofs/verification.json).
These counts describe the checked
mathematical library, not verification of the Rust grounder, masks or GPU
execution. Historical corpus and performance results retain their original
source identities in the [grounding comparison](grounding-measurements.md).

The static closure comparison checked 53 candidate executions against an
independent ordered-set reference. Owned seeds, indexed selections and manual
selections share that reference, including reused epochs and the 4,096-atom
boundary. Each of the four tight-oracle physical tests exercises both Atomic
and Grouped support construction. The relation tests cover typed equality masks and
prepared-view refusals.
The shared-context tests cover formula execution while relation columns remain
prepared, non-destructive contention refusal and failure propagation to peers.
They check reuse after healthy, settled preparation cancellation, as well as
caller control during static and formula execution. Ordinary-session tests
check exact context identity across explicit GPU eager/lazy closure and formula
setup. Automatic execution remains on CPU, including when the caller supplies
GPU resources. Repeated sessions preserve
independent subjects, budgets, costs and outcomes while sharing the device;
policy and observer refusals preserve later reuse.
The formula-session tests distinguish tight support from general reduct checking
using checked plan preconditions. They verify automatic tight dispatch, general
checking for a non-tight positive cycle, and a finite tight-work refusal before
device submission. An unseeded positive cycle grounds to the empty theory and
therefore exercises tight checking; source recursion alone does not determine
the ground theory's plan.
Compiled-profile tests check exact pipeline identity across fresh formula oracles
and ordinary library sessions, including device health and contention boundaries.
Builder collection tests use caller-owned resources and retain incomplete results
without claiming a complete `WorldView`. The independent CPU population covers
both the composed solver and its CLI consumer after their crate separation.
Explicit lazy execution checks preserve source grounding, immutable-upload reuse
and complete candidate accounting. Formula checks exercise dependency-level
original truth, packed auxiliary domains, strict-subset reduction and actual
submission receipts under finite device limits.
Combined language-consumer tests preserve complete answer-set families, scored
observations and all optimum ties across aggregate heads, objectives and output
queries. They require actual GPU work and exact accounting of CPU residuals.
The ordinary table-join case requires positive table preparation, probe and row
counts, actual GPU candidates, exact decided/residual accounting and no pending
results. Its complete Metal family equals the independent CPU family. It checks
host table grounding composed with GPU reduct checking, not a GPU table kernel.
Other devices are outside this measurement; the hardware gate qualifies the
Vulkan selection on a host exposing a Vulkan adapter.

Reproduce the portable snapshot with `scripts/check.sh coverage` at `318c8238`
using the [verification tools](#prepare-verification-tools). At `3b06e479`, run
`scripts/check.sh hardware --metal` for the 60 device tests and
`zetesis test backend --device metal --json` for the three CLI checks.
Retain the coverage JSON and HTML reports and the hardware logs separately.
Running `scripts/check.sh coverage --metal` creates a different, combined
measurement; it is not the population reported above.
Update the badge and this table together only after qualification completes.
Line coverage identifies executed Rust lines; it does not establish assertion
strength, WGSL instruction coverage or formal correctness.
