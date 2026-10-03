import Zetesis.TightPlans

/-!
# Structural recognition of ranked-support bodies

The recognizer accepts exactly the formula syntax represented by `TightPlans.Body`.
It retains the entire interior of a default negation, including implications.
Conjunction and disjunction require both children to belong to the grammar.
Recognition preserves the original formula rather than rewriting it to an
equivalent one. The positive-occurrence test identifies atoms outside every
negation scope, independently of an interpretation or an ordering of atoms.

These are reusable formula-level algorithms and laws. Agreement with a Rust DAG
classifier, complete root extraction and rank validation remains a separate
implementation obligation.
-/

namespace Zetesis.TightBodyRecognition

universe u
variable {α : Type u}
open Ferraris TightPlans

/-- Recover a ranked-support body from its exact formula structure. -/
def recognize : Formula α → Option (Body α)
  | .atom atom => some (.atom atom)
  | .bot => some .bot
  | .conj left right =>
      match recognize left, recognize right with
      | some first, some second => some (.conj first second)
      | _, _ => none
  | .disj left right =>
      match recognize left, recognize right with
      | some first, some second => some (.disj first second)
      | _, _ => none
  | .imp interior .bot => some (.neg interior)
  | .imp _ _ => none

/-- A returned body has exactly the input formula. The proof follows the formula
    structure: conjunction and disjunction combine the two child equalities,
    while negation retains its interior without recursively restricting it. -/
theorem recognized_formula (formula : Formula α) (body : Body α)
    (recognized : recognize formula = some body) : body.formula = formula := by
  induction formula generalizing body with
  | atom atom =>
      simp only [recognize, Option.some.injEq] at recognized
      cases recognized
      rfl
  | bot =>
      simp only [recognize, Option.some.injEq] at recognized
      cases recognized
      rfl
  | conj left right leftExact rightExact =>
      cases first : recognize left with
      | none => simp [recognize, first] at recognized
      | some leftBody =>
          cases second : recognize right with
          | none => simp [recognize, first, second] at recognized
          | some rightBody =>
              simp only [recognize, first, second, Option.some.injEq] at recognized
              cases recognized
              have leftFormula : leftBody.formula = left := leftExact leftBody first
              have rightFormula : rightBody.formula = right := rightExact rightBody second
              show Formula.conj leftBody.formula rightBody.formula = .conj left right
              rw [leftFormula, rightFormula]
  | disj left right leftExact rightExact =>
      cases first : recognize left with
      | none => simp [recognize, first] at recognized
      | some leftBody =>
          cases second : recognize right with
          | none => simp [recognize, first, second] at recognized
          | some rightBody =>
              simp only [recognize, first, second, Option.some.injEq] at recognized
              cases recognized
              have leftFormula : leftBody.formula = left := leftExact leftBody first
              have rightFormula : rightBody.formula = right := rightExact rightBody second
              show Formula.disj leftBody.formula rightBody.formula = .disj left right
              rw [leftFormula, rightFormula]
  | imp interior consequent _ _ =>
      cases consequent with
      | bot =>
          simp only [recognize, Option.some.injEq] at recognized
          cases recognized
          rfl
      | atom atom => simp [recognize] at recognized
      | conj left right => simp [recognize] at recognized
      | disj left right => simp [recognize] at recognized
      | imp left right => simp [recognize] at recognized

/-- Every body in the ranked-support grammar is recognized without changing it.
    Induction combines the child recognitions; a negation is accepted directly. -/
theorem recognize_body (body : Body α) : recognize body.formula = some body := by
  induction body with
  | atom atom => rfl
  | bot => rfl
  | conj left right leftRecognized rightRecognized =>
      simp [Body.formula, recognize, leftRecognized, rightRecognized]
  | disj left right leftRecognized rightRecognized =>
      simp [Body.formula, recognize, leftRecognized, rightRecognized]
  | neg interior => rfl

/-- Recognition succeeds with a particular body exactly when that body's formula
    is the input. This combines structural soundness and grammar completeness. -/
theorem recognize_exact (formula : Formula α) (body : Body α) :
    recognize formula = some body ↔ body.formula = formula := by
  constructor
  · exact recognized_formula formula body
  · intro sameFormula
    rw [← sameFormula]
    exact recognize_body body

/-- Test for any atom occurrence outside default negation. The test requires no
    equality or enumeration operation on the atom type. -/
def hasPositive : Body α → Bool
  | .atom _ => true
  | .bot | .neg _ => false
  | .conj left right | .disj left right => hasPositive left || hasPositive right

/-- The Boolean test is true exactly when the body's existing positive-occurrence
    relation has a witness. Frozen interiors contribute no positive occurrence. -/
theorem hasPositive_iff (body : Body α) :
    hasPositive body = true ↔ ∃ atom, body.Positive atom := by
  induction body with
  | atom atom => simp [hasPositive, Body.Positive]
  | bot => simp [hasPositive, Body.Positive]
  | conj left right leftExact rightExact =>
      simp only [hasPositive, Bool.or_eq_true, leftExact, rightExact,
        Body.Positive, exists_or]
  | disj left right leftExact rightExact =>
      simp only [hasPositive, Bool.or_eq_true, leftExact, rightExact,
        Body.Positive, exists_or]
  | neg interior => simp [hasPositive, Body.Positive]

/-- The false test characterizes a body with no unfrozen atom occurrence.
    This is the complement of `hasPositive_iff`, not a claim about whether the
    formula is true in any interpretation. -/
theorem hasPositive_false_iff (body : Body α) :
    hasPositive body = false ↔ ∀ atom, ¬ body.Positive atom := by
  have absent : ¬ (hasPositive body = true) ↔ ¬ ∃ atom, body.Positive atom :=
    not_congr (hasPositive_iff body)
  simpa only [Bool.not_eq_true, not_exists] using absent

end Zetesis.TightBodyRecognition
