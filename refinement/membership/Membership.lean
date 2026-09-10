import Harness.Funs
import Mask

open Aeneas Aeneas.Std Result
open ZetesisExtract

namespace Membership

abbrev Interpretation := zetesis_ferraris.theory.Interpretation

/-- Every atom in the declared universe has a stored 64-bit word. Padding bits
are immaterial because the source checks the universe before indexing. -/
def Represented (candidate : Interpretation) : Prop :=
  ∀ atom : Nat, atom < candidate.theory.value.atoms.val →
    atom / 64 < candidate.words.val.length

/-- Packed membership selects a word and then its least-significant-bit index;
atoms outside the declared universe are absent. -/
def denotes (candidate : Interpretation) (atom : Nat) : Bool :=
  decide (atom < candidate.theory.value.atoms.val) &&
    (candidate.words.val[atom / 64]?.map (fun word =>
      word.bv.getLsbD (atom % 64))).getD false

/-- Out-of-universe membership returns false before inspecting storage. -/
theorem outside (candidate : Interpretation) (atom : Usize)
    (outside : candidate.theory.value.atoms.val ≤ atom.val) :
    zetesis_ferraris.theory.Interpretation.contains candidate atom = ok false := by
  simp [zetesis_ferraris.theory.Interpretation.contains,
    zetesis_ferraris.theory.Theory.atom_count,
    alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, UScalar.lt_equiv,
    Nat.not_lt.mpr outside]

/-- Under the storage invariant, the extracted query returns exactly the
packed bit. The proof establishes division and indexing bounds, then applies
the one-bit mask law; it does not assume membership correctness. -/
theorem contains_refines (candidate : Interpretation) (atom : Usize)
    (represented : Represented candidate) :
    zetesis_ferraris.theory.Interpretation.contains candidate atom =
      ok (denotes candidate atom.val) := by
  by_cases inside : atom.val < candidate.theory.value.atoms.val
  · obtain ⟨quotient, division, quotient_value, _⟩ :=
      UScalar.div_bv_spec atom (y := 64#usize) (by simp)
    have index_bound : quotient.val < candidate.words.length := by
      simpa only [show quotient.val = atom.val / 64 by simpa using quotient_value] using (represented atom.val inside :
        atom.val / 64 < candidate.words.val.length)
    obtain ⟨word, indexing, word_value⟩ := WP.spec_imp_exists
      (alloc.vec.Vec.index_usize_spec candidate.words quotient index_bound)
    obtain ⟨remainder, modulus, remainder_value⟩ := WP.spec_imp_exists
      (UScalar.rem_spec atom (y := 64#usize) (by simp))
    have shift_bound : remainder.val < 64 := by
      simpa only [show remainder.val = atom.val % 64 by simpa using remainder_value] using (Nat.mod_lt atom.val (by decide : 0 < 64))
    obtain ⟨mask, shifting, _, mask_value⟩ := WP.spec_imp_exists
      (U64.ShiftLeft_spec (1#u64) remainder shift_bound)
    simp [zetesis_ferraris.theory.Interpretation.contains,
      zetesis_ferraris.theory.Theory.atom_count,
      alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
      UScalar.lt_equiv, inside, division,
      alloc.vec.Vec.index_slice_index, indexing, modulus, shifting, lift]
    change (word.bv &&& mask.bv != 0#64) = denotes candidate atom.val
    rw [mask_value]
    change (word.bv &&& (1#64 <<< remainder.val) != 0#64) = _
    rw [mask_bit word.bv remainder.val shift_bound]
    have stored := represented atom.val inside
    simp [denotes, inside, word_value, quotient_value, remainder_value,
      List.getElem?_eq_getElem stored]
  · rw [outside candidate atom (Nat.le_of_not_gt inside)]
    simp [denotes, inside]

end Membership
