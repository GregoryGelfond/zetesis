import Zetesis.ConstrainedPositive
import Zetesis.ThresholdConnectives

/-!
# Complete pair exclusions as an at-most-one constraint

An edge population covers every unordered pair of distinct listed indices and
contains no other pair. Its negative conjunctions have exactly the original
truth of the negated unit threshold at two. Negative constraints freeze to their
original truth, so this replacement also preserves every arbitrary M/J reduct
and answer-set membership in an unchanged surrounding theory.

Indices are distinct; edges may repeat and may use either orientation. Conditions
may be arbitrary formulas. Rust must separately establish exact atom identity,
complete edge coverage, ordinary source validation, provenance and bounded
allocation/work. This module does not verify the Rust sorting or union-find
certificate, nor native threshold storage or propagation.
-/

namespace Zetesis.PairwiseExclusions

open Ferraris ThresholdReduct ThresholdConnectives

universe u v
variable {α : Type u} {β : Type v}

/-- Two unit contributions require two distinct selected indices when the
    supplied index list is duplicate-free. A true head needs one tail witness;
    a false head leaves the same two-witness obligation in the tail. -/
theorem masked_unit_sum_two (mask : β → Bool) (indices : List β) :
    indices.Nodup → (2 ≤ maskedSum mask (unitRow indices) ↔
      ∃ left ∈ indices, ∃ right ∈ indices,
        left ≠ right ∧ mask left = true ∧ mask right = true) := by
  induction indices with
  | nil => simp [unitRow, maskedSum, queries, Thresholds.enabledSum]
  | cons head rest ih =>
    intro distinct
    have headAbsent : head ∉ rest := (List.nodup_cons.mp distinct).1
    have tailLaw := ih (List.nodup_cons.mp distinct).2
    have positive := masked_unit_sum_positive mask rest
    change 2 ≤ (if mask head then 1 else 0) + maskedSum mask (unitRow rest) ↔ _
    cases enabled : mask head with
    | false =>
      simp only [Bool.false_eq_true, ↓reduceIte, Nat.zero_add]
      constructor
      · intro reached
        obtain ⟨left, leftIn, right, rightIn, unequal, leftTrue, rightTrue⟩ :=
          tailLaw.mp reached
        exact ⟨left, List.mem_cons_of_mem head leftIn,
          right, List.mem_cons_of_mem head rightIn, unequal, leftTrue, rightTrue⟩
      · rintro ⟨left, leftIn, right, rightIn, unequal, leftTrue, rightTrue⟩
        have leftRest : left ∈ rest := by
          rcases List.mem_cons.mp leftIn with equal | present
          · subst left
            simp [enabled] at leftTrue
          · exact present
        have rightRest : right ∈ rest := by
          rcases List.mem_cons.mp rightIn with equal | present
          · subst right
            simp [enabled] at rightTrue
          · exact present
        exact tailLaw.mpr ⟨left, leftRest, right, rightRest, unequal, leftTrue, rightTrue⟩
    | true =>
      simp only [↓reduceIte]
      constructor
      · intro reached
        have tailPositive : 1 ≤ maskedSum mask (unitRow rest) := by omega
        obtain ⟨right, rightIn, rightTrue⟩ := positive.mp tailPositive
        have unequal : head ≠ right := by
          intro equal
          exact headAbsent (equal.symm ▸ rightIn)
        exact ⟨head, List.mem_cons_self, right, List.mem_cons_of_mem head rightIn,
          unequal, enabled, rightTrue⟩
      · rintro ⟨left, leftIn, right, rightIn, unequal, leftTrue, rightTrue⟩
        have tailWitness : ∃ index ∈ rest, mask index = true := by
          rcases List.mem_cons.mp leftIn with equal | present
          · subst left
            rcases List.mem_cons.mp rightIn with equal | present
            · exact False.elim (unequal equal.symm)
            · exact ⟨right, present, rightTrue⟩
          · exact ⟨left, present, leftTrue⟩
        have tailPositive := positive.mpr tailWitness
        omega

/-- Every listed edge has two distinct endpoints in the supplied carrier, and
    every distinct unordered carrier pair has at least one listed orientation. -/
structure Coverage (indices : List β) (edges : List (β × β)) : Prop where
  endpoints : ∀ edge ∈ edges,
    edge.1 ∈ indices ∧ edge.2 ∈ indices ∧ edge.1 ≠ edge.2
  pairs : ∀ left ∈ indices, ∀ right ∈ indices, left ≠ right →
    (left, right) ∈ edges ∨ (right, left) ∈ edges

/-- The body of each ordinary pairwise exclusion, retaining every occurrence. -/
def bodies (conditions : β → Formula α) (edges : List (β × β)) : List (Formula α) :=
  edges.map (fun edge => .conj (conditions edge.1) (conditions edge.2))

/-- Complete pair exclusions and a native unit threshold below two have the
    same original truth. Sound endpoints justify the reverse direction; complete
    unordered coverage supplies the violated original constraint in the forward
    direction. Neither common provenance nor graph connectedness is a premise. -/
theorem original (M : Atoms α) (conditions : β → Formula α)
    (indices : List β) (edges : List (β × β))
    (distinct : indices.Nodup) (covered : Coverage indices edges) :
    Models M ((bodies conditions edges).map Ferraris.Neg) ↔
      Satisfies M (Ferraris.Neg (formula conditions (unitRow indices) 2)) := by
  classical
  have thresholdWitness : Satisfies M (formula conditions (unitRow indices) 2) ↔
      ∃ left ∈ indices, ∃ right ∈ indices,
        left ≠ right ∧ Satisfies M (conditions left) ∧ Satisfies M (conditions right) := by
    rw [ThresholdReduct.original, masked_unit_sum_two (active M conditions) indices distinct]
    simp only [active, decide_eq_true_eq]
  constructor
  · intro allConstraints thresholdTrue
    obtain ⟨left, leftIn, right, rightIn, unequal, leftTrue, rightTrue⟩ :=
      thresholdWitness.mp thresholdTrue
    rcases covered.pairs left leftIn right rightIn unequal with forward | backward
    · exact allConstraints _ (List.mem_map.mpr ⟨_,
        List.mem_map.mpr ⟨(left, right), forward, rfl⟩, rfl⟩) ⟨leftTrue, rightTrue⟩
    · exact allConstraints _ (List.mem_map.mpr ⟨_,
        List.mem_map.mpr ⟨(right, left), backward, rfl⟩, rfl⟩) ⟨rightTrue, leftTrue⟩
  · intro native constraint member
    obtain ⟨body, bodyIn, rfl⟩ := List.mem_map.mp member
    obtain ⟨edge, edgeIn, rfl⟩ := List.mem_map.mp bodyIn
    have endpoints := covered.endpoints edge edgeIn
    intro pairTrue
    exact native (thresholdWitness.mpr ⟨edge.1, endpoints.1, edge.2,
      endpoints.2.1, endpoints.2.2, pairTrue.1, pairTrue.2⟩)

/-- Every negative constraint freezes to its original truth at the candidate,
    including at tested interpretations that are not candidate subsets. -/
theorem constraint_frozen_iff (M J : Atoms α) (body : Formula α) :
    Satisfies J (Reduct M (Ferraris.Neg body)) ↔ Satisfies M (Ferraris.Neg body) := by
  classical
  constructor
  · intro reduced
    by_cases satisfied : Satisfies M (Ferraris.Neg body)
    · exact satisfied
    · rw [ConstrainedPositive.reduct_of_false M (Ferraris.Neg body) satisfied] at reduced
      exact False.elim reduced
  · exact ConstrainedPositive.constraint_frozen M J body

/-- A whole negative family has the same candidate-only frozen truth. -/
theorem constraints_frozen_iff (M J : Atoms α) (family : List (Formula α)) :
    Models J (ReductTheory M (family.map Ferraris.Neg)) ↔
      Models M (family.map Ferraris.Neg) := by
  constructor
  · intro reduced constraint member
    obtain ⟨body, bodyIn, rfl⟩ := List.mem_map.mp member
    exact (constraint_frozen_iff M J body).mp (reduced _
      (List.mem_map.mpr ⟨_, List.mem_map.mpr ⟨body, bodyIn, rfl⟩, rfl⟩))
  · exact ConstrainedPositive.constraints_frozen M J family

/-- Certified replacement preserves every frozen M/J reading. The native
    threshold needs no special reduct rewrite because its enclosing constraint
    freezes entirely according to the already-proved original truth. -/
theorem frozen (M J : Atoms α) (conditions : β → Formula α)
    (indices : List β) (edges : List (β × β))
    (distinct : indices.Nodup) (covered : Coverage indices edges) :
    Models J (ReductTheory M ((bodies conditions edges).map Ferraris.Neg)) ↔
      Satisfies J (Reduct M (Ferraris.Neg (formula conditions (unitRow indices) 2))) := by
  rw [constraints_frozen_iff, constraint_frozen_iff]
  exact original M conditions indices edges distinct covered

/-- Complete pair replacement preserves answer sets in an arbitrary surrounding
    theory. Both constraint families filter exactly the same original answer
    sets of that context, by the general satisfied-constraint extension law. -/
theorem stable_in_context (M : Atoms α) (context : Theory α)
    (conditions : β → Formula α) (indices : List β) (edges : List (β × β))
    (distinct : indices.Nodup) (covered : Coverage indices edges) :
    Stable M (context ++ (bodies conditions edges).map Ferraris.Neg) ↔
      Stable M (context ++ [Ferraris.Neg (formula conditions (unitRow indices) 2)]) := by
  have originalLaw := original M conditions indices edges distinct covered
  have nativeLaw := ConstrainedPositive.stable_append_constraints context
    [formula conditions (unitRow indices) 2] M
  simp only [List.map_cons, List.map_nil] at nativeLaw
  rw [ConstrainedPositive.stable_append_constraints, nativeLaw]
  simp only [Models, List.mem_singleton, forall_eq] at originalLaw ⊢
  exact and_congr Iff.rfl originalLaw

end Zetesis.PairwiseExclusions
