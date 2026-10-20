import Zetesis.ThresholdDivision
import Zetesis.ThresholdPropagation

/-!
# Necessary divisibility of a partially selected sum

A compatible complete reading consists of the known contribution plus a
selection from the undecided conditions. A common divisor of their coefficients
therefore divides every possible remainder. If that arithmetic progression
misses the required interval, no compatible reading satisfies the interval.

The condition formulas may be correlated. The progression contains all possible
sums; its points need not be realizable. These laws concern original truth and
do not replace the reduct. Grouping equal conditions, computing the divisor,
complement prefix/suffix reduction, finite-width arithmetic, processed-event snapshots and Rust queue execution
remain separate obligations.
-/

namespace Zetesis.ThresholdResidues

open Ferraris ThresholdReduct ThresholdPropagation

universe u v
variable {α : Type u} {β : Type v}

/-- Retain every occurrence whose condition is neither known true nor known
    false. Repeated positions keep their weights. -/
def remaining (lower upper : β → Bool) (inputs : List (Nat × β)) :
    List (Nat × β) :=
  inputs.filter (fun entry => upper entry.2 && !lower entry.2)

/-- Every compatible sum is the known contribution plus the selected undecided
    contribution. Induction distinguishes known true, known false and undecided
    conditions; compatibility fixes the complete reading in the first two cases. -/
theorem sum_decomposition (lower upper actual : β → Bool)
    (inputs : List (Nat × β)) (compatible : Compatible lower upper actual) :
    maskedSum actual inputs = maskedSum lower inputs +
      maskedSum actual (remaining lower upper inputs) := by
  induction inputs with
  | nil => simp [remaining, maskedSum, queries, Thresholds.enabledSum]
  | cons input rest ih =>
    obtain ⟨weight, index⟩ := input
    have reading (mask : β → Bool) (tail : List (Nat × β)) :
        maskedSum mask ((weight, index) :: tail) =
          (if mask index then weight else 0) + maskedSum mask tail := by
      rfl
    have lowerTruth : lower index = true → actual index = true := compatible.1 index
    have upperTruth : actual index = true → upper index = true := compatible.2 index
    cases low : lower index <;>
      cases high : upper index <;>
      cases truth : actual index <;>
      simp_all [remaining, Nat.add_assoc, Nat.add_left_comm]

/-- Dividing the undecided weights gives a witness for the complete sum's
    arithmetic progression. The divisor need not be greatest or positive: the
    weight-divisibility premise and exact sum decomposition suffice. -/
theorem sum_residue (lower upper actual : β → Bool)
    (inputs : List (Nat × β)) (divisor : Nat)
    (compatible : Compatible lower upper actual)
    (divides : ∀ entry ∈ remaining lower upper inputs, divisor ∣ entry.1) :
    maskedSum actual inputs = maskedSum lower inputs + divisor *
      maskedSum actual (ThresholdDivision.divide divisor (remaining lower upper inputs)) := by
  have decomposed : maskedSum actual inputs = maskedSum lower inputs +
      maskedSum actual (remaining lower upper inputs) :=
    sum_decomposition lower upper actual inputs compatible
  have scaled : maskedSum actual (remaining lower upper inputs) = divisor *
      maskedSum actual (ThresholdDivision.divide divisor (remaining lower upper inputs)) :=
    ThresholdDivision.masked_sum_divide divisor actual (remaining lower upper inputs) divides
  exact decomposed.trans (congrArg (fun value => maskedSum lower inputs + value) scaled)

/-- The common divisor divides every compatible remainder after subtracting
    the known contribution. The scaled undecided sum is the divisibility witness. -/
theorem residual_divisible (lower upper actual : β → Bool)
    (inputs : List (Nat × β)) (divisor : Nat)
    (compatible : Compatible lower upper actual)
    (divides : ∀ entry ∈ remaining lower upper inputs, divisor ∣ entry.1) :
    divisor ∣ maskedSum actual inputs - maskedSum lower inputs := by
  have residue : maskedSum actual inputs = maskedSum lower inputs + divisor *
      maskedSum actual (ThresholdDivision.divide divisor (remaining lower upper inputs)) :=
    sum_residue lower upper actual inputs divisor compatible divides
  refine ⟨maskedSum actual
    (ThresholdDivision.divide divisor (remaining lower upper inputs)), ?_⟩
  omega

/-- Refining a divisor cannot remove an existing interval intersection. If the
    refined divisor divides the earlier one, write the earlier divisor as their
    product. Each earlier progression point then has the same value at a scaled
    index in the refined progression. Positivity is unnecessary for this
    inclusion. A progression point need not be a realizable selected sum. -/
theorem interval_intersection_refines (base earlier refined low high : Nat)
    (divides : refined ∣ earlier)
    (intersects : ∃ multiple : Nat,
      low ≤ base + earlier * multiple ∧ base + earlier * multiple < high) :
    ∃ multiple : Nat,
      low ≤ base + refined * multiple ∧ base + refined * multiple < high := by
  obtain ⟨factor, factorization⟩ := divides
  obtain ⟨multiple, within⟩ := intersects
  have samePoint : base + earlier * multiple =
      base + refined * (factor * multiple) := by
    rw [factorization, Nat.mul_assoc]
  refine ⟨factor * multiple, ?_⟩
  simpa only [← samePoint] using within

/-- A nonempty interval whose width is at least a positive divisor contains
    a point of that divisor's progression above the known base.

    Divide the distance from the base to the lower endpoint. If its remainder
    is zero, the quotient reaches that endpoint; otherwise the next quotient
    reaches a point less than one divisor beyond it. The interval's width
    contains that point. This is a lattice witness, not an attainable sum. -/
theorem wide_interval_intersects (base divisor low high : Nat)
    (above : base ≤ low) (positive : 0 < divisor)
    (wide : divisor ≤ high - low) :
    ∃ multiple : Nat,
      low ≤ base + divisor * multiple ∧ base + divisor * multiple < high := by
  let distance := low - base
  have distanceEquation : base + distance = low := by
    dsimp [distance]
    omega
  have division : distance % divisor + divisor * (distance / divisor) = distance :=
    Nat.mod_add_div distance divisor
  have remainderBound : distance % divisor < divisor := Nat.mod_lt distance positive
  by_cases aligned : distance % divisor = 0
  · refine ⟨distance / divisor, ?_⟩
    omega
  · have nextPoint : divisor * (distance / divisor + 1) =
        divisor * (distance / divisor) + divisor := by
      simp only [Nat.mul_add, Nat.mul_one]
    refine ⟨distance / divisor + 1, ?_⟩
    omega

/-- An open coefficient fitting inside the interval proves that every positive
    divisor of it has a progression point in the interval. Reuse the coefficient
    witness, then refine the progression by divisibility. -/
theorem open_coefficient_intersects (base coefficient divisor low high : Nat)
    (above : base ≤ low) (positive : 0 < coefficient)
    (wide : coefficient ≤ high - low) (divides : divisor ∣ coefficient) :
    ∃ multiple : Nat,
      low ≤ base + divisor * multiple ∧ base + divisor * multiple < high := by
  have witness : ∃ multiple : Nat,
      low ≤ base + coefficient * multiple ∧ base + coefficient * multiple < high :=
    wide_interval_intersects base coefficient low high above positive wide
  exact interval_intersection_refines base coefficient divisor low high divides witness

/-- An interval containing no point of the residual progression cannot contain
    a compatible sum. The argument uses the exact witness above, not an assumption
    that undecided conditions can be chosen independently. The upper bound is
    exclusive, matching the conjunction of a required and a forbidden threshold. -/
theorem interval_refuted (lower upper actual : β → Bool)
    (inputs : List (Nat × β)) (divisor low high : Nat)
    (compatible : Compatible lower upper actual)
    (divides : ∀ entry ∈ remaining lower upper inputs, divisor ∣ entry.1)
    (excluded : ∀ remainder : Nat,
      ¬ (low ≤ maskedSum lower inputs + divisor * remainder ∧
        maskedSum lower inputs + divisor * remainder < high)) :
    ¬ (low ≤ maskedSum actual inputs ∧ maskedSum actual inputs < high) := by
  have residue : maskedSum actual inputs = maskedSum lower inputs + divisor *
      maskedSum actual (ThresholdDivision.divide divisor (remaining lower upper inputs)) :=
    sum_residue lower upper actual inputs divisor compatible divides
  intro within
  have progressionPoint : ¬ (low ≤ maskedSum lower inputs + divisor *
      maskedSum actual (ThresholdDivision.divide divisor (remaining lower upper inputs)) ∧
      maskedSum lower inputs + divisor *
        maskedSum actual (ThresholdDivision.divide divisor (remaining lower upper inputs)) < high) :=
    excluded (maskedSum actual (ThresholdDivision.divide divisor (remaining lower upper inputs)))
  exact progressionPoint (by simpa only [← residue] using within)

/-- The arithmetic exclusion rules out satisfaction of a lower threshold
    together with failure of an upper threshold, for arbitrary condition formulas.
    This is a necessary original-truth restriction, not an answer-set test. -/
theorem formula_interval_refuted (M : Atoms α) (conditions : β → Formula α)
    (lower upper : β → Bool) (inputs : List (Nat × β)) (divisor low high : Nat)
    (compatible : Compatible lower upper (active M conditions))
    (divides : ∀ entry ∈ remaining lower upper inputs, divisor ∣ entry.1)
    (excluded : ∀ remainder : Nat,
      ¬ (low ≤ maskedSum lower inputs + divisor * remainder ∧
        maskedSum lower inputs + divisor * remainder < high)) :
    ¬ (Satisfies M (formula conditions inputs low) ∧
      ¬ Satisfies M (formula conditions inputs high)) := by
  intro satisfied
  have below : low ≤ maskedSum (active M conditions) inputs :=
    (ThresholdReduct.original M conditions inputs low).mp satisfied.1
  have above : maskedSum (active M conditions) inputs < high := by
    have absent : ¬ high ≤ maskedSum (active M conditions) inputs := by
      intro reached
      exact satisfied.2 ((ThresholdReduct.original M conditions inputs high).mpr reached)
    omega
  exact interval_refuted lower upper (active M conditions) inputs divisor low high
    compatible divides excluded ⟨below, above⟩

/-- Intersect the necessary progression with both the required interval and the
    upper reading. Every compatible sum obeys that finite upper bound, so a
    progression point outside it cannot rescue a branch. This strengthens the
    exclusion premise available to a consumer; it does not assert attainability.

    First obtain the exact residue witness. Then enclose the actual sum by the
    upper mask and contradict the excluded bounded progression point. -/
theorem bounded_interval_refuted (lower upper actual : β → Bool)
    (inputs : List (Nat × β)) (divisor low high : Nat)
    (compatible : Compatible lower upper actual)
    (divides : ∀ entry ∈ remaining lower upper inputs, divisor ∣ entry.1)
    (excluded : ∀ remainder : Nat,
      ¬ (low ≤ maskedSum lower inputs + divisor * remainder ∧
        maskedSum lower inputs + divisor * remainder < high ∧
        maskedSum lower inputs + divisor * remainder ≤ maskedSum upper inputs)) :
    ¬ (low ≤ maskedSum actual inputs ∧ maskedSum actual inputs < high) := by
  have residue : maskedSum actual inputs = maskedSum lower inputs + divisor *
      maskedSum actual (ThresholdDivision.divide divisor (remaining lower upper inputs)) :=
    sum_residue lower upper actual inputs divisor compatible divides
  have bounded : maskedSum actual inputs ≤ maskedSum upper inputs :=
    (sum_bounds lower upper actual inputs compatible).2
  intro within
  have excludedPoint : ¬ (low ≤ maskedSum lower inputs + divisor *
      maskedSum actual (ThresholdDivision.divide divisor (remaining lower upper inputs)) ∧
      maskedSum lower inputs + divisor *
        maskedSum actual (ThresholdDivision.divide divisor (remaining lower upper inputs)) < high ∧
      maskedSum lower inputs + divisor *
        maskedSum actual (ThresholdDivision.divide divisor (remaining lower upper inputs)) ≤
          maskedSum upper inputs) :=
    excluded (maskedSum actual (ThresholdDivision.divide divisor (remaining lower upper inputs)))
  have enclosedInterval : low ≤ maskedSum actual inputs ∧
      maskedSum actual inputs < high ∧ maskedSum actual inputs ≤ maskedSum upper inputs :=
    ⟨within.1, within.2, bounded⟩
  exact excludedPoint (by simpa only [← residue] using enclosedInterval)

/-- If switching one condition off leaves no bounded progression point in the
    required interval, every satisfying compatible reading makes it true.
    Assigning that condition affects all its occurrences; the complementary
    coefficients may still describe correlated formulas.

    Assume the condition false, strengthen the upper reading with that fact,
    and apply the residual exclusion to the resulting compatible masks. -/
theorem condition_required_true_by_residue [DecidableEq β]
    (lower upper actual : β → Bool) (inputs : List (Nat × β))
    (condition : β) (divisor low high : Nat)
    (compatible : Compatible lower upper actual)
    (within : low ≤ maskedSum actual inputs ∧ maskedSum actual inputs < high)
    (divides : ∀ entry ∈ remaining lower (assign upper condition false) inputs,
      divisor ∣ entry.1)
    (excluded : ∀ remainder : Nat,
      ¬ (low ≤ maskedSum lower inputs + divisor * remainder ∧
        maskedSum lower inputs + divisor * remainder < high ∧
        maskedSum lower inputs + divisor * remainder ≤
          maskedSum (assign upper condition false) inputs)) :
    actual condition = true := by
  cases truth : actual condition with
  | true => rfl
  | false =>
    have enclosed : ∀ index, actual index = true →
        assign upper condition false index = true := by
      intro index enabled
      by_cases same : index = condition
      · subst index
        simp [truth] at enabled
      · simpa [assign, same] using compatible.2 index enabled
    have branchCompatible : Compatible lower (assign upper condition false) actual :=
      ⟨compatible.1, enclosed⟩
    have impossible : ¬ (low ≤ maskedSum actual inputs ∧ maskedSum actual inputs < high) :=
      bounded_interval_refuted lower (assign upper condition false) actual inputs
        divisor low high branchCompatible divides excluded
    exact False.elim (impossible within)

/-- If switching one condition on leaves no bounded progression point in the
    required interval, every satisfying compatible reading makes it false.

    Assuming the condition true strengthens the lower reading. Its complete
    grouped contribution becomes part of the base; the remaining coefficients
    supply the complementary progression. The same exclusion law then applies. -/
theorem condition_required_false_by_residue [DecidableEq β]
    (lower upper actual : β → Bool) (inputs : List (Nat × β))
    (condition : β) (divisor low high : Nat)
    (compatible : Compatible lower upper actual)
    (within : low ≤ maskedSum actual inputs ∧ maskedSum actual inputs < high)
    (divides : ∀ entry ∈ remaining (assign lower condition true) upper inputs,
      divisor ∣ entry.1)
    (excluded : ∀ remainder : Nat,
      ¬ (low ≤ maskedSum (assign lower condition true) inputs + divisor * remainder ∧
        maskedSum (assign lower condition true) inputs + divisor * remainder < high ∧
        maskedSum (assign lower condition true) inputs + divisor * remainder ≤
          maskedSum upper inputs)) :
    actual condition = false := by
  cases truth : actual condition with
  | false => rfl
  | true =>
    have enclosed : ∀ index, assign lower condition true index = true →
        actual index = true := by
      intro index enabled
      by_cases same : index = condition
      · subst index
        exact truth
      · exact compatible.1 index (by simpa [assign, same] using enabled)
    have branchCompatible : Compatible (assign lower condition true) upper actual :=
      ⟨enclosed, compatible.2⟩
    have impossible : ¬ (low ≤ maskedSum actual inputs ∧ maskedSum actual inputs < high) :=
      bounded_interval_refuted (assign lower condition true) upper actual inputs
        divisor low high branchCompatible divides excluded
    exact False.elim (impossible within)

end Zetesis.ThresholdResidues
