import Zetesis.UniversalConditionals

/-!
# Finite alternatives inside universal body conditionals

Each completed condition row has one antecedent and a finite disjunction of
consequent alternatives. The outer quantification is universal; the inner one is
existential. Each alternative is an already signed Ferraris formula, so default
negation stays inside that disjunction. An empty completed alternative list is
false, while an empty completed list of condition rows is true.

The laws quantify over supplied complete lists. They neither prove completeness
of source joins nor authorize acceptance after interrupted enumeration. Local
witnesses have already selected the alternatives: they are not extra premises
or supporting rules. Substitution scope, projection analysis, machine arithmetic,
resource accounting and the Rust compiler remain separate proof obligations.
-/

namespace Zetesis.ConsequentAlternatives

universe u
variable {α : Type u}
open Ferraris
open RuleFactorization (any)

/-- One completed condition assignment and all its admitted signed consequents. -/
structure Row (α : Type u) where
  condition : Formula α
  alternatives : List (Formula α)

/-- Finite conditional semantics retains every antecedent in its implication. -/
def body (rows : List (Row α)) : Formula α :=
  UniversalConditionals.universal
    (rows.map (fun row => ⟨row.condition, any row.alternatives⟩))

/-- Every active condition row needs at least one true signed alternative. -/
theorem original_semantics (M : Atoms α) (rows : List (Row α)) :
    Satisfies M (body rows) ↔
      ∀ row ∈ rows, Satisfies M row.condition →
        ∃ alternative ∈ row.alternatives, Satisfies M alternative := by
  simp [body, UniversalConditionals.original_semantics, RuleFactorization.satisfies_any]

/-- Original truth and frozen truth are separate requirements. A condition true
    at M remains an antecedent when its reduct is evaluated at J. -/
theorem frozen_semantics (M J : Atoms α) (rows : List (Row α)) :
    Satisfies J (Reduct M (body rows)) ↔
      ∀ row ∈ rows,
        (Satisfies M row.condition →
          ∃ alternative ∈ row.alternatives, Satisfies M alternative) ∧
        (Satisfies J (Reduct M row.condition) →
          ∃ alternative ∈ row.alternatives, Satisfies J (Reduct M alternative)) := by
  simp [body, UniversalConditionals.frozen_semantics,
    RuleFactorization.satisfies_any, RuleFactorization.reduct_any]

/-- If every supplied consequent has candidate-only frozen truth, the complete
    conditional has that property too. This premise holds for default-negated
    projections, but not for arbitrary positive consequents. The implication
    antecedents are retained: their reduct truth entails their original truth. -/
theorem candidate_consequents (M J : Atoms α) (rows : List (Row α))
    (frozen : ∀ row ∈ rows, ∀ alternative ∈ row.alternatives,
      Satisfies J (Reduct M alternative) ↔ Satisfies M alternative) :
    Satisfies J (Reduct M (body rows)) ↔ Satisfies M (body rows) := by
  rw [frozen_semantics, original_semantics]
  constructor
  · intro reduced row member
    exact (reduced row member).1
  · intro original row member
    refine ⟨original row member, ?_⟩
    intro condition
    have active : Satisfies M row.condition := by
      classical
      by_cases original : Satisfies M row.condition
      · exact original
      · rw [RuleFactorization.false_reduct M row.condition original] at condition
        exact False.elim condition
    obtain ⟨alternative, present, truth⟩ := original row member active
    exact ⟨alternative, present, (frozen row member alternative present).mpr truth⟩

/-- Vacuity assumes that the finite condition domain has been exhausted. -/
theorem empty_rows_original (M : Atoms α) :
    Satisfies M (body ([] : List (Row α))) := by
  simp [original_semantics]

/-- The same completed empty domain is vacuous in every frozen interpretation. -/
theorem empty_rows_frozen (M J : Atoms α) :
    Satisfies J (Reduct M (body ([] : List (Row α)))) := by
  simp [frozen_semantics]

/-- With no consequent alternatives, this row requires its condition to be false. -/
theorem empty_alternatives_original (M : Atoms α) (condition : Formula α) :
    Satisfies M (body [⟨condition, []⟩]) ↔ ¬ Satisfies M condition := by
  simp [original_semantics]

/-- Original falsity freezes the empty-alternative row to a vacuous implication;
    no assumption that J is a subset of M is required. -/
theorem empty_alternatives_frozen (M J : Atoms α) (condition : Formula α) :
    Satisfies J (Reduct M (body [⟨condition, []⟩])) ↔ ¬ Satisfies M condition := by
  have inactive_reduct : ¬ Satisfies M condition → ¬ Satisfies J (Reduct M condition) := by
    intro inactive
    simp [RuleFactorization.false_reduct M condition inactive, Satisfies]
  simp only [frozen_semantics, List.mem_singleton, forall_eq, List.not_mem_nil,
    false_and, exists_false, imp_false]
  exact ⟨And.left, fun inactive => ⟨inactive, inactive_reduct inactive⟩⟩

/-- Moving default negation outside a disjunction changes original truth. This
    concrete counterexample rules out that tempting source simplification. -/
theorem polarity_placement_changes_truth :
    let M : Atoms Bool := fun atom => atom = true
    Satisfies M (any [.imp (.atom true) .bot, .imp (.atom false) .bot]) ∧
      ¬ Satisfies M (.imp (any [.atom true, .atom false]) .bot) := by
  simp [any, Satisfies]

/-- Replacing the signed formulas by an extensionally equal finite collection
    preserves membership of the enclosing rule in an arbitrary theory. This
    permits reordering and duplicate elimination, but assumes full coverage of
    identical formulas rather than merely equal truth at the current candidate. -/
theorem alternative_collection_keeps_stability (M : Atoms α)
    (condition remaining head : Formula α) (left right : List (Formula α))
    (coverage : ∀ formula, formula ∈ left ↔ formula ∈ right) (context : Theory α) :
    Stable M (.imp (.conj remaining (body [⟨condition, left⟩])) head :: context) ↔
      Stable M (.imp (.conj remaining (body [⟨condition, right⟩])) head :: context) := by
  have original : Satisfies M (body [⟨condition, left⟩]) ↔
      Satisfies M (body [⟨condition, right⟩]) := by
    simp only [original_semantics, List.mem_singleton, forall_eq, coverage]
  have frozen (J : Atoms α) : Satisfies J (Reduct M (body [⟨condition, left⟩])) ↔
      Satisfies J (Reduct M (body [⟨condition, right⟩])) := by
    simp only [frozen_semantics, List.mem_singleton, forall_eq, coverage]
  have rule_original : Satisfies M (.imp (.conj remaining (body [⟨condition, left⟩])) head) ↔
      Satisfies M (.imp (.conj remaining (body [⟨condition, right⟩])) head) := by
    simp only [Satisfies, original]
  have rule_frozen (J : Atoms α) :
      Satisfies J (Reduct M (.imp (.conj remaining (body [⟨condition, left⟩])) head)) ↔
      Satisfies J (Reduct M (.imp (.conj remaining (body [⟨condition, right⟩])) head)) := by
    simp only [RuleFactorization.reduct_imp, RuleFactorization.reduct_conj,
      rule_original, frozen]
  simp only [Stable, models_cons, ReductTheory, List.map_cons, rule_original, rule_frozen]

end Zetesis.ConsequentAlternatives
