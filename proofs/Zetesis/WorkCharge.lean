import Std.Tactic

/-!
# Batched bookkeeping under an inclusive work bound

Charging an indivisible operation's payload in one step preserves repeated
unit charging: success records the entire amount; failure records precisely
the available prefix. Neither operation is permitted after a refused charge.

The correspondence uses natural-number counters and an unchanged, permissive
control boundary. Rust overflow, cancellation and deadline polling, allocation,
and the charged operation itself remain implementation obligations. In
particular, this law does not assert observation of an asynchronous stop between
bookkeeping units that perform no real operation.
-/

namespace Zetesis.WorkCharge

/-- The charged prefix and whether the entire request was admitted. -/
inductive Outcome where
  | accepted (recorded : Nat)
  | exhausted (recorded : Nat)
  deriving DecidableEq

/-- Unit charging stops before the first unit beyond the inclusive ceiling. -/
def repeated (recorded limit : Nat) : Nat → Outcome
  | 0 => .accepted recorded
  | amount + 1 =>
    if recorded < limit then repeated (recorded + 1) limit amount
    else .exhausted recorded

/-- Batched charging records either the complete request or the available prefix. -/
def charge (recorded limit amount : Nat) : Outcome :=
  if amount ≤ limit - recorded then .accepted (recorded + amount)
  else .exhausted limit

/-- A batch has exactly the outcome of repeated unit charging.

Induct on the requested amount. When a unit is available, charging it leaves
one fewer requested unit and one fewer available unit. When none is available,
both transitions refuse at the same ceiling.
-/
theorem repeated_eq_charge (recorded limit amount : Nat)
    (within : recorded ≤ limit) :
    repeated recorded limit amount = charge recorded limit amount := by
  induction amount generalizing recorded with
  | zero => simp [repeated, charge]
  | succ amount induction =>
    by_cases available : recorded < limit
    · have after_unit : recorded + 1 ≤ limit := by omega
      simp only [repeated, available, ↓reduceIte]
      rw [induction (recorded + 1) after_unit]
      by_cases fits : amount ≤ limit - (recorded + 1)
      · have full_fits : amount + 1 ≤ limit - recorded := by omega
        simp only [charge, fits, full_fits, ↓reduceIte]
        congr 1
        omega
      · have full_refused : ¬ amount + 1 ≤ limit - recorded := by omega
        simp only [charge, fits, full_refused, ↓reduceIte]
    · have at_limit : recorded = limit := by omega
      simp [repeated, at_limit, charge]

/-- Every refused request records the full available prefix, not the demand. -/
theorem exhausted_records_limit (recorded limit amount : Nat)
    (refused : limit - recorded < amount) :
    charge recorded limit amount = .exhausted limit := by
  simp only [charge, Nat.not_le.mpr refused, ↓reduceIte]

/-- A completed request never records more work than the caller's ceiling. -/
theorem accepted_within_limit (recorded limit amount result : Nat)
    (within : recorded ≤ limit)
    (accepted : charge recorded limit amount = .accepted result) :
    result ≤ limit := by
  unfold charge at accepted
  split at accepted
  next fits => cases accepted; omega
  next => cases accepted

end Zetesis.WorkCharge
