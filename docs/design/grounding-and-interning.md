# Term identity and grounding

Grounding, candidate generation and reduct checking compose exact joins,
selections, masks, reductions and fixed-point operations. A shared execution
representation should make that composition explicit while preserving typed
logical values. The [architecture map](../book/architecture/alignment.md) connects
these operations to answer-set semantics.

This document describes the current representation and the obligations of a
solver-wide term-interning design. The latter is not yet implemented. Existing
atom IDs and relation dictionaries do not establish that all term payloads are
interned, nor that grounding runs on the GPU.

## Current representation

[`Value`](../../crates/zetesis-core/src/value.rs) represents integers, strings,
symbols, structural values and extrema. Strings and symbols carry text;
structural values retain a validated flat representation. An atom contains a
predicate signature and an ordered argument vector.

Several execution representations already replace larger objects with IDs:

| Representation | Meaning of an ID |
| --- | --- |
| [`AtomInterner`](../../crates/zetesis-core/src/atom_interner.rs) | A unique atom in one appendable owner |
| [`GroundProgram`](../../crates/zetesis-core/src/ground.rs) | An atom position in one static ground program |
| Formula theory | An atom or formula-node position in that theory |
| [`Relation`](../../crates/zetesis-core/src/relation.rs) | A value in one relation's equality dictionary, or a row occurrence |

These coordinate systems serve different purposes. A relation row is not a term,
and atom discovery does not establish truth. Candidate masks and reduct
evaluation already use compact coordinates; their meaning depends on the
associated owner and program.

Typed payload comparison remains in atom construction and lookup. The static
relational grounder searches a sorted atom catalog. Formula grounding uses the
appendable interner's per-predicate ordered indexes, and possible-support
relations have their own catalog lifetimes. A profile of one path does not
identify the cost of another. See the [grounding chapter](../book/architecture/grounding.md)
for the supported eager and lazy routes.

## Canonical term ownership

The proposed foundation is one authoritative owner for each admitted program's
typed terms, with stable compact references used throughout execution. Names,
numbers, constructor structure and ordered children remain available through
typed views. Source syntax and provenance remain themelios data.

The ownership contract must cover relational and formula grounding, bindings,
aggregate tuples, objectives, candidate construction, observations and retained
models. Extending only one lookup while retaining competing execution payloads
would leave the underlying composition problem unresolved.

Eager, lazy and hybrid grounding should share this vocabulary and the operations
on it: substitution, tuple lookup, joins and column selection. Their demand and
retention policies can differ without creating separate representations of a
logical term. This does not remove their distinct completeness obligations.

Required properties are:

- Equal typed terms have equal IDs within an owner; distinct terms do not alias.
- Appending terms preserves the meaning of every existing ID. Incomplete
  construction never publishes a partially formed term.
- Cross-owner use is checked or explicitly imported. Equal integer IDs from
  unrelated owners do not establish equal terms.
- Columns, postings, permutations and device buffers are derived views with
  explicit owner and snapshot identity. They do not own competing term payloads.
- Stable IDs do not make a prepared view current after discovery. Each view must
  remain tied to its old snapshot, extend with the owner, or be rebuilt before
  use with a newer generation. A completed-support certificate cannot silently
  cover subsequently discovered rows.
- Arithmetic reads numeric payloads and retains checked error behavior. An ID
  is not the number it names.
- Terms created during grounding or observation have a defined construction and
  publication phase. Freezing the owner immediately after parsing is insufficient.
- Storage accounting includes payload arenas, indexes, view storage, growth
  overlap and retained owners; reduced payload copying does not excuse uncharged
  index memory.

Canonical identity does not require every operation to use one physical order.
The current API distinguishes canonical storage order from ASP term order.
Ordered construction and lookup must use the same named comparator; discovery
order must never silently replace either semantic operation. Stable IDs can
coexist with derived ordered views. Renumbering requires an explicit checked
mapping of every affected atom, mask and formula reference.

## Performance questions

Interning can reduce repeated text comparisons, argument copying and retained
payload. Compact argument columns can also give CPU and GPU primitives common
inputs. These are hypotheses to measure across the complete pipeline, including
construction, indexing, conversion and output.

Same-owner term-ID equality is constant time. Constructing or hashing an atom
tuple still depends on its arity; interning a new compound visits its structure.
Semantic ordering may require payload comparison or a prepared rank. Neither
integer storage nor columnar layout alone demonstrates vectorization or a
GPU speedup.

The [scalability examples](../../examples/scalability/README.md) vary problem size
independently of worker count. [Einstein's Riddle](../../examples/einstein-riddle.lp)
provides a string-bearing, wide-join case. Its `solution/6` producer joins five
possible relations; current formula hybrid grounding still prepares that
producer eagerly. A larger work allowance permits an investigation but does not
remove the work. Measure its actual formula-grounding path separately from
synthetic relational joins.

Use the [measurement protocols](../book/reference/measurement-protocols.md) to
retain source and executable identities, complete result checks, refusals, phase
times and separate memory observations. The existing
[worker-scaling study](../book/reference/worker-scaling.md) describes the exact
older implementations named there; it is not evidence for later schedulers.

## Preservation obligations

The representation change needs encode/decode exactness, identity preservation,
append stability, both ordering contracts, substitution correspondence and
complete tuple-to-column correspondence. Transporting an interpretation through
the atom mapping must preserve original satisfaction and satisfaction of its
frozen reduct. Grounding completeness, objective tuple identity and observed
output remain separate obligations.

The [Lean correspondence](../book/lean/correspondence.md) records existing laws
and their implementation boundaries. Equality interning alone does not prove
the complete migration correct. Tests must also exercise owner mismatches,
reversed discovery order, word and column boundaries, structural terms,
resource refusal, parallel execution and retained results.
