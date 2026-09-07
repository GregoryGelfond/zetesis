import Zetesis.StructuralBindings
import Zetesis.ConsequentAlternatives

/-!
# Structured witnesses in positive conditional consequents

A completed condition binding filters a finite support carrier by constructor
shape and named-value agreement. The selected complete source atoms form one
disjunction; extracted values neither replace these atoms nor assert their truth.
The condition remains the antecedent of the resulting implication. This applies
to positive witnesses only, not arithmetic inversion or negative quantifiers.

Constructor/predicate signs and anonymous positions are represented by the
supplied shape check and constraint extraction: signs are part of the complete
atom, and anonymous positions add no equality constraint. Complete support
coverage and faithful extraction are explicit premises, not proved properties of
the Rust compiler. These laws do not establish Rust traversal, scope analysis,
allocation/work bounds, source normalization or completion after a resource stop.
-/

namespace Zetesis.StructuredWitnesses

universe u v
variable {α : Type u} {β : Type v} [DecidableEq β]
open Ferraris StructuralBindings
open RuleFactorization (any)

/-- One whole source atom and the finite structural agreement test for its row. -/
structure Witness (α : Type u) (β : Type v) where
  atom : α
  shape : Bool
  constraints : List (Nat × β)

/-- Every matched support row contributes its complete atom, including signs. -/
def alternatives (rows : List (Witness α β)) (before : Binding β) : List (Formula α) :=
  (selectRows rows Witness.shape Witness.constraints before).map (fun row => .atom row.atom)

/-- Selection requires a compatible extension of the completed condition binding;
    membership in the support carrier alone does not establish truth. -/
theorem original_witness_truth (M : Atoms α) (rows : List (Witness α β))
    (before : Binding β) :
    Satisfies M (any (alternatives rows before)) ↔
      ∃ row ∈ rows, row.shape = true ∧
        (∃ after, Extends before after ∧ Agrees after row.constraints) ∧ M row.atom := by
  have selected (row : Witness α β) :
      row ∈ selectRows rows Witness.shape Witness.constraints before ↔
        row ∈ rows ∧ row.shape = true ∧
          ∃ after, Extends before after ∧ Agrees after row.constraints :=
    selected_row_coverage rows (fun row => row ∈ rows) Witness.shape Witness.constraints
      before (fun _ => Iff.rfl) row
  simp only [alternatives, RuleFactorization.satisfies_any, List.mem_map]
  constructor
  · rintro ⟨formula, ⟨row, member, rfl⟩, truth⟩
    obtain ⟨present, shape, compatible⟩ := (selected row).mp member
    exact ⟨row, present, shape, compatible, truth⟩
  · rintro ⟨row, present, shape, compatible, truth⟩
    exact ⟨.atom row.atom, ⟨row, (selected row).mpr ⟨present, shape, compatible⟩, rfl⟩, truth⟩

/-- Arbitrary M and J need the same complete selected atom true in both worlds.
    No subset relation, current-candidate filtering or synthetic support is used. -/
theorem frozen_witness_truth (M J : Atoms α) (rows : List (Witness α β))
    (before : Binding β) :
    Satisfies J (Reduct M (any (alternatives rows before))) ↔
      ∃ row ∈ rows, row.shape = true ∧
        (∃ after, Extends before after ∧ Agrees after row.constraints) ∧
          M row.atom ∧ J row.atom := by
  have selected (row : Witness α β) :
      row ∈ selectRows rows Witness.shape Witness.constraints before ↔
        row ∈ rows ∧ row.shape = true ∧
          ∃ after, Extends before after ∧ Agrees after row.constraints :=
    selected_row_coverage rows (fun row => row ∈ rows) Witness.shape Witness.constraints
      before (fun _ => Iff.rfl) row
  simp only [alternatives, RuleFactorization.reduct_any, List.mem_map]
  constructor
  · rintro ⟨formula, ⟨row, member, rfl⟩, truth⟩
    obtain ⟨present, shape, compatible⟩ := (selected row).mp member
    exact ⟨row, present, shape, compatible, (atom_reduct M J row.atom).mp truth⟩
  · rintro ⟨row, present, shape, compatible, truth⟩
    exact ⟨.atom row.atom, ⟨row, (selected row).mpr ⟨present, shape, compatible⟩, rfl⟩,
      (atom_reduct M J row.atom).mpr truth⟩

/-- Local witness matching preserves every completed outer/condition slot. -/
theorem completed_condition_preserved (before after : Binding β) (row : Witness α β)
    (success : matchRow row.shape before row.constraints = some after) :
    Extends before after := by
  cases shape : row.shape with
  | false => simp [matchRow, shape] at success
  | true =>
    have matched : matchConstraints before row.constraints = some after := by
      simpa [matchRow, shape] using success
    exact (matching_sound before after row.constraints matched).1

/-- Failed local selection cannot change the binding used for the next row. -/
theorem refused_witness_preserves_condition (before : Binding β) (row : Witness α β)
    (failure : matchRow row.shape before row.constraints = none) :
    commit before (matchRow row.shape before row.constraints) = before := by
  exact failure_rolls_back before row.shape row.constraints failure

/-- The unchanged condition remains a logical antecedent even after row selection.
    Exhausting witnesses is separate from exhausting the universal condition rows. -/
theorem active_condition_requires_witness (M : Atoms α) (condition : Formula α)
    (rows : List (Witness α β)) (before : Binding β) :
    Satisfies M (.imp condition (any (alternatives rows before))) ↔
      (Satisfies M condition →
        ∃ row ∈ rows, row.shape = true ∧
          (∃ after, Extends before after ∧ Agrees after row.constraints) ∧ M row.atom) := by
  change (Satisfies M condition → Satisfies M (any (alternatives rows before))) ↔ _
  rw [original_witness_truth]

/-- Reordering or coalescing duplicate complete support rows preserves the entire
    enclosing rule's stable membership. The premise is complete row identity,
    stronger than equality of extracted values or current truth values. -/
theorem complete_row_collection_keeps_stability (M : Atoms α)
    (condition remaining head : Formula α) (left right : List (Witness α β))
    (before : Binding β) (coverage : ∀ row, row ∈ left ↔ row ∈ right)
    (context : Theory α) :
    Stable M (.imp (.conj remaining
      (ConsequentAlternatives.body [⟨condition, alternatives left before⟩])) head :: context) ↔
    Stable M (.imp (.conj remaining
      (ConsequentAlternatives.body [⟨condition, alternatives right before⟩])) head :: context) := by
  have same (formula : Formula α) :
      formula ∈ alternatives left before ↔ formula ∈ alternatives right before := by
    simp only [alternatives, List.mem_map, selectRows, List.mem_filter, coverage]
  exact ConsequentAlternatives.alternative_collection_keeps_stability M condition remaining head
    (alternatives left before) (alternatives right before) same context

end Zetesis.StructuredWitnesses
