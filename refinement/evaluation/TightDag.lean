import AdmissionValidation

open Aeneas Aeneas.Std Result
open ZetesisExtract
open Zetesis

/-!
# The formula at a recognized DAG node

An ordered extracted node table unfolds to the formulas of `DagSharing`.
Successful stored-node lookups determine the exact corresponding formula layer;
the children retain their meanings in the same immutable table. This supplies
syntactic meaning to bounded shape recognizers without assuming their semantic
correctness. The lookup premises are data reads, not evaluator truth equations.

`meaning` is total, with falsum outside the table. Every stored-node theorem
requires a successful lookup and derives its bound, so that default cannot
justify recognition. Atom-universe admission, source grounding and the actual
recognizer's control flow are separate obligations.
-/
namespace TightDag

/-- The unfolded formula at a stored machine index in this theory. -/
def meaning (program : theory.Theory) (index : Usize) : Ferraris.Formula Nat :=
  (DagSharing.meanings (program.value.nodes.val.map EvaluationSemantics.node)).getD
    index.val .bot

/-- The evaluator's child-order invariant supplies structural DAG admission.
The proof adds the last node to the admitted prefix; its children are already
inside that prefix by the same invariant. No atom-universe premise is needed. -/
theorem ordered_well_formed (table : List theory.Node)
    (ordered : EvaluationSpecification.Ordered table) :
    DagSharing.WellFormed (table.map EvaluationSemantics.node) := by
  induction table using List.reverseRecOn with
  | nil => exact DagSharing.WellFormed.nil
  | append_singleton table last inductionHypothesis =>
    have prefixOrder : EvaluationSpecification.Ordered table := by
      intro index inside
      have whole := ordered index (by simp; omega)
      simpa only [List.getElem_append_left inside] using whole
    have lastOrder : EvaluationSpecification.ChildrenBefore table.length last := by
      simpa using ordered table.length (by simp)
    have lastValid : DagSharing.ValidNode (table.map EvaluationSemantics.node).length
        (EvaluationSemantics.node last) := by
      simpa using (AdmissionValidation.valid_node_iff table.length last).mpr lastOrder
    simpa using DagSharing.WellFormed.snoc (inductionHypothesis prefixOrder) lastValid

/-- A successful lookup gives the exact formula layer at that position.
`stored_meaning` for well-formed DAGs makes all child lookups refer to the
completed table without changing their previously unfolded meanings. -/
theorem stored_meaning (program : theory.Theory) (index : Usize) (entry : theory.Node)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (read : program.value.nodes.val[index.val]? = some entry) :
    meaning program index = DagSharing.decode
      (DagSharing.meanings (program.value.nodes.val.map EvaluationSemantics.node))
      (EvaluationSemantics.node entry) := by
  have inside : index.val < program.value.nodes.val.length :=
    List.getElem?_eq_some_iff.mp read |>.1
  have converted : (program.value.nodes.val.map EvaluationSemantics.node).getD
      index.val .bot = EvaluationSemantics.node entry := by
    simp [List.getD_eq_getElem?_getD, read]
  have decoded := DagSharing.stored_meaning
    (ordered_well_formed program.value.nodes.val ordered) index.val (by simpa using inside)
  rw [converted] at decoded
  exact decoded.symm

/-- Reading an atom node determines its semantic atom, independently of the
node's occurrence identity. -/
theorem stored_atom (program : theory.Theory) (index atom : Usize)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (read : program.value.nodes.val[index.val]? = some (.Atom atom)) :
    meaning program index = .atom atom.val := by
  simpa [EvaluationSemantics.node, DagSharing.decode] using
    stored_meaning program index (.Atom atom) ordered read

/-- Reading the falsum constructor determines falsum, rather than an absent
lookup's default meaning. -/
theorem stored_false (program : theory.Theory) (index : Usize)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (read : program.value.nodes.val[index.val]? = some .False) :
    meaning program index = .bot := by
  simpa [EvaluationSemantics.node, DagSharing.decode] using
    stored_meaning program index .False ordered read

/-- A stored disjunction denotes the disjunction of its two stored children. -/
theorem stored_or (program : theory.Theory) (index left right : Usize)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (read : program.value.nodes.val[index.val]? = some (.Or left right)) :
    meaning program index = .disj (meaning program left) (meaning program right) := by
  simpa [meaning, EvaluationSemantics.node, DagSharing.decode] using
    stored_meaning program index (.Or left right) ordered read

/-- A stored implication denotes the implication between its stored children.
This is formula equality, not only classical truth equivalence. -/
theorem stored_implies (program : theory.Theory) (index left right : Usize)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (read : program.value.nodes.val[index.val]? = some (.Implies left right)) :
    meaning program index = .imp (meaning program left) (meaning program right) := by
  simpa [meaning, EvaluationSemantics.node, DagSharing.decode] using
    stored_meaning program index (.Implies left right) ordered read

end TightDag
