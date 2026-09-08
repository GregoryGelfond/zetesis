import Zetesis.AggregateConsumers
import Zetesis.UniversalConditionals

/-!
# Universal conditionals over completed outer values

A completed outer binding selects an aggregate equality, ordinary activation
and a complete finite family of conditional implications. Consuming the binding
does not establish its equality or the truth of any possible local witness.
The original and frozen laws retain those obligations separately.

These laws assume that the supplied row family covers the intended local
bindings. They do not establish source safety, Rust proposal or join coverage,
scoped variable allocation, support completion, or resource-limit refinement.
Empty rows mean completed empty enumeration, never an interrupted prefix.
-/

namespace Zetesis.ConditionalConsumers
open Ferraris UniversalConditionals
universe u v
variable {A : Type u} {V : Type v}

/-- A selected proposal retains its equality and all conditional implications. -/
def body (equality activation : Formula A) (rows : List (Instance A)) : Formula A :=
  .conj (.conj equality activation) (universal rows)

/-- Possibility of a row cannot replace the truth of its antecedent. -/
theorem original_body (M : Atoms A) (equality activation : Formula A)
    (rows : List (Instance A)) :
    Satisfies M (body equality activation rows) ↔
      (Satisfies M equality ∧ Satisfies M activation) ∧
        ∀ row ∈ rows, Satisfies M row.condition → Satisfies M row.consequent := by
  change (Satisfies M equality ∧ Satisfies M activation) ∧
    Satisfies M (universal rows) ↔ _
  exact and_congr Iff.rfl (original_semantics M rows)

/-- Equality and activation are tested in the reduct. Each conditional retains
    both its original implication and its implication between frozen formulas.
    Consequents can contain arbitrary default-negation polarity. -/
theorem frozen_body (M J : Atoms A) (equality activation : Formula A)
    (rows : List (Instance A)) :
    Satisfies J (Reduct M (body equality activation rows)) ↔
      (Satisfies J (Reduct M equality) ∧ Satisfies J (Reduct M activation)) ∧
        ∀ row ∈ rows,
          (Satisfies M row.condition → Satisfies M row.consequent) ∧
          (Satisfies J (Reduct M row.condition) →
            Satisfies J (Reduct M row.consequent)) := by
  rw [body, RuleFactorization.reduct_conj, RuleFactorization.reduct_conj]
  exact and_congr Iff.rfl (frozen_semantics M J rows)

/-- A completed empty local family removes only the conditional obligation;
    it does not assert the aggregate equality or ordinary activation. -/
theorem empty_body_equivalent (equality activation : Formula A) :
    ChoiceIntervals.Equivalent (body equality activation [])
      (.conj equality activation) := by
  constructor
  · intro M
    simp only [original_body, List.not_mem_nil, false_implies, implies_true,
      and_true, Satisfies]
  · intro M J
    rw [frozen_body, RuleFactorization.reduct_conj]
    simp only [List.not_mem_nil, false_implies, implies_true, and_true]

/-- An unrealized proposal leaves its whole rule vacuous under every frozen
    interpretation, even when its local conditional family is nonempty. -/
theorem unrealized_rule_frozen (M J : Atoms A) (equality activation head : Formula A)
    (rows : List (Instance A)) (unrealized : ¬ Satisfies M equality) :
    Satisfies J (Reduct M (.imp (body equality activation rows) head)) := by
  have inactive : ¬ Satisfies M (body equality activation rows) := by
    intro holds
    exact unrealized ((original_body M equality activation rows).mp holds).1.1
  have reduct_inactive : ¬ Satisfies J (Reduct M (body equality activation rows)) := by
    rw [RuleFactorization.false_reduct M _ inactive]
    exact id
  rw [RuleFactorization.reduct_imp]
  exact ⟨fun holds => False.elim (inactive holds),
    fun holds => False.elim (reduct_inactive holds)⟩

/-- A complete finite proposal carrier retains the selected conditional family
    paired with the actual value. This does not infer carrier completeness from
    successful evaluation of a prefix. -/
theorem covered_conditionals (carrier : List V) (keep : V → Bool)
    (rows : V → List (Instance A)) (actual : V)
    (covered : actual ∈ carrier) (accepted : keep actual = true) :
    (actual, rows actual) ∈ AggregateConsumers.substitutions carrier keep rows :=
  AggregateConsumers.covered_consumer carrier keep rows actual covered accepted

end Zetesis.ConditionalConsumers
