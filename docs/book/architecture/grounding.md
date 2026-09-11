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
enclosing rule and its written element occurrences. It uses the pinned themelios
parser and original coordinates, and refuses correspondence it cannot establish.
In particular, merging two whole rules must not combine their separate counting
groups. This source preservation supports subsequent lowering; successful
correspondence checking does not prove the lowering's answer-set semantics.

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

An atom in a **possible support relation** is a witness available to source
enumeration. It is not thereby true in a candidate, and an aggregate's proposed
result is not thereby its evaluated result. The emitted formula must retain the
original activation and equality conditions. Structured positive witnesses can
bind local variables before dependent arithmetic is checked. Positive-witness
matching does not invert arithmetic or introduce a global guessed value universe.

### Relation rows and vector operations

A relation row is one complete typed tuple. Formula support's
[`RelationRows`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support.rs)
assigns append-only, predicate-local row identities. Each argument column has a
`BTreeMap<Value, Vec<usize>>` from typed values to those row identities. For a
positive witness, the selector chooses the shortest posting list supplied by
known whole-column equalities. The matcher then checks the complete tuple,
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

Row identity connects relational semantics to masks, intersections and gathers.
Combining two column masks means intersecting positions in the same relation
snapshot; it must not combine values from different tuples. Ordinary source
indexes still gather values from complete atoms. The separate bounded
[`relation` library](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/relation.rs)
provides an immutable column view for primitive experiments. It borrows the
original atoms and encodes complete typed values through one equality dictionary.
It preserves row occurrences and their order, including duplicate tuples, and
keeps original catalog indices distinct from local positions. Explicit predicate
arity and row count distinguish an empty relation from a nullary tuple.

Queries and selections borrow their exact relation owner. A selection validates
ordered positions; it does not establish complete grounding or answer-set
membership. Equality filtering is complete relative to its supplied input rows:

```text
query = ResolveEqualities(relation, known_values)
selected = Filter(AllEqualitiesHold(query), supplied_rows)
bindings = FilterMap(MatchWholeTuple(pattern, binding), selected)
```

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
This library view does not replace ordinary source storage. Production adoption
requires a coherent tuple owner and removal of redundant stores; an experimental
copy alone establishes neither a smaller memory footprint nor faster grounding.

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
