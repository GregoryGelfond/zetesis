# Interpretations and retained atoms

`Interpretation`, also available as `Model`, denotes a finite true-atom set.
Every absent atom is false. Constructing this value establishes neither
satisfaction nor answer-set membership. An ordinary session returns an
`AnswerSet` only after its membership engine accepts the interpretation.
Display selection never changes this identity.

## Catalog and selection

An `AtomCatalog` owns immutable typed atoms in their original dense index order.
Its constructor consumes a `Vec<Atom>` without copying, reordering or reallocating
its cells. A `Model` retains that catalog and a canonical selection of positions.
It validates every position, orders selected handles by complete atom identity
and coalesces duplicate logical atoms. Different catalogs and index orders can
therefore denote equal models. Signed predicates, strings, symbols, numbers and
structured values retain their distinct identities.

Admitted formula owners and eager `GroundProgram` values expose both their
original `atoms()` slice and shared `atom_catalog()`. Formula answers and decoded
static words select those catalogs without cloning atoms. A batched lazy closure
freezes its catalog after the complete no-delta round, then all completed worlds
select that same owner. Source carriers can grow before that publication point.
Scalar closure transfers its owned consequences into the same model representation.

Cloning a model shares both the catalog and selected-position vector. A retained
model remains valid after its source, graph or session is dropped. It keeps the
**entire catalog** alive, including unselected atoms. This amortizes atom ownership
across a family, but a lone sparse answer may retain more payload than a separate
copy containing only its true atoms.

The implementation is in
[`zetesis-core::model`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/model.rs).
The library has one retained model representation; it does not construct a second
hidden tree for observation or interoperability.

## Borrowing and explicit copies

`model.atoms()` returns `ModelAtoms`, an immutable semantic collection view.
It replaces the earlier concrete `&BTreeSet<Atom>` boundary. The view provides
`iter`, `len`, `is_empty`, `contains`, `get`, `first` and `last`. Its iterator is
canonical, double-ended, exact-size and fused. Borrowed atoms refer to their
original catalog entries. Equality and ordering compare logical atom sequences,
not catalog addresses.

Use `model.clone()` to retain an interpretation cheaply. Cloning `ModelAtoms`
only copies its borrows and cannot extend the owner's lifetime. A caller needing
an independently owned standard collection can explicitly collect
`model.atoms().iter().cloned()` into a `BTreeSet<Atom>` or `Vec<Atom>`; that copies
selected payloads. The [session example](sessions.md) instead retains models and
compares their logical identities directly.

## Costs and limits

For `N` catalog atoms and `M` supplied selected positions:

| Operation | Work and ownership |
| --- | --- |
| Catalog construction | Moves the vector and traverses atom/value descriptions once to record a checked canonical payload size. No atom payload is copied. |
| Selection | Checks indices, performs `O(M log M)` typed atom comparisons and retains `O(M)` position cells. Equal logical atoms coalesce. |
| Model clone | Constant time; shares the selected owner and catalog without allocation. |
| Complete iteration | `O(M)` borrowed atom visits after duplicate removal. |
| Membership | `O(log M)` typed atom comparisons. |
| Model equality/order | Lexicographic comparison of selected atom values; identical selected owners compare immediately. |

Typed comparisons can inspect predicate names, tuples and structured values.
`Model::new` consumes an atom iterator, sorts and deduplicates its values, and
selects its resulting catalog. It remains an infallible allocation door.
`Model::from_positions` returns a typed invalid-position or selection-reservation
error without a partial model. Arc envelope allocations remain infallible.
Neither constructor implicitly grounds or solves a program.

`max_optimal_bytes` and `WorldViewLimits::max_bytes` count canonical retained
payload, including every referenced catalog atom and the selected-position
record. They conservatively count a shared catalog once per retained model.
Sparse models can consequently reach these limits earlier than a selected-only
payload measure would suggest. The catalog records its checked size once;
subsequent retention checks use that summary. Overflow remains a refusal.

This byte measure excludes spare vector capacity, Arc and allocator overhead,
shared subject data and execution state. It is not RSS. `AtomCatalog::capacity`
and `Model::selection_capacity` expose retained vector capacities separately.
No universal memory or solve-time improvement follows from sharing alone.

## Representation argument

The selected true set contains exactly atoms decoded at selected positions.
Sorting and duplicate removal preserve that set. Changing unselected catalog
entries does not affect truth. Renumbering positions preserves the interpretation
when each replacement position decodes to the same original atom.

[`ModelSelections`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ModelSelections.lean)
formulates these laws. Runtime index checks, canonical Rust comparisons, Arc
lifetimes and resource accounting remain separate refinement obligations.
These representation laws do not establish answer-set membership themselves.
