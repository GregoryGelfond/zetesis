# Source programs and grounding

Source processing and solving are separate library capabilities. themelios owns
syntax, logical terms, program structure and source provenance.
`zetesis-themelios` checks the supported semantic profile and lowers it into
relational templates or a finite formula theory. It also retains objective and
observation data associated with that representation.

Successful parsing is only the first boundary. Profile admission, binding
safety, arithmetic evaluation and resource limits can still fail. A frontend
syntax diagnostic, an unsupported zetesis construct, and an exceeded resource
ceiling have distinct meanings; none denotes an inconsistent program.

The [checked source-preparation example](../rust/source.md) follows
`prepare_formula`, the retained analysis basis, `ground` and a complete CPU
collection. It contrasts two Boolean source families whose different counting
identities must survive materialization.

## Preserving source identity

Zetesis's private
[`Catalog`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_choice_source.rs)
checks that each retained Boolean choice family corresponds to an original
enclosing rule and its written element occurrences. A single themelios
`raise_occurrences` call supplies source-ordered statements, part keys,
diagnostics and original provenance. Zetesis retains the affected occurrences
before collecting that owner into themelios's ordinary program set. It neither
lexes source nor reparses statement fragments. The original syntax supplies
bounded node counts and an independent check of the Boolean element locations.
In particular, merging two whole rules must not combine their separate counting
groups. This source preservation supports subsequent lowering; successful
correspondence checking does not prove the lowering's answer-set semantics.

Raising diagnostics precede metadata collection and occurrence-copy admission.
Only selected rule spans and nodes consume the catalog's copy-work allowance;
unrelated source bytes are not recopied for each selected rule. The upstream
occurrence owner is materialized, and retained Boolean variants coexist with it
until program collection. Source and syntax limits bound that initial owner;
expansion Values and Origins bound the additional retained variants. These
logical limits exclude allocator overhead and are not process-memory measurements.

At the formula boundary, tuple activity and atom permission remain independent.
For `{a}.1#count{1:#true:a;1:b}1.`, `a` activates the shared tuple through its
Boolean occurrence; an eligible selected `b` activates that same tuple through
an atomic occurrence. The tuple contributes once, while only positive atomic
head occurrences can supply atom permission. Neither a true Boolean nor a
satisfied bound supplies support for an atom in its condition. The exact three
answers `{a}`, `{b}` and `{a,b}` are covered by the maintained
[Boolean element contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/boolean_element_contracts.rs).

`BooleanHeadElements.coalesced_group_in_context` proves preservation for an
assumed keyed row family and covering atomic permissions;
`boolean_group_in_context` establishes that a Boolean-only measured head filters
the context's answers without supplying new atom support. Their
[proof boundary](../lean/correspondence.md) leaves source occurrence assignment,
Rust formula construction and execution refinement open. These are obligations
of zetesis's source bridge and solver; this architectural account is separate
from teaching themelios's parsing and logical-program APIs.

## Eager and lazy execution

For admitted relational programs, **eager** grounding explicitly compiles a
complete bounded graph. This is useful when repeated candidate checks amortize
its construction. `GroundProgram::compile` is the operation that creates this
graph; constructing a `Program` or `Seed` does not silently invoke it.

**Lazy** relational execution retains source templates and streams relevant
instances during positive inference. It can avoid storing the complete ground
program. Candidate generation still owes coverage of all relevant gate choices,
and each completed closure owes coverage of all instances enabled in its final
snapshot. Laziness moves and limits materialization; it does not remove these
obligations or guarantee smaller memory use on every input.

The finite formula source path currently materializes a bounded theory. Explicit
lazy formula grounding is unsupported. This is a current implementation boundary,
not a theorem that general formulas require eager grounding. Automatic selection
chooses among implemented profiles; it does not relax admission limits to obtain
an answer.

## Source instances as a composition

The useful lower-level operations have logical contracts:

| Operation | Input and result | Obligation |
| --- | --- | --- |
| Bind | Join finite positive witnesses into substitutions | Preserve repeated-variable agreement and local scope |
| Filter | Evaluate scalar conditions on bound values | Never invent missing bindings or use undefined arithmetic as a value |
| Gate | Test frozen positive/negative candidate conditions | Use the candidate, not the growing consequence set |
| Project | Construct a head or constraint instance | Preserve the complete atom and its source instance |

The formula path evaluates terms as finite expression plans. Each operation
reads the completed prefix of earlier results. The final operation uses the same
checked evaluator and returns its value directly; only intermediate results
occupy scratch storage. Work, operand-copy charges and first-error order remain
the same. The [evaluator](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/evaluation.rs)
clears that prefix on success, failure and unwind, retaining at most 32 empty
cells between evaluations. A one-node expression needs no scratch cells;
constructing or copying its returned value can still allocate. This storage
schedule has a separate [preservation law](../lean/correspondence.md).

Each formula join owns one reusable expression workspace. Prefix checks, binding
generators and final filters borrow it in sequence; pending generators do not
retain another workspace. Reuse changes storage ownership, not evaluation order.
In particular, a false final filter does not hide an arithmetic error in a later
final filter. The [caller regressions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/evaluation/tests/callers.rs)
check these actual consumers as well as their values and failure boundaries.

Formula bindings retain source variable identities in optional slots. A pending
producer or an unrelated component variable is absent; numeric zero remains an
ordinary value. Body-prefix views borrow only the original body scope, excluding
head-only generated slots. Every scalar and atom reader checks availability and
returns a located failure for a missing required input. Local joins cannot bind
an absent outer input. Generator backtracking clears exhausted outputs before an
earlier input changes. Owning frame capacities, including optional cells, are
charged separately from copied value payloads.

The [binding implementation](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_binding.rs)
and [scope laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/BindingScopes.lean)
state these distinct responsibilities. The laws prove restriction and checked
read properties for abstract partial assignments; concrete source compilation,
arithmetic error order and Rust execution remain correspondence obligations.

Formula construction has an explicit ownership boundary. Source instantiation
consumes the source IR and owns the completed support catalog and its snapshot
while it emits formulas and activates objectives. A consuming builder then adds
coherence and support guards.
It passes nodes and roots into theory validation and retains the ordered atoms
and origins for the compiled owner. Interning indexes, producer tables and
aggregate caches remain construction scratch.
The [finalizer](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground.rs)
preserves vector order and cumulative work/resource charges. Releasing completed
scratch reduces overlap between phases; it does not establish a lower earlier
construction peak or process RSS. The [ownership chapter](ownership.md) relates
these lifetimes to prepared views and execution state.

The final formula catalog uses the shared core
[`AtomInterner`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/atom_interner.rs).
It owns each atom once and indexes stable insertion positions with an AVL tree.
Lookup performs logarithmically many checked node probes and full typed
comparisons; compared descriptors and text prefixes are additional charged work.
Insertion plans links and rotations in reusable scratch, admits capacity and
publication work, then publishes the new identity. It neither hashes complete
payloads nor shifts a sorted index. Canonical traversal is separate from the
dense insertion order.

A committed prefix supplies the count-plan collector's exact dense atom slice.
The first commit transfers the pending vector; later commits move only pending
atoms after borrowed views end. Finalization transfers that sequence into the
existing immutable `AtomCatalog`. Possible support and emitted atoms retain
distinct populations, and commitment establishes no truth.

The located source
[adapter](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground/atoms.rs)
charges lookup, copying, index construction and commit against cumulative formula
work; exact refusal cutoffs can change with these operations. `Limits::for_atoms`
derives a finite conservative named-capacity envelope from the applicable atom
ceiling and actual layouts, including index/scratch and buffer-growth overlap.
A refusal preserves existing IDs, although capacity already acquired can remain.
`FormulaResource::AtomStorageBytes` reports that capacity ceiling; allocation
failure retains the original `TryReserveError` directly in
`FormulaFailure::AtomAllocation`. Nested payload remains under the separate
scalar-byte budget. Neither named capacity nor cumulative scalar bytes measure
process RSS.

An atom in a **possible support relation** is a witness available to source
enumeration. It is not thereby true in a candidate, and an aggregate's proposed
result is not thereby its evaluated result. The emitted formula must retain the
original activation and equality conditions. Structured positive witnesses can
bind local variables before dependent arithmetic is checked. Positive-witness
matching does not invert arithmetic or introduce a global guessed value universe.

A checked `AtomKey` borrows a pattern and its current binding. Support and delta
membership use that full typed identity without making a temporary atom. Formula
interning uses the same key and materializes values only for a new dense ID.
Missing required inputs still produce a located failure; an incomplete head
prefix defers the membership check. Membership does not discharge authored-body
validation. Existing support, current delta and formula atoms retain distinct
roles even though they share the identity operation.

### Completed possible support

Formula grounding grows possible support by complete rounds. Each round uses an
immutable snapshot, exhausts the admitted positive joins and binding proposals,
and collects new positive head atoms. Aggregate and conditional truth remains in
the emitted formulas; it does not prune possible producers. A proposed aggregate
assignment value retains its original equality.

For a whole normalized positive-flat program, a private
[producer plan](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/producers.rs)
checks every original IR occurrence against the same source applicability used
by optional domain guards. It checks signed predicate names and arities,
positive dependency edges and the existing themelios SCC order. Canonical source
rules may coalesce duplicates; plan slots still refer to the original IR rule
array and preserve its provenance. The plan borrows that preparation and caches
positive body occurrence IDs. Reverse signed-predicate postings mark original
producer IDs in a packed active set. Bootstrap visits zero-input producers;
later rounds visit only producers affected by the previous round's newly
published predicates. Selected producers retain their original order and the
same first-new binding partitions. All selected scans finish before new atoms
are published. An empty active set still admits the final support round but
needs no new catalog/query snapshot.

Plan construction and traversal consume the cumulative formula work allowance.
Its header, occurrence arrays, borrowed predicate postings, packed active set
and temporary construction arrays count
against `max_support_bytes` beside the live catalog and query views. Preparation
uses checked signature searches and a conservative finite comparison allowance
for the upstream graph's opaque tree lookup; this is not an exact count of that
lookup's comparisons. Wake lookup, posting visits and packed-set reads/writes
are also charged. Temporary graph metadata is released after preparation, and
the plan is released before completed support is returned. These
checks qualify support scheduling, not satisfiability, unique-answer claims or
source-to-Rust semantic refinement. Richer programs keep the existing schedule.
`GroundingWork.support_producer_visits` counts charged entries into actual rule
variant traversal; `support_snapshot_preparations` counts charged growing
snapshot attempts, including attempts that subsequently fail. These optional
fields do not retroactively turn missing historical observations into zeros.
The [producer scheduling laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/producer-scheduling.md)
state complete input registration and published old-head history as explicit
premises. Fewer visits do not by themselves establish a timing improvement.

Ordinary positive atom heads with positive flat witnesses and pure scalar checks
or generators use [delta joins](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/delta.rs).
Flat ordinary `not` and `not not` atoms may occur between these inputs. They
provide no bindings and do not test truth during possible-support discovery;
their arguments use the existing admitted bindings and scalar evaluation.
Their truth remains in the original emitted formula and its frozen reduct.
Each new tuple combination has one first source occurrence that selects a newly
appended row. Earlier occurrences select old rows; later occurrences select all
current rows. This partitions the combinations even when one predicate occurs
several times or cardinality planning changes execution order:

```text
for pivot in PositiveSourceOccurrences(rule):
    inputs = MapOccurrences(rule, occurrence =>
        oldRows       if occurrence < pivot
        newRows       if occurrence = pivot
        currentRows   if occurrence > pivot)
    proposals = Union(proposals, JoinAndEvaluate(rule, inputs))
```

An ordinary producer without positive inputs runs once, even if it has negative
non-input atoms. Changing such atoms' possible presence cannot enable another
head proposal, because support generation ignores their truth. Aggregate,
conditional, projected, structural and nonnormal producers retain full-round traversal. Every
selected input still passes the same typed tuple matcher and scalar evaluator.
An empty proposal set establishes completion only after every required variant
and conservative producer has finished. Final formula emission visits all
complete authored-body bindings, including those with false scalar filters;
support membership cannot conceal a required arithmetic error.

The preservation argument concerns possible heads, not answer-set truth.
Removing these flat negative non-inputs leaves the positive occurrence order,
binding generation, scalar validity and head projection unchanged. Old
combinations have already proposed their heads; each new combination belongs to
one delta partition. `NormalSupport.propose_gate_independent` states the
corresponding gate-invariance law for mathematical normalized rules. Connecting
the compiler's typed instances and checked expressions to that law remains a
representation obligation. Scheduling fewer old combinations can change the
charged work and the first bounded refusal; it does not change the cumulative
generated-value population or turn a failed attempt into completed support.

The [support builder](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support.rs)
returns `CompletedCatalog` only after an entire round adds no atom. Its
`CompletedSupport` view borrows the same authoritative catalog used by final
formula grounding. Intermediate snapshots have no completion capability, and a
round, work, value or storage failure returns an error before objective activation.
A finite snapshot during growth does not establish that value generation will
terminate.

The shared [source-activity module](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_source_activity.rs)
uses this completed view for objective and projection eligibility. Acyclic
producer dependencies are evaluated in order. Unresolved components begin with
optional atoms from complete possible support, then aggregate every producer's
activity in complete rounds. Each changing round resolves at least one optional
atom as required or absent; established information cannot be retracted. The
old and new activity tables are admitted together. This finite refinement does
not enumerate answer sets or decide whether optional literals can hold together.

Rich producer bodies share the original scoped aggregate, conditional and
projection lowering. A three-state fold covers their original Boolean truth;
an optional atom and its negation remain optional. Actual costs still test the
original model through the objective query, with weight, priority and complete
tuple resolved from one binding. Source activity neither rewrites that model's
theory nor evaluates its frozen reduct.

An independently established absent producer contributes no objective row or
redundant zero-cost priority slot. For example, a required `a` makes the proposed
`n(0)` impossible in `n(N):-N=#count{1:a}`; a downstream observer of that absent
row need not publish its priority. This can shorten the raw cost vector while
preserving costs aligned by priority, the complete answer family and optimum
ties. Other conservative carriers still retain zero slots when the abstraction
does not establish absence.

The semantic bridge requires more than an empty delta. Complete typed rule and
value generation must ensure that restricting an original model to any closed
support carrier preserves its frozen reduct. Answer-set minimality then rules
out atoms outside that carrier. [SourceSupport](../lean/theorems.md) and the
[proof guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/source-support.md)
state this premise explicitly; successful Rust completion is not a proof of the
source-to-reduct correspondence.

Formula nodes use an exact-key hash index with randomized hashing. The separate
node sequence establishes dense IDs and output order; hash-table iteration never
participates in formula construction. Complete-key equality preserves node identity under hash collisions, and node ceilings are checked before a new ID is published.

Completed formula theories always include double-negated necessary support
guards. These guards strengthen candidate checks while preserving reduct subset
freedom; they are mathematically redundant, not a selectable construction option.
Construction uses one
[`Metadata` owner](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground/metadata.rs)
with flat atom headers and shared append arenas. Each header identifies its
producer sequence, ordered source locations and first atom occurrence. Producer
links preserve insertion order and repetitions. Origin links preserve full source
identity and span order, with duplicates removed when encountered. Necessary
support formulas still fold the complete producer sequence at the final support
stage; collecting metadata creates no formula nodes early.

This removes each atom's intermediate heap buffers. It adds a link word per
entry, and origin insertion can still inspect all locations associated with an
atom. The final public `formula_origins()` view owns a vector per root. Once root
and origin limits admit a guard, its ordered locations are copied directly into
that final vector. The shared arena remains live during this output assembly;
lower peak memory or faster solving does not follow from allocation count alone.
Grounding work includes shared arena relocation, origin comparisons and inserts,
producer traversal, and the final evidence traversal and copy. Allocation failure
and either origin ceiling remain located admission failures, never UNSAT.


### Optional final-rule domain guards

Eager formula preparation can request a domain attempt through the library's
`with_domain_analysis` method. It is default off, independently of `Indexed` or
`Table`, and changes neither possible-support completion nor the original
formula/reduct semantics. The source guide provides a
[checked on/off example](../rust/source.md#optional-domains-during-final-instantiation).

The private applicability check covers the exact normalized whole source and
its original positive flat rule occurrences. It excludes computed or generated
terms, negative body literals, structural/local scopes and richer producers;
a favorable dependency projection cannot qualify. The analyzer borrows that
same immutable Program until final instantiation ends. Normalized statement
deduplication does not merge the rule occurrences or their provenance.

Every complete binding must belong to the upper domain of each mandatory
positive argument. Intersecting those domains for one source variable remains
necessary, including repeated occurrences. Unknown contributes no restriction.
A global Unknown/Stopped analysis or an inapplicable program keeps complete
fallback. These are upper bounds on source bindings, not facts about candidate
truth or answer-set membership.

The guard builder retains borrowed source symbols for the meets, converts one
atomic value at a time through the existing compiler, and resolves it through
the completed support owner's sole equality dictionary. It retains only IDs and
original rule/row/column ownership. Both join strategies offer their original
rows to the same guard before binding and deeper probes. An Indexed posting can
include rows that fail another bound column; Table intersects its equalities
before offering rows. Thus offered-row and guard-rejection counts need not
match across strategies, even when complete bindings do.

`DomainBindings.complete_binding_survives` states the necessary-meet law under
explicit argument coverage. `guarded_continuations_exact` preserves the ordered
complete result list, allowing a locally matching row with no complete
continuation to disappear. The [domain-binding guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/domain-bindings.md)
keeps analyzer soundness, source/IR correspondence, dictionary identity and
recursive Rust enumeration as separate obligations. The laws are not an
end-to-end proof of the source grounder or its resource failures. Arithmetic
validation stays on the existing complete path for the excluded profiles.

Applicability, analysis, bridge and guard work consume the remaining cumulative
formula budget. Analysis has separate finite logical populations and bounded
standard allocations, outside the named support/guard byte allowance and
without a new caller-control API. Rule guards account their named scratch and
actual capacities alongside live support/table/query owners; failed work and
observed capacity remain in receipts. Optional analysis may cost more work or
storage than it saves. Its observations establish activity and completion scope,
not elapsed-time, memory or scalability improvement.

### Relation rows and vector operations

A relation row is one complete typed tuple. Formula support's
[`SupportCatalog`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/relations.rs)
owns each possible atom once, with append-only, predicate-local row identities.
A checked AVL index of those identities supports membership checks without
another atom collection. The core relation owner retains its typed equality dictionary
and argument columns across growth rounds. Each column has a
`BTreeMap<u32, Vec<usize>>` from dictionary IDs to original row positions;
insertion extends only the new row's postings. An immutable snapshot borrows
these existing columns and postings. Row and equality IDs survive append, while
queries remain bound to one particular immutable view. Numeric ID order is not
ASP term order.

For a positive witness, the selector resolves known whole-column equalities and
chooses the shortest posting list. Equal-length lists retain the first known
column's list. The matcher then checks the complete tuple in original row order,
including repeated variables and structured terms. Without a known equality,
the selector offers all relation rows; a missing bound key selects none:

```text
rows = ShortestPosting(relation, KnownEqualities(pattern, binding))
bindings = FilterMap(MatchWholeTuple(pattern, binding), rows)
```

This describes witness selection, not complete grounding. Scope, generators,
scalar guards and candidate gates retain their separate contracts. The
[bounded posting diagnostic](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/README.md#inspect-posting-selectivity)
compares intersections with an independent full-row scan. It is compiled only
for tests and does not replace production selection. Its counters measure
selectivity; integer comparisons and complete tuple probes have different costs.

The catalog cannot grow while its snapshot is borrowed. Once a round finishes,
the snapshot drops before new atoms are appended. A failed tuple or posting
extension returns no usable support owner. The completed final snapshot supplies
formula emission and objective eligibility. The core catalog uses the shared
checked AVL implementation for row and dictionary identity. Nodes contain only
IDs and links; typed comparisons inspect their actual descriptor/text prefixes.
An insertion plans the new tuple's dictionary leaves in a bounded metadata
overlay, then publishes row, equality and column changes after all fallible
checks. No historical sorted row or dictionary sequence is shifted. If a tuple
introduces `a` new values into a dictionary of size `d`, tentative patches occupy
O(a log d) cells; the checked overlay lookups can use O(a² log² d) metadata work.
Typed comparisons, node inspection, append copies and posting construction
consume the grounding work budget. Snapshot construction visits predicates
without revisiting their rows.

Scalar reduct closure uses the same per-predicate catalog. Before each round it
prepares a complete ordered ID view for each changed extent and reuses that view
for the round's joins. Preparation traverses O(n) row IDs and accounts its cache
and traversal capacity. Subsequent indexed row access is constant time and
borrows the authoritative tuple. A duplicate or refused insertion preserves an
existing prepared extent; a successful append invalidates it.

Scalar closure first visits every template against empty derived truth, including
facts, zero-positive rules and constraints under the frozen candidate. In each
later round, a binding is visited at its first source occurrence containing a new
row: earlier occurrences select Old rows, that occurrence selects New rows, and
later occurrences select Current rows. These disjoint choices preserve repeated
predicates and source occurrences. Newness uses stable per-predicate insertion
IDs, never canonical ranks, which can move when a smaller tuple is appended.
For mixed extents, one derived ID buffer partitions the canonical rows into
ordered Old and New slices. It shares the sole tuple payload owner and is cached
by both old cutoff and current extent. All-old and all-new extents reuse the
complete ordered view or an empty slice.

A round completes every selected binding and constraint before advancing its
frontier and publishing pending heads. Frozen gates and pure equality filters
preserve the eligibility of old positive bindings. Their consequences are
already present, and earlier constraint triggers remain latched. Together with
bootstrap and completed earlier scans, the final no-change round establishes
complete source coverage. The final gate-carrier comparison still checks both
directions, even for a rejected candidate. Failure returns no partial closure
and discards dirty workspace history. Public `source::scan` and shared-world
source traversal retain their complete ordered scans; this scalar schedule does
not change the answer-set definition or claim device delta execution.

[`DeltaRounds`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DeltaRounds.lean)
separates three mathematical obligations: first-new partitions cover each new
binding, old enabled heads are already in current truth, and the retained
constraint latch equals the old family's triggers. The
[reading guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/delta-rounds.md)
maps those premises to scalar catalog cutoffs, complete bootstrap and round
publication. These laws preserve a consequence step and constraint verdict;
they do not prove Rust row decoding, interruption handling, a termination bound
or answer-set membership. Those require their own representation and completed
execution arguments.

Completed scalar `Statistics::tuple_probes` counts source rows offered to the
whole-row matcher, including rejected rows. It excludes prefix-search comparisons
and catalog membership operations. Each probe belongs to a charged join-loop
step. `work` also includes canonical and delta-view preparation, initialization,
ID reads and writes, pivot checks, pending publication and final completion.
Fewer emitted bindings alone do not establish fewer probes or less total work;
none of these counters establishes an elapsed-time gain.

`zetesis_cpu::PreparedQueries` inspects one exact admitted `Program` once to
bound the assignment, cursor and undo buffers used by its joins. Its preparation
work and bytes have independent finite limits and a separate receipt. A
`ClosureWorkspace` retains the actual empty catalog metadata, predicate owners
and reference-free cursor/undo and old/new ID capacities between candidates. Assignments borrow
only the current immutable round; no candidate truth survives completion.
`Catalog::take_atoms` transfers the completed atom vector without copying its
payload and invalidates its old row/equality IDs. The exported result owns its
atoms while the empty indexes can serve another candidate. Any failed check
discards dirty workspace state before reuse. A different program instance retires
the old workspace, even when its source text is equal.

The one-shot `check_view` uses the same evaluator and charges preparation plus
candidate work to its existing cumulative work limit. Reused preparation has a
separate work receipt, so exact resource cutoffs can differ while completed
closures, constraints and seed checks agree. Retained capacities are admitted
under each prepared call's current limits, including an empty program. The
one-shot empty-program path constructs no preparation or workspace and retains
its vacuous zero-round result. This removes repeated allocation and dimension
inspection; it does not establish a timing gain. Both entry points use the
same scalar delta schedule described above.

`BatchOracle` retains that preparation and a bounded set of workspaces across
independent batches. Its indexed borrowed seed producer is divided into at most
one contiguous range per configured worker. Each range has one exclusive
workspace and its candidates run sequentially; range tasks may be stolen by
Rayon. Result order and candidate occurrences are preserved without materializing
seed views. This schedule bounds owners directly, independently of worker thread
identities. It can balance uneven candidate costs differently from per-candidate
work stealing. Shared-round and static execution keep their separate algorithms
and the same nonblocking batch admission slot.

Batch preparation uses `PreparationLimits`; a preparation stop is distinct from
an individual candidate stop. `query_statistics` reports completed preparation
builds, assigned slots retained from earlier submissions, actual cached capacity
and the latest admitted collective envelope. The envelope counts shared cache
and preparation headers once, all idle workspace capacity, and each assigned
workspace's maximum of retained capacity and its remaining per-closure allowance.
A workspace's per-closure allowance already counts the prepared header, so that
header is subtracted before combining owners. Actual unused workspace-vector
capacity and conservative replacement overlap are included. Lowering a limit
can refuse already-retained capacity; even an empty batch checks an existing
cache, while an empty batch never creates preparation. Final result retention,
source payload, allocator/tree overhead and worker stacks remain separate. These
receipts describe bounded ownership and reuse, not timing or process RSS.

`zetesis_cpu::Limits::max_closure_bytes` bounds each scalar closure's
named predicate/catalog cells, atom and value buffers, nested payload, prepared
order and old/new ID views, query-owner/assignment/cursor/undo capacities and
pending tuples, including operation scratch and conservative buffer
growth overlap. Its default is 128 MiB; zero is a zero-byte allowance. Pending
atoms use fallible buffer reservation, and moving them into catalogs transfers
their payload charge rather than counting a second payload owner. The reported
`peak_closure_bytes` is a maximum for this named envelope. Shared structural
buffers are conservatively counted per occurrence. Tree-container allocations
(including vacant slots), allocator metadata, Arc counters and final `Model`
retention are excluded. Actual allocator slack can exceed the proposed reservation
before refusal. Completed checks report their observed peak; a scalar refusal
returns no `Check` or statistics. This is a composable admission allowance, not a bound on all transient
allocator memory or total RSS. Collective worker admission and result retention
have separate owners.
`FormulaLimits::max_support_bytes` bounds the catalog's atom-vector cells,
equality layout, postings, borrowed snapshot objects and query capacity,
including construction scratch. Nested atom payloads, allocator/tree overhead
and unrelated grounding state retain separate bounds; this limit does not
measure total memory or RSS.

The command-line equivalent is the advanced `--max-support-bytes` option,
shown by `--help-all` and recorded by `--stats`. It defaults to 128 MiB and
applies to eager formula admission. Zero is a zero-byte ceiling, not unlimited
memory. Increasing this allowance does not change source-atom, work or other
independent limits.

Row identity connects relational semantics to masks, intersections and gathers.
Combining two column masks means intersecting positions in the same relation
snapshot; it must not combine values from different tuples. The bounded
[`relation` library](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/relation.rs)
provides the immutable column view used by eager formula support and primitive
experiments. It borrows the
original atoms and encodes complete typed values through one equality dictionary.
It preserves row occurrences and their order, including duplicate tuples, and
keeps original catalog indices distinct from local positions. Explicit predicate
arity and row count distinguish an empty relation from a nullary tuple.

The lazy source grounder retains its candidate-specific relation and world-mask
contracts; this eager support representation does not make a possible atom true.
Queries and selections borrow their exact relation owner. A selection validates
ordered positions; it does not establish complete grounding or answer-set
membership. Equality filtering is complete relative to its supplied input rows:

```text
query = ResolveEqualities(relation, known_values)
selected = Filter(AllEqualitiesHold(query), supplied_rows)
bindings = FilterMap(MatchWholeTuple(pattern, binding), selected)
```

This conjunction-filter primitive does not replace the eager grounder's shortest
posting policy. Additional filtering skips budgeted matcher visits and value
extraction, which can change their checked-failure boundaries. The production
change preserves exactly the previous offered rows, preserving that boundary as
well as successful bindings.

The same equality predicate can produce ordered positions or packed row bits.
[`Relation::select_mask`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/relation/selection.rs)
writes matching input positions directly into their mask words. This avoids a
position-vector intermediate when the consumer requires masks. Decoding the
result over original row order yields the same selected occurrences; distinct
rows containing equal tuples keep distinct bits. The packed output still spans
the original relation, so sparse inputs do not imply small mask storage.

An empty conjunction retains every supplied row; a missing dictionary value
retains none. Numeric ID order does not implement numeric comparison, ASP term
order or arithmetic. The existing matcher remains responsible for structural
terms, repeated variables and checked binding generation. The
[column laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/column-relations.md)
state the reconstruction and ordered-selection arguments and their limits.
The eager support catalog uses this representation as described above. Adoption
by other grounding consumers requires the same coherent tuple ownership and
removal of redundant stores; an experimental copy alone establishes neither a
smaller memory footprint nor faster grounding.

The formula binding planner also derives finite integer envelopes from directed
affine comparisons when ordinary binding steps cannot advance. For an inequality
`a₁X₁ + … + aₙXₙ ≤ b`, it derives an endpoint for one variable from known
endpoints of the other terms. Each round reads the previous intervals, combines
simultaneous proposals by minimum or maximum, and installs only missing
endpoints. At most two endpoints per variable can be installed. This bounds the
analysis independently of numeric tightening or contradictory cycles.

```text
intervals = EmptyEndpoints(variables)
repeat at most 2 × Count(variables):
    proposals = ReduceBounds(Map(DeriveFrom(intervals), inequalities))
    additions = MissingEndpoints(intervals, proposals)
    if Empty(additions): stop
    intervals = Install(intervals, additions)
range = FirstFiniteUnboundRange(intervals, boundVariables)
return range
```

This is the finite-envelope fallback, not the complete source planner. Existing
joins and generators remain responsible for their own scopes and dependencies.
The scheduler consumes the returned range and resumes binding. Its cursor
enumerates the resulting plan and retains every original condition.
The [implementation](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_binding_guard/envelope.rs)
uses checked coefficients and bounds, charges analysis work and storage, and
retains the original scalar conditions. A conservative interval may propose
extra rows; it may not omit a satisfying substitution.

For `V` scoped variables, `C` inequalities and at most `E` expression nodes,
one fallback uses `O(EV + CV + V)` logical storage and at most `O(CV³)` endpoint
work. Existing binding steps run first, so already resolved scopes avoid this
analysis. These bounds describe the dense implementation, not the size of its
subsequent substitution family.

The distinctions matter to parallelism. A union of several worlds' relations
can offer shared source instances, but a join can combine atoms that coexist in
no individual world. Each world must therefore recheck positive truth. Optional
world-membership masks prune a prefix only if no current world satisfies it.
They do not prune future snapshots or the candidate carrier.

See [lazy checking](../rust/parallel.md) for the injected execution contract,
and [the theorem map](../lean/theorems.md) for the corresponding coverage laws.
