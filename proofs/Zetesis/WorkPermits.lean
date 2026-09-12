import Std

/-!
# Shared work allowances with scoped grants

A finite allowance is partitioned into committed work, available permits and
outstanding grants. A query consumes a grant locally; settlement records exactly
its consumed part and returns the unused part. An empty available pool alone is
not exhaustion while an outstanding grant can still return permits.

These natural-number laws justify the accounting design for joined independent
queries. They do not prove Rust mutex/condition-variable behavior, RAII cleanup,
thread progress, word arithmetic, cancellation latency or the search algorithm.
No candidate is made stable or unstable by a scheduling decision.
-/

namespace Zetesis.WorkPermits

/-- Outstanding counts complete unsettled grants, including their used parts. -/
structure Ledger where
  spent : Nat
  available : Nat
  outstanding : Nat

/-- Every permit remains in exactly one accounting category. -/
def Conserves (limit : Nat) (state : Ledger) : Prop :=
  state.spent + state.available + state.outstanding = limit

/-- Move a grant from the available pool to one query's ownership. -/
def grant (amount : Nat) (state : Ledger) : Ledger :=
  { state with
    available := state.available - amount,
    outstanding := state.outstanding + amount }

/-- Remove a whole grant, commit its used part, and return its unused part. -/
def settle (amount used : Nat) (state : Ledger) : Ledger :=
  { spent := state.spent + used,
    available := state.available + (amount - used),
    outstanding := state.outstanding - amount }

/-- An admitted grant preserves the total allowance. The amount moved out of
available permits is exactly the amount added to outstanding grants. -/
theorem grant_conserves (limit amount : Nat) (state : Ledger)
    (conserved : Conserves limit state) (admitted : amount ≤ state.available) :
    Conserves limit (grant amount state) := by
  simp only [Conserves, grant] at *
  omega

/-- Settlement preserves the allowance when the returned grant is outstanding
and its used part does not exceed that grant. The used/unused partition accounts
for every removed outstanding permit. -/
theorem settle_conserves (limit amount used : Nat) (state : Ledger)
    (conserved : Conserves limit state) (owned : amount ≤ state.outstanding)
    (consumed : used ≤ amount) : Conserves limit (settle amount used state) := by
  have partition : used + (amount - used) = amount := by omega
  simp only [Conserves, settle] at *
  omega

/-- Actual work, including locally used but unsettled permits, never exceeds
the allowance when local consumption is covered by the outstanding grants. -/
theorem consumed_within_limit (limit locallyUsed : Nat) (state : Ledger)
    (conserved : Conserves limit state) (covered : locallyUsed ≤ state.outstanding) :
    state.spent + locallyUsed ≤ limit := by
  simp only [Conserves] at conserved
  omega

/-- With no available permits and no outstanding grants, committed work exactly
exhausts the allowance. Omitting the outstanding premise would be invalid. -/
theorem settled_exhaustion (limit : Nat) (state : Ledger)
    (conserved : Conserves limit state) (unavailable : state.available = 0)
    (joined : state.outstanding = 0) : state.spent = limit := by
  simpa only [Conserves, unavailable, joined, Nat.add_zero] using conserved

/-- Returning any unused part makes permits available again, independently of
how much committed work or other outstanding grants already exist. -/
theorem unused_return_enables_work (amount used : Nat) (state : Ledger)
    (unused : used < amount) : 0 < (settle amount used state).available := by
  simp only [settle]
  omega

end Zetesis.WorkPermits
