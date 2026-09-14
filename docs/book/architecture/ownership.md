# Ownership, dependencies and parallel work

An execution representation has two obligations: preserve its logical meaning
and identify who owns the storage that realizes it. Equal dimensions or equal
printed atoms establish neither obligation. A formula assignment belongs to one
original theory; a relation mask belongs to one row domain; a result belongs to
one submitted candidate occurrence.

The [semantic definitions](semantics.md) describe the mathematical objects. The
[transform map](alignment.md) describes their execution. This chapter explains
which state those operations may share and when that state can be released.

## Three distinct lifetimes

**The logical subject** is an immutable admitted program or theory. It supplies
the meaning of atoms, rules and formulas. Source provenance and observations
remain associated with their admitted owner. Borrowed prepared inputs let a
caller reuse that owner without parsing or admitting it again.

**Prepared execution data** describes that subject for a particular operation:
a complete ground graph, typed relation columns, an indexed formula graph or a
certified support plan. It may retain a shared subject handle and derived
storage. Reusing it requires the same subject, compatible capabilities and a
valid physical owner. Sharing that storage alone does not establish that a
candidate is an answer set; a semantic certificate has its own applicability
and membership contract.

**Search and invocation state** belongs to a particular computation. Candidates,
frozen truth, positive snapshots, pending deltas, work budgets and incumbents
have different roles even when several use packed bits. Reusing a device or
allocation must not reuse a previous search's truth, coverage or budget.

| State | What makes reuse valid | When it is no longer needed |
| --- | --- | --- |
| Original program or theory | Exact immutable subject identity | After its readers and checked results release it |
| Construction indexes and support | The construction phase's atom and formula domains | After the final consumer has produced the retained representation |
| Relation columns and postings | Exact source, tuple dictionary and row domain | After the last query against that snapshot |
| Prepared reduct encoding | Exact immutable original theory | After its enumeration and borrowed queries release it |
| Original truth and query parameters | One candidate in that theory | Before another candidate uses the workspace |
| Compiled device primitive | Device capabilities and pipeline contract | After all prepared users and invocations release it |
| Frozen candidate and positive snapshot | Candidate, world, round and catalog identity | After all required evaluations of that round complete |
| Pending result and readback | Exact invocation and submitted occurrence order | After checked completion or failure cleanup |

A derived index is not necessarily wasteful duplication. Columns, postings and
packed device graphs answer different execution questions. Their denotations,
construction cost and lifetimes should be explicit. Duplicating owned logical
payload merely to index it requires a separate justification.

Formula instantiation consumes its source IR and retains only the analysis,
provenance, activated objectives and emitted builder needed by later phases.
The formula atom builder owns each complete atom once. Its checked AVL index
stores dense IDs and links; complete typed comparisons decide identity without
copying a second set of keys. Insertion fixes first-insertion order, while
committing a pending suffix preserves those IDs and transfers its ownership.
Consuming finalization transfers the completed atom vector into the immutable
catalog and releases the construction index. These operations consume the
enclosing work budget; catalog membership alone does not establish truth.

## Prepared formula queries

An ordinary formula enumeration constructs its `PreparedReduct` lazily, when a
candidate first needs exact subset checking. One immutable encoding represents
the original theory's reduct for every candidate. The encoding separates the
prospective subset's atom values from the candidate's membership and original
implication truth. A successful `EvaluationWorkspace` operation authenticates
that truth against the exact candidate and theory; an arbitrary external truth
mask cannot parameterize the query.

The coordinator owns this prepared encoding across batches and candidate
restrictions. A scalar check reuses its `ReductWorkspace`; concurrent completion
workers borrow the same encoding and own disjoint evaluation, parameter and
search storage. Every query resets logical state, and every returned subset is
independently checked against the original frozen reduct. Candidate restrictions
do not change the theory whose answer sets are being sought.

The original candidate encoding and the reduct encoding have independent
dimension limits. Their work shares the enumeration's cumulative search budget.
Cold preparation and each query's retained capacity also have an explicit byte
ceiling. Parallel completion charges the shared prepared owner once, then adds
the disjoint worker and result allowances. Allocation overhead and process RSS
remain outside these named-storage accounts. Failed preparation publishes no
owner; a later completion-capacity refusal can retain a successfully prepared
owner for retry without discarding its charged work.

This architecture avoids rebuilding the reduct's structure for each candidate.
The fixed encoding can be larger than a separately simplified reduct, so reuse
alone establishes neither a time nor a memory improvement. The
[parametric reduct law](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/parametric-reduct.md) states the
semantic preservation argument and its remaining implementation obligations.

## Device resource scope

[`GpuContext`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/src/context.rs)
owns a selected device, queue, granted capabilities and shared health. Each
primitive retains its pipeline and prepared subject. Its `from_context`
constructor shares these device resources; existing `new_selected`
constructors create independent contexts. `GpuFormulaProfile` explicitly retains
one compiled formula pipeline, its exact context and gate implementation.
`GpuFormulaOracle::from_profile` checks Busy, health and granted formula
capabilities, then starts fresh residency, epoch and batch accounting.
`GpuFormulaProfile::same_instance` compares the compiled owner; equal shader or
adapter metadata does not establish that identity. Preparation does not create an
answer-set claim or change the primitive's input subject.

Compilation reuse changes handle ownership only. Dispatch still uses that
profile's exact gate implementation, validates each candidate against the
oracle's original theory. Each oracle owns its subject residency and reusable
transport; each invocation refreshes candidate-local truth. The
profile retains no mutable truth or epoch that could influence another oracle's
frozen-reduct query. Reusing it requires fixed-cost checks and an owner clone;
each live oracle's retained graph and transport remain separate costs.

A primitive operation holds an exclusive context lease through submission,
readback and error cleanup. Another operation receives `GpuErrorKind::Busy`
without entering a waiting queue. This is serialized composition of primitives;
their internal GPU parallelism remains unchanged. Prepared data does not retain
that execution lease. The
[composition example](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/tests/hardware_context.rs)
executes formula checks while relation columns remain prepared on the same
context, then uses those columns again.

Device failure invalidates every primitive sharing that context. A new primitive
on the same invalidated context cannot repair it. Checked preflight capacity
refusals leave the context reusable. Relation preparation submits no queue work;
it releases its mapped host access before settling scopes. A cancellation or
deadline at that stage leaves a healthy context reusable only after every scope
and device-health check succeeds. An interrupted submitted operation may still
be live and invalidates the context. Controlled static, formula and lazy batch
entry polls control and then checks shared health, including empty batches.
Readback observes control between waits of at most 50 milliseconds and releases
mapped access before returning. Driver calls, shader execution and scope
settlement are not preempted. `GpuError::interruption()` retains the exact stop;
ordinary solving reports incomplete work without an automatic backend retry.
Candidate proposals remain accounted for even when no device result returns.
Formula work and sweep counters include decoded results only; work performed by
an interrupted unreturned submission is unknown.

Byte ceilings retain their per-primitive scope. They do not automatically sum all
prepared objects sharing a context or include device infrastructure. There is no
global context, pipeline registry or unbounded work queue. Ordinary sessions can
share an explicitly supplied context through `ExecutionResources` and
`Session::builder`. `ExecutionResources::with_formula_profile` additionally
reuses the supplied compilation across ordinary formula sessions and derives
its context from that profile, preventing mismatched context/profile pairs.
Each session still owns its executor, subject residency, search, candidate queue,
counters and incumbent. Context-only resources compile per session; neither
form installs a global cache. Hard adapter policy is checked before profile
reuse; CPU and automatic formula policies retain their existing CPU route. The
[session example](../rust/sessions.md#reuse-and-identity) shows this composition.

## Parallelism follows the dependencies

Independent candidate checks share the immutable program while retaining
candidate-local truth and outcomes. An owned Rayon pool can execute those checks
concurrently and return them in input order. Repeated equal candidates are still
separate submitted occurrences.

Within lazy grounding, source work may be shared across worlds. One batch-owned
atom catalog retains each demanded identity. Its committed prefix is immutable
during a source scan; callbacks append to a disjoint pending tail. Canonically
ordered local IDs select source rows by borrowing that prefix, so the Union and
Worlds policies do not construct separate owned atom snapshots. IDs preserve
first-demand order and do not encode canonical atom order.

The union of positive snapshots supplies possible bindings; each world's own
packed snapshot and frozen seed determine whether an offered instance contributes
to that world. Catalog presence alone says neither that the atom is true nor that
the candidate is stable. World membership and pending consequence masks remain
separate from the identity index. The source visitor still rebuilds its borrowed
relation grouping and copies one bounded rule instance for each callback.

The following describes the dependency contract, not a second implementation:

```text
round_input := immutable(world_snapshots, frozen_seeds, catalog_identity)
offered_instances := source_scan(round_input)
pending := union_of_exact_chunk_consequences(offered_instances, round_input)

if source_scan_complete and all_required_evaluations_complete:
    commit_pending_atom_identities
    next_snapshots := world_snapshots union pending
else:
    retain_incomplete_progress
```

Every chunk uses the same round truth. Catalog growth must preserve existing atom
identities and each world's packed stride. A completed chunk alone cannot commit
a round. Identity commit also occurs when the last round adds no consequence:
underived offered heads still belong to the final catalog. Source snapshots drop
before the committed vector can move. Finalization transfers that vector into
the shared Model catalog, with a separate selected-position list per world.
Formula checking similarly preserves the original candidate while
searching for a proper-subset model of its frozen reduct.

GPU workgroups and lanes are physical schedules for these operations. They do
not change the subject or justify dropping a dependency. Conversely, a serial
loop in one kernel is not itself a semantic requirement: another schedule is
valid when it preserves the same operation and its bounded outcome contract.

## Memory contracts

State live retained capacity separately from cumulative construction charges.
Releasing a completed phase's scratch can reduce overlap with the next phase;
it does not refund work already performed. Keeping spare buffer capacity can
avoid reallocation while increasing retained storage. An admission limit must
account for the capacity actually retained, including simultaneously live
packing, output and transport storage within its stated scope.

Independent scalar closure construction has a per-candidate named-capacity
allowance, `Limits::max_closure_bytes`. `BatchOracle` retains one preparation
for its exact `Program` and reuses empty query workspaces across submissions.
Scalar delta rows are derived ID selections over each predicate's sole tuple
owner. Their live and spare capacities, headers and conservative replacement
overlap enter the same per-candidate allowance. Cutoffs, cache keys and logical
ID lengths are reset before reuse; stable insertion IDs belong to one candidate's
catalog lifetime and do not become persistent truth across candidates.
Before executing a batch it admits the shared cache, idle retained workspaces,
and the allowance for each assigned workspace against its collective limit.
If `C` is the cache and spare slot capacity, `P` the prepared header already
included in `C`, `R_i` a workspace's retained capacity, and `L` the per-candidate
allowance, the required envelope is
`C + sum(idle R_i) + sum(assigned max(R_i, L - P))`. Every assigned workspace
requires `L >= P`. At most `min(submitted candidates, worker count)` workspaces
are assigned, each to a contiguous candidate range; candidates inside one range
run sequentially. An empty batch admits only retained collective capacity and
does not prepare a program. Returned models, input seeds, worker stacks and
allocator overhead are separate owners.

Ordinary sessions map `SolveConfig::max_source_work` to immutable preparation
work and `max_closure_batch_bytes` to both preparation storage and the collective
owner allowance. Candidate closure work retains its separate `max_work` counter.
The existing CLI options expose these bounds without a combined work quota.
A preparation stop is distinct from an individual candidate stop; neither
establishes candidate exhaustion. A collective reservation refusal starts no
candidate and releases the admission slot.

`SemanticOutcome::query_execution()` retains the CPU producer's typed
`QueryStatistics` snapshot for independent relational execution. Preparation
builds are cumulative; active and reused workspace counts describe the latest
independent attempt that acquired admission. They count assigned owners, not
completed candidates. Retained bytes describe the current cache. Reserved bytes
are the latest attempt's admitted envelope, or zero if it admitted none.
Text statistics, JSON and failure
reports use this same observation. If reading the snapshot fails, the original
typed fault and any earlier successful snapshot remain distinguishable; checked
models are retained without a coverage claim. Shared CPU, static and device
routes expose their own execution receipts instead.

[`StorageOwners`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/StorageOwners.lean)
proves the natural-number bound obtained by summing independently bounded
owners. Its premise is admitted capacity: allocator slack can exceed a proposed
reservation before the actual-capacity check refuses it. A scalar stop returns
no completed statistics; a core catalog failure separately carries its capacity
receipt. This is not a hard bound on transient allocator or process memory.
Concrete capacity measurement, worker scheduling and checked arithmetic remain
implementation obligations.

The lazy coordinator's `max_host_bytes` is a mixed, explicitly scoped envelope.
It includes actual committed/pending catalog, AVL/path and ordered-ID capacities,
their named growth overlap, nested atom payload measures, and requested packed
transport, source membership/workspace, copied-instance scratch and final model
positions. The final position allowance bounds logical selected slots; it
excludes the extra capacity retained by `Model`'s geometrically growing Vec.
Other allocator overhead and rounding outside the catalog/ordered-ID capacities,
Arc envelopes, caller inputs and backend-private transport are also excluded.
Source work includes initialization, identity lookup/insertion, canonical row
selection and prefix commits; the quota is shared across the whole batch.

Authored payload bounds exclude any costs their API says they exclude, such as
allocator metadata or driver allocations. They are not process RSS. Dropping a
device-buffer handle does not measure physical memory retirement. The
[validation reference](../reference/validation.md#performance-evidence) reports
elapsed time, authored storage and measured peak memory as separate evidence.

## Correctness boundaries

The Lean library supplies laws for typed row reconstruction, immutable-world
consequences, complete rounds and candidate accounting. The implementation must
still establish that its actual owners, IDs, buffers and transitions meet those
laws' premises. Rust borrowing helps enforce lifetimes; it does not prove source
coverage, reduct minimality or shader behavior.

The [implementation correspondence](../lean/correspondence.md) records those
remaining obligations. Failure and incomplete execution retain their own typed
outcomes. They never establish rejection, inconsistency, exhaustive enumeration
or proved optimality merely because no answer was delivered.
