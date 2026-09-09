import Zetesis.NegativeHeads

/-!
# Boolean operands in rule heads

Boolean constants coexist with signed atomic head occurrences. They have the
same truth in every original and frozen interpretation and supply no positive
atom support. Consequently the necessary-producer guard remains valid for this
larger ground grammar. Bodies may be arbitrary finite Ferraris formulas.

The constant laws justify the three source default-negation forms independently
of implementation simplification. The source compiler must still establish
complete finite rows, safety and provenance before any tautological simplification.
These laws do not prove Rust admission, possible-support joins, limits or shaders.
-/

namespace Zetesis.BooleanHeads

open Ferraris

universe u
variable {α : Type u}

def constant (value : Bool) : Formula α :=
  if value then .imp .bot .bot else .bot

/-- Boolean head operands do not depend on atom membership. -/
theorem constant_original (M : Atoms α) (value : Bool) :
    Satisfies M (constant value) ↔ value = true := by
  cases value <;> simp [constant, Satisfies]

/-- The same statement holds for arbitrary M and J, without a subset premise. -/
theorem constant_frozen (M J : Atoms α) (value : Bool) :
    Satisfies J (Reduct M (constant value)) ↔ value = true := by
  cases value <;> simp [constant, Reduct, Satisfies]

/-- A default-negated Boolean depends only on its constant truth. -/
theorem negative_constant_frozen (M J : Atoms α) (value : Bool) :
    Satisfies J (Reduct M (Neg (constant value))) ↔ value = false := by
  cases value <;> simp [constant, Ferraris.Neg, Reduct, Satisfies]

/-- Double default negation restores the constant, without supplying support. -/
theorem double_negative_constant_frozen (M J : Atoms α) (value : Bool) :
    Satisfies J (Reduct M (Neg (Neg (constant value)))) ↔ value = true := by
  cases value <;> simp [constant, Ferraris.Neg, Reduct, Satisfies]

inductive Literal (α : Type u) where
  | atom : NegativeHeads.Literal α → Literal α
  | boolean : Bool → Literal α
  deriving DecidableEq

def literal : Literal α → Formula α
  | .atom atom => NegativeHeads.literal atom
  | .boolean value => constant value

def head (literals : List (Literal α)) : Formula α :=
  RuleFactorization.any (literals.map literal)

structure Rule (α : Type u) where
  body : Formula α
  literals : List (Literal α)

def ruleFormula (rule : Rule α) : Formula α := .imp rule.body (head rule.literals)

def theory (rules : List (Rule α)) : Theory α := rules.map ruleFormula

def HasProducer (M : Atoms α) (rules : List (Rule α)) (a : α) : Prop :=
  ∃ rule ∈ rules, Literal.atom (.positive a) ∈ rule.literals ∧ Satisfies M rule.body

/-- Retaining positive true atoms retains a true reduct head. Boolean and
    default-negated occurrences require no atom to be kept in J. -/
theorem head_reduct_of_positive_kept (M J : Atoms α) (literals : List (Literal α))
    (kept : ∀ a, Literal.atom (.positive a) ∈ literals → M a → J a)
    (holds : Satisfies M (head literals)) :
    Satisfies J (Reduct M (head literals)) := by
  rw [head, RuleFactorization.satisfies_any] at holds
  obtain ⟨F, hF, htruth⟩ := holds
  obtain ⟨l, hl, rfl⟩ := List.mem_map.mp hF
  rw [head, RuleFactorization.reduct_any]
  refine ⟨literal l, List.mem_map.mpr ⟨l, hl, rfl⟩, ?_⟩
  cases l with
  | boolean value =>
    exact (constant_frozen M J value).mpr ((constant_original M value).mp htruth)
  | atom atom =>
    cases atom with
    | positive a => exact (atom_reduct M J a).mpr ⟨htruth, kept a hl htruth⟩
    | negative a => exact (neg_atom_reduct M J a).mpr htruth
    | doubleNegative a =>
      exact (double_neg_atom_reduct M J a).mpr
        ((double_neg_satisfies M (.atom a)).mp htruth)

/-- Removing an unsupported atom preserves every true rule reduct, contradicting
    stable-model minimality. Boolean head occurrences never witness a producer. -/
theorem stable_has_positive_producer (M : Atoms α) (rules : List (Rule α))
    (stable : Stable M (theory rules)) (a : α) (present : M a) :
    HasProducer M rules a := by
  classical
  apply Classical.byContradiction
  intro unsupported
  let J : Atoms α := fun b => M b ∧ b ≠ a
  have proper : ProperSub J M := by
    refine ⟨fun _ membership => membership.1, ?_⟩
    intro back
    exact (back a present).2 rfl
  apply stable.2
  refine ⟨J, proper, ?_⟩
  intro F hF
  obtain ⟨G, hG, rfl⟩ := List.mem_map.mp hF
  obtain ⟨rule, hrule, rfl⟩ := List.mem_map.mp hG
  have original : Satisfies M (ruleFormula rule) :=
    stable.1 _ (List.mem_map.mpr ⟨rule, hrule, rfl⟩)
  rw [ruleFormula, RuleFactorization.reduct_imp]
  refine ⟨original, ?_⟩
  intro body_reduct
  have body_original : Satisfies M rule.body :=
    NegativeHeads.reduct_truth_requires_original M J rule.body body_reduct
  apply head_reduct_of_positive_kept M J rule.literals
  · intro b hpositive hb
    refine ⟨hb, ?_⟩
    intro equal
    subst b
    exact unsupported ⟨rule, hrule, hpositive, body_original⟩
  · exact original body_original

open Classical in
noncomputable def supportFormula (rules : List (Rule α)) (a : α) : Formula α :=
  .imp (.atom a) (RuleFactorization.any
    ((rules.filter (fun rule => Literal.atom (.positive a) ∈ rule.literals)).map Rule.body))

/-- A support implication describes only the original positive producer fact. -/
theorem support_formula_truth (M : Atoms α) (rules : List (Rule α)) (a : α) :
    Satisfies M (supportFormula rules a) ↔ (M a → HasProducer M rules a) := by
  classical
  change (M a → Satisfies M (RuleFactorization.any _)) ↔ _
  constructor
  · intro holds present
    obtain ⟨F, hF, htruth⟩ := (RuleFactorization.satisfies_any M _).mp (holds present)
    obtain ⟨rule, hrule, rfl⟩ := List.mem_map.mp hF
    have member := List.mem_filter.mp hrule
    exact ⟨rule, member.1, of_decide_eq_true member.2, htruth⟩
  · intro holds present
    obtain ⟨rule, hrule, hpositive, htruth⟩ := holds present
    apply (RuleFactorization.satisfies_any M _).mpr
    exact ⟨rule.body, List.mem_map.mpr ⟨rule,
      List.mem_filter.mpr ⟨hrule, decide_eq_true hpositive⟩, rfl⟩, htruth⟩

noncomputable def supportFormulas (rules : List (Rule α)) (atoms : List α) : Theory α :=
  atoms.map (supportFormula rules)

/-- Every stable model satisfies the necessary positive-producer implications. -/
theorem stable_satisfies_support (M : Atoms α) (rules : List (Rule α)) (atoms : List α)
    (stable : Stable M (theory rules)) : Models M (supportFormulas rules atoms) := by
  intro F hF
  obtain ⟨a, _, rfl⟩ := List.mem_map.mp hF
  exact (support_formula_truth M rules a).mpr
    (stable_has_positive_producer M rules stable a)

/-- Double-negated producer guards constrain candidates without changing which
    interpretations are stable under the original rule theory. -/
theorem guarded_theory_preserves_stable_models (M : Atoms α) (rules : List (Rule α))
    (atoms : List α) :
    Stable M (GuardTheory (supportFormulas rules atoms) ++ theory rules) ↔
      Stable M (theory rules) := by
  apply stable_guard_theory_iff_of_stable_entails
  intro candidate stable
  exact stable_satisfies_support candidate rules atoms stable

end Zetesis.BooleanHeads
