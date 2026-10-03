import SelectionContextProjection
import RuntimeLoopOrigin
import RuntimeTickOrigin

open Aeneas Aeneas.Std Result ControlFlow Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects
open SelectionContextProjection

/-!
# The reached tick behind a selection refusal

A source refusal cannot be invented by exhaustion or a membership read. The terminal
body must have reached its tick and returned that tick's exact reason and work.
The full loop law retains the actual continuation prefix, terminal state and
ordered event partition. It requires no coverage, clear token or semantic truth
premise; backend failure is not a completed source refusal.
-/
namespace SelectionRefusal

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

/-- A selection continuation has reached a tick, so it consumes at least one read.
No assumption is made about stored control tokens or candidate membership. -/
theorem continuation_nonempty (candidate : theory.Interpretation) (cursor : Cursor) (selected : alloc.vec.Vec Usize) (work : oracle.Work)
    (events : List Event) (next : State)
    (run : Runs (CheckerContexts.selectionBody (M := Computation) ContextEvents.tick candidate cursor selected work)
      events (.cont next)) : events ≠ [] := by
  have first := run
  unfold CheckerContexts.selectionBody at first
  obtain ⟨advancedValue, advanced, _⟩ := RuntimeRuns.embedded_bind_inv
    (core.iter.range.IteratorRange.next core.iter.range.StepUsize cursor) _ _ _ first
  rcases advancedValue with ⟨item, nextCursor⟩
  cases item with
  | some atom =>
    rw [context_at_tick ContextEvents.tick candidate cursor nextCursor atom selected work advanced] at run
    obtain ⟨first, last, ticked, history, tickRun, _⟩ := RuntimeRuns.bind_inv _ _ _ _ run
    have progress : first ≠ [] := RuntimeTickOrigin.tick_nonempty work ticked.2 first ticked.1 tickRun
    intro empty
    exact progress (List.append_eq_nil_iff.mp (history.symm.trans empty)).1
  | none =>
    have exhausted : CheckerContexts.selectionBody (M := Computation) ContextEvents.tick candidate cursor selected work =
        ITree.ret (.done (.Ok (), selected, work)) := by
      simp only [CheckerContexts.selectionBody, advanced, ContextEvents.lift_result,
        RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]
    rw [exhausted] at run
    have impossible := (RuntimeRuns.returned_inv _ _ _ run).1
    cases impossible

/-- A terminal source refusal is exactly the reached tick's refusal. Its full
body history is the tick history: the preceding iterator and following source
Result conversion are embedded operations and consume no events. -/
theorem body_refused (candidate : theory.Interpretation) (cursor : Cursor)
    (selected output : alloc.vec.Vec Usize) (work after : oracle.Work)
    (events : List Event) (reason : zetesis_cpu.cancellation.Stop)
    (run : Runs (CheckerContexts.selectionBody (M := Computation) ContextEvents.tick candidate cursor selected work)
      events (.done (.Err reason, output, after))) :
    selected = output ∧ Runs (ContextEvents.tick work) events (.Err reason, after) := by
  have liftSelf {T : Type} (operation : Result T) : (liftM operation : Result T) = operation := rfl
  have takeReturn {A B : Type} (operation : Result A) (next : A → Result B)
      (result : B) (returned : (operation >>= next) = ok result) :
      ∃ value, operation = ok value ∧ next value = ok result := by
    cases actual : operation with
    | ret value => exact ⟨value, rfl, by simpa only [actual, bind_tc_ok] using returned⟩
    | vis effect continuation => simp only [actual, bind_tc_vis, vis_not_ok] at returned
    | div => simp only [actual, bind_tc_div, div_not_ok] at returned
  have first := run
  unfold CheckerContexts.selectionBody at first
  obtain ⟨advancedValue, advanced, _⟩ := RuntimeRuns.embedded_bind_inv
    (core.iter.range.IteratorRange.next core.iter.range.StepUsize cursor) _ _ _ first
  rcases advancedValue with ⟨item, nextCursor⟩
  cases item with
  | some atom =>
    rw [context_at_tick ContextEvents.tick candidate cursor nextCursor atom selected work advanced] at run
    obtain ⟨first, last, ticked, history, tickRun, continuation⟩ := RuntimeRuns.bind_inv _ _ _ _ run
    obtain ⟨computed, silent⟩ := (RuntimeRuns.embed_iff _ _ _).mp continuation
    have sameEvents : events = first := by simpa only [silent, List.append_nil] using history
    rcases ticked with ⟨answer, returned⟩
    cases answer with
    | Err stop =>
      simp only [CheckerContexts.selectionBody, liftSelf, advanced, bind_tc_ok,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
        core.convert.FromSame.from] at computed
      have same := Result.ok_injective computed
      have sameResult : ((.Err stop, selected, returned) : Outcome) =
          (.Err reason, output, after) := ControlFlow.done.inj same
      have sameReason : stop = reason := core.result.Result.Err.inj (congrArg (fun value : Outcome => value.1) sameResult)
      have sameOutput : selected = output := congrArg (fun value : Outcome => value.2.1) sameResult
      have sameWork : returned = after := congrArg (fun value : Outcome => value.2.2) sameResult
      subst stop
      subst returned
      exact ⟨sameOutput, by simpa only [sameEvents] using tickRun⟩
    | Ok accepted =>
      cases accepted
      simp only [CheckerContexts.selectionBody, liftSelf, advanced, bind_tc_ok,
        core.result.Result.Insts.CoreOpsTry.branch] at computed
      obtain ⟨truth, _, finished⟩ := takeReturn _ _ _ computed
      cases truth with
      | false =>
        have impossible := Result.ok_injective finished
        cases impossible
      | true =>
        obtain ⟨appended, _, published⟩ := takeReturn _ _ _ finished
        have impossible := Result.ok_injective published
        cases impossible
  | none =>
    have exhausted : CheckerContexts.selectionBody (M := Computation) ContextEvents.tick candidate cursor selected work =
        ITree.ret (.done (.Ok (), selected, work)) := by
      simp only [CheckerContexts.selectionBody, advanced, ContextEvents.lift_result,
        RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]
    rw [exhausted] at run
    have impossible := (RuntimeRuns.returned_inv _ _ _ run).1
    cases impossible

/-- A refused selection retains an actually reached atom-range cursor, vector
and work record. The terminal tick returns the same reason and the source result
retains that reached vector, including any earlier successful appends. -/
theorem loop_refused (candidate : theory.Interpretation) (cursor : Cursor)
    (selected output : alloc.vec.Vec Usize) (work after : oracle.Work)
    (events : List Event) (reason : zetesis_cpu.cancellation.Stop)
    (run : Runs (CheckerContexts.selectionLoop (M := Computation) RuntimeEffects.loop
      (CheckerContexts.selectionBody ContextEvents.tick) cursor candidate selected work)
      events (.Err reason, output, after)) :
    ∃ before last reached,
      events = before ++ last ∧
      RuntimeLoopOrigin.Prefix
        (fun state : State => CheckerContexts.selectionBody ContextEvents.tick candidate state.1 state.2.1 state.2.2)
        (cursor, selected, work) before reached ∧ reached.2.1 = output ∧
      Runs (ContextEvents.tick reached.2.2) last (.Err reason, after) := by
  obtain ⟨before, last, reached, history, prefixRun, terminal⟩ :=
    RuntimeLoopOrigin.completed_origin
      (fun state : State => CheckerContexts.selectionBody ContextEvents.tick candidate state.1 state.2.1 state.2.2)
      (fun state observations next step => continuation_nonempty candidate state.1 state.2.1 state.2.2 observations next step)
      (cursor, selected, work) events (.Err reason, output, after) run
  obtain ⟨sameOutput, tickRun⟩ := body_refused candidate reached.1 reached.2.1 output reached.2.2 after last reason terminal
  exact ⟨before, last, reached, history, prefixRun, sameOutput, tickRun⟩

/-- A refused selection entry traces its returned vector and reason to the
actual range scan beginning at zero. The reached vector includes precisely the
mutations made along the retained continuation prefix. -/
theorem refused (program : theory.Theory) (candidate : theory.Interpretation)
    (selected output : alloc.vec.Vec Usize) (work after : oracle.Work)
    (events : List Event) (reason : zetesis_cpu.cancellation.Stop)
    (run : Runs (ReferenceEvents.selectAtoms program candidate selected work)
      events (.Err reason, output, after)) :
    ∃ before last reached,
      events = before ++ last ∧
      RuntimeLoopOrigin.Prefix
        (fun state : State => CheckerContexts.selectionBody ContextEvents.tick candidate state.1 state.2.1 state.2.2)
        (⟨0#usize, program.value.atoms⟩, selected, work) before reached ∧ reached.2.1 = output ∧
      Runs (ContextEvents.tick reached.2.2) last (.Err reason, after) := by
  let cursor : Cursor := ⟨0#usize, program.value.atoms⟩
  have setup : ReferenceEvents.selectAtoms program candidate selected work =
      CheckerContexts.selectionLoop (M := Computation) RuntimeEffects.loop
        (CheckerContexts.selectionBody ContextEvents.tick) cursor candidate selected work := by
    simp only [ReferenceEvents.selectAtoms, CheckerContexts.selectAtoms, theory.Theory.atom_count,
      alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, cursor,
      ContextEvents.lift_result, RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind, bind_ok]
  rw [setup] at run
  exact loop_refused candidate cursor selected output work after events reason run

end SelectionRefusal
