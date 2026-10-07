import NativePackedSetup

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract
open Zetesis Zetesis.Refinement

/-!
# Negative membership verdicts and their evidence

A completed original-root failure refutes original satisfaction. A completed
positive subset search returns the actual proper-subset witness, rather than
merely asserting that some countermodel exists. Both refute answer-set membership.

These laws concern generated phase calls under the recorded library models.
NativePublicMembership derives these calls from completed public-wrapper returns,
retaining explicit input, allocation and ownership contracts.
-/
namespace NativeMembershipVerdicts

/-- An actual original-root failure excludes answer-set membership. The reason
is failed satisfaction of the original theory, independently of subset search.
Cancellation and work exhaustion cannot satisfy this completed-root premise. -/
theorem original_failure (frozen : NativeCountermodelSemantics.FrozenEvaluation)
    (root : Usize) (after : oracle.Work)
    (failed : oracle.failed_root frozen.program frozen.values.slice frozen.after =
      ok (core.result.Result.Ok (some root), after)) :
    ¬ Ferraris.Stable (TightEvaluation.interpretation (NativeMembership.denotes frozen.candidate))
      (NativeRootSemantics.assertions frozen.program) := by
  have satisfaction : (some root : Option Usize) = none ↔
      Ferraris.Models (TightEvaluation.interpretation (NativeMembership.denotes frozen.candidate))
        (NativeRootSemantics.assertions frozen.program) := by
    exact NativeRootSemantics.completed_original frozen.program frozen.candidate
      frozen.old frozen.before frozen.after frozen.valid frozen.stored frozen.rootsBounded
      frozen.values frozen.after after (some root) frozen.completed failed
  intro stable
  have impossible : (some root : Option Usize) = none := by
    exact satisfaction.mpr stable.1
  cases impossible

/-- A completed true search returns its own proper-subset reduct model.
Finite scan and counter proofs supply coverage and identity of that witness;
no separately proposed or assumed countermodel is substituted for the result. -/
theorem returned_witness (frozen : NativeCountermodelSemantics.FrozenEvaluation)
    (destination : alloc.vec.Vec Usize) (selectionWork : oracle.Work)
    (sameUniverse : frozen.program.value.atoms = frozen.candidate.theory.value.atoms)
    (vacant : destination.val = [])
    (selected : alloc.vec.Vec Usize) (selectionAfter : oracle.Work)
    (selectedComplete : oracle.select_atoms frozen.program frozen.candidate destination selectionWork =
      ok (core.result.Result.Ok (), selected, selectionAfter))
    (empty : theory.Interpretation) (old : alloc.vec.Vec Bool)
    (sizeExact : empty.theory.value.atoms.val = frozen.candidate.theory.value.atoms.val)
    (zero : PackedSubsets.Represents (ScalarSubsets.raw empty.words)
      ([] : List (Fin frozen.candidate.theory.value.atoms.val)))
    (witness : theory.Interpretation) (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (completed : oracle.find_countermodel frozen.program frozen.values.slice selected.slice
      empty old selectionAfter = ok (core.result.Result.Ok true, witness, output, after)) :
    Ferraris.ProperSub (TightEvaluation.interpretation (NativeMembership.denotes witness))
      (TightEvaluation.interpretation (NativeMembership.denotes frozen.candidate)) ∧
      Ferraris.Models (TightEvaluation.interpretation (NativeMembership.denotes witness)) frozen.reduct := by
  let atoms := PackedSubsets.selectedAtoms frozen.candidate.theory.value.atoms.val
    (frozen.candidate.words.val.map UScalar.bv)
  have coordinates : selected.val.map UScalar.val = atoms.map Fin.val := by
    exact NativeFixedSelection.completed_selection frozen.program frozen.candidate destination selectionWork
        sameUniverse frozen.stored vacant selected selectionAfter selectedComplete
  have carrier : FiniteMembership.candidate (atoms.map Fin.val) =
      NativeMembership.denotes frozen.candidate := by
    rw [← coordinates]
    exact (NativeFixedSelection.completed_carrier frozen.program frozen.candidate destination selectionWork
      sameUniverse frozen.stored vacant selected selectionAfter selectedComplete).2.2
  exact NativeMembershipSearch.completed_witness frozen atoms selected.slice coordinates
    (PackedSubsets.selected_atoms_nodup _ _) carrier empty old selectionAfter sizeExact zero
    witness output after completed

/-- A proper-subset model of the actual frozen reduct excludes membership,
even when the original candidate satisfies every asserted root. This uses the
same reduct as the successful original evaluation, not a separate oracle. -/
theorem witness_excludes_membership (frozen : NativeCountermodelSemantics.FrozenEvaluation)
    (witness : theory.Interpretation)
    (proper : Ferraris.ProperSub (TightEvaluation.interpretation (NativeMembership.denotes witness))
      (TightEvaluation.interpretation (NativeMembership.denotes frozen.candidate)))
    (modeled : Ferraris.Models (TightEvaluation.interpretation (NativeMembership.denotes witness))
      frozen.reduct) :
    ¬ Ferraris.Stable (TightEvaluation.interpretation (NativeMembership.denotes frozen.candidate))
      (NativeRootSemantics.assertions frozen.program) := by
  intro stable
  exact stable.2 ⟨TightEvaluation.interpretation (NativeMembership.denotes witness), proper, modeled⟩

end NativeMembershipVerdicts
