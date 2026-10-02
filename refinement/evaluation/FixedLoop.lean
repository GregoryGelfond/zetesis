import ReductTrace

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisExtract

/-!
# The generated loop under fixed observation tokens

The unchanged generated `oracle.evaluate_loop` threads the control value returned
by each generated body into the next invocation. This module follows that exact
threading, constructs a finite sequence of its calls, and proves the generated
loop returns the sequence's outcome under the current mathematical external
models. `Atomic.load` in those models reads a fixed field of its supplied value.

This is an exact theorem about the extracted function under those definitions.
It is not a model of concurrently changing Rust atomics, timer progress, pointer
identity or allocator effects. The broader `EvaluationTrace.Trace` permits
externally refreshed control values; this module proves the generated fixed-token
calls erase into that sound relation, without claiming every such trace is a run
of the generated loop.
-/
namespace FixedEvaluationLoop

/-- Actual generated calls with the returned control value passed to the next
invocation. Unlike the broader trace relation, no fresh control input is supplied
between calls. The count records continuation transitions only. -/
inductive Calls (candidate : theory.Interpretation) (frozen : Option (Slice Bool)) :
    EvaluationTrace.State → zetesis_cpu.cancellation.Cancellation → Nat →
      EvaluationTrace.Outcome → Prop where
  | finish {state : EvaluationTrace.State} {control : zetesis_cpu.cancellation.Cancellation}
      {outcome : EvaluationTrace.Outcome}
      (step : oracle.evaluate_loop.body candidate frozen state.cursor state.output
        state.limits control state.statistics =
        ok (.done (outcome.result, outcome.output, outcome.work))) :
      Calls candidate frozen state control 0 outcome
  | next {state nextState : EvaluationTrace.State}
      {control returnedControl : zetesis_cpu.cancellation.Cancellation}
      {count : Nat} {outcome : EvaluationTrace.Outcome}
      (step : oracle.evaluate_loop.body candidate frozen state.cursor state.output
        state.limits control state.statistics =
        ok (.cont (nextState.cursor, nextState.output, nextState.limits,
          returnedControl, nextState.statistics)))
      (rest : Calls candidate frozen nextState returnedControl count outcome) :
      Calls candidate frozen state control (count + 1) outcome

/-- Forgetting the control-threading restriction gives a trace of actual calls.
Every constructor retains the same generated result equation. -/
theorem calls_are_trace (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) {state : EvaluationTrace.State}
    {control : zetesis_cpu.cancellation.Cancellation} {count : Nat}
    {outcome : EvaluationTrace.Outcome}
    (calls : Calls candidate frozen state control count outcome) :
    EvaluationTrace.Trace candidate frozen state count outcome := by
  induction calls with
  | @finish state control outcome step =>
    exact EvaluationTrace.Trace.finish control step
  | @next state nextState control returnedControl count outcome step rest ih =>
    exact EvaluationTrace.Trace.next control returnedControl step ih

/-- One unfolding of the actual generated loop either returns the body's done
result or invokes the same generated loop with every returned continuation field.
In particular it retains the returned control, rather than choosing a new token. -/
theorem loop_unfold (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (state : EvaluationTrace.State) (control : zetesis_cpu.cancellation.Cancellation) :
    oracle.evaluate_loop state.cursor candidate frozen state.output
      state.limits control state.statistics = (do
        let transition ← oracle.evaluate_loop.body candidate frozen state.cursor state.output
          state.limits control state.statistics
        match transition with
        | .done result => ok result
        | .cont (cursor, output, limits, returnedControl, statistics) =>
          oracle.evaluate_loop cursor candidate frozen output limits returnedControl statistics) := by
  rw [oracle.evaluate_loop, loop]
  dsimp only [uncurry]
  congr 1
  funext transition
  cases transition with
  | done result => rfl
  | cont result =>
    obtain ⟨cursor, output, limits, returnedControl, statistics⟩ := result
    rfl

/-- A finite sequence with exact returned-control threading is the actual
result of the unchanged generated loop under the current external definitions.
No loop-result equation is supplied as a premise. -/
theorem calls_execute (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) {state : EvaluationTrace.State}
    {control : zetesis_cpu.cancellation.Cancellation} {count : Nat}
    {outcome : EvaluationTrace.Outcome}
    (calls : Calls candidate frozen state control count outcome) :
    oracle.evaluate_loop state.cursor candidate frozen state.output
      state.limits control state.statistics =
      ok (outcome.result, outcome.output, outcome.work) := by
  induction calls with
  | @finish state control outcome step =>
    rw [loop_unfold candidate frozen state control, step]
    simp only [bind_tc_ok]
  | @next state nextState control returnedControl count outcome step rest ih =>
    rw [loop_unfold candidate frozen state control, step]
    simpa only [bind_tc_ok] using ih

/-- Under the stored-word, ordered-table and mask-coverage invariants, the
actual body has a finite control-threaded call sequence. There are no more
continuations than unvisited nodes. A work or control refusal is a terminating
typed outcome, so no successful-completion or remaining-budget premise is needed.

Proof: the existing generated-body refinement supplies its actual transition.
A done transition closes the sequence. A continuation preserves the invariant
and strictly decreases the unvisited node count; recurse with the returned
control value, exactly as the generated loop does. -/
theorem calls_exist (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (table : List theory.Node)
    (state : EvaluationTrace.State) (control : zetesis_cpu.cancellation.Cancellation)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered table)
    (covered : ∀ mask ∈ frozen, table.length ≤ mask.val.length)
    (invariant : EvaluationTrace.Invariant candidate frozen table state) :
    ∃ count : Nat, ∃ outcome : EvaluationTrace.Outcome,
      Calls candidate frozen state control count outcome ∧
      count ≤ table.length - state.cursor.iter.i := by
  have construct (remaining : Nat) :
      ∀ current : EvaluationTrace.State,
        ∀ currentControl : zetesis_cpu.cancellation.Cancellation,
        EvaluationTrace.Invariant candidate frozen table current →
        table.length - current.cursor.iter.i = remaining →
        ∃ count : Nat, ∃ outcome : EvaluationTrace.Outcome,
          Calls candidate frozen current currentControl count outcome ∧ count ≤ remaining := by
    induction remaining using Nat.strong_induction_on with
    | h remaining ih =>
      intro current currentControl currentInvariant unvisited
      obtain ⟨transition, actual, effect⟩ := EvaluationTrace.invocation_refines
        candidate frozen table current currentControl stored ordered covered currentInvariant
      cases transition with
      | done result =>
        obtain ⟨result, output, work⟩ := result
        exact ⟨0, ⟨result, output, work⟩, Calls.finish actual, Nat.zero_le remaining⟩
      | cont result =>
        obtain ⟨cursor, output, limits, returnedControl, statistics⟩ := result
        let nextState : EvaluationTrace.State := ⟨cursor, output, limits, statistics⟩
        change EvaluationTrace.Continued candidate frozen table current nextState at effect
        obtain ⟨nextInvariant, advanced, _, _, _⟩ := effect
        have lessUnvisited : table.length - nextState.cursor.iter.i < remaining := by
          have within : nextState.cursor.iter.i ≤ table.length := nextInvariant.bounded
          omega
        obtain ⟨count, outcome, rest, bounded⟩ := ih
          (table.length - nextState.cursor.iter.i) lessUnvisited
          nextState returnedControl nextInvariant rfl
        refine ⟨count + 1, outcome, Calls.next actual rest, ?_⟩
        have within : nextState.cursor.iter.i ≤ table.length := nextInvariant.bounded
        omega
  exact construct (table.length - state.cursor.iter.i) state control invariant rfl

/-- The unchanged generated loop terminates with an outcome witnessed by an
actual-call trace. Its output is the exact visited truth prefix, and its work
increase is exactly the number of continuation steps. This is conditional on
the current external definitions, whose atomic tokens have fixed observations;
it is not an equivalence with arbitrary refreshed-observation traces.

Proof: construct the control-threaded calls, execute them by unfolding the real
loop, then erase only their control restriction to apply the established trace
preservation theorem. -/
theorem loop_refines (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (table : List theory.Node)
    (state : EvaluationTrace.State) (control : zetesis_cpu.cancellation.Cancellation)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered table)
    (covered : ∀ mask ∈ frozen, table.length ≤ mask.val.length)
    (invariant : EvaluationTrace.Invariant candidate frozen table state) :
    ∃ count : Nat, ∃ outcome : EvaluationTrace.Outcome,
      oracle.evaluate_loop state.cursor candidate frozen state.output
        state.limits control state.statistics =
        ok (outcome.result, outcome.output, outcome.work) ∧
      EvaluationTrace.Trace candidate frozen state count outcome ∧
      EvaluationTrace.Preserved candidate frozen table state count outcome := by
  obtain ⟨count, outcome, calls, _⟩ :=
    calls_exist candidate frozen table state control stored ordered covered invariant
  have traced : EvaluationTrace.Trace candidate frozen state count outcome :=
    calls_are_trace candidate frozen calls
  have preserved : EvaluationTrace.Preserved candidate frozen table state count outcome :=
    EvaluationTrace.trace_refines candidate frozen table stored ordered covered traced invariant
  exact ⟨count, outcome, calls_execute candidate frozen calls, traced, preserved⟩

/-- The actual generated evaluator clears its old output, initializes the actual
source cursor, and returns a trace-correct prefix with exact work accounting.
Initial prefix correctness is proved from setup; no caller-supplied agreement
between old output and formula truth is required. Typed refusals remain distinct
from complete evaluation. The same fixed-token scope as `loop_refines` applies. -/
theorem evaluate_refines (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (oldOutput : alloc.vec.Vec Bool) (work : oracle.Work)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (covered : ∀ mask ∈ frozen, program.value.nodes.val.length ≤ mask.val.length) :
    ∃ count : Nat, ∃ outcome : EvaluationTrace.Outcome,
      oracle.evaluate program candidate frozen oldOutput work =
        ok (outcome.result, outcome.output, outcome.work) ∧
      EvaluationTrace.Trace candidate frozen
        (ReductTrace.initialState program work.limits work.statistics) count outcome ∧
      EvaluationTrace.Preserved candidate frozen program.value.nodes.val
        (ReductTrace.initialState program work.limits work.statistics) count outcome := by
  obtain ⟨count, outcome, executed, traced, preserved⟩ := loop_refines
    candidate frozen program.value.nodes.val
    (ReductTrace.initialState program work.limits work.statistics) work.cancellation
    stored ordered covered
    (ReductTrace.initial_invariant program candidate frozen work.limits work.statistics)
  refine ⟨count, outcome, ?_, traced, preserved⟩
  rw [EvaluatorSetup.evaluate_from_empty]
  exact executed

/-- Whenever the actual generated evaluator returns source success, its complete
output equals the mathematical value fold of the actual source table. This
uses an actual `evaluate` result as premise, not an assumed trace or correct
prefix. A supplied mask is still arbitrary covered data; deriving its original
truth is a separate two-pass composition. -/
theorem completed_values (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (oldOutput : alloc.vec.Vec Bool) (work : oracle.Work)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (covered : ∀ mask ∈ frozen, program.value.nodes.val.length ≤ mask.val.length)
    (output : alloc.vec.Vec Bool) (returned : oracle.Work)
    (completed : oracle.evaluate program candidate frozen oldOutput work =
      ok (core.result.Result.Ok (), output, returned)) :
    output.val = EvaluationSpecification.values candidate frozen program.value.nodes.val := by
  obtain ⟨count, outcome, executed, traced, _⟩ :=
    evaluate_refines program candidate frozen oldOutput work stored ordered covered
  have same : (outcome.result, outcome.output, outcome.work) =
      (core.result.Result.Ok (), output, returned) :=
    Result.ok_injective (executed.symm.trans completed)
  have sourceSuccess : outcome.result = core.result.Result.Ok () :=
    congrArg Prod.fst same
  have sameOutput : outcome.output = output :=
    congrArg (fun result => result.2.1) same
  have exactValues : outcome.output.val =
      EvaluationSpecification.values candidate frozen program.value.nodes.val :=
    EvaluationTrace.completed_values candidate frozen program.value.nodes.val stored
      ordered covered traced
      (ReductTrace.initial_invariant program candidate frozen work.limits work.statistics)
      sourceSuccess
  rw [← sameOutput]
  exact exactValues

end FixedEvaluationLoop
