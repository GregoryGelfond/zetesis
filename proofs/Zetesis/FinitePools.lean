import Zetesis.Ferraris

/-!
# Finite source pool occurrences

Each occurrence chooses independently; the product enumerates complete formulas.
Membership is specified relationally, independently of the product. Complete
coverage preserves original models, every frozen M/J reduct, and stable models
in arbitrary context. Duplicate positions are retained by enumeration; only the
logical theory treats repeated formulas idempotently.

This does not verify source recognition, Rust cursors, choice-head construction,
provenance, budgets, arithmetic or completed support. Local choice products are
submitted together to the existing one-group lowering contract.
-/

namespace Zetesis.FinitePools
open Ferraris
universe u v
variable {α : Type u} {β : Type v}

def product : List (List β) → List (List β)
  | [] => [[]]
  | alternatives :: rest => alternatives.flatMap
      (fun value => (product rest).map (fun values => value :: values))

def Binds : List (List β) → List β → Prop
  | [], [] => True
  | alternatives :: rest, value :: values => value ∈ alternatives ∧ Binds rest values
  | _, _ => False

theorem product_complete (pools : List (List β)) (values : List β) :
    values ∈ product pools ↔ Binds pools values := by
  induction pools generalizing values with
  | nil => cases values <;> simp [product, Binds]
  | cons alternatives rest ih =>
    cases values with
    | nil => simp [product, Binds]
    | cons value values => simp [product, Binds, List.mem_flatMap, ih]

theorem empty_occurrence (rest : List (List β)) :
    product ([] :: rest) = [] := by
  simp [product]

theorem duplicate_alternative (value : β) (rest : List (List β)) (values : List β) :
    values ∈ product ([value, value] :: rest) ↔ values ∈ product ([value] :: rest) := by
  cases values <;> simp [product_complete, Binds]

theorem independent_equal_occurrences :
    [1, 2] ∈ product [[1, 2], [1, 2]] ∧
    [2, 1] ∈ product [[1, 2], [1, 2]] := by
  decide

/-- Every row emits an entire rule formula, including its disjunction. -/
def expansion (pools : List (List β)) (statement : List β → Formula α) : Theory α :=
  (product pools).map statement

theorem original_in_context (M : Atoms α) (pools : List (List β))
    (rows : List (List β)) (complete : ∀ values, values ∈ rows ↔ Binds pools values)
    (statement : List β → Formula α) (context : Theory α) :
    Models M (expansion pools statement ++ context) ↔
      Models M (rows.map statement ++ context) := by
  simp only [Models, expansion, List.mem_append, List.mem_map, product_complete, complete]

theorem frozen_in_context (M J : Atoms α) (pools : List (List β))
    (rows : List (List β)) (complete : ∀ values, values ∈ rows ↔ Binds pools values)
    (statement : List β → Formula α) (context : Theory α) :
    Models J (ReductTheory M (expansion pools statement ++ context)) ↔
      Models J (ReductTheory M (rows.map statement ++ context)) := by
  simp only [Models, ReductTheory, expansion, List.mem_append, List.mem_map,
    product_complete, complete]

theorem stable_in_context (M : Atoms α) (pools : List (List β))
    (rows : List (List β)) (complete : ∀ values, values ∈ rows ↔ Binds pools values)
    (statement : List β → Formula α) (context : Theory α) :
    Stable M (expansion pools statement ++ context) ↔
      Stable M (rows.map statement ++ context) := by
  simp only [Stable, original_in_context M pools rows complete,
    frozen_in_context M _ pools rows complete]

theorem flattening_changes_original_truth :
    ¬ Models (fun atom : Nat => atom = 1)
      [.disj (.atom 1) (.atom 3), .disj (.atom 2) (.atom 3)] ∧
    Models (fun atom : Nat => atom = 1)
      [.disj (.atom 1) (.disj (.atom 2) (.atom 3))] := by
  simp [Models, Satisfies]

end Zetesis.FinitePools
