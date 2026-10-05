import FixedLoop

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisExtract

/-!
# Changing observations need not describe a fixed-token loop

This checked example uses the actual generated evaluator body and loop. Two
false nodes require no atom or child reads. Refreshing only the cancellation
observation after the first node gives a stopped trace; retaining the initial
clear token gives successful exhaustion after both nodes. It is a mathematical
counterexample to identifying every refreshed trace with a fixed-token loop,
not a claim that either token record describes concurrent Rust storage.
-/
namespace ChangingObservations

/-- The same finite source slice is used throughout both computations. -/
def nodes : Slice theory.Node := Slice.from [.False, .False] (by scalar_tac)

/-- The represented zero-atom interpretation needs no packed words. -/
def candidate : theory.Interpretation := {
  theory := { owner := 0, value := {
    atoms := 0#usize
    nodes := { slice := nodes }
    roots := alloc.vec.Vec.new Usize } }
  words := alloc.vec.Vec.new U64 }

/-- Observation records differ only in the value returned by cancellation load;
both configurations have no deadline. -/
def control (cancelled : Bool) : zetesis_cpu.cancellation.Cancellation := {
  cancelled := { owner := 1, value := { nextRead := cancelled } }
  deadline := none
  slot := none }

/-- Two units admit both nodes, starting from no charged work. -/
def initial : EvaluationTrace.State := {
  cursor := { iter := { slice := nodes, i := 0 }, count := 0#usize }
  output := alloc.vec.Vec.new Bool
  limits := { max_work := 2#u64, max_subsets := 0#u64 }
  statistics := { work := 0#u64, subsets := 0#u64 } }

/-- There is a refreshed trace that observes clear then cancelled, records one
false value and one work unit, and returns cancellation. From the identical
initial state the actual generated loop with the fixed clear token instead
completes with two false values and two work units. Thus even a refresh that
changes only one observation need not preserve the fixed loop's result.

Proof: derive the first body continuation; use the cancelled observation to
finish the refreshed trace. Derive a second clear continuation and exhaustion,
then apply the actual loop equation to its returned-control call sequence. -/
theorem refreshed_trace_differs_from_fixed_loop :
    ∃ stopped completed : EvaluationTrace.Outcome,
      EvaluationTrace.Trace candidate none initial 1 stopped ∧
      stopped.result = .Err .Cancelled ∧
      stopped.output.val = [false] ∧ stopped.work.statistics.work.val = 1 ∧
      oracle.evaluate_loop initial.cursor candidate none initial.output
        initial.limits (control false) initial.statistics =
        ok (completed.result, completed.output, completed.work) ∧
      completed.result = .Ok () ∧
      completed.output.val = [false, false] ∧
      completed.work.statistics.work.val = 2 ∧
      stopped.output.val ≠ completed.output.val := by
  have stored : Membership.Represented candidate := by
    intro atom inside
    simp [candidate] at inside
  obtain ⟨firstCount, firstWork, firstOutput, countOne, workOne, firstStep,
      firstValues, firstLength, firstAligned⟩ :=
    EvaluationProgress.present_step candidate none initial.cursor initial.output
      initial.limits (control false) initial.statistics
      (by simp [initial]) (by simp [initial]) (by simp [initial, nodes]) stored
      (by simp [initial, nodes, Evaluation.ChildrenPresent])
      (by intro mask member; cases member) rfl (by decide)
  let middle : EvaluationTrace.State := {
    cursor := {
      iter := { initial.cursor.iter with i := initial.cursor.iter.i + 1 }
      count := firstCount }
    output := firstOutput
    limits := initial.limits
    statistics := { initial.statistics with work := firstWork } }
  have firstValuesExact : firstOutput.val = [false] := by
    simpa [initial, nodes, Evaluation.masked, Evaluation.value] using firstValues
  have firstWorkExact : firstWork.val = 1 := by
    simpa [initial] using workOne
  have middleAligned : middle.cursor.count.val = middle.cursor.iter.i := by
    simpa [middle, initial] using countOne
  have middleLength : middle.output.val.length = middle.cursor.iter.i := by
    simpa [middle, initial] using firstLength
  have middleInside : middle.cursor.iter.i < middle.cursor.iter.slice.val.length := by
    simp [middle, initial, nodes]
  let stopped : EvaluationTrace.Outcome := {
    result := .Err .Cancelled
    output := middle.output
    work := ⟨middle.limits, control true, middle.statistics⟩ }
  have stopStep : oracle.evaluate_loop.body candidate none middle.cursor middle.output
      middle.limits (control true) middle.statistics =
      ok (.done (stopped.result, stopped.output, stopped.work)) :=
    EvaluationProgress.control_stops candidate none middle.cursor middle.output
      middle.limits (control true) middle.statistics .Cancelled
      middleAligned middleInside rfl
  have refreshed : EvaluationTrace.Trace candidate none initial 1 stopped :=
    EvaluationTrace.Trace.next (control false) (control false) firstStep
      (EvaluationTrace.Trace.finish (control true) stopStep)
  obtain ⟨secondCount, secondWork, secondOutput, countTwo, workTwo, secondStep,
      secondValues, _, _⟩ :=
    EvaluationProgress.present_step candidate none middle.cursor middle.output
      middle.limits (control false) middle.statistics middleAligned middleLength
      middleInside stored
      (by simp [middle, initial, nodes, Evaluation.ChildrenPresent])
      (by intro mask member; cases member) rfl
      (by simp [middle, initial, firstWorkExact])
  let finalState : EvaluationTrace.State := {
    cursor := {
      iter := { middle.cursor.iter with i := middle.cursor.iter.i + 1 }
      count := secondCount }
    output := secondOutput
    limits := middle.limits
    statistics := { middle.statistics with work := secondWork } }
  let completed : EvaluationTrace.Outcome := {
    result := .Ok ()
    output := finalState.output
    work := ⟨finalState.limits, control false, finalState.statistics⟩ }
  have secondValuesExact : secondOutput.val = [false, false] := by
    simpa [middle, initial, nodes, Evaluation.masked, Evaluation.value,
      firstValuesExact] using secondValues
  have secondWorkExact : secondWork.val = 2 := by
    simpa [middle, firstWorkExact] using workTwo
  have exhausted : finalState.cursor.iter.slice.val.length ≤ finalState.cursor.iter.i := by
    simp [finalState, middle, initial, nodes]
  have finishStep : oracle.evaluate_loop.body candidate none finalState.cursor
      finalState.output finalState.limits (control false) finalState.statistics =
      ok (.done (completed.result, completed.output, completed.work)) :=
    Evaluation.exhausted_step candidate none finalState.cursor finalState.output
      finalState.limits (control false) finalState.statistics exhausted
  have calls : FixedEvaluationLoop.Calls candidate none initial (control false) 2 completed :=
    FixedEvaluationLoop.Calls.next firstStep
      (FixedEvaluationLoop.Calls.next secondStep (FixedEvaluationLoop.Calls.finish finishStep))
  have actualLoop := FixedEvaluationLoop.calls_execute candidate none calls
  refine ⟨stopped, completed, refreshed, rfl, firstValuesExact, firstWorkExact,
    actualLoop, rfl, secondValuesExact, secondWorkExact, ?_⟩
  simp only [stopped, completed, finalState, middle, firstValuesExact, secondValuesExact]
  decide

/-- It is false that every refreshed trace from this initial state is an
execution of its fixed-clear-token generated loop. The counterexample even
retains the same deadline configuration and changes only cancellation's value. -/
theorem not_every_refreshed_trace_executes :
    ¬ (∀ (count : Nat) (outcome : EvaluationTrace.Outcome),
      EvaluationTrace.Trace candidate none initial count outcome →
      oracle.evaluate_loop initial.cursor candidate none initial.output
        initial.limits (control false) initial.statistics =
        ok (outcome.result, outcome.output, outcome.work)) := by
  obtain ⟨stopped, completed, traced, _, _, _, executed, _, _, _, different⟩ :=
    refreshed_trace_differs_from_fixed_loop
  intro everyTraceExecutes
  have sameResult : (completed.result, completed.output, completed.work) =
      (stopped.result, stopped.output, stopped.work) :=
    Result.ok_injective (executed.symm.trans (everyTraceExecutes 1 stopped traced))
  have sameValues : completed.output.val = stopped.output.val :=
    congrArg (fun result => result.2.1.val) sameResult
  exact different sameValues.symm

end ChangingObservations
