# Embedding an ordinary solve

A `Session` owns solving state and borrows a coherent admitted input.
`SolveConfig` selects execution policy and limits; `zetesis_cpu::Cancellation`
carries shared cancellation and an optional deadline. The session accepts these
values without argument parsing, standard streams or answer rendering. This
example enumerates the guided tour's complete family:

```rust
# extern crate zetesis_solve;
# extern crate zetesis_core;
# extern crate zetesis_cpu;
# extern crate zetesis_themelios;
{{#include ../examples/session.rs:example}}
```

`admit` is the strict normal-profile door used by this example. Use
`admit_extended` for its supported scalar extensions, or `admit_formula` for the
broader finite formula profile. These are explicit library choices. The ordinary
source driver provides automatic profile selection; `Session` receives an
already prepared representation and does not retry admission itself.

`SolveConfig::validate` checks representation-independent policy combinations.
Prepared input adds its profile constraints; the selected executor then checks
its own resources. CPU closure setup conservatively requires
`workers * max_closure_bytes <= max_closure_batch_bytes` for lazy, eager and
shared execution, before allocating its pool or initializing candidates.
Formula and device execution do not inherit this unused reservation. With valid
policies, an already cancelled request stops before executor resource checks
and allocation; an incompatible policy remains a setup error.

For ordinary resource policy, derive preparation and execution settings together:

```rust
# extern crate zetesis_solve;
# extern crate zetesis_themelios;
use std::num::NonZeroUsize;
use zetesis_solve::Resources;

let resources = Resources::new(512 * 1024 * 1024, NonZeroUsize::new(4).unwrap());
let input = zetesis_themelios::admit_formula(
    "1 { a; b } 1.".into(),
    resources.admission_options(),
    resources.expansion_limits(),
    resources.formula_limits(),
)?;
let config = resources.solve_config();
assert_eq!(config.workers.get(), 4);
# let _ = input;
# Ok::<(), Box<dyn std::error::Error>>(())
```

This policy derives named storage capacities from memory and keeps work as
checked statistics without a selected operation-count cutoff. The independent
capacities are not a global resident-memory guarantee. Explicit limit fields
remain available when an embedding needs a bounded operation; other optional
analyses and device dispatches also retain internal effort bounds.

Tight, positive and stratified certificate preparation uses the session's named scratch
allowance and shared search work. It has no separate fixed producer, dependency
or operation-count cutoff. Each certificate still checks the complete theory;
unsupported programs or insufficient certificate storage leave general reduct
checking available. Cancellation and allocation failures interrupt the run.

## Pulling and stopping

`Session` is a fused iterator of `Result<AnswerSet, SolveFailure>`. Each
privately constructed `AnswerSet` retains its original `Subject`, complete interpretation and optional
fully evaluated score. Hidden atoms remain present even when `#show` would omit
them from human output. `SessionModel` remains a compatibility alias. These are
completed membership results under the selected implementation, not Lean proof
objects. Membership is always decided by zetesis's own reduct check, on the CPU
or on a qualified GPU route; the library accepts no caller-supplied checker.

`Session::enumerate` streams all answers of the original program, including
nonoptimal answers. It evaluates objective scores but disables both candidate
pruning and incumbent selection. `Session::new` retains the ordinary behavior:
an active objective selects incumbent ties; without an objective it enumerates
the original family. `SemanticOutcome::selection()` records `All` or `Optimal`
independently of whether that search finished.

The [checked selection example](outcomes.md#check-selection-and-complete-capture)
compares these two constructors on the same objective-bearing source, including
full hidden atoms, costs and a deliberately incomplete collection.

`models: 0` requests exhaustive enumeration. In unrestricted enumeration, a positive model
limit can terminate successfully at that limit without exhausting the search.
Dropping the iterator or calling `stop()` early does not imply that unseen
candidates have been checked. Inspect `SemanticOutcome::completion()`; it is
optional precisely because a consumer may stop before a terminal classification
exists.

Live `progress()` readings do not wait for workers. Terminal reports and
`stop()` join native workers before recording their final work and decision
counts. They preserve the original interruption reason and do not cancel a
caller-owned token. Cooperative cleanup may extend beyond a requested deadline;
it does not establish exhaustive coverage.

For `Session::new` with an objective, the search phase ends before retained incumbents are yielded.
Exhausted search establishes optimum ties; an interrupted phase may yield
unproved incumbents. A score alone is not an optimum certificate. Use the
outcome's `optimum_proved()` and interruption information to distinguish proved
ties from an interrupted incumbent. Yielded tie limits also affect how much of
the selected family a consumer receives.

## Prepared independent CPU execution

An independent relational CPU session retains one `PreparedQueries` owner for its exact
program and a bounded set of empty `ClosureWorkspace` capacities. Each submitted
batch assigns disjoint candidate ranges to those workspaces. Candidate truth is
reset between checks; returned models own their atoms independently of later
workspace reuse. The scalar evaluator uses disjoint delta bindings after a
complete initial scan. Completed earlier rounds account for old consequences
and constraint triggers; the final no-change round completes source coverage.
Preparation and retained capacity reuse preserve the exact least reduct closure
and answer membership. Shared-world execution retains its separate full source
schedule.

`SolveConfig::max_source_work` bounds immutable query preparation, separately
from each candidate's `max_work`. `max_closure_batch_bytes` supplies the shared
preparation and workspace allowance, while `max_closure_bytes` bounds each
candidate's named closure storage. Preparation refusal appears as
`Interruption::Preparation`; a stopped individual check remains
`Interruption::Oracle`. Successful earlier answers remain valid after either
stop, but the search has not established exhaustion.

`SemanticOutcome::query_execution()` exposes the latest captured CPU
`QueryStatistics` and any typed snapshot fault. Its preparation work is separate
from candidate work, and its reuse counts refer to assigned workspace slots,
not successful candidates. No snapshot is inferred for another execution route.
A snapshot failure preserves the last successful observation and is reported as
an execution error after the already checked prefix; it cannot manufacture a
completion result. The ordinary CLI's `--stats`, JSON `query_execution` object
and failure reports view this same receipt. These named capacity observations
are not process RSS or evidence of a speedup.

`SemanticOutcome::closure_execution()` sums, over the independent CPU closure
route's completed checks, the counters each check returns: rounds, charged
work, derived atoms and the lazy route's join counters, with the largest
admitted closure envelope. Stopped checks are counted but contribute no work,
because a stopped check returns no counters. The shared, device and formula
routes leave it absent and expose their own receipts.

## Formula membership plans

An ordinary CPU formula session with `Oracle::Auto` attempts an applicable class
plan before general reduct checking. Full normalized source analysis can prefer
positive or stratified consequence checking; a dependency projection supplies no such class
certificate. Attempt order is only a hint: each plan validates the complete
original ground theory itself.

`PositivePlan` admits positive atomic-head producers, including recursive ones,
and arbitrary constraints. It computes their least consequences once, then
checks original roots through the first failure. Clause search restricts candidates
to that interpretation, or excludes them all when a constraint fails. Region search
uses a singleton cursor, checking accumulated restrictions before publication.
A failed constraint leaves no answer
set; it does not necessarily rule out larger classical models. `TightPlan`
instead establishes its own acyclicity and supportedness conditions. Neither
plan changes the definition of membership. CPU tight checks retain exclusive
`TightWorkspace` scratch in each worker; immutable plans can be shared. Every
call checks its retained capacity against the current allowance. Interrupted
or rejected candidates cannot leave truth or support in the next check.

`StratifiedPlan` extends direct evaluation to the supported stratified normal
grammar. It completes positive components in signed dependency order and checks
the original constraints. Positive and stratified certificates share the same
single-answer publication mechanism. Unsupported formulas fall back to another
applicable certificate or general reduct checking. This direct route is currently
CPU-only; an explicit GPU request retains its device checking contract. The
[semantic argument](../architecture/semantics.md) explains their relation to the
frozen reduct.

An explicit Metal or Vulkan formula session with `Oracle::Auto` uses the same
accounted tight preparation, then executes `GpuTightOracle` when it succeeds.
`StableModels::prepare_tight_certificate` exposes the shared immutable plan to
the GPU route without enabling CPU membership. Repeated calls retain the
first construction attempt and its work; CPU checking can activate that same
owner before enumeration. Theories without a complete tight certificate continue
through general device propagation, as does explicit `Oracle::Countermodel`.
A positive program may itself qualify for tight checking; positivity alone does
not select the general route. Device failures are returned
without CPU replacement. Tight device work counts a complete original-truth and
support scan; `gpu_formula_rounds` only limits the general propagator.

`completion_workers` bounds unresolved queries. Complete tight device results
use scalar original validation and batch commit, preserving the configured
scratch limit without creating residual worker threads. Their completion receipt
reports the selected scalar executor and zero effective residual workers.

General device sessions with parallel region production retain separate proposal
and residual Rayon pools. Four workers in each pool therefore retain eight Rayon
threads, although production, device checking and completion are joined phases.
The pools belong to the session; `ExecutionResources` does not share them across
sessions. A scalar completion executor creates no residual pool. Worker counts
describe these executors, not a bound on all process or driver threads.

Semantic preparation precedes executor construction. A preparation interruption
remains an owned session outcome with its work. Executor construction errors
retain the existing fallible session-start contract; no session is returned in
that case. Enabled phase measurements still include the preparation attempted
before such a start failure.

Class-shape or optional-capacity refusals retain general checking. Cancellation,
exhausted shared work and allocation failure remain explicit interruptions.
`ExecutionObservation::PositiveMembership`, `StratifiedMembership` and
`TightMembership` report the selected plan; the search statistics retain attempted construction, restrictions
and checking work. Use `Oracle::Countermodel` to select the general comparison
path explicitly. Automatic device execution uses tight checking when its exact
certificate is available; other theories retain general device checking.

General completion under the clauses method lazily constructs one
`PreparedReduct` for the exact original theory; under the default regions
method the proper-subset query is a region tree over the original formulas and
no encoding is built. Subsequent candidates supply membership and authenticated original-truth
parameters, rather than rebuilding the encoding. Scalar checking reuses one
workspace; Rayon workers borrow the shared encoding with disjoint query state.
A workspace retains a complete watch index and completed unconditional unit
consequences only for the exact prepared owner. Before each subset search, it
clears candidate-dependent assignments and decisions, then supplies fresh
candidate parameters. An interrupted unconditional propagation is recomputed;
a candidate conflict never becomes an unconditional refutation. Workers reuse
these results within a completion batch; the coordinator retains the encoding
across batches. Returned countermodels
still undergo independent frozen-reduct validation. Candidate restrictions do
not replace this original owner.

`SolveConfig::max_reduct_bytes` bounds cold preparation and each query's named
storage separately. `max_completion_scratch_bytes` bounds the shared prepared
owner plus simultaneous completion workspaces and results. That setting also
supplies a separate allowance for optional class preparation and checking; it is
not a combined process-memory ceiling. Preparation work is charged once to the
session search budget and reported in `SolvePhase::ReductPreparation`.
Reuse preserves consumed work and pending-candidate accounting after a refusal.

## Hybrid formula sessions

`PreparedFormula::ground_hybrid()` and its bundle counterpart return a shared
`HybridFormula` owning the original source, retained core and streamed constraint
plans. Pass it through `PreparedInput::hybrid(&owner)`. The profile uses indexed
host joins; richer constraints remain in the eager core. `Grounder::Lazy` or
`Auto` can use CPU or GPU core membership. A GPU request uses the existing tight
checker or general formula executor, with exact CPU completion of unresolved
results. Source-constraint propagation and final acceptance remain on the host.
An eager schedule, closure oracle or external batch executor is refused for
this profile.

Objective programs use the ordinary scoring and optimal-answer selection.
Each new core answer must satisfy the streamed constraints before it can be
scored, retained or used to improve an objective bound. Rejected proposals leave
the incumbent unchanged. An interrupted check cannot establish optimality.
Retained optimal ties have already passed the check and are not checked again
when returned. A model limit caps returned optimal ties without stopping the
search for a better answer; `AnswerSelection::All` instead limits accepted
answers in enumeration order.

Preparation still completes possible support, arithmetic admission and the atom
catalog. Eligible instances are visited during admission, but their full
constraint DAGs are not stored. The original eager doors remain available.
[Source preparation](source.md#stream-ordinary-constraints) details eligibility,
identity and retained-memory bounds.

When timing is enabled, streamed checks are included in
`SolvePhase::OriginalValidation` during solving. Initial source preparation and
core admission are separate; their elapsed time is not the total cost of source
evaluation in a hybrid session.

The session first rejects candidate regions with a certain source-constraint
violation. This operation runs only on original candidate regions and returns
`NotRefuted` when it cannot establish exclusion. Source constraints may also
hold or cut one open atom when their other body literals are true. The session
alternates these deductions with formula propagation before choosing a split,
using one source allowance for the entire closure. It does not change proper-subset
queries of the frozen reduct. The ordinary formula enumerator obtains surviving
core answers, then
checks the streamed constraints before returning an `AnswerSet` of the original
`Subject::Hybrid`. A violation rejects that core answer. A completed check permits
publication; an interrupted or failed check remains pending and establishes
neither acceptance nor exhaustion. With `AnswerSelection::All`, a positive model
limit counts accepted original-program answers. With `Optimal`, it caps retained
and delivered best-known ties without stopping the search for a better answer.
Exhaustion establishes their optimality; an interrupted search can return verified
incumbents without proving an optimum.

This example checks every full answer, including the domain facts. The core has
eight answers; early region rejection leaves four for membership and final
source checking. Each returned answer chooses a q-prefix followed by a p-suffix:

```rust
# extern crate zetesis_solve;
# extern crate zetesis_core;
# extern crate zetesis_cpu;
# extern crate zetesis_themelios;
{{#include ../examples/hybrid.rs:example}}
```

Run the maintained example from a checkout:

```sh
cargo run --locked -p zetesis-solve --no-default-features --example book-hybrid
```

`SolveConfig::constraints` supplies `ConstraintCheckLimits` for each candidate
closure or final answer check: charged work, substitutions and structural-capture
reservations. The session retains a cumulative receipt across these operations. The
retained `max_scalar_bytes` field bounds requested capture-delta capacity growth;
each join reuses those cells, which borrow canonical terms. Flat binding copies retain IDs and frozen
constructor lookup reuses admitted terms, so neither adds a scalar-byte charge.
Their physical storage remains under the admitted support allowance. The
`scalar_bytes` receipt is cumulative reserved bytes, not live capacity or RSS.
These limits are separate from source admission and per-candidate reduct-oracle
limits. Completed support indexes are reused; constraint joins run for regions
and for consumed core answers. The worker checks and final checker share that
receipt. Consequence passes within one closure share its allowance. Additional
region scans and preparation can cost more than the avoided membership work,
so the schedule still needs workload-specific measurement.

Joined candidate production retains one source checker per active producer
slot across device batches, with at most the configured worker count of slots.
The checker belongs to the exact source owner, has exclusive access to its
scratch and can move between executor threads. `zetesis_sat::RegionFilterWorker`
therefore requires `Send`; the callback is never invoked concurrently on one
checker. Completion or stop releases this scratch while retaining statistics
and pending candidate receipts. Each callback's teardown is isolated; a panic
is reported as `Incomplete::WorkerPanicked` when no earlier failure takes
precedence. Scalar and singleton checks may still prepare
per operation. These preparation counts do not measure allocations or device
kernel time.

`SemanticOutcome::hybrid_execution()` reports consumed `core_answers`,
`accepted`, `rejected`, `pending` and cumulative constraint-check statistics.
Their invariant is `core_answers = accepted + rejected + pending`. Core answers
still buffered by the inner enumerator are excluded. Search statistics and
device batch receipts describe the retained core. A device-checked core
answer is counted as an original answer only after complete source acceptance.
The `region_filter` counters record attempted preparations, region checks,
refutations and failures. Those are not core-answer counts. Core membership
statistics describe surviving proposals; `verified_models()` counts original-program
answers. Cancellation or a deadline observed during source checking uses
`Interruption::Constraint`; a stop in core search retains its
`Interruption::Countermodel` cause. Typed resource or evaluation failures retain
the failure cause and incomplete outcome.
The typed source failure retains its worker-local receipt; the hybrid outcome
reports the settled total across all source checkers. A shared first-recorded
failure is resolved after workers join, including when a requested model limit
ends the session. Recording order does not assert wall-clock failure order.
Cloning the admitted owner shares identity and preparation; starting another
session starts new search and check budgets.

## Collecting the original world view

`WorldView::collect(input, config, limits, cancellation)` owns a fresh unrestricted
session and returns its complete original answer-set family only after
exhaustion. Use `models: 0`; a positive model limit is honored and can prevent
completion. Every member retains its full interpretation and original subject.
Objectives annotate the members with scores without removing nonoptimal answers.

The [source preparation example](source.md) checks the distinct empty-program
and inconsistent-program outcomes through this same API.

An inconsistent program has an empty `WorldView`. An empty program has a world
view containing one empty `AnswerSet`. Thus `WorldView::is_empty()` establishes
inconsistency, while an empty vector collected from a stopped stream does not.
An arbitrary vector and a detached outcome cannot construct a `WorldView`.

`WorldViewLimits` independently bounds retained answers, summed full atoms and
portable payload bytes: each distinct occurrence catalog's encoding once,
selected positions per answer, and one optional score record per answer.
Equal-content separate catalogs are charged separately. A sparse answer keeps
its entire shared prefix alive; identities outside its occurrence map are not
charged by this measure. The byte bound also excludes shared subjects, spare
vector/hash capacity, owner-index entries, allocator/Arc overhead, engine state
and the one answer being considered for collection. It is not a process-memory
limit. Members are moved
into the collection without a repeated membership check or full-model clone;
retained space can still be exponential in program size.

[Retained interpretations](models.md) explains catalog ownership, logical model
identity and the distinction between shared payload and selected true atoms.

`WorldViewFailure` preserves the checked prefix, original subject, typed cause
and available semantic outcome. Search interruption, scoring failure and
collection limits cannot produce a complete value. A model whose score or
storage admission failed can remain in verified accounting without appearing in
the retained prefix. No partial result is treated as a smaller program.
`into_parts()` transfers the original cause, subject, answers and outcome into
`WorldViewFailureParts` without cloning. `into_answer_sets()` remains the explicit
choice to keep only the answers and discard the other failure evidence.

These are the native collection guarantees. The streaming `Session` remains the
surface for partial observations. [`zetesis::Solver`](agent.md) connects native
execution to the canonical agent and query contracts; their nonempty snapshots
remain distinct from the native, possibly empty complete family.

## Reuse and identity

### Projected enumeration

`SessionBuilder::projected(ProjectionLimits::default())` requests one full
`AnswerSet` per key in the prepared source's explicit `#project` domain. The
key is the answer's restriction to that fixed atom domain. Hidden atoms remain
in the returned interpretation; `#show` remains a separate display operation.
Without this request the library enumerates full answer identities.

The grounder completes the domain with the original source owner.
`PreparedInput::projection()` exposes that immutable domain. A projected request
without an explicit declaration returns `ProjectionError::MissingDeclaration`.
An explicit empty domain has one class if the selected family is nonempty.
The source door uses finite formula admission; eager and hybrid owners both
retain the complete projection domain.

Objective selection precedes projection. Two answers with the same key can have
different costs, so discarding one before optimization could discard an optimum.
The requested model count bounds representatives, while original membership,
candidate and objective accounting continue to describe full answers.
`SemanticOutcome::projection()` separately reports representatives, duplicates,
charged work, named storage capacity and whether the selected key image was
completely enumerated. Search exhaustion or a proved optimum alone does not
establish complete delivery of representatives.

History limits are independent of search and objective limits. `max_keys` counts
distinct keys; duplicates need no additional entry. `max_bytes` bounds the history
header, packed keys, lookup index and reusable current-key buffer, including
replacement overlap. The borrowed prepared domain, full answers and solver state
have separate owners and limits. The peak receipt records allocated capacities;
an unallocated refused request does not increase it. `max_work` charges domain
probes, key resets, hashing, comparisons, copies and index visits, including
collision handling and failed attempts. These are named operation and capacity
allowances, not instruction counts or process RSS.

A refused key is not returned, earlier representatives remain valid, and the failure retains
original verification evidence. Cancellation and history exhaustion never prove
unsatisfiability. The CLI requests projection when the source declares
`#project`; its JSON outcome includes the separate projection receipt.
That receipt describes enumeration through the session. External rendering or
writer delivery can still fail after the session returns a representative.

`WorldView` always denotes the complete original answer family.
`SessionBuilder::collect` therefore selects all full identities, overriding both
objective selection and a prior projected request. Use a streaming session for
representatives. The laws in `Zetesis.ProjectedAnswers` distinguish original
membership, selected properties and equality of the represented key image;
they do not verify the concrete Rust history or source-domain construction.

This checked example compares the four original answers with their two
projection classes. `#show.` hides every atom in displayed output, but it does
not erase the full interpretations or merge their projection keys.

```rust
# extern crate zetesis_solve;
# extern crate zetesis_cpu;
# extern crate zetesis_themelios;
{{#include ../examples/projected-answers.rs:example}}
```

Run it from the checkout root with:

```sh
cargo run --locked -p zetesis-solve --no-default-features --example book-projected-answers
```

### Composing requests

`Session::builder(input, config, cancellation)` composes a request before execution.
Use `selection(AnswerSelection::All)` for unrestricted enumeration, or retain
the default objective selection. `resources(&resources)` supplies reusable
execution infrastructure. `collect(limits)` consumes the unstarted request,
selects all original answers regardless of a prior selection setting, and
requires complete retention. `collect_observed` uses one observer throughout
setup and all pulls. A consumed session cannot establish that a newly retained
suffix is the complete family.

`start()` and `start_observed(&mut observer)` use the
same session initialization; the builder retains no observer. Existing
constructors are conveniences over this path. Creating or modifying a request
does no validation, grounding or device discovery. Starting it preserves the
ordinary strategy checks and control polling order.

`measurements(&measurements)` supplies an explicit shared host-measurement scope;
its detailed setting replaces `config.stats`. `SolveMeasurements::stages_only()`
records coarse host stages without enabling detailed search clocks or grounding
counters; `new(false)` remains fully disabled. Source preparation and publication
can record into the same scope without entering the solver's private state.
Each formula session imports only new cumulative timing work, so repeated
snapshots cannot count it again. Measurements never establish semantic coverage.

For terminal definitions, `GroundingMode::EagerBaseTerminalDefinitions` identifies
eager base materialization followed by host reconstruction during solving, and
`GroundingMode::HybridBaseTerminalDefinitions` a hybrid base — an eager producer
core whose eligible constraints are streamed — followed by the same
reconstruction. The route supplies the base's kind (`TerminalBaseMark`), so the
mode does not depend on whether admission was observed. A grounding interval
covers the base (for a hybrid base, its core), not the complete original theory. The
`AnswerReconstruction` phase records attempted extensions, including refusals;
`terminal_execution()` separately records consumed base answers, completed
original answers and an unfinished attempt. Its accepted work totals include
source admission and remain distinct from base search work; it also reports
admission's charge, the per-answer allowance each reconstruction may use, the
latest call's accepted charge and the componentwise peak of accepted charges. These counters are
available without timing instrumentation. CLI phase schema 4 introduced
`answer_reconstruction`; schema 5 adds `model_construction`. Maintained readers
accept schemas 1–4 without inventing measurements absent from older records.

Reconstruction prepares immutable rule patterns on its first attempted answer.
The first call includes that preparation work; later calls reuse the patterns
but still account for their retained storage. Truth selections, bindings and
cursors remain private to each answer. Per-answer allowances are unchanged.

Formula sessions prepare one semantic ordering of their fixed atom catalog.
Each verified interpretation selects positions through this ordering before
scoring or output. The catalog remains the authority for atom contents; ranks
are an execution view, not a second atom store or a cross-catalog identity.
CPU and device membership paths use the same model construction.

`SemanticOutcome::model_construction()` retains accepted work, prepared rank
capacity, peak construction metadata and completed model count, even without
timing instrumentation. `SolveConfig::max_model_work` bounds each selection's
work; one-time order preparation has a separate allowance of the same size.
Every attempt starts with its own full allowance, while the receipt retains
accepted work cumulatively, including failed attempts. A refused operation is
not charged. Overflow of the cumulative work or completed-model counter is a
`SolveError::ModelStatisticsOverflow`, before an unrecorded operation or model
can be published. The work interruption's proposed count is local to the
refused preparation or selection; JSON's `required` carries that count, while
`statistics.model_construction.work` remains cumulative. `max_model_bytes`
bounds prepared ranks plus active construction metadata. This byte scope excludes the borrowed catalog and earlier
models, whose owners account for their storage separately. Ordinary policy
derives the byte capacity from memory and uses the work counter's representation
maximum. A library caller can select a smaller work allowance explicitly.
Cancellation or either bound interrupts enumeration with a typed reason. It does
not turn a verified but unconstructed answer into a published model, establish
UNSAT, or prove an incumbent optimal.

Bounded construction stops still allow previously retained incumbents to drain.
An allocation or representation fault follows the session's ordinary failure
path: its retained evidence includes earlier incumbent counts, but that path
does not deliver the remaining models. Neither behavior discards the original
failure reason in favor of a later cleanup stop.

`PreparedInput::program` borrows a native `zetesis_core::Program` without source
metadata. It reaches the same relational session as a source-admitted program,
including lazy execution; it does not first compile a complete ground graph.
`PreparedInput::relational` borrows the native program and its display metadata
from one [prepared canonical owner](source.md#admit-an-existing-logical-program).

An execution observer receives typed, borrowed facts about preparation and
execution choices. It need not parse diagnostics. This example constructs the
fact `ready.` through the native library and retains only its selected grounding
mode:

```rust
# extern crate zetesis_solve;
# extern crate zetesis_core;
# extern crate zetesis_cpu;
{{#include ../examples/observations.rs:example}}
```

`enumerate_observed` observes setup; `next_observed` observes subsequent pulls.
Each call borrows the observer independently. Ordinary iterator pulls discard
execution observations. The solver retains no event queue; any collection or
side effects belong to the observer. Successful observation does not establish
membership, coverage or publication.

The CPU backend keeps CPU execution throughout the session: relational checking
records `CpuClosure` and formula preparation records `CpuFormula`. Earlier
automatic-selection, deferred-discovery and CPU-retry variants are no longer part
of the enum. A GPU backend can produce device observations; supplying resources
alone does not select it.

An observer error stops the relevant operation and is retained separately from
device errors. Some formula
initialization failures are retained in a session and delivered by its first
pull. After a failed pull, later pulls return `None`; the failure retains the
original cause and any established semantic evidence. `SolveFailure::subject`
can identify a prepared input even when setup failed before a search outcome
existed.

The admitted owner can outlive several sessions. Each session starts fresh
search budgets, worker pools, pending queues and incumbent storage. Reusing an
owner does not resume a previous search.

### Share execution resources

With the `gpu` feature, `ExecutionResources::with_gpu(&context)` retains one
explicit `GpuContext`. It can serve several sessions without selecting another
device. This example constructs two independent native programs and solves each
on the same device. It requires an accessible physical GPU; the manual checks
its compilation without attempting device discovery.

```rust,no_run
# extern crate zetesis_solve;
# extern crate zetesis_core;
# extern crate zetesis_cpu;
# extern crate zetesis_wgpu;
{{#include ../examples/resources.rs:example}}
```

Resources retain device infrastructure, not programs, candidates, prepared
formula graphs, worker pools or incumbents. Context-only resources create fresh
primitive pipelines. `ExecutionResources::with_formula_profile(&profile)` also
retains an exact compiled `GpuFormulaProfile`; repeated general formula sessions reuse
that compilation while creating fresh subject preparation, residency and epochs.
When tight support is selected, its distinct kernel compiles on the profile's
exact context; the general formula pipeline is not used for support checking.
The profile includes its exact context and gate projection. It cannot be paired
with a different device through an independent field. The builder clones shared
handles, so the original variables need not outlive the session.

Resource ownership does not choose an execution policy. `Backend::Cpu` ignores
a supplied context. `Backend::Gpu(None)` requests the platform's native API;
`Backend::Gpu(Some(GpuApi::Metal))` and `Backend::Gpu(Some(GpuApi::Vulkan))`
request the named API. A GPU request uses the supplied adapter only if it matches
that API and the hardware-GPU requirement. A mismatch is refused; the session
does not discover a replacement device.

Operations sharing a context are serialized through nonblocking leases. A busy
context refuses the overlapping operation; no hidden queue is created. Device
invalidation affects its other clients, while input and observer refusals do
not invalidate it. Sessions retain separate work budgets and semantic evidence.
Per-primitive allocation limits do not impose a combined context or process
memory ceiling. See the [ownership contract](../architecture/ownership.md#device-resource-scope).

The following example reuses one compiled formula profile for complete
collections over distinct source owners. Collection includes nonoptimal answers
and their scores. It requires an accessible physical GPU.

```rust,no_run
# extern crate zetesis_solve;
# extern crate zetesis_cpu;
# extern crate zetesis_themelios;
# extern crate zetesis_wgpu;
{{#include ../examples/profiles.rs:example}}
```

### Retain the original subject

`AnswerSet::subject()` retains the checked `Program`, `Theory`, original
`HybridFormula` or `TerminalFormula`. Hybrid and terminal-definition answers
retain their full source owners, not merely their core or base theories.
`Subject::same_instance` distinguishes shared owners from independently admitted but
structurally equal inputs. `into_interpretation()` explicitly discards the
subject association for raw-model interoperability; use it only when your own
boundary can maintain the required context.

The example uses a tiny family to show streaming and bounded complete collection.
A production consumer can process each answer as it arrives; full materialization
is an explicit choice.
Neither collection nor successful iteration acknowledges delivery to an external
sink; [publication is a separate boundary](outcomes.md).
