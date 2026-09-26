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

Ranks below describe valid positions in one fixed catalog. Equal decoded atoms
must have equal ranks, and strict rank order must agree with the supplied atom
order. The laws assume those correspondences; they do not construct ranks or
prove a sorting implementation. A rank has no meaning in another catalog.
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

/-- Complete coverage of the same rank classes preserves selected truth.
Positions are valid in one fixed catalog; distinct positions may decode to the
same atom. Each direction chooses a representative with the same rank and then
uses decoded equality. Numeric rank identity alone supplies no owner transfer. -/
theorem rank_coverage_preserves_interpretation
    (catalog : Slot → Atom) (rank : Slot → Nat)
    (same_atom : ∀ left right, rank left = rank right ↔ catalog left = catalog right)
    (positions representatives : List Slot)
    (covered : ∀ key,
      key ∈ representatives.map rank ↔ key ∈ positions.map rank) :
    denotes (fun position => some (catalog position)) representatives =
      denotes (fun position => some (catalog position)) positions := by
  apply atoms_ext
  intro atom
  constructor
  · rintro ⟨position, present, decoded⟩
    have ranked : rank position ∈ positions.map rank :=
      (covered (rank position)).mp (List.mem_map.mpr ⟨position, present, rfl⟩)
    obtain ⟨original, selected, same_rank⟩ := List.mem_map.mp ranked
    have same_value : catalog original = catalog position :=
      (same_atom original position).mp same_rank
    exact ⟨original, selected, (congrArg some same_value).trans decoded⟩
  · rintro ⟨position, present, decoded⟩
    have ranked : rank position ∈ representatives.map rank :=
      (covered (rank position)).mpr (List.mem_map.mpr ⟨position, present, rfl⟩)
    obtain ⟨representative, selected, same_rank⟩ := List.mem_map.mp ranked
    have same_value : catalog representative = catalog position :=
      (same_atom representative position).mp same_rank
    exact ⟨representative, selected, (congrArg some same_value).trans decoded⟩

/-- Permuting selected positions and retaining one representative of each rank
preserves their interpretation. Coalescing is expressed on ranks, not raw
positions: two distinct occurrences may denote the same atom. Permutation and
duplicate removal use `canonicalization_exact` on ranks; the preceding law
then supplies decoded truth. -/
theorem rank_coalescing_preserves_interpretation
    (catalog : Slot → Atom) (rank : Slot → Nat)
    (same_atom : ∀ left right, rank left = rank right ↔ catalog left = catalog right)
    (positions ordered representatives : List Slot)
    (permutation : ordered.Perm positions)
    (coalesced : representatives.map rank = (ordered.map rank).eraseDups) :
    denotes (fun position => some (catalog position)) representatives =
      denotes (fun position => some (catalog position)) positions := by
  have ordered_values : (ordered.map rank).Perm
      (positions.filterMap (fun position => some (rank position))) := by
    simpa only [List.filterMap_eq_map'] using permutation.map rank
  have covered : ∀ key,
      key ∈ representatives.map rank ↔ key ∈ positions.map rank := by
    intro key
    rw [coalesced]
    have normalized := canonicalization_exact (fun position => some (rank position))
      positions (ordered.map rank) ordered_values key
    simpa only [denotes, List.mem_map, Option.some.injEq] using normalized
  exact rank_coverage_preserves_interpretation catalog rank same_atom
    positions representatives covered

/-- Strict rank order transports exactly to the decoded atom order when each
comparison agrees. The order relation is explicit; this does not choose ASP
arithmetic order or assume that occurrence positions are already ordered. -/
theorem rank_order_iff
    (catalog : Slot → Atom) (rank : Slot → Nat) (before : Atom → Atom → Prop)
    (ordered_ranks : ∀ left right,
      rank left < rank right ↔ before (catalog left) (catalog right))
    (positions : List Slot) :
    (positions.map rank).Pairwise (fun left right => left < right) ↔
      (positions.map catalog).Pairwise before := by
  simp only [List.pairwise_map]
  constructor
  · intro ranked
    exact ranked.imp (fun {left right} less => (ordered_ranks left right).mp less)
  · intro decoded
    exact decoded.imp (fun {left right} less => (ordered_ranks left right).mpr less)

/-- Strictly increasing ranks exclude duplicate decoded atoms when equal atoms
have equal ranks. Distinct equal-valued occurrence positions remain admissible
in the catalog, but cannot both survive this selected order. -/
theorem increasing_ranks_decode_unique
    (catalog : Slot → Atom) (rank : Slot → Nat)
    (same_atom : ∀ left right, rank left = rank right ↔ catalog left = catalog right)
    (positions : List Slot)
    (ordered : (positions.map rank).Pairwise (fun left right => left < right)) :
    (positions.map catalog).Nodup := by
  have positions_ordered : positions.Pairwise (fun left right => rank left < rank right) :=
    List.pairwise_map.mp ordered
  have distinct : positions.Pairwise (fun left right => catalog left ≠ catalog right) := by
    apply positions_ordered.imp
    intro left right less equal
    exact (Nat.ne_of_lt less) ((same_atom left right).mpr equal)
  exact List.pairwise_map.mpr distinct

end Zetesis.ModelSelections
