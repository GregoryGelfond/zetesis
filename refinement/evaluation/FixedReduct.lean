import FixedLoop

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Reduct satisfaction from two generated evaluator calls

The first successful call computes original truth. Its actual output supplies
the second call's frozen mask. The generated-loop theorem establishes both
value sequences; the shared ASP library identifies the second with Ferraris
reduct truth. No trace, correct mask or evaluator-correctness premise is supplied.

These theorems use the fixed-observation external model of `FixedEvaluationLoop`.
They do not verify concurrently changing atomics, Rust owner identity, allocation,
the `FrozenReduct` wrapper, root checking or proper-subset enumeration. Both
interpretations use the same numeric atom identities; satisfaction needs no
subset relation between them.
-/
namespace FixedReductEvaluation

/-- Two successful generated evaluations compute the mathematical frozen reduct
when the second uses the first call's actual output as its mask. Old output
contents need no invariant: generated setup clears them in each call.

Proof: the loop theorem establishes original values and therefore mask coverage.
Apply it again to the masked call, then use the shared reduct-fold correspondence.
The premises record actual successful results, not their semantic correctness.
-/
theorem completed_values (program : theory.Theory) (outer tested : theory.Interpretation)
    (oldOriginal oldTested : alloc.vec.Vec Bool) (outerWork testedWork : oracle.Work)
    (outerStored : Membership.Represented outer) (testedStored : Membership.Represented tested)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (original result : alloc.vec.Vec Bool) (outerReturned testedReturned : oracle.Work)
    (originalComplete : oracle.evaluate program outer none oldOriginal outerWork =
      ok (core.result.Result.Ok (), original, outerReturned))
    (testedComplete : oracle.evaluate program tested (some original.slice) oldTested testedWork =
      ok (core.result.Result.Ok (), result, testedReturned)) :
    result.val = Zetesis.ReductEvaluation.values (Membership.denotes outer)
      (Membership.denotes tested) (program.value.nodes.val.map EvaluationSemantics.node) := by
  have originalValues : original.val =
      EvaluationSpecification.values outer none program.value.nodes.val :=
    FixedEvaluationLoop.completed_values program outer none oldOriginal outerWork outerStored
      ordered (by intro mask member; cases member) original outerReturned originalComplete
  have maskCovered : ∀ mask ∈ some original.slice,
      program.value.nodes.val.length ≤ mask.val.length := by
    intro mask member
    cases member
    change program.value.nodes.val.length ≤ original.val.length
    rw [originalValues, EvaluationSpecification.values_length]
  have testedValues : result.val =
      EvaluationSpecification.values tested (some original.slice) program.value.nodes.val :=
    FixedEvaluationLoop.completed_values program tested (some original.slice) oldTested
      testedWork testedStored ordered maskCovered result testedReturned testedComplete
  exact testedValues.trans (EvaluationSemantics.frozen_values outer tested original.slice
    program.value.nodes.val originalValues)

/-- Each returned frozen truth is true exactly when the tested interpretation
satisfies that formula's Ferraris reduct, with the outer interpretation fixed by
the first actual call. This proves satisfaction, not answer-set membership.

The lookup is a total mathematical observation. An unavailable position denotes
falsum; this convention does not authorize an out-of-bounds implementation read.
-/
theorem completed_satisfaction (program : theory.Theory) (outer tested : theory.Interpretation)
    (oldOriginal oldTested : alloc.vec.Vec Bool) (outerWork testedWork : oracle.Work)
    (outerStored : Membership.Represented outer) (testedStored : Membership.Represented tested)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (original result : alloc.vec.Vec Bool) (outerReturned testedReturned : oracle.Work)
    (originalComplete : oracle.evaluate program outer none oldOriginal outerWork =
      ok (core.result.Result.Ok (), original, outerReturned))
    (testedComplete : oracle.evaluate program tested (some original.slice) oldTested testedWork =
      ok (core.result.Result.Ok (), result, testedReturned)) (index : Nat) :
    result.val.getD index false = true ↔
      Zetesis.Ferraris.Satisfies
        (Zetesis.TightEvaluation.interpretation (Membership.denotes tested))
        (Zetesis.Ferraris.Reduct
          (Zetesis.TightEvaluation.interpretation (Membership.denotes outer))
          ((Zetesis.DagSharing.meanings
            (program.value.nodes.val.map EvaluationSemantics.node)).getD index .bot)) := by
  rw [completed_values program outer tested oldOriginal oldTested outerWork testedWork
    outerStored testedStored ordered original result outerReturned testedReturned
    originalComplete testedComplete]
  rw [Zetesis.ReductEvaluation.value_at]
  exact Zetesis.TightEvaluation.formula_value_true _ _

end FixedReductEvaluation
