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
static words select those catalogs without cloning atoms. Shared lazy rounds
freeze one catalog after their complete no-delta scan; all completed worlds
then select that same owner. Source carriers can grow before that publication
point. Scalar closure transfers its owned consequences into the same model representation.

Cloning a model shares both the catalog and selected-position vector. A retained
model remains valid after its source, graph or session is dropped. It keeps the
**entire catalog** alive, including unselected atoms. This amortizes atom ownership
across a family, but a lone sparse answer may retain more payload than a separate
copy containing only its true atoms.

The implementation is in
[`zetesis-core::model`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/model.rs).
The library has one retained model representation; it does not construct a second
hidden tree for observation or interoperability.

## Building a catalog during grounding

`zetesis_core::atom_interner::AtomInterner` owns distinct atoms in first-insertion
order. Lookup returns the existing local position or a checked vacant entry;
only inserting a vacant entry materializes an atom. Its index contains integer
positions and links, rather than a second collection of atom keys. Complete typed
identity still decides equality. The index is an execution representation, not
an alternative meaning for an atom.

During a synchronous round, `split` lends an immutable committed prefix and a
disjoint append capability. A source scan can borrow committed atoms while its
callback discovers new identities in the pending suffix. `commit_with` joins the
two regions only after those borrows end, preserving every dense position.
Canonical order is an ordered position view; it never renumbers the owner.
Committing an atom does not select it as true in any candidate interpretation.

The builder uses an iterative AVL index. `find_atom_with` and `find_key_with`
borrow the owner immutably, allocate nothing, and return a local position or
absence after a checked search of committed and pending identities. Occupied
entries use that same probe without changing retained mutation scratch. Entry
records each descent in two target-sized words and a checked length, local to
lookup and path preparation. The AVL height bound makes this record sufficient
for every representable node population. A vacant entry replays those directions
through the exclusively borrowed tree to prepare its insertion path, without
repeating typed comparisons. Each search visits a logarithmic path, charging the
actual predicate and value prefixes compared. The node-work unit includes child
selection and fixed local direction recording; replay admits each node visit and
path-step write separately. These are operation units, not machine instructions.
Canonical traversal visits the index once. Fallible reservations and work checks
precede publication, so a stopped insertion changes neither membership nor old
links. Already acquired capacity can remain after failure. The builder's byte
measure covers its documented vector/index/scratch capacities and conservative
growth overlap. The bounded local direction record and borrowed-view stack
headers are excluded; nested atom payload and process RSS are separate measures.

This unique-builder contract is deliberately narrower than `AtomCatalog::new`.
The public immutable constructor can retain duplicate dense slots in arbitrary
order. Consuming an interner transfers its completed vector into that existing
representation; it does not introduce another retained-model type. Positions
belong to their owner and must not be compared across unrelated catalogs as
semantic identities. The
[`implementation`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/atom_interner.rs)
documents the entry, borrowing, allocation and final-transfer contracts.
The bounded
[`atom_interning` example](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/examples/README.md#appendable-atom-interning-probe)
checks typed identities and append rounds, and reports work and storage separately.

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

`max_optimal_bytes` and `WorldViewLimits::max_bytes` use the common
[`ModelRetention`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/retention.rs)
ledger. It counts each distinct catalog allocation once, including every
unselected atom, then adds a selected-position record per retained model entry.
Cloned selections still count per entry. Equal-content catalogs from separate
allocations remain separate owners; logical equality never establishes sharing.
World-view collection adds one optional score record per answer. Optimal-tie
retention adds its one best-score record once for the entire tied family. A score
record has a one-byte option tag; a present score adds a u64 length and one
i32/i64 pair per priority. Even an empty selected interpretation has a length
record and retains its catalog.

Catalog construction records its checked canonical size once. Retention reads
that summary and uses a private hash index with expected amortized constant-time
owner lookup (linear worst case). The index retains the actual catalog handles,
preventing allocation-address reuse while an identity is registered; it copies
no atom payload. `admit` and `replace` return exclusive pending admissions. A
consumer reserves its own entry slot before committing the ledger and publishing
the entry. Failure or abandonment preserves the previous charge and owner set.
A better incumbent replaces the old family only after its complete new payload
is admitted. Scoring work and verified/scored counts are not undone by a storage
refusal. The standalone `Model::retained_payload_bytes` still describes one model
in isolation; summing it does not account for sharing.

This byte measure excludes spare vector/hash capacity, owner-index entries,
Arc and allocator overhead, shared subject data and execution state. Replacement
admits the new retained family, not transient overlap with the still-live old
family. Successful reservations can leave capacity after an abandoned admission.
It is not RSS. `AtomCatalog::capacity`
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

[`AtomCatalogs`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AtomCatalogs.lean)
adds the split/commit correspondence and preservation of an old selection during
discovery. Unique local IDs require complete atom uniqueness; the laws do not
assume that arbitrary immutable catalog inputs satisfy that extra premise.
