# Reading the zetesis mathematical library

Start with the logical question, then follow the definitions and laws that answer
it. All linked Lean modules are in this package and imported by its checked
umbrella. These paths are a curated introduction; the [full module map](../README.md)
and [complete theorem index](../theorems.json) provide the inventory. Definitions
and theorem hypotheses are authoritative; prose is a reading aid.

| Question | Reading path |
| --- | --- |
| What makes an interpretation a stable model? | [Ferraris](../Zetesis/Ferraris.lean): formula truth, the frozen reduct, minimality and their connection. |
| Why does least closure suffice for the normal-rule specialization? | [Semantics](../Zetesis/Semantics.lean), [Iteration](../Zetesis/Iteration.lean), then the normal/lifted bridge modules listed in the full map. |
| When can grounding remain incomplete while work proceeds? | [LiftedBridge](../Zetesis/LiftedBridge.lean): sound intermediate stages and explicit final coverage premises for stable acceptance. |
| What permits a cheaper exact membership check? | [TightPlans](../Zetesis/TightPlans.lean) and [CertifiedExecution](../Zetesis/CertifiedExecution.lean), with the [worked structured proof](certified-membership.md). |
| What does complete batched enumeration require? | [BatchAccounting](../Zetesis/BatchAccounting.lean): proposal, pending work, exact classification and exhaustion under supplied coverage. |
| Does publishing no answers establish UNSAT? | [Outcomes](../Zetesis/Outcomes.lean): completed semantic absence, sound delivery and a counterexample with no delivered records. |
| How can candidate feedback preserve the original problem? | [Feedback](../Zetesis/Feedback.lean), [ObjectiveBounds](../Zetesis/ObjectiveBounds.lean) and [SignedObjectiveBounds](../Zetesis/SignedObjectiveBounds.lean). |
| What must source transformations preserve? | [GroundGuards](../Zetesis/GroundGuards.lean), [UniversalConditionals](../Zetesis/UniversalConditionals.lean) and [FinitePools](../Zetesis/FinitePools.lean): original truth and frozen contexts under explicit coverage. |
| What are the traps in otherwise plausible optimizations? | [Examples](../Zetesis/Examples.lean) and the counterexamples in [Feedback](../Zetesis/Feedback.lean): choices and candidate-relative reasoning matter. |
| How do finite source values and local alternatives preserve meaning? | [StructuralBindings](../Zetesis/StructuralBindings.lean) and [FiniteValues](../Zetesis/FiniteValues.lean) state matching/construction laws; [ConsequentAlternatives](../Zetesis/ConsequentAlternatives.lean) separates universal condition rows from their existential signed consequents. Compiler coverage and local scope remain explicit unproved bridges. |

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

[EvaluationPrefix](../Zetesis/EvaluationPrefix.lean) specifies the live-prefix
invariant beneath reusable expression storage. [LazyRounds](../Zetesis/LazyRounds.lean)
specifies fresh source coverage and separate world truth beneath shared device
batches. Both are finite execution foundations. Their Rust memory, cursor and
shader correspondences remain explicit work; neither supplies a device certificate.
