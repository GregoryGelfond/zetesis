import Zetesis.FiniteValues

/-!
# Positive arguments as consumers of declared inputs

A support row supplies an already fixed source environment, a captured complete
value, and its original atom. A finite value plan reads that environment; equality
checks the captured value against its successful result. Filtering retains whole
rows and never manufactures a binding or a supporting atom. In particular, the
captured value is not an input to evaluation and cannot supply an inverse.

Evaluation terminates by the finite plan tail (`FiniteValues.evaluate`); selection
terminates by the finite row list. Costs of the supplied partial operations and
Rust work/byte ceilings are not modeled here. These laws assume complete support
rows and independently established input environments. They do not prove Rust
compilation, readiness, join completeness, deferred-error order, arithmetic, or
the correspondence between source positions and private capture slots.
-/

namespace Zetesis.PositiveArguments
open Ferraris FiniteValues
universe u v
variable {V : Type u} {A : Type v} [DecidableEq V]

/-- No environment update is returned, including when evaluation fails. -/
def check (environment : Nat → V) (captured : V) (plan : List (Step V)) :
    Option Bool :=
  match evaluate environment plan [] with
  | some (value :: _) => some (decide (captured = value))
  | _ => none

/-- Agreement on declared source reads preserves both decisions and failures. -/
theorem declared_input_agreement (plan : List (Step V)) (left right : Nat → V)
    (agreement : ∀ slot ∈ inputs plan, left slot = right slot) (captured : V) :
    check left captured plan = check right captured plan := by
  have same_evaluation : evaluate left plan [] = evaluate right plan [] :=
    input_agreement plan left right agreement []
  unfold check
  rw [same_evaluation]

/-- A successful result is tested by exact complete-value equality. -/
theorem successful_check (environment : Nat → V) (captured result : V)
    (plan : List (Step V)) (previous : List V)
    (evaluated : evaluate environment plan [] = some (result :: previous)) :
    check environment captured plan = some true ↔ captured = result := by
  simp [check, evaluated]

theorem undefined_check (environment : Nat → V) (captured : V)
    (plan : List (Step V)) (failed : evaluate environment plan [] = none) :
    check environment captured plan = none := by
  simp [check, failed]

/-- An empty plan cannot obtain a result from the captured argument. -/
theorem empty_plan (environment : Nat → V) (captured : V) :
    check environment captured [] = none := by
  rfl

structure Row (V : Type u) (A : Type v) where
  environment : Nat → V
  captured : V
  atom : A

def select (plan : List (Step V)) (rows : List (Row V A)) : List (Row V A) :=
  rows.filter fun row => decide (check row.environment row.captured plan = some true)

/-- Relative to the supplied carrier, selection is both sound and complete. -/
theorem selected_iff (plan : List (Step V)) (rows : List (Row V A)) (row : Row V A) :
    row ∈ select plan rows ↔
      row ∈ rows ∧ check row.environment row.captured plan = some true := by
  simp [select]

/-- A selected atom comes from a complete original row, never from equality. -/
theorem selected_atom_has_support (plan : List (Step V)) (rows : List (Row V A))
    (row : Row V A) (selected : row ∈ select plan rows) :
    ∃ original ∈ rows, original = row ∧ original.atom = row.atom := by
  have original_row : row ∈ rows := (selected_iff plan rows row).mp selected |>.1
  exact ⟨row, original_row, rfl, rfl⟩

/-- Requiring the same check twice cannot create additional substitutions. -/
theorem repeated_selection (plan : List (Step V)) (rows : List (Row V A)) :
    select plan (select plan rows) = select plan rows := by
  simp only [select, List.filter_filter]
  congr 1
  funext row
  simp

/-- When the captured argument is the computed value, the original atom stays. -/
theorem original_atom_preserved (environment : Nat → V) (captured result : V)
    (plan : List (Step V)) (previous : List V) (resolve : V → A)
    (evaluated : evaluate environment plan [] = some (result :: previous))
    (accepted : check environment captured plan = some true)
    (M : Atoms A) (context : Formula A → Formula A) :
    Satisfies M (context (.atom (resolve captured))) ↔
      Satisfies M (context (.atom (resolve result))) := by
  have same_value : captured = result :=
    (successful_check environment captured result plan previous evaluated).mp accepted
  have same_atom : resolve captured = resolve result := congrArg resolve same_value
  exact original_literal_identity M (resolve captured) (resolve result) same_atom 0 context

/-- The same atom identity preserves every frozen M/J context, without J ⊆ M. -/
theorem frozen_atom_preserved (environment : Nat → V) (captured result : V)
    (plan : List (Step V)) (previous : List V) (resolve : V → A)
    (evaluated : evaluate environment plan [] = some (result :: previous))
    (accepted : check environment captured plan = some true)
    (M J : Atoms A) (context : Formula A → Formula A) :
    Satisfies J (Reduct M (context (.atom (resolve captured)))) ↔
      Satisfies J (Reduct M (context (.atom (resolve result)))) := by
  have same_value : captured = result :=
    (successful_check environment captured result plan previous evaluated).mp accepted
  have same_atom : resolve captured = resolve result := congrArg resolve same_value
  exact frozen_literal_identity M J (resolve captured) (resolve result) same_atom 0 context

end Zetesis.PositiveArguments
