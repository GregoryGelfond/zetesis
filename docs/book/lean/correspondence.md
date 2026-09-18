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

The [ownership chapter](../architecture/ownership.md) and
[session example](../rust/sessions.md#reuse-and-identity) connect these obligations
to the maintained implementation.

The CPU source join borrows bound values from its immutable relation snapshot.
Backtracking clears references; emitting a ground instance constructs its owned
values. This changes storage ownership without changing the substitution used by
whole-tuple matching, filters or frozen gates. The implementation obligation is
that each live binding denotes its matched source value until that branch ends,
and emitted values preserve full typed identity. Rust lifetimes prevent the
snapshot from being invalidated while those references are live; identity and
join tests check the concrete behavior. This does not establish source-join
coverage or a Lean-to-Rust refinement.

Gate and consequence membership uses a checked `AtomKey` over that borrowed
assignment. The key denotes the same signed predicate and complete typed tuple
as materialization. Gate lookup and duplicate-head lookup create no owned atom;
only a new consequence is copied into the pending delta. Key construction charges
the argument span separately from catalog lookup receipts, including a deferred
gate with a missing slot. Emitted source instances still own their values.
`AtomKeys.tuple_agrees` equates views agreeing on all requested reads;
`membership_identity` connects successful substitution to extensional tuple
membership. Rust comparison/hash equivalence, index construction and binding
lifetimes remain implementation obligations.

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
handles, while `SeedView` borrows either those handles or an owned `Seed` without
materialization. Arc lifetime safety, program-instance validation, canonical
comparison and the executable candidate cursor remain separate correspondence
obligations. This interpretation law does not establish answer-set membership
or complete candidate coverage.

`GatePositions.atoms_exact` identifies the gate subsequence of an indexed atom
carrier. `retained_position_exact` relates each gate rank to its original dense
position and atom. Rust's `GateAtom` retains an opaque program identity, atom and
checked positive position. `SeedAtom::resolve_in` uses that position in the
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

The scalar lazy closure retains one typed `Catalog` for each predicate. Every
round borrows their existing ordered rows; new consequences remain separate
until that round's complete template scan finishes. Catalogs are consumed once
to assemble the final interpretation. This preserves the synchronous
least-closure argument while removing per-round relation reconstruction.
`RelationExtension` states preservation of old row reconstruction and equality
selection when row references and dictionary meanings survive extension.
Concrete insertion rollback, borrowed access order and catalog work accounting
remain executable obligations. `catalog_work` is a subtotal of oracle work;
subtracting it leaves the other charged source operations, not a runtime estimate.
Control is polled around each bounded catalog operation. Final `Model`
canonicalization retains its separate comparison and allocation contract.

[`ModelSelections`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ModelSelections.lean)
relates selected catalog positions to their interpretation. Canonical ordering
and coalescing preserve the true set; unselected catalog entries supply no truth,
and a renumbering preserves meaning when the selected atoms agree. Rust
[`Model`](../rust/models.md) retains a shared catalog and canonical selection.
Static and formula answers share their program catalogs; completed batched lazy
closures share a catalog frozen after the final complete round. Checked positions,
logical comparison, ownership and allocation remain executable obligations.
The common `ModelRetention` ledger charges each distinct catalog allocation once,
including unselected atoms, then selected-position records per entry and the
consumer's score records. `StorageOwners.sum_within_component_bounds` supplies
the finite-sum bound when those canonical components are completely covered
once. It does not establish the ledger's allocation-identity/hash invariant,
checked arithmetic or transactional publication. Rust keeps a live catalog
handle for every private identity key and commits only after both owner-index
and consumer-slot reservations. Abandoned or refused admissions preserve the
prior retained family; replacement admits the complete new family before
dropping the old one. The measure excludes capacity/allocator overhead and
transient old/new overlap, so it is not an allocated-byte or process-memory bound.

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
`Catalog::take_atoms` transfers completed atoms into the result and resets the
remaining identity metadata. Unlike append, this operation invalidates prior
row and equality IDs. The next candidate cannot inherit those IDs or their truth.
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
remains outstanding. The Rust implementation must additionally preserve this
invariant under word arithmetic, locking, unwinding and cancellation. A joined
batch records spent operations only after all leases settle; a lease is not
evidence of candidate execution or membership.

## Candidate generation and query representation

Candidate restrictions and storage transformations preserve different objects.
A necessary restriction may remove impossible proposals; a query representation
must preserve the same classical assignments. Neither operation changes the
original program or the reduct used to check membership.

| Law | Implementation boundary | Remaining correspondence |
| --- | --- | --- |
| [`GateRestrictions.answer_set_avoids`, `suffix_region_rejected`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/GateRestrictions.lean) | Source-derived positive gate restrictions and binary seed-region skipping | Each witness uses actual unconditional facts, complete source bindings and the stated gate indices. Possible support alone is insufficient. Rust counter jumps and resource accounting need refinement. |
| [`Bounds.narrowed_contains_accepted`, `lower_constraint_refutes`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Bounds.lean) | `Candidates::bounded` narrows the undecided region to a fixed point by the region's lower and upper closures (`definite_closure`, `possible_closure`, the `MustGate` and `MayGate` readings of the shared closure rounds), holds the lower closure's gate atoms in every seed, never offers a gate atom outside the upper closure, and offers no seed when a constraint fires in a lower closure | The Rust closures must be the least fixed points of the two consequence operators over the exact admitted program; the counter's enumeration inside a counted region and the restriction plan's treatment of held premises are Rust obligations. Each offered seed is still checked in full. |
| [`Search.CoverageTree.split`, `split_partition`, `split_disjoint`, `mem_outputs_iff`, `Bounds.conflicting_atom_refutes`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Search.lean) | `zetesis_cpu::regions::Traversal`, shared by both routes: a region is narrowed by the caller's narrowing, refuted, offered as one candidate when decided, split on the atom its narrowing preferred or else its highest open atom into the out and in regions, or counted as a flat interval when its narrowing decided nothing beyond the split and the caller counts; the closure route narrows by its two closures, the formula route by the theory's knowledge; under `--region-workers` several workers walk the same tree over a shared pool of regions | That the Rust split chooses a fresh atom and forms the two cubes of the law, that the counted interval offers exactly the region's seeds, and that the leaf order is the counter's order when no atom is preferred and one worker walks are Rust obligations; with several workers the partition law is what makes every leaf arrive once, and the order is unspecified; the tree's leaves are exactly the accepted seeds only because each leaf seed is checked in full. |
| [`TightPlans.stable_supported`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/TightPlans.lean) | A candidate the complete tight certificate finds with an unsupported present atom is rejected as `Check::Unsupported` without a reduct query; the candidate without that atom is the law's witness | The plan's coverage of the theory's producers is established by its construction, not yet by a proved refinement of that construction; the Rust check's evaluation of producer bodies remains executable evidence. |
| [`DisjunctiveSupport.answer_set_supported`, `answer_set_supported_with_choices`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DisjunctiveSupport.lean) | Necessary ordinary sole-head support and enabled atomic-choice support | Extraction must cover every asserted root, coalesce repeated ordinary head atoms and recognize exact atomic choices in either operand order using semantic atom identity. Choice bodies remain arbitrary original formulas; they supply permission, not a self-premise. The original theory remains the reduct subject. This is necessary support, not ranked sufficiency; Rust DAG extraction, Boolean encoding and bounded failure remain unproved. |
| [`PackedQueryLiterals.decode_encode`, `packed_truth`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/PackedQueryLiterals.lean) | Packed classical literals in `Cnf` and borrowed clause views | Admission must establish machine representability and valid offsets, including repeated offsets for empty clauses. Natural-number arithmetic does not prove machine operations or allocation. |
| [`IndexedCandidates.index_equals_all_blocks`, `failed_literal_forced`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/IndexedCandidates.lean) | The authoritative semantic-projection exclusion index and independently checked completed assignments | Flat-trie insertion, watches and trial undo must implement exact complete keys. Separate history admission must preserve prior keys and publish no key on refusal; concrete capacity accounting remains a Rust obligation. A failed-literal conclusion needs a completed branch refutation; a stopped trial supplies none. |
| [`OptionalIndex.successor_fits`, `optional_round_trip`, `replacement_commutes`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/OptionalIndex.lean) | Optional positive-successor child links in the compact projection trie | Planned node count must fit 32 bits, decoded links must index allocated nodes, and only complete suffixes may be attached. The laws preserve identity and absence; they do not establish Rust layout, allocation, rollback or control accounting. |
| [`FormulaRegions.classical_consequence_forces`, `classical_consequence_cuts`, `no_model_refutes`, `restricted_consequence_forces`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FormulaRegions.lean) | The clauses method (`--search clauses`) read as the coverage tree of `Search.lean`: a search node is a cube, the atoms its propagation forces or cuts narrow it, a conflict refutes it, and a complete assignment is a leaf the reduct decides; the support restriction is a restriction every stable model satisfies | That the cursor's propagation returns only classical consequences of the clauses it holds, and that those clauses are the theory and restrictions every stable model satisfies, are Rust obligations; the search proposes and never decides membership. |
| [`FormulaBounds.read_sound`, `never_root_refutes`, `known_sound`, `known_mono`, `unsupported_cut`, `sole_support_forces`, `decided_leaf_models`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FormulaBounds.lean) | `zetesis_ferraris::narrow`, the narrowing of the regions proposer in `zetesis-sat`: the sure and impossible readings of every node under a region, the knowledge closed in both directions over the DAG, the support cut and the sole-support demand, to a fixed point; `zetesis_cpu::regions::Traversal` walks the tree of `Search.lean` for both routes | The Rust pass must agree with `read` on the admitted DAG, one visit per node, and `producers` must extract the covered fragment from the roots. The Rust closure must derive only `Known` judgements; the fixed point is the least one because every rule only adds knowledge, and a child region may start from its parent's knowledge by `known_mono`. That a fully decided region no reading refutes is a classical model is `decided_leaf_models`, which is why a leaf is proposed to the reduct without a classical check. An atomic choice read as a producer its body alone supports lies outside the Lean fragment and remains a Rust obligation, as it does for `support_restriction`; so does narrowing a candidate-only restriction without producers. |
| [`ReductRegions.leaf_refutes`, `exhausted_stable`, `countermodels_exact`, `stable_iff_no_countermodel`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ReductRegions.lean) | The reduct's proper-subset query as a region tree under `--search regions`: `ReductQuery` in `zetesis-sat` walks the subsets of a classical model with the frozen reduct's knowledge, returns a leaf other than the candidate as the countermodel and a covered tree as stability | That the masked reading of the original DAG is the reduct's is `FerrarisMask`'s law and the mask's correctness a Rust obligation; that the traversal covers the query root exactly is `Search.CoverageTree`; the countermodel is validated independently before it is returned. |
| [`CandidateCursor.completed_coverage`, `stable_outputs_exact`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/CandidateCursor.lean) | Retained candidate traversal, completed checks and exhaustion under the clauses proposer | Rust traversal must denote the abstract finite forest, preserve its open remainder and block only completed checks. Exact accepted output additionally requires candidate coverage and a correct membership oracle. |

The [execution chapter](../architecture/execution.md) explains these operations
in the solver. The [candidate cursor contract](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-sat/docs/candidate-cursor.md)
and [projection-index contract](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-sat/docs/candidate-pruning.md)
describe their concrete ownership and failure boundaries.

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
unproved implementation bridges. The
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
prefix and observing the last value. Both schedules preserve the first error.
The Rust [expression evaluator](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/evaluation.rs)
uses one checked operation for intermediate nodes and the root. The law assumes
pure partial operations; source-plan validity, operand indices, checked scalar
arithmetic, resource charges, allocation and cleanup remain implementation
correspondences. A smaller scratch prefix does not establish a timing or
process-memory result.

`EvaluationPrefix.reset_preservation` states that an empty live prefix hides all
earlier workspace values. A Rust join lends its existing workspace to partial
comparisons, binding generators and final filters. Each evaluation returns an
owned value and clears its prefix before the next borrow. Preserving expression
order, complete-filter error precedence, copy charges and cleanup is a concrete
caller obligation; the reset law does not justify skipping later expressions
after an earlier final filter rejects a binding.

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
