import Std

/-!
# Observations along a charged computation

A charge admits a half-open range of zero-based operation numbers. An
observation is due when that range contains a multiple of the positive period.
This is a counting law, not a bound on elapsed time. Unmetered work, concurrent
flags, allocation and the implementation's exit settlement remain separate.
-/
namespace Zetesis.ChargeCadence

/-- A charged range contains an observation boundary. -/
def PollDue (period charged amount : Nat) : Prop :=
  ∃ offset < amount, (charged + offset) % period = 0

instance (period charged amount : Nat) : Decidable (PollDue period charged amount) :=
  inferInstanceAs (Decidable (∃ offset < amount, (charged + offset) % period = 0))

/-- Distance to the first boundary at or after the current count. -/
def distance (period charged : Nat) : Nat :=
  if charged % period = 0 then 0 else period - charged % period

/-- Constant-size decision rule, requiring no addition of the two counters. -/
def dueRule (period charged amount : Nat) : Bool :=
  decide (amount ≠ 0 ∧ (charged % period = 0 ∨ period - charged % period < amount))

/-- A zero charge admits no operation and requires no observation. -/
theorem pollDue_zero (period charged : Nat) : ¬ PollDue period charged 0 := by
  rintro ⟨offset, below, _⟩
  omega

/-- A positive period puts the next boundary strictly within one period. -/
theorem fewer_than_interval (period charged : Nat) (positive : 0 < period) :
    distance period charged < period := by
  have remainder := Nat.mod_lt charged positive
  simp only [distance]
  split <;> omega

/-- Advancing by the distance reaches a multiple of the period. -/
theorem distance_reaches_boundary (period charged : Nat) (positive : 0 < period) :
    (charged + distance period charged) % period = 0 := by
  have remainder := Nat.mod_lt charged positive
  by_cases aligned : charged % period = 0
  · simp [distance, aligned]
  · have short : period - charged % period < period := by omega
    rw [Nat.add_mod]
    simp only [distance, aligned, ↓reduceIte, Nat.mod_eq_of_lt short]
    have sum : charged % period + (period - charged % period) = period := by omega
    rw [sum, Nat.mod_self]

/-- No boundary occurs before the stated distance. -/
theorem distance_minimal (period charged offset : Nat) (positive : 0 < period)
    (boundary : (charged + offset) % period = 0) :
    distance period charged ≤ offset := by
  by_cases aligned : charged % period = 0
  · simp [distance, aligned]
  · by_cases enough : distance period charged ≤ offset
    · exact enough
    apply False.elim
    have tooSmall := enough
    have remainder := Nat.mod_lt charged positive
    have short : offset < period - charged % period := by
      simp only [distance, aligned, ↓reduceIte] at tooSmall
      omega
    have offsetBound : offset < period := by omega
    have sumBound : charged % period + offset < period := by omega
    rw [Nat.add_mod, Nat.mod_eq_of_lt offsetBound, Nat.mod_eq_of_lt sumBound] at boundary
    omega

/-- The range decision agrees with the existence of a boundary. The next
boundary witnesses the forward direction; minimality proves the reverse. -/
theorem dueRule_iff (period charged amount : Nat) (positive : 0 < period) :
    dueRule period charged amount = true ↔ PollDue period charged amount := by
  have exactDistance : PollDue period charged amount ↔ distance period charged < amount := by
    constructor
    · rintro ⟨offset, inside, boundary⟩
      have first := distance_minimal period charged offset positive boundary
      omega
    · intro inside
      exact ⟨distance period charged, inside, distance_reaches_boundary period charged positive⟩
  rw [exactDistance]
  simp only [dueRule, decide_eq_true_eq, distance]
  split <;> omega

/-- At least one charge in the sequence contains an observation boundary. -/
def Observes (period charged : Nat) : List Nat → Prop
  | [] => False
  | amount :: rest => PollDue period charged amount ∨ Observes period (charged + amount) rest

/-- Consecutive charged ranges cover every offset below their total. Split at
one charge: the offset is either inside it or in the remaining suffix. -/
theorem observes_covered_boundary (period charged offset : Nat) (charges : List Nat)
    (inside : offset < charges.sum) (boundary : (charged + offset) % period = 0) :
    Observes period charged charges := by
  induction charges generalizing charged offset with
  | nil => simp at inside
  | cons amount rest ih =>
    by_cases first : offset < amount
    · exact Or.inl ⟨offset, first, boundary⟩
    · have remaining : offset - amount < rest.sum := by simp only [List.sum_cons] at inside; omega
      have same : charged + amount + (offset - amount) = charged + offset := by omega
      exact Or.inr (ih (charged + amount) (offset - amount) remaining (same ▸ boundary))

/-- A sequence reaching the first boundary observes there or earlier. That
boundary is fewer than one period of charged operations away; intervening
zero charges do not alter the result. No wall-clock latency is asserted. -/
theorem observes_next_multiple (period charged : Nat) (charges : List Nat)
    (positive : 0 < period) (reaches : distance period charged < charges.sum) :
    Observes period charged charges ∧ distance period charged < period := by
  exact ⟨observes_covered_boundary period charged (distance period charged) charges reaches
    (distance_reaches_boundary period charged positive), fewer_than_interval period charged positive⟩

end Zetesis.ChargeCadence
