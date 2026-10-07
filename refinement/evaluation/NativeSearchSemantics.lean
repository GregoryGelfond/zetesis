import NativeSearchRepresentation
import NativeSubsetQueryTotal

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract
open Zetesis Zetesis.Refinement

/-!
# Meaning of a search over an actually frozen reduct

The frozen truth vector comes from the actual original evaluator. This record
retains that call and its preconditions, so later search theorems do not assume
an oracle's correctness or an arbitrary mask's semantic meaning.
-/
namespace NativeCountermodelSemantics

/-- Evidence that the original evaluation completed. This record retains no
    successful owner check or source-admission evidence. -/
structure FrozenEvaluation where
  program : theory.Theory
  candidate : theory.Interpretation
  old : alloc.vec.Vec Bool
  before : oracle.Work
  values : alloc.vec.Vec Bool
  after : oracle.Work
  stored : NativeMembership.Represented candidate
  valid : NativeStructure.WellFormed program.value.atoms.val (NativeExecution.view program)
  rootsBounded : ∀ root ∈ program.value.roots.val, root.val < (NativeTable.rows (NativeExecution.view program)).length
  completed : oracle.evaluate program candidate none old before =
    ok (core.result.Result.Ok (), values, after)

/-- The semantic reduct of the original roots with respect to the candidate. -/
noncomputable def FrozenEvaluation.reduct (frozen : FrozenEvaluation) : Ferraris.Theory Nat :=
  Ferraris.ReductTheory (TightEvaluation.interpretation (NativeMembership.denotes frozen.candidate))
    (NativeRootSemantics.assertions frozen.program)

/-- Original completion supplies mask coverage from the actual evaluator's
    output-length law. It is not a separate assumption about frozen storage. -/
theorem frozen_covered (frozen : FrozenEvaluation) :
    (NativeTable.rows (NativeExecution.view frozen.program)).length ≤ frozen.values.slice.val.length := by
  have exactValues : frozen.values.val = NativeSpecification.values frozen.candidate none
      (NativeTable.rows (NativeExecution.view frozen.program)) := by
    exact NativeExecution.completed_values frozen.program frozen.candidate none frozen.old frozen.before
      frozen.valid frozen.stored (by simp) frozen.values frozen.after frozen.completed
  change (NativeTable.rows (NativeExecution.view frozen.program)).length ≤ frozen.values.val.length
  rw [exactValues, NativeSpecification.values_length]

/-- Each actual completed query is the satisfaction test for the frozen reduct.
    The tested interpretation need not satisfy the original program. -/
theorem query_meaning (frozen : FrozenEvaluation) (tested : theory.Interpretation)
    (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (stored : NativeMembership.Represented tested) (accepted : Bool)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (completed : oracle.check_subset frozen.program tested frozen.values.slice old before =
      ok (core.result.Result.Ok accepted, output, after)) :
    accepted = true ↔ Ferraris.Models (TightEvaluation.interpretation
      (NativeMembership.denotes tested)) frozen.reduct := by
  exact NativeSubsetQuery.completed_reduct frozen.program frozen.candidate tested frozen.old old
    frozen.before before frozen.stored stored frozen.valid frozen.rootsBounded
    frozen.values output frozen.after after accepted frozen.completed completed

/-- A completed false query refutes exactly its represented positional state.
    It does not refute supersets, unrelated candidates or an unfinished query. -/
theorem query_refutes (frozen : FrozenEvaluation) {size : Nat}
    (atoms : List (Fin size)) (bits : List Bool) (tested : theory.Interpretation)
    (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (sizeExact : tested.theory.value.atoms.val = size)
    (represented : PackedSubsets.Represents (ScalarSubsets.raw tested.words)
      (SubsetCounter.selected atoms bits))
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (completed : oracle.check_subset frozen.program tested frozen.values.slice old before =
      ok (core.result.Result.Ok false, output, after)) :
    ¬ Ferraris.Models (TightEvaluation.interpretation
      (FiniteMembership.candidate ((SubsetCounter.selected atoms bits).map Fin.val)))
      frozen.reduct := by
  have exactMeaning : false = true ↔ Ferraris.Models
      (TightEvaluation.interpretation (NativeMembership.denotes tested)) frozen.reduct := by
    exact query_meaning frozen tested old before
      (NativeSearchRepresentation.stored _ tested sizeExact represented) false output after completed
  rw [NativeSearchRepresentation.denotes _ tested sizeExact represented] at exactMeaning
  intro modeled
  exact Bool.false_ne_true (exactMeaning.mpr modeled)

end NativeCountermodelSemantics
