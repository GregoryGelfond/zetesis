import Evaluator.Funs
import Membership
import Iteration

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisExtract

namespace Evaluation

/-- A formula node reads only entries already present in its truth prefix. -/
def ChildrenPresent (truths : List Bool) : theory.Node → Prop
  | .Atom _ | .False => True
  | .And left right | .Or left right | .Implies left right =>
    left.val < truths.length ∧ right.val < truths.length

/-- The supplied frozen mask, when present, has the current node's position. -/
def MaskPresent (frozen : Option (Slice Bool)) (index : Usize) : Prop :=
  ∀ mask ∈ frozen, index.val < mask.val.length

/-- Boolean meaning of one node, over exact packed atom truth and an already
computed prefix. The child-bounds premise excludes default list reads. -/
def value (candidate : theory.Interpretation) (truths : List Bool) : theory.Node → Bool
  | .Atom atom => Membership.denotes candidate atom.val
  | .False => false
  | .And left right => truths[left.val]?.getD false && truths[right.val]?.getD false
  | .Or left right => truths[left.val]?.getD false || truths[right.val]?.getD false
  | .Implies left right => !truths[left.val]?.getD false || truths[right.val]?.getD false

/-- Masking retains truth only at positions selected by the supplied frozen mask. -/
def masked (frozen : Option (Slice Bool)) (index : Usize) (truth : Bool) : Bool :=
  truth && (frozen.map (fun mask => mask.val[index.val]?.getD false)).getD true

/-- A bounded vector read yields the stored truth; the total specification's
absent-entry default is unused. -/
theorem read_exact (output : alloc.vec.Vec Bool) (index : Usize)
    (bounded : index.val < output.val.length) :
    alloc.vec.Vec.index (core.slice.index.SliceIndexUsizeSlice Bool) output index =
      ok (output.val[index.val]?.getD false) := by
  rw [EvaluatorIteration.vector_read output index bounded]
  simp [List.getElem?_eq_getElem bounded]

/-- Mask lookup uses the generated closure, including its real slice indexing.
A missing optional mask supplies true without running the closure. -/
theorem mask_exact (frozen : Option (Slice Bool)) (index : Usize)
    (bounded : MaskPresent frozen index) :
    core.option.Option.is_none_or
      oracle.evaluate.closure.Insts.CoreOpsFunctionFnOnceTupleSharedSliceBoolBool
      frozen index = ok ((frozen.map (fun mask => mask.val[index.val]?.getD false)).getD true) := by
  cases frozen with
  | none => rfl
  | some mask =>
    have present : index.val < mask.val.length := bounded mask (by simp)
    simp [core.option.Option.is_none_or,
      oracle.evaluate.closure.Insts.CoreOpsFunctionFnOnceTupleSharedSliceBoolBool.call_once,
      Slice.index_usize, List.getElem?_eq_getElem present]

/-- The generated continuation reads a mask only when node truth is true.
With a present mask index and room to append, this produces exactly masked
truth and preserves the entire preceding prefix. The continuation is arbitrary. -/
theorem masked_append {α : Type} (output : alloc.vec.Vec Bool)
    (frozen : Option (Slice Bool)) (index : Usize) (truth : Bool)
    (resume : alloc.vec.Vec Bool → α) (bounded : MaskPresent frozen index)
    (room : output.val.length < Usize.max) :
    ∃ extended : alloc.vec.Vec Bool,
      (if truth then do
        let selected ← core.option.Option.is_none_or
          oracle.evaluate.closure.Insts.CoreOpsFunctionFnOnceTupleSharedSliceBoolBool
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

/-- The source iterator is an enumerated immutable formula slice. -/
abbrev Cursor := core.iter.adapters.enumerate.Enumerate (core.slice.iter.Iter theory.Node)

/-- The extracted loop either retains its iterator and work fields for another
node, or returns a typed result with the current prefix and complete work record. -/
abbrev Transition := ControlFlow
  (Cursor × alloc.vec.Vec Bool × oracle.Limits ×
    zetesis_cpu.cancellation.Cancellation × oracle.Statistics)
  (core.result.Result Unit zetesis_cpu.cancellation.Stop × alloc.vec.Vec Bool × oracle.Work)

/-- After the real iterator and tick succeed, the generated body appends exactly
one masked node value. Child and mask bounds justify reads; packed representation
justifies atom truth. No node-evaluation or output-agreement premise is supplied.
`EvaluationProgress.present_step` discharges the iterator and tick equations. -/
theorem step_after_tick (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (cursor nextCursor : Cursor)
    (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (index : Usize) (node : theory.Node) (charged : oracle.Work)
    (advanced : core.iter.adapters.enumerate.IteratorEnumerate.next
      (core.iter.traits.iterator.IteratorSliceIter theory.Node) cursor =
        ok (some (index, node), nextCursor))
    (ticked : oracle.Work.tick ⟨limits, control, statistics⟩ =
      ok (core.result.Result.Ok (), charged))
    (stored : Membership.Represented candidate)
    (children : ChildrenPresent output.val node) (mask : MaskPresent frozen index)
    (room : output.val.length < Usize.max) :
    ∃ extended : alloc.vec.Vec Bool,
      oracle.evaluate_loop.body candidate frozen cursor output limits control statistics =
        ok (.cont (nextCursor, extended, charged.limits, charged.cancellation, charged.statistics)) ∧
      extended.val = output.val ++ [masked frozen index (value candidate output.val node)] := by
  obtain ⟨extended, finished, exactPrefix⟩ := masked_append output frozen index
    (value candidate output.val node)
    (fun next => (cont (nextCursor, next, charged.limits, charged.cancellation,
      charged.statistics) : Transition)) mask room
  refine ⟨extended, ?_, exactPrefix⟩
  simp only [oracle.evaluate_loop.body, advanced, bind_tc_ok, ticked,
    core.result.Result.Insts.CoreOpsTry.branch, Aeneas.Std.uncurry]
  cases node with
  | Atom atom =>
    simpa only [value, Cursor, Membership.contains_refines candidate atom stored,
      bind_tc_ok, Aeneas.Std.uncurry] using finished
  | False => simpa [value] using finished
  | And left right =>
    have leftRead := read_exact output left children.1
    have rightRead := read_exact output right children.2
    simp only [leftRead, rightRead, bind_tc_ok]
    cases leftTruth : output.val[left.val]?.getD false <;>
      simpa only [value, leftTruth, Bool.false_and, Bool.true_and, Bool.false_or,
        Bool.true_or, Bool.not_true, Bool.not_false, Bool.false_eq_true,
        if_false, if_true, bind_tc_ok, Aeneas.Std.uncurry] using finished
  | Or left right =>
    have leftRead := read_exact output left children.1
    have rightRead := read_exact output right children.2
    simp only [leftRead, rightRead, bind_tc_ok]
    cases leftTruth : output.val[left.val]?.getD false <;>
      simpa only [value, leftTruth, Bool.false_and, Bool.true_and, Bool.false_or,
        Bool.true_or, Bool.not_true, Bool.not_false, Bool.false_eq_true,
        if_false, if_true, bind_tc_ok, Aeneas.Std.uncurry] using finished
  | Implies left right =>
    have leftRead := read_exact output left children.1
    have rightRead := read_exact output right children.2
    simp only [leftRead, rightRead, bind_tc_ok]
    cases leftTruth : output.val[left.val]?.getD false <;>
      simpa only [value, leftTruth, Bool.false_and, Bool.true_and, Bool.false_or,
        Bool.true_or, Bool.not_true, Bool.not_false, Bool.false_eq_true,
        if_false, if_true, bind_tc_ok, Aeneas.Std.uncurry] using finished

/-- A typed stop from the actual tick exits before evaluating or appending a
node. It preserves the previous output and the complete returned work record. -/
theorem step_stops (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (cursor nextCursor : Cursor)
    (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (index : Usize) (node : theory.Node) (returned : oracle.Work)
    (reason : zetesis_cpu.cancellation.Stop)
    (advanced : core.iter.adapters.enumerate.IteratorEnumerate.next
      (core.iter.traits.iterator.IteratorSliceIter theory.Node) cursor =
        ok (some (index, node), nextCursor))
    (stopped : oracle.Work.tick ⟨limits, control, statistics⟩ =
      ok (core.result.Result.Err reason, returned)) :
    oracle.evaluate_loop.body candidate frozen cursor output limits control statistics =
      ok (.done (core.result.Result.Err reason, output, returned)) := by
  simp [oracle.evaluate_loop.body, advanced, stopped,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- Exhausting the actual slice completes without polling or charging a node.
No storage, mask, truth, cancellation or remaining-work premise is needed. -/
theorem exhausted_step (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (cursor : Cursor)
    (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (exhausted : cursor.iter.slice.val.length ≤ cursor.iter.i) :
    oracle.evaluate_loop.body candidate frozen cursor output limits control statistics =
      ok (.done (core.result.Result.Ok (), output, ⟨limits, control, statistics⟩)) := by
  simp [oracle.evaluate_loop.body, EvaluatorIteration.enumerate_next_exhausted cursor exhausted]

end Evaluation
