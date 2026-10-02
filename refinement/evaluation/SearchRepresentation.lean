import ScalarSubsets
import SelectedAtoms

open Aeneas Aeneas.Std Result
open ZetesisExtract
open Zetesis Zetesis.Refinement

/-!
# Packed search states as semantic interpretations

The carry proof uses bounded coordinates. Extracted formula evaluation uses
natural coordinates. These lemmas establish their agreement from stored words,
rather than introducing a second evaluator or assuming agreement of predicates.
-/
namespace SearchRepresentation

/-- Exact packed storage for a bounded selection supplies every word that the
    actual membership operation may read. No padding bit creates an atom. -/
theorem stored {size : Nat} (chosen : List (Fin size))
    (candidate : theory.Interpretation)
    (sizeExact : candidate.theory.value.atoms.val = size)
    (represented : PackedSubsets.Represents (ScalarSubsets.raw candidate.words) chosen) :
    Membership.Represented candidate := by
  intro atom inside
  have bounded : atom < size := by simpa only [sizeExact] using inside
  have present := PackedSubsets.coordinate_stored (ScalarSubsets.raw candidate.words)
    chosen represented ⟨atom, bounded⟩
  simpa only [ScalarSubsets.raw, List.length_map] using present

/-- The extracted membership predicate is exactly the finite selected set,
    including false membership outside the declared universe. -/
theorem denotes {size : Nat} (chosen : List (Fin size))
    (candidate : theory.Interpretation)
    (sizeExact : candidate.theory.value.atoms.val = size)
    (represented : PackedSubsets.Represents (ScalarSubsets.raw candidate.words) chosen) :
    Membership.denotes candidate = FiniteMembership.candidate (chosen.map Fin.val) := by
  funext atom
  by_cases inside : atom < size
  · have bounded : atom < candidate.theory.value.atoms.val := by simpa only [sizeExact] using inside
    rw [SelectedAtoms.packed_denotation, PackedInterpretations.contains_eq_bit]
    change (decide (atom < candidate.theory.value.atoms.val) &&
      PackedInterpretations.bit64 (ScalarSubsets.raw candidate.words) atom) = _
    rw [represented.2]
    simp [FiniteMembership.candidate, bounded]
  · have absent : atom ∉ chosen.map Fin.val := by
      rintro member
      obtain ⟨coordinate, _, equal⟩ := List.mem_map.mp member
      exact inside (equal ▸ coordinate.isLt)
    simp [Membership.denotes, sizeExact, inside, FiniteMembership.candidate, absent]

/-- Forgetting the bound on coordinates commutes with positional selection.
    This transports the existing subset-counter laws to the evaluator's natural
    atom coordinates without rebuilding their coverage argument. -/
theorem selected_values {size : Nat} (atoms : List (Fin size)) (bits : List Bool) :
    (SubsetCounter.selected atoms bits).map Fin.val =
      SubsetCounter.selected (atoms.map Fin.val) bits := by
  induction atoms generalizing bits with
  | nil => simp [SubsetCounter.selected]
  | cons atom rest ih =>
    cases bits with
    | nil => simp [SubsetCounter.selected]
    | cons bit tail =>
      cases bit <;> simp [SubsetCounter.selected, ih]

/-- A represented, nonfull counter state is a proper semantic subset of the
    candidate coordinates. Distinctness rules out an omitted position denoting
    an atom that remains present at another position. -/
theorem proper {size : Nat} (atoms : List (Fin size)) (bits : List Bool)
    (candidate : theory.Interpretation)
    (unique : atoms.Nodup) (width : bits.length = atoms.length)
    (nonfull : SubsetCounter.full bits ≠ true)
    (sizeExact : candidate.theory.value.atoms.val = size)
    (represented : PackedSubsets.Represents (ScalarSubsets.raw candidate.words)
      (SubsetCounter.selected atoms bits)) :
    Ferraris.ProperSub (TightEvaluation.interpretation (Membership.denotes candidate))
      (TightEvaluation.interpretation (FiniteMembership.candidate (atoms.map Fin.val))) := by
  have uniqueValues : (atoms.map Fin.val).Nodup := List.Nodup.map Fin.val_injective unique
  have visited : bits ∈ SubsetCounter.states (atoms.map Fin.val).length := by
    apply (SubsetCounter.states_exact _ bits).mpr
    exact ⟨by simpa using width, nonfull⟩
  rw [denotes (SubsetCounter.selected atoms bits) candidate sizeExact represented, selected_values]
  exact SubsetCounter.selected_proper (atoms.map Fin.val) uniqueValues bits visited

end SearchRepresentation
