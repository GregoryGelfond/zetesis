import EvaluationContextProjection
import RuntimeLoopOrigin
import RuntimeTickOrigin
import RuntimeRefusal

open Aeneas Aeneas.Std Result ControlFlow Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# Reached tick refusals in eventful evaluation

A typed evaluator stop originates in its actual tick at an actually reached
loop state. The receipt preserves the preceding continuation history, the final
read history, partial output and work. Exhaustion performs no tick; checked node,
mask or append failures remain backend outcomes rather than typed stops. No
formula, storage, clear-token or semantic-prefix premise is used here.
-/
namespace EvaluationRefusal

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

/-- Continuation and typed refusal both require a node tick. Successful
exhaustion is the one returning transition that need not observe a control. -/
def requiresTick : Evaluation.Transition → Prop
  | .cont _ => True
  | .done (answer, _, _) => match answer with | .Ok _ => False | .Err _ => True

/-- Actual pure advancement, observed tick, and pure continuation for a body
that reached a node. The whole observation history belongs to that one tick. -/
structure TickPhase (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (cursor : Evaluation.Cursor) (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (events : List Event) (transition : Evaluation.Transition) where
  next : Evaluation.Cursor
  index : Usize
  node : theory.Node
  answer : core.result.Result Unit zetesis_cpu.cancellation.Stop
  work : oracle.Work
  advanced : core.iter.adapters.enumerate.IteratorEnumerate.next
    (core.iter.traits.iterator.IteratorSliceIter theory.Node) cursor = ok (some (index, node), next)
  ticked : Runs (ContextEvents.tick ⟨limits, control, statistics⟩) events (answer, work)
  continued : RuntimeContexts.evaluationContext (M := Result) (fun _ => ok (answer, work))
    candidate frozen cursor output limits control statistics = ok transition

/-- A returning transition that requires a tick supplies all three actual phase
calls. Invert the pure iterator first, rule out exhaustion, then separate the
event tick from its embedded suffix. Neither backend phase contributes events. -/
theorem body_phases (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (cursor : Evaluation.Cursor) (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (events : List Event) (transition : Evaluation.Transition)
    (required : requiresTick transition)
    (run : Runs (RuntimeContexts.evaluationContext (M := Computation) ContextEvents.tick
      candidate frozen cursor output limits control statistics) events transition) :
    Nonempty (TickPhase candidate frozen cursor output limits control statistics events transition) := by
  have initial := run
  unfold RuntimeContexts.evaluationContext at initial
  obtain ⟨_, _, advancedValue, _, iteratorRun, _⟩ := RuntimeRuns.bind_inv _ _ _ _ initial
  obtain ⟨advanced, _⟩ := (RuntimeRuns.embed_iff _ _ _).mp iteratorRun
  change core.iter.adapters.enumerate.IteratorEnumerate.next
    (core.iter.traits.iterator.IteratorSliceIter theory.Node) cursor = ok advancedValue at advanced
  rcases advancedValue with ⟨item, nextCursor⟩
  cases item with
  | none =>
    have exhausted : RuntimeContexts.evaluationContext (M := Computation) ContextEvents.tick
        candidate frozen cursor output limits control statistics =
        ITree.ret (.done (.Ok (), output, ⟨limits, control, statistics⟩)) := by
      simp only [RuntimeContexts.evaluationContext, advanced, ContextEvents.lift_result,
        embed_ok, Bind.bind, itree_ret_bind]
    rw [exhausted] at run
    have same := (RuntimeRuns.returned_inv _ _ _ run).1
    subst transition
    exact False.elim required
  | some item =>
    rcases item with ⟨index, node⟩
    rw [EvaluationContextProjection.context_at_tick ContextEvents.tick candidate frozen cursor
      nextCursor index node output limits control statistics advanced] at run
    obtain ⟨observations, suffix, ticked, partition, tickRun, continued⟩ :=
      RuntimeRuns.bind_inv _ _ _ _ run
    obtain ⟨computed, silent⟩ := (RuntimeRuns.embed_iff _ _ _).mp continued
    have sameEvents : events = observations := by simpa only [silent, List.append_nil] using partition
    rcases ticked with ⟨answer, work⟩
    exact ⟨⟨nextCursor, index, node, answer, work, advanced, sameEvents ▸ tickRun, computed⟩⟩

/-- Every observed continuation consumes its reached tick's control read,
independently of stored control bits, formula validity or available budget. -/
theorem body_nonempty (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (cursor : Evaluation.Cursor) (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (events : List Event)
    (next : Evaluation.Cursor × alloc.vec.Vec Bool × oracle.Limits ×
      zetesis_cpu.cancellation.Cancellation × oracle.Statistics)
    (run : Runs (RuntimeContexts.evaluationContext (M := Computation) ContextEvents.tick
      candidate frozen cursor output limits control statistics) events (.cont next)) : events ≠ [] := by
  obtain ⟨phase⟩ := body_phases candidate frozen cursor output limits control statistics
    events (.cont next) True.intro run
  exact RuntimeTickOrigin.tick_nonempty ⟨limits, control, statistics⟩ phase.work events phase.answer phase.ticked

/-- A refused evaluator body returns its unchanged input output and the exact
refused tick. A successful tick cannot produce a typed stop in the subsequent
pure node operations, although a backend operation there may fail to return. -/
theorem body_refused (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (cursor : Evaluation.Cursor) (input output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (events : List Event) (after : oracle.Work) (reason : zetesis_cpu.cancellation.Stop)
    (run : Runs (RuntimeContexts.evaluationContext (M := Computation) ContextEvents.tick
      candidate frozen cursor input limits control statistics)
      events (.done (.Err reason, output, after))) :
    output = input ∧ Runs (ContextEvents.tick ⟨limits, control, statistics⟩) events (.Err reason, after) := by
  have takeReturn {A B : Type} (operation : Result A) (following : A → Result B)
      (result : B) (completed : (operation >>= following) = ok result) :
      ∃ value, operation = ok value ∧ following value = ok result := by
    cases actual : operation with
    | ret value => exact ⟨value, rfl, by simpa only [actual, bind_tc_ok] using completed⟩
    | vis effect continuation => simp only [actual, bind_tc_vis, vis_not_ok] at completed
    | div => simp only [actual, bind_tc_div, div_not_ok] at completed
  have liftSelf {T : Type} (operation : Result T) : (liftM operation : Result T) = operation := rfl
  obtain ⟨phase⟩ := body_phases candidate frozen cursor input limits control statistics
    events (.done (.Err reason, output, after)) True.intro run
  have computed := phase.continued
  cases answer : phase.answer with
  | Err stopped =>
    have exactSuffix : RuntimeContexts.evaluationContext (M := Result)
        (fun _ => ok (.Err stopped, phase.work)) candidate frozen cursor input limits control statistics =
        ok (.done (.Err stopped, input, phase.work)) := by
      simp only [RuntimeContexts.evaluationContext, liftSelf, phase.advanced, bind_tc_ok,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
        core.convert.FromSame.from]
    rw [answer] at computed
    have same := ControlFlow.done.inj (Result.ok_injective (exactSuffix.symm.trans computed))
    have stopSame : stopped = reason := core.result.Result.Err.inj (congrArg Prod.fst same)
    have outputSame : input = output := congrArg (fun result => result.2.1) same
    have workSame : phase.work = after := congrArg (fun result => result.2.2) same
    exact ⟨outputSame.symm, by simpa only [answer, stopSame, workSame] using phase.ticked⟩
  | Ok accepted =>
    cases accepted
    rw [answer] at computed
    simp only [RuntimeContexts.evaluationContext, liftSelf, phase.advanced, bind_tc_ok,
      core.result.Result.Insts.CoreOpsTry.branch] at computed
    cases phase.node
    all_goals
      repeat' first
      | obtain ⟨value, _, computed⟩ := takeReturn _ _ _ computed
      | split at computed
    all_goals
      have impossible := Result.ok_injective computed
      contradiction

/-- The five fields carried by the generated evaluator loop, without an added
semantic invariant or replacement runtime representation. -/
abbrev State := Evaluation.Cursor × alloc.vec.Vec Bool × oracle.Limits ×
  zetesis_cpu.cancellation.Cancellation × oracle.Statistics

/-- A refused eventful loop reaches an actual refused tick after an actual
continuation prefix. The terminal body retains the truth workspace at that
point; its tick retains the supplied stop and work. No earlier stop can have
been skipped, because every prefix constructor is a returned continuation. -/
theorem loop_refused (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (cursor : Evaluation.Cursor) (input output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (events : List Event) (after : oracle.Work) (reason : zetesis_cpu.cancellation.Stop)
    (run : Runs (CheckerContexts.evaluationLoop (M := Computation) RuntimeEffects.loop
      (RuntimeContexts.evaluationContext ContextEvents.tick) cursor candidate frozen
      input limits control statistics) events (.Err reason, output, after)) :
    ∃ before last reached,
      events = before ++ last ∧
      RuntimeLoopOrigin.Prefix
        (fun state : State => RuntimeContexts.evaluationContext ContextEvents.tick candidate frozen
          state.1 state.2.1 state.2.2.1 state.2.2.2.1 state.2.2.2.2)
        (cursor, input, limits, control, statistics) before reached ∧
      output = reached.2.1 ∧
      Runs (ContextEvents.tick ⟨reached.2.2.1, reached.2.2.2.1, reached.2.2.2.2⟩)
        last (.Err reason, after) := by
  let body := fun state : State =>
    RuntimeContexts.evaluationContext (M := Computation) ContextEvents.tick candidate frozen
      state.1 state.2.1 state.2.2.1 state.2.2.2.1 state.2.2.2.2
  have progress : ∀ state observations next, Runs (body state) observations (.cont next) →
      observations ≠ [] := by
    rintro ⟨position, truths, allowance, cancellation, counters⟩ observations next executing
    exact body_nonempty candidate frozen position truths allowance cancellation counters observations next executing
  obtain ⟨before, last, reached, history, prefixRun, terminal⟩ :=
    RuntimeLoopOrigin.completed_origin body progress
      (cursor, input, limits, control, statistics) events (.Err reason, output, after) run
  obtain ⟨unchanged, ticked⟩ := body_refused candidate frozen reached.1 reached.2.1 output
    reached.2.2.1 reached.2.2.2.1 reached.2.2.2.2 last after reason terminal
  exact ⟨before, last, reached, history, prefixRun, unchanged, ticked⟩

/-- A refused evaluator entry computes its empty initial workspace and actual
node cursor, then supplies the reached tick and exact prefix from `loop_refused`.
Its partial output is retained; a source stop is not a completed truth table. -/
theorem evaluate_refused (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (input output : alloc.vec.Vec Bool) (work after : oracle.Work)
    (events : List Event) (reason : zetesis_cpu.cancellation.Stop)
    (run : Runs (ReferenceEvents.evaluate program candidate frozen input work)
      events (.Err reason, output, after)) :
    ∃ before last reached,
      events = before ++ last ∧
      RuntimeLoopOrigin.Prefix
        (fun state : State => RuntimeContexts.evaluationContext ContextEvents.tick candidate frozen
          state.1 state.2.1 state.2.2.1 state.2.2.2.1 state.2.2.2.2)
        (EvaluatorSetup.initialCursor program, alloc.vec.Vec.new Bool,
          work.limits, work.cancellation, work.statistics) before reached ∧
      output = reached.2.1 ∧
      Runs (ContextEvents.tick ⟨reached.2.2.1, reached.2.2.2.1, reached.2.2.2.2⟩)
        last (.Err reason, after) := by
  have cleared : alloc.vec.Vec.clear Global input = ok (alloc.vec.Vec.new Bool) := rfl
  have nodesRead : theory.Theory.nodes program =
      ok (alloc.vec.Vec.deref program.value.nodes) := by
    simp only [theory.Theory.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, bind_tc_ok]
  have sliceStarted : core.slice.Slice.iter (alloc.vec.Vec.deref program.value.nodes) =
      ok { slice := alloc.vec.Vec.deref program.value.nodes, i := 0 } := rfl
  have enumerated : core.iter.traits.iterator.Iterator.enumerate.trait_default
      (core.iter.traits.iterator.IteratorSliceIter theory.Node)
      { slice := alloc.vec.Vec.deref program.value.nodes, i := 0 } =
      ok (EvaluatorSetup.initialCursor program) := rfl
  have setup : ReferenceEvents.evaluate program candidate frozen input work =
      CheckerContexts.evaluationLoop (M := Computation) RuntimeEffects.loop
        (RuntimeContexts.evaluationContext ContextEvents.tick) (EvaluatorSetup.initialCursor program)
        candidate frozen (alloc.vec.Vec.new Bool) work.limits work.cancellation work.statistics := by
    simp only [ReferenceEvents.evaluate, CheckerContexts.evaluate, cleared, nodesRead,
      sliceStarted, enumerated, ContextEvents.lift_result, RuntimeEffects.embed_ok,
      Bind.bind, itree_ret_bind]
  rw [setup] at run
  exact loop_refused candidate frozen (EvaluatorSetup.initialCursor program) (alloc.vec.Vec.new Bool)
    output work.limits work.cancellation work.statistics events after reason run

end EvaluationRefusal
