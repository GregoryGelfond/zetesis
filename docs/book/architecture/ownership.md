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

Core construction descriptions (`Value`, `Atom`, `Template`) are distinct from
admitted execution views. `Program` imports its constants and signed predicates
into a frozen canonical vocabulary. Its template rows, explicit finite domain
and signature order retain typed IDs and topology. Interned subterms are not
thereby members of the substitution domain. `TemplateCatalog` supplies the same
ordered component storage to rules and objectives without treating an objective
as a synthetic rule or source of support.

`TemplateRef`, `PatternRef`, `FilterRef` and `TemplateTerm` borrow that storage.
`ObjectiveTemplateRef` adds objective weight polarity, priority slot and closed
condition metadata. Owned condition descriptions import into one objective tuple
authority over the same vocabulary. Conditions already backed by a source catalog
retain that supplied authority. The objective's aggregate byte allowance counts
exact shared owners once; different source prefixes can conservatively recount
shared segments. This boundary does not establish one global authority for all
source syntax, generated bindings and observation values.

A Program's `CarrierAtom` retains signature and explicit-domain coordinates,
sharing the canonical vocabulary. Sparse candidates do not enumerate or count
the complete Cartesian carrier. A positional `GateAtom` additionally witnesses
a checked rank for that exact Program. Equal logical atoms from separately
admitted Programs are comparable semantically but cannot exchange applicability
witnesses. Tuple writers created for the same Program share vocabulary identity
while retaining independent atom-row scopes and candidate truth.

Formula source preparation admits scalar, constructor, predicate and pattern
components into one canonical authority. Compiled expressions and patterns retain
occurrence coordinates and topology; temporary themelios source values remain
at the admission boundary. Preparation transfers this authority and its accepted
work/storage receipts into instantiation, which retains the analysis, provenance,
activated objectives and emitted builder needed by later phases. Its atom builder
owns the evolving authority; checked AVL indexes store discovery positions and
links, while exact interning indexes store only
canonical IDs. First discovery fixes a local position. Committing a pending
suffix preserves those positions and publishes a readable prefix. Finalization
transfers the discovery map and shares its immutable payload, then releases the
construction indexes. A refused operation can retain complete canonical
components without discovering or selecting them. All checked operations consume
the enclosing work budget; catalog presence establishes no truth.

The current snapshot's segment directories can grow in place only when both
the snapshot and its growing vocabulary are exclusively owned. Frozen vocabulary
remains shared. Otherwise publication constructs fresh directories, preserving
every externally retained prefix. Reservations precede visible changes; both
retained capacity after refusal and temporary replacement overlap are counted.

The eager formula join owns its current partial binding and undo trails. A
completed row without a generated body or head suffix lends that binding to its
immediate consumer through the existing `Binding` view. The final join depth
stays intact until the borrow ends; the next advance performs its pending undo
exactly once. Generated continuations return an owned binding through the same
row interface. Support derivation and arithmetic family tasks explicitly request
owned rows: those tasks can retain a row while taking join evidence or extending
their continuation stack. Their copy still precedes complete-row filtering, so
copy-budget refusal cannot move behind an authored-expression failure.

One base-traversal state distinguishes searching, a completed nonempty row awaiting
undo, a visited empty-pattern prefix, and exhaustion. Visiting the empty prefix
does not establish exhaustion: its next traversal step still admits its ordinary
work before finishing. Generated continuations have their own finite cursors and
can finish consuming an owned row independently of that base state.

Lent and owned completion admit the same substitution and checked slot span.
An owned binding copies only scoped term IDs and explicit absence, retaining one
vocabulary witness and a capacity lease; it does not copy scalar payload. The
join's completed base-row copies record `binding_snapshots`, which is not an
enumeration count. Emitted atom selections retain source-scoped discovery
coordinates, while formula topology has its own owner. Lending changes neither
positive-row order nor source scopes, typed values, comparison meanings, negative
gates or required rejected-row validation. It does not establish a process memory
bound or a measured speedup.

Compiled observation metadata has its own immutable vocabulary, shared with the
source bundle's directives and selectors. Evaluation borrows that vocabulary and
the supplied model as fixed inputs. Captures remain borrowed term views; generated
values use scoped IDs in a per-operation derived arena. Typed wildcard-key
metadata is separate from logical ground terms. Only the final observation result
exports owned themelios symbols. These are the contracts of the source and
observation boundaries, not a claim that every subsystem shares one physical
owner or one memory allowance.

## Shared original narrowing index

Region enumeration prepares one immutable `IndexedTheory` in `zetesis-sat`:
the exact admitted `Theory` and its `Narrower` index, constructed together.
Scalar candidates, parallel candidate producers and native membership workers
share that owner. Proper-subset queries use the same original DAG index under
each candidate's frozen truth. Reuse requires `Theory::same_instance`; separately
admitted equal formulas are different subjects. This is enumeration-owned
preparation, with no global cache or public interchange of raw index handles.

Only immutable indexing is shared. Each candidate region owns its knowledge;
each reduct traversal starts with private knowledge under its candidate's mask.
Evaluation workspaces, budget leases and traversal state remain separate.
Candidate-only restrictions retain their own indexes and never enter the
original theory or supply its support. Tight and positive certificates retain
their existing membership procedures and do not execute a general reduct query.

Index construction is charged once to candidate preparation, one work unit per
original node. Reusing that index adds no construction charge to reduct-region
work. A standalone membership check without candidate preparation constructs
and charges its own index. Failed query reads retain their existing work and
query-count receipts, and sharing does not change verification or search limits.
The index constructor retains its existing allocation boundary; the shared
handle uses an infallible Arc allocation, as does Theory. These logical work
and ownership facts do not establish a process RSS bound or a measured speedup.

## Prepared formula queries

A formula enumeration under the clauses method (`--search clauses`) constructs
its `PreparedReduct` lazily, when a candidate first needs exact subset
checking; the regions method, the default, queries the reduct as a region tree
over the original formulas under the candidate's mask and builds no encoding,
as the [execution chapter](execution.md) describes. One immutable encoding represents
the original theory's reduct for every candidate. The encoding separates the
prospective subset's atom values from the candidate's membership and original
implication truth. A successful `EvaluationWorkspace` operation authenticates
that truth against the exact candidate and theory; an arbitrary external truth
mask cannot parameterize the query.

The coordinator owns this prepared encoding across batches and candidate
restrictions. A scalar check reuses its `ReductWorkspace`; concurrent completion
workers borrow the same encoding and own disjoint evaluation, parameter and
search storage. Each worker reuses that storage within its batch; completion
workers are currently created afresh for subsequent batches. Before each subset
search, the worker clears candidate-dependent truth, decisions and ordering.
It retains a complete watch index and a completed unconditional propagation
prefix for the exact immutable encoding. These assignments follow from the
encoded query's unit clauses before any candidate parameters are supplied;
they need not be facts of the original ASP program. New authenticated parameters
extend that prefix. A watch false in the prefix remains protected by another
watch true there, so completed prefix events need not be replayed.

An interrupted index construction cannot be reused. Interrupted unconditional
propagation publishes no reusable prefix; a retry undoes it before recomputing.
A completed unconditional conflict refutes every parameterization of this
encoding, whereas a candidate conflict refutes only that query. A different
owner invalidates both the index and the unconditional result.

Every returned subset is independently checked against the original frozen
reduct. Candidate restrictions do not change the theory whose answer sets are
being sought. Retained watch positions carry structural information, not a truth
claim about a previous candidate.

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

## Candidate frontier observations

`RegionSearchStatistics::frontier` records queued, inactive candidate regions in
the parallel proposal executor. Its byte count includes allocated entry capacity
and the owned decision and knowledge buffers of queued regions. An empty frontier
can retain capacity. The count and byte peaks are maxima over frontier changes;
they need not occur together.

The observation excludes active regions, temporary split copies, shared immutable
indexes, candidate batches, thread stacks, allocator overhead and device storage.
It is neither peak process memory nor a memory limit. Uninstrumented traversal
routes report absence, not zero. The statistics JSON preserves that distinction
in `search.candidate_regions.frontier`.

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

Advertised support and enabled use are distinct observations. `GpuInfo::features`
reports the adapter's optional wgpu features; discovery creates no device.
`GpuContext::features` reports the subset granted to the retained device, and
`GpuContext::limits` exposes its granted limits. Current constructors request no
optional features. These operations perform no submission and establish neither
device health nor an optimized execution path. The device inventory reports
advertised features using wgpu's names; a later feature-specific kernel still
needs explicit admission, an exact baseline and physical qualification.

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
[composition example](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/tests/integration/hardware_context.rs)
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
separate from the identity index. The source visitor rebuilds its borrowed
relation grouping and offers a callback-scoped instance containing checked
`AtomKey` views. Its key buffers own metadata; they do not copy Atom/Value payload.
Retaining callback results requires a separate admitted ownership operation.

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
before the discovery map can move. Finalization transfers that map into
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
catalog lifetime and do not become persistent truth across candidates. A dense
relation's recorded discovery positions, kept for rows derived again, are
identity metadata of the workspace's authority: they count in the same
allowance, are skipped rather than refused when the work or byte allowance
cannot hold them, and are discarded with a failed or foreign workspace.
Before executing a batch it admits idle retained workspaces and the allowance
for each assigned workspace against its collective limit. If `S` is spare slot
capacity (zero once every reserved slot holds a workspace), `H` the separately
allocated preparation header, `P` the prepared queries' retained bytes (including
`H`), `R_i` a workspace's retained capacity, and `L` the
per-candidate allowance, the required envelope is
`H + S + sum(idle R_i) + sum(assigned max(R_i, L - P))`. Every assigned workspace
requires `L >= P`. The other immutable preparation payload has its separate
preparation ceiling. The cache's own header and Arc counters are bookkeeping
outside this envelope, like allocator metadata. Since `H <= P`,
`workers * L` remains the conservative setup allowance: CPU closure setup checks that product
before allocating its pool, compiling static rules or initializing candidates.
This conservative guard also covers eager and shared CPU closure execution;
formula and device routes apply their own resource checks instead.
`SolveConfig::validate` checks representation-independent policies. With valid
policies, an already cancelled session stops before executor resource checks.
The command derives `L` as each worker's share of the collective ceiling when
`--max-closure-bytes` is not given. At most
`min(submitted candidates, worker count)` workspaces are
assigned, each to a contiguous candidate range; candidates inside one range run
sequentially. An empty batch admits only retained collective capacity and does
not prepare a program. Returned models, input seeds, worker stacks and
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
builds and adoptions are cumulative; an adoption shares a compatible immutable
owner supplied by candidate narrowing without reconstructing it. The retained
receipt still describes that owner's original construction work. Active and
reused workspace counts describe the latest
independent attempt that acquired admission. They count assigned owners, not
completed candidates. Retained bytes describe the current cache. Reserved bytes
are the latest attempt's admitted envelope, or zero if it admitted none.
Text statistics, JSON and failure
reports use this same observation. If reading the snapshot fails, the original
typed fault and any earlier successful snapshot remain distinguishable; checked
models are retained without a coverage claim. Shared and device routes expose
their own execution receipts instead.

`SemanticOutcome::closure_execution()` is the other receipt of the independent
routes, lazy and eager: the per-check counters of `Statistics` and
`StaticStatistics`, summed over completed checks, with the number of stopped
checks whose partial work no counter reports. Its peak closure envelope is the
largest capacity a completed check admitted or reserved, in the same terms as
`max_closure_bytes`; it is not an allocation peak.

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
their named growth overlap and canonical payload, plus requested packed
transport, source membership/workspace, instance-key scratch and final model
positions. The final position allowance bounds logical selected slots; it
excludes the extra capacity retained by `Model`'s geometrically growing Vec.
Other allocator overhead and rounding outside the catalog/ordered-ID capacities,
Arc envelopes, caller inputs and backend-private transport are also excluded.
Source work includes initialization, identity lookup/insertion, canonical row
selection and prefix commits; the quota is shared across the whole batch.

The borrowed source reader has a separate `max_scan_bytes` allowance for its
row-reference directory, join scratch and current instance-key buffer. It
includes actual capacities and old/replacement overlap, while borrowing the
Program and catalog payload. `max_instance_bytes` bounds one instance's logical
referenced identity; it does not stand in for the scan workspace allowance.
World masks and callback transport keep their existing accounts. These owners
can be simultaneously live, so their separate admitted bounds must be composed.

Authored payload bounds exclude any costs their API says they exclude, such as
allocator metadata or driver allocations. They are not process RSS. Dropping a
device-buffer handle does not measure physical memory retirement. The
[measurement reference](../reference/measurement-protocols.md#performance-evidence) reports
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
