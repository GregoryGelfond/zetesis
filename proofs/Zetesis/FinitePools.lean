import Zetesis.Ferraris

/-!
# Finite source pool occurrences

Each occurrence chooses independently; the product enumerates complete formulas.
Membership is specified relationally, independently of the product. Complete
coverage preserves original models, every frozen M/J reduct, and stable models
in arbitrary context. Duplicate positions are retained by enumeration; only the
logical theory treats repeated formulas idempotently.

This does not verify source recognition, Rust cursors, scope recognition, choice-head construction,
provenance, budgets, arithmetic or completed support. Local products are
submitted together to their existing one-group lowering or objective activation
contracts. The local occurrence laws below preserve the original element identity;
they do not equate the consumers' different measures or quantifiers.
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


section ScopedOccurrences

universe w
variable {σ : Type v} {ρ : Type w}

/-- Resolving a constructor or arithmetic context after independent finite
    child choices has precisely the declared child-binding image. The context
    is supplied only after its own syntax, safety and arithmetic admission. -/
theorem value_context_complete (pools : List (List β)) (context : List β → ρ)
    (result : ρ) :
    result ∈ (product pools).map context ↔
      ∃ values, Binds pools values ∧ context values = result := by
  simp only [List.mem_map, product_complete]

/-- Alternatives of one local element stay inside the surrounding collection.
    The element identity remains an input to emission, even when two elements
    have identical value alternatives. This covers source occurrence keys as
    well as complete tuple or objective keys supplied by the consumer. -/
def localOccurrences (elements : List σ) (pools : σ → List (List β))
    (emit : σ → List β → ρ) : List ρ :=
  elements.flatMap (fun element => (product (pools element)).map (emit element))

/-- A local emitted row has an original element and a complete independent
    binding. Conversely every such binding is retained. No new enclosing rule
    or choice group is introduced by this local union. -/
theorem local_occurrences_complete (elements : List σ) (pools : σ → List (List β))
    (emit : σ → List β → ρ) (row : ρ) :
    row ∈ localOccurrences elements pools emit ↔
      ∃ element ∈ elements, ∃ values,
        Binds (pools element) values ∧ emit element values = row := by
  simp only [localOccurrences, List.mem_flatMap, List.mem_map, product_complete]

/-- Grouping a complete key uses any eligible occurrence, not its number of
    source copies. This same premise can feed choice eligibility, aggregate
    activity, and objective activation; their downstream measures differ. -/
theorem local_activity_complete (elements : List σ) (pools : σ → List (List β))
    (emit : σ → List β → ρ) (active : ρ → Prop) :
    (∃ row ∈ localOccurrences elements pools emit, active row) ↔
      ∃ element ∈ elements, ∃ values,
        Binds (pools element) values ∧ active (emit element values) := by
  constructor
  · rintro ⟨row, member, enabled⟩
    obtain ⟨element, original, values, bound, identity⟩ :=
      (local_occurrences_complete elements pools emit row).mp member
    exact ⟨element, original, values, bound, identity.symm ▸ enabled⟩
  · rintro ⟨element, original, values, bound, enabled⟩
    have member : emit element values ∈ localOccurrences elements pools emit := by
      exact (local_occurrences_complete elements pools emit (emit element values)).mpr
        ⟨element, original, values, bound, rfl⟩
    exact ⟨emit element values, member, enabled⟩

end ScopedOccurrences

end Zetesis.FinitePools
