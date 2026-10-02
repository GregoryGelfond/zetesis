# Reading the zetesis mathematical library

Start with the logical question, then follow the definitions and laws that answer
it. All linked Lean modules are in this package and imported by its checked
umbrella. These paths are a curated introduction; the [full module map](../README.md)
and [complete theorem index](../theorems.json) provide the inventory. Definitions
and theorem hypotheses are authoritative; prose is a reading aid.

| Question | Reading path |
| --- | --- |
| When may an arithmetic-undefined substitution be omitted? | [ArithmeticFamilies](../Zetesis/ArithmeticFamilies.lean) specifies completed-family admission and warnings, distinguishes empty joins from zero divisors, and shows why prefixes and flattened outer bindings cannot justify the verdict. Concrete classification and family coverage remain implementation obligations. |
| Can an aggregate be evaluated directly through the reduct? | [AggregateReduct](../Zetesis/AggregateReduct.lean) formalizes Ferraris Proposition 7: retain the original guard and evaluate the same guard over frozen eligibility. Whole-tuple grouping and concrete arithmetic remain separate obligations. |
| When is parallel signed addition safe from intermediate overflow? | [AggregateRanges](../Zetesis/AggregateRanges.lean) bounds every mathematical intermediate sum by separate positive and negative carrier totals. The actual execution must preserve contribution occurrences and implement the admitted arithmetic. |
| When do total and group capacities imply stronger local bounds? | [PartitionCapacities](../Zetesis/PartitionCapacities.lean) separates the counting argument from the caller's coverage and theory-entailment premises. |
| What makes an interpretation an answer set? | [Ferraris](../Zetesis/Ferraris.lean): formula truth, the frozen reduct, minimality and their connection. |
| Does an executable finite checker implement that definition? | [Finite membership](finite-membership.md) constructs complete subinterpretation coverage and composes it with [computed reduct evaluation](reduct-evaluation.md), without an oracle-agreement premise. |
| How does a finite scan reach least normal closure? | [Executable closure](finite-closure.md) proves a sequential scan invariant, an explicit head-list bound and exact constraint and gate checks. |
| How are finite interpretations represented by machine words? | [Packed interpretations](packed-interpretations.md) proves zero initialization, insertion and numeric word export as a separate representation model. |
| Why does least closure suffice for the normal-rule specialization? | The [normalized/Ferraris bridge](normal-ferraris.md) proves that both independent answer-set definitions coincide, then connects Ferraris membership to [Semantics](../Zetesis/Semantics.lean)'s least closure. [Iteration](../Zetesis/Iteration.lean) addresses bounded closure computation. |
| When does an original positive formula theory have one least answer set? | [Positive atomic-head theories](positive-theory.md) allows monotone And/Or bodies and positive cycles, while checking every original constraint. Its least consequences give an answer set exactly when those constraints hold. CSR execution and source completeness remain separate obligations. |
| Can positive producers coexist with arbitrary constraints? | [Constrained positive theories](constrained-positive.md) proves that satisfied constraints freeze to truth and supply no support. Exact original constraints filter the producers' least answer; a failed nonmonotone constraint does not rule out larger classical models. |
| When do positive definitions give a unique answer-set extension? | [TerminalDefinitions](../Zetesis/TerminalDefinitions.lean) separates base and derived atoms, retains every producer, and proves both directions through frozen-reduct minimality. The base theory may contain arbitrary formulas. |
| Can constraints be checked in bounded source chunks? | [Finite constraint streams](streamed-constraints.md) proves scan accounting, completed partition invariance and filtering of an arbitrary retained theory under complete source coverage and pointwise original evaluation. Pending work is not satisfaction. |
| Can a streamed constraint reject an unfinished candidate region? | [Region refutation](streamed-constraints.md#refuting-a-region-before-it-becomes-a-candidate) proves that one authenticated sure-body witness excludes every original answer set in that region. A completed sufficient scan without a witness does not establish satisfaction. |
| Can one connective network represent every candidate's frozen reduct? | [Parametric reducts](parametric-reduct.md) retains authenticated original implication truth and tests proper subsets against all roots. Concrete CNF encoding, parameter reset and native search remain executable obligations. |
| When can scalar closure use only new row combinations? | [Delta rounds with completed history](delta-rounds.md) separates first-new occurrence coverage, previously published consequences and a whole-family constraint latch. Bootstrap, concrete row matching and completed execution remain explicit obligations. |
| May a closure round record heads as positions, and take a block of rows whole? | [RowSteps](../Zetesis/RowSteps.lean): marks under a position injective on a carrier of every derived head are exactly the new atoms and count them; the heads derived through an occurrence whose value no filter or gate reads are the heads of the rows its relation holds, at the same offset under a shared rank. The concrete index and the recognition of such rules are implementation obligations. |
| When may a constraint over a keyed value ask for one atom? | [KeyedConstraints](../Zetesis/KeyedConstraints.lean): a disequality over a keyed value fires exactly when the key's body holds and the demanded atom is absent; a digit and a carry solve their equation exactly as the truncating remainder and quotient; constraints that fire together on every answer set of the rest of the program replace one another. That the relation is keyed is a premise. |
| When may the closure route's counter narrow its region by the program's two closures? | [Bounds](../Zetesis/Bounds.lean) proves that the iterated narrowing of the undecided region by its lower and upper closures keeps every accepted seed, that a constraint firing in a lower closure refutes the region, and that a held atom no seed of the region can derive refutes it. That the Rust closures are the least fixed points of the two readings remains a Rust obligation. |
| May a descendant read held and cut root coordinates without rebuilding owned atom sets? | [RegionBounds](../Zetesis/RegionBounds.lean) identifies exact partial lookup with the materialized selections and proves equality of both gate readings and completed narrowing. Canonical root ordering, lookup correctness, completed-root coverage and decision commit remain Rust obligations. |
| How do regions cover the candidate space exactly? | [Search](../Zetesis/Search.lean) defines cubes and the coverage tree whose `split` parts a region into two disjoint regions, so that the leaves are exactly the accepted seeds of the root. |
| May a region of formula candidates be narrowed by its readings, and is a decided leaf a classical model? | [FormulaBounds](../Zetesis/FormulaBounds.lean) gives the sure and never readings of a formula under a region, the knowledge closed over the DAG, the support cut and the sole-support demand, and proves that a fully decided region no reading refutes is a classical model; [FormulaChains](../Zetesis/FormulaChains.lean), whose declarations are in the same namespace, lets a chain of one connective be read as one node by counters. |
| Why may the clauses proposer narrow by classical consequence? | [FormulaRegions](../Zetesis/FormulaRegions.lean) reads the clause search as the coverage tree: propagation forces and cuts only classical consequences, a conflict refutes, and the reduct still decides every leaf. |
| Why does a covered query tree over a candidate's subsets prove stability? | [ReductRegions](../Zetesis/ReductRegions.lean) states the reduct's proper-subset query as a region tree under the candidate's mask: a leaf other than the candidate is a countermodel, a covered tree is stability, and the masked readings are the reduct's. |
| Why does a walk by several workers emit every accepted leaf once? | [Frontier](../Zetesis/Frontier.lean) proves that the pending regions, whoever holds each, and the leaves emitted are together a permutation of the tree's outputs after every step in any interleaving, so a walk that empties the frontier emits every accepted leaf once. |
| Can supported atoms retain their positions without enumerating every gate tuple? | [GatePositions](../Zetesis/GatePositions.lean) relates the mixed-radix digit fold to lexicographic block offsets and bounds valid tuple offsets. Its subsequence law preserves original carrier positions. Completed support, Rust canonical ordering, checked arithmetic and program identity remain separate obligations. |
| What must canonical occurrence maps preserve? | Read [ModelSelections](../Zetesis/ModelSelections.lean), then [CanonicalCatalog](../Zetesis/CanonicalCatalog.lean): decoded selections survive normalization, valid-prefix extension and transfers that preserve decoded atoms. Equal raw IDs need not denote equal atoms. |
| Can a shared base and an appended suffix use separate lookup indexes? | [ClosedCatalog](../Zetesis/ClosedCatalog.lean) proves sound combined lookup and complete absence. Global uniqueness additionally identifies the exact returned position. |
| How do canonical constants compose with partial bindings? | [CanonicalTemplates](../Zetesis/CanonicalTemplates.lean) connects ordered substitution to decoding and [BindingScopes](../Zetesis/BindingScopes.lean)'s checked reads. The available identity domain is a premise; Rust must validate scope and prefix. |
| When do vocabulary coordinates identify and order atoms? | [CarrierCoordinates](../Zetesis/CarrierCoordinates.lean) separates decoder uniqueness, transported order and finite sparse selections. No full-carrier ordinal is required, and independent atom-row scopes remain distinct even with a shared vocabulary. |
| When does completed possible support cover answer sets? | [The support argument](source-support.md) separates an empty-delta closure result from the source-to-reduct projection premise. Minimality then places every answer-set atom inside the carrier; optional objective activity still does not establish realizability. |
| When may possible-support rounds visit only affected producers? | [Affected producer scheduling](producer-scheduling.md) requires complete positive-input registration, zero-input bootstrap and previously published old heads. It preserves original producer identities and possible-head closure without discharging final constraints. |
| Can source contributions share an arena? | [SourceContributions](../Zetesis/SourceContributions.lean) gives finite-chain uniqueness, preservation under changes to other entries, and per-atom append-order laws. Rust link mutation, origin ordering and resource accounting remain separate obligations. |
| When can grounding remain incomplete while work proceeds? | [LiftedBridge](../Zetesis/LiftedBridge.lean): sound intermediate stages and explicit final coverage premises for stable acceptance. |
| What permits a cheaper exact membership check? | [TightPlans](../Zetesis/TightPlans.lean) and [CertifiedExecution](../Zetesis/CertifiedExecution.lean), with the [worked structured proof](certified-membership.md). |
| How do Boolean tables and producer reductions implement that certificate? | [TightEvaluation](../Zetesis/TightEvaluation.lean) proves the finite evaluator and support correspondence; [reading the argument](tight-evaluation.md) separates computed truth from unproved device transport. |
| What does complete batched enumeration require? | [BatchAccounting](../Zetesis/BatchAccounting.lean): proposal, pending work, exact classification and exhaustion under supplied coverage. |
| When does a retained collection represent the world view? | [WorldViews](../Zetesis/WorldViews.lean) composes original answer coverage, exact completed classification and complete capture. A sound prefix or complete optimal selection need not contain every original answer. |
| How do observations preserve that original family? | [Reading the observation laws](observations.md) separates complete bindings, enabled values, local quantifier order and original-model identity beside each display. |
| What does projected enumeration preserve? | [ProjectedAnswers](../Zetesis/ProjectedAnswers.lean) separates full representative membership, selected properties and complete key-image coverage. It does not identify representatives with the complete original family. |
| How can an observation recover a missing scalar? | [ObservationBindings](../Zetesis/ObservationBindings.lean) proves unique integer translation/reflection candidates and retains the complete bounded guard. Checked machine arithmetic remains a separate obligation. |
| Does publishing no answers establish UNSAT? | [Outcomes](../Zetesis/Outcomes.lean): completed semantic absence, sound delivery and a counterexample with no delivered records. |
| How can candidate feedback preserve the original problem? | [Feedback](../Zetesis/Feedback.lean), [ObjectiveBounds](../Zetesis/ObjectiveBounds.lean) and [SignedObjectiveBounds](../Zetesis/SignedObjectiveBounds.lean). |
| When may a newer bound replace an older one? | [ObjectiveBounds](../Zetesis/ObjectiveBounds.lean) preserves candidates and prior consequences under explicit logical strengthening, independently of permanent restrictions. |
| Can an ordered adjacency row share contiguous storage? | [AdjacencyRows](../Zetesis/AdjacencyRows.lean) recovers the exact row and its ordered folds; the Rust offset builder remains a refinement obligation. |
| What must source transformations preserve? | [GroundGuards](../Zetesis/GroundGuards.lean), [UniversalConditionals](../Zetesis/UniversalConditionals.lean) and [FinitePools](../Zetesis/FinitePools.lean): original truth and frozen contexts under explicit coverage. |
| How do finite occurrences compose? | [Finite occurrence families](finite-occurrences.md) distinguishes whole-context products from unions of element-local instances. Shared coverage laws do not erase each consumer's quantifier order. |
| What does a conditional disjunct mean? | [Conditional heads](conditional-heads.md) derives original and frozen truth while keeping condition eligibility separate from positive head support. |
| What are the traps in otherwise plausible optimizations? | [Examples](../Zetesis/Examples.lean) and the counterexamples in [Feedback](../Zetesis/Feedback.lean): choices and candidate-relative reasoning matter. |
| How do finite source values and local alternatives preserve meaning? | [StructuralBindings](../Zetesis/StructuralBindings.lean) and [FiniteValues](../Zetesis/FiniteValues.lean) state matching/construction laws; [ConsequentAlternatives](../Zetesis/ConsequentAlternatives.lean) separates universal condition rows from their existential signed consequents. Compiler coverage and local scope remain explicit unproved bridges. |
| How may an aggregate result feed a universal conditional? | [ConditionalConsumers](../Zetesis/ConditionalConsumers.lean) retains the original equality, activation and complete local implications under every frozen interpretation. A proposed value is not an established aggregate result. |
| How does a weighted head separate permission from its bound? | [HeadMeasures](../Zetesis/HeadMeasures.lean) keeps the eligibility reduct in permission rules and numeric agreement in a candidate constraint. Numeric compilation and complete head carriers remain separate obligations. |
| What does a signed choice or aggregate operand contribute? | [SignedHeadElements](../Zetesis/SignedHeadElements.lean) preserves sign-aware atom keys, Boolean occurrences and complete tuple keys. It proves frozen signed activity, positive-only permission coalescing and contextual answer-set preservation through the existing unsigned laws. |
| Does a zero or missing contribution remove head permission? | [Neutral head contributions](head-contributions.md) separates measures from independent permissions, explains the original/frozen laws, and gives the declared missing-extremum extension with its conservation domain and explicit reference differences. |
| How do nonnumeric extrema retain that separation? | [OrderedHeadActivity](../Zetesis/OrderedHeadActivity.lean) uses an arbitrary logical value carrier and explicit ordered-selection laws. It retains signed activity, independent permission and the candidate/frozen aggregate guard. |
| When may a logical numeric-aggregate bound become a constant? | [OrderedBounds](../Zetesis/OrderedBounds.lean) requires comparison agreement across every tuple mask, then proves original/frozen replacement and contextual preservation. Classical truth in one candidate is insufficient for a body replacement. |
| How are resolved objective priorities kept correlated with weights? | [ObjectivePriorities](../Zetesis/ObjectivePriorities.lean) keeps both fields on one eligible row, distinguishes priority presence from model activation, and proves per-priority evaluation preserves cost vectors and optimum ties. The completed priority layout and source carrier remain explicit premises. |
| Can a retained zero-cost priority change optimality? | `ObjectivePriorities.zero_slot_comparison` and `zero_slot_optima` preserve ranking and all optimal answer sets when an always-zero slot is inserted at a fixed position. Whether a source row is always inactive remains a separate coverage and evaluation obligation. |
| How does an objective node table compute original truth? | [Reading the condition-table correspondence](objective-condition-table.md) follows the prefix invariant, backward-reference admission and last-node result. The mathematical representation law does not verify Rust indexing or source eligibility. |
| When do required tuples determine an aggregate's value? | [AggregateInvariants](../Zetesis/AggregateInvariants.lean) separates required, possible and actual tuple carriers. Optional zero contributions preserve sums; optional dominated values preserve extrema with explicit empty endpoints. The priority corollary retains complete eligibility and the same-row numeric-weight witness. |
| What changes when a source measure can take several values? | [SourceMeasures](../Zetesis/SourceMeasures.lean) enumerates every selection between required and possible keys, proves actual-value coverage and singleton compatibility, and filters complete objective rows without confusing source presence with answer activity. |
| How do equality columns preserve complete tuple matches? | [Typed relation columns](column-relations.md) explains dictionary round trips, correlated reconstruction, ordered selection and exact snapshot applicability. |
| Can table selection preserve positive source bindings? | [The binding-family argument](table-bindings.md) composes flat matcher necessity, exact row selection and finite joins while retaining each source/row occurrence and resulting binding. Completed support and fallible execution remain separate obligations. |
| May a join discard a locally matching row that cannot finish? | [Necessary domains and complete bindings](domain-bindings.md) requires conservative argument coverage and preserves the ordered complete continuations, including repeated results. Analyzer soundness and concrete scheduling remain separate obligations. |
| How do source producers propagate argument bounds? | [DomainProducers](../Zetesis/DomainProducers.lean) intersects mandatory input domains within a producer and unions alternative producers. Its transfer is monotone, and any closed upper bound covers finite recursive derivations. Source extraction and Rust fixed-point execution remain separate obligations. |
| When can domain filtering restrict candidates safely? | [Compatible answer-set completions](domain-contraction.md) connects singleton-distinctness and affine filtering to an unchanged theory under explicit recognition and activation premises. |
| Why does a finite comparison envelope preserve bindings? | [IntegerEnvelopes](../Zetesis/IntegerEnvelopes.lean) composes directed path and affine bounds, enumerates closed integer intervals and retains the original guard. Correlation is checked by that guard rather than asserted by the envelope. |
| How are negative anonymous consequents projected? | [ProjectedConditionals](../Zetesis/ProjectedConditionals.lean) places default negation after each complete existential witness projection and before the disjunction of source alternatives. It proves the finite-carrier bridge and distinguishes the different empty carriers. |
| How can an application use the semantic bridge? | [The checked choices consumer](../Zetesis/Examples/Choices.lean) computes the guided tour candidate's least reduct closure and applies `NormalFerraris.ferraris_answer_set_iff_closure`. It establishes one answer set, not complete enumeration. |

The intended layering is: logical definitions → reusable semantic laws → justified
algorithmic specializations → representation and execution obligations. Existing
proofs do not yet close every arrow to Rust or GPU execution. A finite formula
law, for example, cannot establish that a source grounder supplied every required
instance merely because the law accepts a finite list.

The [proof convention](../STYLE.md) explains how substantial arguments should be
read and written. The [membership guide](certified-membership.md) works through
one complete proof using named claims and explicit assumptions. The library
contains both compact and expanded proofs; each statement and its hypotheses
remain the contract to inspect.

The execution extensions connect these paths to [signed singleton heads](../Zetesis/SingletonHeads.lean),
[constructor shape and extraction](../Zetesis/ConstructorPatterns.lean),
[checked scalar plans](../Zetesis/ScalarArithmetic.lean) and
[finite gate projection](../Zetesis/GateProjection.lean). Each module states its
unproved implementation correspondence explicitly.

[Consuming positive arguments](../Zetesis/PositiveArguments.lean) extend the
finite-value path: exact checks filter supplied support rows without creating
bindings or atoms. Independent inputs and complete row coverage are premises;
Rust readiness, source/capture correspondence and resource completion remain
separate obligations.


Aggregate-producing source bounds and tuple-count heads connect
[AggregateBounds](../Zetesis/AggregateBounds.lean) with
[CountEligibility](../Zetesis/CountEligibility.lean): a proposed value retains its
original equality, and complete tuple/head correspondence retains every eligibility
formula. These are source-to-formula obligations, not permission to substitute
possible-support membership for logical truth.

[CountHeadActivity](../Zetesis/CountHeadActivity.lean) removes the tuple/head
bijection premise from the semantic row-table description. Head permission
coalesces by atom; selected activity coalesces by the complete tuple. The older
bijection laws remain useful for optimizations that require their stronger
premise. [ObjectiveTransport](../Zetesis/ObjectiveTransport.lean) separately
transports completed presence carriers and model-relative objective activation.
Its premises do not establish a clingo-compatible priority layout from possible
support. Neither addition proves the concrete source compiler.

[WeightedHeadActivity](../Zetesis/WeightedHeadActivity.lean) reuses the independent
tuple carrier for signed sums. Its canonical formula agrees with direct numeric
selection, and its bound is evaluated only in the candidate. Identical complete
row sets preserve the whole group in context for the same key carrier. Concrete
signed lowering, source coverage and finite-width arithmetic remain separate
implementation obligations.

[EvaluationPrefix](../Zetesis/EvaluationPrefix.lean) specifies the live-prefix
invariant beneath reusable expression storage. Its `root_preservation` law
permits returning the final operation directly, preserving the reference plan's
last value or first error. [LazyRounds](../Zetesis/LazyRounds.lean)
specifies fresh source coverage and separate world truth beneath shared device
batches. Both are finite execution foundations. Their Rust memory, cursor and
shader correspondences remain explicit work; neither supplies a device certificate.

[ChoiceConsumers](../Zetesis/ChoiceConsumers.lean) and
[NegativeEligibility](../Zetesis/NegativeEligibility.lean) specialize those source
obligations to completed scalar filters and frozen negative eligibility.
[OptionalIndex](../Zetesis/OptionalIndex.lean) provides a representation law for
optional finite identities. [WorldMasks](../Zetesis/WorldMasks.lean) narrows source
coverage to bindings enabled in some immutable world while retaining per-world
truth checks. These are distinct proof layers: semantic coverage does not itself
verify a packed representation or concrete source traversal.

[OuterNegativeConsumers](../Zetesis/OuterNegativeConsumers.lean) keeps negative
tests over completed values tied to the frozen candidate and supplied complete
projection families. [OuterRanges](../Zetesis/OuterRanges.lean) preserves the
association between a completed outer value, its finite integer range and each
original/frozen clause. Both make source binding and coverage assumptions visible
instead of treating a proposed value as a logical conclusion.

[ProjectedConditionals](../Zetesis/ProjectedConditionals.lean) composes those
negative projection laws with [ConsequentAlternatives](../Zetesis/ConsequentAlternatives.lean).
Every condition remains in its implication. When all consequent alternatives
have candidate-only frozen truth, the complete conditional does too; that is an
explicit hypothesis, not a law about arbitrary positive consequents. One empty
anonymous witness family, no source alternatives and no condition rows have
different meanings. Source enumeration must establish which complete carrier was
supplied before the logical laws apply.

[OrderedHeadActivity](../Zetesis/OrderedHeadActivity.lean) separates complete tuple
keys from their first logical values. Canonical extrema evaluate the selected
values in the candidate and again over frozen activity; the surrounding head
bound is candidate-only. The efficient predicate-witness law assumes the relevant
ordered-selection equation, so using it with Rust's term comparator requires an
independent correspondence argument. These laws operate on complete values. [HeadContributions](../Zetesis/HeadContributions.lean)
adds the declared neutral-missing-value projection before that reduction, with
independent permissions and a [documented conservation domain](head-contributions.md).

[JoinFrames](../Zetesis/JoinFrames.lean) follows the world-membership path into a
finite execution schedule: reset the root, overwrite each child, then use it as
the next parent. Its resulting membership is independent of old frame contents.
This permits a mathematical storage-reuse argument while leaving packed Rust
storage, exact allocation accounting and physical GPU correspondence explicit.

[AggregateDependencies](../Zetesis/AggregateDependencies.lean) continues the
outer-value path through complete families indexed by predecessor rows. Its
coverage laws compose without treating proposed values as realized aggregate
results; original activation and every equality remain in frozen clauses.
Source scheduling and concrete carrier completeness remain unproved bridges.

[BinaryWatch](../Zetesis/BinaryWatch.lean) is a small algorithmic specialization:
two distinct positions exhaust a binary clause, leaving no replacement to find.
The mathematical result requires valid distinct watches; the Rust registry,
propagation order, charged work and reduct encoding remain separate obligations.

[TernaryWatch](../Zetesis/TernaryWatch.lean) identifies the sole remaining position
of a three-position clause. `replacement_exact` proves that any covering scan
and one availability test there return the same replacement. Valid distinct
watches and a fixed availability predicate are explicit premises; concrete
registry, accounting and cancellation behavior remain implementation obligations.
