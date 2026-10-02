import FixedReduct

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Truth at the asserted formulas

The asserted theory consists of the formulas named by the stored roots, in their
stored order. Repeated roots remain repeated assertions. Unasserted nodes are
shared subformulas, not additional requirements on an interpretation.

These laws compose successful generated evaluation with the existing ASP root
semantics. They do not by themselves identify an actual root-scan result. The
fixed-observation model and representation assumptions are those of FixedReduct.
-/
namespace RootSemantics

/-- The mathematical formulas asserted by the actual stored root list. -/
def assertions (program : theory.Theory) : Zetesis.Ferraris.Theory Nat :=
  Zetesis.DagSharing.assertions
    (program.value.nodes.val.map EvaluationSemantics.node)
    (program.value.roots.val.map (fun root => root.val))

/-- Truth at every asserted root of a successful original evaluation is exactly
satisfaction of the represented theory. The generated evaluation supplies the
truth values; the reusable ASP theorem supplies the conjunction over roots. -/
theorem original_roots (program : theory.Theory) (candidate : theory.Interpretation)
    (oldOutput : alloc.vec.Vec Bool) (work : oracle.Work)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (output : alloc.vec.Vec Bool) (returned : oracle.Work)
    (completed : oracle.evaluate program candidate none oldOutput work =
      ok (core.result.Result.Ok (), output, returned)) :
    Zetesis.TightEvaluation.rootsTrue output.val
      (program.value.roots.val.map (fun root => root.val)) = true ↔
      Zetesis.Ferraris.Models
        (Zetesis.TightEvaluation.interpretation (Membership.denotes candidate))
        (assertions program) := by
  rw [FixedEvaluationLoop.completed_values program candidate none oldOutput work stored
    ordered (by intro mask member; cases member) output returned completed,
    EvaluationSemantics.original_values]
  exact Zetesis.TightEvaluation.roots_true_iff _ _ _

/-- Truth at every asserted root of the computed frozen evaluation is exactly
satisfaction of the represented Ferraris reduct theory. The outer interpretation
is fixed by the first successful call; the second may test any interpretation.
Neither this conjunction nor a successful root scan proves minimality. -/
theorem reduct_roots (program : theory.Theory) (outer tested : theory.Interpretation)
    (oldOriginal oldTested : alloc.vec.Vec Bool) (outerWork testedWork : oracle.Work)
    (outerStored : Membership.Represented outer) (testedStored : Membership.Represented tested)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (original result : alloc.vec.Vec Bool) (outerReturned testedReturned : oracle.Work)
    (originalComplete : oracle.evaluate program outer none oldOriginal outerWork =
      ok (core.result.Result.Ok (), original, outerReturned))
    (testedComplete : oracle.evaluate program tested (some original.slice) oldTested testedWork =
      ok (core.result.Result.Ok (), result, testedReturned)) :
    Zetesis.TightEvaluation.rootsTrue result.val
      (program.value.roots.val.map (fun root => root.val)) = true ↔
      Zetesis.Ferraris.Models
        (Zetesis.TightEvaluation.interpretation (Membership.denotes tested))
        (Zetesis.Ferraris.ReductTheory
          (Zetesis.TightEvaluation.interpretation (Membership.denotes outer))
          (assertions program)) := by
  rw [FixedReductEvaluation.completed_values program outer tested oldOriginal oldTested
    outerWork testedWork outerStored testedStored ordered original result outerReturned
    testedReturned originalComplete testedComplete]
  exact Zetesis.ReductEvaluation.roots_true_iff _ _ _ _

end RootSemantics
