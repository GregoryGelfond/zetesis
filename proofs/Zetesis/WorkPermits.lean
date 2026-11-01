import Std

/-!
# Shared work allowances with scoped grants

A finite allowance is partitioned into committed work, available permits and
outstanding grants. A query consumes a grant locally; settlement records exactly
its consumed part and returns the unused part. An empty available pool alone is
not exhaustion while an outstanding grant can still return permits.

A local window records one grant and its remaining permits. Taking a bounded
prefix and returning an owned unused part preserve that window; settlement
partitions the original grant into used and unused permits. These scalar laws
do not specify whether a caller refills, waits, polls or admits a whole request.

These natural-number laws justify the accounting design for joined independent
queries. They do not prove Rust mutex/condition-variable behavior, RAII cleanup,
thread progress, word arithmetic, cancellation latency or the search algorithm.
No candidate is made stable or unstable by a scheduling decision.
-/

namespace Zetesis.WorkPermits

/-- A local allowance snapshot. Validity is an explicit hypothesis, not a
construction rule: remaining permits must belong to this grant. -/
structure Window where
  granted : Nat
  remaining : Nat

/-- Remaining permits cannot exceed the grant that owns them. -/
def Window.Valid (window : Window) : Prop :=
  window.remaining ≤ window.granted

/-- The empty detached lease owns no grant and no remaining permits. -/
def Window.empty : Window := ⟨0, 0⟩

/-- The part of the grant already taken from this local window. -/
def Window.used (window : Window) : Nat :=
  window.granted - window.remaining

/-- Take at most the requested amount and at most what remains. -/
def Window.taken (window : Window) (wanted : Nat) : Nat :=
  min wanted window.remaining

/-- Consume a bounded prefix of the current window, without refilling it. -/
def Window.take (window : Window) (wanted : Nat) : Window :=
  { window with remaining := window.remaining - window.taken wanted }

/-- Return unused permits to their current grant. The laws below require the
returned amount to be bounded by the part already taken from this window. -/
def Window.refund (window : Window) (unused : Nat) : Window :=
  { window with remaining := window.remaining + unused }

/-- Taking a bounded prefix preserves the local window: it only decreases the
remaining permits and does not change their grant. -/
theorem take_preserves_window (wanted : Nat) (window : Window)
    (valid : window.Valid) : (window.take wanted).Valid := by
  show window.remaining - window.taken wanted ≤ window.granted
  exact Nat.le_trans (Nat.sub_le _ _) valid

/-- The taken prefix and new remainder partition the old remainder. This holds
also for a zero request or complete exhaustion; it needs no premise about the
size of the grant because the prefix is bounded by the remainder itself. -/
theorem take_partitions_remaining (wanted : Nat) (window : Window) :
    window.taken wanted + (window.take wanted).remaining = window.remaining := by
  have bounded : window.taken wanted ≤ window.remaining :=
    Nat.min_le_right wanted window.remaining
  show window.taken wanted + (window.remaining - window.taken wanted) = window.remaining
  omega

/-- Returning only unused permits already taken from this grant preserves its
window and decreases the taken part by exactly the returned amount. The numeric
bound is explicit; ownership of an actual nested borrow remains a separate
representation obligation.

The bound places the enlarged remainder below the grant. Subtracting that
remainder then gives the exact change in the taken part. -/
theorem refund_preserves_window (unused : Nat) (window : Window)
    (valid : window.Valid) (refundable : unused ≤ window.used) :
    (window.refund unused).Valid ∧
      (window.refund unused).used + unused = window.used := by
  have before : window.remaining ≤ window.granted := valid
  have owned : unused ≤ window.granted - window.remaining := refundable
  constructor
  · show window.remaining + unused ≤ window.granted
    omega
  · show window.granted - (window.remaining + unused) + unused =
      window.granted - window.remaining
    omega

/-- A valid grant partitions exactly into its used and unused parts, and an
empty detached lease is valid. This is the local receipt used by settlement;
it does not perform publication into a source ledger or prove a drop action.
A cumulative meter may retain its charged history instead of becoming empty. -/
theorem settlement_partitions_grant (window : Window) (valid : window.Valid) :
    window.used + window.remaining = window.granted ∧ Window.empty.Valid := by
  constructor
  · have bounded : window.remaining ≤ window.granted := valid
    show window.granted - window.remaining + window.remaining = window.granted
    omega
  · exact Nat.le_refl 0

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
