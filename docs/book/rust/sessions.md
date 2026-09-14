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

## Pulling and stopping

`Session` is a fused iterator of `Result<AnswerSet, SolveFailure>`. Each
privately constructed `AnswerSet` retains its original `Subject`, complete interpretation and optional
fully evaluated score. Hidden atoms remain present even when `#show` would omit
them from human output. `SessionModel` remains a compatibility alias. These are
completed native membership results, not Lean proof objects.

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
canonical payload bytes, including each answer's entire referenced atom catalog,
selected positions and score priorities. Shared catalogs are conservatively
counted once per retained answer. The byte bound excludes
shared subjects, spare capacity, allocator/Arc overhead, engine state and the one answer being
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
retains an exact compiled `GpuFormulaProfile`; repeated formula sessions reuse
that compilation while creating fresh subject preparation, residency and epochs.
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
