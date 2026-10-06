# Term identity and grounding

Grounding, candidate generation and reduct checking compose joins, selections,
masks, reductions and fixed-point operations. Canonical term storage gives these
operations a shared typed vocabulary. The
[architecture map](../book/architecture/alignment.md) connects the operations to
answer-set semantics; the [grounding chapter](../book/architecture/grounding.md)
describes their eager, lazy and hybrid schedules.

## Logical values and execution coordinates

themelios owns source programs, logical symbols and provenance. At the native
boundary, [`Value`](../../crates/zetesis-core/src/value.rs) and `Atom` provide
owned descriptions for import and export. Execution catalogs intern their text,
typed terms and complete atom tuples; they do not retain a second copy of the
imported `Value` or `Atom` payload.

The coordinate systems remain distinct:

| Coordinate | Meaning |
| --- | --- |
| [`TermKey`](../../crates/zetesis-core/src/catalog/terms.rs) | A typed term in a particular vocabulary and readable prefix |
| Canonical atom identity | One signed predicate with its ordered arguments in an owner |
| [`AtomCatalog`](../../crates/zetesis-core/src/catalog.rs) position | An occurrence in a published catalog, which may contain duplicate atoms |
| [`GroundProgram`](../../crates/zetesis-core/src/ground.rs) or formula atom position | An atom coordinate in that execution program |
| [`Relation`](../../crates/zetesis-core/src/relation.rs) dictionary ID | A whole typed value in one equality dictionary |
| Relation row position | An original tuple occurrence, including repetitions |

A numeric ID is neither an arithmetic value nor an ASP ordering rank. Atom
discovery establishes identity and possible support, not truth. Candidate masks
and formula references acquire meaning only through their associated program.

## Ownership and publication

[`Catalog`](../../crates/zetesis-core/src/catalog.rs) supplies an append authority
and immutable read snapshots. Equal typed terms share an identity within that
authority. Strings and symbols remain distinct, compound children retain their
order, and complete atom identity includes the signed predicate and arguments.

Appending preserves all existing identities. Failed construction may retain
complete interned components and reserved capacity, but publishes no partial
term or atom. A snapshot retains a fixed prefix of sealed storage; later appends
cannot change its contents. Published occurrence maps share that storage without
retaining the writer or its lookup indexes.

[`AtomInterner`](../../crates/zetesis-core/src/catalog/interner.rs) adds unique
atom discovery order and typed lookup. Its committed prefix and append capability
can be borrowed separately during a grounding round. New discoveries become
visible at the next commit. The typed ordered index stores identities rather
than another atom collection.

[`TermAssignment`](../../crates/zetesis-core/src/catalog/terms.rs) stores optional
term IDs under one vocabulary witness. Read and assignment operations check the
owner and accessible prefix. Foreign values require typed comparison or explicit
import; coincident integer IDs from unrelated owners do not establish equality.
Independent programs need not share an interner.

## Grounding views

Formula grounding's
[`SupportCatalog`](../../crates/zetesis-themelios/src/formula_support/relations.rs)
holds one evolving atom authority. Bindings, relation membership and pending
discoveries refer to it through scoped coordinates.
[`Computation`](../../crates/zetesis-themelios/src/formula_support/computation.rs)
separates term construction from immutable relation membership: computing a term
does not assert an atom.

The [publication boundary](../../crates/zetesis-themelios/src/formula_support/publication.rs)
shares sealed source storage across final views and transfers occurrence maps.
Closing support can retire its query and possible-truth indexes while retaining
the canonical base needed for subsequent operations. In particular, terminal
definition reconstruction starts from each answer's true rows; the shared store
never substitutes possible support for that answer's interpretation.

Eager, lazy and hybrid paths share typed binding, matching and relation
operations. Their discovery, retention and completeness policies still differ.
The static normal-program profile also uses canonical catalogs, but materializes
its admitted carrier and filter-valid rules. Sharing storage does not make every
schedule demand-driven.

## Columnar execution

A core relation view preserves the signed predicate, row order and duplicate
occurrences. Its dictionary refers to whole source terms; argument columns hold
local equality IDs. Formula support retains these columns, and equality postings
for the columns a join can bind, across growth rounds. Queries bind to a particular immutable view;
stable IDs alone do not make a query valid against a later snapshot.

For a positive body occurrence, known equalities select a posting list. The
whole-tuple matcher still checks repeated variables, structured terms and the
remaining conditions. An equality mask is a selection, not a complete match or
an answer-set certificate.

The [GPU relation primitive](../../crates/zetesis-wgpu/src/relation/mod.rs) uses
the same local equality dictionary and returns masks over original row
occurrences. Its uploaded columns are derived execution data. Typed payload
remains with the source owner. This shared representation does not imply that
all grounding operations execute on the GPU; each caller's schedule determines
the actual host and device work.

## Domain analysis and arithmetic

[`zetesis-domain`](../../crates/zetesis-domain/src/lib.rs) analyzes the exact
borrowed themelios program. Its finite argument domains are conservative upper
bounds over source symbols; `Unknown` permits every symbol. An abstract fixed
point proves neither finite grounding nor precise correlations between arguments.
These borrowed source bounds are analysis results, not another execution term
store.

The grounding layer separately derives
[finite integer envelopes](../../crates/zetesis-themelios/src/formula_binding_guard/envelope.rs)
for eligible binding scopes. The original guard still filters the proposed rows.
This is bounded coverage analysis, not a general integer constraint solver.
Arithmetic resolves numeric payloads and preserves checked failure behavior;
term IDs cannot be used as operands. Relational comparison follows ASP term
order and must not be narrowed to numbers merely because it uses `<` or `>`.

## Costs and correctness boundaries

Canonical identity avoids repeated payload comparison when two authenticated
keys suffice. The interner's and the relation catalogs' ordered indexes first
compare an arrival with their last entry in order: rows derived in order, as a
rule's heads usually are, are placed after it with one typed comparison instead
of one per tree level; any other arrival takes the full search. Constructing an atom still depends on its arity, importing a new
compound visits its structure, and semantic ordering can require typed content
comparison. Columnar storage alone establishes neither vectorization nor a
speedup. Measure construction, indexing, execution and output together using the
[measurement protocols](../book/reference/measurement-protocols.md).

Storage limits account for named payload, index, view and workspace capacities,
including admitted growth overlap. Immutable owners can be shared by several
results; summing each view's retained base would count that storage repeatedly.
These capacity measures are not process RSS or allocator bookkeeping. Exceeded
limits and incomplete construction remain explicit failures, never evidence of
unsatisfiability.

The preservation obligations are encode/decode exactness, owner isolation,
append stability, typed ordering, substitution correspondence and complete
tuple-to-column correspondence. Transporting an interpretation through an atom
mapping must preserve satisfaction of both the original program and its frozen
reduct. Grounding completeness, objective tuple identity and observed output
remain separate obligations.

The [Lean correspondence](../book/lean/correspondence.md) states which laws and
implementation links are established. Canonical storage and regression tests do
not by themselves prove the complete solver correct.
