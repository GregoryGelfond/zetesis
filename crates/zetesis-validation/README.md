# zetesis-validation

Reusable corpus integrity checks, bounded process capture and reported-answer
comparison, with three command adapters:

| Command | Purpose |
|---|---|
| `zetesis-corpus` | Verify curated source collections or compare selected upstream cases. |
| `zetesis-validate` | Run the self-contained kr-domains regression collection against external clingo and zetesis. |
| `zetesis-perf` | Compare ordinary runs or measure a grounding/backend matrix. |

The production solver neither invokes nor depends on this validation executable.
clingo is an external reference. Reports record observations and their limits;
a process exit alone does not establish answer-set correctness.

See [Install and run](../../README.md#install-and-run) for installation and the
[contributing guide](../../CONTRIBUTING.md#verification-and-review) for gates.
Library entry points are documented in [src/lib.rs](src/lib.rs); generate their
reference with `cargo doc --locked -p zetesis-validation --no-deps --open`.

## Verify and compare source collections

From the repository root:

```sh
zetesis-corpus verify-examples examples/kr-domains
zetesis-corpus verify validation/upstream/clingo-5.8.2/curated
zetesis-validate --repo . --report target/validation-report.json
zetesis-validate --repo . --reference-only --report target/reference-report.json
```

The default kr-domains collection is self-contained and excludes clingcon.
Its manifest records original/cleaned hashes, typed contracts and annotation
removal provenance. Verification checks integrity; it does not execute a solver.
The solver receives ordinary ASP and does not interpret elenctic annotations.
Licenses and source provenance remain with the curated collections.

`--clingo` and `--zetesis` select executable paths or names resolved through
PATH. Relative executable paths resolve from the caller's working directory,
not the corpus. `--help` lists capture and native execution limits.
Omitting `--report` writes JSON to stdout; progress uses stderr.
Explicit `--corpus` or `--manifest` selects the original-source manifest mode.

The validator checks pinned source hashes before spawning children. Its default
corpus run executes those paths directly: keep them unchanged during execution.
It does not reserve paths or reseal them afterward. The selected and performance
libraries separately use private source copies and before/after identity checks.

Reports preserve exact arguments, source identities, actual reference version,
process outcomes, diagnostics, normalized answers and per-case decisions.
A clean reference-only run says nothing about native compatibility.
The command exits 0 when every case passes the requested mode, 1 for case failure,
and 2 for setup, integrity or report-publication errors.

## Understand answer parity

The kr-domains comparison checks complete displayed witness multisets, objective
costs, optimum ties, reported model counts and typed corpus contracts. It retains
duplicate symbols within a display and duplicate displays across distinct models.
It cannot reconstruct hidden atoms suppressed by `#show`, so it does not
establish arbitrary full hidden-model identity.

The clingo `optN` decoder reconciles witness and summary counts, removing exactly
the repeated final incumbent that accompanies the supported optimal enumeration
format. Unsupported or contradictory output is a failure, not a smaller answer
collection. Refusal, timeout, incomplete coverage, output limits, malformed
reports and semantic mismatch remain separate outcomes.

`answers::native_json::parse` reads the native schema-1 view with typed full
atoms, shown positions/terms, costs and terminal accounting.
`answers::clingo_json` and `native_text` provide bounded reported-display views.
These decoders validate a producer's claims; they do not independently prove
stability. JSON integer consumers need lossless values, and the current JSON
object decoder does not reject duplicate object keys.

The selected-corpus API can check full-model identity for fixtures whose source
contract excludes projection and objectives:

```sh
zetesis-corpus compare validation/upstream/clingo-5.8.2/curated \
  --clingo /path/to/clingo --zetesis /path/to/zetesis \
  --report target/upstream-parity.json
```

That stronger claim belongs to those selected sources, not every corpus program.
See [reported answers](src/answers.rs),
[native decoding](tests/native_json_answers.rs) and
[selected comparisons](src/selected.rs).

## Require actual GPU execution

```sh
zetesis-validate --repo . --native-backend metal \
  --native-oracle countermodel --native-batch-size 64 \
  --report target/metal-validation.json
```

An explicit physical backend with the countermodel oracle enables statistics
and requires matching hybrid-route evidence. Adapter/API identity, completed
GPU decisions, CPU residual checks, candidate accounting and declared allocation
limits must reconcile. A requested GPU or matching answers alone cannot qualify
physical execution.

Outer-search UNSAT can legitimately require no membership dispatch. Reports
distinguish that case from exercised GPU queries, and a physical campaign must
contain actual completed GPU membership work. No dummy device work is added.
Telemetry comes from the trusted native executable; it is not hardware
attestation, GPU kernel timing or process-memory measurement.

`--native-completion-workers` and
`--native-max-completion-scratch-bytes` pass unchanged to the solver.
The logical scratch allowance is separate from RSS and the default scalar CPU
cursor. A zero allowance can produce a recorded incomplete native run; the
validator does not raise it or silently substitute another route.
Current completion telemetry must reconcile those requested settings.
See [execution contracts](tests/support/execution_contracts.rs) and
[CLI controls](tests/cli_contracts.rs).

## Measure ordinary solves and execution matrices

```sh
zetesis-perf examples/kr-domains --zetesis /path/to/zetesis \
  --clingo /path/to/clingo --report target/cpu-comparison.json

zetesis-perf examples/kr-domains --suite corpus \
  --profile cpu-eager --profile cpu-lazy \
  --profile metal-eager --profile metal-lazy \
  --zetesis /path/to/zetesis --clingo /path/to/clingo \
  --report target/execution-matrix.json
```

The default ordinary comparison uses the maintained CPU baseline cases, not the
whole collection. It times uninstrumented processes after qualification;
separate statistics invocations are outside that timed population.
The matrix instead uses native `--json --stats --models 0`, so it measures
instrumented runs including typed output. Native full-atom JSON and clingo's
selected display output can differ in volume. These are different protocols,
not interchangeable timing populations.

`--suite queens` selects the maintained queens encodings;
`--suite corpus` selects the full clean collection. Repeated `--profile`
arguments select execution cells. Requested eager/lazy and CPU/GPU policies are
recorded alongside actual routes; unsupported combinations remain failures.
Native and reference worker settings are explicit and can differ.
Arrange a quiet measurement window and choose explicit invocation/campaign
deadlines appropriate to the suite. Input/executable seals do not capture
thermal state, dynamic libraries or the rest of the host environment.

Repeated `--case <manifest-relative.lp>` selects arbitrary runnable clean corpus
cases for the ordinary CPU campaign. Add `--memory-runs 5` for separate paired
child RSS observations; these never enter timed samples. The CLI seals its own
runner executable alongside the solvers and selected transitive source closure.
Full native help is captured when short help advertises it, under the same
capture limits, with no fallback after a failed full-help query.

Explicit selections and memory populations use ordinary report schema 2.
Named presets without those extensions retain schema 1. Completed schema-2
reports include separate wall-nanosecond and RSS-byte distributions, with exact
inclusive quartiles encoded as `whole + quarters / 4`. Failed campaigns retain
their evidence and have no accepted summary. Matrix report schemas are separate.

Every scheduled attempt is retained. A failed cell records skipped later
positions; samples are not silently replaced. `accounted` means every position
has a disposition, whereas `passed` requires all requested observations to pass.
Check both semantic qualification and sample scope before comparing timings.

Host phase timings and process wall time are separate. Lazy grounding is
interleaved with solving; a missing duration stays unavailable. Logical payload,
transfer and authored allocation counts are not RSS. Neither successful parity
nor reduced storage implies a speedup.

The reusable entry points are `performance::run(&Request)`,
`performance::run_with_runner(&Request, runner)` and
`performance::matrix::run(&Request)`. `Schedule::for_cases` validates bounded
explicit selections; `Schedule::with_memory` appends the separate population.
`run_with_runner` seals the supplied executable and uses it as a fresh helper
only for resource samples. They own bounded acquisition and separate
report publication. See [performance](src/performance.rs),
[matrix scheduling](tests/matrix_schedule.rs) and
[matrix accounting](tests/matrix_campaign.rs). The [comparison guide](../../scripts/README-comparison.md)
documents reproducible commands, limits and protocol boundaries.

## Compose capture, contracts and publication

| Library capability | Boundary |
|---|---|
| `examples::load`, `verify_originals`, `Contract::check` | Source integrity and declared expected outcomes; callers establish capture and producer completion. |
| `curated::open` | Bounded verified sources, provenance and immutable selected contracts. |
| `process::invoke` | Explicit executable/arguments/directory with bounded stdout/stderr capture and typed cleanup outcomes. |
| `process::invoke_supervised` | Separately supervise trusted helpers, retaining their group reservation through cleanup after failed helper completion. |
| `answers` | Bounded decoding and reconciliation of reported answers. |
| `selected::run` | Private source copies, selected comparisons and before/after seals. |
| `Report::publish` | A separate bounded publication step that refuses an existing destination. |

Curated verification reads its pinned manifest, license and ASP sources. A
bounded decoder reconciles preserved assertion excerpts with exact source bytes,
helper arguments and selected-model expectations, without reading C++ files or
invoking a solver. Public upstream revision links, whole-file hashes and original
spans remain inspectable provenance; complete model contracts stay separate from
the helper's display projection. Updating a pinned manifest requires a deliberate
source/provenance review.

Selected comparison reports use schema 2. In `requested_limits.corpus`,
`license_bytes` replaces schema 1's `original_bytes`; it limits the retained
license read. The selected campaign does not read original upstream C++ files.
Previously captured schema 1 reports retain their original field names.

On Linux/macOS, process capture uses the safe process-group backend.
`Stop::Completed` means the direct child was reaped and both captured streams
reached EOF. It does not establish solver exhaustion, descendant termination
or machine quiescence. Descendants that leave the group are outside that cleanup
contract. Unresolved direct children remain explicit `PendingChild` values;
drop does not silently wait in the background. Other-platform compatibility
capture has a weaker contract.

Resource samples use a fresh Rust helper's `RUSAGE_CHILDREN`, excluding the
helper's own RSS. macOS bytes and Linux KiB are recorded and converted explicitly.
Usage propagated from descendants waited for by the solver can contribute; this
is not simultaneous process-tree RSS or device memory. The parent exclusively
waits for the helper, and the helper exclusively waits for its solver in the
inherited group. The trusted helper may report success only after that wait and
resource-record publication. Acceptance also requires the separate solver exit,
distinct PID and platform-correct units. Failed helpers trigger group cleanup,
including when a remaining solver has closed both inherited output pipes.

Timeout, cleanup deadline, retained capture bytes, decoded structure and report
bytes have separate ceilings. Byte limits include failed prefixes but exclude
allocator and operating-system overhead; deadlines are not hard real-time
guarantees. Publication requires the documented parent-directory ownership
assumptions and is not a durable-storage guarantee.

Maintained tests include [capture](tests/process_capture.rs),
[curated integrity](tests/curated_corpus.rs),
[clean examples](tests/example_corpus.rs),
[reported answers](tests/reported_answers.rs),
[selected runs](tests/selected_campaign.rs) and
[ordinary timing](tests/performance_campaign.rs).
The [outcome guide](../../docs/book/rust/outcomes.md) explains the corresponding
semantic distinctions on the solver side.
