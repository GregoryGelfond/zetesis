import Zetesis.QueryCompaction

/-!
# Nonnegative weighted threshold recurrence

This executable query-tree recurrence is the unshared form of the nonnegative
threshold dynamic program: excluding the head keeps the threshold, including it
requires the residual threshold. Its Boolean meaning and bound monotonicity are
proved for arbitrary condition queries. These classical query laws do not yet
prove aggregate-to-Ferraris strong equivalence, tuple grouping, priority/source
semantics, Rust DAG sharing, finite-width arithmetic, or resource accounting.
-/

namespace Zetesis.Thresholds

open QueryCompaction

universe u
variable {α : Type u}

/-- The exact natural-number sum of weights whose condition query is true. -/
def enabledSum (valuation : α → Bool) : List (Nat × Query α) → Nat
  | [] => 0
  | (weight, condition) :: rest =>
      (if condition.eval valuation then weight else 0) + enabledSum valuation rest

/-- A threshold query assembled solely with monotone AND/OR connectives around
    its supplied conditions. Truncated subtraction handles a weight above bound. -/
def thresholdQuery : List (Nat × Query α) → Nat → Query α
  | [], bound => .wire (.constant (decide (bound = 0)))
  | (weight, condition) :: rest, bound =>
      .gate true (thresholdQuery rest bound)
        (.gate false condition (thresholdQuery rest (bound - weight)))

/-- The recurrence computes the exact nonnegative weighted threshold, including
    empty inputs, zero weights and zero bounds. -/
theorem threshold_query_exact (valuation : α → Bool)
    (inputs : List (Nat × Query α)) (bound : Nat) :
    (thresholdQuery inputs bound).eval valuation = true ↔
      bound ≤ enabledSum valuation inputs := by
  induction inputs generalizing bound with
  | nil => simp [thresholdQuery, Query.eval, Wire.eval, enabledSum]
  | cons input rest ih =>
    obtain ⟨weight, condition⟩ := input
    simp only [thresholdQuery, Query.eval, gateValue, ↓reduceIte, Bool.or_eq_true,
      ih, enabledSum]
    cases condition.eval valuation <;> simp [ih] <;> omega

/-- Raising a threshold cannot create a satisfying assignment. -/
theorem threshold_bound_antitone (valuation : α → Bool)
    (inputs : List (Nat × Query α)) (lower upper : Nat) (order : lower ≤ upper)
    (satisfies : (thresholdQuery inputs upper).eval valuation = true) :
    (thresholdQuery inputs lower).eval valuation = true := by
  apply (threshold_query_exact valuation inputs lower).mpr
  exact Nat.le_trans order ((threshold_query_exact valuation inputs upper).mp satisfies)

/-- Increasing the evaluated nonnegative sum preserves a satisfied threshold.
    This explicit hypothesis is needed when conditions contain negation. -/
theorem threshold_sum_monotone (first second : α → Bool)
    (inputs : List (Nat × Query α)) (bound : Nat)
    (grows : enabledSum first inputs ≤ enabledSum second inputs)
    (satisfies : (thresholdQuery inputs bound).eval first = true) :
    (thresholdQuery inputs bound).eval second = true := by
  apply (threshold_query_exact second inputs bound).mpr
  exact Nat.le_trans ((threshold_query_exact first inputs bound).mp satisfies) grows

end Zetesis.Thresholds
