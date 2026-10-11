import Zetesis.ThresholdReduct
import Zetesis.OperandArena

/-!
# Finite Boolean connectives as unit-weight thresholds

A conjunction of n conditions requires all n unit contributions; a disjunction
requires one. These original-satisfaction laws remain exact when the supplied
condition list is sorted or deduplicated, provided its membership is unchanged.
Conditions may be arbitrary formulas. Distinctness is not a semantic premise:
repeated positions contribute separately and repeat the same truth requirement.

The laws supply a semantic bridge for original-candidate consequences. They do
not prescribe a Rust row representation, recognize equal condition IDs, verify
normalization or establish a propagation algorithm. In particular, matching a
Boolean unit row with a weighted row requires matching complete coefficient and
condition pairs; common condition membership alone is insufficient. Frozen
propagation and executable correspondence remain separate obligations.
-/

namespace Zetesis.ThresholdConnectives

open Ferraris ThresholdReduct

universe u v
variable {α : Type u} {β : Type v}

/-- Give each listed condition one contribution, retaining order and repetition. -/
def unitRow (indices : List β) : List (Nat × β) :=
  indices.map (fun index => (1, index))

/-- A unit row's selected sum cannot exceed its number of positions. Each head
    contributes at most one, and induction bounds the remaining positions. -/
theorem masked_unit_sum_le_length (mask : β → Bool) (indices : List β) :
    maskedSum mask (unitRow indices) ≤ indices.length := by
  induction indices with
  | nil => simp [unitRow, maskedSum, queries, Thresholds.enabledSum]
  | cons index rest ih =>
    have headBound : (if mask index then 1 else 0) ≤ 1 := by
      cases mask index <;> decide
    change (if mask index then 1 else 0) + maskedSum mask (unitRow rest) ≤
      rest.length + 1
    omega

/-- A unit row reaches its full length exactly when every listed condition is
    true. The tail cannot compensate for a false head: its sum is at most its
    length. A true head leaves exactly the same full-length obligation on the
    tail. The empty conjunction therefore requires zero contributions. -/
theorem masked_unit_sum_full (mask : β → Bool) (indices : List β) :
    indices.length ≤ maskedSum mask (unitRow indices) ↔
      ∀ index ∈ indices, mask index = true := by
  induction indices with
  | nil => simp [unitRow, maskedSum, queries, Thresholds.enabledSum]
  | cons index rest ih =>
    have tailBound : maskedSum mask (unitRow rest) ≤ rest.length :=
      masked_unit_sum_le_length mask rest
    have step : rest.length + 1 ≤
        (if mask index then 1 else 0) + maskedSum mask (unitRow rest) ↔
        mask index = true ∧ rest.length ≤ maskedSum mask (unitRow rest) := by
      cases mask index <;> simp <;> omega
    change rest.length + 1 ≤
      (if mask index then 1 else 0) + maskedSum mask (unitRow rest) ↔
        ∀ element ∈ index :: rest, mask element = true
    rw [step, ih]
    simp only [List.mem_cons, forall_eq_or_imp]

/-- A unit row reaches one exactly when a listed condition is true. A true head
    is a witness; otherwise the tail must provide one. With no positions the
    sum is zero, so the empty disjunction remains false. -/
theorem masked_unit_sum_positive (mask : β → Bool) (indices : List β) :
    1 ≤ maskedSum mask (unitRow indices) ↔
      ∃ index ∈ indices, mask index = true := by
  induction indices with
  | nil => simp [unitRow, maskedSum, queries, Thresholds.enabledSum]
  | cons index rest ih =>
    have step : 1 ≤ (if mask index then 1 else 0) + maskedSum mask (unitRow rest) ↔
        mask index = true ∨ 1 ≤ maskedSum mask (unitRow rest) := by
      cases mask index <;> simp <;> omega
    change 1 ≤ (if mask index then 1 else 0) + maskedSum mask (unitRow rest) ↔
      ∃ element ∈ index :: rest, mask element = true
    rw [step, ih]
    simp only [List.mem_cons, or_and_right, exists_or, exists_eq_left]

/-- A finite conjunction has the original truth of the unit threshold on any
    list with the same condition membership, at that list's length. Membership
    equivalence transports the universal truth requirement; the full unit-sum
    law and the existing threshold semantics then give the threshold formula.
    Sorting and deduplication need no stronger semantic premise. -/
theorem conjunction_original (M : Atoms α) (conditions : β → Formula α)
    (source normalized : List β)
    (members : ∀ index, index ∈ source ↔ index ∈ normalized) :
    Satisfies M (NormalFerraris.conjunction (source.map conditions)) ↔
      Satisfies M (formula conditions (unitRow normalized) normalized.length) := by
  classical
  have sameRequirements : (∀ index ∈ source, Satisfies M (conditions index)) ↔
      ∀ index ∈ normalized, Satisfies M (conditions index) := by
    constructor
    · intro holds index member
      exact holds index ((members index).mpr member)
    · intro holds index member
      exact holds index ((members index).mp member)
  calc
    Satisfies M (NormalFerraris.conjunction (source.map conditions)) ↔
        ∀ index ∈ source, Satisfies M (conditions index) := by
      simp only [NormalFerraris.satisfies_conjunction, List.forall_mem_map]
    _ ↔ ∀ index ∈ normalized, Satisfies M (conditions index) := sameRequirements
    _ ↔ normalized.length ≤ maskedSum (active M conditions) (unitRow normalized) := by
      simpa only [active, decide_eq_true_eq] using
        (masked_unit_sum_full (active M conditions) normalized).symm
    _ ↔ Satisfies M (formula conditions (unitRow normalized) normalized.length) :=
      (ThresholdReduct.original M conditions (unitRow normalized) normalized.length).symm

/-- A finite disjunction has the original truth of the unit threshold at one
    on any list with the same condition membership. Membership equivalence
    preserves its possible witness, and the positive unit-sum law identifies
    exactly the same witnesses in the threshold. Empty lists and repeated
    conditions retain their usual disjunction meaning. -/
theorem disjunction_original (M : Atoms α) (conditions : β → Formula α)
    (source normalized : List β)
    (members : ∀ index, index ∈ source ↔ index ∈ normalized) :
    Satisfies M (Refinement.OperandArena.disjunction (source.map conditions)) ↔
      Satisfies M (formula conditions (unitRow normalized) 1) := by
  classical
  have sameWitnesses : (∃ index ∈ source, Satisfies M (conditions index)) ↔
      ∃ index ∈ normalized, Satisfies M (conditions index) := by
    constructor
    · rintro ⟨index, member, holds⟩
      exact ⟨index, (members index).mp member, holds⟩
    · rintro ⟨index, member, holds⟩
      exact ⟨index, (members index).mpr member, holds⟩
  calc
    Satisfies M (Refinement.OperandArena.disjunction (source.map conditions)) ↔
        ∃ index ∈ source, Satisfies M (conditions index) := by
      simp only [Refinement.OperandArena.satisfies_disjunction, List.mem_map]
      constructor
      · rintro ⟨condition, ⟨index, member, rfl⟩, holds⟩
        exact ⟨index, member, holds⟩
      · rintro ⟨index, member, holds⟩
        exact ⟨conditions index, ⟨index, member, rfl⟩, holds⟩
    _ ↔ ∃ index ∈ normalized, Satisfies M (conditions index) := sameWitnesses
    _ ↔ 1 ≤ maskedSum (active M conditions) (unitRow normalized) := by
      simpa only [active, decide_eq_true_eq] using
        (masked_unit_sum_positive (active M conditions) normalized).symm
    _ ↔ Satisfies M (formula conditions (unitRow normalized) 1) :=
      (ThresholdReduct.original M conditions (unitRow normalized) 1).symm

end Zetesis.ThresholdConnectives
