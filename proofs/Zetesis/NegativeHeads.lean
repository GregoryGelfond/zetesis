import Zetesis.FerrarisGuards
import Zetesis.RuleFactorization

/-!
# Default-negated disjunctive heads and necessary positive support

The head grammar contains positive atoms, default-negated atoms and double
default-negated atoms. Rule bodies can be arbitrary finite Ferraris formulas.
An atom in a stable model has a positive head occurrence in a rule whose body
holds in that model. Negative and double-negative occurrences are not producers.

This proves the semantic support obligation for this ground grammar. It does
not verify the Rust parser, possible-support grounding, resource accounting or
source interval expansion. Those remain separate refinement obligations.
-/

namespace Zetesis.NegativeHeads

open Ferraris

universe u
variable {α : Type u}

inductive Literal (α : Type u) where
  | positive : α → Literal α
  | negative : α → Literal α
  | doubleNegative : α → Literal α
  deriving DecidableEq

def literal : Literal α → Formula α
  | .positive a => .atom a
  | .negative a => Neg (.atom a)
  | .doubleNegative a => Neg (Neg (.atom a))

def head (literals : List (Literal α)) : Formula α :=
  RuleFactorization.any (literals.map literal)

structure Rule (α : Type u) where
  body : Formula α
  literals : List (Literal α)

def ruleFormula (rule : Rule α) : Formula α := .imp rule.body (head rule.literals)

def theory (rules : List (Rule α)) : Theory α := rules.map ruleFormula

def HasProducer (M : Atoms α) (rules : List (Rule α)) (a : α) : Prop :=
  ∃ rule ∈ rules, Literal.positive a ∈ rule.literals ∧ Satisfies M rule.body

theorem negative_literal_reduct (M J : Atoms α) (a : α) :
    Satisfies J (Reduct M (literal (.negative a))) ↔ ¬ M a :=
  neg_atom_reduct M J a

theorem double_negative_literal_reduct (M J : Atoms α) (a : α) :
    Satisfies J (Reduct M (literal (.doubleNegative a))) ↔ M a :=
  double_neg_atom_reduct M J a

/-- Retaining the true positive occurrences suffices to retain a true head in
    the reduct. Negative literal truth depends exclusively on the frozen M. -/
theorem head_reduct_of_positive_kept (M J : Atoms α) (literals : List (Literal α))
    (kept : ∀ a, Literal.positive a ∈ literals → M a → J a)
    (holds : Satisfies M (head literals)) :
    Satisfies J (Reduct M (head literals)) := by
  rw [head, RuleFactorization.satisfies_any] at holds
  obtain ⟨F, hF, htruth⟩ := holds
  obtain ⟨l, hl, rfl⟩ := List.mem_map.mp hF
  rw [head, RuleFactorization.reduct_any]
  refine ⟨literal l, List.mem_map.mpr ⟨l, hl, rfl⟩, ?_⟩
  cases l with
  | positive a => exact (atom_reduct M J a).mpr ⟨htruth, kept a hl htruth⟩
  | negative a => exact (neg_atom_reduct M J a).mpr htruth
  | doubleNegative a =>
    exact (double_neg_atom_reduct M J a).mpr
      ((double_neg_satisfies M (.atom a)).mp htruth)

/-- A true reduct subformula must have been true in the frozen interpretation. -/
theorem reduct_truth_requires_original (M J : Atoms α) (F : Formula α)
    (holds : Satisfies J (Reduct M F)) : Satisfies M F := by
  classical
  apply Classical.byContradiction
  intro absent
  rw [RuleFactorization.false_reduct M F absent] at holds
  exact holds

/-- Removing an unsupported atom leaves a model of every original rule's reduct.
    The contradiction is with subset minimality, without shifting disjunctions. -/
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
  have body_original := reduct_truth_requires_original M J rule.body body_reduct
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
    ((rules.filter (fun rule => Literal.positive a ∈ rule.literals)).map Rule.body))

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

theorem stable_satisfies_support (M : Atoms α) (rules : List (Rule α)) (atoms : List α)
    (stable : Stable M (theory rules)) : Models M (supportFormulas rules atoms) := by
  intro F hF
  obtain ⟨a, _, rfl⟩ := List.mem_map.mp hF
  exact (support_formula_truth M rules a).mpr
    (stable_has_positive_producer M rules stable a)

/-- The actual guard is double-negated. Adding positive support implications
    directly would be a different reduct transformation and is not justified. -/
theorem guarded_theory_preserves_stable_models (M : Atoms α) (rules : List (Rule α))
    (atoms : List α) :
    Stable M (GuardTheory (supportFormulas rules atoms) ++ theory rules) ↔
      Stable M (theory rules) := by
  apply stable_guard_theory_iff_of_stable_entails
  intro candidate stable
  exact stable_satisfies_support candidate rules atoms stable

end Zetesis.NegativeHeads
