import CheckerContexts
import TickProjection
import RuntimeLoop
import SubsetSteps
import ReferenceEvents

open Aeneas Aeneas.Std Result ControlFlow Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# Completed subset carries with returning reads

The checked carry context replaces only its tick call. A completed event run
therefore has the same packed writes, population and result as the actual generated body under
clear fixed observations. This projection assumes no packed-storage or population invariants: a
successful finite run itself rules out backend read failure. It neither supplies
subset meaning to its words nor establishes that a runtime operation returns.
-/
namespace CarryContextProjection

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

abbrev Cursor := core.slice.iter.Iter Usize
abbrev State := Cursor × theory.Interpretation × Usize × oracle.Work
abbrev Outcome := core.result.Result Unit zetesis_cpu.cancellation.Stop × theory.Interpretation × Usize × oracle.Work
abbrev Transition := ControlFlow State Outcome

/-- Source success is a completed carry; source stops remain excluded. -/
def Successful : Transition → Prop
  | .cont _ => True
  | .done (answer, _, _, _) => answer = .Ok ()

/-- A continuation must consume a read; a completed exit may be pure. -/
def Progress (events : List Event) : Transition → Prop
  | .cont _ => events ≠ []
  | .done _ => True

/-- Work retained by either branch of a body invocation. -/
def returnedWork : Transition → oracle.Work
  | .cont (_, _, _, work) => work
  | .done (_, _, _, work) => work

/-- Embedding a backend tick embeds the entire unchanged carry context,
including checked iterator, packed reads and writes and any backend failure. -/
theorem embed_context
    (tick : oracle.Work → Result (core.result.Result Unit zetesis_cpu.cancellation.Stop × oracle.Work))
    (cursor : Cursor) (subset : theory.Interpretation) (present : Usize) (work : oracle.Work) :
    CheckerContexts.carryBody (M := Computation) (fun state => embed (tick state)) cursor subset present work =
      embed (CheckerContexts.carryBody (M := Result) tick cursor subset present work) := by
  have liftSelf {T : Type} (operation : Result T) : (liftM operation : Result T) = operation := rfl
  have embedThen {T U : Type} (operation : Result T) (next : T → Result U) :
      embed (do let value ← operation; next value) =
        (do let value ← embed operation; embed (next value)) := RuntimeEffects.embed_bind operation next
  unfold CheckerContexts.carryBody
  simp only [liftSelf, ContextEvents.lift_result, embedThen]
  congr 1
  funext advanced
  rcases advanced with ⟨item, nextCursor⟩
  cases item with
  | none => rfl
  | some root =>
    dsimp only
    rw [embedThen]
    congr 1
    funext ticked
    rcases ticked with ⟨answer, returned⟩
    dsimp only
    rw [embedThen]
    congr 1
    funext branch
    cases branch with
    | Break residual => simp only [embedThen]
    | Continue value =>
      simp only [embedThen, apply_ite]


/-- A present selected coordinate reaches exactly one tick; all remaining operations are an
embedding of the unchanged generated suffix. The supplied iterator equation is
an actual operation result, not an assumption of packed updates. -/
theorem context_at_tick
    (tick : oracle.Work → Computation (core.result.Result Unit zetesis_cpu.cancellation.Stop × oracle.Work))
    (cursor nextCursor : Cursor) (atom : Usize)
    (subset : theory.Interpretation) (present : Usize) (work : oracle.Work)
    (advanced : core.slice.iter.IteratorSliceIter.next cursor = ok (some atom, nextCursor)) :
    CheckerContexts.carryBody (M := Computation) tick cursor subset present work =
      (do let ticked ← tick work
          embed (CheckerContexts.carryBody (M := Result) (fun _ => ok ticked) cursor subset present work)) := by
  simp only [← embed_context]
  simp only [CheckerContexts.carryBody, advanced, ContextEvents.lift_result,
    RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]

/-- A successful finite carry-body run projects to the actual body, preserves
its control record and consumes a read on every continuation. Exhaustion has no
tick; otherwise a source stop cannot yield a successful transition. All later
backend operations are recovered from the successful embedded suffix. -/
theorem body_completed (cursor : Cursor) (subset : theory.Interpretation) (present : Usize) (work : oracle.Work)
    (events : List Event) (transition : Transition)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (successful : Successful transition)
    (run : Runs (CheckerContexts.carryBody (M := Computation) ContextEvents.tick cursor subset present work)
      events transition) :
    oracle.advance_subset_loop.body cursor subset present work = ok transition ∧
      (returnedWork transition).cancellation = work.cancellation ∧
      Progress events transition := by
  have liftSelf {T : Type} (operation : Result T) : (liftM operation : Result T) = operation := rfl
  have takeReturn {A B : Type} (operation : Result A) (next : A → Result B)
      (result : B) (returned : (operation >>= next) = ok result) :
      ∃ value, operation = ok value ∧ next value = ok result := by
    cases actual : operation with
    | ret value => exact ⟨value, rfl, by simpa only [actual, bind_tc_ok] using returned⟩
    | vis effect continuation => simp only [actual, bind_tc_vis, vis_not_ok] at returned
    | div => simp only [actual, bind_tc_div, div_not_ok] at returned
  have first := run
  unfold CheckerContexts.carryBody at first
  obtain ⟨iteratorEvents, tailEvents, advancedValue, _, iteratorRun, _⟩ :=
    RuntimeRuns.bind_inv _ _ _ _ first
  obtain ⟨advanced, _⟩ := (RuntimeRuns.embed_iff _ _ _).mp iteratorRun
  change core.slice.iter.IteratorSliceIter.next cursor = ok advancedValue at advanced
  rcases advancedValue with ⟨item, nextCursor⟩
  cases item with
  | some atom =>
    rw [context_at_tick ContextEvents.tick cursor nextCursor atom subset present work advanced] at run
    obtain ⟨before, after, ticked, partition, tickRun, continuation⟩ := RuntimeRuns.bind_inv _ _ _ _ run
    obtain ⟨computed, silent⟩ := (RuntimeRuns.embed_iff _ _ _).mp continuation
    rcases ticked with ⟨answer, returned⟩
    cases answer with
    | Err reason =>
      have refused : CheckerContexts.carryBody (M := Result) (fun _ => ok (.Err reason, returned))
          cursor subset present work = ok (.done (.Err reason, subset, present, returned)) := by
        simp only [CheckerContexts.carryBody, liftSelf, advanced, bind_tc_ok,
          core.result.Result.Insts.CoreOpsTry.branch,
          core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
          core.convert.FromSame.from]
      have same : .done (.Err reason, subset, present, returned) = transition :=
        Result.ok_injective (refused.symm.trans computed)
      rw [← same] at successful
      cases successful
    | Ok accepted =>
      cases accepted
      obtain ⟨ticked, observed⟩ := TickProjection.completed_tick work returned before clear tickRun
      have retained : returned.cancellation = work.cancellation :=
        (TickProjection.completed_receipt work returned before clear tickRun).2.2.2.1
      have sameContext : CheckerContexts.carryBody (M := Result) (fun _ => ok (.Ok (), returned))
          cursor subset present work = oracle.advance_subset_loop.body cursor subset present work := by
        rw [← CheckerContexts.carryBody_reconstruct]
        simp only [CheckerContexts.carryBody, liftSelf, advanced, bind_tc_ok, ticked]
      have actual : oracle.advance_subset_loop.body cursor subset present work = ok transition :=
        sameContext.symm.trans computed
      have frame : (returnedWork transition).cancellation = work.cancellation := by
        simp only [CheckerContexts.carryBody, liftSelf, advanced, bind_tc_ok,
          core.result.Result.Insts.CoreOpsTry.branch] at computed
        obtain ⟨index, _, indexed⟩ := takeReturn _ _ transition computed
        obtain ⟨⟨packed, replace⟩, _, borrowed⟩ := takeReturn _ _ transition indexed
        obtain ⟨offset, _, offsetRead⟩ := takeReturn _ _ transition borrowed
        obtain ⟨bit, _, shifted⟩ := takeReturn _ _ transition offsetRead
        simp only [lift, bind_tc_ok] at shifted
        split at shifted
        · obtain ⟨count, _, finished⟩ := takeReturn _ _ transition shifted
          have same := Result.ok_injective finished
          rw [← same]
          exact retained
        · obtain ⟨count, _, finished⟩ := takeReturn _ _ transition shifted
          have same := Result.ok_injective finished
          rw [← same]
          exact retained
      have nonempty : events ≠ [] := by
        rw [partition, silent, List.append_nil]
        exact observed
      refine ⟨actual, frame, ?_⟩
      cases transition with
      | cont state => exact nonempty
      | done result => trivial
  | none =>
    have contextExact : CheckerContexts.carryBody (M := Computation) ContextEvents.tick cursor subset present work =
        ITree.ret (.done (.Ok (), subset, present, work)) := by
      simp only [CheckerContexts.carryBody, advanced, ContextEvents.lift_result,
        RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]
    rw [contextExact] at run
    obtain ⟨same, _⟩ := RuntimeRuns.returned_inv _ _ _ run
    subst transition
    have actual : oracle.advance_subset_loop.body cursor subset present work =
        ok (.done (.Ok (), subset, present, work)) := by
      simp only [oracle.advance_subset_loop.body, advanced, bind_tc_ok, uncurry]
    exact ⟨actual, rfl, trivial⟩

/-- A completed returning-event carry loop has the exact generated-loop result
and retains its supplied control record. The loop induction uses only successful
body projection and nonempty continuation histories; packed subset and population invariants are
not premises of this operational correspondence. -/
theorem loop_completed (cursor : Cursor) (subset output : theory.Interpretation)
    (present count : Usize) (work after : oracle.Work) (events : List Event)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (run : Runs (CheckerContexts.carryLoop (M := Computation) RuntimeEffects.loop
      (CheckerContexts.carryBody ContextEvents.tick) cursor subset present work) events (.Ok (), output, count, after)) :
    oracle.advance_subset_loop cursor subset present work = ok (.Ok (), output, count, after) ∧
      after.cancellation = work.cancellation := by
  let body : State → Computation Transition := fun state =>
    CheckerContexts.carryBody ContextEvents.tick state.1 state.2.1 state.2.2.1 state.2.2.2
  let fixed : State → Result Transition := fun state =>
    oracle.advance_subset_loop.body state.1 state.2.1 state.2.2.1 state.2.2.2
  let invariant : State → Prop := fun state => state.2.2.2.cancellation = work.cancellation
  let accepted : Outcome → Prop := fun result => result.1 = .Ok ()
  have continues (state : State) (observations : List Event) (next : State)
      (retained : invariant state) (step : Runs (body state) observations (.cont next)) :
      fixed state = ok (.cont next) ∧ invariant next ∧ observations ≠ [] := by
    have currentClear : EvaluatorControl.observation state.2.2.2.cancellation = none := by
      rw [retained]; exact clear
    obtain ⟨actual, frame, progress⟩ := body_completed state.1 state.2.1 state.2.2.1 state.2.2.2 observations
      (.cont next) currentClear trivial step
    exact ⟨actual, frame.trans retained, progress⟩
  have finishes (state : State) (observations : List Event) (result : Outcome)
      (retained : invariant state) (success : accepted result)
      (step : Runs (body state) observations (.done result)) :
      fixed state = ok (.done result) ∧ result.2.2.2.cancellation = work.cancellation := by
    have currentClear : EvaluatorControl.observation state.2.2.2.cancellation = none := by
      rw [retained]; exact clear
    obtain ⟨actual, frame, _⟩ := body_completed state.1 state.2.1 state.2.2.1 state.2.2.2 observations
      (.done result) currentClear success step
    exact ⟨actual, frame.trans retained⟩
  have loopRun : Runs (RuntimeEffects.loop body (cursor, subset, present, work)) events (.Ok (), output, count, after) := run
  have actual := RuntimeLoop.completed_loop body fixed invariant accepted continues
    (fun state observations result retained success step =>
      (finishes state observations result retained success step).1)
    (cursor, subset, present, work) events (.Ok (), output, count, after) rfl rfl loopRun
  have frame := RuntimeLoop.completed_post body invariant accepted
    (fun result => result.2.2.2.cancellation = work.cancellation)
    (fun state observations next retained step => (continues state observations next retained step).2)
    (fun state observations result retained success step =>
      (finishes state observations result retained success step).2)
    (cursor, subset, present, work) events (.Ok (), output, count, after) rfl rfl loopRun
  exact ⟨actual, frame⟩

/-- The carry entry borrows the same selected-coordinate slice and starts its
actual iterator at zero. A successful event carry projects to that generated
entry, retaining exact words, population and work as well as the control record.
No meaning of the selected coordinates is assumed by this operational law. -/
theorem completed (selected : Slice Usize) (subset output : theory.Interpretation)
    (present count : Usize) (work after : oracle.Work) (events : List Event)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (run : Runs (ReferenceEvents.advanceSubset selected subset present work)
      events (.Ok (), output, count, after)) :
    oracle.advance_subset selected subset present work = ok (.Ok (), output, count, after) ∧
      after.cancellation = work.cancellation := by
  let cursor : Cursor := ⟨selected, 0⟩
  have setup : ReferenceEvents.advanceSubset selected subset present work =
      CheckerContexts.carryLoop (M := Computation) RuntimeEffects.loop
        (CheckerContexts.carryBody ContextEvents.tick) cursor subset present work := by
    simp only [ReferenceEvents.advanceSubset, CheckerContexts.advanceSubset,
      SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter, cursor,
      ContextEvents.lift_result, RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]
  rw [setup] at run
  obtain ⟨actual, frame⟩ := loop_completed cursor subset output present count work after events clear run
  have generated : oracle.advance_subset selected subset present work =
      oracle.advance_subset_loop cursor subset present work := by
    simp only [oracle.advance_subset,
      SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter, bind_tc_ok, cursor]
  rw [generated]
  exact ⟨actual, frame⟩

end CarryContextProjection
