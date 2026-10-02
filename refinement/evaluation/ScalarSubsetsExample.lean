import ScalarSubsets

open Aeneas Aeneas.Std Result

/-!
# A packed update across the 64-bit word boundary

The example composes the actual backend operations used for a mutable word
update. It is not an extracted carry or subset-search loop. The two existing bits
straddle the updated coordinate, so the result checks both addressing and the
returned mutable-update function.
-/
namespace ScalarSubsetsExample

/-- Bits 63 and 65 are initially true; bit 64 between them is false. -/
def boundaryWords : alloc.vec.Vec U64 :=
  alloc.vec.Vec.from [9223372036854775808#u64, 2#u64] (by scalar_tac)

/-- All three neighboring coordinates are true after the boundary insertion. -/
def filledWords : alloc.vec.Vec U64 :=
  alloc.vec.Vec.from [9223372036854775808#u64, 3#u64] (by scalar_tac)

/-- Setting atom 64 addresses the second word at offset zero and leaves bits 63
and 65 true. The source-level mask, index and update primitives are composed in
the statement, rather than replaced by the abstract insertion operation. -/
theorem setting_at_the_boundary_preserves_neighbors :
    (do
      let index ← 64#usize / 64#usize
      let (value, replace) ← alloc.vec.Vec.index_mut
        (core.slice.index.SliceIndexUsizeSlice U64) boundaryWords index
      let offset ← 64#usize % 64#usize
      let mask ← 1#u64 <<< offset
      ok (replace (value ||| mask))) =
      ok filledWords := by
  obtain ⟨index, offset, mask, value, replace, divided, remainder, shifted,
      indexed, _, inserted, _⟩ := ScalarSubsets.word_operations boundaryWords 64#usize (by decide)
  have output : replace (value ||| mask) = filledWords := by
    apply alloc.vec.Vec.ext
    apply (List.map_inj_right (fun left right same => U64.bv_eq_imp_eq left right same)).mp
    change ScalarSubsets.raw (replace (value ||| mask)) = ScalarSubsets.raw filledWords
    rw [inserted]
    rfl
  rw [divided]
  simp only [bind_tc_ok]
  rw [indexed]
  simp only [bind_tc_ok]
  rw [remainder]
  simp only [bind_tc_ok]
  rw [shifted]
  simp only [bind_tc_ok]
  exact congrArg Result.ok output

/-- Clearing the boundary bit from that result uses the same checked address
and AND-complement update, restoring exactly the original two words. -/
theorem clearing_at_the_boundary_preserves_neighbors :
    (do
      let index ← 64#usize / 64#usize
      let (value, replace) ← alloc.vec.Vec.index_mut
        (core.slice.index.SliceIndexUsizeSlice U64) filledWords index
      let offset ← 64#usize % 64#usize
      let mask ← 1#u64 <<< offset
      ok (replace (value &&& ~~~mask))) = ok boundaryWords := by
  obtain ⟨index, offset, mask, value, replace, divided, remainder, shifted,
      indexed, _, _, cleared⟩ := ScalarSubsets.word_operations filledWords 64#usize (by decide)
  have output : replace (value &&& ~~~mask) = boundaryWords := by
    apply alloc.vec.Vec.ext
    apply (List.map_inj_right (fun left right same => U64.bv_eq_imp_eq left right same)).mp
    change ScalarSubsets.raw (replace (value &&& ~~~mask)) = ScalarSubsets.raw boundaryWords
    rw [cleared]
    rfl
  rw [divided]
  simp only [bind_tc_ok]
  rw [indexed]
  simp only [bind_tc_ok]
  rw [remainder]
  simp only [bind_tc_ok]
  rw [shifted]
  simp only [bind_tc_ok]
  exact congrArg Result.ok output

end ScalarSubsetsExample
