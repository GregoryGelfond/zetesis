# zetesis-validation

Reusable corpus integrity checks, bounded process capture, reported-answer
comparison and performance measurements. The main command provides:

| Command | Purpose |
|---|---|
| `zetesis test corpus` | Compare the maintained corpus with clingo. |
| `zetesis test backend` | Check a device against known complete answer families. |
| `zetesis test scalability` | Qualify maintained authored/corpus workloads across CPU thread counts. |

The separate `zetesis-bench` tool composes this crate's campaigns: `corpus`
measures ordinary solver processes, `compare` compares retained measurement
reports, `perf` compares ordinary runs or measures a grounding/backend matrix,
and `series` compares published series reports. See the
[command guide](../../docs/book/reference/commands.md) and the
[benchmarking guide](../../docs/book/reference/benchmarking.md) for these
interfaces.

This crate also builds two developer tools. They are not installed; build them
from the checkout with `cargo build --locked --release -p zetesis-validation`
and run them from `target/release`. The detailed examples below also cover
their source-integrity and selected-upstream capabilities:

| Command | Purpose |
|---|---|
| `zetesis-corpus` | Verify curated source collections or compare selected upstream cases. |
| `zetesis-validate` | Run the self-contained correctness regression collection against external clingo and zetesis. |

The production solver neither invokes nor depends on these validation executables.
clingo is an external reference. Reports record observations and their limits;
a process exit alone does not establish answer-set correctness.

See [INSTALL.md](../../INSTALL.md) for installation and the
[contributing guide](../../CONTRIBUTING.md#verification-and-review) for gates.
Library entry points are documented in [src/lib.rs](src/lib.rs); generate their
reference with `cargo doc --locked -p zetesis-validation --no-deps --open`.

## Verify and compare source collections

From the repository root:

```sh
zetesis-corpus verify-examples examples/correctness
zetesis-corpus verify validation/upstream/clingo-5.8.2/curated
zetesis-validate --repo . --report target/validation-report.json
zetesis-validate --repo . --reference-only --report target/reference-report.json
```

The default correctness collection is self-contained and excludes clingcon.
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

Library consumers call `corpus_comparison::run(&Request, on_case)` and inspect
typed `CaseResult` decisions and completed `ReportedAnswers`. The observer runs
after each case, outside the child deadline; it must return promptly without
changing inputs or interfering with execution. `Report::passed`,
`answer_parity_passed` and `physical_status` answer distinct questions without
parsing JSON. `Report::to_json` supplies the fallible schema-1 presentation view;
publication belongs to the caller. A rendering failure does not alter the
retained comparison decision. See [the corpus API](src/corpus_comparison.rs) and
its [direct-consumer tests](tests/integration/corpus_comparison.rs).

`CaseResult::native_exit` and `reference_exit` retain both the exit code and, on
Unix, the terminating signal. The JSON view preserves `exit_code` and adds
`exit_signal`; a missing reaped exit is distinct from signal termination.
`process::resolve_executable` accepts an explicit search path and is shared by
the command adapters. The corpus compatibility workflow supplies the ambient
PATH anew for each invocation; the benchmark adapter resolves it once per run.

## Understand answer parity

The correctness comparison checks complete displayed witness multisets, objective
costs, optimum ties, reported model counts and typed corpus contracts. It retains
duplicate symbols within a display and duplicate displays across distinct models.
It cannot reconstruct hidden atoms suppressed by `#show`, so it does not
establish arbitrary full hidden-model identity.

The clingo `optN` decoder reconciles witness and summary counts, removing exactly
the repeated final incumbent that accompanies the supported optimal enumeration
format. Unsupported or contradictory output is a failure, not a smaller answer
collection. Refusal, timeout, incomplete coverage, output limits, malformed
reports and semantic mismatch remain separate outcomes.

`answers::native_json::parse` reads the native view, schema 2 or the schema-1
form of earlier executables, with typed full atoms, shown positions/terms,
costs and terminal accounting.
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
[native decoding](tests/integration/native_json_answers.rs) and
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
See [execution contracts](src/corpus_comparison/execution/tests.rs) and
[CLI controls](tests/integration/cli_contracts.rs).

## Measure ordinary solves and execution matrices

```sh
zetesis-bench perf examples/correctness --zetesis /path/to/zetesis \
  --clingo /path/to/clingo --report target/cpu-comparison.json

zetesis-bench perf examples/correctness --suite corpus \
  --profile cpu-eager --profile cpu-lazy \
  --profile metal-eager --profile metal-lazy \
  --zetesis /path/to/zetesis --clingo /path/to/clingo \
  --report target/execution-matrix.json
```

The default ordinary comparison uses the maintained CPU baseline cases, not the
whole collection. It times uninstrumented processes after qualification;
separate statistics invocations are outside that timed population.
The matrix instead uses native `--json --stats --models 0`, so it measures
instrumented runs including typed output. The memory allowance is the host's,
as the command takes it, and each native sample's statistics record it. Native full-atom JSON and clingo's
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
child RSS observations; these never enter timed samples, and the matrix and
series campaigns take the same option, running their memory rounds after the
timed rounds. The CLI seals its own
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

The matrix accepts the actual closure, countermodel, tight-support and positive-
consequences procedures. Certified positive consequences require eager CPU
formula execution and remain distinct from an explicitly requested closure or
countermodel procedure. Phase schema 3 adds cold `reduct_preparation`; exact
schema-1 and schema-2 records remain readable with their original phase sets.
Unknown versions or phase/procedure names and contradictory typed/text views are
refused. A missing phase is not retroactively supplied to an older observation.

`matrix::Observation::from_statistics` exposes the same decoder used by the
matrix runner for separately retained, caller-bounded JSON and stderr. It checks
telemetry consistency only; callers still establish process completion and full
answer-family qualification. It performs no solving or process invocation.

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
[matrix scheduling](tests/integration/matrix_schedule.rs) and
[matrix accounting](../zetesis-bench/tests/matrix_campaign.rs). The [comparison guide](../../scripts/README-comparison.md)
documents reproducible commands, limits and protocol boundaries.

### Derive explicit parameter workloads

`performance::matrix::run_workloads(&Request, &[Workload])` uses the same
full-enumeration schedule, capture and answer checks for explicit instances.
The plan's suite is the allowed base population. Multiple instances can share
an entry path; their content identities must be distinct. Use one request per
native executable when comparing two zetesis revisions with the same clingo
reference and workload list.

The request's optional `reference` names clingo and what it is for:
`ReferencePolicy::AllPhases` times it beside every profile, and
`QualificationOnly` uses it only to establish each census. Without one, the
campaign is clingo-free: each native family is qualified against its workload's
recorded contract, and a workload without a contract, such as an amended board,
is recorded as needing clingo and is not launched. The report records the policy
its run used, `clingo_free` included, and rebuilds its schedule from it.
`Plan::with_cases` narrows the suite to named cases, run in the order given; a
path that is not a case of the suite is refused before anything is launched.

`Workload::original` retains the default corpus contract. `Workload::amended`
accepts `ConstantAmendment { source_path, name, expected, replacement }` and
locates the declaration through themelios-syntax. The source must contain exactly
one unannotated declaration with a nonnegative decimal i32 literal. Expressions,
signed source literals, radix literals and repeated declarations are refused.
The replacement may be any i32. Only the numeric token changes; parsed include
spellings must still match the admitted closure. This is source derivation,
not a claim that the amendment preserves answer sets.

For example, all six curated queens encodings declare `#const n = 8.`.
An amendment of `n` from 8 to 10 produces a separate workload without modifying
the examples tree. Admission records the original contract as provenance,
base/derived source hashes and exact byte edits. The changed workload requires
a complete clingo result; the default N=8 model count is not its expectation.
Only completed selected displays, multiplicities and costs are compared with
clingo. Complete native records remain available in the captures.

`WorkloadLimits` bounds amendment count, metadata and input/derived content
before retention. Combined workloads must also fit the request's existing
source/metadata ceilings. Each launched private source closure is sealed before
and after execution. Derived-workload reports use matrix schema 2; unchanged
suite reports retain schema 1. No first-answer phase is added by this entry
point; memory rounds follow the plan when requested. See [workload admission](tests/integration/workload_admission.rs) and
[matrix acquisition](../zetesis-bench/tests/matrix_campaign.rs) for checked library usage.
The [manual's runnable client](../../docs/book/reference/measurement-protocols.md#compare-a-parameterized-workload)
shows a complete N=4 comparison using this API, with explicit executable paths
and a new report destination.

### Share authored scalability checks and measurements

The maintained commands select one library-owned workload population:

```sh
zetesis test scalability --threads 1,2,4,8,14 --report scalability-check.json
zetesis-bench corpus --suite scalability --grounder eager \
  --compare-threads 1,2,4,8,14 --repetitions 4 --memory-runs 2 \
  --timeout-seconds 30 --campaign-seconds 1800 --report scalability-timing.json
```

`performance::scalability::workloads` returns authored queens at n=8/9/10,
pigeonhole at h=5/6/7, and unchanged queens variant 02, SEND+MORE=MONEY and task
allocation. `--include-einstein` adds the unchanged riddle. The examples root is
separate from the clean corpus root: both default to the repository's respective
`examples` and `examples/correctness` directories. Authored source/manifest
digests are checked; default contracts travel with the source, while amended
sizes require a complete clingo family. No upstream cleaning provenance is
invented for authored programs.

The reusable `scalability::run_with_cancellation` admits those workloads and
delegates to the same bounded matrix runner. Its request must select
`matrix::Suite::Scalability`; a plain matrix run without explicit workloads
refuses that suite instead of silently running only its three corpus entries.
`matrix::Plan::qualification` schedules one invocation per native profile for
each workload, and one reference invocation when clingo takes part. It adds no timing, warmup or RSS rounds
and refuses adding memory rounds. This is the plan used by `test scalability`.
`zetesis-bench corpus --compare-threads` retains normal native measurement
rounds and uses clingo only for qualification. Both preserve the complete workload and
executable identities, typed refusals, cancellation and no-clobber report
publication. `--stats` on the test controls optional elapsed presentation;
mandatory route observations remain in its evidence.

`Workload::authored(AuthoredProgram { root, entry, sha256, contract }, amendments,
limits)` provides the same bounded admission independently of the maintained
selection. It parses through themelios, rejects unsupported source closure and
checks constant edits before retaining original/derived identities. The shared
fixture population and its limits belong to the library, not a standalone
experiment runner. See [authored workload admission](tests/integration/authored_workloads.rs)
and [CLI checks](../zetesis-cli/tests/integration/scalability_commands.rs).

### Measure a fixed series of cells

`performance::series::workloads(&Corpus, WorkloadLimits)` returns the
twenty-two cells on which a sequence of solver changes is measured: seventeen
generated programs, three constant-amended queens boards and two unchanged
corpus entries, in a fixed order. `performance::families::Family` generates the
programs: each family is one shape with one size (independent sets in choice
and negation form, disjunction, tied optima, transitive closures, derivation
chains, stratified negation, a producer chain, a Latin square with its first
row fixed, a line walked with move or stay), its bytes a pure function of
the family and the size, its complete family a closed form the contract
states. `Workload::generated(family, size, limits)` admits one as a sealed
workload with entry `generated/<family>-<size>.lp`; the report retains the
family, size, byte count and digest, and materialization checks that the
generator still produces those bytes. Generated cells reach routes the corpus
never takes, in particular the closure route; the sizes were measured to keep
every cell's complete native JSON output under `series::CAPTURE_BYTES`, which
the library does not check.

`zetesis-bench perf --suite series` runs the cells through the instrumented matrix.
The `cpu-auto` profile requests the shipped defaults, automatic grounding and
oracle; the observation retains the grounding mode actually taken, so an
automatic cell cannot be read as an explicit eager or lazy one. `--time-limit`
adds a cooperative deadline to every native profile, which changes what the
solver polls at every charged unit and is therefore part of the profile's
identity; `--oracle` requests a reduct procedure explicitly. Cells a change is meant to move, a refusal or a timeout, stay in the
set: their typed decisions are the observation.

`zetesis-bench series --report LABEL=PATH …` derives one comparison from published
reports over the same cells and profiles: exact integer medians of the timed
native and reference intervals, later-over-earlier ratios of medians in the
order given, the counters the native records carry (published models,
candidates examined, charged search work, driver and phase medians), and each
report's native executable seal. A cell that did not pass is listed by its
decisions, never averaged. The reports may differ in the search method
alone, which a profile spells as `search`, or as `candidates` in a report
written before that field, with `region_workers` beside it in one
campaign's reports. The comparison also scores each report against
the reference solver: a scoreboard per report and profile lists the cells
where both passed, counts the wins, the cells whose native median lies below
the reference's, and gives every compared cell, fastest ratio first, with
the native intervals split into grounding, candidate proposal and
membership, the reference's own grounding and solving times, both peak
resident sets and the device bytes the native run accounted. Proposal sums
the candidate setup and generation phases; membership sums certificate
setup and checks, closure and exact reduct membership, reduct preparation,
original validation and the device's host oracle; grounding is the
grounding stage, absent under lazy grounding, which grounds within
membership. `--json` writes the derived comparison for retention beside the
manual's observations; the raw reports stay with their builds. See [series cells](tests/integration/series_cells.rs), [family
generation](tests/integration/performance_families.rs) and [the view](tests/integration/series_view.rs).

## Compose capture, contracts and publication

| Library capability | Boundary |
|---|---|
| `examples::load`, `verify_originals`, `Contract::check` | Source integrity and declared expected outcomes; callers establish capture and producer completion. |
| `curated::open` | Bounded verified sources, provenance and immutable selected contracts. |
| `process::invoke` | Explicit executable/arguments/directory with bounded stdout/stderr capture and typed cleanup outcomes. |
| `process::invoke_supervised` | Separately supervise trusted helpers, retaining their group reservation through cleanup after failed helper completion. |
| `answers` | Bounded decoding and reconciliation of reported answers. |
| `selected::run` | Private source copies, selected comparisons and before/after seals. |
| `corpus_comparison::run` | Fixed corpus comparisons with typed outcomes and an explicit progress observer. |
| `performance::scalability::run_with_cancellation` | One sealed authored/corpus selection for qualification-only and measured matrices. |
| `selected::Report::publish`, `performance::Report::publish` | Separate bounded publication that refuses an existing destination. |

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

Maintained tests include [capture](tests/integration/process_capture.rs),
[curated integrity](tests/integration/curated_corpus.rs),
[clean examples](tests/integration/example_corpus.rs),
[reported answers](tests/integration/reported_answers.rs),
[selected runs](tests/integration/selected_campaign.rs) and
[ordinary timing](../zetesis-bench/tests/performance_campaign.rs).
The [outcome guide](../../docs/book/rust/outcomes.md) explains the corresponding
semantic distinctions on the solver side.


### Application benchmark adapters

`performance::command::run` accepts the existing typed matrix request and an
explicit `matrix::NativeInvocation`. `Solve` uses `solve --backend … --threads …
--all`; `Legacy` retains the flat interface for sealed older executables. Both
request machine answers and statistics and share all fixed scheduling,
qualification, timeout, no-clobber publication and failure accounting. The
command adapter owns the maintained series expansion but never raises supplied
limits; the legacy executable still applies its documented series presets.

`matrix::Report::summary` borrows source identities and profiles and derives
compact counts, all-passing timed distributions and separate validated RSS
medians directly from typed samples. Its JSON view has format
`zetesis_benchmark_summary`, schema 1. It does not duplicate or reparse raw answer
captures. A failed or unlaunched timed position prevents a timing distribution
for that cell; the outcome counts remain visible. Memory medians use only
successfully validated separate memory rounds and are absent when unavailable.

`series::read_compare` reads saved matrix reports within an explicit per-file
source-byte bound, then calls the existing identity-checked comparison.
`zetesis-bench series` and `zetesis-bench compare` use this same reader.
`process::memory::measure_to_file` is the fresh supervised helper's shared
no-clobber resource-record adapter, not a cumulative in-process RSS API, and
`process::memory::run_helper` is the helper's whole behaviour, answered under
`process::memory::HELPER_COMMAND` by `zetesis-bench`.
