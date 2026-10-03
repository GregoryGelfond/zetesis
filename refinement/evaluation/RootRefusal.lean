import RootContextProjection
import RuntimeLoopOrigin
import RuntimeTickOrigin

open Aeneas Aeneas.Std Result ControlFlow Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects
open RootContextProjection

/-!
# The reached tick behind a root-scan refusal

A source refusal cannot be invented by exhaustion or a truth read. The terminal
body must have reached its tick and returned that tick's exact reason and work.
The full loop law retains the actual continuation prefix, terminal state and
ordered event partition. It requires no coverage, clear token or semantic truth
premise; backend failure is not a completed source refusal.
-/
namespace RootRefusal

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

/-- A root continuation has reached a tick, so it consumes at least one read.
No assumption is made about stored control tokens or root truth. -/
theorem continuation_nonempty (values : Slice Bool) (cursor : Cursor) (work : oracle.Work)
    (events : List Event) (next : State)
    (run : Runs (CheckerContexts.rootBody (M := Computation) ContextEvents.tick values cursor work)
      events (.cont next)) : events ≠ [] := by
  by_cases inside : cursor.i < cursor.slice.val.length
  · have advanced := EvaluatorIteration.slice_next_present cursor inside
    rw [context_at_tick ContextEvents.tick values cursor _ _ work advanced] at run
    obtain ⟨first, last, ticked, history, tickRun, _⟩ := RuntimeRuns.bind_inv _ _ _ _ run
    have progress : first ≠ [] := RuntimeTickOrigin.tick_nonempty work ticked.2 first ticked.1 tickRun
    intro empty
    exact progress (List.append_eq_nil_iff.mp (history.symm.trans empty)).1
  · have advanced := EvaluatorIteration.slice_next_exhausted cursor (Nat.le_of_not_gt inside)
    have exhausted : CheckerContexts.rootBody (M := Computation) ContextEvents.tick values cursor work =
        ITree.ret (.done (.Ok none, work)) := by
      simp only [CheckerContexts.rootBody, advanced, ContextEvents.lift_result,
        RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]
    rw [exhausted] at run
    have impossible := (RuntimeRuns.returned_inv _ _ _ run).1
    cases impossible

/-- A terminal source refusal is exactly the reached tick's refusal. Its full
body history is the tick history: the preceding iterator and following source
Result conversion are embedded operations and consume no events. -/
theorem body_refused (values : Slice Bool) (cursor : Cursor) (work after : oracle.Work)
    (events : List Event) (reason : zetesis_cpu.cancellation.Stop)
    (run : Runs (CheckerContexts.rootBody (M := Computation) ContextEvents.tick values cursor work)
      events (.done (.Err reason, after))) :
    Runs (ContextEvents.tick work) events (.Err reason, after) := by
  have liftSelf {T : Type} (operation : Result T) : (liftM operation : Result T) = operation := rfl
  have takeReturn {A B : Type} (operation : Result A) (next : A → Result B)
      (result : B) (returned : (operation >>= next) = ok result) :
      ∃ value, operation = ok value ∧ next value = ok result := by
    cases actual : operation with
    | ret value => exact ⟨value, rfl, by simpa only [actual, bind_tc_ok] using returned⟩
    | vis effect continuation => simp only [actual, bind_tc_vis, vis_not_ok] at returned
    | div => simp only [actual, bind_tc_div, div_not_ok] at returned
  by_cases inside : cursor.i < cursor.slice.val.length
  · have advanced := EvaluatorIteration.slice_next_present cursor inside
    rw [context_at_tick ContextEvents.tick values cursor _ _ work advanced] at run
    obtain ⟨first, last, ticked, history, tickRun, continuation⟩ := RuntimeRuns.bind_inv _ _ _ _ run
    obtain ⟨computed, silent⟩ := (RuntimeRuns.embed_iff _ _ _).mp continuation
    have sameEvents : events = first := by simpa only [silent, List.append_nil] using history
    rcases ticked with ⟨answer, returned⟩
    cases answer with
    | Err stop =>
      simp only [CheckerContexts.rootBody, liftSelf, advanced, bind_tc_ok,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
        core.convert.FromSame.from] at computed
      have same := Result.ok_injective computed
      have sameResult : ((.Err stop, returned) : Outcome) = (.Err reason, after) := ControlFlow.done.inj same
      have sameReason : stop = reason := core.result.Result.Err.inj (congrArg (fun value : Outcome => value.1) sameResult)
      have sameWork : returned = after := congrArg (fun value : Outcome => value.2) sameResult
      subst stop
      subst returned
      simpa only [sameEvents] using tickRun
    | Ok accepted =>
      cases accepted
      simp only [CheckerContexts.rootBody, liftSelf, advanced, bind_tc_ok,
        core.result.Result.Insts.CoreOpsTry.branch] at computed
      obtain ⟨truth, _, finished⟩ := takeReturn _ _ _ computed
      cases truth <;> have impossible := Result.ok_injective finished <;> cases impossible
  · have advanced := EvaluatorIteration.slice_next_exhausted cursor (Nat.le_of_not_gt inside)
    have exhausted : CheckerContexts.rootBody (M := Computation) ContextEvents.tick values cursor work =
        ITree.ret (.done (.Ok none, work)) := by
      simp only [CheckerContexts.rootBody, advanced, ContextEvents.lift_result,
        RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]
    rw [exhausted] at run
    have impossible := (RuntimeRuns.returned_inv _ _ _ run).1
    cases impossible

/-- A refused scan reaches a specific terminal cursor and work record through
actual continuation calls. The remaining history is exactly that state's tick
returning the scan's reason and returned work; it is not an arbitrary witness. -/
theorem loop_refused (values : Slice Bool) (cursor : Cursor) (work after : oracle.Work)
    (events : List Event) (reason : zetesis_cpu.cancellation.Stop)
    (run : Runs (CheckerContexts.rootLoop (M := Computation) RuntimeEffects.loop
      (CheckerContexts.rootBody ContextEvents.tick) cursor values work) events (.Err reason, after)) :
    ∃ before last reached,
      events = before ++ last ∧
      RuntimeLoopOrigin.Prefix
        (fun state : State => CheckerContexts.rootBody ContextEvents.tick values state.1 state.2)
        (cursor, work) before reached ∧
      Runs (ContextEvents.tick reached.2) last (.Err reason, after) := by
  obtain ⟨before, last, reached, history, prefixRun, terminal⟩ :=
    RuntimeLoopOrigin.completed_origin
      (fun state : State => CheckerContexts.rootBody ContextEvents.tick values state.1 state.2)
      (fun state observations next step => continuation_nonempty values state.1 state.2 observations next step)
      (cursor, work) events (.Err reason, after) run
  exact ⟨before, last, reached, history, prefixRun,
    body_refused values reached.1 reached.2 after last reason terminal⟩

/-- The refused entry has the same reachable origin, starting at the real
stored root iterator produced by its pure setup. No earlier roots or work are
reconstructed from an unconstrained terminal state. -/
theorem refused (program : theory.Theory) (values : Slice Bool) (work after : oracle.Work)
    (events : List Event) (reason : zetesis_cpu.cancellation.Stop)
    (run : Runs (ReferenceEvents.failedRoot program values work) events (.Err reason, after)) :
    ∃ before last reached,
      events = before ++ last ∧
      RuntimeLoopOrigin.Prefix
        (fun state : State => CheckerContexts.rootBody ContextEvents.tick values state.1 state.2)
        (FixedRootScan.initialCursor program, work) before reached ∧
      Runs (ContextEvents.tick reached.2) last (.Err reason, after) := by
  have setup : ReferenceEvents.failedRoot program values work =
      CheckerContexts.rootLoop (M := Computation) RuntimeEffects.loop
        (CheckerContexts.rootBody ContextEvents.tick) (FixedRootScan.initialCursor program) values work := by
    simp only [ReferenceEvents.failedRoot, CheckerContexts.rootScan, theory.Theory.roots,
      alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
      SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter,
      FixedRootScan.initialCursor, ContextEvents.lift_result, RuntimeEffects.embed_ok,
      Bind.bind, itree_ret_bind, bind_ok]
  rw [setup] at run
  exact loop_refused values (FixedRootScan.initialCursor program) work after events reason run

end RootRefusal
