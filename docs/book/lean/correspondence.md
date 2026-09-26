# Connecting proofs to implementations

The reduct is the common semantic foundation. Connecting it to an executable
solver requires several separate arguments; a theorem at one level cannot
silently stand for all of them.

| Layer | Existing mathematical or executable object | Additional correspondence required |
| --- | --- | --- |
| Semantic definition | Predicate interpretations, rules, formula trees, reduct and minimality | Relate the intended source language to these definitions |
| Source transformation | Laws over finite bindings, eligible tuples and complete families | Prove concrete cursor coverage, local scope, provenance and translation |
| Evaluation | Least-consequence laws and frozen-mask equivalence | Relate Rust DAGs, dense IDs and packed membership to their denotations |
| Scheduling | Independent-world and completed-batch laws | Prove actual buffers, snapshots, submission identities and commits implement that schedule |
| Machine execution | Checked Rust behavior and qualified WGSL paths | Establish executable refinement, arithmetic and device semantics |
| Observation | Semantic coverage and delivery laws | Connect actual output writes and counters to the retained semantic evidence |

## Keyed constraints and checked source arithmetic

`KeyedConstraints.one_value` and `asked_constraints_preserve` justify replacing
constraints that reject the same answer sets under an established keyed-relation
property. `digit_carry_asked` proves the digit/carry equation over mathematical
integers with truncating division. These laws do not establish the source
compiler's type checks, `i32` arithmetic, evaluation reachability or diagnostics.

The Rust recognizer in `formula_keys` requires an independent interval proof
for every arithmetic operation of the original constraint, including retained
body terms. Fact-only relations or fact-only conditions of keyed values provide
numeric bounds; source comparisons do not. Every intermediate must fit `i32`,
and the digit/carry dividend must be numeric. An incomplete or stopped proof
leaves the checked source constraint in place. This protects both required
refusals and the exclusion of substitutions where no arithmetic is reached.
The concrete interval analysis and its connection to all possible source
bindings remain Rust obligations, covered by paired rewritten/unrewritten
tests; they are not an executable Lean refinement.

## Arithmetic families

[`ArithmeticFamilies`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ArithmeticFamilies.lean)
specifies admission from the classified outcomes of a complete substitution
family. Empty joins are admissible and silent. A defined but false substitution
is a valid definedness witness. Fatal evaluation failures cannot be rescued by
other substitutions. Reordering or revisiting the same outcomes preserves both
admission and warnings.

The counterexamples state two required boundaries: an unfinished prefix cannot
establish that a family is entirely undefined, and combining families from
different outer bindings can conceal a local refusal. Rust must establish
complete original-family coverage, joint expression definedness, correct
zero-divisor classification and those local scopes. Normalized fragments must
preserve their original family identity; sharing a diagnostic span is not
enough. These laws do not certify the source walker, its work limits or checked
arithmetic. An arithmetic fault and a missing positive fact are different objects.

[`ScalarArithmetic`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ScalarArithmetic.lean)
specifies checked scalar values and faults, and its `evaluate` plan stops at the
first fault. The source evaluator shares the scalar operations but has an
additional traversal policy: after a numeric zero divisor it checks independent
DAG branches within the reached evaluation phase, refuses a fatal error in one
of them, and skips operations whose
operands are undefined. The strict first-fault plan does not prove that traversal
or its fatal-error precedence. `ArithmeticFamilies` starts after each instance
has been correctly classified; neither module proves the concrete classifier.
Body-before-head and condition-before-consequent staging remain separate Rust
obligations: omission in an earlier phase does not invoke a later phase merely
to search for another fault.
Closed-term preparation and post-solve observations retain strict evaluation.

## Ownership and execution correspondence

An ownership refactor can preserve the semantic theorem statements. Its proof
obligation is then to preserve the denotation and evidence through the changed
representation or transition, rather than to define another answer-set relation.

The formula grounder's consuming phases retain atom/node/root order, origins,
activated objectives and the original cumulative budget. Discarded support and
construction indexes have no later semantic consumer. The ID-only atom catalog
requires lookup soundness, lookup completeness and stable insertion IDs under
full typed equality. The dictionary laws below express the lookup contract
abstractly; checked typed comparison, AVL ordering and balancing, the bounded
direction record and its exclusive-tree replay, transactional publication and
fallible Rust allocation remain executable obligations. The Rust lookup module
states the height bound and replay argument; the abstract dictionary laws do not
prove that concrete AVL representation.

`SourceContributions.chain_unique` identifies the ordered observations of a
finite chain in shared storage. `chain_preserved` shows that changing other
entries leaves those observations unchanged. The append-log laws retain each
atom's producer order, including duplicate occurrences, and isolate contributions
to distinct atoms. The formula grounder's shared metadata must additionally
establish valid owner-local links, exact iterator lengths, sorted unique source
locations and correspondence between producer chains and the append log.
Those Rust mutation, allocation and accounting obligations are not proved by
the abstract chain laws. Preserved producer and origin sequences are consumed
at the same support-guard construction phase; the original theory and reduct
remain the semantic authorities.

Device context identity, logical subject identity and submission identity remain
separate. Sharing a context changes resource and failure ownership, not a
candidate's frozen reduct. A busy refusal starts no device query. Context-wide
invalidation prevents a later primitive from reusing failed execution state.
`LazyRounds` and `BatchAccounting` supply the existing semantic round and
coverage laws; context leases, scope cleanup and their concrete Rust transitions
remain executable correspondence obligations.

Ordinary sessions can retain the same device through explicit execution
resources. Each start creates a fresh candidate stream, budget and outcome over
its original logical subject. The correspondence must preserve that independence under
sequential reuse, automatic CPU selection with supplied device resources and shared device failure.
The builder changes request composition; it does not change candidate coverage,
the frozen reduct or the point at which a checked result is committed.

`SessionBuilder::collect` consumes a fresh request, selects the unrestricted
original family and retains each checked answer once. `WorldViews` requires
original-answer coverage, completed accounting and complete capture; its prefix
law applies to interruptions and retention refusals. A selected optimum or the
remaining suffix of an already consumed session cannot replace that family.

`ProjectedAnswers.covered_key_image` identifies the key image represented by a
selected family. `selected_property_survives` preserves answer-set membership
and established optimality because representatives remain selected original
answers. These laws assume a fixed key and complete representative coverage.
`retain_covers` and `retain_unique` establish the coverage and uniqueness
invariants for one exact identity decision over an already consumed prefix.
They do not prove source `#project` compilation, concrete history lookup,
allocation, cancellation or receipt updates. `WorldView` collection retains
full identity and does not use projected representatives as a complete family.

A shared `GpuFormulaProfile` retains one exact compiled pipeline, context and
gate projection. Each oracle starts with fresh residency, epochs and execution
counters. This does not change the masked satisfaction or frozen-reduct laws.
Pipeline identity, capability checks, initialization and device execution remain
Rust/WGSL correspondence obligations; existing Lean theorems do not certify
those handles or shader compilation.

The `zetesis-solve` boundary owns semantic sessions and outcomes. The CLI
publishes their checked values through the same public API. Its error adapter
moves the original cause and evidence; a presentation value cannot create a
solver conclusion. Crate extraction changes neither the mathematical subject
nor the required preservation laws.

Typed execution observations describe attempted setup or execution choices.
They are not membership or coverage receipts. A failed observer can stop driving
the session but cannot reinterpret its error as a device failure, request
fallback or establish a conclusion. The observation boundary must preserve
already checked evidence and distinguish it from publication, as required by
`Outcomes`. Tests exercise these failure paths; they do not close the formal
refinement.

The ordinary driver's semantic snapshot is the authority for membership,
completion and incumbent evidence. Publication acknowledgements and timings
remain separate; successful and partial reporting values are derived views.
The refinement obligation is that every projection retains the snapshot's
conclusions, including absent completion, and cannot feed fabricated conclusions
back into it. `Outcomes` already distinguishes established membership, complete
search and delivery. Removing redundant mutable copies does not turn these laws
into a proof of the concrete Rust projection or writer behavior.

The streaming `AnswerRenderer` boundary consumes a borrowed interpretation,
evaluated `#show` channels and objective score. Its controller acknowledges an
answer only after the callback succeeds and fixes the terminal callback stage
once per invocation. Human and JSON encodings share that controller; custom
views do not participate in membership checking. These are concrete publication
contracts under the same `Outcomes` obligations, exercised by Rust regression
tests. They add no end-to-end refinement theorem.

The [ownership chapter](../architecture/ownership.md) and
[session example](../rust/sessions.md#reuse-and-identity) connect these obligations
to the maintained implementation.

The CPU source join borrows bound values from its immutable relation snapshot.
Backtracking clears slots; a ground instance lends checked keys over those
bindings, and retained consequences select canonical identities. This changes
storage ownership without changing the substitution used by whole-tuple
matching, filters or frozen gates. Each live binding must denote its matched
source value until that branch ends, and consequence admission must preserve
full typed identity. Rust lifetimes prevent the snapshot from being invalidated
while those references are live; identity and join tests check the concrete
behavior. This does not establish source-join coverage or a Lean-to-Rust
refinement.

The formula join lends its completed partial frame to both support generation
and rule instantiation through the same `Binding` interface when no generator
needs ownership. The relevant existing
laws are `BindingScopes.readAll_agrees`, `AtomKeys.tuple_agrees` and
`TableBindings.join_family_preserved`: equal checked reads and equal ordered
join-step families preserve typed atom identities and the joined family. The
implementation must establish those premises while delaying the final-depth
undo until the borrower finishes. Owning continuations still materialize before
filters; lending does not copy scalar payload or record an owned snapshot.
Rust lifetimes, pending-undo sequencing, substitution/work ceilings and the
ordering of resource refusal versus source diagnostics remain executable
obligations, covered by cursor, ownership, bounded-prefix and source tests. No
new theorem or existing law certifies the concrete cursor state machine.

Gate and consequence membership uses a checked `AtomKey` over that borrowed
assignment. The key denotes the same signed predicate and complete typed tuple
as materialization. Gate lookup and duplicate-head lookup create no owned atom;
a new tree consequence retains a discovery ID after canonical import, and a
dense consequence is a pending coordinate bit. Key construction charges
the argument span separately from catalog lookup receipts, including a deferred
gate with a missing slot. CPU source instances borrow checked keys for the
callback lifetime; their buffers own metadata, not logical payload.
`AtomKeys.tuple_agrees` equates views agreeing on all requested reads;
`membership_identity` connects successful substitution to extensional tuple
membership. Rust comparison/hash equivalence, index construction and binding
lifetimes remain implementation obligations.

A dense relation stores a bounded predicate's rows as bits over the
mixed-radix index of the arguments' ranks in their bounds. A round's dense
heads are marked as pending bits and joined into the relation after the
round, and a block step marks a block of heads from a block of rows by words.
[`RowSteps`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/RowSteps.lean)
states the laws of these steps over an abstract position function:
`marks_are_new_atoms` and `absorbed_is_union`, that under a position
injective on a carrier holding every derived head the marks are exactly the
new atoms and joining them publishes the held and the derived atoms;
`marks_distinct`, that distinct new atoms have distinct marks, so the marks
count what the derived-atom limit bounds; `block_heads`, that when no filter
or gate reads the stepped value, every other positive atom is fixed, and
the fixed part of the rule is live at some value of the stepped argument,
the other positive atoms holding and the filters and gates passing there,
the heads derived through the occurrence are the heads of the values whose
row the relation holds; and `block_places`, that relations placing a value
by the same rank after a base give a row and its head the same offset. The
Rust obligations are that the mixed-radix index is such a position, a
bijection between the tuples inside the bounds and the bit positions; that
the inferred bounds are such a carrier, an upper domain of every derivable
head, which holds because each head argument's bound is closed under every
template's contribution to it; that position order is canonical atom order,
which follows from each argument's values being kept in canonical order with
the first argument most significant; that the block-step plan admits only
rules and relations meeting the block's conditions, and a block is stepped
only at the innermost depth of a join, where every guard of the rule has
been judged; and the word arithmetic of the join. The argument bound, dense
relation, block-step plan and family tests check them. Under the laws and the
obligations, the closure over dense relations is the closure over catalogs,
step for step; the consequence step and constraint verdict of `DeltaRounds`
are the same whichever store holds the rows.

The objective consumers use `AtomLookup` over immutable model selections or an
`AtomIndex` over the original catalog. The index owns permutations of row IDs,
not additional atoms. Its required laws are exact full-key membership and
predicate filtering in original row order. Model lookup must additionally
exclude unselected catalog atoms. The checked value comparison must agree with
canonical storage identity, which is distinct from ASP term order. Existing
identity laws explain the denotation; stable merge sorting, binary search,
lifetimes and their charged failure prefixes remain Rust correspondence
obligations. Independent scan, shuffled-catalog and work-cutoff tests check
these concrete boundaries.

`SeedSelections.materialization_exact` shows that ordering and coalescing the
atom denotations of shared handles preserves their exact true set. Distinct
handles may denote the same atom. The Rust `SeedSelection` owns canonical shared
coordinate tokens, while `SeedView` borrows their semantic atom views without
materializing owned atoms. Arc lifetime safety, program-instance validation, canonical
comparison and the executable candidate cursor remain separate correspondence
obligations. This interpretation law does not establish answer-set membership
or complete candidate coverage.

`GatePositions.atoms_exact` identifies the gate subsequence of an indexed atom
carrier. `retained_position_exact` relates each gate rank to its original dense
position and atom. Rust's `GateAtom` retains a Program-bound `CarrierAtom`
coordinate token and checked positive position. `SeedAtom::resolve_in` uses that position in the
compiled gate table; ordinary CPU and GPU packing share this resolver. The
implementation must establish the common predicate/domain order, inseparable
token construction, program identity and checked indices. The Lean laws prove
the filtered-list correspondence, not those Rust obligations or membership.
For the concrete order bridge, admitted signatures and domain values are sorted
and unique. `AtomIter` advances the last tuple coordinate fastest, giving the
same lexicographic tuple order as `Atom::Ord`, within its signature-first order.
Nullary predicates contribute one tuple even when the domain is empty. The
[full-carrier control](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/tests/gate_positions.rs)
compares enumeration with an independently generated, sorted and deduplicated
product, including signed predicates and typed values. This is executable test
evidence and a source argument, not a Lean proof of Rust's iterator. Canonical
carrier ranks remain distinct from append-assigned catalog identities and ASP
term comparison order.

[`CarrierCoordinates`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/CarrierCoordinates.lean)
makes the coordinate bridge explicit. `decode_injective` requires unique signed
signatures and typed domain values. `tuple_before_iff` and `decode_before_iff`
transport order only when those component decoders preserve and reflect the
chosen relations; increasing interning IDs do not supply that premise.
`sparse_membership_iff` identifies a selected atom with its unique coordinate
without assuming a finite or machine-representable whole carrier.
`shared_vocabulary_does_not_identify_atoms` separates shared term/signature
meaning from independent atom-row positions. These laws assume admitted
coordinate types; Rust still checks arity, bounds, exact Program applicability,
immutable owner retention and fallible coordinate construction.

`GatePositions.rank_fold_eq_blocks` relates the left-to-right domain-rank fold
to the sum of lexicographic block offsets. `blockOffset_lt_cardinality` places
valid digits inside a carrier of size `domain_size ^ arity`, including the one
empty tuple at arity zero. `GateIndex` adds the cardinalities of earlier gate
signatures and one to that offset. These arithmetic laws do not establish the
Rust binary search, signature/domain order, checked powers and additions, or
the connection to `AtomIter`; the full-carrier tests check that connection.
An index token keeps the original program and full gate position even when
only supported atoms are supplied to `locate`.

The closure candidate root may enumerate its completed upper bound's gate atoms
directly, holding its lower bound and offering only the remaining upper atoms.
`Bounds.narrowed_contains_accepted` supplies the coverage argument for every
completed pass. The semantic gate carrier stays the original full carrier:
atoms outside the upper bound are absent, including when read under default
negation. Neither the upper bound nor the selected handles replaces the
original program. A stopped upper-closure prefix cannot justify a smaller root;
`Bounds.closed_upper_sound` requires closure. The mixed-radix law supplies the
position calculation, not this semantic bound or executable enumeration proof.

[`RegionBounds.materialization_exact`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/RegionBounds.lean)
relates a descendant's borrowed root-coordinate bounds to the owned cube formed
from its held and uncut atom selections. It assumes exact partial key lookup
and that each materialized list is a permutation of its selected denotations;
`SeedSelections.materialization_exact` supplies ordering and coalescing equality.
`gate_readings_exact` and `narrowing_exact` then identify both must/may readings
and the complete narrowing operation. The existing `Bounds` preservation and
conflict laws apply to that same cube, not to a new acceptance test.

Rust retains distinct canonical root coordinates disjoint from fixed-held atoms.
The theorem needs no disjointness assumption to establish union/set equality;
exact lookup already requires unambiguous coordinate identity. Matching region
length, signed and typed atom comparison, binary-search direction, original
program ownership and complete-root publication remain executable obligations.
An indexed missing key denotes false only in that completed-root representation;
the symbolic unbounded root and stopped root preparation keep their existing
owned-cube/fallback semantics. Both closures must borrow the same pre-pass
decisions. All conflicts must be checked before committing any new decision,
including a previously held atom missing from the upper result or a lower-derived
gate already cut from the region. An upper constraint verdict does not refute
the region. A stopped upper computation publishes no smaller bound; earlier
completed decisions survive a later resource stop, while cancellation and
deadlines retain stopped coverage. These control and allocation properties are
Rust obligations, not consequences of the representation equality.

The scalar lazy closure retains one canonical tuple authority per Program
workspace. Per-predicate `Catalog` values retain scoped membership and
ordering/equality metadata. Every round borrows committed rows while a disjoint
appender admits newly discovered identities. Pending discovery IDs and dense
coordinate marks remain separate from truth until the complete scan finishes.
Final result publication selects the completed truth over an immutable prefix;
reset clears memberships, frontiers and pending marks while retaining identity
and reusable capacity for the next candidate.
`RelationExtension` states preservation of old row reconstruction and equality
selection when row references and dictionary meanings survive extension. It does
not establish the new Rust owner/prefix checks, publication protocol or reset
implementation. `catalog_work` is a subtotal of oracle work; subtracting it
leaves other charged operations, not a runtime estimate. Existing selection and
round laws supply the semantic interfaces, while the complete composition with
canonical tuple storage remains an executable refinement obligation.

[`ModelSelections`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ModelSelections.lean)
relates selected catalog positions to their interpretation. Canonical ordering
and coalescing preserve the true set; unselected catalog entries supply no truth,
and a renumbering preserves meaning when the selected atoms agree. Rust
[`Model`](../rust/models.md) retains a shared catalog and canonical selection.
Static and formula answers share their program catalogs; completed batched lazy
closures share a catalog frozen after the final complete round. Checked positions,
logical comparison, ownership and allocation remain executable obligations.

`rank_coalescing_preserves_interpretation` refines this boundary for cached
semantic ranks over valid positions in one fixed catalog. Equal ranks must mean
equal decoded atoms in both directions; permuting positions and coalescing their
ranks then preserves selected truth. `rank_order_iff` transports strict rank
order to an explicitly supplied atom order, and `increasing_ranks_decode_unique`
excludes duplicate decoded atoms from that selection. These premises do not
follow from increasing occurrence IDs, a shared vocabulary or equal catalog
lengths. Rust must construct and bind the ranks to the exact catalog, retain
selected representatives, and check work, capacity, cancellation and publication.
The laws do not verify that implementation or its sorting algorithm.

The common `ModelRetention` ledger charges each distinct occurrence catalog's
portable encoding once, including repeated and unselected entries, then
selected-position records per entry and the consumer's score records.
`StorageOwners.sum_within_component_bounds` supplies
the finite-sum bound when those canonical components are completely covered
once. It does not establish the ledger's allocation-identity/hash invariant,
checked arithmetic or transactional publication. Rust keeps a live catalog
handle for every private identity key and commits only after both owner-index
and consumer-slot reservations. Abandoned or refused admissions preserve the
prior retained family; replacement admits the complete new family before
dropping the old one. The measure excludes canonical identities outside the
occurrence maps, capacity/allocator overhead and transient old/new overlap.
It is not an allocated-byte or process-memory bound, even when a retained
catalog's shared prefix contains those additional identities.

[`AtomCatalogs`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AtomCatalogs.lean)
describes a catalog split into a committed prefix and a pending suffix.
`commit_preserves_lookup` equates split lookup with lookup after concatenation;
`discovery_preserves_interpretation` shows that new identities add no truth to an
old selection. Successful equal lookups have the same position when the complete
catalog is unique. This last premise applies to unique builders, not arbitrary
public `AtomCatalog` inputs, which can retain duplicate dense slots. Index
ordering, transactional publication, allocation, machine bounds and Rust borrows
remain implementation obligations. These representation laws do not replace the
reduct or establish answer-set membership.

[`ClosedCatalog`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ClosedCatalog.lean)
composes a base-row lookup with a local lookup whose result is offset by the
base length. Sound component queries return the same atom as the concatenated
row decoder; complete component queries make absence exact. Only identifying
one exact row position additionally requires uniqueness across the whole
base and suffix. These laws reuse `AtomCatalogs` and supply the query premises
for canonical discovery below. They do not establish hash collision handling,
scope authentication, machine offsets, physical sharing or publication. The
storage prefix/suffix partition is independent of the semantic base/defined-atom
partition in `TerminalDefinitions`.

[`CanonicalCatalog`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/CanonicalCatalog.lean)
separates original occurrences from canonical IDs. `selection_exact` composes
those two decoders, including duplicate occurrences; `normalized_selection_exact`
preserves that interpretation after a specified permutation and duplicate
removal. `append_preserves_interpretation` requires every selected mapped ID to
lie in the old prefix, so an invalid old ID cannot silently become a new selected
atom. `owner_transfer_preserves_interpretation` requires agreement of decoded
atoms, and `raw_ids_do_not_determine_interpretation` shows why raw equality is
insufficient. The laws reuse `ModelSelections`; they do not establish DAG
interning uniqueness, Rust comparison/hash consistency, lifetimes or storage
accounting. In Rust, one atom operation resolves each canonical row once and
reads every field from that resolution while the immutable prefix is
borrowed; this is one evaluation of the decoder, not a retained copy. Atom
hashing writes the same fields as the ingress atom with equal contents, which
the canonical/ingress hash tests check across segments and readers.

`CanonicalCatalog.find_discovery` separates canonical query resolution from a
unique discovery map. `discovery_lookup_exact` equates that composition with the
existing structural occurrence lookup, assuming sound and complete canonical
queries, injective successful decoding, and an exact inverse of discovery.
`undiscovered_identity_absent` shows that a canonical row without a discovery
entry remains absent from every discovery slot. The inverse premise does not
apply to occurrence catalogs that repeat an identity at different positions.
Rust must maintain the inverse at publication, authenticate the readable scope
and prefix, and preserve refusal and allocation boundaries. These laws establish
neither a concrete hash/index implementation nor support or answer-set membership.

The selected discovery-order operation reuses `normalized_selection_exact` once
its result is a permutation of the supplied identities. Rust must establish
that the semantic index visits each discovery once, that the temporary mask
selects exactly the supplied positions, and that filtering preserves typed
order. `ColumnRelations.row_mask_roundtrip` uses the same membership,
uniqueness and ordered-filter argument for numeric row order; it is not a
verification of the AVL traversal. Dense traversal and sparse sorting therefore
share the denotation obligation while retaining separate executable checks for
ordering, scratch, interruption and transactional replacement.

Checked equality can reuse a canonical identity only after authenticating its
scope and readable prefix. Equal IDs then use the same decoder; deciding that
unequal IDs denote unequal terms additionally requires interning uniqueness.
The Rust stores must establish that stronger invariant, including normalized
aliases and exact hash-collision checks. These catalog laws do not prove it.
Foreign scopes and sign-adjusted predicate views require typed comparison.
Neither equality shortcut establishes numeric or ASP order. The callback before
the identity probe and preservation of its refusal are executable obligations.

`TemplateCatalog` stores shared ordered terms, patterns and filters beneath
separate rule and objective policies. `Program` retains rule topology and its
explicit domain/signature order; `ObjectiveProgram` retains weight polarity,
priority slots and ID-only closed conditions. Shared component storage creates
no synthetic support or objective rule.
The source compiler uses the same component schema for its constants, predicates,
patterns and filters. Its private coordinates are valid only with the
preparation that admitted them. Generated terms extend the vocabulary without
changing the meaning of the static prefix; truth and support remain separate
selections. This owner pairing and prefix preservation are concrete Rust
obligations, not consequences of integer equality.
[`CanonicalTemplates`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/CanonicalTemplates.lean)
proves that decoding commutes with substitution of ordered argument tuples,
including repeated occurrences and absent variables. Its variable-only case
equals the existing `BindingScopes.readAll` operation. The identity type ranges
over an available vocabulary: Rust must establish the scope and prefix checks
that make its decoder total. Refinement of the concrete template storage and
flat condition-node evaluator to these definitions and
`ObjectiveConditions.Query` remains an implementation obligation. These laws
neither prove source compilation nor infer answer-set membership from identity.

`AtomAppender::insert_pattern_with` borrows a checked projection of constants and
assigned variables, reused for discovery lookup and row admission. The tuple
is a mathematical sequence; the implementation need not materialize an ID vector.
`CanonicalTemplates.substitute_decodes` supplies its substitution obligation;
`AtomKeys.key_identity` and `membership_identity` relate the decoded tuple and
signed predicate to extensional lookup. Rust must establish the scope, prefix,
arity and complete-binding premises, preserve repeated arguments, and apply
logical limits even on an occupied lookup. Shared immutable input borrows must
keep each projected coordinate unchanged from validation through publication.
Fixed preparation metadata, scratch/growth accounting and
publication after all fallible checks remain executable obligations. Retaining
a complete canonical row after refusal does not establish discovery or support
membership.

Reusing a prepared canonical lookup additionally requires the same immutable
tuple and unchanged logical row index until publication. The exclusive Rust
entry enforces the writer boundary; index reservations may rehash storage but
must not change logical membership. The prepared result contains no bucket
address and is discarded on failure. These are implementation obligations for
the existing lookup law, not a separate answer-set semantics or a new proof of
the Rust implementation.

`DerivedTerms` gives registered input aliases and newly constructed terms one
fresh identity scope. Input edges borrow exact immutable catalog prefixes;
generated compounds retain local child IDs and borrowed constructor names.
`TermRead` exposes both through the same typed term view. The input payload is
not copied into the derived arena. Equal terms from different input owners, or
from an alias and a construction, must receive the same registered identity.

The existing laws state the required semantic boundary.
`CanonicalCatalog.owner_transfer_preserves_interpretation` requires decoded
agreement when identities change owners; it does not infer agreement from equal
integers. `CanonicalTemplates.substitute_decodes` preserves ordered child
occurrences, including repetitions, once scope and prefix checks establish the
decoder's domain. `StructuralBindings` and `Observations` then state matching
and completed-output laws over those decoded values. No new reduct is involved.

Exact alias interning, collision checks, cached expanded measures, borrowed
lifetimes and preservation of a complete prefix on refusal or a caught callback
panic remain Rust obligations. So does the absence of copied input payload.
`StorageOwners` applies only after a complete, disjoint accounting of the shared
inputs and the arena's own capacities; logical occurrence charges do not measure
physical sharing. The formula-DAG laws in `DagSharing` do not certify this typed
term representation or its Rust implementation.

[`PartitionedScan`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/PartitionedScan.lean)
proves that exact finite partitioning preserves an eligible-atom count and maximum
index. Zero availability excludes a fitting strict-subset completion; one
available atom must be false in every such completion. The maximum names that
atom only under the count-one premise. The formula kernel distributes this scan
over 64 lanes and merges their summaries. Its strided coverage, bounded `u32`
arithmetic, immutable scan inputs, atom correspondence and synchronization remain
Rust/WGSL obligations. The laws justify domain narrowing, not answer-set acceptance.

[`DependencySchedule`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DependencySchedule.lean)
preserves initial agreement with a reference valuation by induction over ready
assignments. The evaluator reads only declared dependencies; those dependencies already agree,
and the reference satisfies each local equation. Complete root coverage then
preserves ordered observations. This explains the formula kernel's original-truth
schedule before reduct masking. Constructing its packed levels, preserving node
IDs and proving concurrent writes and barriers refine the sequential schedule
remain implementation obligations. A cached order does not cache candidate truth.

The CPU oracle's [`Work::charge`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cpu/src/oracle.rs)
charges a payload amount before the corresponding comparison or copy.
[`WorkCharge`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/WorkCharge.lean)
equates its available work prefix with repeated unit charging under unchanged
control, including exact quota exhaustion. A positive charge polls control once;
a zero charge remains a no-op. This law describes the CPU oracle's prefix
convention. Formula source admission instead checks an entire proposed charge
before changing its counter. Rust word bounds and the placement of control
checks remain concrete implementation obligations; the law does not promise
observation of asynchronous cancellation between bookkeeping units.

[`PreparedQueries`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cpu/src/oracle/prepared.rs)
shares one exact program and records the join dimensions it inspected. Reusing
that owner does not reuse a reduct or an interpretation. Each candidate starts
with empty relations; assignments borrow only its current immutable round.
Final assembly publishes a selected immutable prefix and resets truth-bearing
relation memberships, frontiers and pending marks. The canonical authority and
its discovered identities remain available; their presence supplies no truth to
the next candidate. Previous results retain their original immutable prefix.
The required refinement is equality with a fresh full-round closure, including
constraints and frozen-seed agreement. The scalar and batch reuse controls
compare those results, retained buffer addresses and failure recovery. They do
not constitute a Lean proof of the Rust implementation.

The batch oracle partitions ordered candidate occurrences into disjoint ranges,
each with an exclusive workspace. Its coverage obligation is that splitting
preserves every occurrence once and ordered concatenation restores the input
sequence. Its storage obligation includes shared preparation, idle capacities
and active reservations without counting shared headers twice.
[`StorageOwners.shared_idle_active_within_limit`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/StorageOwners.lean)
sums explicit shared and idle bounds with an active bound of
`max(retained, allowance)` per owner. The batch maps `allowance` to the scalar
limit minus the shared preparation header, after checked subtraction; an empty
batch reserves only retained storage. The consumer must account for every named
region with disjoint owner entries and count the shared component once. The
pointwise capacity bounds, Rust measurement, cache retirement, checked arithmetic
and range scheduling remain implementation obligations. The theorem does not
establish a hard allocator or RSS cap, including on a refused allocation. Reused
capacity can increase retained space or change work counts; neither this law nor
semantic equivalence proves a speedup.

Independent scalar rounds use the same occurrence partition over stable catalog
insertion IDs. `DeltaRounds.coverage_of_first_delta` requires a correspondence
between those IDs and complete body bindings. `delta_step_exact` then preserves
the complete inflationary step when old consequences are already present;
`latched_constraints_exact` additionally requires exact prior constraint history.
Bootstrap handles zero-positive rules and constraints. These premises explain
why the final no-change round can combine its selected scans with completed
history instead of revisiting every old binding. They do not certify the Rust
matcher, cache boundaries, failure cleanup or shared-world/device schedules.
The [delta-round guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/delta-rounds.md)
connects each premise to its concrete owner and publication boundary.

Eager support joins partition new tuple combinations by their first new source
occurrence. `DeltaJoins.partition_complete` proves coverage and
`partition_disjoint` proves uniqueness. Source occurrence identity survives
execution-order changes and repeated predicates. The Rust certificate must
select only the eligible positive producer grammar; posting slices must denote
the specified old/new intervals. Rich producers and final authored-body
validation retain separate complete scans. These laws establish the partition,
not the source evaluator or termination of value generation.

Parallel exact queries lease bounded work allowances from one shared owner.
`WorkPermits` partitions the allowance into spent, available and outstanding
permits. Granting preserves that total; settlement records consumed work and
returns the unused part. Empty availability is not exhaustion while a grant
remains outstanding. After a grant is fully consumed, Rust commits its used
permits and attempts to acquire a replacement under one ledger lock. Successful
renewal composes `settle_conserves`, with consumed and granted amounts equal,
then `grant_conserves` for a replacement bounded by availability. A successful
renewal returns no unused permits and retains an outstanding replacement, so it
cannot enable a previously blocked waiter. Settling the last outstanding grant
with no available permits must wake waiters even though no permits were returned.
The Rust implementation must additionally preserve conservation and these
notification obligations under word arithmetic, locking, unwinding and
cancellation; the Lean laws do not prove condition-variable progress. A joined
batch records spent operations only after all leases settle; a lease is not
evidence of candidate execution or membership.

The parallel region walk scopes each lease to one region and settles it before
waiting for another region or sending a model. Its shared allowance includes
coordinator certificate preparation; each worker reserves a finite checking
bound before running its certificate and settles its actual returned work.
The bounds follow the checkers' charged visits: nodes, roots, producers and atoms
for a tight check, and atoms, nodes and roots for a positive check. An unwind
consumes its reservation conservatively and yields an incomplete worker failure.
Returning unused grants before an idle wait, releasing blocked
sends before joining on iterator drop, and counting joined certificate checks
are Rust lifecycle and accounting obligations, beyond permit conservation. The
bounded subprocess regressions in `zetesis-sat/tests/parallel_regions.rs` exercise
shutdown and idle grants; the certificate regressions compare complete scalar
and parallel work and reject an insufficient shared allowance. An injected
worker unwind also checks that idle peers wake and coverage remains incomplete.

The native parallel walk keeps one deque per worker. Its owner removes the
newest region, and an idle worker tries to steal the oldest region from a peer.
Each deque has its own mutex; a thief skips a busy deque. Payload preparation,
narrowing and membership checks hold no queue lock. A split first reserves both
slots fallibly, then raises the unresolved-region count by one before publishing
either child under the same queue guard. A resolved region decrements the count
once. Taking or stealing only transfers ownership. Thus pending and active
regions remain one frontier until refuted, split or checked, and an idle worker
can establish termination only when the count reaches zero.

`Pending.Step.perm` and `Pending.Walk.exhausted` describe the abstract preservation
and exhaustion laws; the atomic count, mutex protocol and absence of lost
ownership remain Rust refinement obligations. Depth-first local order retains
at most one older sibling per ancestor plus the newest children. Each split
decides another atom, and stealing starts only with an empty local deque, giving
at most `atom_count + 1` live entries per deque. Slot capacity grows fallibly as
needed and is released after joining, including a partially launched worker set.
This slot reservation does not make region/knowledge payload cloning fallible.
An idle worker that observes cancellation or a deadline
records the typed stop before publishing closure. The coordinator may already
be waiting after its own control poll; channel disconnection must retain that
stop instead of establishing exhausted coverage. A concurrent close may follow a
worker's eligibility check; that already-active operation retains the existing
bounded-stop contract.
The Lean laws do not prove the scheduler's progress or Rust memory ordering.

Original and frozen `Narrower` operations expose metered entry points returning
an independent `NarrowingAttempt` receipt. The injected SAT budget acquires a
local or shared permit before each charged read, so the shared ceiling bounds
execution itself. A refused acquisition prevents that read; all earlier reads
remain in the attempt and joined region counters even on a stop. The local
`RegionLimits` APIs wrap the same closure. Preservation of semantic narrowing
still depends on `FormulaBounds` and `FerrarisMask`; permit conservation does
not prove the reading rules or knowledge ownership. Prefix tests for original
and frozen narrowing and a shared one-permit regression exercise the Rust
admission/receipt boundary.

## Candidate generation and query representation

Candidate restrictions and storage transformations preserve different objects.
A necessary restriction may remove impossible proposals; a query representation
must preserve the same classical assignments. Neither operation changes the
original program or the reduct used to check membership.

| Law | Implementation boundary | Remaining correspondence |
| --- | --- | --- |
| [`GateRestrictions.answer_set_avoids`, `suffix_region_rejected`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/GateRestrictions.lean) | Source-derived positive gate restrictions and binary seed-region skipping | Each witness uses actual unconditional facts, complete source bindings and the stated gate indices. Possible support alone is insufficient. Rust counter jumps and resource accounting need refinement. |
| [`Bounds.narrowed_contains_accepted`, `lower_constraint_refutes`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Bounds.lean) | `Candidates::bounded` narrows the undecided region to a fixed point by the region's lower and upper closures (`definite_closure`, `possible_closure`, the `MustGate` and `MayGate` readings of the shared closure rounds), holds the lower closure's gate atoms in every seed, never offers a gate atom outside the upper closure, and offers no seed when a constraint fires in a lower closure | The Rust closures must be the least fixed points of the two consequence operators over the exact admitted program; the counter's enumeration inside a counted region and the restriction plan's treatment of held premises are Rust obligations. Each offered seed is still checked in full. |
| [The `split` constructor of `Search.CoverageTree`, `Cube.split_partition`, `Cube.split_disjoint`, `CoverageTree.mem_outputs_iff`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Search.lean), [`Bounds.conflicting_atom_refutes`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Bounds.lean) | `zetesis_cpu::regions::Traversal`, the one-worker walk of both routes: a region is narrowed by the caller's narrowing, refuted, offered as one candidate when decided, split on the atom its narrowing preferred or else its highest open atom into the out and in regions, or counted as a flat interval when its narrowing decided nothing beyond the split and the caller counts; the closure route narrows by its two closures, the formula route by the theory's knowledge. Under several workers the formula route walks the same tree in `zetesis_sat`'s parallel regions, a second implementation of the same law: each worker takes its newest region from a deque and idle workers steal older regions from peers | That the Rust split chooses a fresh atom and forms the two cubes of the law, in the traversal and in the parallel walk alike, that the counted interval offers exactly the region's seeds, and that the leaf order is the counter's order when no atom is preferred and one worker walks are Rust obligations; with several workers, [`Pending.Step.perm`, `Pending.Walk.exhausted`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Frontier.lean) is the law: the frontier of pending regions, whoever holds each, and the leaves emitted are together a permutation of the tree's outputs after every step in any interleaving, so a walk that empties the frontier emits every accepted leaf once, in the schedule's order; that the workers' deque removals, transfers and active regions form one frontier of the tree is the Rust obligation; the tree's leaves are exactly the accepted seeds only because each leaf seed is checked in full. |
| [`TightPlans.stable_supported`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/TightPlans.lean) | A candidate the complete tight certificate finds with an unsupported present atom is rejected as `Check::Unsupported` without a reduct query; the candidate without that atom is the law's witness | The plan's coverage of the theory's producers is established by its construction, not yet by a proved refinement of that construction; the Rust check's evaluation of producer bodies remains executable evidence. |
| [`DisjunctiveSupport.answer_set_supported`, `answer_set_supported_with_choices`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DisjunctiveSupport.lean) | Necessary ordinary sole-head support and enabled atomic-choice support | Extraction must cover every asserted root, coalesce repeated ordinary head atoms and recognize exact atomic choices in either operand order using semantic atom identity. Choice bodies remain arbitrary original formulas; they supply permission, not a self-premise. The original theory remains the reduct subject. This is necessary support, not ranked sufficiency; Rust DAG extraction, Boolean encoding and bounded failure remain unproved. |
| [`PackedQueryLiterals.decode_encode`, `packed_truth`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/PackedQueryLiterals.lean) | Packed classical literals in `Cnf` and borrowed clause views | Admission must establish machine representability and valid offsets, including repeated offsets for empty clauses. Natural-number arithmetic does not prove machine operations or allocation. |
| [`IndexedCandidates.index_equals_all_blocks`, `failed_literal_forced`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/IndexedCandidates.lean) | The authoritative semantic-projection exclusion index and independently checked completed assignments | Flat-trie insertion, watches and trial undo must implement exact complete keys. Separate history admission must preserve prior keys and publish no key on refusal; concrete capacity accounting remains a Rust obligation. A failed-literal conclusion needs a completed branch refutation; a stopped trial supplies none. |
| [`OptionalIndex.successor_fits`, `optional_round_trip`, `replacement_commutes`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/OptionalIndex.lean) | Optional positive-successor child links in the compact projection trie | Planned node count must fit 32 bits, decoded links must index allocated nodes, and only complete suffixes may be attached. The laws preserve identity and absence; they do not establish Rust layout, allocation, rollback or control accounting. |
| [`FormulaRegions.classical_consequence_forces`, `classical_consequence_cuts`, `no_model_refutes`, `restricted_consequence_forces`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FormulaRegions.lean) | The clauses method (`--search clauses`) read as the coverage tree of `Search.lean`: a search node is a cube, the atoms its propagation forces or cuts narrow it, a conflict refutes it, and a complete assignment is a leaf the reduct decides; the support restriction is a restriction every stable model satisfies | That the cursor's propagation returns only classical consequences of the clauses it holds, and that those clauses are the theory and restrictions every stable model satisfies, are Rust obligations; the search proposes and never decides membership. |
| [`FormulaBounds.read_sound`, `never_root_refutes`, `known_sound`, `known_mono`, `unsupported_cut`, `sole_support_forces`, `unsupported_cut_with_choices`, `sole_rule_forces_with_choices`, `sole_choice_forces`, `known_blocked_no_support`, `known_choice_blocked_no_support`, `restriction_forces`, `restriction_cuts`, `restriction_contradiction_refutes`, `restricted_stable_narrowing`, `decided_leaf_models`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FormulaBounds.lean), [`FormulaBounds.disj_chain_sure`, `disj_chain_never`, `disj_chain_unit`, `conj_chain_sure`, `conj_chain_never`, `conj_chain_unit`, declared in `FormulaChains.lean`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FormulaChains.lean) | `zetesis_ferraris::Narrower::narrow_known`, the narrowing of the regions proposer in `zetesis-sat`: the sure and never readings of every node under a region, the knowledge closed in both directions over the DAG, the support cut and the sole-support rule, to a fixed point; `zetesis_cpu::regions::Traversal` walks the tree of `Search.lean` for both routes with one worker, and `zetesis_sat`'s parallel regions walk it for the formula route with several | The Rust closure must agree with `Known` on the admitted DAG, and `producers` must extract the covered fragment from the roots. The Rust closure must derive only `Known` judgements; the fixed point is the least one because every rule only adds knowledge, and a child region may start from its parent's knowledge by `known_mono`. The closure reads each maximal tree of one connective as one node with two counters and applies the chain rules, each admissible in `Known` as a sequence of the binary rules along the chain (`FormulaChains`); that the Rust chains are such trees, with inner nodes that have that one parent, and that a node false under the frozen mask is read by its operands' masks, are Rust obligations. That a fully decided region no reading refutes is a classical model is `decided_leaf_models`, which is why a leaf is proposed to the reduct without a classical check. An atomic choice is a producer of its atom that only an impossible body blocks, a choice having no other head: `unsupported_cut_with_choices`, `sole_rule_forces_with_choices` and `sole_choice_forces` state the support cut and the sole-support rule over ordinary producers and choices together, from `DisjunctiveSupport.answer_set_supported_with_choices`. The narrowing blocks a producer by the theory's knowledge, a body known to fail or another head known to hold, and not by the readings alone; `known_blocked_no_support` and `known_choice_blocked_no_support` say such a producer supports its atom in no classical model of the theory inside the region, which is the premise those laws ask. That `producers` recognizes the choices of the theory, in either operand order, remains a Rust obligation. A restriction, the support restriction or an objective bound, is narrowed by its readings alone, without producers: `restriction_forces` and `restriction_cuts` are its knowledge sound in its models, and `restricted_stable_narrowing` is the region narrowed by both the theory's and the restriction's knowledge keeping every stable model that satisfies the restriction; that every model still sought satisfies each restriction the enumeration adds is the Rust obligation. |
| [`ReductRegions.leaf_refutes`, `exhausted_stable`, `countermodels_exact`, `stable_iff_no_countermodel`, `masked_read_eq_reduct`, `masked_reads_falsum`, `masked_known_sound`, `masked_known_narrows`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ReductRegions.lean) | The reduct's proper-subset query as a region tree under `--search regions`: `ReductQuery` in `zetesis-sat` walks the subsets of a classical model with the frozen reduct's knowledge, returns a leaf other than the candidate as the countermodel and a covered tree as stability | That evaluation of the original DAG under the mask is the reduct's is `FerrarisMask`'s law. That the narrowing's two readings under the mask are the readings of the reduct is `masked_read_eq_reduct`; a masked node reads as falsum whatever its operands read (`masked_reads_falsum`), so it has no rules of its own, and the nodes above it combine that reading as any operand's. `masked_known_sound` says propagation under the mask, where every rule relating a connective to its operands asks that the connective be unmasked and a masked node is known to fail, knows of an original formula only what propagation over the reduct theory knows of its reduct, and `masked_known_narrows` that the query may narrow by it. That the stored mask is each node's truth in the candidate, and that the Rust propagates by those rules over the shared DAG, are Rust obligations; that the traversal covers the query root exactly is `Search.CoverageTree`; the countermodel is validated independently before it is returned. |
| [`CandidateCursor.completed_coverage`, `stable_outputs_exact`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/CandidateCursor.lean) | Retained candidate traversal, completed checks and exhaustion under the clauses proposer | Rust traversal must denote the abstract finite forest, preserve its open remainder and block only completed checks. Exact accepted output additionally requires candidate coverage and a correct membership oracle. |

The [execution chapter](../architecture/execution.md) explains these operations
in the solver. The [candidate cursor contract](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-sat/docs/candidate-cursor.md)
and [projection-index contract](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-sat/docs/candidate-pruning.md)
describe their concrete ownership and failure boundaries.

The packed `Region` stores disjoint held and cut masks over real atoms, while
`Knowledge` stores sure/never masks separately over nodes and atoms. Its seen
snapshot denotes the region decisions already incorporated; differencing the
current decided mask supplies only new decisions, and the completed snapshot
also includes atoms learned by propagation. `FormulaBounds.known_mono` supplies
the mathematical ancestor-inheritance law. The Rust API additionally requires
the same narrower, theory, producer set and frozen truth, and knowledge closed
for an enclosing region. The law neither proves the packed representation nor
validates those unchecked caller preconditions. Exact word/index conversion,
zero unused tail bits, polarity decoding, complete seen snapshots and correct
child cloning remain Rust obligations. The cross-word original/frozen
regressions are executable evidence for those boundaries, not formal refinement.
Retained-byte accounting includes the owned masks; the full seen-mask scan and
snapshot writes are outside the existing charged-read work counters.

Region candidate preparation and frozen proper-subset queries share one
immutable index constructed with the exact original `Theory`. Reuse checks
instance identity. Each region or query retains private `Knowledge`, and each
candidate supplies freshly authenticated frozen truth. Sharing preserves the
subjects of the existing `FormulaBounds` and `ReductRegions` laws; it changes
ownership, not the definition of a reduct. Index construction, identity checks,
mutable-state separation and attribution of construction work remain Rust
refinement obligations.

The batched parallel proposer separates classical candidate production from
membership. Its workers use the original region readings and disjoint splits;
they do not certify stable models. Read `Frontier` with a proposal family that
contains every still-required stable interpretation. Necessary support cuts may
exclude other classical models, so classical truth alone is not the proposal
coverage predicate. `FormulaBounds` supplies the preservation premises for
these cuts and decided classical leaves. `Pending.Walk.perm` accounts for leaves
emitted so far together with those still held by the frontier.

After producers join, emitted candidates enter the pending membership ledger.
`BatchAccounting.exact_commit_preserves_soundness` justifies committing a fully
classified prefix; `completed_results_exact` additionally requires coverage and
an empty queued and pending remainder without delayed failure. Batch capacity
is a scheduling boundary, not exhaustion. A producer stop retains completed
candidates and reports the unresolved remainder as incomplete; it cannot turn
an empty delivered batch into a complete negative result. The concrete queue
ownership, reservation arithmetic, panic/cancellation paths, budget settlement
before checking and transfer between rounds remain Rust obligations. The two
abstract laws do not themselves verify that concurrent implementation.

`SessionBuilder::executor` exposes that same formula batch boundary to an
external `BatchExecutor`; it does not replace the candidate frontier or objective
owner. The complete original-theory tight certificate, when selected, is shared
with the executor through `MembershipPlan`. Original satisfaction is checked
before invocation. `CandidateBatch::finish` and the host receipt consumer retain
exact theory and candidate-slice association and check result count. Neither
operation proves the soundness of a verdict or its association with the right
position within that slice. The batch laws therefore still require a sound
executor for the selected operation as an explicit premise. Exact residual
completion uses the existing host checker; a callback interruption, refusal or
fault cannot discharge its pending candidates. Rust ownership, callback effects,
resource compliance and transport identities remain implementation obligations,
with no additional theorem or device qualification claimed by this API.

## Representation and source laws

Source scalar validation and construction share a bounded symbol walk and the
core borrowed node's text/spelling measures. The validation consumer retains
logical bounds without constructing an output value; actual capacity admission
belongs to construction. This changes storage work, not the value's mathematical
identity or the requirement to validate inactive authored expressions. The Rust
borrowed-view correspondence, traversal and capacity checks remain executable
obligations; no Lean source-to-value refinement is claimed by this separation.

[`ColumnRelations`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ColumnRelations.lean)
relates complete typed tuples to aligned equality-ID columns. Dictionary round
trips and exact cell encoding imply reconstruction; equality selection returns
exactly the satisfying subsequence in original order. `source_posting_exact`
preserves the complete ordered posting for one source equality, so a fixed
shortest-posting chooser can retain its original matcher visits.
`row_mask_roundtrip` decodes exact membership bits to the same increasing row
selection; it assumes unique positions and the correct row domain.
`full_matches_preserved` assumes a total Boolean
matcher and that every complete match satisfies the prefilter. The Rust
[`relation` module](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/relation.rs)
provides checked owner-bound views. Construction, catalog mapping, resource
accounting, fallible matching and any device masks still need executable
correspondence arguments. The [proof guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/column-relations.md)
explains the hypotheses with a correlated-tuple example.

The append dictionary's canonical inverse implements the same encoder. Each
published representative must have exactly one inverse entry naming its local
equality ID, and every inverse entry must name that representative's whole
canonical term. Under this invariant, an authenticated identity miss is a true
dictionary absence. The semantic AVL remains responsible for new-value placement
and queries without an eligible canonical identity. Rust must preserve the
invariant through append, refusal, capacity growth and clear, and validate the
vocabulary and readable prefix before lookup. These transaction and resource
properties are executable obligations; the dictionary round-trip laws do not
prove a particular hash-map implementation.

[`FiniteTables`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FiniteTables.lean)
relates intersections of value supports to surviving complete rows. Projection
preserves these rows, contracts their domains and is idempotent for a single
table. Recomputing from the original table preserves previously surviving rows
when domains widen. The [Rust primitive](../rust/finite-tables.md) borrows typed
values and retains row identity. Direct selection stops at an ordered row mask;
full projection additionally returns witnessed domains.

[`TableBindings`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/TableBindings.lean)
connects this row law to positive source matching. `flat_match_survives` derives
alias coherence and necessary constant/incoming-binding domains from
`StructuralBindings.matching_sound`; canonical table labels have explicit source
argument origins. `indexed_matches_preserved` preserves the ordered list of row
occurrences and resulting bindings. `join_family_preserved` composes equal step
families over the same finite schedule, retaining full source-occurrence/row
traces, including repeated predicates and duplicate-valued row positions.

The explicit table grounding strategy consumes completed possible support and
retains the existing matcher. Structural patterns and growth-round snapshots
keep indexed probes. Concrete scope construction, typed decoding, packed bits,
owner lifetimes and cumulative resource accounting remain executable
correspondence obligations. Relational mismatch in the mathematical matcher
does not represent interruption or an undefined authored expression. Comparisons,
negative conditions and aggregate validation keep their separate continuation;
the table cannot silently prune a required error. Source rows remain original
atoms in the emitted formulas. The [binding guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/table-bindings.md)
separates these obligations from the independent support-coverage and reduct
arguments.

[`DomainBindings`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DomainBindings.lean)
addresses a different filtering boundary: a local match may have no complete
continuation. Conservative argument coverage makes the intersection for each
source variable necessary, and `kept_binding_survives` keeps it necessary once
every value a comparison over that variable alone excludes is removed, for the
bindings the exclusion rule keeps. Selecting every row that can finish then
preserves the exact ordered completion list, including multiplicity. The
optional eager consumer applies these guards to the exact normalized positive
program, in every support-completion round and in final instantiation; the
ordinary command requests it. Unknown,
stopped and inapplicable analysis supply no narrowing. Concrete analyzer
soundness, source/IR correspondence, dictionary identity, the agreement of the
guard's comparison verdict with the join's, and recursive matching remain
unproved implementation bridges. Prepared candidates now retain scoped source
term IDs. Each support snapshot decodes them into its own relation dictionaries;
the preservation premise concerns decoded values, not equality of numeric IDs
across those authorities. The
[domain-binding guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/domain-bindings.md)
also states the separate work, storage and authored-error obligations.

[`DomainContraction`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DomainContraction.lean)
proves concrete singleton-distinctness and affine filters preserve compatible
assignments under fixed constraints. Finite compositions preserve that family;
an empty mandatory domain excludes the certified branch. Relating these
assignments to answer sets requires a fixed assignment interpretation and sound
recognition of the active constraints. The laws do not
implement constraint recognition or establish source admission, support,
minimality or existence. In particular, applying a filter before source
evaluation requires a separate argument that required arithmetic errors remain
observable. The [domain guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/domain-contraction.md)
distinguishes these prospective filters from the existing source-domain analysis.

The normal-rule and Ferraris foundations retain independent definitions.
The [NormalFerraris bridge](normal-rules.md) proves their equivalence under the
specified normalized-rule translation, including its filtered/direct-map
distinction. It does not verify `from_ground_program`, atom interning or formula
DAG construction. Likewise, `FrozenReduct` is a Rust representation of a fixed
candidate's reduct; its existence does not close the Rust-to-Lean mask
correspondence.

`EvaluationWorkspace` computes original node truth before exposing a borrowed
`FormulaEvaluation`. Rust's ownership boundary ties that view to the exact
interpretation and prevents reuse of its backing workspace while the view lives.
The denotational satisfaction relation supplies the mathematical specification;
the topological evaluation loop, node-index admission, complete-root decision
and charged failure prefixes remain Rust refinement obligations. Independent
syntax-tree controls check satisfaction and all asserted-root truth across the
finite test family. These tests do not establish a general implementation proof.

[`ParametricReduct`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ParametricReduct.lean)
proves that a fixed connective network agrees with frozen-reduct truth below a
candidate when its original-truth parameters are accurate. Atoms read the tested
subset; implication retains its original candidate-truth guard. The countermodel
law covers every asserted root and a proper subset, while original candidate
satisfaction remains separate. Rust `PreparedReduct` binds one immutable theory
and takes parameters from its authenticated `FormulaEvaluation`. Its CNF
encoding, subset/properness clauses, replaced assumptions, assignment reset,
independent witness check and shared/worker storage accounting remain concrete
obligations. Retaining a watch index additionally requires exact immutable-owner
identity, valid distinct watched positions, complete indexing before reuse, and
structurally complete watch updates at every interruption point. The prepared
CNF's completed unconditional unit consequences may be retained while only the
candidate-dependent assignment suffix is cleared. Interrupted unconditional
propagation must publish no reusable result, and a candidate conflict must not
become an unconditional refutation. A base-false watched literal must remain
protected by a base-true other watch after suffix undo.
`Propagation.unconditional_sweeps_models_iff` proves the abstract composition:
intersecting arbitrary candidate domains with unconditional narrowing preserves
exactly their completions of the same query. It does not prove the Rust unit
propagator, its quiescence or the watch registry lifecycle. Neither retained
capacity nor reuse authenticates stale parameters.

[`PositiveTheory`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/PositiveTheory.lean)
characterizes complete atomic-head theories with monotone bodies, including
positive cycles and exactly the truth constructor `False → False`. The least
producer-closed interpretation is the sole possible answer set; it is an answer
set exactly when all positive constraints hold there. Under this strict grammar,
a failed constraint rules out every classical model. Rust `PositivePlan` uses
that positive producer argument and the arbitrary-constraint extension below.
Its CSR correspondence must establish sound
activation, complete child incidences including aliases, finite queue exhaustion
and complete constraint checking against the same owner. The law does not prove
Rust indexing, allocation or work admission, nor source completeness or ordinary
candidate enumeration. A dependency projection or class hint cannot replace
the original-root certificate. See the [reading guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/positive-theory.md).

[`ConstrainedPositive`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ConstrainedPositive.lean)
proves that any original constraint `F → False` satisfied by a candidate has a
reduct true in every interpretation. It needs no subset or positive-body
premise. Adding arbitrary constraints therefore filters existing answer sets
without supplying support. For positive producers, the least consequences are
the unique answer set exactly when the original constraints hold there. Failure
rules out answer sets, but need not rule out larger classical models. Rust must
classify every original root, compute closure only from positive producers,
then evaluate the same original theory through `EvaluationWorkspace`. A false
producer is an invariant refusal, not a constraint verdict. Exact evaluation
consumes remaining work and counts its actual capacity beside the retained least
interpretation after CSR release. The append/partition law does not prove those
Rust ownership, work, first-error or source-completeness obligations.

[`TerminalDefinitions`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/TerminalDefinitions.lean)
separates arbitrary base atoms from new derived atoms using a disjoint sum. Each
definition has one derived head and a positive conjunction of base atoms; the
base theory never reads derived atoms. `stable_iff` characterizes every full
answer set as a base answer set plus exactly its definition consequences.
`stable_extend_iff` and `unique_stable_extension` give membership preservation
and a unique full extension of each base answer set. Several producers of one
head contribute by existential union; empty bodies and duplicate occurrences
are allowed. The frozen-reduct characterization requires a subset of a candidate
satisfying the definitions and imposes consequence containment. Minimality,
rather than classical satisfaction alone, excludes extra derived atoms.

Applying this law to source programs still requires a complete semantic
partition: every producer occurrence, all body scopes and implicit coherence
constraints must respect the base/derived separation. Ground-instance coverage,
preserved source diagnostics, canonical owner correspondence, reconstruction
before publishing a full answer, and failure and coverage accounting remain
unproved implementation obligations. This one-layer theorem does not establish
a recursive definition schedule or certify a source or runtime optimization.

The executable consumer is
[`formula_terminal`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_terminal.rs).
Its partition matches the exact normalized source to every selected IR producer
and scans remaining semantic reads. Reconstruction joins true base-model rows
and publishes their union with derived heads through one canonical descendant
store. [`TerminalSession`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/terminal_session.rs)
retains the original subject, counts only completed extensions as original
answers, and records an unfinished extension separately. These Rust checks
implement the stated obligations; the existing theorem does not prove their
source-to-proposition or machine-execution correspondence.

[`StreamedConstraints`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/StreamedConstraints.lean)
adds an executable finite scan beneath that filtering law. A violation names an
original occurrence, completion certifies that every checked body is false, and
interruption retains the suffix after a checked false prefix. A partition's
flattened occurrences must be a permutation of the complete source family.
`completed_partition` and `partition_invariance` preserve the completed
satisfaction verdict across chunk sizes and orders; they do not preserve the
first violation or authorize acceptance before every required part completes.
`stable_iff_completed_partition` combines pointwise Boolean-to-formula original
truth with the append law for an arbitrary retained theory. Concrete source
lowering, local and aggregate-family completeness, arithmetic admission, prepared
owner identity, cursor coverage and bounded execution remain separate obligations.
The occurrence-count fuel bound does not bound Rust join or expression work;
less retained materialization need not mean less replay work. See the
[constraint-stream guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/streamed-constraints.md).

[`StreamedRegions`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/StreamedRegions.lean)
adds a sufficient original-candidate region test using the existing `Cube` and
`FormulaBounds.Sure` readings. Positive and double-negated atoms held in the
lower bound, and default-negated atoms absent from the upper bound, make a
normalized body true in every interpretation between those bounds. One
authenticated original constraint occurrence with that body excludes every
answer set of the retained theory plus the constraints from the region.
`necessary_selection_preserves_witness` preserves existence of a sure witness
when every sure occurrence passes the supplied selection. The sign law supplies
the necessary held/cut conditions for positive-row and signed-predicate filters;
the predicate domain must include each admitted atom, including unsupported
negative occurrences. The lemma does not prove Rust's row-to-dense-ID mapping,
join traversal, or equivalence of resource-limited prefixes.
Hybrid join-plan reuse adds a representation obligation: the cached order and
comparison schedule must equal fresh planning for the same rule, empty outer
binding and completed support owner. Candidate truth, bindings and arithmetic
failure state must not enter the cached plan. Rust owner checks and repeated-scan
tests exercise this boundary; the semantic selection lemma does not prove the
Rust cache or its storage accounting.
`scan_refutes` supplies the soundness premise of `CoverageTree.refuted`; it does
not prove that Rust constructs a coverage tree. A completed sufficient scan
without a witness is only `NotRefuted`, not original satisfaction; an interrupted
scan is not refutation. The final exact per-candidate checks retain their full
family-coverage obligation. Concrete lowering, discharged guards, complete body
readings, catalog-coordinate identity and sound candidate bounds remain premises
to establish. This law does not authorize applying the same original-constraint
hook inside a proper-subset search of the frozen reduct.

For membership checking, clause search restricts candidates to the least
interpretation; region search may still propose a larger original model of a
positive cycle. The positive checker separately authenticates original
satisfaction and compares the candidate with the least consequences. A distinct
original model is refuted: the least consequences are a proper-subset model of
its frozen producer reduct, and `constraints_frozen` supplies satisfaction of
the constraint reducts even when the least set fails the original constraints.
The checker accepts exactly the least original model. This application still
depends on the complete root partition and retained-owner correspondence above.

The head-element laws assume a correctly identified activity family. Explicit
aggregate elements use complete tuple keys; ordinary Boolean choices use original
source occurrences, with local witnesses coalesced within an occurrence. Ordinary
atomic choices distinguish their default-negation sign as well as their atom.
The Rust
source adapter receives original occurrences from themelios before program-set
collection and checks their Boolean element locations against the original
syntax. It retains complete source identities and separate enclosing-rule
scopes. Tests cover duplicate rules, separate files and finite interpretations;
proving this adapter implements the Lean family remains a separate obligation.

The shared Rust
[`HeadLiteral`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ir.rs)
retains the sign and operand. Its `positive_atom` operation controls producer
eligibility, while
[`Builder::head_literal`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground.rs)
constructs signed truth. This is the implementation distinction corresponding to
the signed activity and permission laws. The finite `M/J` tests inspect the whole
admitted theory, including necessary-support guards and candidate-frozen bounds;
they do not establish pointwise equivalence between unguarded source formulas and
every internal activity node.

[`Builder::initialize`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground.rs)
establishes the named `FALSUM` and `VERUM` nodes used by formula construction.
Their mathematical meanings agree with `GroundGuards.constant`; its
[`constant_original` and `constant_frozen`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/GroundGuards.lean)
laws state that these constants retain their truth in every interpretation and
frozen reduct. Rust tests check the emitted constants through original and
frozen satisfaction, and check the node/work limits and counter restoration on
initialization failure. The named indices and those operational properties
remain implementation obligations, not consequences established by the Lean
laws alone.

[`OrderedHeadActivity`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/OrderedHeadActivity.lean)
relates selected signed tuple activity to an ordered value reduction. In Rust,
[`contribution`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_head_aggregate.rs)
borrows the complete first tuple value, and `HeadContributions` retains it for
the existing value-extremum lowering. The laws require complete-key coverage,
comparison properties and a logical empty value. They do not prove that the
Rust comparator, source join or charged value copy realizes those premises.

[`OrderedBounds`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/OrderedBounds.lean)
separates a numeric measure from a logical bound whose order against every
integer is the same. `constant_aggregate` requires a complete tuple-mask carrier
and constant comparison on every mask; it then preserves both original truth
and arbitrary frozen `M/J` queries. `measured_head_in_context` preserves the
separate positive permissions when the bound is replaced. In Rust,
[`numeric_comparison`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground.rs)
uses the existing logical comparator after source and tuple validation. The
concrete comparator, binding coverage and resource accounting remain executable
correspondence obligations. Excluding nonnumeric bounds from the numeric count-plan
certificate is a separate runtime admission rule, not a consequence granted by
the theorem.

[`HeadContributions`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/HeadContributions.lean)
keeps numeric normalization independent of head permission. Missing/nonnumeric
sum values and nonpositive positive-only weights are neutral contributions;
`neutral_permission_is_not_truth` proves that their permission still matters in
the frozen reduct. Removing an absent bound preserves the permission formula in
arbitrary context. These laws apply after complete active tuple keys have been
coalesced. The [head contribution contract](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/head-contributions.md)
separates formal source translation, executable evidence and reference differences.
Its declared missing-extremum extension projects present first values before
the existing ordered reduction. `complete_extremum_conservative` preserves the
prior complete-value domain; `selected_extremum_values` requires complete key
coverage. `extremum_head_in_context` lifts an implementation's exact original
measure truth through the candidate-only bound with unchanged permissions.
This does not prove source coverage or Rust refinement.

[`ObjectivePriorities`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ObjectivePriorities.lean)
keeps weight, priority, tuple and eligibility in one resolved row.
`completed_presence` assumes exact eligible binding coverage; `partition_vector`
and `partition_optima` preserve the cost vector and all optimal ties under a fixed
priority layout. The Rust
[`Preparation::specialize`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground/objectives.rs)
resolves each template from the same binding while retaining its original-model
condition. These laws do not prove that possible aggregate support establishes
priority presence, that the source join is complete, or that checked arithmetic
and bounded execution implement mathematical evaluation. Objective laws also do
not establish search completion or alter answer-set acceptance.

[`ObjectiveConditions`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ObjectiveConditions.lean)
evaluates conditions in the supplied original interpretation.
Its `formula_query_truth` law permits reading a formula through a closed Boolean
query, with an explicit map from source atom identities to queried atoms.
Implication becomes `not antecedent or consequent` only for this original-truth
operation. `formula_query_changes_reduct` gives a counterexample to using that
conversion as a program rewrite: at `{a}`, the empty interpretation satisfies
the reduct of `a → a`, but not that of `not a ∨ a`.
[`ObjectiveEligibility`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ObjectiveEligibility.lean)
separately proves a three-valued truth cover: absent rows cannot contribute,
required rows are true, and optional rows retain both possibilities. Optional
does not assert that a row can be realized in an answer set. Complete source
bindings and a sound activity classification are premises, not consequences of
the truth algebra. The source analysis still needs its own refinement.

[`SourceSupport`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SourceSupport.lean)
connects completed producer closure to answer-set coverage under its explicit
`ProjectionCompatible` premise. Restricting an original model to the closed
carrier must preserve that model's frozen reduct; minimality then excludes
atoms outside the carrier. `completed_activity_covers` derives optional/absent
truth coverage for objective queries. Proposal monotonicity is a separate
hypothesis of the finite-stage containment law, not a termination theorem.

[`NormalSupport`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/NormalSupport.lean)
discharges that projection premise for possible heads of mathematical normalized
rules, and for any producer containing their proposals. Original satisfaction
supplies each required head's membership in the model; producer closure supplies
its membership in the carrier. Constraints survive restriction under unchanged
gates. The normal/Ferraris bridge then supplies the same frozen-reduct verdict.
This advances the mathematical producer argument without certifying Rust's
source bindings, rich heads or aggregate assignment machinery.

`NormalSupport.propose_gate_independent` states that changing only normalized
candidate gates leaves possible heads unchanged at every carrier. The eager
support scheduler admits flat ordinary negative and double-negative non-inputs
under its existing first-new-positive-occurrence partition. Rust must preserve
the positive bindings, checked scalar operations, source occurrence IDs and
old-head publication history; the emitted original formulas retain the gates.
The law does not equate answer sets of programs with different gates.

[`ProducerScheduling`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ProducerScheduling.lean)
retains original producer identities beside their ground rules. Complete
positive-input registration ensures every newly enabled body wakes its producer.
If every old head proposal is already published, selected proposals plus the
current carrier equal the full inflationary step; an empty selected set then
establishes possible-head closure. Zero-input producers require complete
bootstrap. The Rust positive-flat source plan preserves original IR slots,
uses signed-predicate reverse postings and a packed wake set, and retains the
existing first-new binding partitions. Complete source instantiation, exact
postings, checked work/storage and all-success round publication are executable
correspondences. Final constraints and answer-set membership remain separate.

The Rust [support builder](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support.rs)
creates `CompletedCatalog` only after an admitted round establishes no new atom.
For selective traversal, completed old-head history and complete wake coverage
justify that conclusion even when no producer needs another snapshot. Its immutable
`CompletedSupport` view supplies the same typed rows to source activity,
projection-domain preparation and final grounding. Intermediate snapshots and resource-stopped rounds cannot
supply this capability. The types enforce the completion handoff; they do not
prove that Rust's source generation establishes `ProjectionCompatible`.
The [support proof guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/source-support.md)
identifies the remaining binding, key/value coverage, arithmetic and resource
correspondences. Recursive producer syntax alone neither prevents finite
completion nor establishes it.

[`ObjectiveConditionTable`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ObjectiveConditionTable.lean)
connects the finite node-table representation to those unfolded queries. Each
operand names an earlier node. The invariant equates each stored Boolean with
the original truth of its unfolded query. One node preserves that invariant;
induction over the remaining nodes gives the complete table. Backward admission
separately proves that all lookups succeed. The result is the last node, or true
when the table is empty.

For example, `atom a; neg 0; disj 0 1` shares the first node and computes the
original truth of `a or not a`. This is a query-evaluation law, not permission to
replace that formula inside the original program: a program transformation must
also preserve frozen-reduct truth. The [proof reading](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/objective-condition-table.md)
gives the named steps and explicit premises. Rust atom conversion, validation,
indexing, allocation and work/cancellation behavior remain unproved executable
correspondences.

`ObjectivePriorities.zero_slot_comparison` and `zero_slot_optima` show that an
always-zero slot at a fixed priority position preserves ordering and all optimum
ties. They do not establish an identical raw priority layout in clingo. Proving
an extra slot always zero requires the source-coverage and actual-condition
arguments above; a zero observed in one answer is insufficient.

[`Observations`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Observations.lean)
connects complete query bindings to the distinct enabled-value channel, under
explicit binding and value-expansion coverage. The same row supplies its
condition truth and constructed values. Its family laws attach a display to
each original interpretation and recover the exact original list, preserving
order and multiplicity even when displays coincide. The world-view corollary
requires an already established `WorldViews.Represents` premise; output does
not prove membership or enumeration coverage.

`Observations.necessary_filter_preserves_terms` permits discarding rows that
cannot enable a completed query. Predicate equality supplies such a necessary
condition for an atom pattern. Rust locates that predicate's contiguous range in
the canonically ordered model, then matches complete tuples. Correct binary
bounds, cursor restoration, source-alternative identity and error ordering remain
implementation obligations; the theorem does not verify that Rust code.

The [observation reading](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/observations.md)
relates these laws to scoped queries, complete tuple keys and the Rust compiler
and evaluator. Source safety, anonymous projection, checked arithmetic,
construction, ownership and resource/cancellation completion remain executable
refinement obligations. The laws apply after completed evaluation, not to an
error prefix or an unfinished display.

`Observations.shared_equality_choice_exact` states the finite two-edge equality
law with one middle value and an arbitrary surrounding guard. The Rust planner
retains each original comparison while sharing that value through a bound value slot.
Structural capture reads the complete retained value and its typed components;
aggregate capture reads the actual supplied-model extremum. Source scheduling,
matching, widened numeric comparisons, copied-value charges and cleanup remain
concrete correspondence obligations.

[`AggregateInvariants`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AggregateInvariants.lean)
states when optional tuple keys cannot change a required sum or extremum.
The source [measure carrier](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_objective_dependencies/presence/flat/carrier.rs)
coalesces complete keys and classifies required and possible activity.
An invariant result is the singleton case.
[`SourceMeasures`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SourceMeasures.lean)
proves finite carrier membership and coverage of actual active selections.
Optional keys need not be independent in an answer. The concrete source
classification, carrier reduction, checked arithmetic and unary transport remain
implementation obligations. The mathematical list enumerator is a reference
definition; it does not verify the Rust subset-sum or extremum operations.

[`IntegerEnvelopes`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/IntegerEnvelopes.lean)
proves directed bound coverage and integer floor/ceiling laws.
`filtered_bindings_exact` states that a covering envelope followed by the original
guard recovers exactly the satisfying bindings. The Rust
[envelope analysis](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_binding_guard/envelope.rs)
uses these mathematical obligations. Normalizing expressions, scheduling
endpoint inference, checking finite-width arithmetic and enumerating the
resulting intervals remain concrete refinement obligations.

The [affine reader](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_binding_guard/envelope/affine.rs)
requires source-normalized expression plans.
[`prepare`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ir.rs)
applies checked bottom-up normalization before compiling those plans: closed
arithmetic subtrees have become constants. Supported pool selection and interval
lowering preserve this property; intervals become variable slots. The reader
does not substitute a binding's values. An expression such as `X-X` therefore
remains syntactically open even when its affine coefficients cancel, and cannot
supply a closed multiplication factor. This compiler invariant is a premise of
the implementation review, not a theorem of `IntegerEnvelopes`.

Affine coefficients describe mathematical integers. The retained
[guard evaluator](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/evaluation.rs)
still checks the original expression's scalar arithmetic. Coefficient and bound
capacity refusals remain distinct from source arithmetic errors; neither is an
arithmetic value or evidence of completed enumeration.

[`EvaluationPrefix.root_preservation`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/EvaluationPrefix.lean)
equates returning a final operation's result with appending it to the completed
prefix and observing the last value. Both strict schedules preserve the first error.
The Rust [expression evaluator](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/evaluation.rs)
uses one checked operation for intermediate nodes and the root. The law assumes
pure partial operations and stopping at the first fault. It does not cover the
source mode's missing-operand mask or continued independent branches after a
zero divisor. Source-plan validity, operand indices, checked scalar
arithmetic, resource charges, allocation and cleanup remain implementation
correspondences. A smaller scratch prefix does not establish a timing or
process-memory result.

`EvaluationPrefix.reset_preservation` states that an empty live prefix hides all
earlier workspace values. A Rust join lends its existing workspace to partial
comparisons, binding generators and final filters. Each evaluation returns an
owned value and clears its prefix before the next borrow. Preserving expression
order, complete-filter error precedence, copy charges and cleanup is a concrete
caller obligation. The reset law neither establishes source-family exclusions
nor permits an earlier final filter to hide an independently required check.

[`ProjectedConditionals`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ProjectedConditionals.lean)
separates anonymous witness disjunctions, signed source alternatives and
universal condition rows. The Rust
[`Projection`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ir.rs)
has flat-argument and structural-witness representations. `Builder::project`
completes their witness formula before the conditional compiler applies its
sign. Structural matching uses a private binding frame whose input prefix
excludes later outer bindings. This implements the intended quantifier order;
source support completeness, frame construction and matcher correspondence
remain separate proof obligations. A resource stop cannot establish an empty
completed carrier.

[`TernaryWatch.remaining_unique`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/TernaryWatch.lean)
identifies the one position outside two distinct watches in a three-position
clause. `replacement_exact` equates a covering generic scan with one availability
test at that position. The Rust [replacement operation](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-sat/src/search.rs)
uses this index calculation. Its watch registry must establish valid distinct
positions and keep the assignment fixed during inspection. Registry updates,
candidate ordering, charged work, cancellation and CNF-to-reduct correspondence
remain separate executable obligations.

## Read hypotheses as caller obligations

The tight device checker packs producer support into 32-bit words.
`TightEvaluation.head_support_append` proves that splitting the producer list
joins each head's support with Boolean OR; `head_support_true` connects that
support to an enabled original producer. The shader represents this OR by
atomic updates to head bits by default. The grouped support primitive gives each
word one owner that reduces its complete producer group.
`TightEvaluation.head_support_group` proves that exact group membership preserves
support for every atom owned by the group. This is a Boolean witness law;
preserving original occurrence counts and charged work requires a separate
argument. Rust grouping, word addressing, atomic execution, barriers and
readback remain unproved implementation correspondences. The packed membership
refinement below concerns a different, 64-bit Rust representation and does not
certify either shader schedule.

CPU and device membership can consume the same immutable original-theory
`TightPlan`; preparation does not itself classify a candidate. A completed
original-root and ranked-support check uses
`TightEvaluation.computed_verdict_sound`. Failed support under complete producer
coverage uses `TightEvaluation.unsupported_refutes`: every stable interpretation
of that grammar must support each present atom. This rejection needs no rank
assumption and is distinct from failure to model an original root. Accordingly,
the device adapter maps a stable verdict to `NoProperSubset`, original-root
failure to `NotModel`, and authenticated failed support to `Refuted`. The
primitive's failed-support variant is named `Residual` because it does not
return a deletion witness; that name alone supplies no refutation premise.

This shared semantic plan does not identify host scheduling with device work.
Automatic device execution selects the tight checker only after successful
certificate preparation. A theory admitted only by `PositivePlan` still uses
the general device checker. Exact indexed producers, full candidate coverage,
original-root truth, completed scans and decoded verdict identity must satisfy
the respective laws; missing certificates and interrupted or malformed device
results establish none of their conclusions. Rust certificate compilation and
WGSL execution remain outside the proved correspondence.

The static closure transport validates a nonzero submission epoch, input-world
ordinal and completion marker before publishing any batch. The marker is written
after the workgroup's closure writes synchronize. Exact record count, verdict
bits, closure padding and the original seed's gate projection are also checked.
Pure malformed-record tests establish refusal at this host boundary; they do not
establish that a particular device ran the shader. Framing does not independently
prove least closure or constraint satisfaction, and the existing semantic laws do
not certify the Rust/WGSL protocol or arbitrary driver behavior. Physical tests
retain independent complete-closure comparisons across resident submissions.

For a frozen mask, correctness means agreement with the fixed candidate's
classical truth, not merely matching dimensions. For lazy inference, final
source coverage means coverage at the final positive snapshot, not a successful
earlier scan. For an aggregate, a finite possible carrier is not proof that
every element is realized in a model. For a certificate, original producers and
their rank must satisfy the actual theorem's premises.

The library includes counterexamples to tempting shortcuts: positive
disjunction without a least model, cross-world joins without a common witness,
stale membership omitting newly enabled truth, and empty delivery hiding a valid
answer. These identify the exact invariants an implementation must preserve.

## What the checks establish

`lake build` checks the declared Lean package. Its
[`Audit.lean`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Audit.lean)
and maintained theorem index provide additional declaration and axiom-accounting
checks. An axiom report describes transitive logical dependencies; it is not an
audit of memory safety, allocation, integer overflow or device execution.

Rust unit tests, generated semantic comparisons, resource-boundary tests,
external clingo comparisons and physical GPU checks supply executable evidence
at their respective scopes. Agreement on finite tests is useful but not a proof
of every admitted program. A physical test must establish actual submitted work
on the requested adapter; CPU-authored readback tests alone cannot do that.

The current result is a checked mathematical library together with a tested
native solver. Describing the whole Rust/GPU solver as formally verified would
exceed the established correspondence. Progress consists of closing particular
arrows in this table while preserving explicit limits, not of replacing those
limits with a proof count.

The optional [packed-membership refinement](membership.md) checks the extracted
Rust `Interpretation::contains` body against packed-bit membership under a
storage invariant. It has a separate toolchain and explicit translation and
library-model assumptions; it does not close the whole evaluation layer above.

The [neuromorphic appendix](../appendices/neuromorphic.md) applies the same
discipline to proposed event backends: it separates existing mask and inference
laws from the unproved transport, epoch and completion correspondences.
