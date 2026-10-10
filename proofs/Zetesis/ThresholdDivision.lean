import Zetesis.ThresholdReduct

/-!
# Dividing nonnegative threshold weights by a common divisor

Dividing every weight by the same positive divisor preserves threshold truth
when the divisor divides every weight and the threshold is rounded upward.
The selected sum scales by that divisor for every mask. This arithmetic law
preserves both original satisfaction and frozen-reduct satisfaction for arbitrary
interpretations M and J, including arbitrary formulas used as conditions.

Zero weights, input order and repeated positions are retained. Computing a common
divisor, signed bounds, finite-width arithmetic, shared storage, resource costs
and correspondence with executable normalization remain separate obligations.
-/

namespace Zetesis.ThresholdDivision

open Ferraris ThresholdReduct

universe u v
variable {α : Type u} {β : Type v}

/-- Divide every weight while preserving the condition and its position. -/
def divide (divisor : Nat) (inputs : List (Nat × β)) : List (Nat × β) :=
  inputs.map (fun (weight, index) => (weight / divisor, index))

/-- A zero threshold remains zero. For a positive divisor, a positive threshold
    uses upward division without first adding the divisor to the threshold. -/
def roundedBound (divisor bound : Nat) : Nat :=
  if bound = 0 then 0 else 1 + (bound - 1) / divisor

/-- A common divisor of every positive weight also divides the zero weights.
    This connects the positive-weight formulation to the uniform row premise. -/
theorem divides_all (divisor : Nat) (inputs : List (Nat × β))
    (dividesPositive : ∀ entry ∈ inputs, 0 < entry.1 → divisor ∣ entry.1) :
    ∀ entry ∈ inputs, divisor ∣ entry.1 := by
  intro entry member
  by_cases zero : entry.1 = 0
  · simpa only [zero] using Nat.dvd_zero divisor
  · exact dividesPositive entry member (Nat.pos_of_ne_zero zero)

/-- The rounded threshold is reached exactly when the original threshold is
    reached after multiplying the selected sum by the positive divisor.
    For a positive bound, strict comparison of the predecessor converts natural
    division's floor to the required ceiling. -/
theorem rounded_bound_le_iff (divisor bound total : Nat)
    (positive : 0 < divisor) :
    roundedBound divisor bound ≤ total ↔ bound ≤ divisor * total := by
  by_cases zero : bound = 0
  · simp [roundedBound, zero]
  · have positiveBound : 0 < bound := Nat.pos_of_ne_zero zero
    simp only [roundedBound, zero, ↓reduceIte]
    calc
      1 + (bound - 1) / divisor ≤ total ↔ (bound - 1) / divisor < total := by
        omega
      _ ↔ bound - 1 < total * divisor := Nat.div_lt_iff_lt_mul positive
      _ ↔ bound ≤ total * divisor := by omega
      _ ↔ bound ≤ divisor * total := by rw [Nat.mul_comm divisor total]

/-- Every selected sum is the common divisor times the sum of the divided
    row. Induction accounts for each position separately; equal condition IDs
    therefore keep all their contributions. The equality itself needs only
    divisibility, so positivity is left to the threshold comparison theorem. -/
theorem masked_sum_divide (divisor : Nat) (mask : β → Bool)
    (inputs : List (Nat × β))
    (divides : ∀ entry ∈ inputs, divisor ∣ entry.1) :
    maskedSum mask inputs = divisor * maskedSum mask (divide divisor inputs) := by
  revert divides
  induction inputs with
  | nil =>
    intro _
    simp [divide, maskedSum, queries, Thresholds.enabledSum]
  | cons entry rest ih =>
    intro divides
    obtain ⟨weight, index⟩ := entry
    have headDivides : divisor ∣ weight := divides (weight, index) (by simp)
    have tailDivides : ∀ entry ∈ rest, divisor ∣ entry.1 := by
      intro entry member
      exact divides entry (List.mem_cons_of_mem (weight, index) member)
    have tailSum : maskedSum mask rest =
        divisor * maskedSum mask (divide divisor rest) := ih tailDivides
    have headSum : (if mask index then weight else 0) =
        divisor * (if mask index then weight / divisor else 0) := by
      cases mask index <;> simp [Nat.mul_div_cancel' headDivides]
    change (if mask index then weight else 0) + maskedSum mask rest =
      divisor * ((if mask index then weight / divisor else 0) +
        maskedSum mask (divide divisor rest))
    rw [headSum, tailSum, Nat.mul_add]

/-- Dividing a row by a positive common divisor and rounding the threshold
    upward preserves its reading under every mask. -/
theorem masked_threshold_divide (divisor bound : Nat) (mask : β → Bool)
    (inputs : List (Nat × β)) (positive : 0 < divisor)
    (divides : ∀ entry ∈ inputs, divisor ∣ entry.1) :
    bound ≤ maskedSum mask inputs ↔
      roundedBound divisor bound ≤ maskedSum mask (divide divisor inputs) := by
  rw [masked_sum_divide divisor mask inputs divides]
  exact (rounded_bound_le_iff divisor bound
    (maskedSum mask (divide divisor inputs)) positive).symm

/-- A threshold formula and its divided form have the same original truth.
    The conditions may be arbitrary formulas; the arithmetic law applies to
    the mask formed by their satisfaction in the supplied interpretation. -/
theorem original (M : Atoms α) (conditions : β → Formula α)
    (inputs : List (Nat × β)) (divisor bound : Nat) (positive : 0 < divisor)
    (divides : ∀ entry ∈ inputs, divisor ∣ entry.1) :
    Satisfies M (formula conditions inputs bound) ↔
      Satisfies M (formula conditions (divide divisor inputs)
        (roundedBound divisor bound)) := by
  have sameReading : bound ≤ maskedSum (active M conditions) inputs ↔
      roundedBound divisor bound ≤
        maskedSum (active M conditions) (divide divisor inputs) :=
    masked_threshold_divide divisor bound (active M conditions) inputs positive divides
  exact (ThresholdReduct.original M conditions inputs bound).trans
    (sameReading.trans (ThresholdReduct.original M conditions
      (divide divisor inputs) (roundedBound divisor bound)).symm)

/-- The divided formula has the same frozen truth for arbitrary M and J.
    The established reduct law has two explicit gates: the threshold on original
    condition truth and the threshold on recursively frozen condition truth.
    The masked arithmetic equivalence preserves each gate independently. No
    subset relation or original-model premise is required. -/
theorem frozen (M J : Atoms α) (conditions : β → Formula α)
    (inputs : List (Nat × β)) (divisor bound : Nat) (positive : 0 < divisor)
    (divides : ∀ entry ∈ inputs, divisor ∣ entry.1) :
    Satisfies J (Reduct M (formula conditions inputs bound)) ↔
      Satisfies J (Reduct M (formula conditions (divide divisor inputs)
        (roundedBound divisor bound))) := by
  have sameOriginal : bound ≤ maskedSum (active M conditions) inputs ↔
      roundedBound divisor bound ≤
        maskedSum (active M conditions) (divide divisor inputs) :=
    masked_threshold_divide divisor bound (active M conditions) inputs positive divides
  have sameFrozen :
      bound ≤ maskedSum (active J (fun index => Reduct M (conditions index))) inputs ↔
      roundedBound divisor bound ≤
        maskedSum (active J (fun index => Reduct M (conditions index)))
          (divide divisor inputs) :=
    masked_threshold_divide divisor bound
      (active J (fun index => Reduct M (conditions index))) inputs positive divides
  exact (ThresholdReduct.frozen M J conditions inputs bound).trans
    ((and_congr sameOriginal sameFrozen).trans
      (ThresholdReduct.frozen M J conditions (divide divisor inputs)
        (roundedBound divisor bound)).symm)

/-- Package both laws in the existing equivalence relation, so the established
    connective and substitution laws can carry this rewrite into a context. -/
theorem equivalent (conditions : β → Formula α) (inputs : List (Nat × β))
    (divisor bound : Nat) (positive : 0 < divisor)
    (divides : ∀ entry ∈ inputs, divisor ∣ entry.1) :
    ChoiceIntervals.Equivalent (formula conditions inputs bound)
      (formula conditions (divide divisor inputs) (roundedBound divisor bound)) := by
  exact ⟨fun M => original M conditions inputs divisor bound positive divides,
    fun M J => frozen M J conditions inputs divisor bound positive divides⟩

end Zetesis.ThresholdDivision
