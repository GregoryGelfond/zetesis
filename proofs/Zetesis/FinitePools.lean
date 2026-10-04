import Zetesis.Ferraris

/-!
# Finite source pool occurrences

Each occurrence chooses independently; the product enumerates complete formulas.
Membership is specified relationally, independently of the product. Complete
coverage preserves original models, every frozen M/J reduct, and stable models
in arbitrary context. Duplicate positions are retained by enumeration; only the
logical theory treats repeated formulas idempotently.

This does not verify source recognition, Rust cursors, scope recognition,
choice-head construction, provenance, budgets, arithmetic or completed support.
Local products remain inside their enclosing group. `localOccurrences` retains
an identity supplied by its consumer; its membership laws do not choose counting
identity. `expandedOccurrences` instead assigns each pool-expanded position its
own identity before grounding witnesses. Neither construction equates the
consumers' different measures or quantifiers.
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
    have identical value alternatives. The consumer supplies any complete
    tuple or objective key. Membership alone does not establish a counting
    identity for Boolean alternatives expanded from one source element. -/
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

/-- An emitted family has an active row exactly when a supplied element has
    an active complete binding. A consumer may use this for existential
    activation; the law does not identify distinct counted alternatives. -/
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


/-- A pool-expanded element is identified by its position in the complete
    alternative list. Grounding witnesses retain that position. Equal values
    at different positions therefore remain different counted occurrences. -/
def expandedOccurrences (alternatives : List σ) (witnesses : σ → List ρ) :
    List (Nat × ρ) :=
  alternatives.zipIdx.flatMap (fun (element, index) =>
    (witnesses element).map (fun witness => (index, witness)))

/-- One expanded occurrence is active exactly when its alternative exists and
    has an active grounding witness. The key is the alternative's position,
    not its value or a witness's position; equal pool alternatives are counted
    separately, while repeated witnesses of one alternative share activity.

    The proof reads the indexed expansion in both directions and uses the
    original list lookup to authenticate the occurrence. This does not prove
    that a source compiler enumerates the complete pool product. -/
theorem expanded_occurrence_activity (alternatives : List σ)
    (witnesses : σ → List ρ) (active : ρ → Prop) (index : Nat) :
    (∃ row ∈ expandedOccurrences alternatives witnesses,
      row.1 = index ∧ active row.2) ↔
      ∃ alternative, alternatives[index]? = some alternative ∧
        ∃ witness ∈ witnesses alternative, active witness := by
  simp only [expandedOccurrences, List.mem_flatMap, List.mem_map]
  constructor
  · rintro ⟨row, ⟨⟨alternative, position⟩, member, witness, present, rfl⟩,
      same, enabled⟩
    change position = index at same
    subst position
    exact ⟨alternative, (List.mk_mem_zipIdx_iff_getElem?.mp member), witness, present, enabled⟩
  · rintro ⟨alternative, at_index, witness, present, enabled⟩
    exact ⟨(index, witness),
      ⟨(alternative, index), List.mk_mem_zipIdx_iff_getElem?.mpr at_index,
        witness, present, rfl⟩, rfl, enabled⟩

-- ANCHOR: expanded_boolean_occurrences
/-- Two equal pool alternatives each have two equal local witnesses. The
    resulting four rows carry two occurrence identities, not one or four. -/
example :
    expandedOccurrences [1, 1] (fun _ => [7, 7]) =
      [(0, 7), (0, 7), (1, 7), (1, 7)] ∧
    ((expandedOccurrences [1, 1] (fun _ => [7, 7])).map Prod.fst).eraseDups.length = 2 := by
  decide
-- ANCHOR_END: expanded_boolean_occurrences

end ScopedOccurrences

end Zetesis.FinitePools
