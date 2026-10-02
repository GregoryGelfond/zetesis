import Setup
import Trace
import Semantics

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Original and frozen evaluation through actual generated steps

Both traces start at the actual evaluator's empty-prefix setup and address the
same stored formula table. The completed original trace produces the mask of
the frozen trace. The proof derives that mask's truth and coverage, then applies
the shared Ferraris reduct laws; it does not assume a correct mask or a correct
external evaluator. The tested interpretation need not be a subset of the outer
interpretation, as satisfaction of a reduct is defined for any interpretation.

The trace relation supplies changing observations between generated calls. Its
connection to the generated whole loop, Rust pointer ownership, allocation and
concurrent memory remains outside these theorems. Both passes share numeric
atom identities in this model; these theorems do not derive owner-token
agreement or relate those tokens to Rust allocations.
-/
namespace ReductTrace

/-- The generated entry state, with its actual source cursor and empty output.
Work limits and counters remain supplied by the caller. -/
def initialState (program : theory.Theory) (limits : oracle.Limits)
    (statistics : oracle.Statistics) : EvaluationTrace.State :=
  ⟨EvaluatorSetup.initialCursor program, alloc.vec.Vec.new Bool, limits, statistics⟩

/-- Generated setup establishes the entire initial truth invariant. In
particular, the empty prefix is proved correct without a caller agreement premise. -/
theorem initial_invariant (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (limits : oracle.Limits) (statistics : oracle.Statistics) :
    EvaluationTrace.Invariant candidate frozen program.value.nodes.val
      (initialState program limits statistics) := by
  refine ⟨?_, ?_, ?_, ?_⟩
  · exact (EvaluatorSetup.initial_alignment program).1
  · exact Nat.zero_le _
  · rfl
  · simp [initialState, EvaluatorSetup.initialCursor,
      EvaluationSpecification.prefixValues, EvaluationSpecification.values]

/-- A completed original trace returns the full computed original-truth table.
Only stored-word coverage and ordered child indices are input invariants. The
trace itself records actual generated calls, including their control inputs. -/
theorem completed_original_values (program : theory.Theory) (outer : theory.Interpretation)
    (limits : oracle.Limits) (statistics : oracle.Statistics)
    (stored : Membership.Represented outer)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    {count : Nat} {outcome : EvaluationTrace.Outcome}
    (trace : EvaluationTrace.Trace outer none (initialState program limits statistics) count outcome)
    (completed : outcome.result = .Ok ()) :
    outcome.output.val = EvaluationSpecification.values outer none program.value.nodes.val := by
  exact EvaluationTrace.completed_values outer none program.value.nodes.val stored ordered
    (by intro mask member; cases member) trace
    (initial_invariant program outer none limits statistics) completed

/-- Completing an original trace and then a trace masked by its actual output
computes the existing mathematical reduct evaluator. The original trace supplies
both mask truth and mask length; neither is assumed of an external table.
Both traces begin with an empty output, so no prior-prefix agreement is assumed.

Proof: establish initial invariants, derive the completed original values and
hence mask coverage, derive the completed frozen values, and use the structural
correspondence to the shared reduct fold. -/
theorem completed_reduct_values (program : theory.Theory) (outer tested : theory.Interpretation)
    (outerLimits testedLimits : oracle.Limits) (outerStats testedStats : oracle.Statistics)
    (outerStored : Membership.Represented outer) (testedStored : Membership.Represented tested)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    {outerCount testedCount : Nat} {original result : EvaluationTrace.Outcome}
    (originalTrace : EvaluationTrace.Trace outer none
      (initialState program outerLimits outerStats) outerCount original)
    (originalComplete : original.result = .Ok ())
    (testedTrace : EvaluationTrace.Trace tested (some original.output.slice)
      (initialState program testedLimits testedStats) testedCount result)
    (testedComplete : result.result = .Ok ()) :
    result.output.val = Zetesis.ReductEvaluation.values (Membership.denotes outer)
      (Membership.denotes tested) (program.value.nodes.val.map EvaluationSemantics.node) := by
  have originalValues : original.output.val =
      EvaluationSpecification.values outer none program.value.nodes.val :=
    completed_original_values program outer outerLimits outerStats outerStored ordered
      originalTrace originalComplete
  have maskCovered : ∀ mask ∈ some original.output.slice,
      program.value.nodes.val.length ≤ mask.val.length := by
    intro mask member
    cases member
    change program.value.nodes.val.length ≤ original.output.val.length
    rw [originalValues, EvaluationSpecification.values_length]
  have testedValues : result.output.val =
      EvaluationSpecification.values tested (some original.output.slice) program.value.nodes.val :=
    EvaluationTrace.completed_values tested (some original.output.slice) program.value.nodes.val
      testedStored ordered maskCovered testedTrace
      (initial_invariant program tested (some original.output.slice) testedLimits testedStats)
      testedComplete
  exact testedValues.trans (EvaluationSemantics.frozen_values outer tested
    original.output.slice program.value.nodes.val originalValues)

/-- Every returned frozen truth is the truth of its formula's explicit Ferraris
reduct. This is a consequence of the two completed traces and the already-proved
general reduct theorem, with the original trace fixing the outer interpretation. -/
theorem completed_reduct_truth (program : theory.Theory) (outer tested : theory.Interpretation)
    (outerLimits testedLimits : oracle.Limits) (outerStats testedStats : oracle.Statistics)
    (outerStored : Membership.Represented outer) (testedStored : Membership.Represented tested)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    {outerCount testedCount : Nat} {original result : EvaluationTrace.Outcome}
    (originalTrace : EvaluationTrace.Trace outer none
      (initialState program outerLimits outerStats) outerCount original)
    (originalComplete : original.result = .Ok ())
    (testedTrace : EvaluationTrace.Trace tested (some original.output.slice)
      (initialState program testedLimits testedStats) testedCount result)
    (testedComplete : result.result = .Ok ()) :
    result.output.val =
      (Zetesis.DagSharing.meanings (program.value.nodes.val.map EvaluationSemantics.node)).map
        (fun formula => Zetesis.TightEvaluation.formulaValue (Membership.denotes tested)
          (Zetesis.Ferraris.Reduct
            (Zetesis.TightEvaluation.interpretation (Membership.denotes outer)) formula)) := by
  rw [completed_reduct_values program outer tested outerLimits testedLimits outerStats testedStats
    outerStored testedStored ordered originalTrace originalComplete testedTrace testedComplete]
  exact Zetesis.ReductEvaluation.values_correspond _ _ _

/-- A returned truth lookup is true exactly when the tested interpretation
satisfies the corresponding explicit reduct. Unavailable formula positions have
falsum as their total meaning; this lookup law does not authorize an unchecked
Rust index. It proves satisfaction, not minimality or answer-set membership. -/
theorem completed_reduct_satisfaction (program : theory.Theory)
    (outer tested : theory.Interpretation)
    (outerLimits testedLimits : oracle.Limits) (outerStats testedStats : oracle.Statistics)
    (outerStored : Membership.Represented outer) (testedStored : Membership.Represented tested)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    {outerCount testedCount : Nat} {original result : EvaluationTrace.Outcome}
    (originalTrace : EvaluationTrace.Trace outer none
      (initialState program outerLimits outerStats) outerCount original)
    (originalComplete : original.result = .Ok ())
    (testedTrace : EvaluationTrace.Trace tested (some original.output.slice)
      (initialState program testedLimits testedStats) testedCount result)
    (testedComplete : result.result = .Ok ()) (index : Nat) :
    result.output.val.getD index false = true ↔
      Zetesis.Ferraris.Satisfies
        (Zetesis.TightEvaluation.interpretation (Membership.denotes tested))
        (Zetesis.Ferraris.Reduct
          (Zetesis.TightEvaluation.interpretation (Membership.denotes outer))
          ((Zetesis.DagSharing.meanings
            (program.value.nodes.val.map EvaluationSemantics.node)).getD index .bot)) := by
  rw [completed_reduct_values program outer tested outerLimits testedLimits outerStats testedStats
    outerStored testedStored ordered originalTrace originalComplete testedTrace testedComplete]
  rw [Zetesis.ReductEvaluation.value_at]
  exact Zetesis.TightEvaluation.formula_value_true _ _

end ReductTrace
