import CarryContextProjection
import RuntimeLoopOrigin
import RuntimeTickOrigin

open Aeneas Aeneas.Std Result ControlFlow Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects
open CarryContextProjection

/-!
# The reached tick behind a carry refusal

A source refusal cannot be invented by exhaustion or a packed update. The terminal
body must have reached its tick and returned that tick's exact reason and work.
The full loop law retains the actual continuation prefix, terminal state and
ordered event partition. It requires no coverage, clear token or semantic truth
premise; backend failure is not a completed source refusal.
-/
namespace CarryRefusal

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

/-- A carry continuation has reached a tick, so it consumes at least one read.
No assumption is made about stored control tokens or packed values. -/
theorem continuation_nonempty (cursor : Cursor) (subset : theory.Interpretation) (present : Usize) (work : oracle.Work)
    (events : List Event) (next : State)
    (run : Runs (CheckerContexts.carryBody (M := Computation) ContextEvents.tick cursor subset present work)
      events (.cont next)) : events ≠ [] := by
  by_cases inside : cursor.i < cursor.slice.val.length
  · have advanced := EvaluatorIteration.slice_next_present cursor inside
    rw [context_at_tick ContextEvents.tick cursor _ _ subset present work advanced] at run
    obtain ⟨first, last, ticked, history, tickRun, _⟩ := RuntimeRuns.bind_inv _ _ _ _ run
    have progress : first ≠ [] := RuntimeTickOrigin.tick_nonempty work ticked.2 first ticked.1 tickRun
    intro empty
    exact progress (List.append_eq_nil_iff.mp (history.symm.trans empty)).1
  · have advanced := EvaluatorIteration.slice_next_exhausted cursor (Nat.le_of_not_gt inside)
    have exhausted : CheckerContexts.carryBody (M := Computation) ContextEvents.tick cursor subset present work =
        ITree.ret (.done (.Ok (), subset, present, work)) := by
      simp only [CheckerContexts.carryBody, advanced, ContextEvents.lift_result,
        RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]
    rw [exhausted] at run
    have impossible := (RuntimeRuns.returned_inv _ _ _ run).1
    cases impossible

/-- A terminal source refusal is exactly the reached tick's refusal. Its full
body history is the tick history: the preceding iterator and following source
Result conversion are embedded operations and consume no events. -/
theorem body_refused (cursor : Cursor) (subset output : theory.Interpretation)
    (present count : Usize) (work after : oracle.Work)
    (events : List Event) (reason : zetesis_cpu.cancellation.Stop)
    (run : Runs (CheckerContexts.carryBody (M := Computation) ContextEvents.tick cursor subset present work)
      events (.done (.Err reason, output, count, after))) :
    subset = output ∧ present = count ∧ Runs (ContextEvents.tick work) events (.Err reason, after) := by
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
    rw [context_at_tick ContextEvents.tick cursor _ _ subset present work advanced] at run
    obtain ⟨first, last, ticked, history, tickRun, continuation⟩ := RuntimeRuns.bind_inv _ _ _ _ run
    obtain ⟨computed, silent⟩ := (RuntimeRuns.embed_iff _ _ _).mp continuation
    have sameEvents : events = first := by simpa only [silent, List.append_nil] using history
    rcases ticked with ⟨answer, returned⟩
    cases answer with
    | Err stop =>
      simp only [CheckerContexts.carryBody, liftSelf, advanced, bind_tc_ok,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
        core.convert.FromSame.from] at computed
      have same := Result.ok_injective computed
      have sameResult : ((.Err stop, subset, present, returned) : Outcome) =
          (.Err reason, output, count, after) := ControlFlow.done.inj same
      have sameReason : stop = reason := core.result.Result.Err.inj (congrArg (fun value : Outcome => value.1) sameResult)
      have sameOutput : subset = output := congrArg (fun value : Outcome => value.2.1) sameResult
      have sameCount : present = count := congrArg (fun value : Outcome => value.2.2.1) sameResult
      have sameWork : returned = after := congrArg (fun value : Outcome => value.2.2.2) sameResult
      subst stop
      subst returned
      exact ⟨sameOutput, sameCount, by simpa only [sameEvents] using tickRun⟩
    | Ok accepted =>
      cases accepted
      simp only [CheckerContexts.carryBody, liftSelf, advanced, bind_tc_ok,
        core.result.Result.Insts.CoreOpsTry.branch] at computed
      obtain ⟨index, _, indexed⟩ := takeReturn _ _ _ computed
      obtain ⟨⟨packed, replace⟩, _, borrowed⟩ := takeReturn _ _ _ indexed
      obtain ⟨offset, _, offsetRead⟩ := takeReturn _ _ _ borrowed
      obtain ⟨bit, _, shifted⟩ := takeReturn _ _ _ offsetRead
      simp only [lift, bind_tc_ok] at shifted
      split at shifted
      · obtain ⟨updatedCount, _, finished⟩ := takeReturn _ _ _ shifted
        have impossible := Result.ok_injective finished
        cases impossible
      · obtain ⟨updatedCount, _, finished⟩ := takeReturn _ _ _ shifted
        have impossible := Result.ok_injective finished
        cases impossible
  · have advanced := EvaluatorIteration.slice_next_exhausted cursor (Nat.le_of_not_gt inside)
    have exhausted : CheckerContexts.carryBody (M := Computation) ContextEvents.tick cursor subset present work =
        ITree.ret (.done (.Ok (), subset, present, work)) := by
      simp only [CheckerContexts.carryBody, advanced, ContextEvents.lift_result,
        RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]
    rw [exhausted] at run
    have impossible := (RuntimeRuns.returned_inv _ _ _ run).1
    cases impossible

/-- A refused carry retains an actually reached cursor, packed subset,
population and work record. Earlier clear steps are represented in the exact
prefix; the refusing iteration returns that reached subset and population
unchanged and propagates the terminal tick's reason. -/
theorem loop_refused (cursor : Cursor) (subset output : theory.Interpretation)
    (present count : Usize) (work after : oracle.Work)
    (events : List Event) (reason : zetesis_cpu.cancellation.Stop)
    (run : Runs (CheckerContexts.carryLoop (M := Computation) RuntimeEffects.loop
      (CheckerContexts.carryBody ContextEvents.tick) cursor subset present work)
      events (.Err reason, output, count, after)) :
    ∃ before last reached,
      events = before ++ last ∧
      RuntimeLoopOrigin.Prefix
        (fun state : State => CheckerContexts.carryBody ContextEvents.tick state.1 state.2.1 state.2.2.1 state.2.2.2)
        (cursor, subset, present, work) before reached ∧ reached.2.1 = output ∧ reached.2.2.1 = count ∧
      Runs (ContextEvents.tick reached.2.2.2) last (.Err reason, after) := by
  obtain ⟨before, last, reached, history, prefixRun, terminal⟩ :=
    RuntimeLoopOrigin.completed_origin
      (fun state : State => CheckerContexts.carryBody ContextEvents.tick state.1 state.2.1 state.2.2.1 state.2.2.2)
      (fun state observations next step => continuation_nonempty state.1 state.2.1 state.2.2.1 state.2.2.2 observations next step)
      (cursor, subset, present, work) events (.Err reason, output, count, after) run
  obtain ⟨sameOutput, sameCount, tickRun⟩ := body_refused reached.1 reached.2.1 output
    reached.2.2.1 count reached.2.2.2 after last reason terminal
  exact ⟨before, last, reached, history, prefixRun, sameOutput, sameCount, tickRun⟩

/-- A refused carry entry starts from the supplied selected-coordinate slice.
Its exact continuation prefix preserves reachability of the returned subset and
population; the final iteration propagates the reached tick's reason. -/
theorem refused (selected : Slice Usize) (subset output : theory.Interpretation)
    (present count : Usize) (work after : oracle.Work) (events : List Event)
    (reason : zetesis_cpu.cancellation.Stop)
    (run : Runs (ReferenceEvents.advanceSubset selected subset present work)
      events (.Err reason, output, count, after)) :
    ∃ before last reached,
      events = before ++ last ∧
      RuntimeLoopOrigin.Prefix
        (fun state : State => CheckerContexts.carryBody ContextEvents.tick state.1 state.2.1 state.2.2.1 state.2.2.2)
        (⟨selected, 0⟩, subset, present, work) before reached ∧ reached.2.1 = output ∧ reached.2.2.1 = count ∧
      Runs (ContextEvents.tick reached.2.2.2) last (.Err reason, after) := by
  let cursor : Cursor := ⟨selected, 0⟩
  have setup : ReferenceEvents.advanceSubset selected subset present work =
      CheckerContexts.carryLoop (M := Computation) RuntimeEffects.loop
        (CheckerContexts.carryBody ContextEvents.tick) cursor subset present work := by
    simp only [ReferenceEvents.advanceSubset, CheckerContexts.advanceSubset,
      SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter, cursor,
      ContextEvents.lift_result, RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]
  rw [setup] at run
  exact loop_refused cursor subset output present count work after events reason run

end CarryRefusal
