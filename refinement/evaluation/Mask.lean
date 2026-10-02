import Init.Data.BitVec.Lemmas

namespace Membership

/-- A one-bit mask is nonzero exactly when the selected stored bit is true. -/
theorem mask_bit (word : BitVec 64) (offset : Nat) (bound : offset < 64) :
    (word &&& (1#64 <<< offset) != 0#64) = word.getLsbD offset := by
  rw [← BitVec.twoPow_eq, BitVec.and_twoPow]
  have nonzero : BitVec.twoPow 64 offset ≠ 0#64 := by
    intro zero
    have bit := congrArg (fun value : BitVec 64 => value.getLsbD offset) zero
    simp [bound] at bit
  cases bit : word.getLsbD offset <;> simp [nonzero]

end Membership
