import Zetesis.ObjectiveEligibility
import Zetesis.ThresholdPropagation

/-!
# Reusing a weighted row within one source-activity snapshot

A threshold row has immutable condition identities and nonnegative weights.
Its lower sum reads the required conditions; its upper sum reads every condition
not known absent. Every cut over that same row can read those two sums without
folding the row again. The bounds cover each compatible complete truth reading,
including correlated conditions and repeated condition identities.

The sufficient reuse premise is equality of activity at every row occurrence.
It does not follow merely from retaining the same row identity: a later source
refinement round may change a condition's activity. The Rust owner must end or
invalidate its evaluated-row cache at that boundary. Source tuple coalescing,
signed-weight lowering, checked machine arithmetic and resource accounting stay
separate implementation obligations. These are grounding-activity laws, not
reduct transformations or answer-set membership certificates.

For the append-only formula workspace, `EvaluationPrefix.evaluate_append` and
`EvaluationPrefix.storage_preservation` already justify continuation from a
completed immutable prefix. The Rust correspondence must additionally establish
that every reused node reads that same source-activity snapshot and that a
failed node never advances the initialized prefix.
-/

namespace Zetesis.SourceActivityReuse

open ObjectiveEligibility ThresholdReduct ThresholdPropagation

universe u
variable {Index : Type u}

/-- Required source conditions supply the lower truth reading. -/
def required : Activity → Bool
  | .required => true
  | _ => false

/-- Every condition except an absent one belongs to the upper truth reading. -/
def possible : Activity → Bool
  | .absent => false
  | _ => true

/-- A complete row's lower and upper sums retain every weighted occurrence.
    Source tuple deduplication, when required, precedes this representation. -/
def bounds (activities : Index → Activity) (inputs : List (Nat × Index)) : Nat × Nat :=
  (maskedSum (fun index => required (activities index)) inputs,
   maskedSum (fun index => possible (activities index)) inputs)

/-- A signed cut may lie below zero or above all possible row sums.
    Required takes precedence exactly as in the source-activity reader. -/
def threshold (summary : Nat × Nat) (bound : Int) : Activity :=
  if bound ≤ (summary.1 : Int) then .required
  else if (summary.2 : Int) < bound then .absent
  else .optional

/-- Covered source activities induce the lower/upper compatibility required by
    the existing threshold algebra. Optional activity asserts no realization. -/
theorem activity_masks_compatible (activities : Index → Activity) (actual : Index → Bool)
    (covered : ∀ index, Covers (activities index) (actual index)) :
    Compatible (fun index => required (activities index))
      (fun index => possible (activities index)) actual := by
  constructor
  · intro index known
    have pointwise := covered index
    cases activity : activities index <;> simp_all [required, Covers]
  · intro index holds
    have pointwise := covered index
    cases activity : activities index <;> simp_all [possible, Covers]

/-- One row reduction supplies sound activity for every signed threshold cut.
    First enclose the actual sum with the established threshold bounds; then
    each decisive comparison implies the corresponding actual Boolean truth.
    The remaining case stays optional and makes no existence claim. -/
theorem threshold_covers (activities : Index → Activity) (actual : Index → Bool)
    (inputs : List (Nat × Index)) (bound : Int)
    (covered : ∀ index, Covers (activities index) (actual index)) :
    Covers (threshold (bounds activities inputs) bound)
      (decide (bound ≤ (maskedSum actual inputs : Int))) := by
  have enclosed := sum_bounds
    (fun index => required (activities index))
    (fun index => possible (activities index)) actual inputs
    (activity_masks_compatible activities actual covered)
  have lower_bound : ((bounds activities inputs).1 : Int) ≤
      (maskedSum actual inputs : Int) := by
    exact Int.ofNat_le.mpr enclosed.1
  have upper_bound : (maskedSum actual inputs : Int) ≤
      ((bounds activities inputs).2 : Int) := by
    exact Int.ofNat_le.mpr enclosed.2
  unfold threshold
  split
  next sufficient =>
    change decide (bound ≤ (maskedSum actual inputs : Int)) = true
    exact decide_eq_true (Int.le_trans sufficient lower_bound)
  next insufficient_lower =>
    split
    next insufficient_upper =>
      change decide (bound ≤ (maskedSum actual inputs : Int)) = false
      apply decide_eq_false
      omega
    next undecided =>
      exact True.intro

/-- Cached sums remain valid when every condition occurring in the immutable
    row has the same activity. Changes outside that row are irrelevant. This
    premise must be re-established after a source refinement or owner change;
    equality of the row's identity alone does not establish it. -/
theorem bounds_equal_of_occurrence_agreement
    (first second : Index → Activity) (inputs : List (Nat × Index))
    (same : ∀ entry ∈ inputs, first entry.2 = second entry.2) :
    bounds first inputs = bounds second inputs := by
  induction inputs with
  | nil => rfl
  | cons entry rest induction =>
    have head_same : first entry.2 = second entry.2 := same entry (by simp)
    have tail_same : ∀ member ∈ rest, first member.2 = second member.2 := by
      intro member present
      exact same member (by simp [present])
    have tail_bounds : bounds first rest = bounds second rest := induction tail_same
    have lower_same := congrArg Prod.fst tail_bounds
    have upper_same := congrArg Prod.snd tail_bounds
    change
      ((if required (first entry.2) then entry.1 else 0) +
        (bounds first rest).1,
       (if possible (first entry.2) then entry.1 else 0) +
        (bounds first rest).2) =
      ((if required (second entry.2) then entry.1 else 0) +
        (bounds second rest).1,
       (if possible (second entry.2) then entry.1 else 0) +
        (bounds second rest).2)
    rw [head_same, lower_same, upper_same]

end Zetesis.SourceActivityReuse
