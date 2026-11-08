import Zetesis.ThresholdPropagation
import Zetesis.ThresholdConnectives

/-!
# Necessary contributions under an exclusive capacity

A body's necessary conditions can exhaust a row's capacity together with the
conditions already held true. Their overlap contributes only once. The union
decomposition is exact; its sum is a lower bound whenever both masks imply the
actual condition truth. A recorded held sum may underestimate the current held
sum without invalidating that lower bound.

The weighted laws retain every input position. The unit-count corollary uses a
duplicate-free carrier, so its filtered length counts distinct conditions. A
Rust projection must separately establish this carrier and its unit weights,
the implications that make the projected conditions necessary, and the validity
of its recorded held sum. No queue, finite-width calculation or executable
refinement is proved here.

The formula consequence concerns original satisfaction. An original producer
implication does not automatically supply the necessary-condition premise for
an arbitrary frozen reduct; that premise would have to be established afresh.
-/

namespace Zetesis.ThresholdCapacity

open Ferraris ThresholdReduct ThresholdPropagation ThresholdConnectives

universe u v
variable {α : Type u} {β : Type v}

/-- The union of held and required conditions counts the held contributions and
    only the additional required contributions. Each position contributes its
    weight once even when both masks select it. Repeated input positions retain
    their separate weights, as in the existing threshold semantics. -/
theorem masked_sum_union (held required : β → Bool) (inputs : List (Nat × β)) :
    maskedSum (fun index => held index || required index) inputs =
      maskedSum held inputs +
        maskedSum (fun index => required index && !held index) inputs := by
  induction inputs with
  | nil => simp [maskedSum, queries, Thresholds.enabledSum]
  | cons input rest ih =>
    obtain ⟨weight, index⟩ := input
    change (if held index || required index then weight else 0) +
      maskedSum (fun index => held index || required index) rest =
        ((if held index then weight else 0) + maskedSum held rest) +
          ((if required index && !held index then weight else 0) +
            maskedSum (fun index => required index && !held index) rest)
    rw [ih]
    cases held index <;> cases required index <;> simp <;> omega

/-- Held truth together with a body's necessary conditions supplies a lower
    bound for the actual sum. First, both masks are contained in actual truth.
    Second, their exact union avoids double counting. Replacing the held sum
    by any recorded lower estimate can only weaken the conclusion. -/
theorem capacity_lower_bound (held required actual : β → Bool)
    (inputs : List (Nat × β)) (recorded : Nat)
    (heldTrue : ∀ index, held index = true → actual index = true)
    (requiredTrue : ∀ index, required index = true → actual index = true)
    (recordedBelow : recorded ≤ maskedSum held inputs) :
    recorded + maskedSum (fun index => required index && !held index) inputs ≤
      maskedSum actual inputs := by
  have unionIncluded : ∀ index, (held index || required index) = true →
      actual index = true := by
    intro index included
    have either : held index = true ∨ required index = true := by
      simpa only [Bool.or_eq_true] using included
    rcases either with fromHeld | fromRequired
    · exact heldTrue index fromHeld
    · exact requiredTrue index fromRequired
  have unionBelow : maskedSum (fun index => held index || required index) inputs ≤
      maskedSum actual inputs :=
    masked_sum_mono (fun index => held index || required index) actual inputs unionIncluded
  calc
    recorded + maskedSum (fun index => required index && !held index) inputs ≤
        maskedSum held inputs +
          maskedSum (fun index => required index && !held index) inputs :=
      Nat.add_le_add_right recordedBelow _
    _ = maskedSum (fun index => held index || required index) inputs :=
      (masked_sum_union held required inputs).symm
    _ ≤ maskedSum actual inputs := unionBelow

/-- A unit-weight sum counts exactly the selected list positions. Repeated
    indices retain separate positions; a count of distinct conditions therefore
    additionally requires a duplicate-free input list. -/
theorem unit_sum_filter_length (mask : β → Bool) (indices : List β) :
    maskedSum mask (unitRow indices) = (indices.filter mask).length := by
  induction indices with
  | nil => simp [unitRow, maskedSum, queries, Thresholds.enabledSum]
  | cons index rest ih =>
    change (if mask index then 1 else 0) + maskedSum mask (unitRow rest) =
      ((index :: rest).filter mask).length
    cases enabled : mask index <;> simp [enabled, ih, Nat.add_comm]

/-- For a duplicate-free unit row, the additional sum is exactly the number of
    required conditions not already held. The carrier certificate makes this a
    count of distinct conditions; it is not a license to replace arbitrary
    weighted or repeated input rows by a set of condition IDs. -/
theorem unit_capacity_lower_bound (held required actual : β → Bool)
    (carrier : { indices : List β // indices.Nodup }) (recorded : Nat)
    (heldTrue : ∀ index, held index = true → actual index = true)
    (requiredTrue : ∀ index, required index = true → actual index = true)
    (recordedBelow : recorded ≤ maskedSum held (unitRow carrier.val)) :
    recorded + (carrier.val.filter (fun index => required index && !held index)).length ≤
      maskedSum actual (unitRow carrier.val) := by
  have lowerBound : recorded +
      maskedSum (fun index => required index && !held index) (unitRow carrier.val) ≤
        maskedSum actual (unitRow carrier.val) :=
    capacity_lower_bound held required actual (unitRow carrier.val) recorded
      heldTrue requiredTrue recordedBelow
  rw [unit_sum_filter_length] at lowerBound
  exact lowerBound

/-- A body is false when its necessary conditions, together with held truth,
    would reach a forbidden threshold. Assuming the body true supplies the
    required mask; the lower-bound law then contradicts the exclusive bound.
    This is an original-candidate consequence, not a reduct replacement. -/
theorem body_excluded_original (M : Atoms α) (conditions : β → Formula α)
    (body : Formula α) (inputs : List (Nat × β)) (held required : β → Bool)
    (recorded bound : Nat)
    (heldTrue : ∀ index, held index = true → Satisfies M (conditions index))
    (necessary : Satisfies M body →
      ∀ index, required index = true → Satisfies M (conditions index))
    (recordedBelow : recorded ≤ maskedSum held inputs)
    (exclusive : ¬ Satisfies M (formula conditions inputs bound))
    (exhausted : bound ≤ recorded +
      maskedSum (fun index => required index && !held index) inputs) :
    ¬ Satisfies M body := by
  classical
  intro bodyTrue
  have heldActive : ∀ index, held index = true → active M conditions index = true := by
    intro index enabled
    exact (decide_eq_true_eq).mpr (heldTrue index enabled)
  have requiredActive : ∀ index, required index = true →
      active M conditions index = true := by
    intro index enabled
    exact (decide_eq_true_eq).mpr (necessary bodyTrue index enabled)
  have actualLowerBound : recorded +
      maskedSum (fun index => required index && !held index) inputs ≤
        maskedSum (active M conditions) inputs :=
    capacity_lower_bound held required (active M conditions) inputs recorded
      heldActive requiredActive recordedBelow
  have thresholdTrue : Satisfies M (formula conditions inputs bound) :=
    (ThresholdReduct.original M conditions inputs bound).mpr
      (Nat.le_trans exhausted actualLowerBound)
  exact exclusive thresholdTrue

end Zetesis.ThresholdCapacity
