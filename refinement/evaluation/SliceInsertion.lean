import ScalarSubsets

open Aeneas Aeneas.Std Result
open Zetesis.Refinement

/-!
# Checked packed insertion through a mutable slice

The generated constructor helper uses checked scalar arithmetic, slice indexing
and a slice update. This law identifies those actual operations with the shared
packed insertion primitive. Storage coverage is a premise, not a default read
that might hide an invalid index. Every unrelated bit and the word count are
preserved by the shared insertion law.
-/
namespace SliceInsertion

/-- The generated insertion block sets exactly the selected bit of a stored
word and preserves the slice length. All scalar and storage operations are
actual backend operations, with their bounds derived from the atom coordinate.

Proof: checked division/remainder locate the word and bit. The remainder bounds
the shift; storage coverage bounds indexing and update. Mapping the returned
slice to bit vectors commutes with that update and gives packed insertion. -/
theorem word_operations (words : Slice U64) (atom : Usize)
    (stored : atom.val / 64 < words.val.length) :
    ∃ index offset : Usize, ∃ mask value : U64, ∃ updated : Slice U64,
      atom / 64#usize = ok index ∧
      atom % 64#usize = ok offset ∧
      (1#u64 <<< offset) = ok mask ∧
      Slice.index_usize words index = ok value ∧
      Slice.update words index (value ||| mask) = ok updated ∧
      updated.val.length = words.val.length ∧
      updated.val.map UScalar.bv =
        PackedInterpretations.insert (words.val.map UScalar.bv) atom.val := by
  obtain ⟨index, divided, indexValue, _⟩ :=
    UScalar.div_bv_spec atom (y := 64#usize) (by simp)
  obtain ⟨offset, remainder, offsetValue⟩ :=
    WP.spec_imp_exists (UScalar.rem_spec atom (y := 64#usize) (by simp))
  have indexExact : index.val = atom.val / 64 := by simpa using indexValue
  have offsetExact : offset.val = atom.val % 64 := by simpa using offsetValue
  have offsetBound : offset.val < 64 := by
    rw [offsetExact]
    exact Nat.mod_lt _ (by decide)
  obtain ⟨mask, shifted, _, maskBits⟩ := WP.spec_imp_exists
    (U64.ShiftLeft_spec (1#u64) offset offsetBound)
  have maskExact : mask.bv = 1#64 <<< offset.val := by
    simpa only [show (1#u64).bv = 1#64 from rfl] using maskBits
  have indexBound : index.val < words.length := by simpa [indexExact] using stored
  obtain ⟨value, indexed, valueExact⟩ :=
    WP.spec_imp_exists (Slice.index_usize_spec words index indexBound)
  obtain ⟨updated, written, updateExact⟩ :=
    WP.spec_imp_exists (Slice.update_spec words index (value ||| mask) indexBound)
  have contents : updated.val = words.val.set index.val (value ||| mask) := by
    rw [updateExact]
    exact Slice.set_val_eq words index (value ||| mask)
  have readBits :
      PackedInterpretations.word (words.val.map UScalar.bv) (atom.val / 64) = value.bv := by
    simp [PackedInterpretations.word, valueExact, indexExact,
      List.getElem?_eq_getElem stored]
  have inserted : updated.val.map UScalar.bv =
      PackedInterpretations.insert (words.val.map UScalar.bv) atom.val := by
    rw [contents, List.map_set]
    change (words.val.map UScalar.bv).set index.val (value.bv ||| mask.bv) = _
    rw [maskExact, offsetExact, indexExact]
    simp only [PackedInterpretations.insert, readBits]
  have sameLength : updated.val.length = words.val.length := by
    simp [contents]
  exact ⟨index, offset, mask, value, updated, divided, remainder, shifted,
    indexed, written, sameLength, inserted⟩

end SliceInsertion
