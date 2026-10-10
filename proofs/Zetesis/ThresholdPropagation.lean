import Zetesis.ThresholdReduct

/-!
# Necessary consequences of nonnegative threshold bounds

A lower mask marks conditions already known true. An upper mask marks conditions
not yet known false. Every compatible complete interpretation has a weighted sum
between these two readings. Requiring a threshold can therefore force a condition
when its removal makes even the upper reading insufficient; forbidding a threshold
can force a condition false when adding it makes the lower reading sufficient.

The sums retain every input position. Assigning one condition changes all its
occurrences, so repeated conditions use the sum of their weights. These laws are
necessary consequences, not a claim that interval propagation discovers every
consequence of correlated conditions. They do not establish queue discipline,
finite-width counters, source tuple coalescing or implementation refinement.
-/

namespace Zetesis.ThresholdPropagation

open ThresholdReduct Ferraris

universe u v
variable {α : Type u} {β : Type v}

/-- The actual condition truth lies between the lower and upper readings. -/
def Compatible (lower upper actual : β → Bool) : Prop :=
  (∀ index, lower index = true → actual index = true) ∧
  (∀ index, actual index = true → upper index = true)

/-- Enabling additional conditions cannot decrease their nonnegative sum.
    Induction preserves each position, including repeated condition IDs. -/
theorem masked_sum_mono (lower upper : β → Bool) (inputs : List (Nat × β))
    (included : ∀ index, lower index = true → upper index = true) :
    maskedSum lower inputs ≤ maskedSum upper inputs := by
  induction inputs with
  | nil => simp [maskedSum, queries, Thresholds.enabledSum]
  | cons input rest ih =>
    obtain ⟨weight, index⟩ := input
    have head : (if lower index then weight else 0) ≤
        (if upper index then weight else 0) := by
      cases truth : lower index with
      | false => simp
      | true =>
        have enabled : upper index = true := included index truth
        simp [enabled]
    change (if lower index then weight else 0) + maskedSum lower rest ≤
      (if upper index then weight else 0) + maskedSum upper rest
    exact Nat.add_le_add head ih

/-- Both partial readings enclose every compatible complete sum. -/
theorem sum_bounds (lower upper actual : β → Bool) (inputs : List (Nat × β))
    (compatible : Compatible lower upper actual) :
    maskedSum lower inputs ≤ maskedSum actual inputs ∧
      maskedSum actual inputs ≤ maskedSum upper inputs := by
  exact ⟨masked_sum_mono lower actual inputs compatible.1,
    masked_sum_mono actual upper inputs compatible.2⟩

/-- A sufficient lower sum guarantees the threshold in every compatible reading. -/
theorem threshold_of_lower (lower upper actual : β → Bool)
    (inputs : List (Nat × β)) (bound : Nat)
    (compatible : Compatible lower upper actual)
    (sufficient : bound ≤ maskedSum lower inputs) :
    bound ≤ maskedSum actual inputs := by
  exact Nat.le_trans sufficient (sum_bounds lower upper actual inputs compatible).1

/-- An insufficient upper sum rules out the threshold in every compatible reading. -/
theorem threshold_absent_of_upper (lower upper actual : β → Bool)
    (inputs : List (Nat × β)) (bound : Nat)
    (compatible : Compatible lower upper actual)
    (insufficient : maskedSum upper inputs < bound) :
    maskedSum actual inputs < bound := by
  exact Nat.lt_of_le_of_lt (sum_bounds lower upper actual inputs compatible).2 insufficient

/-- Assign a condition consistently at every position where it occurs. -/
def assign [DecidableEq β] (mask : β → Bool) (condition : β) (truth : Bool) : β → Bool :=
  fun index => if index = condition then truth else mask index

/-- All weights attached to one condition, preserving the multiplicity of positions. -/
def coefficient [DecidableEq β] (condition : β) : List (Nat × β) → Nat
  | [] => 0
  | (weight, index) :: rest =>
      (if index = condition then weight else 0) + coefficient condition rest

/-- Turning a condition on adds its complete grouped coefficient to the reading
    where it is off. Equal condition IDs never collapse their distinct weights. -/
theorem assigned_sum [DecidableEq β] (mask : β → Bool) (condition : β)
    (inputs : List (Nat × β)) :
    maskedSum (assign mask condition true) inputs = coefficient condition inputs +
      maskedSum (assign mask condition false) inputs := by
  induction inputs with
  | nil => simp [maskedSum, queries, Thresholds.enabledSum, coefficient]
  | cons input rest ih =>
    obtain ⟨weight, index⟩ := input
    change (if assign mask condition true index then weight else 0) +
      maskedSum (assign mask condition true) rest =
      (if index = condition then weight else 0) + coefficient condition rest +
        ((if assign mask condition false index then weight else 0) +
          maskedSum (assign mask condition false) rest)
    by_cases same : index = condition
    · simp [assign, same, ih, Nat.add_assoc]
    · simp only [assign, same, ↓reduceIte, ih]
      omega

/-- A required threshold forces a condition when removing all its occurrences
    leaves even the upper reading below the bound. Assuming that condition false
    would enclose the actual sum below the required bound, a contradiction. -/
theorem condition_required_true [DecidableEq β] (lower upper actual : β → Bool)
    (inputs : List (Nat × β)) (bound : Nat) (condition : β)
    (compatible : Compatible lower upper actual)
    (required : bound ≤ maskedSum actual inputs)
    (essential : maskedSum (assign upper condition false) inputs < bound) :
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
    have below : maskedSum actual inputs ≤
        maskedSum (assign upper condition false) inputs :=
      masked_sum_mono actual (assign upper condition false) inputs enclosed
    omega

/-- A forbidden threshold forces a condition false when adding all its occurrences
    already makes the lower reading sufficient. If it were true, that strengthened
    lower reading would be contained in the actual sum and violate the prohibition. -/
theorem condition_required_false [DecidableEq β] (lower upper actual : β → Bool)
    (inputs : List (Nat × β)) (bound : Nat) (condition : β)
    (compatible : Compatible lower upper actual)
    (forbidden : maskedSum actual inputs < bound)
    (decisive : bound ≤ maskedSum (assign lower condition true) inputs) :
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
    have above : maskedSum (assign lower condition true) inputs ≤
        maskedSum actual inputs :=
      masked_sum_mono (assign lower condition true) actual inputs enclosed
    omega

/-- The lower-reading consequence is satisfaction of the existing Ferraris
    threshold formula, for arbitrary formulas used as its conditions. -/
theorem formula_of_lower (M : Atoms α) (conditions : β → Formula α)
    (inputs : List (Nat × β)) (bound : Nat) (lower upper : β → Bool)
    (compatible : Compatible lower upper (active M conditions))
    (sufficient : bound ≤ maskedSum lower inputs) :
    Satisfies M (formula conditions inputs bound) := by
  exact (original M conditions inputs bound).mpr
    (threshold_of_lower lower upper (active M conditions) inputs bound compatible sufficient)

/-- The upper-reading consequence excludes satisfaction of the same threshold
    formula. Source condition correlations do not weaken this necessary bound. -/
theorem formula_absent_of_upper (M : Atoms α) (conditions : β → Formula α)
    (inputs : List (Nat × β)) (bound : Nat) (lower upper : β → Bool)
    (compatible : Compatible lower upper (active M conditions))
    (insufficient : maskedSum upper inputs < bound) :
    ¬ Satisfies M (formula conditions inputs bound) := by
  intro satisfies
  have required : bound ≤ maskedSum (active M conditions) inputs :=
    (original M conditions inputs bound).mp satisfies
  have excluded : maskedSum (active M conditions) inputs < bound :=
    threshold_absent_of_upper lower upper (active M conditions) inputs bound compatible insufficient
  omega

end Zetesis.ThresholdPropagation
