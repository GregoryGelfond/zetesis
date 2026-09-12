import Zetesis.Core

/-!
# First-new-occurrence support joins

A positive tuple combination chooses one row per source occurrence. For each
occurrence, old rows form a prefix of the current row domain. Every combination
containing a new row belongs to exactly one partition: its first new occurrence.
Earlier occurrences use old rows, the pivot uses new rows, and later occurrences
use all current rows. Source occurrence order is independent of join execution
order and does not identify repeated predicates with one another.

These laws establish finite partition coverage and disjointness. They do not
prove source certification, scalar evaluation, posting boundaries, Rust memory
accounting or fixed-point completion. Rich producers require their separate full
scan. Final authored-body validation remains a distinct obligation.
-/

namespace Zetesis.DeltaJoins

/-- A row choice belongs to each occurrence's current finite domain. -/
def Current (n : Nat) (upper row : Nat → Nat) : Prop :=
  ∀ occurrence, occurrence < n → row occurrence < upper occurrence

/-- The pivot chooses a new row and every earlier source occurrence an old row.
    Later occurrences need only the current-domain condition. -/
def FirstDelta (n : Nat) (old upper row : Nat → Nat) (pivot : Nat) : Prop :=
  pivot < n ∧ Current n upper row ∧ old pivot ≤ row pivot ∧
    ∀ occurrence, occurrence < pivot → row occurrence < old occurrence

/-- A finite nonempty set of true positions has a least position. Induct on
    the domain size: retain a true prefix position, or use the final position. -/
theorem first_true (truth : Nat → Bool) (n : Nat)
    (present : ∃ occurrence, occurrence < n ∧ truth occurrence = true) :
    ∃ pivot, pivot < n ∧ truth pivot = true ∧
      ∀ occurrence, occurrence < pivot → truth occurrence = false := by
  induction n with
  | zero => obtain ⟨occurrence, inside, _⟩ := present; omega
  | succ n smaller =>
    by_cases earlierFound : ∃ occurrence, occurrence < n ∧ truth occurrence = true
    · obtain ⟨pivot, inside, chosen, before⟩ := smaller earlierFound
      exact ⟨pivot, by omega, chosen, before⟩
    · obtain ⟨occurrence, inside, chosen⟩ := present
      have last : occurrence = n := by
        by_cases same : occurrence = n
        · exact same
        · exact False.elim (earlierFound ⟨occurrence, by omega, chosen⟩)
      subst occurrence
      refine ⟨n, by omega, chosen, ?_⟩
      intro occurrence before
      cases value : truth occurrence with
      | false => rfl
      | true => exact False.elim (earlierFound ⟨occurrence, before, value⟩)

/-- A current combination containing a new row appears in a first-delta
    partition, and every such partition contains a new row. The least true
    position of the new-row predicate supplies the forward direction. -/
theorem partition_complete (n : Nat) (old upper row : Nat → Nat) :
    (Current n upper row ∧ ∃ occurrence, occurrence < n ∧ old occurrence ≤ row occurrence) ↔
      ∃ pivot, FirstDelta n old upper row pivot := by
  constructor
  · rintro ⟨current, occurrence, inside, newRow⟩
    have present : ∃ i, i < n ∧ decide (old i ≤ row i) = true :=
      ⟨occurrence, inside, by simp [newRow]⟩
    obtain ⟨pivot, bounded, chosen, before⟩ :=
      first_true (fun i => decide (old i ≤ row i)) n present
    refine ⟨pivot, bounded, current, by simpa using chosen, ?_⟩
    intro i earlier
    have excluded := before i earlier
    simp only [decide_eq_false_iff_not] at excluded
    omega
  · rintro ⟨pivot, bounded, current, chosen, _⟩
    exact ⟨current, pivot, bounded, chosen⟩

/-- Distinct pivots cannot retain the same row combination. The earlier pivot
    would have to be both new at itself and old before the later pivot. -/
theorem partition_disjoint (n : Nat) (old upper row : Nat → Nat) (left right : Nat)
    (first : FirstDelta n old upper row left)
    (second : FirstDelta n old upper row right) : left = right := by
  obtain ⟨_, _, leftNew, beforeLeft⟩ := first
  obtain ⟨_, _, rightNew, beforeRight⟩ := second
  by_cases earlier : left < right
  · have leftOld := beforeRight left earlier
    omega
  · by_cases later : right < left
    · have rightOld := beforeLeft right later
      omega
    · omega

end Zetesis.DeltaJoins
