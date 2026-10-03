import Aeneas
import Zetesis.PackedInterpretations

open Aeneas Aeneas.Std Result

/-!
# Unsigned ceiling division at the constructor boundary

This authored external model states the trusted Rust standard-library contract
for `usize::div_ceil`. It is not an extraction of the standard-library body.
Zero divisors retain the backend division-by-zero failure. Otherwise the model
computes the quotient and possible remainder increment in natural numbers and
uses the backend's checked scalar constructor. In particular it introduces no
machine addition of padding that could overflow before division.

The word-count law connects the constructor's divisor 64 to the existing packed
representation. It holds for every represented unsigned input, including zero
and the largest host value; it does not assume an admitted theory or a successful
allocation. Runtime/library correspondence remains a trusted boundary.
-/

/-- The exact generated external signature, with division-by-zero refusal and
checked conversion of the mathematical ceiling to the host unsigned scalar. -/
@[rust_fun "core::num::{usize}::div_ceil"]
def core.num.Usize.div_ceil (value divisor : Usize) : Result Usize :=
  if divisor.val = 0 then fail .divisionByZero
  else UScalar.tryMk .Usize
    (value.val / divisor.val + if value.val % divisor.val = 0 then 0 else 1)

namespace UsizeCeiling

/-- A zero divisor fails explicitly, including when the dividend is zero. -/
theorem zero_divisor (value : Usize) :
    core.num.Usize.div_ceil value 0#usize = fail .divisionByZero := by
  simp [core.num.Usize.div_ceil]

/-- Dividing any host unsigned value upward by 64 returns exactly the packed
word count. No machine padding addition or extra bound on the input is needed.

Proof: quotient and remainder give the existing natural-number word count.
This count is no larger than the dividend, so its checked scalar conversion
cannot overflow. The result's numeric value is that same count. -/
theorem word_count64 (value : Usize) :
    ∃ count : Usize, core.num.Usize.div_ceil value 64#usize = ok count ∧
      count.val = Zetesis.Refinement.PackedInterpretations.count64 value.val := by
  have ceiling : value.val / 64 + (if value.val % 64 = 0 then 0 else 1) =
      Zetesis.Refinement.PackedInterpretations.count64 value.val := by
    unfold Zetesis.Refinement.PackedInterpretations.count64
    split_ifs <;> omega
  have bounded : Zetesis.Refinement.PackedInterpretations.count64 value.val ≤ value.val := by
    unfold Zetesis.Refinement.PackedInterpretations.count64
    omega
  have fits : UScalar.inBounds .Usize
      (Zetesis.Refinement.PackedInterpretations.count64 value.val) := by
    have inputBound := value.hBounds
    simp only [UScalar.inBounds] at *
    omega
  have checked : UScalar.check_bounds .Usize
      (Zetesis.Refinement.PackedInterpretations.count64 value.val) :=
    (UScalar.check_bounds_eq_inBounds _ _).mpr fits
  let count : Usize := UScalar.ofNatCore
    (Zetesis.Refinement.PackedInterpretations.count64 value.val)
    (UScalar.check_bounds_imp_inBounds checked)
  have returned : core.num.Usize.div_ceil value 64#usize = ok count := by
    change (if (64 : Nat) = 0 then _ else
      UScalar.tryMk .Usize (value.val / 64 + if value.val % 64 = 0 then 0 else 1)) = _
    rw [if_neg (by decide), ceiling]
    unfold UScalar.tryMk UScalar.tryMkOpt
    rw [dif_pos checked]
    rfl
  have exactValue : count.val =
      Zetesis.Refinement.PackedInterpretations.count64 value.val := by
    simp [count, UScalar.ofNatCore_val_eq]
  exact ⟨count, returned, exactValue⟩

end UsizeCeiling
