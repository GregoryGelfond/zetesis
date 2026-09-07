import Zetesis.FerrarisGuards

/-!
# Finite existential body factorization

For a fixed head and two independent finite families of condition formulas,
replace all pairwise body-to-head implications by one implication whose body
conjoins the two existential disjunctions. Original truth and every frozen
reduct interpretation are preserved. Conditions are arbitrary formulas: no
classical simplification of negation or aggregates is performed.

The Cartesian-product shape is explicit in the input theory. Establishing this
shape from source variable dependencies, joins, fixed head bindings and complete
support is a separate compiler obligation. This module is not a Rust refinement.
-/

namespace Zetesis.RuleFactorization

universe u
variable {α : Type u}
open Ferraris

def any : List (Formula α) → Formula α
  | [] => .bot
  | first :: rest => .disj first (any rest)

def pairs (left right : List (Formula α)) (head : Formula α) : Theory α :=
  left.flatMap (fun a => right.map (fun b => Formula.imp (.conj a b) head))

def factor (left right : List (Formula α)) (head : Formula α) : Formula α :=
  .imp (.conj (any left) (any right)) head

theorem false_reduct (M : Atoms α) (F : Formula α) (h : ¬ Satisfies M F) :
    Reduct M F = .bot := by
  classical
  cases F with
  | atom a => simpa [Reduct, Satisfies] using h
  | bot => rfl
  | conj F G => simp only [Reduct]; rw [if_neg h]
  | disj F G => simp only [Reduct]; rw [if_neg h]
  | imp F G => simp only [Reduct]; rw [if_neg h]

theorem reduct_conj (M J : Atoms α) (F G : Formula α) :
    Satisfies J (Reduct M (.conj F G)) ↔
      Satisfies J (Reduct M F) ∧ Satisfies J (Reduct M G) := by
  classical
  by_cases hF : Satisfies M F <;> by_cases hG : Satisfies M G
  · simp [Reduct, Satisfies, hF, hG]
  · simp [Reduct, Satisfies, hF, hG, false_reduct M G hG]
  · simp [Reduct, Satisfies, hF, hG, false_reduct M F hF]
  · simp [Reduct, Satisfies, hF, hG, false_reduct M F hF]

theorem reduct_disj (M J : Atoms α) (F G : Formula α) :
    Satisfies J (Reduct M (.disj F G)) ↔
      Satisfies J (Reduct M F) ∨ Satisfies J (Reduct M G) := by
  classical
  by_cases hF : Satisfies M F <;> by_cases hG : Satisfies M G
  · simp [Reduct, Satisfies, hF, hG]
  · simp [Reduct, Satisfies, hF, hG]
  · simp [Reduct, Satisfies, hF, hG]
  · simp [Reduct, Satisfies, hF, hG, false_reduct M F hF, false_reduct M G hG]

theorem reduct_imp (M J : Atoms α) (F G : Formula α) :
    Satisfies J (Reduct M (.imp F G)) ↔
      Satisfies M (.imp F G) ∧
        (Satisfies J (Reduct M F) → Satisfies J (Reduct M G)) := by
  classical
  change Satisfies J (if Satisfies M (.imp F G) then
    Formula.imp (Reduct M F) (Reduct M G) else Formula.bot) ↔ _
  by_cases h : Satisfies M (.imp F G)
  · rw [if_pos h]
    exact ⟨fun reduced => ⟨h, reduced⟩, And.right⟩
  · rw [if_neg h]
    exact ⟨False.elim, fun result => h result.1⟩

theorem satisfies_any (M : Atoms α) (formulas : List (Formula α)) :
    Satisfies M (any formulas) ↔ ∃ F ∈ formulas, Satisfies M F := by
  induction formulas with
  | nil => simp [any, Satisfies]
  | cons F rest ih => simp [any, Satisfies, ih]

theorem reduct_any (M J : Atoms α) (formulas : List (Formula α)) :
    Satisfies J (Reduct M (any formulas)) ↔
      ∃ F ∈ formulas, Satisfies J (Reduct M F) := by
  induction formulas with
  | nil => simp [any, Reduct, Satisfies]
  | cons F rest ih => simp [any, reduct_disj, ih]

theorem models_pairs (M : Atoms α) (left right : List (Formula α)) (head : Formula α) :
    Models M (pairs left right head) ↔
      ∀ a ∈ left, ∀ b ∈ right, Satisfies M a → Satisfies M b → Satisfies M head := by
  constructor
  · intro h a ha b hb hma hmb
    exact h (.imp (.conj a b) head)
      (List.mem_flatMap.mpr ⟨a, ha, List.mem_map.mpr ⟨b, hb, rfl⟩⟩) ⟨hma, hmb⟩
  · intro h F hF
    obtain ⟨a, ha, inner⟩ := List.mem_flatMap.mp hF
    obtain ⟨b, hb, rfl⟩ := List.mem_map.mp inner
    exact fun ⟨hma, hmb⟩ => h a ha b hb hma hmb

theorem original_factorization (M : Atoms α) (left right : List (Formula α))
    (head : Formula α) :
    Models M (pairs left right head) ↔ Satisfies M (factor left right head) := by
  rw [models_pairs]
  simp only [factor, Satisfies, satisfies_any]
  constructor
  · intro h ⟨⟨a, ha, hma⟩, ⟨b, hb, hmb⟩⟩
    exact h a ha b hb hma hmb
  · intro h a ha b hb hma hmb
    exact h ⟨⟨a, ha, hma⟩, ⟨b, hb, hmb⟩⟩

theorem reduct_models_pairs (M J : Atoms α) (left right : List (Formula α))
    (head : Formula α) :
    Models J (ReductTheory M (pairs left right head)) ↔
      Models M (pairs left right head) ∧
        ∀ a ∈ left, ∀ b ∈ right,
          Satisfies J (Reduct M a) → Satisfies J (Reduct M b) →
            Satisfies J (Reduct M head) := by
  constructor
  · intro h
    have pair : ∀ a ∈ left, ∀ b ∈ right,
        Satisfies M (.imp (.conj a b) head) ∧
          (Satisfies J (Reduct M a) ∧ Satisfies J (Reduct M b) →
            Satisfies J (Reduct M head)) := by
      intro a ha b hb
      have member : Formula.imp (.conj a b) head ∈ pairs left right head :=
        List.mem_flatMap.mpr ⟨a, ha, List.mem_map.mpr ⟨b, hb, rfl⟩⟩
      have result := h (Reduct M (.imp (.conj a b) head))
        (List.mem_map.mpr ⟨_, member, rfl⟩)
      simpa only [reduct_imp, reduct_conj] using result
    exact ⟨(models_pairs M left right head).mpr
      (fun a ha b hb hma hmb => (pair a ha b hb).1 ⟨hma, hmb⟩),
      fun a ha b hb hja hjb => (pair a ha b hb).2 ⟨hja, hjb⟩⟩
  · intro ⟨original, reduct⟩ F hF
    obtain ⟨rule, member, rfl⟩ := List.mem_map.mp hF
    obtain ⟨a, ha, inner⟩ := List.mem_flatMap.mp member
    obtain ⟨b, hb, rfl⟩ := List.mem_map.mp inner
    rw [reduct_imp, reduct_conj]
    exact ⟨fun ⟨hma, hmb⟩ => (models_pairs M left right head).mp original a ha b hb hma hmb,
      fun ⟨hja, hjb⟩ => reduct a ha b hb hja hjb⟩

theorem frozen_factorization (M J : Atoms α) (left right : List (Formula α))
    (head : Formula α) :
    Models J (ReductTheory M (pairs left right head)) ↔
      Satisfies J (Reduct M (factor left right head)) := by
  rw [reduct_models_pairs, original_factorization]
  simp only [factor, reduct_imp, reduct_conj, reduct_any]
  constructor
  · intro ⟨original, reduct⟩
    exact ⟨original, fun ⟨⟨a, ha, hja⟩, ⟨b, hb, hjb⟩⟩ => reduct a ha b hb hja hjb⟩
  · intro ⟨original, reduct⟩
    exact ⟨original, fun a ha b hb hja hjb => reduct ⟨⟨a, ha, hja⟩, ⟨b, hb, hjb⟩⟩⟩

theorem models_append (M : Atoms α) (left right : Theory α) :
    Models M (left ++ right) ↔ Models M left ∧ Models M right := by
  constructor
  · intro h
    exact ⟨fun F hF => h F (List.mem_append.mpr (Or.inl hF)),
      fun F hF => h F (List.mem_append.mpr (Or.inr hF))⟩
  · intro ⟨hl, hr⟩ F hF
    rcases List.mem_append.mp hF with left | right
    · exact hl F left
    · exact hr F right

theorem original_in_context (M : Atoms α) (left right : List (Formula α))
    (head : Formula α) (context : Theory α) :
    Models M (pairs left right head ++ context) ↔
      Models M (factor left right head :: context) := by
  rw [models_append, models_cons, original_factorization]

theorem frozen_in_context (M J : Atoms α) (left right : List (Formula α))
    (head : Formula α) (context : Theory α) :
    Models J (ReductTheory M (pairs left right head ++ context)) ↔
      Models J (ReductTheory M (factor left right head :: context)) := by
  simp only [ReductTheory, List.map_append, List.map_cons]
  rw [models_append, models_cons]
  exact and_congr (frozen_factorization M J left right head) Iff.rfl

/-- The factorization may replace roots inside any unchanged surrounding theory.
    Stability is still minimality of the original frozen formula reduct. -/
theorem stable_in_context (M : Atoms α) (left right : List (Formula α))
    (head : Formula α) (context : Theory α) :
    Stable M (pairs left right head ++ context) ↔
      Stable M (factor left right head :: context) := by
  simp only [Stable, original_in_context, frozen_in_context]

end Zetesis.RuleFactorization
