import RuntimeEffects
import Control

open Aeneas Aeneas.Std Result ControlFlow Error
open ZetesisExtract

/-!
# Restricted monadic contexts for returning effects

The five contexts below generalize exact generated definitions. Only their
result monad and named effect call sites change; pure operations remain imported
backend calls. ContextAudit checks exact headers, call counts, body text and
reversal separately from the kernel reconstruction laws. This audited source
generalization is an explicit extraction boundary. Whole-loop runtime
correspondence requires additional returning-event projection laws.
-/
namespace RuntimeContexts

variable {M : Type → Type} [Monad M] [MonadLiftT Result M]

def membershipContext
  (readWord : core.sync.atomic.Atomic U64 (core.sync.atomic.private.Align8 U64) →
    core.sync.atomic.Ordering → M U64)
  (self : zetesis_cpu.cancellation.slot.Membership) : M Bool := do
  let a ← alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref Global self.state
  let i ←
    readWord a
      core.sync.atomic.Ordering.Relaxed
  ok (i != self.active)

def deadlineContext
  (read : core.sync.atomic.Atomic Bool (core.sync.atomic.private.Align1 U8) →
    core.sync.atomic.Ordering → M Bool)
  (_c : zetesis_cpu.cancellation.Cancellation.poll.closure)
  (tupled_args : alloc.sync.Arc zetesis_cpu.cancellation.DeadlineOwner) :
  M Bool
  := do
  let «do» ←
    alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref Global tupled_args
  let d ← alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref Global «do».deadline
  read d.expired
    core.sync.atomic.Ordering.Relaxed

def pollContext
  (read : core.sync.atomic.Atomic Bool (core.sync.atomic.private.Align1 U8) →
    core.sync.atomic.Ordering → M Bool)
  (slotCancelled : zetesis_cpu.cancellation.slot.Membership → M Bool)
  (deadline : Option (alloc.sync.Arc zetesis_cpu.cancellation.DeadlineOwner) → M Bool)
  (self : zetesis_cpu.cancellation.Cancellation) :
  M (core.result.Result Unit zetesis_cpu.cancellation.Stop)
  := do
  let a ← alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref Global self.cancelled
  let b ←
    read a
      core.sync.atomic.Ordering.Relaxed
  if b
  then ok (core.result.Result.Err zetesis_cpu.cancellation.Stop.Cancelled)
  else
    let o ← core.option.Option.as_ref self.slot
    let b1 ←
      match o with
      | none => ok false
      | some membership =>
        do
        let b2 ←
          slotCancelled membership
        if b2
        then ok true
        else ok false
    if b1
    then ok (core.result.Result.Err zetesis_cpu.cancellation.Stop.Cancelled)
    else
      let o1 ← core.option.Option.as_ref self.deadline
      let b2 ←
        deadline o1
      if b2
      then ok (core.result.Result.Err zetesis_cpu.cancellation.Stop.Deadline)
      else ok (core.result.Result.Ok ())

def tickContext
  (poll : zetesis_cpu.cancellation.Cancellation →
    M (core.result.Result Unit zetesis_cpu.cancellation.Stop))
  (self : oracle.Work) :
  M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    oracle.Work)
  := do
  let r ← poll self.cancellation
  let cf ← core.result.Result.Insts.CoreOpsTry.branch r
  match cf with
  | core.ops.control_flow.ControlFlow.Continue _ =>
    if self.statistics.work >= self.limits.max_work
    then
      ok (core.result.Result.Err zetesis_cpu.cancellation.Stop.WorkLimit, self)
    else
      let i ← self.statistics.work + 1#u64
      ok (core.result.Result.Ok (),
        { self with statistics := { self.statistics with work := i } })
  | core.ops.control_flow.ControlFlow.Break residual =>
    let r1 ←
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
        Unit (core.convert.FromSame zetesis_cpu.cancellation.Stop) residual
    ok (r1, self)

def evaluationContext
  (tick : oracle.Work → M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) × oracle.Work))
  (interpretation : theory.Interpretation) (frozen : Option (Slice Bool))
  (iter : core.iter.adapters.enumerate.Enumerate (core.slice.iter.Iter
  theory.Node)) (output : alloc.vec.Vec Bool) (l : oracle.Limits)
  (c : zetesis_cpu.cancellation.Cancellation) (s : oracle.Statistics) :
  M (ControlFlow ((core.iter.adapters.enumerate.Enumerate
    (core.slice.iter.Iter theory.Node)) × (alloc.vec.Vec Bool) ×
    oracle.Limits × zetesis_cpu.cancellation.Cancellation ×
    oracle.Statistics) ((core.result.Result Unit zetesis_cpu.cancellation.Stop)
    × (alloc.vec.Vec Bool) × oracle.Work))
  := do
  let (o, iter1) ←
    core.iter.adapters.enumerate.IteratorEnumerate.next
      (core.iter.traits.iterator.IteratorSliceIter theory.Node) iter
  match o with
  | none =>
    ok (done (core.result.Result.Ok (), output,
      { limits := l, cancellation := c, statistics := s }))
  | some p =>
    let (index, node) := p
    let (r, work) ←
      tick { limits := l, cancellation := c, statistics := s }
    let cf ← core.result.Result.Insts.CoreOpsTry.branch r
    match cf with
    | core.ops.control_flow.ControlFlow.Continue _ =>
      let (work1, iter2, value) ←
        match node with
        | theory.Node.Atom atom =>
          do
          let value1 ← theory.Interpretation.contains interpretation atom
          ok (work, iter1, value1)
        | theory.Node.False => ok (work, iter1, false)
        | theory.Node.And a b =>
          do
          let b1 ←
            alloc.vec.Vec.index (core.slice.index.SliceIndexUsizeSlice Bool)
              output a
          let b2 ←
            if b1
            then
              alloc.vec.Vec.index (core.slice.index.SliceIndexUsizeSlice Bool)
                output b
            else ok false
          ok (work, iter1, b2)
        | theory.Node.Or a b =>
          do
          let b1 ←
            alloc.vec.Vec.index (core.slice.index.SliceIndexUsizeSlice Bool)
              output a
          let b2 ←
            if b1
            then ok true
            else
              alloc.vec.Vec.index (core.slice.index.SliceIndexUsizeSlice Bool)
                output b
          ok (work, iter1, b2)
        | theory.Node.Implies a b =>
          do
          let b1 ←
            alloc.vec.Vec.index (core.slice.index.SliceIndexUsizeSlice Bool)
              output a
          let b2 ←
            if b1
            then
              alloc.vec.Vec.index (core.slice.index.SliceIndexUsizeSlice Bool)
                output b
            else ok true
          ok (work, iter1, b2)
      if value
      then
        let b ←
          core.option.Option.is_none_or
            oracle.evaluate.closure.Insts.CoreOpsFunctionFnOnceTupleSharedSliceBoolBool
            frozen index
        let output1 ← alloc.vec.Vec.push output b
        ok (cont (iter2, output1, work1.limits, work1.cancellation,
          work1.statistics))
      else
        let output1 ← alloc.vec.Vec.push output false
        ok (cont (iter2, output1, work1.limits, work1.cancellation,
          work1.statistics))
    | core.ops.control_flow.ControlFlow.Break residual =>
      let r1 ←
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
          Unit (core.convert.FromSame zetesis_cpu.cancellation.Stop) residual
      ok (done (r1, output, work))

/-- The option model performs no callback when absent, exactly as the active
library model. Its callback is generalized only to admit returning effects. -/
def deadlineOption
    (read : core.sync.atomic.Atomic Bool (core.sync.atomic.private.Align1 U8) →
      core.sync.atomic.Ordering → M Bool)
    (value : Option (alloc.sync.Arc zetesis_cpu.cancellation.DeadlineOwner)) : M Bool :=
  match value with
  | none => pure false
  | some owner => deadlineContext read () owner

/-- Filling the word-read hole recovers the actual generated membership body,
including its inequality with the captured active word. -/
theorem membership_reconstruct (member : zetesis_cpu.cancellation.slot.Membership) :
    membershipContext (M := Result) core.sync.atomic.AtomicU64Align8U64.load member =
      zetesis_cpu.cancellation.slot.Membership.is_cancelled member := by rfl

/-- Replacing the one read hole by the active load reconstructs the actual
generated deadline callback without simplifying that load. -/
theorem deadline_reconstruct (closure : zetesis_cpu.cancellation.Cancellation.poll.closure)
    (owner : alloc.sync.Arc zetesis_cpu.cancellation.DeadlineOwner) :
    deadlineContext (M := Result) core.sync.atomic.AtomicBoolAlign1U8.load closure owner =
      zetesis_cpu.cancellation.Cancellation.poll.closure.Insts.CoreOpsFunctionFnOnceTupleSharedArcDeadlineOwnerBool.call_once
        closure owner := by rfl

/-- Generalizing the existing option callback preserves its pure instance. -/
theorem option_reconstruct
    (value : Option (alloc.sync.Arc zetesis_cpu.cancellation.DeadlineOwner)) :
    deadlineOption (M := Result) core.sync.atomic.AtomicBoolAlign1U8.load value =
      core.option.Option.is_some_and
        zetesis_cpu.cancellation.Cancellation.poll.closure.Insts.CoreOpsFunctionFnOnceTupleSharedArcDeadlineOwnerBool
        value () := by
  cases value <;> rfl

/-- The active read and actual option callback recover the generated poll.
No assumption about a poll's result is used. -/
theorem poll_reconstruct (control : zetesis_cpu.cancellation.Cancellation) :
    pollContext (M := Result) core.sync.atomic.AtomicBoolAlign1U8.load
      zetesis_cpu.cancellation.slot.Membership.is_cancelled
      (fun value => core.option.Option.is_some_and
        zetesis_cpu.cancellation.Cancellation.poll.closure.Insts.CoreOpsFunctionFnOnceTupleSharedArcDeadlineOwnerBool
        value ()) control = zetesis_cpu.cancellation.Cancellation.poll control := by
  cases cancelled : control.cancelled.value.nextRead <;>
    cases configured : control.slot <;>
    simp [pollContext, zetesis_cpu.cancellation.Cancellation.poll,
      zetesis_cpu.cancellation.slot.Membership.is_cancelled,
      alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
      core.sync.atomic.AtomicBoolAlign1U8.load, core.sync.atomic.AtomicU64Align8U64.load,
      core.option.Option.as_ref, liftM, monadLift_self, cancelled, configured]
  all_goals split <;> simp_all

/-- Filling the single poll hole with the actual poll recovers the generated
tick without unfolding that poll. The work-limit comparison stays after it. -/
theorem tick_reconstruct (work : oracle.Work) :
    tickContext (M := Result) zetesis_cpu.cancellation.Cancellation.poll work =
      oracle.Work.tick work := by rfl

/-- Filling the single tick hole with the actual tick reconstructs the generated
evaluation body. Iterator advancement, short circuits and appends stay intact. -/
theorem evaluation_reconstruct (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool))
    (cursor : core.iter.adapters.enumerate.Enumerate (core.slice.iter.Iter theory.Node))
    (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics) :
    evaluationContext (M := Result) oracle.Work.tick candidate frozen cursor output
      limits control statistics = oracle.evaluate_loop.body candidate frozen cursor output
        limits control statistics := by
  unfold evaluationContext oracle.evaluate_loop.body
  congr 1
  funext advanced
  rcases advanced with ⟨item, nextCursor⟩
  cases item with
  | none => rfl
  | some item =>
      rcases item with ⟨index, node⟩
      dsimp only
      congr 1
      funext ticked
      rcases ticked with ⟨answer, work⟩
      dsimp only
      congr 1
      funext branch
      cases branch with
      | Break residual => rfl
      | Continue value =>
          cases node <;> simp [bind_assoc, uncurry]
          all_goals
            congr 1
            funext observed
            cases observed <;> simp

end RuntimeContexts
