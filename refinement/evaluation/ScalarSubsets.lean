import Membership
import Zetesis.PackedSubsets

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Checked scalar and vector primitives for packed subset updates

The actual Aeneas backend operations for unsigned division, remainder, shifting,
mutable vector indexing and returned updates implement the shared packed bit
operations. Population updates use checked machine addition and subtraction;
their bounds come from the positional count invariant.

These are primitive refinement lemmas. `SubsetCarry` composes them with the
actual extracted carry loop. Allocation, owner identity, concurrent observations
and the public membership wrapper remain separate correspondence obligations.
-/
namespace ScalarSubsets

open Zetesis.Refinement

/-- The mathematical word list retains every stored word and its exact bits.
It performs no universe clipping or padding repair. -/
def raw (words : alloc.vec.Vec U64) : List (BitVec 64) :=
  words.val.map UScalar.bv

/-- A bounded atom of the current extracted interpretation names an existing
word. Its raw bit view is exactly the interpretation's membership predicate;
no default value is used to hide missing storage. -/
theorem interpretation_storage (candidate : theory.Interpretation) (atom : Usize)
    (represented : Membership.Represented candidate)
    (inside : atom.val < candidate.theory.value.atoms.val) :
    atom.val / 64 < candidate.words.val.length ∧
      PackedInterpretations.bit64 (raw candidate.words) atom.val =
        Membership.denotes candidate atom.val := by
  have stored : atom.val / 64 < candidate.words.val.length := represented atom.val inside
  refine ⟨stored, ?_⟩
  simp [PackedInterpretations.bit64, PackedInterpretations.word, raw,
    Membership.denotes, inside, List.getElem?_eq_getElem stored]

/-- A mutable word access uses the actual checked arithmetic and indexing
operations. Its successful outputs produce precisely the shared set and clear
operations, and its mask test is the raw membership bit. The sole storage
premise prevents a default word from hiding an invalid mutable access.

Proof: derive quotient and remainder from the scalar specifications. The
remainder bounds the checked shift. The actual mutable index operation returns
the selected value and its vector-update function. Mapping that function's
result to bits commutes with the shared list update. -/
theorem word_operations (words : alloc.vec.Vec U64) (atom : Usize)
    (stored : atom.val / 64 < words.val.length) :
    ∃ index offset : Usize, ∃ mask value : U64, ∃ replace : U64 → alloc.vec.Vec U64,
      atom / 64#usize = ok index ∧
      atom % 64#usize = ok offset ∧
      (1#u64 <<< offset) = ok mask ∧
      alloc.vec.Vec.index_mut (core.slice.index.SliceIndexUsizeSlice U64) words index =
        ok (value, replace) ∧
      (value &&& mask != 0#u64) = PackedInterpretations.bit64 (raw words) atom.val ∧
      raw (replace (value ||| mask)) = PackedInterpretations.insert (raw words) atom.val ∧
      raw (replace (value &&& ~~~mask)) = PackedSubsets.clear (raw words) atom.val := by
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
  obtain ⟨⟨value, replace⟩, indexed, valueExact, replaceExact⟩ := WP.spec_imp_exists
    (alloc.vec.Vec.index_mut_usize_spec words index indexBound)
  have indexedTrait :
      alloc.vec.Vec.index_mut (core.slice.index.SliceIndexUsizeSlice U64) words index =
        ok (value, replace) := by
    simpa only [alloc.vec.Vec.index_mut_slice_index] using indexed
  have readBits : PackedInterpretations.word (raw words) (atom.val / 64) = value.bv := by
    simp [PackedInterpretations.word, raw, valueExact, indexExact,
      List.getElem?_eq_getElem stored]
  have mappedUpdate (replacement : U64) :
      raw (replace replacement) = (raw words).set index.val replacement.bv := by
    rw [replaceExact]
    simp [raw, List.map_set]
  have tested : (value &&& mask != 0#u64) = PackedInterpretations.bit64 (raw words) atom.val := by
    change (value.bv &&& mask.bv != 0#64) = _
    rw [maskExact, Membership.mask_bit _ _ offsetBound]
    simp only [PackedInterpretations.bit64, readBits, offsetExact]
  have inserted : raw (replace (value ||| mask)) =
      PackedInterpretations.insert (raw words) atom.val := by
    rw [mappedUpdate]
    change (raw words).set index.val (value.bv ||| mask.bv) = _
    rw [maskExact, offsetExact, indexExact]
    simp only [PackedInterpretations.insert, readBits]
  have cleared : raw (replace (value &&& ~~~mask)) =
      PackedSubsets.clear (raw words) atom.val := by
    rw [mappedUpdate]
    change (raw words).set index.val (value.bv &&& ~~~mask.bv) = _
    rw [maskExact, offsetExact, indexExact]
    simp only [PackedSubsets.clear, readBits]
  exact ⟨index, offset, mask, value, replace, divided, remainder, shifted,
    indexedTrait, tested, inserted, cleared⟩

/-- The actual mutable vector updater preserves every word and bit except the
selected bit, which the set branch makes true and the clear branch makes false.
Both returned vectors retain the original length. This follows from actual
operation results above and the existing shared packed representation laws;
it is not an independent reimplementation of their bit arithmetic. -/
theorem updates_preserve_other_bits (words : alloc.vec.Vec U64) (atom : Usize)
    (stored : atom.val / 64 < words.val.length) :
    ∃ index offset : Usize, ∃ mask value : U64, ∃ replace : U64 → alloc.vec.Vec U64,
      atom / 64#usize = ok index ∧
      atom % 64#usize = ok offset ∧
      (1#u64 <<< offset) = ok mask ∧
      alloc.vec.Vec.index_mut (core.slice.index.SliceIndexUsizeSlice U64) words index =
        ok (value, replace) ∧
      (replace (value ||| mask)).val.length = words.val.length ∧
      (replace (value &&& ~~~mask)).val.length = words.val.length ∧
      (∀ tested, PackedInterpretations.bit64 (raw (replace (value ||| mask))) tested =
        (PackedInterpretations.bit64 (raw words) tested || decide (tested = atom.val))) ∧
      (∀ tested, PackedInterpretations.bit64 (raw (replace (value &&& ~~~mask))) tested =
        (PackedInterpretations.bit64 (raw words) tested && decide (tested ≠ atom.val))) := by
  obtain ⟨index, offset, mask, value, replace, divided, remainder, shifted, indexed,
      _, inserted, cleared⟩ := word_operations words atom stored
  have rawStored : atom.val / 64 < (raw words).length := by simpa [raw] using stored
  obtain ⟨insertLength, insertBits⟩ := PackedInterpretations.insert_exact (raw words) atom.val rawStored
  obtain ⟨clearLength, clearBits⟩ := PackedSubsets.clear_exact (raw words) atom.val rawStored
  have setLength : (replace (value ||| mask)).val.length = words.val.length := by
    have same := (congrArg List.length inserted).trans insertLength
    simpa [raw] using same
  have unsetLength : (replace (value &&& ~~~mask)).val.length = words.val.length := by
    have same := (congrArg List.length cleared).trans clearLength
    simpa [raw] using same
  refine ⟨index, offset, mask, value, replace, divided, remainder, shifted, indexed,
    setLength, unsetLength, ?_, ?_⟩
  · simpa only [inserted] using insertBits
  · simpa only [cleared] using clearBits

/-- Setting a currently false positional bit admits checked machine increment
and gives exactly the population of the updated bit list. The vector-width bound
supplies machine room; population agreement alone would not justify addition. -/
theorem set_population (tail : List Bool) (present : Usize)
    (counted : present.val = Zetesis.SubsetCounter.population (false :: tail))
    (fits : (false :: tail).length ≤ Usize.max) :
    ∃ updated : Usize, present + 1#usize = ok updated ∧
      updated.val = Zetesis.SubsetCounter.population (true :: tail) := by
  have populationBound : Zetesis.SubsetCounter.population tail ≤ tail.length :=
    Zetesis.SubsetCounter.population_bound tail
  have incrementFits : present.val + (1#usize).val ≤ Usize.max := by
    simp only [Zetesis.SubsetCounter.population] at counted
    simp only [List.length_cons] at fits
    change present.val + 1 ≤ Usize.max
    omega
  obtain ⟨updated, added, incremented⟩ := WP.spec_imp_exists
    (Usize.add_spec (x := present) (y := 1#usize) incrementFits)
  refine ⟨updated, added, ?_⟩
  simpa [Zetesis.SubsetCounter.population, counted] using incremented

/-- Clearing a currently true positional bit admits checked machine decrement
and yields exactly the remaining population. The true bit's contribution proves
that the subtraction cannot underflow. -/
theorem clear_population (tail : List Bool) (present : Usize)
    (counted : present.val = Zetesis.SubsetCounter.population (true :: tail)) :
    ∃ updated : Usize, present - 1#usize = ok updated ∧
      updated.val = Zetesis.SubsetCounter.population (false :: tail) := by
  have subtractFits : (1#usize).val ≤ present.val := by
    simp only [Zetesis.SubsetCounter.population] at counted
    change 1 ≤ present.val
    omega
  obtain ⟨updated, subtracted, decremented, _⟩ := WP.spec_imp_exists
    (Usize.sub_spec (x := present) (y := 1#usize) subtractFits)
  refine ⟨updated, subtracted, ?_⟩
  simpa [Zetesis.SubsetCounter.population, counted] using decremented

end ScalarSubsets
