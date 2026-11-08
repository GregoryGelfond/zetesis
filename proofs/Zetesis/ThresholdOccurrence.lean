import Zetesis.ThresholdPropagation

/-!
# Conservative forcing from threshold occurrences

One occurrence's weight can justify a deduction even when a condition has
several occurrences. These laws refine ThresholdPropagation's complete-row
bounds; they do not model concurrent execution or machine arithmetic.

For stale physical-slot readings, instantiate the index type with occurrence
positions and lift actual truth from the physical interpretation. Lower and
upper readings need not agree at aliased positions. Compatibility is required
at every observed position, and the selected occurrence's inclusion or exclusion
must refer to the reading used in the corresponding sum. Rust/WGSL must connect
later atomic observations to that premise without assuming coherent alias
reads, preserve every contribution and establish finite-width non-overflow.
-/

namespace Zetesis.ThresholdPropagation

open ThresholdReduct

universe u
variable {β : Type u}

/-- Assigning a value already present in a reading preserves that reading. -/
theorem assign_current [DecidableEq β] (mask : β → Bool) (condition : β)
    (truth : Bool) (current : mask condition = truth) :
    assign mask condition truth = mask := by
  funext index
  by_cases same : index = condition
  · subst index
    simp [assign, current]
  · simp [assign, same]

/-- Every occurrence contributes at most the full coefficient of its condition. -/
theorem occurrence_le_coefficient [DecidableEq β] (condition : β)
    (inputs : List (Nat × β)) (weight : Nat)
    (present : (weight, condition) ∈ inputs) :
    weight ≤ coefficient condition inputs := by
  induction inputs with
  | nil => simp at present
  | cons input rest ih =>
    rcases List.mem_cons.mp present with same | later
    · subst input
      simp [coefficient]
    · have rest_bound : weight ≤ coefficient condition rest := ih later
      change weight ≤ (if input.2 = condition then input.1 else 0) +
        coefficient condition rest
      omega

/-- Removing a whole condition removes at least one occurrence's weight.
    This deliberately permits a smaller deduction than coefficient grouping. -/
theorem assigned_false_le_remove_occurrence [DecidableEq β]
    (upper : β → Bool) (condition : β) (inputs : List (Nat × β))
    (weight : Nat) (included : upper condition = true)
    (weight_bound : weight ≤ coefficient condition inputs) :
    maskedSum (assign upper condition false) inputs ≤
      maskedSum upper inputs - weight := by
  have decomposition : maskedSum upper inputs = coefficient condition inputs +
      maskedSum (assign upper condition false) inputs := by
    simpa only [assign_current upper condition true included] using
      assigned_sum upper condition inputs
  omega

/-- Enabling a whole condition adds at least one occurrence's weight. -/
theorem add_occurrence_le_assigned_true [DecidableEq β]
    (lower : β → Bool) (condition : β) (inputs : List (Nat × β))
    (weight : Nat) (excluded : lower condition = false)
    (weight_bound : weight ≤ coefficient condition inputs) :
    maskedSum lower inputs + weight ≤
      maskedSum (assign lower condition true) inputs := by
  have decomposition : maskedSum (assign lower condition true) inputs =
      coefficient condition inputs + maskedSum lower inputs := by
    simpa only [assign_current lower condition false excluded] using
      assigned_sum lower condition inputs
  omega

/-- One occurrence can suffice to force its condition true. Every complete
    interpretation compatible with the readings and required threshold retains
    that occurrence; correlations with other positions do not weaken the claim. -/
theorem condition_required_true_of_occurrence [DecidableEq β]
    (lower upper actual : β → Bool) (inputs : List (Nat × β))
    (bound weight : Nat) (condition : β)
    (compatible : Compatible lower upper actual)
    (required : bound ≤ maskedSum actual inputs)
    (included : upper condition = true)
    (weight_bound : weight ≤ coefficient condition inputs)
    (essential : maskedSum upper inputs - weight < bound) :
    actual condition = true := by
  have insufficient : maskedSum (assign upper condition false) inputs < bound := by
    exact Nat.lt_of_le_of_lt
      (assigned_false_le_remove_occurrence upper condition inputs weight
        included weight_bound) essential
  exact condition_required_true lower upper actual inputs bound condition
    compatible required insufficient

/-- One occurrence can suffice to force its condition false. This uses a
    conservative lower reading and retains every input position. -/
theorem condition_required_false_of_occurrence [DecidableEq β]
    (lower upper actual : β → Bool) (inputs : List (Nat × β))
    (bound weight : Nat) (condition : β)
    (compatible : Compatible lower upper actual)
    (forbidden : maskedSum actual inputs < bound)
    (excluded : lower condition = false)
    (weight_bound : weight ≤ coefficient condition inputs)
    (decisive : bound ≤ maskedSum lower inputs + weight) :
    actual condition = false := by
  have sufficient : bound ≤ maskedSum (assign lower condition true) inputs := by
    exact Nat.le_trans decisive
      (add_occurrence_le_assigned_true lower condition inputs weight
        excluded weight_bound)
  exact condition_required_false lower upper actual inputs bound condition
    compatible forbidden sufficient

end Zetesis.ThresholdPropagation
