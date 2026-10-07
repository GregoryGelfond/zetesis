# Interpretations and retained atoms

`Interpretation`, also available as `Model`, denotes a finite true-atom set.
Every absent atom is false. Constructing this value establishes neither
satisfaction nor answer-set membership. An ordinary session returns an
`AnswerSet` only after its membership engine accepts the interpretation.
Display selection never changes this identity.

## Catalog and selection

An `AtomCatalog` retains one immutable canonical store and an original-order
occurrence map. `AtomCatalog::new(Vec<Atom>)` consumes construction descriptions
and imports their typed contents; it does not adopt their cells or preserve
input addresses. Equal terms, subterms and complete atoms share canonical
identities. Repeated input atoms still retain separate occurrence positions.
A `Model` adds an ordered selection of those positions. Position validation,
semantic ordering and duplicate coalescing produce a true set independently of
the catalog's original order. Signed predicates, strings, symbols, numbers and
structured values retain their distinct identities.

There are three separate coordinates: a canonical identity within its owner,
an original occurrence in a catalog, and a selected true position in a model.
Raw IDs from different owners are not comparable. `AtomCatalog::same_owner`
checks the exact occurrence owner; `shares_terms` checks only the term authority.
Neither asserts the same truth selection. Borrowed `Atoms::same_occurrences`
and relation `Row::occurrence_in` identify an exact source occurrence mapping,
including duplicate positions, without a semantic search.

Admitted formula owners and eager `GroundProgram` values expose an `Atoms` view
and shared `atom_catalog()`. Formula answers and decoded static words select
these catalogs. Shared lazy rounds publish one catalog after their complete
no-delta scan; all completed worlds select that owner. Scalar closure publishes
its final truth over its workspace's canonical authority. Discovery and
publication alone do not make an atom true.

Cloning a model shares both the catalog and selected-position allocation. A
retained model remains valid after its source, graph or session is dropped. It
keeps the **entire published canonical prefix** alive, including unselected
atoms and interned subterms; it does not retain later append segments. A sparse
answer can therefore retain more payload than its selected atoms require.

The implementation is in
[`catalog`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/catalog.rs)
and [`model`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/model.rs).
One retained model representation serves execution and result consumers. Source
syntax and explicit output exports have their own ownership boundaries. Scoped
term workspaces can refer to this catalog without becoming model selections.

## Reusing semantic order

`ModelOrder::prepare_with` borrows one fixed catalog and prepares integer ranks
for its occurrences. Equal logical atoms have equal ranks. Increasing ranks
follow typed storage order, not discovery order or ASP arithmetic comparison.
`select_with` validates positions, orders them through the ranks and coalesces
duplicates, retaining the least supplied occurrence of each selected equivalence
class. It returns an ordinary `Model` sharing the original catalog; the model can
outlive the prepared order. No atom payload is copied or compared again during
selection.

For a catalog of size *n* and a selection of size *m*, preparation takes
O(n log n) semantic comparisons and retains two integer buffers: the ranks and
the occurrences in semantic order. Selection marks the supplied positions in an
*n*-bit mask and then chooses, from *m* and *n* alone and before either runs, one
of two strategies with identical results. A dense selection
(m·⌈log₂(m + 1)⌉ ≥ n) walks the semantic order once in O(n) integer steps,
keeping each rank's first marked occurrence. A sparse one sorts its marked
positions in O(m log m) integer comparisons. Temporary space is the mask plus
O(m) cells.
Both operations accept work/cancellation callbacks and metadata byte ceilings.
`ModelPublication` and `ModelPublicationFailure` retain actual peak metadata;
rejected reservation proposals are not counted as allocations. Catalog payload
and earlier models belong to their separate owners. See
[`ModelOrder`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/model/order.rs)
for the exact callback, capacity and final allocation boundaries, and the
[rank correspondence](../lean/correspondence.md) for the semantic assumptions.

## Building a catalog during grounding

`zetesis_core::atom_interner::AtomInterner` combines one canonical store with
committed and pending discovery maps. Lookup returns an existing discovery
position or a checked vacant entry. Insertion imports missing canonical
components before publishing the new discovery position. An inverse index maps
canonical atom IDs to discovery positions; a separate ordered view supports typed
enumeration and foreign input. Both retain positions and links, not another
collection of logical payloads.

During a synchronous round, `split` lends an immutable committed prefix and a
disjoint append capability. A source scan can borrow committed atoms while its
callback discovers new identities in the pending suffix. `commit_with` joins the
two regions only after those borrows end, preserving every dense position.
Canonical order is an ordered position view; it never renumbers the owner.
Committing an atom does not select it as true in any candidate interpretation.

`find_atom_with` and `find_key_with` borrow the owner immutably and allocate
nothing. An atom from this exact atom authority supplies its canonical ID;
otherwise a tuple with authenticated vocabulary coordinates uses the store's
existing collision-exact tuple index. A sparse inverse then finds its discovery
position through logarithmic integer probes. Canonical rows retained after a
refusal have no inverse entry until discovery succeeds. Shared vocabulary alone
does not authorize an atom ID from a different tuple writer.

`AtomAppender::find_pattern_with` looks up an admitted pattern directly against
a borrowed assignment. It authenticates the predicate, assignment scope and
referenced slots, including constant arguments. It neither copies an argument
vector nor examines unrelated assignment cells. This selected-slot contract is
distinct from the whole-frame check performed by `AssignmentSlice::bind_with`.
Even a nullary pattern requires a valid assignment scope. A discovered position
is still not evidence that the atom is supported or true.

`pattern_lookup_bytes` reports the immutable lookup's named owner and fixed
query-header envelope. Caller arrays and external owners are separate; insertion
scratch is not included. The lookup changes no owner state, including on
cancellation or a callback panic.

Foreign and ingress queries use the typed-order view: one iterative AVL tree per
predicate, kept in predicate order. This view also supplies semantic enumeration
and new insertion placement; numeric IDs do not define term order. An entry
requiring ordered placement first compares against the relation's last atom.
A greater atom uses the checked right spine. Cold preparation reads and retains
each of its `h` nodes once, charging two work permits per node plus reservation
work. A completed increasing insertion keeps the published spine in that same
mutation buffer. The next increasing insertion can reuse it after one checked
certificate comparison. The sparse inverse independently requires the incoming
canonical ID to exceed its own maximum; typed tuple order alone cannot establish
this for a reused closed catalog.

AVL planning remains unchanged. Before publication, the owner admits every
changed node, every cell moved by a rotation's spine repair, every changed suffix
reset and the replacement certificate. Both indexes and the discovery map then
publish together without callbacks or allocation. Any operation that repurposes
a path revokes its certificate before fallible preparation; refusal or unwind
cannot leave tentative scratch certified. Other vacant entries record their
semantic descent in two target-sized words and replay links without repeating
comparisons. Occupied entries do not change mutation scratch. The certificates
add only fixed owner metadata; both indexes still contain one node per discovered
atom, and the inverse needs no array spanning undiscovered canonical identities. These paths share the same checked insertion and publication in
[`AtomAppender`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/catalog/interner.rs).

`AtomAppender::order_selected_with` orders a unique selection of committed or
pending discovery positions through that same semantic index. It validates the
selection, marks it in a temporary bitset, and filters the index's ordered
traversal into a temporary position vector. All replacement writes are admitted
before the caller's selection changes. Refusal leaves that selection unchanged;
no atom payload or permanent ordering index is added.

`selected_order_storage` offers a prospective named storage envelope for dense
selections: at least two positions, with the indexed population no more than
twice the selected population. A caller can retain a sparse sorting strategy
when that offer is absent or its scratch would not fit. The offer excludes the
caller-owned selection and other live storage; the caller subtracts those from
the available allowance. Once execution begins, allocation, work and cancellation
failures propagate. `ordered_ids_with` remains the whole committed-order view.

Fallible reservations and work checks
precede publication, so a stopped insertion changes neither membership nor old
links. Complete canonical components and acquired capacity can remain after a
later discovery refusal; they assert neither discovery nor truth. The builder's
byte measure includes canonical payload, indexes, current snapshot directories,
discovery maps and scratch with conservative growth overlap. The bounded local
direction record, caller frames, allocator bookkeeping and Arc counters are
separate. This named-capacity measure is not process RSS.

This unique-builder contract is deliberately narrower than `AtomCatalog::new`.
The public immutable constructor can retain duplicate dense slots in arbitrary
order. Consuming an interner transfers its discovery map and shares the sealed
canonical prefix with the immutable catalog; it releases construction indexes. Positions
belong to their owner and must not be compared across unrelated catalogs as
semantic identities. The
[`implementation`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/catalog/interner.rs)
documents the entry, borrowing, allocation and final-transfer contracts.
The bounded
[`atom_interning` example](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/examples/README.md#appendable-atom-interning-probe)
checks typed identities and append rounds, and reports work and storage separately.

## Sharing a closed catalog

`AtomInterner::into_closed_with` consumes a writer into a `ClosedCatalog`, retaining
its canonical rows, vocabulary and exact lookup index. `for_closed_catalog`
creates an independent writer over that shared base. Its discovery set starts
empty; later discoveries may select an existing base row or append a new row
using the closed vocabulary. Each writer has a fresh atom identity scope. A
snapshot retains row payload without retaining the construction index.

This is a storage operation. A stored atom is neither a discovered atom nor a
true atom. In particular, a caller extending a selected interpretation must
discover its selected atoms explicitly; it must not select the entire base.
Vocabulary cannot grow after closure, and a descendant cannot be closed again
to create chains of storage overlays. `max_atoms` continues to bound discoveries,
independently of the number of stored base rows.

`CloseFailure` retains the actual named peak and the typed cause. The caller
accounts for external owners separately. `prior_publication_metadata_bytes`
authenticates a publication from the original writer and reports only its
independent metadata. Equal content or a shared vocabulary is insufficient to
deduct shared storage. See the
[`closed-catalog API`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/catalog/interner/closed.rs)
and [`lookup correspondence`](../lean/correspondence.md).

## Borrowing and explicit copies

`model.atoms()` returns `ModelAtoms`, an immutable semantic collection view.
It replaces the earlier concrete `&BTreeSet<Atom>` boundary. The view provides
`iter`, `len`, `is_empty`, `contains`, `get`, `first` and `last`. Its iterator is
canonical, double-ended, exact-size and fused. Iteration yields `AtomRef` by
value; its predicate and arguments yield `PredicateRef` and `TermRef`. These
small references borrow an immutable owner and contain no copied payload.
Equality, ordering and hashing use logical contents across independent owners.
For references carrying canonical identities in the same scope,
`TermRef::equals_ref_with`, `PredicateRef::equals_ref_with` and
`AtomRef::equals_ref_with` decide equality
from authenticated identities after one caller-work check. Across vocabularies
they compare typed contents.
Use these operations for equality tests; an ordering comparison must still
inspect unequal values because identity numbers do not encode semantic order.

`PatternTerms::unify_with` matches whole arguments against a caller-owned binding
frame. It reuses checked typed equality and borrows captured terms. The caller
selects the predicate and reserves the undo trail; the operation allocates
nothing. Mismatches and interrupted attempts can leave prefix captures, so the
caller must undo that trail suffix before trying another row. Only a successful
match establishes complete arity. Matching arguments does not establish truth.

Use `model.clone()` to retain an interpretation cheaply. Cloning `ModelAtoms`
only copies its borrows and cannot extend the owner's lifetime. A caller needing
an independently owned construction description can call
`AtomRef::to_atom(ValueLimits)` or `TermRef::to_value(ValueLimits)` and handle its
explicit refusal. These are export boundaries, not execution lookup adapters.
`TermRef::nodes` reads descriptors without a flattened copy; `write_with` spells
a value with checked work and a separate frame-storage allowance. The output
sink owns its allocation policy, and refusal can leave an output prefix.
The [session example](sessions.md) retains models and compares their identities.

## Scoped term workspaces

The public `zetesis_core::catalog` APIs also support term processing without
constructing a logical program. `VocabularyBuilder::new(max_storage_bytes)`
creates an append authority for typed terms, predicates and constructor shapes.
Use `import_term_with` for construction descriptions or foreign borrowed values,
`construct_term_with` for children already in that vocabulary, and the checked
predicate/constructor declaration methods for metadata. `finish_with` seals a
`Vocabulary`; cloning it shares its immutable payload and indexes. These
operations do not discover atoms or select truth. See the
[`vocabulary API`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/catalog/vocabulary.rs).

`CatalogRead` resolves a particular canonical prefix. Its term-only counterpart,
`TermRead`, also resolves derived terms. A `TermKey` retains an identity witness,
not the payload: resolving it requires a live compatible reader containing that
term. Foreign keys and terms newer than the reader's prefix produce typed
errors. A read borrowed from a mutable builder or arena must end before another
append; retaining a key does not retain that read borrow.

Use `read.assignment()` to create a `TermAssignment` of optional variable slots.
Its checked resize, set and copy operations retain IDs under one scope witness;
an empty slot denotes an unbound variable. `assignment.as_slice().bind_with`
validates the selected slots against a live reader and returns a borrowed
`BindingView` without copying values. This validation visits every selected
slot. `TermSet` separately records explicitly selected roots; interning a child
does not make it a domain member, and ID order is not semantic term order.
`read.predicate_mask_with(decide)` decides every predicate of the read prefix
once, in identity order, and packs the decisions into a `PredicateMask`, one bit
per predicate. `mask.decision(predicate)` answers a canonical predicate of that
vocabulary admitted before preparation; owned ingress, another vocabulary or a
later predicate has no decision, so the caller keeps its general procedure.
`AtomIdentityMap` records values by the owner-scoped identity of canonical atoms,
for any number of atom owners: a lookup hashes the owner's identity and one
identity word rather than the atom's structure, so its cost does not grow with
the number of owners. `retain_held` drops the owners nothing outside the map
still holds, whose atoms can no longer be presented. Equal atoms of different owners are different
keys, and owned ingress or carrier atoms are never recorded; callers that need
structural identity keep a structural index as the authority.
The [`scoped metadata API`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/catalog/terms.rs)
documents each frame's storage allowance and stopped-operation behavior.

`AtomAppender::insert_pattern_with` discovers an atom from a canonical pattern
and a scoped assignment. It validates the predicate, constants and selected
variable slots, preserving argument order and repetitions. One checked borrowed
projection serves discovery lookup and canonical row admission, including new
rows. It reads the immutable pattern and assignment without copying their IDs
into an argument vector. Per-argument logical bounds apply even when the atom
already exists. A refused discovery can leave a complete canonical row for a later
retry, but establishes neither discovery nor support membership.

For repeated instances of an admitted pattern, `prepare_pattern_with` returns
a `PreparedPattern` bound to the exact atom writer and fixed term limits. It
borrows the source template and checks its constants once. Each
`insert_prepared_pattern_with` call still checks the assignment, selected
variables and resource bounds before using the same canonical insertion path.
The capability owns no argument copy or lookup cache; its header counts in the
caller's retained metadata. Construction input uses the general insertion path.

For a head drawn from already matched immutable relation rows,
`prepare_row_pattern_with` binds the same pattern to exact `Relation` owners and
explicit `RowColumn` projections. `PreparedRows` takes the caller's metadata
vectors without copying tuples; `storage_bytes()` reports its header and actual
capacities for that caller's lease. Preparation examines O(a² + b) metadata for
a head of arity a and b inputs, with no scan of the relations' row population.
Insertion authenticates the selected row owners and uses their original canonical
occurrences directly. Finite term limits still apply before lookup, including
occupied rows. Maximum `usize` limits need no measure scan: canonical admission
already checked nodes, depth and the combined encoded and rendered byte length
against limits representable by `usize`. The capability proves coordinate access,
not join completeness, injectivity, freshness or truth. Canonical collision checks,
discovery indexes and their resource refusals remain the shared insertion path.

For authenticated canonical inputs, the exclusive insertion entry retains its
exact row lookup for publication. Index allocation may move storage without
changing that lookup's meaning. The fixed projection and prepared-result headers
count as named scratch alongside the owner; their borrowed inputs retain their
existing ownership. A retry performs a fresh lookup; it does not reuse a decision
left by a failed insertion.

For evaluation over existing immutable owners, `DerivedTerms::new_with`
registers their `CatalogRead` prefixes without copying payload.
`borrow_with` registers an input term in the arena's fresh identity scope;
`scalar_with` creates numbers or extrema, and `construct_with` combines local
child slots using a declared input constructor. Strings and named values must
come from registered canonical inputs. Equal typed contents coalesce even when
registered through different inputs or constructed locally. The arena borrows
its input owners, and its `TermRead` borrows the arena. It does not extend those
input owners or their atom populations. See
[`DerivedTerms`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/catalog/storage/derived.rs).

`catalog::Limits` bounds each newly constructed logical value, including repeated
child occurrences. The builder or arena's named-storage ceiling is separate.
Vocabulary receipts include retained canonical payload and indexes; derived-arena
receipts include its own metadata, generated nodes and reservation overlap but
exclude borrowed input owners. Assignment storage has its own bound. Callers
combining these capabilities must account for their simultaneously live owners
and metadata; none of these receipts measures process RSS. Checked callbacks
admit work, and a refusal can leave complete interned components or reserved
capacity. These standalone capabilities do not require the source frontend's
private compilation or grounding machinery.

## Costs and limits

For `N` catalog atoms and `M` supplied selected positions:

| Operation | Work and ownership |
| --- | --- |
| Catalog construction | Imports descriptions into a shared term DAG and canonical atom rows; records original occurrences and a checked portable encoding measure. Work includes structure, text and exact interning probes. |
| Selection | Checks indices, performs `O(M log M)` typed atom comparisons and retains `O(M)` position cells. Equal logical atoms coalesce. |
| Model clone | Constant time; shares the selected owner and catalog without allocation. |
| Complete iteration | `O(M)` borrowed atom visits after duplicate removal; each yielded view checks prefix membership, and reading its contents resolves the canonical row through the immutable segment directory. |
| Atom traversal | Hashing, ordering, checked comparison and argument iteration resolve a canonical row once per operation; `Arguments::at` and `len` resolve it once per call. |
| Membership | `O(log M)` typed atom comparisons. |
| Model equality/order | Lexicographic comparison of selected atom values; identical selected owners compare immediately. |

Typed comparisons inspect predicate names, tuples and structured values. The
constant-space canonical preorder cursor can revisit ancestors, giving quadratic
work on deep combs; it is not a linear traversal guarantee. Checked comparisons
admit navigation and compared text. Semantic storage order and ASP term order
are separate operations.
`Model::new` consumes an atom iterator, sorts and deduplicates its values, and
selects its resulting catalog. It returns a typed construction or reservation
error. `Model::from_ordered` imports descriptions already in strict semantic
order without sorting; a debug build checks that producer precondition.
`from_ordered_catalog_with` instead validates an already-canonical catalog's
strict order under caller work and selection-storage bounds, without importing
payload again. `publish_ordered_catalog_with` performs the same checks and also
returns the actual selection-buffer peak with a refusal. Rejected capacity
requests and the unallocated final header do not count as observations. A
successful attempt's peak is `selection_bytes`; both measures exclude the
retained catalog and caller storage. This receipt lets a caller account for a
consumed failed attempt without enabling statistics.

`ModelAtoms::of_predicate` borrows one predicate's atoms as the contiguous range
they occupy.
`Model::from_positions` returns a typed invalid-position or selection-reservation
error without a partial model. Arc envelope allocations remain infallible.
Neither constructor implicitly grounds or solves a program.

`max_optimal_bytes` and `WorldViewLimits::max_bytes` use the common
[`ModelRetention`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/retention.rs)
ledger. It counts the portable encoding of each distinct occurrence catalog
once, including repeated and unselected entries, then adds a selected-position
record per retained model entry.
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

Canonical identities outside an occurrence map are excluded from that measure,
even when the catalog keeps their shared prefix alive. The measure also excludes
spare vector/hash capacity, owner-index entries, Arc and allocator overhead,
shared subject data and execution state. Replacement
admits the new retained family, not transient overlap with the still-live old
family. Successful reservations can leave capacity after an abandoned admission.
It is not RSS. `AtomCatalog::capacity` exposes occurrence-map capacity and
`Model::selection_capacity` exposes selection capacity. `AtomCatalog::storage`
measures named canonical-prefix retention, while `publication_bytes` and
`Model::selection_bytes` isolate metadata for composing shared-owner ledgers.
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

[`CanonicalCatalog`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/CanonicalCatalog.lean)
adds the two-stage occurrence-to-identity decode, normalized selection,
old-prefix preservation and owner transfer under explicit decoded agreement.
It also exhibits equal raw IDs denoting different interpretations under different
owners. These laws do not prove Rust interning uniqueness, pointer lifetimes,
checked comparison, allocation or snapshot publication.
