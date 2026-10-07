import NativeEvaluation
import Zetesis.OperandTable

open Aeneas Aeneas.Std
open ZetesisNativeExtract
open Zetesis Zetesis.Refinement Ferraris TightEvaluation

/-!
# Original and frozen truth for one native decoded row

The representation map below supplies the existing general OperandTable laws
with the actual NodeView's children. It is mathematical decoding only: no second
runtime graph or binary expansion is introduced. The masked layer law permits
arbitrary outer and tested interpretations. It does not assume that a caller's
stored mask has been computed by an original pass; table composition must prove
that provenance separately.
-/
namespace NativeSemantics

/-- The actual occurrence row, viewed as natural indices for the existing
mathematical representation. Non-group nodes need no operand arena. -/
def arena : theory.NodeView → List Nat
  | .And row | .Or row => row.val.map UScalar.val
  | _ => []

/-- A decoded native row in the existing general layer representation. Groups
use their whole returned row, preserving duplicates and operand order. -/
def entry : theory.NodeView → OperandTable.Node Nat
  | .Atom atom => .atom atom.val
  | .False => .bot
  | .Implies left right => .imp left.val right.val
  | .And row => .group .conj ⟨0, row.val.length⟩
  | .Or row => .group .disj ⟨0, row.val.length⟩

/-- Decode one native layer from its previously decoded formulas. -/
def meaning (earlier : List (Formula Nat)) (node : theory.NodeView) : Formula Nat :=
  OperandTable.decode (arena node) earlier (entry node)

/-- Native backward references establish the existing layer-validity premise.
The whole-row span is an exact mathematical view, so it cannot be clipped. -/
theorem valid (earlier : Nat) (node : theory.NodeView)
    (children : NativeStructure.ChildrenBefore earlier node) :
    OperandTable.Valid (arena node) earlier (entry node) := by
  cases node with
  | Atom atom | False => trivial
  | Implies left right => exact children
  | And row | Or row =>
    constructor
    · simp [arena, OperandArena.Fits]
    · intro child member
      simp only [arena, OperandArena.contents, Zetesis.AdjacencyRows.slice,
        List.drop_zero, ← List.map_take, List.take_length] at member
      obtain ⟨source, sourceMember, equal⟩ := List.mem_map.mp member
      rw [← equal]
      exact children source sourceMember

/-- The Boolean view operation is the successful checked general layer read.
Child existence justifies every totalized reference expression. -/
theorem read_value (candidate : theory.Interpretation) (truths : List Bool)
    (node : theory.NodeView)
    (children : NativeStructure.ChildrenBefore truths.length node) :
    OperandTable.read (NativeMembership.denotes candidate) (arena node) truths (entry node) =
      some (NativeEvaluation.value candidate truths node) := by
  cases node with
  | Atom atom | False => rfl
  | Implies left right =>
    simp [OperandTable.read, entry, NativeEvaluation.value,
      List.getElem?_eq_getElem children.1, List.getElem?_eq_getElem children.2]
  | And row =>
    have admitted : OperandArena.Admitted (row.val.map UScalar.val) truths.length
        ⟨0, row.val.length⟩ := by
      exact valid truths.length (.And row) children
    rw [show OperandTable.read (NativeMembership.denotes candidate) (arena (.And row)) truths
      (entry (.And row)) = OperandArena.evaluate .conj truths
        (row.val.map UScalar.val) ⟨0, row.val.length⟩ by rfl]
    rw [OperandArena.evaluate_exact _ _ _ _ admitted]
    simp only [NativeEvaluation.value, OperandArena.contents, Zetesis.AdjacencyRows.slice,
      List.drop_zero, ← List.map_take, List.take_length, List.map_map, List.getD_eq_getElem?_getD]
    rfl
  | Or row =>
    have admitted : OperandArena.Admitted (row.val.map UScalar.val) truths.length
        ⟨0, row.val.length⟩ := by
      exact valid truths.length (.Or row) children
    rw [show OperandTable.read (NativeMembership.denotes candidate) (arena (.Or row)) truths
      (entry (.Or row)) = OperandArena.evaluate .disj truths
        (row.val.map UScalar.val) ⟨0, row.val.length⟩ by rfl]
    rw [OperandArena.evaluate_exact _ _ _ _ admitted]
    simp only [NativeEvaluation.value, OperandArena.contents, Zetesis.AdjacencyRows.slice,
      List.drop_zero, ← List.map_take, List.take_length, List.map_map, List.getD_eq_getElem?_getD]
    rfl

/-- Original child truths produce the original truth of this decoded native
layer. The semantic argument is the general OperandTable original-layer law. -/
theorem original (candidate : theory.Interpretation) (earlier : List (Formula Nat))
    (node : theory.NodeView)
    (children : NativeStructure.ChildrenBefore earlier.length node) :
    NativeEvaluation.value candidate
        (earlier.map (formulaValue (NativeMembership.denotes candidate))) node =
      formulaValue (NativeMembership.denotes candidate) (meaning earlier node) := by
  have mappedChildren : NativeStructure.ChildrenBefore
      (earlier.map (formulaValue (NativeMembership.denotes candidate))).length node := by
    simpa only [List.length_map] using children
  have read : OperandTable.read (NativeMembership.denotes candidate) (arena node)
      (earlier.map (formulaValue (NativeMembership.denotes candidate))) (entry node) =
        some (formulaValue (NativeMembership.denotes candidate)
          (OperandTable.decode (arena node) earlier (entry node))) := by
    exact OperandTable.read_original_exact (NativeMembership.denotes candidate)
      (arena node) earlier (entry node) (valid earlier.length node children)
  rw [read_value candidate _ node mappedChildren] at read
  exact Option.some.inj read

/-- Frozen child truths, masked by this layer's actual original truth, produce
its exact Ferraris reduct truth. The tested interpretation is unrestricted;
there is no hidden subset premise or implication simplification. -/
theorem frozen (outer tested : theory.Interpretation) (earlier : List (Formula Nat))
    (node : theory.NodeView)
    (children : NativeStructure.ChildrenBefore earlier.length node) :
    (NativeEvaluation.value tested
        (earlier.map (fun child => formulaValue (NativeMembership.denotes tested)
          (Reduct (interpretation (NativeMembership.denotes outer)) child))) node &&
        formulaValue (NativeMembership.denotes outer) (meaning earlier node)) =
      formulaValue (NativeMembership.denotes tested)
        (Reduct (interpretation (NativeMembership.denotes outer)) (meaning earlier node)) := by
  have mappedChildren : NativeStructure.ChildrenBefore
      (earlier.map (fun child => formulaValue (NativeMembership.denotes tested)
        (Reduct (interpretation (NativeMembership.denotes outer)) child))).length node := by
    simpa only [List.length_map] using children
  have read : (OperandTable.read (NativeMembership.denotes tested) (arena node)
      (earlier.map (fun child => formulaValue (NativeMembership.denotes tested)
        (Reduct (interpretation (NativeMembership.denotes outer)) child))) (entry node)).map
          (fun inner => inner && formulaValue (NativeMembership.denotes outer)
            (OperandTable.decode (arena node) earlier (entry node))) =
        some (formulaValue (NativeMembership.denotes tested)
          (Reduct (interpretation (NativeMembership.denotes outer))
            (OperandTable.decode (arena node) earlier (entry node)))) := by
    exact OperandTable.read_reduct_exact (NativeMembership.denotes outer)
      (NativeMembership.denotes tested) (arena node) earlier (entry node)
      (valid earlier.length node children)
  rw [read_value tested _ node mappedChildren] at read
  exact Option.some.inj read

end NativeSemantics
