import TightClassificationSemantics
import TightWork
import Iteration

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisExtract
open TightClassificationSemantics

/-!
# One actual tight-body classification step

The generated loop inspects an admitted node, charges one unit, and appends its
class. Bounds on the initialized prefix justify child-class reads; the original
node table supplies the exact falsum test for default negation. No semantic
classification result is assumed. A refused tick appends nothing, and an
exhausted iterator does not tick. Runtime observation histories are separate
from these exact per-invocation equations.
-/
namespace TightClassificationStep

abbrev Cursor := core.slice.iter.Iter theory.Node
abbrev Classes := alloc.vec.Vec tight.compile.Body
abbrev Transition := ControlFlow (Cursor × tight.Work × Classes)
  (core.result.Result Classes tight.TightError × tight.Work)

/-- Equality with falsum inspects the actual generated node constructor.
No formula equivalence or truth test is substituted for this structural test. -/
theorem false_test (node : theory.Node) :
    theory.Node.Insts.CoreCmpPartialEqNode.eq node .False =
      ok (match node with | .False => true | _ => false) := by
  cases node <;> simp [theory.Node.Insts.CoreCmpPartialEqNode.eq, theory.Node.read_discriminant]

/-- A typed work refusal stops at this node without appending a class. -/
theorem step_stopped (program : theory.Theory) (cursor nextCursor : Cursor)
    (work stopped : tight.Work) (classes : Classes) (node : theory.Node)
    (reason : tight.TightError)
    (advanced : core.slice.iter.IteratorSliceIter.next cursor = ok (some node, nextCursor))
    (ticked : tight.Work.tick work = ok (.Err reason, stopped)) :
    tight.compile.classify_loop.body program cursor work classes =
      ok (.done (.Err reason, stopped)) := by
  simp [tight.compile.classify_loop.body, advanced, ticked,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
    core.convert.FromSame.from]

/-- No remaining node means successful completion with no additional work. -/
theorem exhausted (program : theory.Theory) (cursor : Cursor)
    (work : tight.Work) (classes : Classes)
    (finished : cursor.slice.val.length ≤ cursor.i) :
    tight.compile.classify_loop.body program cursor work classes =
      ok (.done (.Ok classes, work)) := by
  simp [tight.compile.classify_loop.body,
    EvaluatorIteration.slice_next_exhausted cursor finished]

/-- After a successful iterator step and tick, the actual body appends exactly
one class computed from its stored inputs. Every read is bounded by the admitted
prefix. The theorem does not assume that the resulting class is semantically
correct; `classes_exact` supplies that independent correspondence. -/
theorem step_after_tick (program : theory.Theory) (cursor nextCursor : Cursor)
    (work charged : tight.Work) (classes : Classes) (node : theory.Node)
    (advanced : core.slice.iter.IteratorSliceIter.next cursor = ok (some node, nextCursor))
    (ticked : tight.Work.tick work = ok (.Ok (), charged))
    (before : EvaluationSpecification.ChildrenBefore classes.val.length node)
    (within : classes.val.length ≤ program.value.nodes.val.length)
    (room : classes.val.length < Usize.max) :
    ∃ extended : Classes,
      tight.compile.classify_loop.body program cursor work classes =
        ok (.cont (nextCursor, charged, extended)) ∧
      extended.val = classes.val ++ [classifyNode program.value.nodes.val classes.val node] := by
  obtain ⟨extended, appended, exactValues⟩ := EvaluatorIteration.vector_append classes
    (classifyNode program.value.nodes.val classes.val node) room
  refine ⟨extended, ?_, exactValues⟩
  simp only [tight.compile.classify_loop.body, advanced, bind_tc_ok, ticked,
    core.result.Result.Insts.CoreOpsTry.branch, Aeneas.Std.uncurry]
  cases node with
  | Atom atom | False =>
    simp only [classifyNode] at appended
    simp only [appended, bind_tc_ok]
  | And left right | Or left right =>
    have leftInside : left.val < classes.val.length := before.1
    have rightInside : right.val < classes.val.length := before.2
    have leftRead := EvaluatorIteration.vector_read classes left before.1
    have rightRead := EvaluatorIteration.vector_read classes right before.2
    simp only [leftRead, rightRead, bind_tc_ok]
    have leftValue : classes.val.getD left.val .Opaque = classes.val[left.val] := by
      simp [List.getD_eq_getElem?_getD, List.getElem?_eq_getElem before.1]
    have rightValue : classes.val.getD right.val .Opaque = classes.val[right.val] := by
      simp [List.getD_eq_getElem?_getD, List.getElem?_eq_getElem before.2]
    simp only [classifyNode, leftValue, rightValue] at appended
    cases leftClass : classes.val[left.val] <;> cases rightClass : classes.val[right.val] <;>
      simp [leftClass, rightClass, combine] at appended ⊢ <;> simp [appended]
  | Implies left right =>
    have inside : right.val < program.value.nodes.val.length := Nat.lt_of_lt_of_le before.2 within
    have stored : program.value.nodes.val[right.val]? = some program.value.nodes.val[right.val] :=
      List.getElem?_eq_getElem inside
    have nodesRead : theory.Theory.nodes program =
        ok (alloc.vec.Vec.deref program.value.nodes) := by
      simp [theory.Theory.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref]
    have rightRead : Slice.index_usize (alloc.vec.Vec.deref program.value.nodes) right =
        ok program.value.nodes.val[right.val] := by
      simp [Slice.index_usize, alloc.vec.Vec.deref, List.getElem?_eq_getElem inside]
    simp only [nodesRead, bind_tc_ok, rightRead, false_test]
    simp only [classifyNode, stored] at appended
    cases child : program.value.nodes.val[right.val] <;>
      simp [child] at appended ⊢ <;> simp [appended]

end TightClassificationStep
