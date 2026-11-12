import Std.Tactic

/-!
# Bounded charges for one original evaluation of a shared row

A finite family carries a row charge, a live-guard count and a nonnegative guard
supplement. Its charge is zero when no guard is live, and otherwise the minimum
of the legacy per-guard charge and one row charge plus the guard supplement.
The saved charge and retained charge exactly partition the legacy charge.
Their finite sums preserve this identity and every previously admitted setup.

The scalar minimum is conservative only when both supplied tariffs bound the
represented work. This module does not establish that shader correspondence,
finite-width arithmetic, row or guard identity, cancellation, memory ownership
or hardware behavior. Logical-work charges are not retired instructions.
-/
namespace Zetesis.OriginalRowCharges

/-- One row tariff, its live-guard count and the nonnegative charge covering
all guard visits. Both complete alternative tariffs must bound actual work. -/
structure Family where
  row : Nat
  guards : Nat
  supplement : Nat

/-- The existing tariff charges a complete row traversal per live guard. -/
def legacy (f : Family) : Nat := f.guards * f.row

/-- Keep the lower of the two conservative complete charges for a live
family. An unused row contributes no original-evaluation work. -/
def charge (f : Family) : Nat :=
  if f.guards = 0 then 0 else min (legacy f) (f.row + f.supplement)

/-- The difference between legacy and retained charge; boundedness below
justifies its exact natural-number subtraction. -/
def discount (f : Family) : Nat := legacy f - charge f

/-- No live guards means no original-evaluation charge for this row. -/
theorem empty (row supplement : Nat) : charge ⟨row, 0, supplement⟩ = 0 := by
  simp [charge]

/-- The retained charge never exceeds the legacy charge because it is zero
for an unused family and otherwise takes the minimum with that charge. -/
theorem bounded (f : Family) : charge f ≤ legacy f := by
  by_cases h : f.guards = 0
  · simp [charge, h]
  · simp only [charge, h, if_false]
    exact Nat.min_le_left _ _

/-- A one-guard family retains its row charge. A nonnegative guard supplement
cannot lower the alternative below that existing single-row tariff. -/
theorem one_guard (row supplement : Nat) :
    charge ⟨row, 1, supplement⟩ = row := by
  change (if (1 : Nat) = 0 then 0 else min (1 * row) (row + supplement)) = row
  have nonzero : ¬ ((1 : Nat) = 0) := by decide
  rw [if_neg nonzero, Nat.one_mul]
  exact Nat.min_eq_left (Nat.le_add_right row supplement)

/-- The saved and retained charges exactly partition the legacy charge.
The subtraction is exact because the retained charge is bounded by the legacy. -/
theorem discount_conserves (f : Family) : discount f + charge f = legacy f := by
  exact Nat.sub_add_cancel (bounded f)

/-- Add a selected charge over a complete finite family inventory. -/
def total (cost : Family → Nat) : List Family → Nat
  | [] => 0
  | first :: rest => cost first + total cost rest

/-- The retained charge summed over the same finite families does not exceed
their legacy total. The pointwise bound composes by addition. -/
theorem total_bounded (families : List Family) :
    total charge families ≤ total legacy families := by
  induction families with
  | nil => simp [total]
  | cons first rest ih => exact Nat.add_le_add (bounded first) ih

/-- Saved and retained finite totals exactly partition the legacy total.
Each family satisfies conservation, and addition preserves it. -/
theorem total_conserves (families : List Family) :
    total discount families + total charge families = total legacy families := by
  induction families with
  | nil => simp [total]
  | cons first rest ih =>
    simp only [total]
    have hf := discount_conserves first
    omega

/-- Any setup admitted under the old total remains admitted under the bounded
shared-row total, with the same common work and inclusive ceiling. -/
theorem accepted_setup (families : List Family) (common ceiling : Nat)
    (admitted : common + total legacy families ≤ ceiling) :
    common + total charge families ≤ ceiling := by
  exact Nat.le_trans (Nat.add_le_add_left (total_bounded families) common) admitted

end Zetesis.OriginalRowCharges
