import Zetesis.ChoiceIntervals

/-!
# Fully evaluated ground guards

A guard is an atom-free Boolean expression over already evaluated, total scalar
comparison results. Its executable evaluator is independent of both the frozen
candidate M and the tested reduct interpretation J. Replacing the guard by its
computed truth constant preserves original truth and every frozen pair, even
inside arbitrary formula contexts containing implication and default negation.

A signed comparison chain negates the conjunction of all adjacent results, not
each result separately. The empty-chain convention is mathematical; this module
does not claim that the source parser accepts empty comparison chains.

Source evaluation, machine arithmetic, undefined operations, binding/safety,
finite substitution coverage and the Rust compiler are not proved here. In
particular, an undefined comparison is not a false Boolean result. Ordinary
atom-dependent formulas are outside the guard syntax: classical tautology alone
does not justify replacement by truth before the Ferraris mask is frozen.
-/

namespace Zetesis.GroundGuards

universe u v
variable {α : Type u} {κ : Type v}
open Ferraris
open ChoiceIntervals (Equivalent equivalent_refl substitute substitute_equivalent)

/-- All leaves are total evaluated results, with no atom-membership query. -/
inductive Guard where
  | truth : Bool → Guard
  | neg : Guard → Guard
  | conj : Guard → Guard → Guard
  | disj : Guard → Guard → Guard

def evaluate : Guard → Bool
  | .truth value => value
  | .neg guard => !(evaluate guard)
  | .conj left right => evaluate left && evaluate right
  | .disj left right => evaluate left || evaluate right

def constant (value : Bool) : Formula α :=
  if value then .imp .bot .bot else .bot

def formula : Guard → Formula α
  | .truth value => constant value
  | .neg guard => Neg (formula guard)
  | .conj left right => .conj (formula left) (formula right)
  | .disj left right => .disj (formula left) (formula right)

theorem constant_original (M : Atoms α) (value : Bool) :
    Satisfies M (constant value) ↔ value = true := by
  cases value <;> simp [constant, Satisfies]

theorem constant_frozen (M J : Atoms α) (value : Bool) :
    Satisfies J (Reduct M (constant value)) ↔ value = true := by
  cases value <;> simp [constant, Reduct, Satisfies]

/-- A rule with an evaluated Boolean head has the same original and frozen
    truth as a constraint whose body also requires the complementary constant.

    If the head is true, both rules impose no condition. If it is false, both
    original rules forbid the body; `reduct_imp` additionally requires its
    frozen reduct to be false at J. The body is arbitrary and J need not be a
    subset of M. Evaluation definedness and source diagnostic order are separate
    implementation obligations. -/
theorem constant_head_constraint (body : Formula α) (value : Bool) :
    Equivalent (.imp body (constant value))
      (.imp (.conj body (constant (!value))) .bot) := by
  constructor
  · intro M
    simp only [Satisfies, constant_original]
    cases value <;> simp
  · intro M J
    have bottom_frozen : Satisfies J (Reduct M (.bot : Formula α)) ↔ False :=
      Iff.rfl
    rw [RuleFactorization.reduct_imp, RuleFactorization.reduct_imp,
      RuleFactorization.reduct_conj]
    simp only [Satisfies, constant_original, constant_frozen, bottom_frozen]
    cases value <;> simp

theorem guard_original (M : Atoms α) (guard : Guard) :
    Satisfies M (formula guard) ↔ evaluate guard = true := by
  induction guard with
  | truth value => exact constant_original M value
  | neg guard ih => simp [formula, Ferraris.Neg, Satisfies, evaluate, ih]
  | conj left right ihLeft ihRight => simp [formula, Satisfies, evaluate, ihLeft, ihRight]
  | disj left right ihLeft ihRight => simp [formula, Satisfies, evaluate, ihLeft, ihRight]

/-- No subset premise is needed: all reduct worlds see the same evaluated guard. -/
theorem guard_frozen (M J : Atoms α) (guard : Guard) :
    Satisfies J (Reduct M (formula guard)) ↔ evaluate guard = true := by
  induction guard with
  | truth value => exact constant_frozen M J value
  | neg guard ih =>
    change Satisfies J (Reduct M (.imp (formula guard) .bot)) ↔ _
    rw [RuleFactorization.reduct_imp]
    simp [Satisfies, guard_original, ih, evaluate, Reduct]
  | conj left right ihLeft ihRight =>
    simp [formula, RuleFactorization.reduct_conj, evaluate, ihLeft, ihRight]
  | disj left right ihLeft ihRight =>
    simp [formula, RuleFactorization.reduct_disj, evaluate, ihLeft, ihRight]

theorem evaluated_guard_equivalent (guard : Guard) :
    Equivalent (formula guard : Formula α) (constant (evaluate guard)) := by
  constructor
  · intro M
    exact (guard_original M guard).trans (constant_original M (evaluate guard)).symm
  · intro M J
    exact (guard_frozen M J guard).trans (constant_frozen M J (evaluate guard)).symm

inductive Sign where
  | positive
  | negative
  | doubleNegative

def signed : Sign → Guard → Guard
  | .positive, guard => guard
  | .negative, guard => .neg guard
  | .doubleNegative, guard => .neg (.neg guard)

def chain : List Bool → Guard
  | [] => .truth true
  | value :: rest => .conj (.truth value) (chain rest)

/-- The independent chain specification requires every adjacent relation. -/
def AllTrue (results : List Bool) : Prop := ∀ result ∈ results, result = true

theorem chain_evaluation (results : List Bool) :
    evaluate (chain results) = true ↔ AllTrue results := by
  induction results with
  | nil => simp [chain, evaluate, AllTrue]
  | cons value rest ih => simp [chain, evaluate, AllTrue] at ih ⊢; intro _; exact ih

theorem negative_chain_scope (results : List Bool) :
    evaluate (signed .negative (chain results)) = true ↔ ¬ AllTrue results := by
  simp [signed, evaluate, Bool.not_eq_true, ← chain_evaluation results]

theorem double_negative_chain_scope (results : List Bool) :
    evaluate (signed .doubleNegative (chain results)) = true ↔ AllTrue results := by
  simp [signed, evaluate, chain_evaluation]

/-- One failed adjacent comparison suffices for the negation of a whole chain;
    conjoining individually negated comparisons would produce a different guard. -/
theorem negating_each_comparison_is_different :
    evaluate (signed .negative (chain [true, false])) = true ∧
      evaluate (chain [!true, !false]) = false := by
  decide

/-- A context can retain arbitrary logical formulas at its other leaves. Only
    explicitly selected evaluated guards are replaced. -/
def originalLeaf (logical : κ → Formula α) (guards : κ → Option Guard)
    (key : κ) : Formula α :=
  match guards key with
  | none => logical key
  | some guard => formula guard

def evaluatedLeaf (logical : κ → Formula α) (guards : κ → Option Guard)
    (key : κ) : Formula α :=
  match guards key with
  | none => logical key
  | some guard => constant (evaluate guard)

theorem replacement_in_formula_context (context : Formula κ)
    (logical : κ → Formula α) (guards : κ → Option Guard) :
    Equivalent (substitute context (originalLeaf logical guards))
      (substitute context (evaluatedLeaf logical guards)) := by
  apply substitute_equivalent
  intro key
  cases found : guards key with
  | none => simp [originalLeaf, evaluatedLeaf, found]; exact equivalent_refl _
  | some guard =>
    simpa [originalLeaf, evaluatedLeaf, found] using
      (evaluated_guard_equivalent guard : Equivalent (formula guard : Formula α) _)

/-- Other theory roots, atom identities, M and every proper-subset J remain
    unchanged. This also covers a guard nested in a fixed finite choice or
    aggregate formula; source construction of that formula remains external. -/
theorem replacement_preserves_stability (M : Atoms α) (context : Formula κ)
    (logical : κ → Formula α) (guards : κ → Option Guard) (theory : Theory α) :
    Stable M (substitute context (originalLeaf logical guards) :: theory) ↔
      Stable M (substitute context (evaluatedLeaf logical guards) :: theory) := by
  have same := replacement_in_formula_context context logical guards
  simp only [Stable, models_cons, ReductTheory, List.map_cons, same.1 M, same.2 M]

/-- A formula can be classically always true and still not be a ground guard.
    Replacing p or not p by truth before freezing loses its reduct support. -/
theorem classical_tautology_is_not_a_guard_replacement (a : α) :
    (∀ M, Satisfies M (.disj (.atom a) (Neg (.atom a)))) ∧
      ¬ Equivalent (.disj (.atom a) (Neg (.atom a))) (constant true) := by
  classical
  constructor
  · intro M
    exact Classical.em (M a)
  · intro same
    have impossible := (same.2 Full Empty).mpr
      ((constant_frozen Full Empty true).mpr rfl)
    simp [choice_reduct_guard, Full, Empty] at impossible

end Zetesis.GroundGuards
