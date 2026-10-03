import ContextEvents
import ReferenceEvents
import Setup
import RuntimeLoop
import TickProjection

open Aeneas Aeneas.Std Result ControlFlow Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# Projecting completed eventful evaluation

The named event contexts retain the generated evaluator's operations and order,
while returning observations supply each reached control read. A successful
finite run of those contexts projects to the actual generated evaluator under a
clear stored control value. The complete output and work record are identical;
the original control record is retained. Pure backend failures and divergence
are not replaced by invented successful values.

No formula, storage or prefix invariant is needed for this projection. Existing
semantic refinements can be applied to the recovered actual evaluator equation
with their own stated premises. Typed stops are not successful completion, and
this module makes no projection claim for them. Relating physical Rust histories
to the source-checked event contexts remains a separate correspondence boundary.
-/
namespace EvaluationContextProjection

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

/-- Filling the checked evaluation context with an embedded backend tick merely
embeds its complete backend computation. In particular checked reads, appends,
failure and divergence are preserved rather than replaced by successful values. -/
theorem embed_context
    (tick : oracle.Work → Result ((core.result.Result Unit zetesis_cpu.cancellation.Stop) × oracle.Work))
    (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (cursor : Evaluation.Cursor) (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics) :
    RuntimeContexts.evaluationContext (M := Computation) (fun work => embed (tick work))
      candidate frozen cursor output limits control statistics =
      embed (RuntimeContexts.evaluationContext (M := Result) tick
        candidate frozen cursor output limits control statistics) := by
  have liftSelf {T : Type} (operation : Result T) :
      (liftM operation : Result T) = operation := rfl
  have embedThen {T U : Type} (operation : Result T) (next : T → Result U) :
      embed (do let value ← operation; next value) =
        (do let value ← embed operation; embed (next value)) :=
    RuntimeEffects.embed_bind operation next
  unfold RuntimeContexts.evaluationContext
  simp only [liftSelf, ContextEvents.lift_result, embedThen]
  congr 1
  funext advanced
  rcases advanced with ⟨item, nextCursor⟩
  cases item with
  | none => rfl
  | some item =>
      rcases item with ⟨index, node⟩
      dsimp only
      rw [embedThen]
      congr 1
      funext ticked
      rcases ticked with ⟨answer, work⟩
      dsimp only
      rw [embedThen]
      congr 1
      funext branch
      cases branch with
      | Break residual => simp only [embedThen]
      | Continue value =>
          cases node <;> simp only [embedThen, apply_ite]

/-- After the actual iterator returns a node, the checked context performs its
single effectful tick and otherwise embeds the unchanged backend continuation.
Repeating the known pure iterator equation in that continuation introduces no
observation and assumes no node-read or append success. -/
theorem context_at_tick
    (tick : oracle.Work → Computation ((core.result.Result Unit zetesis_cpu.cancellation.Stop) × oracle.Work))
    (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (cursor nextCursor : Evaluation.Cursor) (index : Usize) (node : theory.Node)
    (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (advanced : core.iter.adapters.enumerate.IteratorEnumerate.next
      (core.iter.traits.iterator.IteratorSliceIter theory.Node) cursor =
      ok (some (index, node), nextCursor)) :
    RuntimeContexts.evaluationContext (M := Computation) tick
      candidate frozen cursor output limits control statistics =
      (do let ticked ← tick ⟨limits, control, statistics⟩
          embed (RuntimeContexts.evaluationContext (M := Result) (fun _ => ok ticked)
            candidate frozen cursor output limits control statistics)) := by
  simp only [← embed_context]
  simp only [RuntimeContexts.evaluationContext, advanced, ContextEvents.lift_result,
    RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]

/-- The control record carried by either actual transition constructor. -/
def returnedControl : Evaluation.Transition → zetesis_cpu.cancellation.Cancellation
  | .cont (_, _, _, control, _) => control
  | .done (_, _, work) => work.cancellation

/-- The pure node continuation returns the tick's control record unchanged.
All node, mask and append operations are recovered from their actual successful
backend binds; this statement requires no assumed truth or storage invariant. -/
theorem context_control
    (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (cursor nextCursor : Evaluation.Cursor) (index : Usize) (node : theory.Node)
    (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (work : oracle.Work) (transition : Evaluation.Transition)
    (advanced : core.iter.adapters.enumerate.IteratorEnumerate.next
      (core.iter.traits.iterator.IteratorSliceIter theory.Node) cursor =
      ok (some (index, node), nextCursor))
    (computed : RuntimeContexts.evaluationContext (M := Result)
      (fun _ => ok (.Ok (), work)) candidate frozen cursor output limits control statistics =
      ok transition) :
    returnedControl transition = work.cancellation := by
  have takeReturn {A B : Type} (operation : Result A) (following : A → Result B)
      (result : B) (completed : (operation >>= following) = ok result) :
      ∃ value, operation = ok value ∧ following value = ok result := by
    cases actual : operation with
    | ret value => exact ⟨value, rfl, by simpa only [actual, bind_tc_ok] using completed⟩
    | vis effect continuation => simp only [actual, bind_tc_vis, vis_not_ok] at completed
    | div => simp only [actual, bind_tc_div, div_not_ok] at completed
  have liftSelf {T : Type} (operation : Result T) :
      (liftM operation : Result T) = operation := rfl
  simp only [RuntimeContexts.evaluationContext, liftSelf, advanced, bind_tc_ok,
    core.result.Result.Insts.CoreOpsTry.branch] at computed
  cases node
  all_goals
    repeat' first
    | obtain ⟨value, _, computed⟩ := takeReturn _ _ _ computed
    | split at computed
  all_goals
    have same := Result.ok_injective computed
    subst transition
    rfl

/-- A continuation or a successfully exhausted invocation. A typed stop is
excluded explicitly, rather than treated as a completed node visit. -/
def Successful : Evaluation.Transition → Prop
  | .cont _ => True
  | .done (answer, _, _) => answer = .Ok ()

/-- A successful finite run of the checked event context projects to the actual
generated body under the unchanged clear control record. Every continuation
consumes a read; this fact later bounds induction over the remaining events.

Proof: exhaustion is a pure iterator return. Otherwise separate the actual tick
run from the embedded backend continuation. Typed tick refusal cannot yield a
successful branch. A successful tick projects to the actual tick, and embedding
inversion supplies the actual backend continuation, including all checked reads. -/
theorem body_completed (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (cursor : Evaluation.Cursor)
    (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (events : List Event) (transition : Evaluation.Transition)
    (clear : EvaluatorControl.observation control = none)
    (successful : Successful transition)
    (run : Runs (RuntimeContexts.evaluationContext (M := Computation) ContextEvents.tick
      candidate frozen cursor output limits control statistics) events transition) :
    oracle.evaluate_loop.body candidate frozen cursor output limits control statistics =
      ok transition ∧ returnedControl transition = control ∧
      (match transition with | .cont _ => events ≠ [] | .done _ => True) := by
  have liftSelf {T : Type} (operation : Result T) :
      (liftM operation : Result T) = operation := rfl
  have initial := run
  unfold RuntimeContexts.evaluationContext at initial
  obtain ⟨initialEvents, remainingEvents, advancedValue, _, iteratorRun, _⟩ :=
    RuntimeRuns.bind_inv _ _ _ _ initial
  obtain ⟨advanced, _⟩ := (RuntimeRuns.embed_iff _ _ _).mp iteratorRun
  change core.iter.adapters.enumerate.IteratorEnumerate.next
    (core.iter.traits.iterator.IteratorSliceIter theory.Node) cursor = ok advancedValue at advanced
  rcases advancedValue with ⟨item, nextCursor⟩
  cases item with
  | some item =>
    rcases item with ⟨index, node⟩
    rw [context_at_tick ContextEvents.tick candidate frozen cursor nextCursor index node output limits control statistics advanced] at run
    obtain ⟨before, after, ticked, partition, tickRun, continuation⟩ :=
      RuntimeRuns.bind_inv _ _ events transition run
    obtain ⟨computed, vacant⟩ := (RuntimeRuns.embed_iff _ _ _).mp continuation
    rcases ticked with ⟨answer, work⟩
    cases answer with
    | Err reason =>
        have refused : RuntimeContexts.evaluationContext (M := Result)
            (fun _ => ok (.Err reason, work)) candidate frozen cursor output limits control statistics =
            ok (.done (.Err reason, output, work)) := by
          simp only [RuntimeContexts.evaluationContext, liftSelf, advanced, bind_tc_ok,
            core.result.Result.Insts.CoreOpsTry.branch,
            core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
            core.convert.FromSame.from]
        have same : .done (.Err reason, output, work) = transition :=
          Result.ok_injective (refused.symm.trans computed)
        rw [← same] at successful
        cases successful
    | Ok accepted =>
        cases accepted
        obtain ⟨ticked, observed⟩ := TickProjection.completed_tick
          ⟨limits, control, statistics⟩ work before clear tickRun
        have sameContext : RuntimeContexts.evaluationContext (M := Result)
            (fun _ => ok (.Ok (), work)) candidate frozen cursor output limits control statistics =
            oracle.evaluate_loop.body candidate frozen cursor output limits control statistics := by
          rw [← RuntimeContexts.evaluation_reconstruct]
          simp only [RuntimeContexts.evaluationContext, liftSelf, advanced, bind_tc_ok, ticked]
        have actual : oracle.evaluate_loop.body candidate frozen cursor output limits control statistics =
            ok transition := sameContext.symm.trans computed
        have nonempty : events ≠ [] := by
          rw [partition, vacant, List.append_nil]
          exact observed
        have tickControl : work.cancellation = control :=
          (TickProjection.completed_receipt ⟨limits, control, statistics⟩ work before clear tickRun).2.2.2.1
        have frame : returnedControl transition = control :=
          (context_control candidate frozen cursor nextCursor index node output limits control
            statistics work transition advanced computed).trans tickControl
        refine ⟨actual, frame, ?_⟩
        cases transition with
        | cont next => exact nonempty
        | done outcome => trivial
  | none =>
    have contextExact : RuntimeContexts.evaluationContext (M := Computation) ContextEvents.tick
        candidate frozen cursor output limits control statistics =
        ITree.ret (.done (.Ok (), output, ⟨limits, control, statistics⟩)) := by
      simp only [RuntimeContexts.evaluationContext, advanced, ContextEvents.lift_result,
        RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]
    rw [contextExact] at run
    obtain ⟨same, _⟩ := RuntimeRuns.returned_inv _ _ _ run
    have actual : oracle.evaluate_loop.body candidate frozen cursor output limits control statistics =
        ok (.done (.Ok (), output, ⟨limits, control, statistics⟩)) := by
      simp only [oracle.evaluate_loop.body, advanced, bind_tc_ok, uncurry]
    refine ⟨?_, ?_, ?_⟩
    · simpa only [same] using actual
    · subst transition
      rfl
    · subst transition
      trivial

/-- Every completed eventful evaluation loop agrees with the actual generated
loop under the same clear stored control, and returns that control unchanged.
No formula, cursor, packed-storage or mask-validity premise is needed: success
of the event run supplies every checked backend return used by the projection.

Proof: a continuation projects to an actual call, retains the control and
consumes a read. A successful exit projects and retains the same control.
Apply the generic finite-history loop laws to these local obligations. -/
theorem completed_loop (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (cursor : Evaluation.Cursor)
    (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (events : List Event) (values : alloc.vec.Vec Bool) (work : oracle.Work)
    (clear : EvaluatorControl.observation control = none)
    (run : Runs (CheckerContexts.evaluationLoop (M := Computation) RuntimeEffects.loop
      (RuntimeContexts.evaluationContext ContextEvents.tick) cursor candidate frozen
      output limits control statistics) events (.Ok (), values, work)) :
    oracle.evaluate_loop cursor candidate frozen output limits control statistics =
        ok (.Ok (), values, work) ∧ work.cancellation = control := by
  let frameInvariant :
      Evaluation.Cursor × alloc.vec.Vec Bool × oracle.Limits ×
        zetesis_cpu.cancellation.Cancellation × oracle.Statistics → Prop :=
    fun (_, _, _, cancellation, _) => cancellation = control
  let body := fun (position, truths, allowance, cancellation, counters) =>
    RuntimeContexts.evaluationContext (M := Computation) ContextEvents.tick
      candidate frozen position truths allowance cancellation counters
  let fixed := fun (position, truths, allowance, cancellation, counters) =>
    oracle.evaluate_loop.body candidate frozen position truths allowance cancellation counters
  let accepted : (core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
      alloc.vec.Vec Bool × oracle.Work → Prop := fun (answer, _, _) => answer = .Ok ()
  let postcondition : (core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
      alloc.vec.Vec Bool × oracle.Work → Prop := fun (_, _, returned) => returned.cancellation = control
  have continues : ∀ before observations after, frameInvariant before →
      Runs (body before) observations (.cont after) →
      fixed before = ok (.cont after) ∧ frameInvariant after ∧ observations ≠ [] := by
    rintro ⟨position, truths, allowance, cancellation, counters⟩ observations
      ⟨nextPosition, nextTruths, nextAllowance, nextCancellation, nextCounters⟩ retained executing
    have observed : EvaluatorControl.observation cancellation = none := retained ▸ clear
    obtain ⟨actual, sameControl, consumed⟩ := body_completed candidate frozen position truths
      allowance cancellation counters observations
      (.cont (nextPosition, nextTruths, nextAllowance, nextCancellation, nextCounters))
      observed True.intro executing
    exact ⟨actual, sameControl.trans retained, consumed⟩
  have finishes : ∀ before observations outcome, frameInvariant before → accepted outcome →
      Runs (body before) observations (.done outcome) →
      fixed before = ok (.done outcome) ∧ postcondition outcome := by
    rintro ⟨position, truths, allowance, cancellation, counters⟩ observations
      ⟨answer, finalValues, finalWork⟩ retained success executing
    have observed : EvaluatorControl.observation cancellation = none := retained ▸ clear
    obtain ⟨actual, sameControl, _⟩ := body_completed candidate frozen position truths
      allowance cancellation counters observations (.done (answer, finalValues, finalWork))
      observed success executing
    exact ⟨actual, sameControl.trans retained⟩
  have executed := RuntimeLoop.completed_loop body fixed frameInvariant accepted continues
    (fun before observations outcome retained success executing =>
      (finishes before observations outcome retained success executing).1)
    (cursor, output, limits, control, statistics) events (.Ok (), values, work) rfl rfl run
  have frame := RuntimeLoop.completed_post body frameInvariant accepted postcondition
    (fun before observations after retained executing =>
      (continues before observations after retained executing).2)
    (fun before observations outcome retained success executing =>
      (finishes before observations outcome retained success executing).2)
    (cursor, output, limits, control, statistics) events (.Ok (), values, work) rfl rfl run
  exact ⟨executed, frame⟩

/-- Successful eventful evaluation projects to the actual generated evaluator,
with identical output, limits, statistics and control record. Its initial clear,
node-slice read and cursor setup are computed from the original library calls.
No output correctness or successful inner call is a premise. -/
theorem completed_evaluate (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (events : List Event) (values : alloc.vec.Vec Bool) (after : oracle.Work)
    (clear : EvaluatorControl.observation before.cancellation = none)
    (run : Runs (ReferenceEvents.evaluate program candidate frozen old before)
      events (.Ok (), values, after)) :
    oracle.evaluate program candidate frozen old before = ok (.Ok (), values, after) ∧
      after.cancellation = before.cancellation := by
  have cleared : alloc.vec.Vec.clear Global old = ok (alloc.vec.Vec.new Bool) := rfl
  have nodesRead : theory.Theory.nodes program =
      ok (alloc.vec.Vec.deref program.value.nodes) := by
    simp only [theory.Theory.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, bind_tc_ok]
  have sliceStarted : core.slice.Slice.iter (alloc.vec.Vec.deref program.value.nodes) =
      ok { slice := alloc.vec.Vec.deref program.value.nodes, i := 0 } := rfl
  have enumerated : core.iter.traits.iterator.Iterator.enumerate.trait_default
      (core.iter.traits.iterator.IteratorSliceIter theory.Node)
      { slice := alloc.vec.Vec.deref program.value.nodes, i := 0 } =
      ok (EvaluatorSetup.initialCursor program) := rfl
  have setup : ReferenceEvents.evaluate program candidate frozen old before =
      CheckerContexts.evaluationLoop (M := Computation) RuntimeEffects.loop
        (RuntimeContexts.evaluationContext ContextEvents.tick) (EvaluatorSetup.initialCursor program)
        candidate frozen (alloc.vec.Vec.new Bool) before.limits before.cancellation before.statistics := by
    simp only [ReferenceEvents.evaluate, CheckerContexts.evaluate, cleared, nodesRead,
      sliceStarted, enumerated, ContextEvents.lift_result, RuntimeEffects.embed_ok,
      Bind.bind, itree_ret_bind]
  rw [setup] at run
  obtain ⟨executed, retained⟩ := completed_loop candidate frozen (EvaluatorSetup.initialCursor program)
    (alloc.vec.Vec.new Bool) before.limits before.cancellation before.statistics
    events values after clear run
  exact ⟨(EvaluatorSetup.evaluate_from_empty program candidate frozen old before).trans executed,
    retained⟩

end EvaluationContextProjection
