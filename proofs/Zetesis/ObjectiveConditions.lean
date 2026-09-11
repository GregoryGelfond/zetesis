import Zetesis.ObjectiveTransport

/-!
# Closed conditions on original models

An objective condition is a finite Boolean query over typed atoms. It has no
rule roots, atom universe or support meaning. Its truth is the original truth
of the corresponding Ferraris formula; the query is never evaluated against
a frozen reduct to decide an objective contribution. Translating only these
conditions preserves complete contribution keys, costs and every optimum tie.

The Rust representation stores an acyclic node table rather than this unfolded
syntax. ObjectiveConditionTable proves correspondence for a mathematical table
with backward references. Rust admission/indexing, finite source binding and eligibility,
priority presence, checked arithmetic, allocation and work completion remain
separate implementation obligations. These laws neither erase a condition from
the original theory nor supply a missing source-presence certificate.
-/

namespace Zetesis.ObjectiveConditions

universe u v
variable {A : Type u} {K : Type v}

/-- A closed query has no binding or atom-producing constructor. -/
inductive Query (A : Type u) where
  | boolean : Bool → Query A
  | atom : A → Query A
  | neg : Query A → Query A
  | conj : Query A → Query A → Query A
  | disj : Query A → Query A → Query A

/-- Read only the supplied original interpretation. -/
def evaluate (model : A → Bool) : Query A → Bool
  | .boolean value => value
  | .atom atom => model atom
  | .neg operand => !(evaluate model operand)
  | .conj left right => evaluate model left && evaluate model right
  | .disj left right => evaluate model left || evaluate model right

/-- The formula meaning is used only for original satisfaction. -/
def formula : Query A → Ferraris.Formula A
  | .boolean false => .bot
  | .boolean true => .imp .bot .bot
  | .atom atom => .atom atom
  | .neg operand => Ferraris.Neg (formula operand)
  | .conj left right => .conj (formula left) (formula right)
  | .disj left right => .disj (formula left) (formula right)

/-- Each query reads precisely the original truth of its formula meaning.
The argument follows its finite constructors; negation concerns original
satisfaction, so no reduct or stability assumption is introduced. -/
theorem original_truth (model : A → Bool) (query : Query A) :
    evaluate model query = true ↔
      Ferraris.Satisfies (fun atom => model atom = true) (formula query) := by
  induction query with
  | boolean value => cases value <;> simp [evaluate, formula, Ferraris.Satisfies]
  | atom atom => rfl
  | neg operand ih =>
    cases value : evaluate model operand <;>
      simp_all [evaluate, formula, Ferraris.Neg, Ferraris.Satisfies]
  | conj left right leftTruth rightTruth =>
    simpa [evaluate, formula, Ferraris.Satisfies] using and_congr leftTruth rightTruth
  | disj left right leftTruth rightTruth =>
    simpa [evaluate, formula, Ferraris.Satisfies] using or_congr leftTruth rightTruth

/-- Replacing the interpretation by an atomwise identical one preserves every
condition. A displayed projection with omitted atoms does not meet this premise. -/
theorem model_identity (left right : A → Bool) (same : ∀ atom, left atom = right atom)
    (query : Query A) : evaluate left query = evaluate right query := by
  have identical : left = right := funext same
  exact congrArg (fun model => evaluate model query) identical

/-- An exact condition translation preserves the complete fixed priority vector
through the existing global contribution-key evaluator. Presence of those slots
is a separate source-completion premise and is not reconstructed here. -/
theorem condition_vector [DecidableEq K] {C : Type u}
    (priorities : List Int) (entries : List (ObjectiveDirections.Entry K C))
    (compile : C → Query A) (truth : C → Bool) (model : A → Bool)
    (correspondence : ∀ condition, evaluate model (compile condition) = truth condition) :
    priorities.map (fun priority => ObjectiveDirections.mixedCost priority
      (evaluate model) (entries.map (ObjectiveTransport.transport compile))) =
      priorities.map (fun priority => ObjectiveDirections.mixedCost priority truth entries) := by
  exact ObjectiveTransport.cost_vector_transport priorities entries compile truth
    (evaluate model) correspondence

end Zetesis.ObjectiveConditions
