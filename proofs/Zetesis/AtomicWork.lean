import Std.Tactic

/-!
# Atomic admission of a whole work request

A caller retains a natural-number receipt for work already accepted. A new
request is accepted exactly when the old receipt plus the entire request fits
the current inclusive limit. Acceptance records that sum; refusal leaves the
old receipt unchanged. In particular, a receipt imported under a smaller new
limit is valid input and is not clamped, even when the request is zero.

These laws model only the scalar admission operation, not machine overflow,
cancellation, poll ordering, callbacks, allocation, concurrency, or solver
correctness. Unlike prefix-consuming work charging, atomic refusal admits none
of the new request.
-/

namespace Zetesis.AtomicWork

/-- The admission decision and the retained receipt after that decision. -/
inductive Outcome where
  | accepted (recorded : Nat)
  | refused (recorded : Nat)
  deriving DecidableEq

/-- Read the retained receipt, independently of the admission decision. -/
def Outcome.receipt : Outcome → Nat
  | .accepted recorded => recorded
  | .refused recorded => recorded

/-- Admit the whole request or retain the old receipt without accepting any part.
No premise requires the old receipt to fit the newly supplied limit. -/
def charge (recorded limit amount : Nat) : Outcome :=
  if recorded + amount ≤ limit then .accepted (recorded + amount)
  else .refused recorded

/-- Acceptance occurs exactly when the complete demand fits, and its receipt is
exactly that demand. The two cases of the admission comparison establish both
directions without assuming that the previous receipt fits the current limit. -/
theorem accepted_iff (recorded limit amount result : Nat) :
    charge recorded limit amount = .accepted result ↔
      recorded + amount ≤ limit ∧ result = recorded + amount := by
  by_cases fits : recorded + amount ≤ limit
  · simp [charge, fits, eq_comm]
  · simp [charge, fits]

/-- There is a successful result exactly when the whole demand fits the limit.
This is the admission criterion; it does not assert that any operation whose
cost is requested has already been executed. -/
theorem accepts_iff_fits (recorded limit amount : Nat) :
    (∃ result, charge recorded limit amount = .accepted result) ↔
      recorded + amount ≤ limit := by
  constructor
  · rintro ⟨result, accepted⟩
    exact ((accepted_iff recorded limit amount result).mp accepted).1
  · intro fits
    exact ⟨recorded + amount,
      (accepted_iff recorded limit amount (recorded + amount)).mpr ⟨fits, rfl⟩⟩

/-- Refusal occurs exactly when the demand exceeds the current limit, and the
refused result retains the old receipt. Refusal does not consume a prefix of
the request or reduce work accepted under an earlier limit. -/
theorem refused_iff (recorded limit amount result : Nat) :
    charge recorded limit amount = .refused result ↔
      limit < recorded + amount ∧ result = recorded := by
  by_cases fits : recorded + amount ≤ limit
  · have does_not_exceed : ¬ limit < recorded + amount := by omega
    simp [charge, fits, does_not_exceed]
  · have exceeds : limit < recorded + amount := by omega
    simp [charge, fits, exceeds, eq_comm]

/-- A successful charge adds the entire requested amount to the old receipt. -/
theorem accepted_additive (recorded limit amount result : Nat)
    (accepted : charge recorded limit amount = .accepted result) :
    result = recorded + amount := by
  exact ((accepted_iff recorded limit amount result).mp accepted).2

/-- Every successful result fits the current inclusive limit, even though no
such bound was assumed for the imported receipt. -/
theorem accepted_within_limit (recorded limit amount result : Nat)
    (accepted : charge recorded limit amount = .accepted result) :
    result ≤ limit := by
  have admission : recorded + amount ≤ limit ∧ result = recorded + amount := by
    exact (accepted_iff recorded limit amount result).mp accepted
  rw [admission.2]
  exact admission.1

/-- Every refused request preserves the exact receipt from before the request. -/
theorem refused_preserves_prefix (recorded limit amount result : Nat)
    (refused : charge recorded limit amount = .refused result) :
    result = recorded := by
  exact ((refused_iff recorded limit amount result).mp refused).2

/-- Work accepted before a limit reduction is never erased: every new request,
including zero, is refused when the imported receipt already exceeds the limit. -/
theorem above_new_limit_refused (recorded limit amount : Nat)
    (above : limit < recorded) :
    charge recorded limit amount = .refused recorded := by
  have demand_exceeds : limit < recorded + amount := by omega
  exact (refused_iff recorded limit amount recorded).mpr ⟨demand_exceeds, rfl⟩

/-- Zero requests still check the current limit. They succeed within the limit
and refuse above it; either decision retains the same numerical receipt. -/
theorem charge_zero (recorded limit : Nat) :
    charge recorded limit 0 =
      if recorded ≤ limit then .accepted recorded else .refused recorded := by
  simp [charge]

/-- A zero request is accepted exactly when the old receipt fits the new limit. -/
theorem zero_accepted_iff (recorded limit : Nat) :
    charge recorded limit 0 = .accepted recorded ↔ recorded ≤ limit := by
  simpa using accepted_iff recorded limit 0 recorded

/-- Regardless of whether zero is accepted, its retained receipt is unchanged. -/
theorem zero_preserves_receipt (recorded limit : Nat) :
    (charge recorded limit 0).receipt = recorded := by
  rw [charge_zero]
  split <;> rfl

/-- Two successful requests under one unchanged ceiling have the same final
receipt as their combined request. This is bookkeeping equality only: it does
not combine the caller's two control observations or assert equivalence when
either request fails.

The first acceptance identifies the intermediate receipt. The second gives
both the final receipt and its bound; associativity yields the combined claim. -/
theorem accepted_composition (recorded limit first second middle result : Nat)
    (first_accepted : charge recorded limit first = .accepted middle)
    (second_accepted : charge middle limit second = .accepted result) :
    charge recorded limit (first + second) = .accepted result := by
  have middle_receipt : middle = recorded + first := by
    exact accepted_additive recorded limit first middle first_accepted
  have second_admission : middle + second ≤ limit ∧ result = middle + second := by
    exact (accepted_iff middle limit second result).mp second_accepted
  have combined_fits : recorded + (first + second) ≤ limit := by
    omega
  have final_receipt : result = recorded + (first + second) := by
    omega
  exact (accepted_iff recorded limit (first + second) result).mpr
    ⟨combined_fits, final_receipt⟩

end Zetesis.AtomicWork
