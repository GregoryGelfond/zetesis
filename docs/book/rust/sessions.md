# Embedding an ordinary solve

A session owns search state and borrows a coherent admitted input. It accepts
semantic configuration and control, with no argument parsing, standard streams
or answer rendering. This example enumerates the guided tour's complete family:

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

An admitted formula session can also use a caller-supplied
[membership executor](executors.md), while retaining the ordinary candidate,
reduct, objective and outcome owners.

## Pulling and stopping

`Session` is a fused iterator of `Result<AnswerSet, SolveFailure>`. Each
privately constructed `AnswerSet` retains its original `Subject`, complete interpretation and optional
fully evaluated score. Hidden atoms remain present even when `#show` would omit
them from human output. `SessionModel` remains a compatibility alias. These are
completed membership results under the selected implementation, not Lean proof
objects. A caller-supplied `BatchExecutor` must satisfy its
[soundness contract](executors.md#state-the-trusted-boundary); the host exactly
completes residual checks but trusts the executor's decisive verdicts.

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
positive consequence checking; a dependency projection supplies no such class
certificate. Attempt order is only a hint: each plan validates the complete
original ground theory itself.

`PositivePlan` admits positive atomic-head producers, including recursive ones,
and arbitrary constraints. It computes their least consequences once, then
checks original roots through the first failure. Clause search restricts candidates
to that interpretation, or excludes them all when a constraint fails. Region search
still checks original models and refutes any larger interpretation. A failed constraint leaves no answer
set; it does not necessarily rule out larger classical models. `TightPlan`
instead establishes its own acyclicity and supportedness conditions. Neither
plan changes the definition of membership. The
[semantic argument](../architecture/semantics.md) explains their relation to the
frozen reduct.

An explicit Metal or Vulkan formula session with `Oracle::Auto` uses the same
accounted tight preparation, then executes `GpuTightOracle` when it succeeds.
`StableModels::prepare_tight_certificate` exposes the shared immutable plan for
an external executor without enabling CPU membership. Repeated calls retain the
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
`ExecutionObservation::PositiveMembership` and `TightMembership` report the
selected plan; the search statistics retain attempted construction, restrictions
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

## Collecting the original world view

`WorldView::collect(input, config, limits, control)` owns a fresh unrestricted
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
canonical payload bytes: each distinct catalog allocation once, selected
positions per answer, and one optional score record per answer. A sparse answer
retains all unselected catalog atoms; equal-content separately allocated catalogs
are charged separately. The byte bound excludes
shared subjects, spare vector/hash capacity, owner-index entries, allocator/Arc overhead, engine state and the one answer being
considered for collection. It is not a process-memory limit. Members are moved
into the collection without a repeated membership check or full-model clone;
retained space can still be exponential in program size.

[Retained interpretations](models.md) explains catalog ownership, logical model
identity and the distinction between shared payload and selected true atoms.

`WorldViewFailure` preserves the checked prefix, original subject, typed cause
and available semantic outcome. Search interruption, scoring failure and
collection limits cannot produce a complete value. A model whose score or
storage admission failed can remain in verified accounting without appearing in
the retained prefix. No partial result is treated as a smaller program.

These are zetesis's collection guarantees. The streaming `Session` remains the
surface for partial observations. Interoperability with another library's solver
or query interfaces requires a separate adapter contract.

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
The current source door uses finite formula admission and eager grounding.

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

`Session::builder(input, config, control)` composes a request before execution.
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
its enabled setting replaces `config.stats`. Source preparation and publication
can record into the same scope without entering the solver's private state.
Each formula session imports only new cumulative timing work, so repeated
snapshots cannot count it again. Measurements never establish semantic coverage.

`PreparedInput::program` borrows a native `zetesis_core::Program` without source
metadata. It reaches the same relational session as a source-admitted program,
including lazy execution; it does not first compile a complete ground graph.

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

Automatic hardware policy keeps CPU execution throughout the session. Independent
relational checking in a GPU-enabled build records `AutomaticCpu`; shared source
rounds record `SharedCpu`, and a CPU-only build records `DeviceNotCompiled`.
Formula preparation records `CpuFormula`. Earlier deferred-discovery and CPU-retry
variants are no longer part of the enum. An explicit GPU request can produce
device observations; supplying resources alone does not select it.

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

Resource ownership does not choose an execution policy. CPU and automatic
execution ignore a supplied context. An explicit GPU request consumes those
resources, checks the requested backend and vendor against the supplied adapter,
and refuses a mismatch. It does not discover a replacement device.

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

`AnswerSet::subject()` retains the checked `Program` or `Theory`.
`Subject::same_instance` distinguishes shared owners from independently admitted but
structurally equal inputs. `into_interpretation()` explicitly discards the
subject association for raw-model interoperability; use it only when your own
boundary can maintain the required context.

The example uses a tiny family to show streaming and bounded complete collection.
A production consumer can process each answer as it arrives; full materialization
is an explicit choice.
Neither collection nor successful iteration acknowledges delivery to an external
sink; [publication is a separate boundary](outcomes.md).
