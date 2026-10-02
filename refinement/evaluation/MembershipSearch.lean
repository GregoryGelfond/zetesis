import FixedSearch
import FixedSelection

open Aeneas Aeneas.Std Result
open ZetesisExtract
open Zetesis Zetesis.Refinement

/-!
# Completed membership through the actual search

The search begins at empty packed storage. Its selected coordinates come from
the actual candidate scan, and its frozen values come from actual evaluation.
Combining those calls with the original root check recovers the general
answer-set definition. The public allocation and owner-checking wrapper is
not extracted here; its setup obligations remain explicit.
-/
namespace MembershipSearch

open CountermodelSemantics CountermodelTrace FixedCountermodelSearch

/-- Starting at the empty subset establishes the counter invariant and yields
    an actual typed search result. The destination's old truth values need no
    semantic invariant because every actual query clears them before evaluation.
-/
theorem empty_search (frozen : FrozenEvaluation) {size : Nat}
    (atoms : List (Fin size)) (selected : Slice Usize)
    (coordinates : selected.val.map UScalar.val = atoms.map Fin.val)
    (unique : atoms.Nodup) (empty : theory.Interpretation)
    (old : alloc.vec.Vec Bool) (work : oracle.Work)
    (sizeExact : empty.theory.value.atoms.val = size)
    (zero : PackedSubsets.Represents (ScalarSubsets.raw empty.words) ([] : List (Fin size))) :
    ∃ outcome : Outcome,
      oracle.find_countermodel frozen.program frozen.values.slice selected empty old work =
        ok (outcome.result, outcome.subset, outcome.values, outcome.work) ∧
      Report frozen atoms (List.replicate atoms.length false) outcome := by
  have invariant : Invariant atoms (List.replicate atoms.length false)
      ⟨empty, old, work, 0#usize⟩ := by
    refine ⟨List.length_replicate, sizeExact, ?_, ?_⟩
    · simpa only [PackedSubsets.selected_false] using zero
    · simp only [PackedSubsets.population_zero, UScalar.ofNatCore_val_eq]
  obtain ⟨outcome, calls, report⟩ := calls_refine frozen atoms selected coordinates unique
    (List.replicate atoms.length false) ⟨empty, old, work, 0#usize⟩ invariant
  exact ⟨outcome, entry_executes frozen.program frozen.values.slice selected empty old
    work outcome calls, report⟩

/-- A completed search from zero returns true exactly when a proper subset
    models the frozen reduct. Finite-counter coverage supplies all subsets;
    completed false is not inferred from a timeout, quota or cancellation.

    Proof: identify the actual completed result with the constructed execution.
    A true report gives its witness. For false, represent any proposed semantic
    proper subset by a counter state and use the report's complete rank interval.
-/
theorem completed_countermodel (frozen : FrozenEvaluation) {size : Nat}
    (atoms : List (Fin size)) (selected : Slice Usize)
    (coordinates : selected.val.map UScalar.val = atoms.map Fin.val)
    (unique : atoms.Nodup)
    (carrier : FiniteMembership.candidate (atoms.map Fin.val) =
      Membership.denotes frozen.candidate)
    (empty : theory.Interpretation) (old : alloc.vec.Vec Bool) (work : oracle.Work)
    (sizeExact : empty.theory.value.atoms.val = size)
    (zero : PackedSubsets.Represents (ScalarSubsets.raw empty.words) ([] : List (Fin size)))
    (found : Bool) (subset : theory.Interpretation)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (completed : oracle.find_countermodel frozen.program frozen.values.slice selected empty old work =
      ok (core.result.Result.Ok found, subset, output, after)) :
    found = true ↔ ∃ J,
      Ferraris.ProperSub J (TightEvaluation.interpretation (Membership.denotes frozen.candidate)) ∧
      Ferraris.Models J frozen.reduct := by
  obtain ⟨outcome, actual, report⟩ := empty_search frozen atoms selected coordinates unique
    empty old work sizeExact zero
  have same : (outcome.result, outcome.subset, outcome.values, outcome.work) =
      (core.result.Result.Ok found, subset, output, after) :=
    Result.ok_injective (actual.symm.trans completed)
  have answer : outcome.result = core.result.Result.Ok found := congrArg Prod.fst same
  cases found with
  | true =>
    have witness : Ferraris.ProperSub
        (TightEvaluation.interpretation (Membership.denotes outcome.subset))
        (TightEvaluation.interpretation (Membership.denotes frozen.candidate)) ∧
        Ferraris.Models (TightEvaluation.interpretation (Membership.denotes outcome.subset))
          frozen.reduct := by
      simpa only [Report, answer, carrier] using report
    exact ⟨fun _ => ⟨_, witness⟩, fun _ => rfl⟩
  | false =>
    have refuted : ∀ tested : List Bool, tested.length = atoms.length →
        SubsetCounter.full tested ≠ true →
        ¬ Ferraris.Models (TightEvaluation.interpretation (FiniteMembership.candidate
          ((SubsetCounter.selected atoms tested).map Fin.val))) frozen.reduct := by
      intro tested width nonfull
      simp only [Report, answer] at report
      exact report tested width (by rw [SubsetCounter.zero_rank]; exact Nat.zero_le _) nonfull
    constructor
    · intro impossible
      cases impossible
    · rintro ⟨J, proper, modeled⟩
      have properCarrier : Ferraris.ProperSub J
          (TightEvaluation.interpretation (FiniteMembership.candidate (atoms.map Fin.val))) := by
        simpa only [carrier] using proper
      obtain ⟨bits, visited, represents⟩ :=
        SubsetCounter.proper_has_state (atoms.map Fin.val) J properCarrier
      obtain ⟨width, nonfull⟩ := (SubsetCounter.states_exact _ bits).mp visited
      have absent := refuted bits (by simpa using width) nonfull
      rw [SearchRepresentation.selected_values, represents] at absent
      exact False.elim (absent modeled)

/-- Successful actual selection supplies the carrier, order and uniqueness for
    the completed search theorem. These properties are derived from the scan;
    no caller-provided subset enumeration or oracle agreement is assumed. -/
theorem selected_countermodel (frozen : FrozenEvaluation)
    (destination : alloc.vec.Vec Usize) (selectionWork : oracle.Work)
    (sameUniverse : frozen.program.value.atoms = frozen.candidate.theory.value.atoms)
    (vacant : destination.val = [])
    (selected : alloc.vec.Vec Usize) (selectionAfter : oracle.Work)
    (selectedComplete : oracle.select_atoms frozen.program frozen.candidate destination selectionWork =
      ok (core.result.Result.Ok (), selected, selectionAfter))
    (empty : theory.Interpretation) (old : alloc.vec.Vec Bool) (work : oracle.Work)
    (sizeExact : empty.theory.value.atoms.val = frozen.candidate.theory.value.atoms.val)
    (zero : PackedSubsets.Represents (ScalarSubsets.raw empty.words)
      ([] : List (Fin frozen.candidate.theory.value.atoms.val)))
    (found : Bool) (subset : theory.Interpretation)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (completed : oracle.find_countermodel frozen.program frozen.values.slice selected.slice empty old work =
      ok (core.result.Result.Ok found, subset, output, after)) :
    found = true ↔ ∃ J,
      Ferraris.ProperSub J (TightEvaluation.interpretation (Membership.denotes frozen.candidate)) ∧
      Ferraris.Models J frozen.reduct := by
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
  exact completed_countermodel frozen atoms selected.slice coordinates
    (PackedSubsets.selected_atoms_nodup _ _) carrier empty old work sizeExact zero
    found subset output after completed

/-- Completed original checking and completed proper-subset search agree with
    the general answer-set definition. Every semantic phase is connected to
    actual extracted calls, threading their returned work records in source order. An exhausted resource has no Boolean in this theorem.

    The remaining implementation boundary is the public wrapper that establishes
    ownership and allocated setup, plus the existing extraction and platform
    models. This theorem does not certify source grounding or GPU execution.
-/
theorem completed_answer_set (frozen : FrozenEvaluation)
    (rootAfter : oracle.Work)
    (originalModel : oracle.failed_root frozen.program frozen.values.slice frozen.after =
      ok (core.result.Result.Ok none, rootAfter))
    (destination : alloc.vec.Vec Usize)
    (sameUniverse : frozen.program.value.atoms = frozen.candidate.theory.value.atoms)
    (vacant : destination.val = [])
    (selected : alloc.vec.Vec Usize) (selectionAfter : oracle.Work)
    (selectedComplete : oracle.select_atoms frozen.program frozen.candidate destination rootAfter =
      ok (core.result.Result.Ok (), selected, selectionAfter))
    (empty : theory.Interpretation) (old : alloc.vec.Vec Bool)
    (sizeExact : empty.theory.value.atoms.val = frozen.candidate.theory.value.atoms.val)
    (zero : PackedSubsets.Represents (ScalarSubsets.raw empty.words)
      ([] : List (Fin frozen.candidate.theory.value.atoms.val)))
    (found : Bool) (subset : theory.Interpretation)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (completed : oracle.find_countermodel frozen.program frozen.values.slice selected.slice empty old selectionAfter =
      ok (core.result.Result.Ok found, subset, output, after)) :
    found = false ↔ Ferraris.Stable
      (TightEvaluation.interpretation (Membership.denotes frozen.candidate))
      (RootSemantics.assertions frozen.program) := by
  have model : Ferraris.Models
      (TightEvaluation.interpretation (Membership.denotes frozen.candidate))
      (RootSemantics.assertions frozen.program) :=
    (TheorySatisfaction.completed_original frozen.program frozen.candidate frozen.old
      frozen.before frozen.after frozen.stored frozen.ordered frozen.rootsBounded frozen.values
      frozen.after rootAfter none frozen.completed originalModel).mp rfl
  have countermodel := selected_countermodel frozen destination rootAfter sameUniverse vacant
    selected selectionAfter selectedComplete empty old selectionAfter sizeExact zero found subset output after completed
  change found = false ↔ _ ∧ ¬ ∃ J, Ferraris.ProperSub J _ ∧ Ferraris.Models J frozen.reduct
  rw [← countermodel]
  cases found <;> simp [model]

end MembershipSearch
