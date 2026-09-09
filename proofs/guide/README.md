# Reading the zetesis mathematical library

Start with the logical question, then follow the definitions and laws that answer
it. All linked Lean modules are in this package and imported by its checked
umbrella. These paths are a curated introduction; the [full module map](../README.md)
and [complete theorem index](../theorems.json) provide the inventory. Definitions
and theorem hypotheses are authoritative; prose is a reading aid.

| Question | Reading path |
| --- | --- |
| Can an aggregate be evaluated directly through the reduct? | [AggregateReduct](../Zetesis/AggregateReduct.lean) formalizes Ferraris Proposition 7: retain the original guard and evaluate the same guard over frozen eligibility. Whole-tuple grouping and concrete arithmetic remain separate obligations. |
| When is parallel signed addition safe from intermediate overflow? | [AggregateRanges](../Zetesis/AggregateRanges.lean) bounds every mathematical intermediate sum by separate positive and negative carrier totals. The actual execution must preserve contribution occurrences and implement the admitted arithmetic. |
| When do total and group capacities imply stronger local bounds? | [PartitionCapacities](../Zetesis/PartitionCapacities.lean) separates the counting argument from the caller's coverage and theory-entailment premises. |
| What makes an interpretation a stable model? | [Ferraris](../Zetesis/Ferraris.lean): formula truth, the frozen reduct, minimality and their connection. |
| Why does least closure suffice for the normal-rule specialization? | The [normalized/Ferraris bridge](normal-ferraris.md) proves that both independent answer-set definitions coincide, then connects Ferraris membership to [Semantics](../Zetesis/Semantics.lean)'s least closure. [Iteration](../Zetesis/Iteration.lean) addresses bounded closure computation. |
| When can grounding remain incomplete while work proceeds? | [LiftedBridge](../Zetesis/LiftedBridge.lean): sound intermediate stages and explicit final coverage premises for stable acceptance. |
| What permits a cheaper exact membership check? | [TightPlans](../Zetesis/TightPlans.lean) and [CertifiedExecution](../Zetesis/CertifiedExecution.lean), with the [worked structured proof](certified-membership.md). |
| How do Boolean tables and producer reductions implement that certificate? | [TightEvaluation](../Zetesis/TightEvaluation.lean) proves the finite evaluator and support correspondence; [reading the argument](tight-evaluation.md) separates computed truth from unproved device transport. |
| What does complete batched enumeration require? | [BatchAccounting](../Zetesis/BatchAccounting.lean): proposal, pending work, exact classification and exhaustion under supplied coverage. |
| Does publishing no answers establish UNSAT? | [Outcomes](../Zetesis/Outcomes.lean): completed semantic absence, sound delivery and a counterexample with no delivered records. |
| How can candidate feedback preserve the original problem? | [Feedback](../Zetesis/Feedback.lean), [ObjectiveBounds](../Zetesis/ObjectiveBounds.lean) and [SignedObjectiveBounds](../Zetesis/SignedObjectiveBounds.lean). |
| What must source transformations preserve? | [GroundGuards](../Zetesis/GroundGuards.lean), [UniversalConditionals](../Zetesis/UniversalConditionals.lean) and [FinitePools](../Zetesis/FinitePools.lean): original truth and frozen contexts under explicit coverage. |
| What are the traps in otherwise plausible optimizations? | [Examples](../Zetesis/Examples.lean) and the counterexamples in [Feedback](../Zetesis/Feedback.lean): choices and candidate-relative reasoning matter. |
| How do finite source values and local alternatives preserve meaning? | [StructuralBindings](../Zetesis/StructuralBindings.lean) and [FiniteValues](../Zetesis/FiniteValues.lean) state matching/construction laws; [ConsequentAlternatives](../Zetesis/ConsequentAlternatives.lean) separates universal condition rows from their existential signed consequents. Compiler coverage and local scope remain explicit unproved bridges. |
| How may an aggregate result feed a universal conditional? | [ConditionalConsumers](../Zetesis/ConditionalConsumers.lean) retains the original equality, activation and complete local implications under every frozen interpretation. A proposed value is not an established aggregate result. |
| How does a weighted head separate permission from its bound? | [HeadMeasures](../Zetesis/HeadMeasures.lean) keeps the eligibility reduct in permission rules and numeric agreement in a candidate constraint. Numeric compilation and complete head carriers remain separate obligations. |

The intended layering is: logical definitions → reusable semantic laws → justified
algorithmic specializations → representation and execution obligations. Existing
proofs do not yet close every arrow to Rust or GPU execution. A finite formula
law, for example, cannot establish that a source grounder supplied every required
instance merely because the law accepts a finite list.

The [proof convention](../STYLE.md) explains how substantial arguments should be
read and written. Small proofs may stay compact. Only the named pilot has been
explicitly refactored under this convention so far; the library-wide migration is
future work.

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

[EvaluationPrefix](../Zetesis/EvaluationPrefix.lean) specifies the live-prefix
invariant beneath reusable expression storage. [LazyRounds](../Zetesis/LazyRounds.lean)
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
