import Zetesis.IntegerEnvelopes
import Zetesis.ScalarArithmetic

/-!
# Signed remainder over a divisor interval

The envelope intersects the dividend-to-zero interval with a bound on the
divisor magnitude. It applies to positive and negative operands.
The first law is independent of machine integers. The final law additionally
uses the scalar evaluator's fault policy and the accepted output endpoints.
Neither law establishes the Rust/Lean correspondence or that source analysis
supplies complete numeric operand domains.
-/

namespace Zetesis.NumericDomains

/-- Every integer between two endpoints has magnitude no greater than the
larger endpoint magnitude. This also holds for intervals crossing zero. -/
theorem endpoint_magnitude (value lower upper : Int)
    (inside : lower ≤ value ∧ value ≤ upper) :
    (value.natAbs : Int) ≤ max (lower.natAbs : Int) (upper.natAbs : Int) := by
  have left_limit : (lower.natAbs : Int) ≤
      max (lower.natAbs : Int) (upper.natAbs : Int) :=
    Int.le_max_left _ _
  have right_limit : (upper.natAbs : Int) ≤
      max (lower.natAbs : Int) (upper.natAbs : Int) :=
    Int.le_max_right _ _
  have reflected_lower : -lower ≤ (lower.natAbs : Int) := by
    have magnitude : -lower ≤ ((-lower).natAbs : Int) := Int.le_natAbs
    simpa only [Int.natAbs_neg] using magnitude
  have upper_magnitude : upper ≤ (upper.natAbs : Int) := Int.le_natAbs
  by_cases nonnegative : 0 ≤ value
  · have magnitude : (value.natAbs : Int) = value := Int.natAbs_of_nonneg nonnegative
    omega
  · have magnitude : (value.natAbs : Int) = -value :=
      Int.ofNat_natAbs_of_nonpos (show value ≤ 0 by omega)
    omega

/-- Truncating remainder lies between the dividend and zero, whichever is
smaller. The divisor may have either sign; zero is harmless for this purely
mathematical bound, though the checked scalar evaluator refuses it.

The remainder has the dividend's weak sign and no greater magnitude. For a
negative dividend, reflection reduces its sign to the nonnegative case. -/
theorem remainder_dividend_envelope (dividend divisor : Int) :
    min dividend 0 ≤ dividend.tmod divisor ∧
      dividend.tmod divisor ≤ max dividend 0 := by
  have magnitude : (dividend.tmod divisor).natAbs ≤ dividend.natAbs := by
    rw [Int.natAbs_tmod]
    exact Nat.mod_le _ _
  have magnitude_cast : ((dividend.tmod divisor).natAbs : Int) ≤
      (dividend.natAbs : Int) := Int.ofNat_le.mpr magnitude
  by_cases nonnegative : 0 ≤ dividend
  · have remainder_nonnegative : 0 ≤ dividend.tmod divisor :=
      Int.tmod_nonneg divisor nonnegative
    have dividend_magnitude : (dividend.natAbs : Int) = dividend :=
      Int.natAbs_of_nonneg nonnegative
    have remainder_magnitude : ((dividend.tmod divisor).natAbs : Int) =
        dividend.tmod divisor := Int.natAbs_of_nonneg remainder_nonnegative
    have below_dividend : dividend.tmod divisor ≤ dividend := by omega
    exact ⟨Int.le_trans (Int.min_le_right _ _) remainder_nonnegative,
      Int.le_trans below_dividend (Int.le_max_left _ _)⟩
  · have dividend_nonpositive : dividend ≤ 0 := by omega
    have reflected_nonnegative : 0 ≤ (-dividend).tmod divisor :=
      Int.tmod_nonneg divisor (show 0 ≤ -dividend by omega)
    have remainder_nonpositive : dividend.tmod divisor ≤ 0 := by
      rw [Int.neg_tmod] at reflected_nonnegative
      omega
    have dividend_magnitude : (dividend.natAbs : Int) = -dividend :=
      Int.ofNat_natAbs_of_nonpos dividend_nonpositive
    have remainder_magnitude : ((dividend.tmod divisor).natAbs : Int) =
        -(dividend.tmod divisor) := Int.ofNat_natAbs_of_nonpos remainder_nonpositive
    have above_dividend : dividend ≤ dividend.tmod divisor := by omega
    exact ⟨Int.le_trans (Int.min_le_left _ _) above_dividend,
      Int.le_trans remainder_nonpositive (Int.le_max_right _ _)⟩

/-- Largest possible strict remainder magnitude for an endpoint-bounded divisor. -/
def remainderMagnitude (lowerDivisor upperDivisor : Int) : Int :=
  max (lowerDivisor.natAbs : Int) (upperDivisor.natAbs : Int) - 1

/-- The transfer clips the negative magnitude by the dividend's lower bound and zero. -/
def remainderLower (lower lowerDivisor upperDivisor : Int) : Int :=
  max (min lower 0) (-(remainderMagnitude lowerDivisor upperDivisor))

/-- The transfer clips the positive magnitude by the dividend's upper bound and zero. -/
def remainderUpper (upper lowerDivisor upperDivisor : Int) : Int :=
  min (max upper 0) (remainderMagnitude lowerDivisor upperDivisor)

/-- A divisor interval excluding zero bounds every signed truncating remainder
by the exact clipped interval used by the arithmetic transfer. Operands may be
correlated; every admitted pair is covered without asserting independence.

First bound the actual divisor's magnitude by its endpoints. Combine the
existing fixed-divisor remainder bound with the dividend-to-zero envelope,
then intersect these two enclosing intervals. -/
theorem remainder_interval (dividend divisor lower upper lowerDivisor upperDivisor : Int)
    (left : lower ≤ dividend ∧ dividend ≤ upper)
    (right : lowerDivisor ≤ divisor ∧ divisor ≤ upperDivisor)
    (zero_excluded : ¬ (lowerDivisor ≤ 0 ∧ 0 ≤ upperDivisor)) :
    remainderLower lower lowerDivisor upperDivisor ≤ dividend.tmod divisor ∧
      dividend.tmod divisor ≤ remainderUpper upper lowerDivisor upperDivisor := by
  have nonzero : divisor ≠ 0 := by
    intro zero
    apply zero_excluded
    omega
  have divisor_bound : (divisor.natAbs : Int) ≤
      max (lowerDivisor.natAbs : Int) (upperDivisor.natAbs : Int) :=
    endpoint_magnitude divisor lowerDivisor upperDivisor right
  have actual_magnitude :=
    Zetesis.IntegerEnvelopes.remainder_magnitude dividend divisor nonzero
  have bounded_magnitude :
      -(remainderMagnitude lowerDivisor upperDivisor) ≤ dividend.tmod divisor ∧
      dividend.tmod divisor ≤ remainderMagnitude lowerDivisor upperDivisor := by
    unfold remainderMagnitude
    omega
  have signed := remainder_dividend_envelope dividend divisor
  have lower_endpoint : min lower 0 ≤ min dividend 0 :=
    Int.le_min.mpr ⟨Int.le_trans (Int.min_le_left _ _) left.1, Int.min_le_right _ _⟩
  have upper_endpoint : max dividend 0 ≤ max upper 0 :=
    Int.max_le.mpr ⟨Int.le_trans left.2 (Int.le_max_left _ _), Int.le_max_right _ _⟩
  have bounded_sign : min lower 0 ≤ dividend.tmod divisor ∧
      dividend.tmod divisor ≤ max upper 0 :=
    ⟨Int.le_trans lower_endpoint signed.1, Int.le_trans signed.2 upper_endpoint⟩
  exact ⟨Int.max_le.mpr ⟨bounded_sign.1, bounded_magnitude.1⟩,
    Int.le_min.mpr ⟨bounded_sign.2, bounded_magnitude.2⟩⟩

/-- A transfer accepted by the exact zero/exception guards and by the output
range check has no scalar fault for any operand pair in its input intervals.
The dividend's lower endpoint is at least i32::MIN; this is the input Range
invariant needed to lift the endpoint exception test to the actual dividend.

The machine-fault claim is separate from the mathematical envelope: MIN % -1
has mathematical remainder zero but remains a scalar overflow refusal. The
accepted output endpoints establish representability, and division_success
then applies the already-proved scalar fault policy. -/
theorem checked_remainder_interval
    (dividend divisor lower upper lowerDivisor upperDivisor : Int)
    (left : lower ≤ dividend ∧ dividend ≤ upper)
    (right : lowerDivisor ≤ divisor ∧ divisor ≤ upperDivisor)
    (zero_excluded : ¬ (lowerDivisor ≤ 0 ∧ 0 ≤ upperDivisor))
    (minimum_bound : -2147483648 ≤ lower)
    (exception_excluded :
      ¬ (lower = -2147483648 ∧ lowerDivisor ≤ -1 ∧ -1 ≤ upperDivisor))
    (accepted_lower :
      Zetesis.ScalarArithmetic.InRange (remainderLower lower lowerDivisor upperDivisor))
    (accepted_upper :
      Zetesis.ScalarArithmetic.InRange (remainderUpper upper lowerDivisor upperDivisor)) :
    Zetesis.ScalarArithmetic.divisionResult dividend divisor (dividend.tmod divisor) =
        .ok (dividend.tmod divisor) ∧
      remainderLower lower lowerDivisor upperDivisor ≤ dividend.tmod divisor ∧
      dividend.tmod divisor ≤ remainderUpper upper lowerDivisor upperDivisor := by
  have enclosure :=
    remainder_interval dividend divisor lower upper lowerDivisor upperDivisor
      left right zero_excluded
  have nonzero : divisor ≠ 0 := by
    intro zero
    apply zero_excluded
    omega
  have ordinary : ¬ (dividend = -2147483648 ∧ divisor = -1) := by
    intro exceptional
    apply exception_excluded
    omega
  have representable : Zetesis.ScalarArithmetic.InRange (dividend.tmod divisor) := by
    unfold Zetesis.ScalarArithmetic.InRange at accepted_lower accepted_upper ⊢
    omega
  have success :=
    Zetesis.ScalarArithmetic.division_success dividend divisor (dividend.tmod divisor)
      nonzero ordinary representable
  exact ⟨success, enclosure⟩

end Zetesis.NumericDomains
