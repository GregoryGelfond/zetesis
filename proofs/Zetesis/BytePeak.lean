import Std.Tactic

/-!
# Observing a byte requirement and retaining its peak

An observation supplies a previous peak, a current total and an inclusive
ceiling, all natural numbers. Its receipt is the maximum of the previous peak
and the supplied total, on either acceptance or refusal. Only the current total
is compared with the ceiling; a historical peak above a new limit is permitted.
The caller determines whether the supplied total is a requested envelope or an
actual capacity observation. This scalar model does not establish either claim.

These laws do not model finite-width arithmetic, allocations, current/live-byte
ownership, release lifetimes, polling, native errors or solver correctness.
Receipt composition does not combine control observations or failure traces.
-/

namespace Zetesis.BytePeak

/-- The ceiling decision together with the peak retained after observation. -/
inductive Outcome where
  | accepted (peak : Nat)
  | refused (peak : Nat)
  deriving DecidableEq

/-- Read the retained peak independently of the ceiling decision. -/
def Outcome.receipt : Outcome → Nat
  | .accepted peak => peak
  | .refused peak => peak

/-- Publish the observed peak and decide whether the supplied total fits.
The ceiling constrains total, not the historical peak. -/
def observe (previous total limit : Nat) : Outcome :=
  if total ≤ limit then .accepted (max previous total)
  else .refused (max previous total)

/-- Acceptance occurs exactly when the supplied total fits the inclusive limit;
its receipt is the maximum of the previous peak and that total. No premise
requires the previous peak to fit this limit. The two comparison cases give
both the decision and the retained receipt. -/
theorem accepted_iff (previous total limit result : Nat) :
    observe previous total limit = .accepted result ↔
      total ≤ limit ∧ result = max previous total := by
  by_cases fits : total ≤ limit
  · simp [observe, fits, eq_comm]
  · simp [observe, fits]

/-- Refusal occurs exactly when the supplied total exceeds the limit. It still
retains the updated peak, rather than rolling back or clamping the observation. -/
theorem refused_iff (previous total limit result : Nat) :
    observe previous total limit = .refused result ↔
      limit < total ∧ result = max previous total := by
  by_cases fits : total ≤ limit
  · have does_not_exceed : ¬ limit < total := by omega
    simp [observe, fits, does_not_exceed]
  · have exceeds : limit < total := by omega
    simp [observe, fits, exceeds, eq_comm]

/-- The retained receipt is the observed maximum for either ceiling decision.
The decision changes no part of that maximum. -/
theorem receipt_eq_max (previous total limit : Nat) :
    (observe previous total limit).receipt = max previous total := by
  by_cases fits : total ≤ limit
  · simp [observe, fits, Outcome.receipt]
  · simp [observe, fits, Outcome.receipt]

/-- An observation never lowers its previous peak and always covers the supplied
total, whether accepted or refused. This follows from the two maximum bounds. -/
theorem receipt_covers_inputs (previous total limit : Nat) :
    previous ≤ (observe previous total limit).receipt ∧
      total ≤ (observe previous total limit).receipt := by
  constructor
  · rw [receipt_eq_max]
    exact Nat.le_max_left previous total
  · rw [receipt_eq_max]
    exact Nat.le_max_right previous total

/-- Zero is accepted under every natural-number ceiling and leaves the previous
peak unchanged, even when that peak exceeds the current limit. -/
theorem observe_zero (previous limit : Nat) :
    observe previous 0 limit = .accepted previous := by
  simp [observe]

/-- Changing only the historical peak cannot change whether this observation is
accepted. The resulting peak may differ; the common admission premise is just
that the supplied total fits the limit. -/
theorem acceptance_independent_of_previous (left right total limit : Nat) :
    (∃ result, observe left total limit = .accepted result) ↔
      (∃ result, observe right total limit = .accepted result) := by
  constructor
  · rintro ⟨result, accepted⟩
    have fits : total ≤ limit := by
      exact ((accepted_iff left total limit result).mp accepted).1
    exact ⟨max right total,
      (accepted_iff right total limit (max right total)).mpr ⟨fits, rfl⟩⟩
  · rintro ⟨result, accepted⟩
    have fits : total ≤ limit := by
      exact ((accepted_iff right total limit result).mp accepted).1
    exact ⟨max left total,
      (accepted_iff left total limit (max left total)).mpr ⟨fits, rfl⟩⟩

/-- If a second observation uses the first observation's receipt, the final
receipt is the maximum of the previous peak and both supplied totals. The two
ceilings may differ: neither decision changes the corresponding receipt.

Apply the one-observation receipt law twice, then associativity of maximum.
This statement neither requires nor authorizes a caller to continue after a
refusal. It does not equate the decisions with a single combined observation,
and does not eliminate any intervening poll, failure or execution boundary. -/
theorem successive_receipts (previous first firstLimit second secondLimit : Nat) :
    (observe (observe previous first firstLimit).receipt second secondLimit).receipt =
      max previous (max first second) := by
  calc
    (observe (observe previous first firstLimit).receipt second secondLimit).receipt =
        max (observe previous first firstLimit).receipt second :=
      receipt_eq_max (observe previous first firstLimit).receipt second secondLimit
    _ = max (max previous first) second :=
      congrArg (fun peak => max peak second) (receipt_eq_max previous first firstLimit)
    _ = max previous (max first second) := Nat.max_assoc previous first second

end Zetesis.BytePeak
