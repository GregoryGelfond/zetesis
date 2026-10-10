import Zetesis.AggregateReduct
import Zetesis.Thresholds

/-!
# Nonnegative threshold formulas and their reducts

The weighted threshold recurrence uses only conjunction and disjunction around
arbitrary condition formulas. Its reduct therefore uses the same recurrence over
recursively frozen conditions. Direct evaluation retains both original truth
and the threshold on frozen eligibility, for arbitrary interpretations M and J.

`Thresholds` proves the classical query recurrence. This module supplies its
formula-level reduct bridge and connects it to the complete-mask reference in
`AggregateReduct`. Weights are natural numbers and positions retain multiplicity.
Source tuple coalescing, shared Rust rows, finite-width arithmetic, propagation,
resource accounting and device execution remain separate obligations.
-/

namespace Zetesis.ThresholdReduct

open Ferraris QueryCompaction

universe u v
variable {α : Type u} {β : Type v} {n : Nat}

/-- One query occurrence per supplied position. Repeated indices retain their
    contributions and share the same condition. -/
def queries (inputs : List (Nat × β)) : List (Nat × Query β) :=
  inputs.map (fun (weight, index) => (weight, .wire (.literal ⟨index, true⟩)))

/-- Add the weights of active positions using the established query reducer. -/
def maskedSum (mask : β → Bool) (inputs : List (Nat × β)) : Nat :=
  Thresholds.enabledSum mask (queries inputs)

open Classical in
/-- Evaluate each position's condition without identifying equal conditions. -/
noncomputable def active (M : Atoms α) (conditions : β → Formula α) : β → Bool :=
  fun index => decide (Satisfies M (conditions index))

/-- A threshold recurrence over formula conditions. Inclusion subtracts a
    nonnegative weight from the bound; exclusion keeps the bound unchanged. -/
def formula (conditions : β → Formula α) : List (Nat × β) → Nat → Formula α
  | [], bound => if bound = 0 then .imp .bot .bot else .bot
  | (weight, index) :: rest, bound =>
      .disj (formula conditions rest bound)
        (.conj (conditions index) (formula conditions rest (bound - weight)))

/-- A threshold formula is true exactly when its active weights reach the bound.
    Induction separates exclusion and inclusion; truncated subtraction gives the
    inclusion case even when one weight exceeds the remaining bound. -/
theorem original (M : Atoms α) (conditions : β → Formula α)
    (inputs : List (Nat × β)) (bound : Nat) :
    Satisfies M (formula conditions inputs bound) ↔
      bound ≤ maskedSum (active M conditions) inputs := by
  classical
  induction inputs generalizing bound with
  | nil =>
    by_cases zero : bound = 0 <;>
      simp [formula, maskedSum, queries, Thresholds.enabledSum, Satisfies, zero]
  | cons entry rest ih =>
    obtain ⟨weight, index⟩ := entry
    have extendSum (mask : β → Bool) :
        maskedSum mask ((weight, index) :: rest) =
          (if mask index then weight else 0) + maskedSum mask rest := by
      rfl
    simp only [formula, Satisfies, ih, extendSum]
    by_cases holds : Satisfies M (conditions index)
    · simp [active, holds] <;> omega
    · simp [active, holds]

/-- The formula recurrence agrees with the existing executable classical query
    recurrence. This agreement alone would not establish reduct preservation. -/
theorem query_original (M : Atoms α) (conditions : β → Formula α)
    (inputs : List (Nat × β)) (bound : Nat) :
    Satisfies M (formula conditions inputs bound) ↔
      (Thresholds.thresholdQuery (queries inputs) bound).eval (active M conditions) = true :=
  (original M conditions inputs bound).trans
    (Thresholds.threshold_query_exact (active M conditions) (queries inputs) bound).symm

/-- Freezing commutes with this monotone recurrence. Conditions remain arbitrary
    formulas; only the surrounding threshold connectives are monotone.
    Conjunction and disjunction commute with reduct satisfaction, so induction
    replaces each condition by its reduct without assuming J is contained in M. -/
theorem frozen_formula (M J : Atoms α) (conditions : β → Formula α)
    (inputs : List (Nat × β)) (bound : Nat) :
    Satisfies J (Reduct M (formula conditions inputs bound)) ↔
      Satisfies J (formula (fun index => Reduct M (conditions index)) inputs bound) := by
  classical
  induction inputs generalizing bound with
  | nil =>
    by_cases zero : bound = 0 <;> simp [formula, zero, Reduct, Satisfies]
  | cons entry rest ih =>
    obtain ⟨weight, index⟩ := entry
    simp only [formula, RuleFactorization.reduct_disj,
      RuleFactorization.reduct_conj, Satisfies, ih]

/-- Direct reduct evaluation checks the original threshold and the same
    threshold on recursively frozen conditions. First, freezing commutes with
    the recurrence. Second, original evaluation reduces it to a masked sum.
    Finally, any true reduct requires original truth: an originally false
    formula would have reduced to falsum. The reverse direction uses the frozen
    threshold; the explicit original gate states the native evaluator contract. -/
theorem frozen (M J : Atoms α) (conditions : β → Formula α)
    (inputs : List (Nat × β)) (bound : Nat) :
    Satisfies J (Reduct M (formula conditions inputs bound)) ↔
      bound ≤ maskedSum (active M conditions) inputs ∧
        bound ≤ maskedSum (active J (fun index => Reduct M (conditions index))) inputs := by
  classical
  have frozenSum : Satisfies J (Reduct M (formula conditions inputs bound)) ↔
      bound ≤ maskedSum (active J (fun index => Reduct M (conditions index))) inputs :=
    (frozen_formula M J conditions inputs bound).trans
      (original J (fun index => Reduct M (conditions index)) inputs bound)
  constructor
  · intro satisfies
    have wasTrue : Satisfies M (formula conditions inputs bound) := by
      apply Classical.byContradiction
      intro absent
      have erased := RuleFactorization.false_reduct M (formula conditions inputs bound) absent
      rw [erased] at satisfies
      exact satisfies
    exact ⟨(original M conditions inputs bound).mp wasTrue, frozenSum.mp satisfies⟩
  · intro satisfies
    exact frozenSum.mpr satisfies.2

/-- Enumerate each finite tuple position once, preserving equal weights and
    equal condition formulas at different positions. -/
def indexed (weights : Fin n → Nat) : List (Nat × Fin n) :=
  (List.finRange n).map (fun index => (weights index, index))

/-- A fixed nonnegative threshold guard for the complete-mask aggregate. -/
def guard (weights : Fin n → Nat) (bound : Nat) (mask : AggregateReduct.Mask n) : Bool :=
  decide (bound ≤ maskedSum mask (indexed weights))

/-- The threshold recurrence and complete-mask aggregate have the same original
    truth. Their shared meaning is the same sum over distinct tuple positions. -/
theorem reference_original (M : Atoms α) (conditions : Fin n → Formula α)
    (weights : Fin n → Nat) (bound : Nat) :
    Satisfies M (formula conditions (indexed weights) bound) ↔
      Satisfies M (AggregateReduct.aggregate conditions (guard weights bound)
        (AggregateReduct.masks n)) := by
  classical
  rw [original, AggregateReduct.original M conditions (guard weights bound)
    (AggregateReduct.masks n) (AggregateReduct.masks_complete n)]
  simp only [guard, decide_eq_true_eq]
  rfl

/-- The recurrence and complete-mask aggregate also have the same frozen truth
    for arbitrary M and J. Both require original guard truth and guard truth on
    recursively frozen eligibility. This is the reduct bridge missing from the
    classical threshold law; it is independent of a concrete storage layout. -/
theorem reference_frozen (M J : Atoms α) (conditions : Fin n → Formula α)
    (weights : Fin n → Nat) (bound : Nat) :
    Satisfies J (Reduct M (formula conditions (indexed weights) bound)) ↔
      Satisfies J (Reduct M (AggregateReduct.aggregate conditions (guard weights bound)
        (AggregateReduct.masks n))) := by
  classical
  rw [frozen, AggregateReduct.direct_reduct]
  simp only [guard, decide_eq_true_eq]
  rfl

end Zetesis.ThresholdReduct
