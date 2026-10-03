import CheckerContexts
import TickProjection
import RuntimeLoop
import SubsetSteps
import ReferenceEvents

open Aeneas Aeneas.Std Result ControlFlow Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# Completed atom selections with returning reads

The checked selection context replaces only its tick call. A completed event run
therefore has the same membership reads, appends and result as the actual generated body under
clear fixed observations. This projection assumes no candidate representation: a
successful finite run itself rules out backend read failure. It neither supplies
semantic membership to the candidate nor establishes that a runtime operation returns.
-/
namespace SelectionContextProjection

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

abbrev Cursor := core.ops.range.Range Usize
abbrev State := Cursor × alloc.vec.Vec Usize × oracle.Work
abbrev Outcome := core.result.Result Unit zetesis_cpu.cancellation.Stop × alloc.vec.Vec Usize × oracle.Work
abbrev Transition := ControlFlow State Outcome

/-- Source success is the completed atom scan; source stops remain excluded. -/
def Successful : Transition → Prop
  | .cont _ => True
  | .done (answer, _, _) => answer = .Ok ()

/-- A continuation must consume a read; a completed exit may be pure. -/
def Progress (events : List Event) : Transition → Prop
  | .cont _ => events ≠ []
  | .done _ => True

/-- Work retained by either branch of a body invocation. -/
def returnedWork : Transition → oracle.Work
  | .cont (_, _, work) => work
  | .done (_, _, work) => work

/-- Embedding a backend tick embeds the entire unchanged selection context,
including checked iterator and membership reads and any backend failure. -/
theorem embed_context
    (tick : oracle.Work → Result (core.result.Result Unit zetesis_cpu.cancellation.Stop × oracle.Work))
    (candidate : theory.Interpretation) (cursor : Cursor) (selected : alloc.vec.Vec Usize) (work : oracle.Work) :
    CheckerContexts.selectionBody (M := Computation) (fun state => embed (tick state)) candidate cursor selected work =
      embed (CheckerContexts.selectionBody (M := Result) tick candidate cursor selected work) := by
  have liftSelf {T : Type} (operation : Result T) : (liftM operation : Result T) = operation := rfl
  have embedThen {T U : Type} (operation : Result T) (next : T → Result U) :
      embed (do let value ← operation; next value) =
        (do let value ← embed operation; embed (next value)) := RuntimeEffects.embed_bind operation next
  unfold CheckerContexts.selectionBody
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
      cases truth with
      | false => rfl
      | true => simp only [↓reduceIte, embedThen]

/-- A present coordinate reaches exactly one tick; all remaining operations are an
embedding of the unchanged generated suffix. The supplied iterator equation is
an actual operation result, not an assumption of candidate membership. -/
theorem context_at_tick
    (tick : oracle.Work → Computation (core.result.Result Unit zetesis_cpu.cancellation.Stop × oracle.Work))
    (candidate : theory.Interpretation) (cursor nextCursor : Cursor) (atom : Usize)
    (selected : alloc.vec.Vec Usize) (work : oracle.Work)
    (advanced : core.iter.range.IteratorRange.next core.iter.range.StepUsize cursor = ok (some atom, nextCursor)) :
    CheckerContexts.selectionBody (M := Computation) tick candidate cursor selected work =
      (do let ticked ← tick work
          embed (CheckerContexts.selectionBody (M := Result) (fun _ => ok ticked) candidate cursor selected work)) := by
  simp only [← embed_context]
  simp only [CheckerContexts.selectionBody, advanced, ContextEvents.lift_result,
    RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]

/-- A successful finite selection-body run projects to the actual body, preserves
its control record and consumes a read on every continuation. Exhaustion has no
tick; otherwise a source stop cannot yield a successful transition. All later
backend operations are recovered from the successful embedded suffix. -/
theorem body_completed (candidate : theory.Interpretation) (cursor : Cursor) (selected : alloc.vec.Vec Usize) (work : oracle.Work)
    (events : List Event) (transition : Transition)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (successful : Successful transition)
    (run : Runs (CheckerContexts.selectionBody (M := Computation) ContextEvents.tick candidate cursor selected work)
      events transition) :
    oracle.select_atoms_loop.body candidate cursor selected work = ok transition ∧
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
  unfold CheckerContexts.selectionBody at first
  obtain ⟨iteratorEvents, tailEvents, advancedValue, _, iteratorRun, _⟩ :=
    RuntimeRuns.bind_inv _ _ _ _ first
  obtain ⟨advanced, _⟩ := (RuntimeRuns.embed_iff _ _ _).mp iteratorRun
  change core.iter.range.IteratorRange.next core.iter.range.StepUsize cursor = ok advancedValue at advanced
  rcases advancedValue with ⟨item, nextCursor⟩
  cases item with
  | some atom =>
    rw [context_at_tick ContextEvents.tick candidate cursor nextCursor atom selected work advanced] at run
    obtain ⟨before, after, ticked, partition, tickRun, continuation⟩ := RuntimeRuns.bind_inv _ _ _ _ run
    obtain ⟨computed, silent⟩ := (RuntimeRuns.embed_iff _ _ _).mp continuation
    rcases ticked with ⟨answer, returned⟩
    cases answer with
    | Err reason =>
      have refused : CheckerContexts.selectionBody (M := Result) (fun _ => ok (.Err reason, returned))
          candidate cursor selected work = ok (.done (.Err reason, selected, returned)) := by
        simp only [CheckerContexts.selectionBody, liftSelf, advanced, bind_tc_ok,
          core.result.Result.Insts.CoreOpsTry.branch,
          core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
          core.convert.FromSame.from]
      have same : .done (.Err reason, selected, returned) = transition :=
        Result.ok_injective (refused.symm.trans computed)
      rw [← same] at successful
      cases successful
    | Ok accepted =>
      cases accepted
      obtain ⟨ticked, observed⟩ := TickProjection.completed_tick work returned before clear tickRun
      have retained : returned.cancellation = work.cancellation :=
        (TickProjection.completed_receipt work returned before clear tickRun).2.2.2.1
      have sameContext : CheckerContexts.selectionBody (M := Result) (fun _ => ok (.Ok (), returned))
          candidate cursor selected work = oracle.select_atoms_loop.body candidate cursor selected work := by
        rw [← CheckerContexts.selectionBody_reconstruct]
        simp only [CheckerContexts.selectionBody, liftSelf, advanced, bind_tc_ok, ticked]
      have actual : oracle.select_atoms_loop.body candidate cursor selected work = ok transition :=
        sameContext.symm.trans computed
      have frame : (returnedWork transition).cancellation = work.cancellation := by
        simp only [CheckerContexts.selectionBody, liftSelf, advanced, bind_tc_ok,
          core.result.Result.Insts.CoreOpsTry.branch] at computed
        obtain ⟨truth, _, finished⟩ := takeReturn _ _ transition computed
        cases truth with
        | false =>
          have same := Result.ok_injective finished
          rw [← same]
          exact retained
        | true =>
          obtain ⟨output, _, published⟩ := takeReturn _ _ transition finished
          have same := Result.ok_injective published
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
    have contextExact : CheckerContexts.selectionBody (M := Computation) ContextEvents.tick candidate cursor selected work =
        ITree.ret (.done (.Ok (), selected, work)) := by
      simp only [CheckerContexts.selectionBody, advanced, ContextEvents.lift_result,
        RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind]
    rw [contextExact] at run
    obtain ⟨same, _⟩ := RuntimeRuns.returned_inv _ _ _ run
    subst transition
    have actual : oracle.select_atoms_loop.body candidate cursor selected work =
        ok (.done (.Ok (), selected, work)) := by
      simp only [oracle.select_atoms_loop.body, advanced, bind_tc_ok, uncurry]
    exact ⟨actual, rfl, trivial⟩

/-- A completed returning-event selection loop has the exact generated-loop result
and retains its supplied control record. The loop induction uses only successful
body projection and nonempty continuation histories; packed membership and selection semantics are
not premises of this operational correspondence. -/
theorem loop_completed (candidate : theory.Interpretation) (cursor : Cursor)
    (selected output : alloc.vec.Vec Usize) (work after : oracle.Work)
    (events : List Event)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (run : Runs (CheckerContexts.selectionLoop (M := Computation) RuntimeEffects.loop
      (CheckerContexts.selectionBody ContextEvents.tick) cursor candidate selected work) events (.Ok (), output, after)) :
    oracle.select_atoms_loop cursor candidate selected work = ok (.Ok (), output, after) ∧
      after.cancellation = work.cancellation := by
  let body : State → Computation Transition := fun state =>
    CheckerContexts.selectionBody ContextEvents.tick candidate state.1 state.2.1 state.2.2
  let fixed : State → Result Transition := fun state =>
    oracle.select_atoms_loop.body candidate state.1 state.2.1 state.2.2
  let invariant : State → Prop := fun state => state.2.2.cancellation = work.cancellation
  let accepted : Outcome → Prop := fun result => result.1 = .Ok ()
  have continues (state : State) (observations : List Event) (next : State)
      (retained : invariant state) (step : Runs (body state) observations (.cont next)) :
      fixed state = ok (.cont next) ∧ invariant next ∧ observations ≠ [] := by
    have currentClear : EvaluatorControl.observation state.2.2.cancellation = none := by
      rw [retained]; exact clear
    obtain ⟨actual, frame, progress⟩ := body_completed candidate state.1 state.2.1 state.2.2 observations
      (.cont next) currentClear trivial step
    exact ⟨actual, frame.trans retained, progress⟩
  have finishes (state : State) (observations : List Event) (result : Outcome)
      (retained : invariant state) (success : accepted result)
      (step : Runs (body state) observations (.done result)) :
      fixed state = ok (.done result) ∧ result.2.2.cancellation = work.cancellation := by
    have currentClear : EvaluatorControl.observation state.2.2.cancellation = none := by
      rw [retained]; exact clear
    obtain ⟨actual, frame, _⟩ := body_completed candidate state.1 state.2.1 state.2.2 observations
      (.done result) currentClear success step
    exact ⟨actual, frame.trans retained⟩
  have loopRun : Runs (RuntimeEffects.loop body (cursor, selected, work)) events (.Ok (), output, after) := run
  have actual := RuntimeLoop.completed_loop body fixed invariant accepted continues
    (fun state observations result retained success step =>
      (finishes state observations result retained success step).1)
    (cursor, selected, work) events (.Ok (), output, after) rfl rfl loopRun
  have frame := RuntimeLoop.completed_post body invariant accepted
    (fun result => result.2.2.cancellation = work.cancellation)
    (fun state observations next retained step => (continues state observations next retained step).2)
    (fun state observations result retained success step =>
      (finishes state observations result retained success step).2)
    (cursor, selected, work) events (.Ok (), output, after) rfl rfl loopRun
  exact ⟨actual, frame⟩

/-- Pure setup starts the actual atom range at zero. A successful event scan
projects to the generated selection entry with the same vector and work record;
its packed-selection meaning is supplied by the existing semantic theorem. -/
theorem completed (program : theory.Theory) (candidate : theory.Interpretation)
    (selected output : alloc.vec.Vec Usize) (work after : oracle.Work)
    (events : List Event)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (run : Runs (ReferenceEvents.selectAtoms program candidate selected work)
      events (.Ok (), output, after)) :
    oracle.select_atoms program candidate selected work = ok (.Ok (), output, after) ∧
      after.cancellation = work.cancellation := by
  let cursor : Cursor := ⟨0#usize, program.value.atoms⟩
  have setup : ReferenceEvents.selectAtoms program candidate selected work =
      CheckerContexts.selectionLoop (M := Computation) RuntimeEffects.loop
        (CheckerContexts.selectionBody ContextEvents.tick) cursor candidate selected work := by
    simp only [ReferenceEvents.selectAtoms, CheckerContexts.selectAtoms, theory.Theory.atom_count,
      alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, cursor,
      ContextEvents.lift_result, RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind, bind_ok]
  rw [setup] at run
  obtain ⟨actual, frame⟩ := loop_completed candidate cursor selected output work after events clear run
  have generated : oracle.select_atoms program candidate selected work =
      oracle.select_atoms_loop cursor candidate selected work := by
    simp only [oracle.select_atoms, theory.Theory.atom_count,
      alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, bind_tc_ok, cursor]
  rw [generated]
  exact ⟨actual, frame⟩

end SelectionContextProjection
