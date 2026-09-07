import Zetesis.Core

/-!
# Ordered relation boundaries

A relation in canonical tuple order admits a monotone boundary test for a fixed
leading key. Binary search finds that boundary without inspecting every row.
The interval contracts below state both retained coverage and excluded rows.
They are independent of ASP term order and of the physical representation.

The search decreases its interval width. The laws assume a total immutable
predicate whose true positions form an initial segment. Relating Rust's Value
order, comparisons and fixed parent binding to this predicate remains a
representation obligation. These laws do not prove Rust execution, resource
accounting, cancellation, matcher rollback or source-carrier completeness.
-/

namespace Zetesis.OrderedProbes

/-- Earlier positions remain before a boundary whenever a later one is before it. -/
def Initial (before : Nat → Bool) : Prop :=
  ∀ i j, i ≤ j → before j = true → before i = true

/-- First false position in a finite half-open interval, or its upper endpoint. -/
def boundary (before : Nat → Bool) (start width : Nat) : Nat :=
  if empty : width = 0 then start
  else
    let half := width / 2
    if before (start + half) then
      boundary before (start + half + 1) (width - half - 1)
    else
      boundary before start half
termination_by width
decreasing_by all_goals omega

/-- A cut includes every true position and excludes every false position in the
    given interval. The endpoint itself need not denote a relation row. -/
def Partitions (before : Nat → Bool) (start width cut : Nat) : Prop :=
  start ≤ cut ∧ cut ≤ start + width ∧
    (∀ i, start ≤ i → i < cut → before i = true) ∧
    (∀ i, cut ≤ i → i < start + width → before i = false)

theorem boundary_partitions (before : Nat → Bool) (ordered : Initial before)
    (start width : Nat) : Partitions before start width (boundary before start width) := by
  induction width using Nat.strongRecOn generalizing start with
  | ind width smaller =>
    by_cases empty : width = 0
    · subst width
      simp [boundary, Partitions]
      constructor <;> intro i lower upper <;> omega
    · have half_smaller : width / 2 < width := by omega
      have tail_smaller : width - width / 2 - 1 < width := by omega
      by_cases middle : before (start + width / 2) = true
      · have tail := smaller (width - width / 2 - 1) tail_smaller (start + width / 2 + 1)
        obtain ⟨lower, upper, left, right⟩ := tail
        rw [boundary, dif_neg empty, if_pos middle]
        refine ⟨by omega, by omega, ?_, ?_⟩
        · intro i inside below
          by_cases early : i ≤ start + width / 2
          · exact ordered i (start + width / 2) early middle
          · exact left i (by omega) below
        · intro i above inside
          exact right i above (by omega)
      · have left_half := smaller (width / 2) half_smaller start
        obtain ⟨lower, upper, left, right⟩ := left_half
        rw [boundary, dif_neg empty, if_neg middle]
        refine ⟨lower, by omega, left, ?_⟩
        intro i above inside
        by_cases early : i < start + width / 2
        · exact right i above early
        · have cannot_be_true : before i ≠ true := by
            intro truth
            exact middle (ordered (start + width / 2) i (by omega) truth)
          cases value : before i <;> simp_all

/-- Restricting a complete row scan to a certified window cannot remove a full
    match when matching implies the window's key condition. -/
theorem complete_match_retained {α : Type} (row : Nat → α)
    (accepts : α → Prop) (lower upper : Nat → Bool) (n start finish : Nat)
    (left : Partitions lower 0 n start)
    (right : Partitions upper start (n - start) finish)
    (key : ∀ i, i < n → accepts (row i) → lower i = false ∧ upper i = true)
    (i : Nat) (inside : i < n) (matched : accepts (row i)) :
    start ≤ i ∧ i < finish := by
  obtain ⟨start_lower, start_upper, before_start, after_start⟩ := left
  obtain ⟨finish_lower, finish_upper, before_finish, after_finish⟩ := right
  obtain ⟨not_below, not_above⟩ := key i inside matched
  have starts_before : start ≤ i := by
    by_cases earlier : start ≤ i
    · exact earlier
    · have truth := before_start i (by omega) (by omega)
      simp_all
  have ends_after : i < finish := by
    by_cases later : i < finish
    · exact later
    · have falsehood := after_finish i (by omega) (by omega)
      simp_all
  exact ⟨starts_before, ends_after⟩

end Zetesis.OrderedProbes
