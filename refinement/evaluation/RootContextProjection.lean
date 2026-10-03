import CheckerContexts
import TickProjection
import RuntimeLoop
import RootScan
import ReferenceEvents

open Aeneas Aeneas.Std Result ControlFlow Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# Completed root scans with returning reads

The checked root context replaces only its tick call. A completed event run
therefore has the same root reads and result as the actual generated body under
clear fixed observations. This projection assumes no truth-slice coverage: a
successful finite run itself rules out backend read failure. It neither supplies
semantic truth to the roots nor establishes that a runtime operation returns.
-/
namespace RootContextProjection

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

abbrev Cursor := core.slice.iter.Iter Usize
abbrev State := Cursor × oracle.Work
abbrev Outcome := core.result.Result (Option Usize) zetesis_cpu.cancellation.Stop × oracle.Work
abbrev Transition := ControlFlow State Outcome

/-- Source success includes a found false root and a complete true scan. -/
def Successful : Transition → Prop
  | .cont _ => True
  | .done (answer, _) => ∃ result, answer = .Ok result

/-- A continuation must consume a read; a completed exit may be pure. -/
def Progress (events : List Event) : Transition → Prop
  | .cont _ => events ≠ []
  | .done _ => True

/-- Work retained by either branch of a body invocation. -/
def returnedWork : Transition → oracle.Work
  | .cont (_, work) => work
  | .done (_, work) => work

/-- Embedding a backend tick embeds the entire unchanged root context,
including checked iterator and truth reads and any backend failure. -/
theorem embed_context
    (tick : oracle.Work → Result (core.result.Result Unit zetesis_cpu.cancellation.Stop × oracle.Work))
    (values : Slice Bool) (cursor : Cursor) (work : oracle.Work) :
    CheckerContexts.rootBody (M := Computation) (fun state => embed (tick state)) values cursor work =
      embed (CheckerContexts.rootBody (M := Result) tick values cursor work) := by
  have liftSelf {T : Type} (operation : Result T) : (liftM operation : Result T) = operation := rfl
  have embedThen {T U : Type} (operation : Result T) (next : T → Result U) :
      embed (do let value ← operation; next value) =
        (do let value ← embed operation; embed (next value)) := RuntimeEffects.embed_bind operation next
  unfold CheckerContexts.rootBody
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
      simp only [embedThen]
      congr 1
      funext truth
      cases truth <;> rfl

/-- A present root reaches exactly one tick; all remaining operations are an
embedding of the unchanged generated suffix. The supplied iterator equation is
an actual operation result, not an assumption of root truth. -/
theorem context_at_tick
    (tick : oracle.Work → Computation (core.result.Result Unit zetesis_cpu.cancellation.Stop × oracle.Work))
    (values : Slice Bool) (cursor nextCursor : Cursor) (root : Usize) (work : oracle.Work)
    (advanced : core.slice.iter.IteratorSliceIter.next cursor = ok (some root, nextCursor)) :
    CheckerContexts.rootBody (M := Computation) tick values cursor work =
      (do let ticked ← tick work
          embed (CheckerContexts.rootBody (M := Result) (fun _ => ok ticked) values cursor work)) := by
  simp only [← embed_context]
  simp only [CheckerContexts.rootBody, advanced, ContextEvents.lift_result,
    RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]

/-- A successful finite root-body run projects to the actual body, preserves
its control record and consumes a read on every continuation. Exhaustion has no
tick; otherwise a source stop cannot yield a successful transition. All later
backend operations are recovered from the successful embedded suffix. -/
theorem body_completed (values : Slice Bool) (cursor : Cursor) (work : oracle.Work)
    (events : List Event) (transition : Transition)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (successful : Successful transition)
    (run : Runs (CheckerContexts.rootBody (M := Computation) ContextEvents.tick values cursor work)
      events transition) :
    oracle.failed_root_loop.body values cursor work = ok transition ∧
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
  by_cases inside : cursor.i < cursor.slice.val.length
  · have advanced := EvaluatorIteration.slice_next_present cursor inside
    rw [context_at_tick ContextEvents.tick values cursor _ _ work advanced] at run
    obtain ⟨before, after, ticked, partition, tickRun, continuation⟩ := RuntimeRuns.bind_inv _ _ _ _ run
    obtain ⟨computed, silent⟩ := (RuntimeRuns.embed_iff _ _ _).mp continuation
    rcases ticked with ⟨answer, returned⟩
    cases answer with
    | Err reason =>
      have refused : CheckerContexts.rootBody (M := Result) (fun _ => ok (.Err reason, returned))
          values cursor work = ok (.done (.Err reason, returned)) := by
        simp only [CheckerContexts.rootBody, liftSelf, advanced, bind_tc_ok,
          core.result.Result.Insts.CoreOpsTry.branch,
          core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
          core.convert.FromSame.from]
      have same : .done (.Err reason, returned) = transition :=
        Result.ok_injective (refused.symm.trans computed)
      rw [← same] at successful
      obtain ⟨_, impossible⟩ := successful
      cases impossible
    | Ok accepted =>
      cases accepted
      obtain ⟨ticked, observed⟩ := TickProjection.completed_tick work returned before clear tickRun
      have retained : returned.cancellation = work.cancellation :=
        (TickProjection.completed_receipt work returned before clear tickRun).2.2.2.1
      have sameContext : CheckerContexts.rootBody (M := Result) (fun _ => ok (.Ok (), returned))
          values cursor work = oracle.failed_root_loop.body values cursor work := by
        rw [← CheckerContexts.rootBody_reconstruct]
        simp only [CheckerContexts.rootBody, liftSelf, advanced, bind_tc_ok, ticked]
      have actual : oracle.failed_root_loop.body values cursor work = ok transition :=
        sameContext.symm.trans computed
      have frame : (returnedWork transition).cancellation = work.cancellation := by
        simp only [CheckerContexts.rootBody, liftSelf, advanced, bind_tc_ok,
          core.result.Result.Insts.CoreOpsTry.branch] at computed
        obtain ⟨truth, _, finished⟩ := takeReturn _ _ transition computed
        cases truth <;>
          have same := Result.ok_injective finished <;>
          rw [← same] <;> exact retained
      have nonempty : events ≠ [] := by
        rw [partition, silent, List.append_nil]
        exact observed
      refine ⟨actual, frame, ?_⟩
      cases transition with
      | cont state => exact nonempty
      | done result => trivial
  · have exhausted := EvaluatorIteration.slice_next_exhausted cursor (Nat.le_of_not_gt inside)
    have contextExact : CheckerContexts.rootBody (M := Computation) ContextEvents.tick values cursor work =
        ITree.ret (.done (.Ok none, work)) := by
      simp only [CheckerContexts.rootBody, exhausted, ContextEvents.lift_result,
        RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]
    rw [contextExact] at run
    obtain ⟨same, _⟩ := RuntimeRuns.returned_inv _ _ _ run
    subst transition
    exact ⟨FixedRootScan.body_exhausted values cursor work (Nat.le_of_not_gt inside), rfl, trivial⟩

/-- A completed returning-event root loop has the exact generated-loop result
and retains its supplied control record. The loop induction uses only successful
body projection and nonempty continuation histories; root coverage and truth are
not premises of this operational correspondence. -/
theorem loop_completed (values : Slice Bool) (cursor : Cursor) (work after : oracle.Work)
    (events : List Event) (answer : Option Usize)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (run : Runs (CheckerContexts.rootLoop (M := Computation) RuntimeEffects.loop
      (CheckerContexts.rootBody ContextEvents.tick) cursor values work) events (.Ok answer, after)) :
    oracle.failed_root_loop cursor values work = ok (.Ok answer, after) ∧
      after.cancellation = work.cancellation := by
  let body : State → Computation Transition := fun state =>
    CheckerContexts.rootBody ContextEvents.tick values state.1 state.2
  let fixed : State → Result Transition := fun state =>
    oracle.failed_root_loop.body values state.1 state.2
  let invariant : State → Prop := fun state => state.2.cancellation = work.cancellation
  let accepted : Outcome → Prop := fun result => ∃ answer, result.1 = .Ok answer
  have continues (state : State) (observations : List Event) (next : State)
      (retained : invariant state) (step : Runs (body state) observations (.cont next)) :
      fixed state = ok (.cont next) ∧ invariant next ∧ observations ≠ [] := by
    have currentClear : EvaluatorControl.observation state.2.cancellation = none := by
      rw [retained]; exact clear
    obtain ⟨actual, frame, progress⟩ := body_completed values state.1 state.2 observations
      (.cont next) currentClear trivial step
    exact ⟨actual, frame.trans retained, progress⟩
  have finishes (state : State) (observations : List Event) (result : Outcome)
      (retained : invariant state) (success : accepted result)
      (step : Runs (body state) observations (.done result)) :
      fixed state = ok (.done result) ∧ result.2.cancellation = work.cancellation := by
    have currentClear : EvaluatorControl.observation state.2.cancellation = none := by
      rw [retained]; exact clear
    obtain ⟨actual, frame, _⟩ := body_completed values state.1 state.2 observations
      (.done result) currentClear success step
    exact ⟨actual, frame.trans retained⟩
  have loopRun : Runs (RuntimeEffects.loop body (cursor, work)) events (.Ok answer, after) := run
  have actual := RuntimeLoop.completed_loop body fixed invariant accepted continues
    (fun state observations result retained success step =>
      (finishes state observations result retained success step).1)
    (cursor, work) events (.Ok answer, after) rfl ⟨answer, rfl⟩ loopRun
  have frame := RuntimeLoop.completed_post body invariant accepted
    (fun result => result.2.cancellation = work.cancellation)
    (fun state observations next retained step => (continues state observations next retained step).2)
    (fun state observations result retained success step =>
      (finishes state observations result retained success step).2)
    (cursor, work) events (.Ok answer, after) rfl ⟨answer, rfl⟩ loopRun
  exact ⟨actual, frame⟩

/-- The actual root-scan entry setup introduces no observation. A successful
returning-event scan therefore projects through the same stored root iterator to
the generated entry, retaining control and the exact failed-root option. -/
theorem completed (program : theory.Theory) (values : Slice Bool) (work after : oracle.Work)
    (events : List Event) (answer : Option Usize)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (run : Runs (ReferenceEvents.failedRoot program values work) events (.Ok answer, after)) :
    oracle.failed_root program values work = ok (.Ok answer, after) ∧
      after.cancellation = work.cancellation := by
  have setup : ReferenceEvents.failedRoot program values work =
      CheckerContexts.rootLoop (M := Computation) RuntimeEffects.loop
        (CheckerContexts.rootBody ContextEvents.tick) (FixedRootScan.initialCursor program) values work := by
    simp only [ReferenceEvents.failedRoot, CheckerContexts.rootScan, theory.Theory.roots,
      alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
      SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter,
      FixedRootScan.initialCursor, ContextEvents.lift_result, RuntimeEffects.embed_ok,
      Bind.bind, itree_ret_bind, bind_ok]
  rw [setup] at run
  obtain ⟨actual, frame⟩ := loop_completed values (FixedRootScan.initialCursor program)
    work after events answer clear run
  rw [FixedRootScan.failed_root_from_start]
  exact ⟨actual, frame⟩

end RootContextProjection
