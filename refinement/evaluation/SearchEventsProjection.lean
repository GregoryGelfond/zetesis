import QueryEventsProjection
import RuntimeLoop

open Aeneas Aeneas.Std Result ControlFlow Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# Projecting the eventful proper-subset search

The outer search retains the actual tested interpretation and work record.
Successful query and carry projections compose in their source order. A failed
query consumes at least its initial poll before continuation, which provides the
finite-history measure. Semantic subset coverage is reused from the fixed
checker after this operational projection, rather than assumed here.
-/
namespace SearchEventsProjection

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

abbrev State := theory.Interpretation × alloc.vec.Vec Bool × oracle.Work × Usize
abbrev Outcome := theory.Interpretation × alloc.vec.Vec Bool × oracle.Work ×
  core.result.Result Bool zetesis_cpu.cancellation.Stop

/-- A continuation or a completed verdict, excluding a terminal typed stop. -/
def accepted (transition : ControlFlow State Outcome) : Prop :=
  match transition with
  | .cont _ => True
  | .done outcome => ∃ answer, outcome.2.2.2 = .Ok answer

/-- The unchanged cancellation handle belongs to the transition's work record. -/
def control (transition : ControlFlow State Outcome) : zetesis_cpu.cancellation.Cancellation :=
  match transition with
  | .cont state => state.2.2.1.cancellation
  | .done outcome => outcome.2.2.1.cancellation

/-- Only continuations need a positive consumed-event measure. -/
def progress (events : List Event) (transition : ControlFlow State Outcome) : Prop :=
  match transition with
  | .cont _ => events ≠ []
  | .done _ => True

section Phases
variable
    (queryProjection : ∀ program subset frozen input before output after answer events,
      EvaluatorControl.observation before.cancellation = none →
      Runs (ReferenceEvents.checkSubset program subset frozen input before) events (.Ok answer, output, after) →
      oracle.check_subset program subset frozen input before = ok (.Ok answer, output, after) ∧
        after.cancellation = before.cancellation ∧ events ≠ [])
    (carryProjection : ∀ selected subset present before next count after events,
      EvaluatorControl.observation before.cancellation = none →
      Runs (ReferenceEvents.advanceSubset selected subset present before) events (.Ok (), next, count, after) →
      oracle.advance_subset selected subset present before = ok (.Ok (), next, count, after) ∧
        after.cancellation = before.cancellation)

include queryProjection carryProjection

/-- An accepted body transition is its actual fixed-body transition, with the
same control handle; every continuation consumes a query observation. -/
theorem completed_body
    (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Usize)
    (subset : theory.Interpretation) (values : alloc.vec.Vec Bool) (work : oracle.Work)
    (present : Usize) (events : List Event) (transition : ControlFlow State Outcome)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (success : accepted transition)
    (run : Runs (CheckerContexts.searchBody ReferenceEvents.checkSubset ReferenceEvents.advanceSubset
      program frozen selected subset values work present) events transition) :
    oracle.find_countermodel_loop.body program frozen selected subset values work present = ok transition ∧
      control transition = work.cancellation ∧ progress events transition := by
  unfold CheckerContexts.searchBody at run
  dsimp only at run
  split at run
  next proper =>
    obtain ⟨queryEvents, tail, queried, partition, queryRun, following⟩ := RuntimeRuns.bind_inv _ _ _ _ run
    rcases queried with ⟨answer, output, middle⟩
    cases answer with
    | Err reason =>
      simp only [ContextEvents.lift_result, embed_ok] at following
      have same := (RuntimeRuns.returned_inv _ _ _ following).1
      subst transition
      obtain ⟨answer, impossible⟩ := success
      cases impossible
    | Ok answer =>
      obtain ⟨actualQuery, queryControl, consumedQuery⟩ :=
        queryProjection program subset frozen values work output middle answer queryEvents clear queryRun
      have clearMiddle : EvaluatorControl.observation middle.cancellation = none := by
        rw [queryControl]; exact clear
      have consumed : events ≠ [] := by
        rw [partition]
        intro vacant
        exact consumedQuery (List.append_eq_nil_iff.mp vacant).1
      cases answer with
      | true =>
        simp only [↓reduceIte, ContextEvents.lift_result, embed_ok] at following
        have same := (RuntimeRuns.returned_inv _ _ _ following).1
        subst transition
        refine ⟨?_, queryControl, trivial⟩
        simp only [oracle.find_countermodel_loop.body, proper, ↓reduceIte, actualQuery, bind_tc_ok, uncurry]
      | false =>
        simp only [Bool.false_eq_true, ↓reduceIte] at following
        obtain ⟨carryEvents, suffix, carried, _, carryRun, finished⟩ := RuntimeRuns.bind_inv _ _ _ _ following
        rcases carried with ⟨carryAnswer, next, count, after⟩
        cases carryAnswer with
        | Err reason =>
          simp only [ContextEvents.lift_result, embed_ok] at finished
          have same := (RuntimeRuns.returned_inv _ _ _ finished).1
          subst transition
          obtain ⟨answer, impossible⟩ := success
          cases impossible
        | Ok value =>
          cases value
          obtain ⟨actualCarry, carryControl⟩ :=
            carryProjection selected subset present middle next count after carryEvents clearMiddle carryRun
          simp only [ContextEvents.lift_result, embed_ok] at finished
          have same := (RuntimeRuns.returned_inv _ _ _ finished).1
          subst transition
          refine ⟨?_, ?_, consumed⟩
          · simp only [oracle.find_countermodel_loop.body, proper, ↓reduceIte,
              actualQuery, bind_tc_ok, uncurry, Bool.false_eq_true, actualCarry]
          · exact carryControl.trans queryControl
  next exhausted =>
    have same := (RuntimeRuns.returned_inv _ _ _ run).1
    subst transition
    refine ⟨?_, rfl, trivial⟩
    simp only [oracle.find_countermodel_loop.body, exhausted, ↓reduceIte]

/-- A successful eventful subset-search loop is the actual generated loop with
exactly the same result and cancellation handle. The local call projections,
not semantic coverage assumptions, justify this operational correspondence. -/
theorem completed_loop
    (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Usize)
    (subset next : theory.Interpretation) (values output : alloc.vec.Vec Bool)
    (work after : oracle.Work) (present : Usize) (answer : Bool) (events : List Event)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (run : Runs (CheckerContexts.searchLoop RuntimeEffects.loop
      (CheckerContexts.searchBody ReferenceEvents.checkSubset ReferenceEvents.advanceSubset)
      program frozen selected subset values work present) events (next, output, after, .Ok answer)) :
    oracle.find_countermodel_loop program frozen selected subset values work present =
      ok (next, output, after, .Ok answer) ∧ after.cancellation = work.cancellation := by
  let body := fun state : State => CheckerContexts.searchBody ReferenceEvents.checkSubset
    ReferenceEvents.advanceSubset program frozen selected state.1 state.2.1 state.2.2.1 state.2.2.2
  let fixed := fun state : State => oracle.find_countermodel_loop.body program frozen selected
    state.1 state.2.1 state.2.2.1 state.2.2.2
  let invariant := fun state : State => state.2.2.1.cancellation = work.cancellation
  let success := fun outcome : Outcome => ∃ verdict, outcome.2.2.2 = .Ok verdict
  have continued : ∀ state observations following, invariant state →
      Runs (body state) observations (.cont following) →
      fixed state = ok (.cont following) ∧ invariant following ∧ observations ≠ [] := by
    intro state observations following same step
    rcases state with ⟨tested, currentValues, currentWork, count⟩
    have currentClear : EvaluatorControl.observation currentWork.cancellation = none := by
      rw [same]; exact clear
    obtain ⟨actual, controlSame, consumed⟩ :=
      completed_body queryProjection carryProjection program frozen selected tested currentValues
        currentWork count observations (.cont following) currentClear trivial step
    exact ⟨actual, controlSame.trans same, consumed⟩
  have finished : ∀ state observations returned, invariant state → success returned →
      Runs (body state) observations (.done returned) →
      fixed state = ok (.done returned) := by
    intro state observations returned same accepted step
    rcases state with ⟨tested, currentValues, currentWork, count⟩
    have currentClear : EvaluatorControl.observation currentWork.cancellation = none := by
      rw [same]; exact clear
    exact (completed_body queryProjection carryProjection program frozen selected tested currentValues
      currentWork count observations (.done returned) currentClear accepted step).1
  have loopRun : Runs (RuntimeEffects.loop body (subset, values, work, present)) events
      (next, output, after, .Ok answer) := run
  have generated := RuntimeLoop.completed_loop body fixed invariant success continued finished
    (subset, values, work, present) events (next, output, after, .Ok answer)
    rfl ⟨answer, rfl⟩ loopRun
  have framed : after.cancellation = work.cancellation := by
    apply RuntimeLoop.completed_post body invariant success
      (fun outcome => outcome.2.2.1.cancellation = work.cancellation)
      (fun state observations following same step =>
        (continued state observations following same step).2)
      ?_ (subset, values, work, present) events (next, output, after, .Ok answer)
      rfl ⟨answer, rfl⟩ loopRun
    intro state observations returned same accepted step
    rcases state with ⟨tested, currentValues, currentWork, count⟩
    have currentClear : EvaluatorControl.observation currentWork.cancellation = none := by
      rw [same]; exact clear
    have retained := (completed_body queryProjection carryProjection program frozen selected tested
      currentValues currentWork count observations (.done returned) currentClear accepted step).2.1
    exact retained.trans same
  exact ⟨generated, framed⟩

/-- The public search wrapper preserves the loop's actual witness, truth value,
workspace and work. Its result reordering adds no observation or semantic claim. -/
theorem completed_search
    (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Usize)
    (subset next : theory.Interpretation) (values output : alloc.vec.Vec Bool)
    (work after : oracle.Work) (answer : Bool) (events : List Event)
    (clear : EvaluatorControl.observation work.cancellation = none)
    (run : Runs (ReferenceEvents.findCountermodel program frozen selected subset values work)
      events (.Ok answer, next, output, after)) :
    oracle.find_countermodel program frozen selected subset values work =
      ok (.Ok answer, next, output, after) ∧ after.cancellation = work.cancellation := by
  unfold ReferenceEvents.findCountermodel CheckerContexts.findCountermodel at run
  obtain ⟨before, suffix, returned, _, loopRun, following⟩ := RuntimeRuns.bind_inv _ _ _ _ run
  rcases returned with ⟨tested, retained, counted, verdict⟩
  have same := (RuntimeRuns.embed_iff _ _ _).mp following |>.1
  have exactResult : (verdict, tested, retained, counted) = (.Ok answer, next, output, after) :=
    Result.ok_injective same
  cases exactResult
  obtain ⟨actual, frame⟩ := completed_loop queryProjection carryProjection program frozen selected subset
    next values output work after 0#usize answer before clear loopRun
  refine ⟨?_, frame⟩
  simp only [oracle.find_countermodel, actual, bind_tc_ok, uncurry]

end Phases
end SearchEventsProjection
