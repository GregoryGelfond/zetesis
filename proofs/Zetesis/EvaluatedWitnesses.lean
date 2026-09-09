import Zetesis.StructuredWitnesses
import Zetesis.PositiveArguments

/-!
# Evaluated positions in positive local witnesses

A finite support row carries its complete source atom, a structural transaction,
and a partial check consuming the transaction's resulting binding. The check
cannot produce a new binding. Failure of evaluation remains distinct from an
ordinary mismatch; neither can publish a partially extracted environment.

The supplied check represents evaluation from independently available inputs
(for example `PositiveArguments.check` after resolving its declared reads).
Faithful extraction, input availability, evaluation and complete support coverage
remain implementation obligations. These laws do not establish Rust compilation,
scope analysis, deferred-error ordering, source traversal, work/byte accounting,
or any GPU correspondence. They do not justify discarding an undefined row or
returning a completed conditional after a resource stop.
-/

namespace Zetesis.EvaluatedWitnesses

open Ferraris StructuralBindings
open RuleFactorization (any)
universe u v
variable {A : Type u} {V : Type v} [DecidableEq V]

/-- Checks consume a completed local environment and retain its whole source atom. -/
structure Witness (A : Type u) (V : Type v) where
  atom : A
  shape : Bool
  constraints : List (Nat × V)
  check : Binding V → Option Bool

/-- The outer `none` is undefined evaluation; `some none` is a refused row.
    Success returns exactly the matched environment, never a check-produced one. -/
def matchChecked (before : Binding V) (row : Witness A V) :
    Option (Option (Binding V)) :=
  match matchRow row.shape before row.constraints with
  | none => some none
  | some after =>
    match row.check after with
    | none => none
    | some false => some none
    | some true => some (some after)

/-- Successful selection consists of structural extraction followed by a true
    consuming check on that exact environment. There is no inverse step. -/
theorem successful_selection (before after : Binding V) (row : Witness A V) :
    matchChecked before row = some (some after) ↔
      matchRow row.shape before row.constraints = some after ∧ row.check after = some true := by
  cases matched : matchRow row.shape before row.constraints with
  | none => simp [matchChecked, matched]
  | some found =>
    cases checked : row.check found with
    | none =>
      simp only [matchChecked, matched, checked, reduceCtorEq, false_iff,
        Option.some.injEq, not_and]
      intro equal
      subst after
      simp [checked]
    | some result =>
      cases result with
      | false =>
        simp only [matchChecked, matched, checked, reduceCtorEq, false_iff,
          Option.some.injEq, not_and]
        intro equal
        subst after
        simp [checked]
      | true =>
        simp only [matchChecked, matched, checked, Option.some.injEq]
        constructor
        · intro equal
          subst after
          exact ⟨rfl, checked⟩
        · exact And.left

/-- Existing outer and condition values survive extraction and evaluation. -/
theorem completed_condition_preserved (before after : Binding V) (row : Witness A V)
    (success : matchChecked before row = some (some after)) : Extends before after := by
  have matched : matchRow row.shape before row.constraints = some after :=
    ((successful_selection before after row).mp success).1
  exact StructuredWitnesses.completed_condition_preserved before after
    ⟨row.atom, row.shape, row.constraints⟩ matched

/-- Undefined evaluation remains an explicit failure after a complete match. -/
theorem undefined_selection (before after : Binding V) (row : Witness A V)
    (matched : matchRow row.shape before row.constraints = some after)
    (undefined : row.check after = none) : matchChecked before row = none := by
  simp [matchChecked, matched, undefined]

/-- A mismatch cannot evaluate an expression against an incomplete environment. -/
theorem unmatched_row_skips_evaluation (before : Binding V) (row : Witness A V)
    (mismatch : matchRow row.shape before row.constraints = none) :
    matchChecked before row = some none := by
  simp [matchChecked, mismatch]

/-- A false consuming check refuses the row without returning extracted values. -/
theorem false_check_refuses (before after : Binding V) (row : Witness A V)
    (matched : matchRow row.shape before row.constraints = some after)
    (refused : row.check after = some false) : matchChecked before row = some none := by
  simp [matchChecked, matched, refused]

/-- This projection identifies successful rows only. A caller must separately
    propagate undefined evaluation; this list is not a failure-handling policy. -/
def selected (before : Binding V) (rows : List (Witness A V)) : List (Witness A V) :=
  rows.filter fun row => ((matchChecked before row).bind id).isSome

/-- Exact membership relative to the supplied finite source carrier. -/
theorem selected_iff (before : Binding V) (rows : List (Witness A V))
    (row : Witness A V) :
    row ∈ selected before rows ↔ row ∈ rows ∧
      ∃ after, matchRow row.shape before row.constraints = some after ∧
        row.check after = some true := by
  have accepted : ((matchChecked before row).bind id).isSome = true ↔
      ∃ after, matchChecked before row = some (some after) := by
    cases result : matchChecked before row with
    | none => simp
    | some found => cases found <;> simp
  simp only [selected, List.mem_filter, accepted, successful_selection]

def alternatives (before : Binding V) (rows : List (Witness A V)) : List (Formula A) :=
  (selected before rows).map fun row => .atom row.atom

/-- Passing data checks does not establish logical truth: a whole source atom
    still has to belong to the interpretation. -/
theorem original_witness_truth (M : Atoms A) (before : Binding V)
    (rows : List (Witness A V)) :
    Satisfies M (any (alternatives before rows)) ↔
      ∃ row ∈ rows, (∃ after, matchRow row.shape before row.constraints = some after ∧
        row.check after = some true) ∧ M row.atom := by
  simp only [alternatives, RuleFactorization.satisfies_any, List.mem_map]
  constructor
  · rintro ⟨formula, ⟨row, member, rfl⟩, truth⟩
    obtain ⟨present, accepted⟩ := (selected_iff before rows row).mp member
    exact ⟨row, present, accepted, truth⟩
  · rintro ⟨row, present, accepted, truth⟩
    exact ⟨.atom row.atom, ⟨row, (selected_iff before rows row).mpr
      ⟨present, accepted⟩, rfl⟩, truth⟩

/-- Arbitrary frozen M/J truth retains the same selected complete atom in both
    interpretations. There is no assumption that J is a subset of M. -/
theorem frozen_witness_truth (M J : Atoms A) (before : Binding V)
    (rows : List (Witness A V)) :
    Satisfies J (Reduct M (any (alternatives before rows))) ↔
      ∃ row ∈ rows, (∃ after, matchRow row.shape before row.constraints = some after ∧
        row.check after = some true) ∧ M row.atom ∧ J row.atom := by
  simp only [alternatives, RuleFactorization.reduct_any, List.mem_map]
  constructor
  · rintro ⟨formula, ⟨row, member, rfl⟩, truth⟩
    obtain ⟨present, accepted⟩ := (selected_iff before rows row).mp member
    exact ⟨row, present, accepted, (atom_reduct M J row.atom).mp truth⟩
  · rintro ⟨row, present, accepted, truth⟩
    exact ⟨.atom row.atom, ⟨row, (selected_iff before rows row).mpr
      ⟨present, accepted⟩, rfl⟩, (atom_reduct M J row.atom).mpr truth⟩

/-- Reordering or coalescing identical complete rows preserves stable membership
    in the enclosing conditional rule and an arbitrary unchanged theory. -/
theorem complete_row_collection_keeps_stability (M : Atoms A)
    (condition remaining head : Formula A) (before : Binding V)
    (left right : List (Witness A V)) (coverage : ∀ row, row ∈ left ↔ row ∈ right)
    (context : Theory A) :
    Stable M (.imp (.conj remaining
      (ConsequentAlternatives.body [⟨condition, alternatives before left⟩])) head :: context) ↔
    Stable M (.imp (.conj remaining
      (ConsequentAlternatives.body [⟨condition, alternatives before right⟩])) head :: context) := by
  have same (formula : Formula A) :
      formula ∈ alternatives before left ↔ formula ∈ alternatives before right := by
    simp only [alternatives, List.mem_map, selected, List.mem_filter, coverage]
  exact ConsequentAlternatives.alternative_collection_keeps_stability M condition remaining head
    (alternatives before left) (alternatives before right) same context

end Zetesis.EvaluatedWitnesses
