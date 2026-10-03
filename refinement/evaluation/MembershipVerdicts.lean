import PackedSetup

open Aeneas Aeneas.Std Result
open ZetesisExtract
open Zetesis Zetesis.Refinement

/-!
# Negative membership verdicts and their evidence

A completed original-root failure refutes original satisfaction. A completed
positive subset search returns the actual proper-subset witness, rather than
merely asserting that some countermodel exists. Both refute answer-set membership.

These laws concern generated phase calls under the recorded library models.
PublicMembership derives these calls from completed public-wrapper returns,
retaining explicit input, allocation and ownership contracts.
-/
namespace MembershipVerdicts

/-- An actual original-root failure excludes answer-set membership. The reason
is failed satisfaction of the original theory, independently of subset search.
Cancellation and work exhaustion cannot satisfy this completed-root premise. -/
theorem original_failure (frozen : CountermodelSemantics.FrozenEvaluation)
    (root : Usize) (after : oracle.Work)
    (failed : oracle.failed_root frozen.program frozen.values.slice frozen.after =
      ok (core.result.Result.Ok (some root), after)) :
    ¬ Ferraris.Stable (TightEvaluation.interpretation (Membership.denotes frozen.candidate))
      (RootSemantics.assertions frozen.program) := by
  have satisfaction := TheorySatisfaction.completed_original frozen.program frozen.candidate
    frozen.old frozen.before frozen.after frozen.stored frozen.ordered frozen.rootsBounded
    frozen.values frozen.after after (some root) frozen.completed failed
  intro stable
  have impossible : (some root : Option Usize) = none := satisfaction.mpr stable.1
  cases impossible

/-- A completed true search returns its own proper-subset reduct model.
Finite scan and counter proofs supply coverage and identity of that witness;
no separately proposed or assumed countermodel is substituted for the result. -/
theorem returned_witness (frozen : CountermodelSemantics.FrozenEvaluation)
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
    Ferraris.ProperSub (TightEvaluation.interpretation (Membership.denotes witness))
      (TightEvaluation.interpretation (Membership.denotes frozen.candidate)) ∧
      Ferraris.Models (TightEvaluation.interpretation (Membership.denotes witness)) frozen.reduct := by
  let atoms := PackedSubsets.selectedAtoms frozen.candidate.theory.value.atoms.val
    (frozen.candidate.words.val.map UScalar.bv)
  have coordinates : selected.val.map UScalar.val = atoms.map Fin.val :=
    FixedSelection.completed_selection frozen.program frozen.candidate destination selectionWork
      sameUniverse frozen.stored vacant selected selectionAfter selectedComplete
  have carrier : FiniteMembership.candidate (atoms.map Fin.val) =
      Membership.denotes frozen.candidate := by
    rw [← coordinates]
    exact (FixedSelection.completed_carrier frozen.program frozen.candidate destination selectionWork
      sameUniverse frozen.stored vacant selected selectionAfter selectedComplete).2.2
  obtain ⟨outcome, actual, report⟩ := MembershipSearch.empty_search frozen atoms selected.slice
    coordinates (PackedSubsets.selected_atoms_nodup _ _) empty old selectionAfter sizeExact zero
  have same : (outcome.result, outcome.subset, outcome.values, outcome.work) =
      (core.result.Result.Ok true, witness, output, after) :=
    Result.ok_injective (actual.symm.trans completed)
  have resultExact : outcome.result = core.result.Result.Ok true := congrArg Prod.fst same
  have witnessExact : outcome.subset = witness := congrArg (fun value => value.2.1) same
  simpa only [FixedCountermodelSearch.Report, resultExact, witnessExact, carrier] using report

/-- A proper-subset model of the actual frozen reduct excludes membership,
even when the original candidate satisfies every asserted root. This uses the
same reduct as the successful original evaluation, not a separate oracle. -/
theorem witness_excludes_membership (frozen : CountermodelSemantics.FrozenEvaluation)
    (witness : theory.Interpretation)
    (proper : Ferraris.ProperSub (TightEvaluation.interpretation (Membership.denotes witness))
      (TightEvaluation.interpretation (Membership.denotes frozen.candidate)))
    (modeled : Ferraris.Models (TightEvaluation.interpretation (Membership.denotes witness))
      frozen.reduct) :
    ¬ Ferraris.Stable (TightEvaluation.interpretation (Membership.denotes frozen.candidate))
      (RootSemantics.assertions frozen.program) := by
  intro stable
  exact stable.2 ⟨TightEvaluation.interpretation (Membership.denotes witness), proper, modeled⟩

end MembershipVerdicts
