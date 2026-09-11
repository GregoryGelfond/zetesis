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
valid physical owner. It does not establish that any candidate is an answer set.

**Search and invocation state** belongs to a particular computation. Candidates,
frozen truth, positive snapshots, pending deltas, work budgets and incumbents
have different roles even when several use packed bits. Reusing a device or
allocation must not reuse a previous search's truth, coverage or budget.

| State | What makes reuse valid | When it is no longer needed |
| --- | --- | --- |
| Original program or theory | Exact immutable subject identity | After its readers and checked results release it |
| Construction indexes and support | The construction phase's atom and formula domains | After the final consumer has produced the retained representation |
| Relation columns and postings | Exact source, tuple dictionary and row domain | After the last query against that snapshot |
| Compiled device primitive | Device capabilities and pipeline contract | After all prepared users and invocations release it |
| Frozen candidate and positive snapshot | Candidate, world, round and catalog identity | After all required evaluations of that round complete |
| Pending result and readback | Exact invocation and submitted occurrence order | After checked completion or failure cleanup |

A derived index is not necessarily wasteful duplication. Columns, postings and
packed device graphs answer different execution questions. Their denotations,
construction cost and lifetimes should be explicit. Duplicating owned logical
payload merely to index it requires a separate justification.

Formula instantiation consumes its source IR and retains only the analysis,
provenance, activated objectives and emitted builder needed by later phases.
The final formula atom catalog owns each complete atom once; its lookup table
stores dense IDs. Full typed equality decides identity even under hash collisions.
The table is not iterated to emit atoms, so randomized hashing cannot change
their first-occurrence order. Consuming finalization transfers the atom vector
and releases the index.

## Device resource scope

[`GpuContext`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/src/context.rs)
owns a selected device, queue, granted capabilities and shared health. Each
primitive retains its pipeline and prepared subject. Its `from_context`
constructor shares these device resources; existing `new_selected`
constructors create independent contexts. Preparation does not create an
answer-set claim or change the primitive's input subject.

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
refusals leave the context reusable. Lazy batch entry polls control and then
checks shared health before source traversal, including a batch that would offer
no device chunks. Failure retains incomplete accounting.

Byte ceilings retain their per-primitive scope. They do not automatically sum all
prepared objects sharing a context or include device infrastructure. There is no
global context, pipeline registry or unbounded work queue. Ordinary sessions can
share an explicitly supplied context through `ExecutionResources` and
`Session::builder`. They prepare separate executors, pipelines and subjects;
resource reuse does not resume search or share candidate truth. The
[session example](../rust/sessions.md#reuse-and-identity) shows this composition.

## Parallelism follows the dependencies

Independent candidate checks share the immutable program while retaining
candidate-local truth and outcomes. An owned Rayon pool can execute those checks
concurrently and return them in input order. Repeated equal candidates are still
separate submitted occurrences.

Within lazy grounding, source work may be shared across worlds. The union of
their positive snapshots supplies possible bindings; each world's own snapshot
and frozen seed determine whether an offered instance contributes to that world.
This is the distinction between sharing work and sharing truth.

The following describes the dependency contract, not a second implementation:

```text
round_input := immutable(world_snapshots, frozen_seeds, catalog_identity)
offered_instances := source_scan(round_input)
pending := union_of_exact_chunk_consequences(offered_instances, round_input)

if source_scan_complete and all_required_evaluations_complete:
    next_snapshots := world_snapshots union pending
else:
    retain_incomplete_progress
```

Every chunk uses the same round truth. Catalog growth must preserve existing atom
identities and each world's packed stride. A completed chunk alone cannot commit
a round. Formula checking similarly preserves the original candidate while
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
