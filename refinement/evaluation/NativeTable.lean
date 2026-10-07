import NativeSpecification
import NativeRows

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract

/-!
# Decoded rows of the paired native table

The reference list is a mathematical view of actual getter results. Successful
admission removes its error default. It stores no second runtime graph and does
not expand wide connectives into binary nodes.
-/
namespace NativeTable

/-- Totalize a getter result only to state the reference table. Well-formed
input always takes the successful branch, proved by `lookup`. -/
def row (arena : Slice Usize) (node : theory.Node) : theory.NodeView :=
  match NativeRows.storedView arena node with
  | .Ok decoded => decoded
  | .Err _ => .False

/-- The native table's complete decoded rows in logical-node order. -/
def rows (view : theory.FormulaView) : List theory.NodeView :=
  view.nodes.val.map (row view.operands)

/-- Decoding retains exactly one row per logical node. -/
theorem length (view : theory.FormulaView) : (rows view).length = view.nodes.val.length := by
  simp [rows]

/-- The generated getter returns precisely the reference row at every admitted
position, with its backward children. No getter equation is assumed here. -/
theorem lookup (atoms : Nat) (view : theory.FormulaView)
    (valid : NativeStructure.WellFormed atoms view) (index : Usize)
    (inside : index.val < (rows view).length) :
    theory.FormulaView.node view index = ok (.Ok (rows view)[index.val]) ∧
      NativeStructure.ChildrenBefore index.val (rows view)[index.val] := by
  have sourceInside : index.val < view.nodes.val.length := by simpa [length] using inside
  obtain ⟨decoded, accepted, children, _⟩ := valid index.val sourceInside
  have exactRow : (rows view)[index.val] = decoded := by
    simp [rows, row, accepted]
  rw [exactRow]
  constructor
  · rw [NativeRows.formula_view_exact]
    simp [NativeRows.tableResult, List.getElem?_eq_getElem sourceInside, accepted]
  · simpa using children

/-- Admission supplies the dependency order used by truth-prefix induction. -/
theorem ordered (atoms : Nat) (view : theory.FormulaView)
    (valid : NativeStructure.WellFormed atoms view) :
    NativeSpecification.Ordered (rows view) := by
  intro index inside
  have sourceInside : index < view.nodes.val.length := by simpa [length] using inside
  obtain ⟨decoded, accepted, children, _⟩ := valid index sourceInside
  simpa [rows, row, accepted] using children

end NativeTable
