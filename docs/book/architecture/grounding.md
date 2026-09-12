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

The final formula catalog owns one atom sequence and indexes it with dense IDs.
Lookup uses full typed atom equality; hashes alone never establish identity.
Insertion order fixes the IDs, and the table is never iterated to emit the final
sequence. Lookup has expected constant table work plus hashing and equality;
collisions can require a full scan of the catalog. The
[catalog](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground/atoms.rs)
reserves sequence and index capacity before publishing a new ID. Failed
reservations return a located `FormulaFailure::AtomAllocation`. Conservative
cumulative scalar-byte charges remain separate from actual live index capacity.

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

Ordinary positive atom heads with positive flat witnesses and pure scalar checks
or generators use [delta joins](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/delta.rs).
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

An ordinary producer without positive inputs runs once. Aggregate, conditional,
negative, structural and nonnormal producers retain full-round traversal. Every
selected input still passes the same typed tuple matcher and scalar evaluator.
An empty proposal set establishes completion only after every required variant
and conservative producer has finished. Final formula emission visits all
complete authored-body bindings, including those with false scalar filters;
support membership cannot conceal a required arithmetic error.

The [support builder](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support.rs)
returns `CompletedCatalog` only after an entire round adds no atom. Its
`CompletedSupport` view borrows the same authoritative catalog used by final
formula grounding. Intermediate snapshots have no completion capability, and a
round, work, value or storage failure returns an error before objective activation.
A finite snapshot during growth does not establish that value generation will
terminate.

Objective eligibility consumes this completed view. Ordinary acyclic producers
can supply a more precise absent/optional/required classification. Recursive
aggregate or conditional producers and their unresolved dependants use the
conservative relation: a covered atom is optional. Actual costs still test the
original model through the objective query, with weight, priority and complete
tuple resolved from one binding. No second aggregate evaluator or support loop
is needed for cyclic objectives.

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


### Relation rows and vector operations

A relation row is one complete typed tuple. Formula support's
[`SupportCatalog`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/relations.rs)
owns each possible atom once, with append-only, predicate-local row identities.
A sorted index of those identities supports membership checks without another
atom collection. The core relation owner retains its typed equality dictionary
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
formula emission and objective eligibility. Membership insertion shifts sorted
row IDs, not atoms. Typed comparisons, ID shifts, append copies and posting
construction consume the grounding work budget. Snapshot construction visits
predicates without revisiting their rows.
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
