import Zetesis.ThresholdReduct

/-!
# Implications between nonnegative thresholds on prefix rows

Appending nonnegative contributions cannot decrease a selected sum. A threshold
on a prefix therefore implies a threshold on the extended row when the latter's
bound is no greater. Failure of the extended threshold conversely excludes the
prefix threshold. Both statements apply to arbitrary formula conditions and,
separately, to their reducts under arbitrary interpretations M and J.

The prefix consists of complete weight/condition pairs. Zero weights and repeated
positions retain their ordinary contribution semantics. These are implication
laws, not equivalences permitting either formula to replace the other. Finding
prefixes, grouping conditions, finite-width arithmetic, propagation scheduling
and executable correspondence remain separate obligations.
-/

namespace Zetesis.ThresholdPrefixes

open Ferraris ThresholdReduct

universe u v
variable {α : Type u} {β : Type v}

/-- Selecting positions from an appended row adds the selected sums of its
    two parts. Induction retains each occurrence, even when a condition appears
    in both parts or occurs several times within either part. -/
theorem masked_sum_append (mask : β → Bool)
    (initial suffix : List (Nat × β)) :
    maskedSum mask (initial ++ suffix) = maskedSum mask initial + maskedSum mask suffix := by
  induction initial with
  | nil => simp [maskedSum, queries, Thresholds.enabledSum]
  | cons entry rest ih =>
    obtain ⟨weight, index⟩ := entry
    change (if mask index then weight else 0) + maskedSum mask (rest ++ suffix) =
      ((if mask index then weight else 0) + maskedSum mask rest) + maskedSum mask suffix
    rw [ih, Nat.add_assoc]

/-- A prefix's selected sum is bounded by the extended row's sum because all
    contributions are nonnegative. -/
theorem masked_sum_prefix_le (mask : β → Bool)
    (initial suffix : List (Nat × β)) :
    maskedSum mask initial ≤ maskedSum mask (initial ++ suffix) := by
  rw [masked_sum_append]
  omega

/-- Reaching the prefix threshold reaches an extended-row threshold whose
    bound is no greater, under every supplied mask. -/
theorem masked_threshold_implies (mask : β → Bool)
    (initial suffix : List (Nat × β)) (sourceBound targetBound : Nat)
    (ordered : targetBound ≤ sourceBound)
    (reached : sourceBound ≤ maskedSum mask initial) :
    targetBound ≤ maskedSum mask (initial ++ suffix) := by
  exact Nat.le_trans ordered
    (Nat.le_trans reached (masked_sum_prefix_le mask initial suffix))

/-- Original satisfaction of the prefix threshold implies satisfaction of the
    extended threshold at a no-greater bound. Conditions are arbitrary formulas;
    their truth in the interpretation supplies the mask for the arithmetic law. -/
theorem original_implies (M : Atoms α) (conditions : β → Formula α)
    (initial suffix : List (Nat × β)) (sourceBound targetBound : Nat)
    (ordered : targetBound ≤ sourceBound)
    (satisfies : Satisfies M (formula conditions initial sourceBound)) :
    Satisfies M (formula conditions (initial ++ suffix) targetBound) := by
  have reached : sourceBound ≤ maskedSum (active M conditions) initial :=
    (ThresholdReduct.original M conditions initial sourceBound).mp satisfies
  have extended : targetBound ≤ maskedSum (active M conditions) (initial ++ suffix) :=
    masked_threshold_implies (active M conditions) initial suffix
      sourceBound targetBound ordered reached
  exact (ThresholdReduct.original M conditions (initial ++ suffix) targetBound).mpr extended

/-- If the extended threshold fails, the prefix threshold at a no-smaller bound
    fails too. This is contraposition of the original-satisfaction implication. -/
theorem original_contrapositive (M : Atoms α) (conditions : β → Formula α)
    (initial suffix : List (Nat × β)) (sourceBound targetBound : Nat)
    (ordered : targetBound ≤ sourceBound)
    (absent : ¬ Satisfies M (formula conditions (initial ++ suffix) targetBound)) :
    ¬ Satisfies M (formula conditions initial sourceBound) := by
  intro satisfies
  exact absent
    (original_implies M conditions initial suffix sourceBound targetBound ordered satisfies)

/-- The same implication holds between the frozen thresholds for arbitrary M
    and J. The prefix reduct supplies both its original and frozen-condition
    threshold readings. Each extends by the masked arithmetic law, establishing
    both gates of the extended reduct. No subset or original-model premise is
    needed. This law does not assert equivalence of the two reducts. -/
theorem frozen_implies (M J : Atoms α) (conditions : β → Formula α)
    (initial suffix : List (Nat × β)) (sourceBound targetBound : Nat)
    (ordered : targetBound ≤ sourceBound)
    (satisfies : Satisfies J (Reduct M (formula conditions initial sourceBound))) :
    Satisfies J (Reduct M (formula conditions (initial ++ suffix) targetBound)) := by
  have readings : sourceBound ≤ maskedSum (active M conditions) initial ∧
      sourceBound ≤ maskedSum (active J (fun index => Reduct M (conditions index))) initial :=
    (ThresholdReduct.frozen M J conditions initial sourceBound).mp satisfies
  have originalReached : targetBound ≤
      maskedSum (active M conditions) (initial ++ suffix) :=
    masked_threshold_implies (active M conditions) initial suffix
      sourceBound targetBound ordered readings.1
  have frozenReached : targetBound ≤
      maskedSum (active J (fun index => Reduct M (conditions index))) (initial ++ suffix) :=
    masked_threshold_implies (active J (fun index => Reduct M (conditions index)))
      initial suffix sourceBound targetBound ordered readings.2
  exact (ThresholdReduct.frozen M J conditions (initial ++ suffix) targetBound).mpr
    ⟨originalReached, frozenReached⟩

/-- Failure of the extended reduct excludes the prefix reduct under the same
    bound ordering, for arbitrary outer and tested interpretations. -/
theorem frozen_contrapositive (M J : Atoms α) (conditions : β → Formula α)
    (initial suffix : List (Nat × β)) (sourceBound targetBound : Nat)
    (ordered : targetBound ≤ sourceBound)
    (absent : ¬ Satisfies J (Reduct M (formula conditions (initial ++ suffix) targetBound))) :
    ¬ Satisfies J (Reduct M (formula conditions initial sourceBound)) := by
  intro satisfies
  exact absent
    (frozen_implies M J conditions initial suffix sourceBound targetBound ordered satisfies)

end Zetesis.ThresholdPrefixes
