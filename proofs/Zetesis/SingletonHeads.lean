import Zetesis.NegativeHeads

/-!
# Singleton signed heads

A singleton rule preserves its literal as the consequent of the original
implication. In particular, double default negation is not erased to a positive
atom. Its reduct truth is fixed by the outer interpretation, and neither kind
of default-negated head supplies positive support.

These ground laws specialize `NegativeHeads` and expose the correspondence used
by singleton admission. They do not verify source binding, finite interval
expansion, the Rust compiler or its resource accounting.
-/

namespace Zetesis.SingletonHeads

open Ferraris NegativeHeads

universe u
variable {α : Type u}

/-- A signed singleton consequent with an arbitrary formula body. -/
def singleton (body : Formula α) (consequent : Literal α) : Rule α :=
  { body, literals := [consequent] }

/-- A one-element disjunction has the original truth of its literal. -/
theorem singleton_head_truth (M : Atoms α) (consequent : Literal α) :
    Satisfies M (head [consequent]) ↔ Satisfies M (literal consequent) := by
  simp only [head, List.map_cons, List.map_nil, RuleFactorization.satisfies_any,
    List.mem_singleton, exists_eq_left]

/-- Singleton correspondence also holds in every frozen interpretation. -/
theorem singleton_head_reduct (M J : Atoms α) (consequent : Literal α) :
    Satisfies J (Reduct M (head [consequent])) ↔
      Satisfies J (Reduct M (literal consequent)) := by
  simp only [head, List.map_cons, List.map_nil, RuleFactorization.reduct_any,
    List.mem_singleton, exists_eq_left]

/-- Singleton compilation retains the original implication. -/
theorem singleton_rule_truth (M : Atoms α) (body : Formula α)
    (consequent : Literal α) :
    Satisfies M (ruleFormula (singleton body consequent)) ↔
      (Satisfies M body → Satisfies M (literal consequent)) := by
  change (Satisfies M body → Satisfies M (head [consequent])) ↔ _
  rw [singleton_head_truth]

/-- The frozen implication requires original truth as well as its reduct test. -/
theorem singleton_rule_reduct (M J : Atoms α) (body : Formula α)
    (consequent : Literal α) :
    Satisfies J (Reduct M (ruleFormula (singleton body consequent))) ↔
      Satisfies M (ruleFormula (singleton body consequent)) ∧
      (Satisfies J (Reduct M body) → Satisfies J (Reduct M (literal consequent))) := by
  change Satisfies J (Reduct M (.imp body (head [consequent]))) ↔ _
  rw [RuleFactorization.reduct_imp, singleton_head_reduct]
  rfl

/-- Default negation in the head tests absence in the frozen outer model. -/
theorem negative_singleton_reduct (M J : Atoms α) (body : Formula α) (a : α) :
    Satisfies J (Reduct M (ruleFormula (singleton body (.negative a)))) ↔
      Satisfies M (ruleFormula (singleton body (.negative a))) ∧
      (Satisfies J (Reduct M body) → ¬ M a) := by
  rw [singleton_rule_reduct, negative_literal_reduct]

/-- Double default negation tests outer membership, not membership in J. -/
theorem double_negative_singleton_reduct (M J : Atoms α) (body : Formula α) (a : α) :
    Satisfies J (Reduct M (ruleFormula (singleton body (.doubleNegative a)))) ↔
      Satisfies M (ruleFormula (singleton body (.doubleNegative a))) ∧
      (Satisfies J (Reduct M body) → M a) := by
  rw [singleton_rule_reduct, double_negative_literal_reduct]

/-- A negative singleton does not produce any atom, even when its body holds. -/
theorem negative_singleton_has_no_producer (M : Atoms α) (body : Formula α) (a b : α) :
    ¬ HasProducer M [singleton body (.negative a)] b := by
  simp [HasProducer, singleton]

/-- The same absence of support holds for a double-negative singleton. -/
theorem double_negative_singleton_has_no_producer (M : Atoms α) (body : Formula α)
    (a b : α) : ¬ HasProducer M [singleton body (.doubleNegative a)] b := by
  simp [HasProducer, singleton]

end Zetesis.SingletonHeads
