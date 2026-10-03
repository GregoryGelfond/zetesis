import AdmissionValidation

open Aeneas Aeneas.Std Result
open ZetesisExtract
open Zetesis.Refinement

/-!
# Refusal order and repeated roots in the generated validators

These examples run the unchanged generated validators on small tables. A node
refusal reports the error of the first refused node in stored order, even when
a later node would be refused for a different reason. Repeated stored roots are
admitted, an unstored root is refused even after repeated stored ones, and
empty input is admitted.
-/
namespace AdmissionValidationExample

/-- An out-of-universe atom stored before a self-referencing conjunction. -/
def atomBeforeEdge : Slice theory.Node :=
  Slice.from [.Atom 1#usize, .And 1#usize 1#usize] (by scalar_tac)

/-- The same two refused nodes in the opposite order. -/
def edgeBeforeAtom : Slice theory.Node :=
  Slice.from [.And 0#usize 0#usize, .Atom 1#usize] (by scalar_tac)

/-- With one atom in the universe, the atom refusal at position zero is
reported although the conjunction at position one refers to itself. -/
theorem atom_refusal_precedes_later_edge :
    theory.validate_nodes 1#usize atomBeforeEdge = ok (.Err .Atom) := by
  rw [AdmissionValidation.validate_nodes_exact]
  simp [atomBeforeEdge, Slice.from_val, TheoryAdmission.scan, TheoryAdmission.node,
    EvaluationSemantics.node, AdmissionValidation.verdict, AdmissionValidation.refusal,
    Except.bind]

/-- In the opposite order, the self-reference at position zero is reported
although the atom at position one lies outside the universe. -/
theorem edge_refusal_precedes_later_atom :
    theory.validate_nodes 1#usize edgeBeforeAtom = ok (.Err .Edge) := by
  rw [AdmissionValidation.validate_nodes_exact]
  simp [edgeBeforeAtom, Slice.from_val, TheoryAdmission.scan, TheoryAdmission.node,
    EvaluationSemantics.node, AdmissionValidation.verdict, AdmissionValidation.refusal,
    Except.bind]

/-- Falsum needs no atom and no earlier node, even in an empty universe. -/
theorem falsum_is_admitted :
    theory.validate_nodes 0#usize (Slice.from [.False] (by scalar_tac)) = ok (.Ok ()) := by
  rw [AdmissionValidation.validate_nodes_exact]
  simp [Slice.from_val, TheoryAdmission.scan, TheoryAdmission.node,
    EvaluationSemantics.node, AdmissionValidation.verdict, Except.bind]

/-- An empty table is admitted against an empty universe. -/
theorem empty_nodes_are_admitted :
    theory.validate_nodes 0#usize (Slice.new theory.Node) = ok (.Ok ()) := by
  rw [AdmissionValidation.validate_nodes_exact]
  simp [Slice.new_val, TheoryAdmission.scan, AdmissionValidation.verdict]

/-- Empty roots are admitted even when no node is stored. -/
theorem empty_roots_are_admitted :
    theory.validate_roots 0#usize (Slice.new Usize) = ok (.Ok ()) := by
  rw [AdmissionValidation.validate_roots_exact]
  simp [Slice.new_val, TheoryAdmission.rootScan, AdmissionValidation.verdict]

/-- Repeating a stored root is admitted. -/
theorem repeated_stored_roots_are_admitted :
    theory.validate_roots 1#usize (Slice.from [0#usize, 0#usize] (by scalar_tac)) =
      ok (.Ok ()) := by
  rw [AdmissionValidation.validate_roots_exact]
  simp [Slice.from_val, TheoryAdmission.rootScan, TheoryAdmission.root,
    AdmissionValidation.verdict, Except.bind]

/-- An unstored root is refused even after repeated stored roots. -/
theorem missing_root_after_repetitions_is_refused :
    theory.validate_roots 1#usize (Slice.from [0#usize, 0#usize, 1#usize] (by scalar_tac)) =
      ok (.Err .Root) := by
  rw [AdmissionValidation.validate_roots_exact]
  simp [Slice.from_val, TheoryAdmission.rootScan, TheoryAdmission.root,
    AdmissionValidation.verdict, AdmissionValidation.refusal, Except.bind]

end AdmissionValidationExample
