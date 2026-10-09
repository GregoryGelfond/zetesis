# zetesis proof library

This Lean library formalizes ASP semantics and reusable algorithms over programs
and interpretations. Separate representation results connect that mathematics
to zetesis's grounding, reduct checking and complete search. The library is
independently usable without the Rust solver. The package uses **Lean 4.33.1**
and its standard library, with no external package dependencies.

Start with [Part III of the manual](../docs/book/lean/foundations.md) for the
semantic foundations and their relationship to the solver. The [reading guide](guide/README.md)
organizes results by logic-programming questions. [STYLE.md](STYLE.md) describes
how declarations, named claims and nested justifications make the proofs readable.
The checked Lean source remains the mathematical deliverable.

## Find a definition or theorem

[theorems.json](theorems.json) is the authoritative inventory of qualified theorem
names and source locations. [Zetesis.lean](Zetesis.lean) imports the complete
library; consumers can instead import individual modules and their dependency
closures. The manual's [theorem map](../docs/book/lean/theorems.md) supplies a
conceptual overview.

| Question | Starting point |
|---|---|
| When does a zero-divisor instance require a warning or refusal? | [ArithmeticFamilies](Zetesis/ArithmeticFamilies.lean) requires a defined witness in the same complete family, permits empty joins, preserves fatal refusals and makes outer-binding scope explicit. |
| What is an answer set under the formula reduct? | [Ferraris](Zetesis/Ferraris.lean) defines truth, the frozen reduct and subset minimality. |
| Can support guards share a condition? | [SupportTransposition](Zetesis/SupportTransposition.lean) transposes complete head-condition incidences, preserving the whole guard family's original and arbitrary frozen truth and answer-set membership in unchanged context. Compiler coverage, provenance and resource bounds remain separate obligations. |
| Does computed frozen-DAG truth equal reduct satisfaction? | [ReductEvaluation](Zetesis/ReductEvaluation.lean) derives the frozen mask from original evaluation and proves exact node and root truth for any tested interpretation. |
| Can finite subset search decide answer-set membership? | [FiniteMembership](Zetesis/FiniteMembership.lean) constructs subset coverage and proves its executable checker agrees with the Ferraris definition. |
| Can a sequential rule scan compute the least closure within a finite bound? | [FiniteClosure](Zetesis/FiniteClosure.lean) proves sound insertion, closedness at an unchanged scan, completion within the head-list bound and exact normalized seed acceptance. |
| Does packed construction establish its own representation invariant? | [PackedInterpretations](Zetesis/PackedInterpretations.lean) proves exact membership, storage and padding for the Lean word operations, followed by exact 64-to-32-bit export. Current Rust extraction remains separate. |
| Are the evaluator's indexed reads present? | [IndexedEvaluation](Zetesis/IndexedEvaluation.lean) proves checked node, mask and root reads complete on admitted input and preserve reduct satisfaction. |
| Can finite connectives read an operand arena exactly? | [OperandArena](Zetesis/OperandArena.lean) proves checked ordered spans and one layer's original/frozen truth. [OperandTable](Zetesis/OperandTable.lean) composes admitted layers into original and paired original/reduct passes. These mathematical laws do not establish Rust storage or scheduling. The separate [native refinement](../refinement/evaluation/README.md#native-operand-row-getters) covers generated inline/span reads and evaluation under its stated contracts and source qualification. |
| Does a binary counter visit every proper subset? | [SubsetCounter](Zetesis/SubsetCounter.lean) proves carry, population tracking, finite completion and exact coverage for distinct supplied atoms. |
| Does packed normal closure compute the same least consequences? | [PackedClosure](Zetesis/PackedClosure.lean) proves direct 32-bit initialization, frozen selection, immediate scans, stopping flags and least-closure correspondence. |
| Do the final packed checks decide seed acceptance? | [PackedAcceptance](Zetesis/PackedAcceptance.lean) combines closure, selected constraints and carrier agreement, with seed admission and gate coverage explicit. |
| Does a word iterator preserve the exported interpretation? | [PackedWordIterator](Zetesis/PackedWordIterator.lean) proves individual checked reads, cursor progress, exact output and exhaustion. |
| Do packed bit updates enumerate the proper subsets? | [PackedSubsets](Zetesis/PackedSubsets.lean) derives selected coordinates, set/clear carry, population and complete semantic coverage. |
| Can the frozen-reduct search stream those counter states? | [CounterSearch](Zetesis/CounterSearch.lean) proves checked evaluation with one computed mask, early countermodel termination and correctness of every completed verdict. |
| Can packed updates and streaming checking compose? | [PackedCounterSearch](Zetesis/PackedCounterSearch.lean) proves all-fuel correspondence and exact completed membership without materializing subset visits. |
| Can admission establish the evaluator premises? | [TheoryAdmission](Zetesis/TheoryAdmission.lean) derives topological, atom and root bounds from ordered checks, then proves checked reduct evaluation completes. |
| When does one least consequence set determine the answer? | [PositiveTheory](Zetesis/PositiveTheory.lean) admits positive atomic-head producers and constraints, including cycles. Its least producer closure is the unique answer set exactly when it satisfies every constraint. |
| May those constraints contain arbitrary formulas? | [ConstrainedPositive](Zetesis/ConstrainedPositive.lean) proves that a satisfied constraint has a tautological frozen reduct. Arbitrary constraints filter the original answer-set family; failure at positive least consequences establishes absence of an answer set, not necessarily absence of classical models. |
| When can positive definitions be evaluated after a base answer set? | [TerminalDefinitions](Zetesis/TerminalDefinitions.lean) proves a unique extension when every definition reads only base atoms and the base cannot read derived atoms. Source recognition and executable reconstruction remain separate obligations. |
| When may a constraint family be checked in bounded chunks? | [StreamedConstraints](Zetesis/StreamedConstraints.lean) proves a bounded scan's checked-prefix invariant and completed partition equivalence, then composes pointwise original evaluation with the arbitrary-theory constraint-filtering law. Its [guide](guide/streamed-constraints.md) states the remaining source and execution obligations. |
| May ranking follow those completed constraint checks? | [StreamedOptimization](Zetesis/StreamedOptimization.lean) filters a complete retained-theory answer-set list by completed partition scans, then proves its minimum integer cost and ties are exactly the full theory's optima. |
| When can a streamed constraint refute a whole candidate region? | [StreamedRegions](Zetesis/StreamedRegions.lean) connects held/cut normalized literals to a sure original body, then authenticates one source witness to exclude every answer set in the region. No witness, even after a completed scan, is not a satisfaction certificate. |
| When may a streamed constraint force a candidate atom? | [StreamedConsequences](Zetesis/StreamedConsequences.lean) proves that a scalar-passing original constraint with one open occurrence preserves all models and answer sets when its pivot is falsified. It separates finite decision progress from source-query completion and interruption. |
| Can one encoding represent the reduct for many candidates? | [ParametricReduct](Zetesis/ParametricReduct.lean) preserves frozen-reduct satisfaction under subset membership and authenticated original truth. CNF construction and query execution remain implementation obligations. |
| When may grounding skip an unchanged producer? | [ProducerScheduling](Zetesis/ProducerScheduling.lean) preserves the full inflationary step under complete dependency registration and previously published old heads. Original producer identity is retained. |
| Why does normalized least closure suffice? | [NormalFerraris](Zetesis/NormalFerraris.lean) proves equivalence with [Semantics](Zetesis/Semantics.lean); the [bridge guide](guide/normal-ferraris.md) explains the argument. |
| When may closure rounds omit old bindings? | [DeltaRounds](Zetesis/DeltaRounds.lean) combines first-new binding coverage with already-published old heads and exact constraint history. The [reading guide](guide/delta-rounds.md) explains bootstrap and the separate Rust obligations. |
| When may source domains remove an unfinished join prefix? | [DomainBindings](Zetesis/DomainBindings.lean) proves variable-meet necessity and exact ordered continuation preservation under explicit coverage. The [reading guide](guide/domain-bindings.md) distinguishes this from equality of local row matches. |
| What does a completed membership result establish? | [CertifiedExecution](Zetesis/CertifiedExecution.lean) and its [worked proof](guide/certified-membership.md) separate a verified result from unfinished work. |
| When does enumeration establish the original world view? | [WorldViews](Zetesis/WorldViews.lean) composes exact membership, original candidate coverage and complete capture; optimal selection remains distinct. |
| How do shown terms preserve full answer identity? | [Observations](Zetesis/Observations.lean) separates complete enabled-value coverage from decorating original models. Its [reading guide](guide/observations.md) states the binding, scope and implementation premises. |
| How do weighted heads handle shared atoms and tuples? | [WeightedHeadActivity](Zetesis/WeightedHeadActivity.lean) connects distinct tuple sums to canonical guards and candidate-only bounds. |
| How do extrema heads handle those aliases? | [ExtremumHeadActivity](Zetesis/ExtremumHeadActivity.lean) connects selected complete tuples to numeric extrema, explicit empty results and independent atom permissions. |
| How do logical values extend extrema heads? | [OrderedHeadActivity](Zetesis/OrderedHeadActivity.lean) connects signed tuple activity to ordered selection over an arbitrary value carrier; complete-key coverage, comparison laws and logical empty values are explicit premises. |
| When is a logical bound constant for a numeric aggregate? | [OrderedBounds](Zetesis/OrderedBounds.lean) preserves original and frozen aggregate truth when the comparison agrees across every finite tuple mask; concrete term ordering and checked evaluation remain premises. |
| How do variable objective priorities preserve correlated values? | [ObjectivePriorities](Zetesis/ObjectivePriorities.lean) selects numeric weight/priority pairs from the same completed row and preserves costs and optimum ties under per-priority evaluation. Exact source eligibility is a separate obligation. |
| How do shared objective query nodes retain original truth? | [ObjectiveConditionTable](Zetesis/ObjectiveConditionTable.lean) proves backward-indexed table correspondence with unfolded conditions, including the empty-table result. The [reading guide](guide/objective-condition-table.md) explains the invariant and remaining implementation obligations. |
| When can an aggregate have one fixed value despite optional tuples? | [AggregateInvariants](Zetesis/AggregateInvariants.lean) proves that zero optional contributions preserve sums and dominated optional values preserve extrema, then composes a fixed priority with complete eligible objective rows. Source classification and checked arithmetic remain separate obligations. |
| When may a dependent aggregate reuse a preceding carrier? | [AggregateDependencies](Zetesis/AggregateDependencies.lean) gives a finite restart algorithm over fixed support and complete projected inputs. It preserves ordered parent/child rows, typed build failures and the original clause sequence. A separate support law coalesces completed equal-projection observations while preserving head union and defined/first-zero evidence. Source dependency extraction, completeness and Rust resource behavior remain separate obligations. |
| How do changing aggregates supply source priorities? | [SourceMeasures](Zetesis/SourceMeasures.lean) defines the finite carrier between required and possible complete keys. The carrier may contain unrealized values; original conditions determine costs. Complete whole rows still determine numeric priority presence. |
| When does a completed producer carrier cover answer sets? | [SourceSupport](Zetesis/SourceSupport.lean) connects closure and reduct minimality under an explicit projection premise; its [reading guide](guide/source-support.md) distinguishes possible support from truth in an answer set. |
| Which producer establishes that projection premise? | [NormalSupport](Zetesis/NormalSupport.lean) proves it for the mathematical normalized-rule producer and any producer covering its proposals. Concrete source instantiation remains a separate obligation. |
| How do comparison generators retain all valid bindings? | [IntegerEnvelopes](Zetesis/IntegerEnvelopes.lean) proves directed path bounds, affine endpoint rounding and finite envelope enumeration. The retained whole guard selects exact bindings; source safety and checked runtime inference remain separate. |
| Can a truth constant supply atom support? | [BooleanHeadElements](Zetesis/BooleanHeadElements.lean) separates Boolean activity from atom permission and preserves bounded head groups in context. |
| What changes when a choice operand has default negation? | [SignedHeadElements](Zetesis/SignedHeadElements.lean) keeps signed contribution identity, candidate-frozen operand truth and positive-only permissions separate; eligibility still has its own reduct. |
| Where does negation apply to an anonymous consequent? | [ProjectedConditionals](Zetesis/ProjectedConditionals.lean) separates complete anonymous witness projections, signed source alternatives and universal condition rows. |
| How does a consumer apply the normal/Ferraris bridge? | [The checked choices example](Zetesis/Examples/Choices.lean) proves that `{a}` is an answer set of the guided tour's two-rule program through its least reduct closure. |
| Does discovering an atom make it true? | [AtomCatalogs](Zetesis/AtomCatalogs.lean) preserves local identities and selected interpretations when pending atoms are appended; discovery alone adds no truth. |
| Can separate base and suffix indexes provide exact lookup? | [ClosedCatalog](Zetesis/ClosedCatalog.lean) combines sound, complete component queries. Duplicates do not invalidate absence; identifying a unique position additionally requires global uniqueness. |
| Can distinct occurrences share a canonical atom identity? | [CanonicalCatalog](Zetesis/CanonicalCatalog.lean) preserves selected interpretations through occurrence decoding, reordering and duplicate removal. Append requires the old prefix; transfer between owners requires decoded agreement. |
| Can template substitution use canonical term identities? | [CanonicalTemplates](Zetesis/CanonicalTemplates.lean) proves that decoding commutes with ordered substitution, preserving repeated arguments and absent variables. Scope and readable-prefix checks remain Rust obligations. |
| Can sparse interpretations use vocabulary coordinates without enumerating the carrier? | [CarrierCoordinates](Zetesis/CarrierCoordinates.lean) requires unique signature and domain decoders for identity, and order-preserving and order-reflecting decoders for ordering. Sparse selection needs no whole-carrier cardinality; shared vocabulary alone does not identify atom-row positions. |
| When does a collective reservation cover active closures? | [StorageOwners](Zetesis/StorageOwners.lean) sums pointwise owner bounds, including one shared component, idle retained sizes and active maxima; it also retains the uniform worker-allowance laws. Actual Rust capacity measurements, disjoint ownership, allocation failures and worker lifetimes remain implementation obligations. |
| When may a constraint over a keyed value ask for one atom instead of reading every value? | [KeyedConstraints](Zetesis/KeyedConstraints.lean) proves that a disequality over a keyed value fires exactly when the key's body holds and the demanded atom is absent, that a digit and a carry solve their equation exactly as the truncating remainder and quotient, and that constraints firing together on every answer set of the rest of the program may replace one another. That a relation is keyed is a premise. |
| May a closure round record heads as positions and take a block of rows whole? | [RowSteps](Zetesis/RowSteps.lean) proves that marks under an injective position are exactly the new atoms and count them, and that the heads derived through an occurrence whose value no guard reads are the heads of the rows its relation holds, place for place under a shared rank. The concrete index and the rule recognition remain implementation obligations. |
| When may the closure route's counter narrow its region by the program's two closures? | [Bounds](Zetesis/Bounds.lean) proves that the iterated narrowing of the undecided region by its lower and upper closures keeps every accepted seed, that a constraint firing in a lower closure refutes the region, and that a held atom no seed of the region can derive refutes it. That the Rust closures are the least fixed points of the two readings remains a Rust obligation. |
| How do regions cover the candidate space exactly? | [Search](Zetesis/Search.lean) defines cubes and the coverage tree whose `split` parts a region into two disjoint regions, so that the leaves are exactly the accepted seeds of the root. |
| May a region of formula candidates be narrowed by its readings, and is a decided leaf a classical model? | [FormulaBounds](Zetesis/FormulaBounds.lean) gives the sure and never readings of a formula under a region, the knowledge closed over the DAG, the support cut and the sole-support demand, and proves that a fully decided region no reading refutes is a classical model; [FormulaChains](Zetesis/FormulaChains.lean), whose declarations are in the same namespace, lets a chain of one connective be read as one node by counters. |
| Why may the clauses proposer narrow by classical consequence? | [FormulaRegions](Zetesis/FormulaRegions.lean) reads the clause search as the coverage tree: propagation forces and cuts only classical consequences, a conflict refutes, and the reduct still decides every leaf. |
| Why does a covered query tree over a candidate's subsets prove stability? | [ReductRegions](Zetesis/ReductRegions.lean) states the reduct's proper-subset query as a region tree under the candidate's mask: a leaf other than the candidate is a countermodel, a covered tree is stability, and the masked readings are the reduct's. |
| Why does a walk by several workers emit every accepted leaf once? | [Frontier](Zetesis/Frontier.lean) proves that the pending regions, whoever holds each, and the leaves emitted are together a permutation of the tree's outputs after every step in any interleaving, so a walk that empties the frontier emits every accepted leaf once. |
| Which implementation correspondences remain open? | The manual's [proof boundary](../docs/book/lean/correspondence.md) separates mathematical laws from executable refinement. |

For example, `NormalFerraris.answer_set_iff` relates the two independently defined
predicates:

```lean
Semantics.Stable P M ↔ Ferraris.Stable M (NormalFerraris.translate P)
```

Its translation selects applicable ground filters. The direct ground-rule-map
corollary makes prior filter discharge explicit; frozen-subset correspondence
states `J ⊆ M`. No assumption that every atom occurs in a rule is needed because
both sides use the same atom universe.

## Build and audit

Install the toolchain pinned by [lean-toolchain](lean-toolchain), then run from
this directory:

```sh
lake build
lake env lean -DautoImplicit=false -DwarningAsError=true Audit.lean
```

`lake build` compiles the umbrella and all semantic modules.
[Audit.lean](Audit.lean) requests the transitive axiom dependencies of every
registered theorem. The admitted standard logical axioms are `propext`,
`Quot.sound` and `Classical.choice`; project axioms, proof holes and native
proof-evaluation shortcuts are not admitted.

From the repository root, check the retained record with:

```sh
cargo run --locked -p zetesis-maintenance -- proof-record --proofs-dir proofs
```

[verification.json](verification.json) identifies the recorded source hashes,
commands and logs. [axiom-audit.txt](axiom-audit.txt) contains the corresponding
complete audit output. The record checker validates inventories, source
locations, counts, hashes and allowed axioms. It does not rerun Lean or establish
that a command was executed; kernel checking and record consistency are separate
requirements. `scripts/check.sh proofs` runs both.

After changing proof sources, regenerate the theorem index and Audit with the
[inventory command](../crates/zetesis-maintenance/README.md#prepare-proof-library-views),
then use the maintained
[proof capture](../crates/zetesis-maintenance/README.md#capture-current-proof-evidence)
to execute the pinned checks and publish their record. It retains the original
record and bounded command receipts outside the repository and validates current
source and tool identities before publication. Existing records are observations
of their recorded sources; they cannot qualify a later edit.

## Verification boundary

The laws prove their stated mathematical conclusions under explicit premises,
such as finite carrier coverage, complete proposals, preserved atom identity or
agreement of an abstract evaluator. A checked preservation law does not establish
that Rust source recognition or a GPU kernel satisfies those premises.

The library does **not** yet formally verify the complete solver. Remaining
obligations include source-to-ground correspondence, concrete atom and formula
representations, Rust and WGSL execution, and resource, cancellation and output
behavior. The normalized/Ferraris bridge proves semantic translation and
minimality equivalence; it does not certify DAG allocation or physical hardware.
Tests, device qualification and performance measurements provide distinct
implementation evidence.
