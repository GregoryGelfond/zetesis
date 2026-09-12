import Zetesis.Core

/-!
# Interpretations selected from retained catalogs

A retained catalog maps original positions to optional typed atoms. A checked
selection denotes precisely the atoms at its selected positions. Ordering and
coalescing equal atoms preserve that denotation. Atom storage outside the
selection does not change truth, even though its owner remains live.

The laws below explain dense-ID preservation and canonical logical equality.
Rust index validation, comparator laws, Arc lifetimes, resource accounting and
allocation remain executable refinement obligations. This representation alone
establishes neither satisfaction nor answer-set membership.
-/

namespace Zetesis.ModelSelections

universe u v w
variable {Slot : Type u} {OtherSlot : Type v} {Atom : Type w}

/-- True atoms witnessed by a selected original catalog position. -/
def denotes (catalog : Slot → Option Atom) (positions : List Slot) : Atoms Atom :=
  fun atom => ∃ position ∈ positions, catalog position = some atom

/-- Canonical ordering and duplicate removal preserve the decoded true set.
Each successful optional lookup supplies exactly one atom. A permutation changes
only arrival order, while duplicate removal preserves set membership. -/
theorem canonicalization_exact [BEq Atom] [LawfulBEq Atom]
    (catalog : Slot → Option Atom) (positions : List Slot) (ordered : List Atom)
    (ordered_values : ordered.Perm (positions.filterMap catalog)) (atom : Atom) :
    atom ∈ ordered.eraseDups ↔ denotes catalog positions atom := by
  have same_members : atom ∈ ordered ↔ atom ∈ positions.filterMap catalog :=
    ordered_values.mem_iff
  simpa only [List.mem_eraseDups, List.mem_filterMap, denotes] using same_members

/-- Catalogs agreeing at every selected position denote the same interpretation.
The whole catalog may remain owned; its unselected entries supply no true atoms. -/
theorem unselected_entries_irrelevant
    (first second : Slot → Option Atom) (positions : List Slot)
    (agree : ∀ position ∈ positions, first position = second position) :
    denotes first positions = denotes second positions := by
  apply atoms_ext
  intro atom
  constructor
  · rintro ⟨position, selected, value⟩
    exact ⟨position, selected, (agree position selected).symm.trans value⟩
  · rintro ⟨position, selected, value⟩
    exact ⟨position, selected, (agree position selected).trans value⟩

/-- Renumbering selected positions preserves their interpretation when each
new position decodes to the same optional atom. The correspondence premise,
not equality of raw integer IDs, establishes unchanged logical identity. -/
theorem renumbering_preserves_interpretation
    (original : Slot → Option Atom) (replacement : OtherSlot → Option Atom)
    (positions : List Slot) (renumber : Slot → OtherSlot)
    (preserves : ∀ position ∈ positions,
      replacement (renumber position) = original position) :
    denotes replacement (positions.map renumber) = denotes original positions := by
  apply atoms_ext
  intro atom
  constructor
  · rintro ⟨mapped, selected, value⟩
    obtain ⟨position, present, rfl⟩ := List.mem_map.mp selected
    exact ⟨position, present, (preserves position present).symm.trans value⟩
  · rintro ⟨position, selected, value⟩
    exact ⟨renumber position, List.mem_map.mpr ⟨position, selected, rfl⟩,
      (preserves position selected).trans value⟩

end Zetesis.ModelSelections
