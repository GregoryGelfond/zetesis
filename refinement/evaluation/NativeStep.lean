import NativeEvaluation
import NativeRange

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisNativeExtract

/-!
# One actual native table-loop invocation

A successful invocation publishes one completed node value. A refused invocation
keeps the prior output prefix but retains all charged node and operand work.
The range, decoded row, bounded truth prefix and optional mask are the same
objects consumed by the extracted function. No assumed evaluation agreement is
used; only the getter's successful row equation is supplied by admission.
-/
namespace NativeStep

abbrev Cursor := core.ops.range.Range Usize
abbrev Transition := ControlFlow (Cursor × alloc.vec.Vec Bool × oracle.Work)
  (core.result.Result Unit zetesis_cpu.cancellation.Stop × alloc.vec.Vec Bool × oracle.Work)

/-- A supplied mask has an entry at the current logical-node position. -/
def MaskPresent (frozen : Option (Slice Bool)) (index : Usize) : Prop :=
  ∀ mask ∈ frozen, index.val < mask.val.length

/-- Masking selects node truth; absence of a mask describes the original pass. -/
def masked (frozen : Option (Slice Bool)) (index : Usize) (truth : Bool) : Bool :=
  truth && (frozen.map (fun mask => mask.val[index.val]?.getD false)).getD true

/-- The real generated mask closure reads the represented current bit. -/
theorem mask_exact (frozen : Option (Slice Bool)) (index : Usize)
    (bounded : MaskPresent frozen index) :
    core.option.Option.is_none_or
      oracle.evaluate.closure_1.Insts.CoreOpsFunctionFnOnceTupleSharedSliceBoolBool
      frozen index = ok ((frozen.map (fun mask => mask.val[index.val]?.getD false)).getD true) := by
  cases frozen with
  | none => rfl
  | some mask =>
    have present : index.val < mask.val.length := by
      exact bounded mask (by simp)
    simp [core.option.Option.is_none_or,
      oracle.evaluate.closure_1.Insts.CoreOpsFunctionFnOnceTupleSharedSliceBoolBool.call_once,
      Slice.index_usize, List.getElem?_eq_getElem present]

/-- A completed truth is appended only after its optional mask read succeeds.
Every prior output cell is retained, including a false current node. -/
theorem masked_append {α : Type} (output : alloc.vec.Vec Bool)
    (frozen : Option (Slice Bool)) (index : Usize) (truth : Bool)
    (resume : alloc.vec.Vec Bool → α) (bounded : MaskPresent frozen index)
    (room : output.val.length < Usize.max) :
    ∃ extended : alloc.vec.Vec Bool,
      (if truth then do
        let selected ← core.option.Option.is_none_or
          oracle.evaluate.closure_1.Insts.CoreOpsFunctionFnOnceTupleSharedSliceBoolBool
          frozen index
        let next ← alloc.vec.Vec.push output selected
        ok (resume next)
      else do
        let next ← alloc.vec.Vec.push output false
        ok (resume next)) = ok (resume extended) ∧
      extended.val = output.val ++ [masked frozen index truth] := by
  obtain ⟨extended, pushed, exactPrefix⟩ :=
    EvaluatorIteration.vector_append output (masked frozen index truth) room
  refine ⟨extended, ?_, exactPrefix⟩
  cases truth <;> simp [masked] at pushed ⊢
  · simp [pushed]
  · rw [mask_exact frozen index bounded]
    simp [pushed]

/-- Complete nodes advance one position and append their masked value; refused
nodes append nothing. Both cases retain an exact receipt, including a partial
operand visit. No source-level error can masquerade as successful exhaustion. -/
def PresentEffect (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (cursor : Cursor) (output : alloc.vec.Vec Bool) (work : oracle.Work)
    (row : theory.NodeView) (used : Nat) : Transition → Prop
  | .cont (next, extended, returned) =>
      next.start.val = cursor.start.val + 1 ∧ next.end = cursor.end ∧
      extended.val = output.val ++
        [masked frozen cursor.start (NativeEvaluation.value candidate output.val row)] ∧
      NativeOperands.Receipt work returned used ∧ used = 1 + NativeEvaluation.occurrences row
  | .done (result, retained, returned) =>
      (∃ reason, result = .Err reason) ∧ retained = output ∧
      NativeOperands.Receipt work returned used ∧ used < 1 + NativeEvaluation.occurrences row

/-- Every in-range invocation yields either a complete append or a typed stop
with the exact partial work. Iterator progress and all node/operand reads are
proved from their generated definitions and bounded imported primitives. -/
theorem present_refines (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (view : theory.FormulaView) (cursor : Cursor) (output : alloc.vec.Vec Bool)
    (work : oracle.Work) (row : theory.NodeView)
    (inside : cursor.start.val < cursor.end.val)
    (lookup : theory.FormulaView.node view cursor.start = ok (.Ok row))
    (stored : NativeMembership.Represented candidate)
    (children : NativeStructure.ChildrenBefore output.val.length row)
    (covered : MaskPresent frozen cursor.start)
    (room : output.val.length < Usize.max) :
    ∃ transition used,
      oracle.evaluate_loop.body candidate frozen view.nodes view.operands cursor output work =
        ok transition ∧ PresentEffect candidate frozen cursor output work row used transition := by
  obtain ⟨next, advanced, progressed, sameEnd⟩ := NativeRange.range_present cursor inside
  rcases NativeEvaluation.tick_cases work with ⟨reason, stopped⟩ | ⟨charged, tick, tickReceipt⟩
  · refine ⟨.done (.Err reason, output, work), 0, ?_,
      ⟨reason, rfl⟩, rfl, NativeOperands.Receipt.refl work, by omega⟩
    simp [oracle.evaluate_loop.body, advanced, stopped,
      core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
  · obtain ⟨result, returned, used, evaluated, receipt, bounded, boundary⟩ :=
      NativeEvaluation.node_refines row candidate (alloc.vec.Vec.deref output) charged
        stored (by simpa [alloc.vec.Vec.deref] using children)
    cases result with
    | Err reason =>
      refine ⟨.done (.Err reason, output, returned), 1 + used, ?_,
        ⟨reason, rfl⟩, rfl, tickReceipt.trans receipt, by omega⟩
      simp [oracle.evaluate_loop.body, advanced, tick, lookup,
        core.result.Result.map_err, core.result.Result.Insts.CoreOpsTry.branch,
        evaluated, Aeneas.Std.uncurry,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
    | Ok truth =>
      obtain ⟨extended, appended, exactPrefix⟩ := masked_append output frozen cursor.start truth
        (fun extended => (.cont (next, extended, returned) : Transition)) covered room
      refine ⟨.cont (next, extended, returned), 1 + used, ?_,
        progressed, sameEnd, ?_, tickReceipt.trans receipt, by omega⟩
      · simpa only [oracle.evaluate_loop.body, advanced, bind_tc_ok, tick,
          core.result.Result.Insts.CoreOpsTry.branch, lookup, core.result.Result.map_err,
          evaluated, Aeneas.Std.uncurry] using appended
      · simpa [boundary.2, alloc.vec.Vec.deref] using exactPrefix

/-- Exhaustion performs no tick, lookup, mask read or output append. -/
theorem exhausted (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (view : theory.FormulaView) (cursor : Cursor) (output : alloc.vec.Vec Bool)
    (work : oracle.Work) (ended : cursor.end.val ≤ cursor.start.val) :
    oracle.evaluate_loop.body candidate frozen view.nodes view.operands cursor output work =
      ok (.done (.Ok (), output, work)) := by
  simp [oracle.evaluate_loop.body, NativeRange.range_exhausted cursor ended]

end NativeStep
