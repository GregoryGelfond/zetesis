# zetesis-experiments

The main application exposes the typed relation, aggregate, tight and lazy
profiles through `zetesis bench primitives`. `primitives::Request` selects a
library configuration and `primitives::measure` publishes borrowed typed events;
human tables consume these events directly. `--json` and an optional `--report`
retain the profile's versioned JSON-lines stream. Physical requests never fall
back to CPU. The current primitive command requires the application's `gpu`
build feature even when selecting a CPU profile, because this experiment crate
still includes its device implementations.

The compatibility `zetesis-bench` executable delegates to `command::execute`.
It retains static, formula, grounding, finite-table and feedback profiles and
its existing views. A complete table applicability refusal remains distinct
from both a passing campaign and an operation failure.

Bounded development experiments with reusable Rust interfaces and a
`zetesis-bench` command adapter. Each profile names the operation it measures,
its reference comparison and its limits. Primitive measurements do not establish
ordinary solver acceleration.

Use [Install and run](../../README.md#install-and-run) to install a release build.
The [execution chapter](../../docs/book/architecture/execution.md) describes how
these operations fit into the solver. Public interfaces start in
[src/lib.rs](src/lib.rs); generate their reference with
`cargo doc --locked -p zetesis-experiments --no-deps --open`.

## Choose a profile

| Profile | Operation | Library entry |
|---|---|---|
| Default | Static reduct closure over deterministic compiled rules | `run`, `Options` |
| `formula` | General formula membership with exact residual completion | `run_formula`, `FormulaOptions` |
| `formula-projection` | Paired exact gate-projection strategies | `run_formula_projection` |
| `aggregate` | Numeric reductions over matched original/frozen eligibility | `aggregate_measurement` |
| `tight` | Complete-theory tight certificates with exact completion | `tight_measurement` |
| `lazy` | Matched relational source rounds and closures | `lazy_measurement` |
| `relation` | Equality masks over one retained typed column view | `relation_measurement` |
| `table` | Complete surviving rows and projected domains on CPU | `table_measurement::measure` |
| `grounding` | Fresh original-source formula admission | `grounding::measure_file`, `write_report` |
| `feedback` | Finite conditional-witness replay and native restriction restarts | `feedback_measurement::measure` |

Device profiles accept physical Metal or Vulkan. CPU mode explicitly omits the
device; `formula-projection` requires a physical device, and `grounding` is
CPU-only, as are `table` and `feedback`. An explicit API never silently falls back to another API, CPU or a
software adapter. Device metadata records selection, not platform-wide
qualification. The default static command requires Metal.

```sh
zetesis-bench --backend cpu --atoms 64,256 --batches 1,64,256
zetesis-bench formula --backend metal --cpu-workers 4 \
  --atoms 64,256 --batches 1,64,256
zetesis-bench aggregate --backend metal --tuples 0,64,4096 \
  --batches 1,32,128 --functions count,sum,sum-plus,min,max \
  --workers 4 --warmups 2 --repetitions 12
zetesis-bench tight --backend metal --atoms 4,64,256 \
  --batches 1,32,128 --families normal,choices --workers 4
zetesis-bench lazy --backend metal --widths 4,8 \
  --batches 1,32,128 --families sparse,dense --workers 4
zetesis-bench relation --backend metal --family independent --payload tuple \
  --rows 4096 --queries 32 --workers 4 --warmups 2 --repetitions 6
zetesis-bench table --case aliased --rows 1024 --queries 32 \
  --workers 4 --warmups 1 --repetitions 3
zetesis-bench grounding examples/correctness/standalone/send-money/send-money.lp \
  --repetitions 3
zetesis-bench feedback --check
zetesis-bench feedback --warmups 1 --repetitions 3
```

Use each profile's `--help` for its independent dimensions and resource limits.
Redirect stdout to retain the report and stderr to retain diagnostics; also
record the command's exit status. Reports are profile-specific TSV, JSON or
JSON-lines views, not a single interchangeable timing schema.

## What each measurement includes

### Conditional countermodel feedback

`feedback` is an experiment over eight fixed finite theories, with at most six
atoms, 64 original DAG nodes and sixteen roots. The [fixtures](src/feedback_measurement/fixtures.rs)
cover empty roots, falsum, one/four atomic choices, six self-loops, mixed
ordinary disjunction and choices, nested implication, and shared/repeated roots
with an unused universe atom. Their complete population has 116 interpretations.
The JSON-lines schema 1 serializes each exact original DAG and semantic atom
universe, its complete reference family, actual bounds and route receipts.

For each owner the experiment first checks all ordered witness/candidate pairs
against independent frozen-reduct evaluation: 4,742 pairs across the study,
including witnesses that are not subsets. The compiler implements
`Allow(J, X) = not (J proper-subset X and J models the reduct of T frozen at X)`.
A guard uses the original candidate truth at each formula node; it is not an
unconditional superset exclusion. For instance a witness `{a}` may reject
`{a,b}` while preserving a stable extension `{a,c}` selected by a choice.
[`Feedback.witness_exact`, `stable_allows` and `all_feedback_preserves_stability`](../../proofs/Zetesis/Feedback.lean)
state the semantic laws. They do not prove the Rust DAG compiler, allocation
admission or SAT restart implementation.

The four routes run in fixed order on each same-owner observation:

- `direct` checks every interpretation in bit order through the original native
  membership operation.
- `feedback` visits that same complete sequence, first testing retained guards.
  It learns only from `NonMinimal` in a real opaque native checked-subject record.
  A filtered interpretation saves one native membership call; it does not save
  candidate generation. This route has zero SAT restarts.
- `search` runs the unmodified native stable-model iterator.
- `restricted` uses the preceding feedback route's already acquired guards.
  After its first delivered answer it installs them through `restrict_candidates`.
  Successful installations are actual restarts retaining prior exact exclusions.
  This is pre-acquired replay, not online learning inside ordinary search. An
  iterator that delivers no answer installs no guard.

Every complete route must match the entire independent exhaustive Ferraris
family with exact atom IDs and no repeated models. Bitset outputs are lossless
within the recorded universe, not displayed projections. `--check` performs
qualification only (32 route observations). Warmup and timed populations are
separate; fixed route order and tiny inputs limit comparative claims. Stage
intervals separate native calls, guard construction/application, search setup and
restriction installation. Total route time also includes orchestration and its
receipt/vector setup. Fixture construction, reference qualification, final family
comparison and JSON publication are outside those clocks. Standalone native
checks expose call counts, not SAT work receipts; only iterator routes report
actual cumulative SAT work and projection storage.

Guard construction reserves a linear bound of `3*N + 2*U + R + 5` nodes, with one
original-node map and canonical witness-ID vector. It copies original node
**descriptors into each guard**, preserving the original theory/reduct and its
shared owner. It does not retain the original checked candidate or create an
atom/value dictionary. Finite defaults allow eight guards, 256 nodes each, 2,048
total retained nodes, 64 KiB construction capacity, 256 KiB retained-plus-building
capacity and one million cumulative construction steps. Reservations and actual
vector capacity are checked before publication. Peak receipts contain observed vector capacity, excluding refused reservation
proposals; actual allocator slack is recorded before any readback refusal. These
are maxima, not sums. Native candidate/CNF/projection
and evaluation limits are independent and recorded in the start event.

Named guard capacity includes guard-entry, node, root, map and witness vectors.
Original source/Arc storage, the temporary native checked record and evaluation
scratch, native CNF/search storage, output vectors, allocator metadata and stack
locals are excluded. Pre-acquired retained guard bytes and native projection
bytes have separate receipts; none is process RSS. Construction or installation
refusal stops the observation with its completed prefix, original error and no
completion record. A failure-publication error retains both causes. No ordinary
solver policy or performance improvement is established by saved-check counts.

### Finite-table domain projection

`table` compares a complete typed row scan, scalar support bitsets and an owned
Rayon pool sharing one immutable support index. Every procedure returns the same
original row occurrences and projected domains. The scan prepares sorted domain
memberships once; table projections resolve their supplied typed domains on each
call. Their distinct setup costs are recorded. The independent reference scans
original rows with linear domain membership and pairwise alias checks.

The fixed cases are correlated numeric columns, independently varying mixed typed
columns and repeated-variable columns with both coherent and incoherent rows.
An explicit source-index map retains duplicate occurrences in reverse source
order. Seven query families repeat in order: full domains, singleton domains,
absent values, alternating half domains, an empty first domain, the complementary
half domains and restored full domains. Restoration tests reuse the same table.
These are finite relation constraints; they do not establish ASP producer support,
source completeness, candidate truth or answer-set membership.

JSON-lines events contain the complete typed subject and its SHA-256, one-time
preparation intervals, table work/capacity receipts and every initial, warmup and
timed batch. Output domain IDs refer to the subject's typed `values` list, not
logical integers or relation dictionary IDs. Original row positions retain their
`original_indices` mapping. Each batch is checked outside its timing interval;
per-query projection and conversion intervals are also separate. Use batch wall
times when comparing scalar and Rayon schedules; summing overlapping worker
intervals does not give parallel wall time. Routes run in fixed scan/table/Rayon
order, so measurements do not control order or thermal effects.

The library caps dimensions at 8192 rows, 128 queries, four variables, seventeen
typed dictionary values, eight workers and five timed repetitions. Preparation
and per-query table work have independent receipts under the requested shared
per-operation ceiling. Limits cover the relation and table input/scratch for
that operation; caller fixture, other retained results and thread stacks are
separate. The report retains a conservative fixture envelope and actual common
result-vector capacities. These measures are not RSS. The CLI stops publication
at 64 MiB. Exit 0 means every scheduled comparison completed without refusal;
exit 1 retains a completed schedule with finite-table applicability refusals;
exit 2 denotes a command, acquisition, correctness, interruption or output failure.
Missing metrics are omitted, never represented by zero.

No GPU procedure or ordinary solving path is selected by this command.
Singleton equality-mask comparisons belong to the distinct `relation` contract;
they must not be compared with timings that also compute projected domains.
Keep source revision and executable hash beside reports when measuring changes.

### Static and general reduct checking

The default profile compares deterministic chain/dependency families with the
dense CPU reference, including closure bits and rejection reasons. Repeated
candidate patterns remain repeated occurrences. Setup, initial-case dispatches
and warm dispatches are reported separately; warm GPU calls must reuse declared
graph and transport buffers.

Static measurements exclude source parsing and complete candidate enumeration.
Scalar, Rayon and GPU routes run in fixed order, so thermal/order effects are
uncontrolled. CPU allocations can be timed while GPU buffers remain resident.
Keep these differences with any comparison.

The formula profile uses the same ordered finite candidates for scalar CPU,
Rayon CPU and hybrid GPU checking. Every GPU residual receives exact CPU
completion within hybrid time; a propagation limit cannot turn a residual into
an accepted answer. The benchmark's residual completion is serial, unlike the
ordinary solver's optional parallel completion. Source grounding, outer search
and objectives are excluded.

`formula-projection` compares Enum and Bitwise projection with alternating pair
order. Each route owns initialized device resources and must preserve ordered
exact results and declared residency. Equal adapter metadata is not proof that
two separately created devices select the same physical adapter.
See [formula measurement](src/formula_measurement.rs) and
[projection tests](tests/formula_projection.rs).

### Ordinary session projection comparison

The maintained [session-projection example](examples/session-projection.rs) compares
complete ordinary formula sessions on one admitted source. It runs scalar CPU and
Rayon exact completion; `--backend metal` or `--backend vulkan` also runs enumerated
and bitwise gate profiles compiled on the **same device context**. Supplied profile
ownership fixes the context and pipeline, and the execution observer checks the
actual projection, reported adapter and requested batch/completion settings. CPU
is the example's default; existing benchmark profile defaults are unchanged.

```sh
cargo build --locked --release -p zetesis-experiments --example session-projection
./target/release/examples/session-projection program.lp --backend cpu \
  --workers 4 --batch-size 32 --warmups 1 --repetitions 3 \
  > session-cpu.jsonl 2> session-cpu.stderr
./target/release/examples/session-projection program.lp --backend metal \
  --workers 4 --batch-size 32 --warmups 1 --repetitions 3 \
  > session-metal.jsonl 2> session-metal.stderr
```

Run each command separately and retain its exit status before proceeding. The
source must be self-contained, at most 1 MiB, and admitted by the library's finite
formula defaults; the example does not load includes or supply constant overrides.
`--workers` must be at least two for the Rayon baseline. Sessions explicitly use
`Oracle::Countermodel`, eager formulas and unrestricted complete-family collection.
Automatic positive/tight membership specialization is outside this comparison.

An untimed scalar session establishes the reference full family. Each warmup and
measured session must exhaust original candidate coverage and preserve every
full model and objective cost, including hidden atoms and nonoptimal answers.
Comparison uses dense IDs only after checking the exact shared typed atom catalog
owner. The report records this maintained in-process comparison, not raw full
families for independent reconstruction. Physical sessions additionally require
positive decoded device work and complete submitted/decoded/committed accounting;
an input with no device candidates is refused as a physical timing sample.

Each round runs scalar, Rayon, then enumerated/bitwise/bitwise/enumerated when a
device is selected. Warmups use that same schedule but emit no sample records.
The default therefore emits six CPU-only or eighteen device-comparison samples.
A final `complete` record and a successful process exit are both required; earlier
sample lines from an interrupted run do not establish a completed schedule. Errors
propagate immediately, including collection failures with their original checked
prefix. An I/O failure can occur after some lines were written.

Source admission, context creation and both pipeline compilations are outside
`session_ns`. Each timed session constructs fresh ordinary search state, CPU
completion pool where applicable, theory/device transport, exact residual queries,
scoring and bounded answer retention. It includes observer work. Comparison,
hashing and JSON output are outside sample clocks; both compiled GPU profiles and the
reference comparison family remain resident during every baseline. The CPU order
is fixed and GPU positions are only locally balanced, so order and thermal effects
remain possible. There is no end-to-end grounding, process-cold startup or RSS
measurement and no whole-session wall deadline. Dispatch waits retain the device
API's finite timeout; setup and compilation are outside that timeout.

Records bind the actual input bytes and executable with SHA-256, package version,
requested API, full reported adapter metadata, and effective formula/search,
projection, objective, reduct, batch and collection limits. In particular,
`max_reduct_bytes` is independent of `max_completion_scratch_bytes`. Retain the
exact repository revision, lockfile, toolchain/build recipe and executable
before/after seals beside reports: a package version or content hash alone does
not attest a source build. Source admission uses the defaults of that recorded
executable; it is not timed or included in session work.

Device observations are `null` for CPU runs. Scalar sessions have no batch
completion receipt; Rayon sessions have real completion observations even when
all device fields are absent. Reduct preparation is absent until a residual needs
it. Counters cover each fresh session; completion counts accumulate its entered
batches, while capacity peaks are maxima. Reported work is logical work, and
named capacity peaks retain their API exclusions; neither is elapsed time or process RSS. These are measurements of the
specified ordinary session routes, not evidence of a general CPU/GPU speedup.

### Native aggregate reductions

The aggregate profile acquires original/frozen eligibility over complete tuple
keys, then compares exact scalar, Rayon and GPU reductions. Preparation reports
Group/Theory construction, mask acquisition, wire preparation and reference work
separately. Acquisition and its retained mask storage must accompany a
reduction-only performance claim.

`device-fresh` clears residency before each sample but retains initialized
device/pipeline state. `device-resident` primes and reuses the group's transport.
Both perform new reductions. Route positions rotate; parity checks, publication
and explicit residency clearing lie outside sample clocks.
These labels describe allocation/upload boundaries, not process-cold execution.

The primitive preserves original/frozen measures and guards. It does not
establish source completeness, head permission or stable-model minimality.
See [aggregate API](src/aggregate_measurement.rs),
[fixture construction](src/aggregate_measurement/fixture.rs) and
[regressions](tests/aggregate_measurement.rs).

### Tight certificates and lazy source rounds

The tight profile compares complete original theories and identical unfiltered
candidate occurrences. Certificate preparation and independent reference checks
are outside sample clocks. Exact CPU completion of every residual is inside.
Small references use exhaustive finite membership; larger cases use general
checking without the tested certificate. Fresh and resident device routes keep
their separate allocation contracts.
GPU work counts complete node, root, producer and atom scans plus initialization
of `ceil(atoms / 32)` support words per candidate. Scalar early-exit work uses
different charges. Reported transport bytes include the packed support buffer;
they are logical requested payload, not process RSS or total device memory.
`--support atomic` keeps independent atomic producer writes; `--support grouped`
selects one complete reduction per support word. The configuration records this
selection. Both preserve the same complete producer occurrences and independently
checked witnesses. Grouped construction adds word offsets and temporary packing
cursors, with exact admission accounting; a smaller number of atomic updates
does not establish faster execution. This option belongs to the primitive
benchmark, not the ordinary solver command.
The explicit `support-uniform` and `support-skewed` families keep the same choice
DAG and candidate sequence. Both retain sixteen producer occurrences per
supported atom in total. Uniform distributes repetitions evenly; skewed retains
each original once and concentrates the remaining occurrences on one head.
Their separate limits are 4096 atoms, 1024 candidates and 65,520 producers.
Normal and choices retain their original 256-atom and 256-candidate limits.
Prepared events retain the complete occurrence sequences; timings from these
larger families require separate resource and measurement qualification.
See [tight API](src/tight_measurement.rs) and
[checking tests](tests/tight/checking.rs).

The lazy profile compares independent scalar/Rayon source joins, portable shared
Union/Worlds rounds and corresponding GPU routes. CPU-only mode uses the four
portable routes. Ordered closures and rejection reasons must match even when
routes charge different work. Source-round execution, transport and returned
results are timed; setup, reference comparison and rendering are separate.

Lazy JSON-lines schema 3 includes `independent` and `queries` sample fields,
with `tuple_probes` added to the independent work observations. Earlier schemas
without that field supply no observation; its absence must not be read as zero.
The synchronous event borrows its sample and failure receipts; a library
consumer retaining samples explicitly clones each `Sample`. JSON shape is unchanged
by that borrowing boundary.
`independent` sums completed Scalar/Rayon checks' work, catalog work, rounds,
bindings, tuple probes and derived atoms across every occurrence, including
duplicate seeds and rejected candidates. Tuple probes count rows offered to the
whole-row matcher, including rejected rows; they exclude prefix comparisons and
catalog lookups. Both catalog work and tuple probes are already part of work. Its
`peak_closure_bytes` is the maximum individual named closure envelope; it is
neither the sum of those envelopes nor a simultaneous batch peak or process RSS.
Shared routes instead retain their existing `source` receipts; their
`independent` and `queries` fields are absent (`null` in JSON).

Scalar uses the one-shot check for each candidate, including query preparation
in that candidate's work. Rayon builds queries for the exact program on the first
invocation, then reuses immutable dimensions and persistent range workspaces.
Its candidate work excludes that separate preparation. Only Rayon records
`queries`, the actual owned pool snapshot: the retained preparation's work and
bytes, cumulative preparation builds, retained/active/reused workspace counts,
retained named bytes and the last admitted collective reservation. The latter
is a configured allowance, not an observed peak. These counts describe owned
and assigned slots, not active threads or successful checks; do not sum repeated
snapshots as new preparation work or add overlapping storage subtotals.
The pool retains its default finite preparation limits and 512 MiB collective
closure allowance, independently of the configured per-candidate limits.

Receipt aggregation, cache observation, parity comparison and publication occur
after the sample timer. Preparation and workspace reuse happen within checking;
the pool itself is created before samples. A failed cache observation produces a
typed failed route, without a successful sample or completion event. This profile
does not time receipt observation or establish a benefit from preparation reuse.

These source fixtures are generated and bounded. Measurements include neither
external parsing nor an ordinary outer candidate search.
See [lazy API](src/lazy_measurement.rs) and
[regressions](tests/lazy_measurement.rs).

### Retained relation selection

The relation profile constructs one bounded typed source, its equality dictionary
and aligned columns. Scalar CPU, Rayon and the requested physical GPU receive the
same ordered queries and produce identical packed row masks. An independent
comparison against the original typed rows checks each complete output. All
routes then reconstruct selections through the same core operation and traverse
every selected argument cell.

Source construction, dictionary construction, query resolution, input selection,
Rayon pool creation, device setup and column upload are reported separately.
Repeated batches reuse the immutable relation, resolved queries and uploaded
columns. Scalar and Rayon construct packed masks directly through the core
equality predicate, then copy them into the common batch. GPU selection includes transfer, readback
and the copy into the common mask representation. Reference checks, source
hashing and report publication lie outside operation clocks. Routes run in fixed
scalar/Rayon/GPU order; order and thermal effects remain uncontrolled.
Validation traverses the rows before timed reconstruction, so that interval is
not a cold-cache observation. In physical mode, uploaded columns remain resident
during CPU samples as well.

The authored storage bound includes shared views, packed outputs and concurrent
temporary masks; later reference validation and reconstruction charge their
position vectors separately. It is neither process RSS nor total device memory. Report
events retain the full typed subject, its hash, complete masks and actual device
work. Complete evidence requires both the final event and successful execution
and publication. A writer failure can leave bytes from an incomplete event;
the caller remains responsible for flushing its output.

Relation report schema 2 records direct CPU mask production. Its
`cpu_selection_work` charges mask-word zero writes, visited rows, short-circuit
equality comparisons and selected-bit writes. Schema 1 counted the position
producer before a separate packing loop. These counters use different work
conventions; complete typed subjects and mask membership remain comparable.
Neither counter includes the common batch's initialization or mask copy.

This profile measures equality selection and typed row access. It does not
measure full tuple matching, source grounding or answer-set solving. See the
[library contract](src/relation_measurement.rs),
[shared fixtures](src/relation_fixtures.rs) and
[regressions](tests/relation_measurement.rs).

### Original-source grounding

The grounding profile loads the original include graph and performs unmeasured
indexed-join reference admission with domain analysis disabled, followed by
complete native solving. It then rotates fresh admission
with no observer, boundary observation and detailed observation. File loading
and initial parsing lie outside the admission timer. Admission includes native
raising, normalization, analysis and materialization; observer grounding spans
isolate finite instance construction.

`--joins indexed` selects the existing shortest-posting joins. `--joins table`
selects reusable support masks for eligible flat positive patterns over completed
possible support. Every timed strategy is checked against the indexed reference;
the report records both choices. Detailed counters distinguish preparation,
reuse, actual table/indexed probes, query work and accounted support peak. Other
contexts retain indexed matching, and resource exhaustion remains a failure.
Compare preparation and query costs together before inferring a grounding gain.

Library callers may set `grounding::Configuration.domain_analysis` to
`Some(DomainLimits)` for measured admissions. The default is `None`; the command
adapter leaves it disabled. The unmeasured reference always uses `None` and
Indexed joins, independently of the measured request. The report serializes all
requested domain limits and the reference policy, as well as the independent
SAT projection-history limits. A domain request does not expand source admission.
Only the frontend's checked normalized positive profile may narrow final-rule
join prefixes; inapplicable or logically stopped analysis keeps the complete-join
fallback. An individually widened argument contributes no restriction; other
finite arguments may still guard rows. Enclosing formula work or storage refusal still fails
the measured admission and preserves its report prefix.

Detailed observations retain the optional `domain_analysis` phase and the actual
frontend counters: `domain_prepare_work` covers applicability, analysis and guard
preparation, including attempted prefixes; `domain_guard_rows`,
`domain_guard_checks` and `domain_rejected_rows` count final-rule row visits,
comparisons and rejections. These do not claim fewer completed-support rounds or
fewer final formulas. Domain preparation is included in admission timing;
successful fallback does not establish an analysis fixed point. An unrequested
phase is absent, unused detailed counters are zero, and unavailable work remains
null and refuses complete attribution. Logical limits and named support capacity
are not allocator or RSS measurements, nor a wall-clock deadline.

Support attribution includes `support_construction_work`, the accepted formula
work of the complete support build, and four disjoint operation subtotals:
`support_production_work`, `support_order_work`, `support_wake_work` and
`support_publication_work`. Production includes selected-rule traversal,
variants, domain guards and head production; publication includes canonical
commit and relation postings. The remainder covers plan preparation, initial
scheduling, snapshots/query preparation and round control. Preparation before
the build and later completed-support query setup are outside the total. These
fields retain accepted charges before refusal or unwind, exclude the refused
charge, and add no new phase callbacks. They are neither elapsed times nor
expansion-budget units. Do not add the subtotals to their containing total.

Production also reports two disjoint subsets: `support_join_work` measures
advancing support-generation joins, including local joins, and
`support_head_work` measures resolving, admitting and selecting a derived head.
Join setup and keyed-group validation remain in the production remainder. These
subsets must not be added to production or to the four construction subtotals;
they preserve the same accepted-prefix, zero and unavailable-field rules.

After timing, each result is checked against the reference atom/formula catalog,
source evidence and complete native model collection. A fingerprint is available
only when its full specified identity view can be formed; unavailability is
explicit. The currently unavailable full term-observation fingerprint must not
be replaced with a weaker hash and called equivalent.
The `zetesis-execution-subject-v2` framing includes the completed projection
domain and its explicit-declaration flag. A domain change therefore cannot hide
behind identical program formulas or directive locations. Its digest is not
interchangeable with the earlier framing version.

Objectives, including inactive declarations, are refused by this profile.
It measures eager formula admission, not lazy grounding. Detailed rule attribution
can blend joins, comparisons and formula emission; it is not standalone
arithmetic-kernel timing. Finite work/storage limits are not a hard wall-clock
deadline. See [grounding API](src/grounding/mod.rs) and
[regressions](tests/grounding.rs).

## Interpret evidence and failures

The measurement libraries accept typed configurations and expose typed results
or synchronous event consumers. Preparation, sampling, parity checks and
publication have separate boundaries. Failed attempts retain their position and
available accounting; there is no replacement sample or success claim after a
failure. A writer can retain an already published prefix.

Host GPU-call intervals include packing, transfers, submission, waits and
readback. They are not device timestamps or shader time. Retained logical masks,
active transfers, authored device allocation and process RSS are different
quantities; overlapping payload scopes must not be added.

Resource limits separately bound fixture dimensions, native work, GPU work,
transport, reference checking and output. Cancelled or bounded computation is
not silently treated as an empty result. A complete measurement record is also
not a proof of ordinary solve coverage.

Portable tests exercise reference agreement, order, resources and publication
failure. Physical tests deliberately remain opt-in:

```sh
cargo test --locked -p zetesis-wgpu --test hardware_aggregate metal -- --ignored --nocapture
cargo test --locked -p zetesis-experiments --test aggregate_measurement metal -- --ignored --nocapture
cargo test --locked -p zetesis-experiments --test relation_measurement metal -- --ignored --nocapture
```

Use `vulkan` in place of `metal` for these controls on a suitable host.
Missing hardware fails the requested physical tests; portable success does not
qualify a device. Additional maintained suites cover
[backend selection](tests/backend_selection.rs),
[formula completion](tests/formula.rs),
[tight execution](tests/tight_measurement.rs),
[grounding limits](tests/grounding.rs) and
[failure contracts](tests/failure_contracts.rs).
For complete programs and cross-solver timing, use
[zetesis-validation](../zetesis-validation/README.md).
