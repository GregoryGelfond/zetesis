import NativeRows

open Aeneas Aeneas.Std
open ZetesisNativeExtract

/-!
# Structural premises for native formula rows

A native row has an atom bound and backward child references. These predicates
describe the complete NodeView already returned by the generated getter; they
do not construct a second stored graph. Raw-table validity additionally requires
each getter to return that complete row from the table's own operand arena.
Admission proves these premises; evaluation uses them at its completed prefix.
-/
namespace NativeStructure

/-- Every operand names an earlier logical node. Duplicate occurrences retain
the same bound independently; neither ordering nor uniqueness is required. -/
def ChildrenBefore (position : Nat) : theory.NodeView → Prop
  | .Atom _ | .False => True
  | .Implies left right => left.val < position ∧ right.val < position
  | .And row | .Or row => ∀ child ∈ row.val, child.val < position

/-- An atom row names an atom in the supplied universe; other rows add none. -/
def AtomBound (atoms : Nat) : theory.NodeView → Prop
  | .Atom atom => atom.val < atoms
  | _ => True

/-- A raw node decodes successfully against its paired arena, with bounded atoms
and backward references. Successful decoding includes native arity and complete
span validity, rather than a clipped or invented operand row. -/
def ValidNode (atoms position : Nat) (arena : Slice Usize) (node : theory.Node) : Prop :=
  ∃ view, NativeRows.storedView arena node = .Ok view ∧
    ChildrenBefore position view ∧ AtomBound atoms view

/-- Every stored node is valid at its own position plus the prefix length.
The offset supports the exact suffix invariant of the generated node scan. -/
def ValidNodes (atoms first : Nat) (arena : Slice Usize)
    (nodes : List theory.Node) : Prop :=
  ∀ position (inside : position < nodes.length),
    ValidNode atoms (first + position) arena nodes[position]

/-- The empty suffix imposes no structural obligations. -/
theorem validNodes_nil (atoms first : Nat) (arena : Slice Usize) :
    ValidNodes atoms first arena [] := by
  intro position inside
  simp at inside

/-- Validating a nonempty suffix checks its first row and then the remaining
rows at the next position. This equation retains all stored occurrences. -/
theorem validNodes_cons (atoms first : Nat) (arena : Slice Usize)
    (node : theory.Node) (rest : List theory.Node) :
    ValidNodes atoms first arena (node :: rest) ↔
      ValidNode atoms first arena node ∧ ValidNodes atoms (first + 1) arena rest := by
  constructor
  · intro valid
    constructor
    · have head : ValidNode atoms (first + 0) arena (node :: rest)[0] :=
        valid 0 (by simp)
      change ValidNode atoms first arena node at head
      exact head
    · intro position inside
      have nextInside : position + 1 < (node :: rest).length := by simpa using inside
      have child : ValidNode atoms (first + (position + 1)) arena
          (node :: rest)[position + 1] := valid (position + 1) nextInside
      change ValidNode atoms (first + (position + 1)) arena rest[position] at child
      simpa only [Nat.add_assoc, Nat.add_comm, Nat.add_left_comm] using child
  · rintro ⟨head, tail⟩ position inside
    cases position with
    | zero => simpa only [Nat.add_zero, List.getElem_cons_zero] using head
    | succ position =>
      have remaining : position < rest.length := by simpa using inside
      simpa only [List.getElem_cons_succ, Nat.add_assoc, Nat.add_comm, Nat.add_left_comm] using tail position remaining

/-- Whole-table validity starts with no preceding nodes. -/
def WellFormed (atoms : Nat) (view : theory.FormulaView) : Prop :=
  ValidNodes atoms 0 view.operands view.nodes.val

/-- Every in-range lookup of a valid table returns a row whose children are
already present at that position and whose atoms lie in the universe. The row
equation concerns the actual generated lookup, not an assumed output agreement. -/
theorem lookup_valid (atoms : Nat) (view : theory.FormulaView)
    (valid : WellFormed atoms view) (index : Usize)
    (inside : index.val < view.nodes.val.length) :
    ∃ row, theory.FormulaView.node view index = Result.ok (.Ok row) ∧
      ChildrenBefore index.val row ∧ AtomBound atoms row := by
  obtain ⟨row, decoded, children, atom⟩ := valid index.val inside
  refine ⟨row, ?_, by simpa using children, atom⟩
  rw [NativeRows.formula_view_exact]
  simp only [NativeRows.tableResult, List.getElem?_eq_getElem inside, decoded]

end NativeStructure
