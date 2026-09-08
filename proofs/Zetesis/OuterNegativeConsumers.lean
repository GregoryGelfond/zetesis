import Zetesis.ChoiceIntervals

/-!
# Negative gates over completed outer values

Once a complete value binding identifies its formulas, a negative outer gate
reads the frozen candidate. It does not read possible support or bind another
value. The original aggregate equality and all other activation formulas remain
in the body. Anonymous arguments denote a finite existential disjunction; its
negative and double-negative gates quantify over the complete matching family.

These formula laws assume that source binding and projection have already
identified the correct formulas. They do not prove Rust admission, safety,
support completeness, cursor scheduling, argument evaluation or resource bounds.
No correspondence between Rust and these formulas is established here.
-/

namespace Zetesis.OuterNegativeConsumers
open Ferraris
universe u
variable {A : Type u}

/-- A default-negative body gate preserves the frozen aggregate equality and
    activation, while testing its selected formula only in the candidate. -/
theorem negative_body_frozen (M J : Atoms A)
    (equality activation gate : Formula A) :
    Satisfies J (Reduct M (.conj (.conj equality activation) (Neg gate))) ↔
      (Satisfies J (Reduct M equality) ∧ Satisfies J (Reduct M activation)) ∧
        ¬ Satisfies M gate := by
  rw [RuleFactorization.reduct_conj, RuleFactorization.reduct_conj,
    ChoiceIntervals.negation_frozen]

/-- Double negation also freezes its test in the candidate; it does not require
    the gate to hold again in the smaller reduct interpretation. -/
theorem double_negative_body_frozen (M J : Atoms A)
    (equality activation gate : Formula A) :
    Satisfies J (Reduct M (.conj (.conj equality activation) (Neg (Neg gate)))) ↔
      (Satisfies J (Reduct M equality) ∧ Satisfies J (Reduct M activation)) ∧
        Satisfies M gate := by
  rw [RuleFactorization.reduct_conj, RuleFactorization.reduct_conj,
    double_neg_formula_reduct]

/-- Negative projection requires the absence of every actual matching witness;
    a possible witness whose formula is false does not block the gate. -/
theorem negative_projection_frozen (M J : Atoms A) (witnesses : List (Formula A)) :
    Satisfies J (Reduct M (Neg (RuleFactorization.any witnesses))) ↔
      ∀ formula ∈ witnesses, ¬ Satisfies M formula := by
  rw [ChoiceIntervals.negation_frozen, RuleFactorization.satisfies_any]
  constructor
  · intro absent formula member truth
    exact absent ⟨formula, member, truth⟩
  · intro absent ⟨formula, member, truth⟩
    exact absent formula member truth

/-- A double-negative projection needs one actual witness in the frozen
    candidate, including when other possible witnesses are absent. -/
theorem double_negative_projection_frozen (M J : Atoms A)
    (witnesses : List (Formula A)) :
    Satisfies J (Reduct M (Neg (Neg (RuleFactorization.any witnesses)))) ↔
      ∃ formula ∈ witnesses, Satisfies M formula := by
  rw [double_neg_formula_reduct, RuleFactorization.satisfies_any]

end Zetesis.OuterNegativeConsumers
