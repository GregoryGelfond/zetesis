import Specification
import Progress

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisExtract

/-!
# Finite traces of actual generated evaluator invocations

A trace records calls to `oracle.evaluate_loop.body` itself. Each invocation has
its own externally supplied entire control value. The relation imposes no fixed
runtime control-object identity or observation history. No equality to the generated
whole-loop function or temporal law for Rust atomics is asserted.

The trace count records successful continuation steps only. An exhausted call
and a typed stop each finish the trace without increasing that count. An outer
backend failure is not represented as either a completed result or a typed stop.
-/
namespace EvaluationTrace

/-- State carried between evaluator invocations. Control is deliberately absent:
the next invocation supplies its own observations. -/
structure State where
  cursor : Evaluation.Cursor
  output : alloc.vec.Vec Bool
  limits : oracle.Limits
  statistics : oracle.Statistics

/-- The exact typed result and data returned by a finishing generated invocation.
An error result remains distinct from successful exhaustion. -/
structure Outcome where
  result : core.result.Result Unit zetesis_cpu.cancellation.Stop
  output : alloc.vec.Vec Bool
  work : oracle.Work

/-- A finite sequence of actual generated body calls. A continuation carries the
returned data forward while permitting fresh control observations for the next
invocation. Its count increases once per successful continuation only. -/
inductive Trace (candidate : theory.Interpretation) (frozen : Option (Slice Bool)) :
    State → Nat → Outcome → Prop where
  | finish {state : State} {outcome : Outcome}
      (control : zetesis_cpu.cancellation.Cancellation)
      (step : oracle.evaluate_loop.body candidate frozen state.cursor state.output
        state.limits control state.statistics =
        ok (.done (outcome.result, outcome.output, outcome.work))) :
      Trace candidate frozen state 0 outcome
  | next {state nextState : State} {count : Nat} {outcome : Outcome}
      (control returnedControl : zetesis_cpu.cancellation.Cancellation)
      (step : oracle.evaluate_loop.body candidate frozen state.cursor state.output
        state.limits control state.statistics =
        ok (.cont (nextState.cursor, nextState.output, nextState.limits,
          returnedControl, nextState.statistics)))
      (rest : Trace candidate frozen nextState count outcome) :
      Trace candidate frozen state (count + 1) outcome

/-- The generated cursor addresses a fixed table, is within its bounds and is
synchronized with its enumeration count. Its stored output is the mathematical
truth prefix of precisely those visited nodes. -/
structure Invariant (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.Node) (state : State) : Prop where
  source : state.cursor.iter.slice.val = table
  bounded : state.cursor.iter.i ≤ table.length
  counter : state.cursor.count.val = state.cursor.iter.i
  values : state.output.val = EvaluationSpecification.prefixValues
    candidate frozen table state.cursor.iter.i

/-- A continuation preserves the truth invariant, advances one position and
charges one unit while retaining limits and the subset counter. -/
def Continued (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.Node) (state nextState : State) : Prop :=
  Invariant candidate frozen table nextState ∧
  nextState.cursor.iter.i = state.cursor.iter.i + 1 ∧
  nextState.limits = state.limits ∧
  nextState.statistics.subsets = state.statistics.subsets ∧
  nextState.statistics.work.val = state.statistics.work.val + 1

/-- A finishing call preserves the current prefix and complete work counters.
Successful exhaustion is possible only at the table's end. A typed stop occurs
strictly before that end and remains an error result. -/
def Finished (table : List theory.Node) (state : State) (outcome : Outcome) : Prop :=
  outcome.output = state.output ∧ outcome.work.limits = state.limits ∧
  outcome.work.statistics = state.statistics ∧
  match outcome.result with
  | .Ok _ => state.cursor.iter.i = table.length
  | .Err _ => state.cursor.iter.i < table.length

/-- The precise state consequences of either generated transition constructor. -/
def Effect (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.Node) (state : State) : Evaluation.Transition → Prop
  | .cont (cursor, output, limits, _, statistics) =>
    Continued candidate frozen table state ⟨cursor, output, limits, statistics⟩
  | .done (result, output, work) => Finished table state ⟨result, output, work⟩

/-- Every admitted single invocation has an actual generated result satisfying
the state invariant or the precise exhaustion/stop boundary. Fresh observations
are supplied as data for this invocation. Success and refusal are derived from
the generated body; no step-result equation is assumed.

Proof: exhaustion performs no tick. For a present node, distinguish the observed
control stop, exhausted allowance, and admitted tick. In the last case combine
the concrete prefix append with the specification's next-prefix law. -/
theorem invocation_refines (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (table : List theory.Node) (state : State)
    (control : zetesis_cpu.cancellation.Cancellation)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered table)
    (covered : ∀ mask ∈ frozen, table.length ≤ mask.val.length)
    (invariant : Invariant candidate frozen table state) :
    ∃ transition : Evaluation.Transition,
      oracle.evaluate_loop.body candidate frozen state.cursor state.output
        state.limits control state.statistics = ok transition ∧
      Effect candidate frozen table state transition := by
  by_cases inside : state.cursor.iter.i < table.length
  · have sliceInside : state.cursor.iter.i < state.cursor.iter.slice.val.length := by
      rw [invariant.source]
      exact inside
    cases observed : EvaluatorControl.observation control with
    | some reason =>
      have stopped : oracle.evaluate_loop.body candidate frozen state.cursor
          state.output state.limits control state.statistics =
          ok (.done (.Err reason, state.output,
            ⟨state.limits, control, state.statistics⟩)) :=
        EvaluationProgress.control_stops candidate frozen state.cursor
          state.output state.limits control state.statistics reason
          invariant.counter sliceInside observed
      refine ⟨.done (.Err reason, state.output,
        ⟨state.limits, control, state.statistics⟩), stopped, ?_⟩
      exact ⟨rfl, rfl, rfl, inside⟩
    | none =>
      by_cases remaining : state.statistics.work.val < state.limits.max_work.val
      · have prefixLength : state.output.val.length = state.cursor.iter.i := by
          rw [invariant.values]
          exact EvaluationSpecification.prefix_length candidate frozen table
            state.cursor.iter.i invariant.bounded
        have children : Evaluation.ChildrenPresent state.output.val
            state.cursor.iter.slice.val[state.cursor.iter.i] := by
          simpa only [invariant.source] using
            (EvaluationSpecification.children_present table[state.cursor.iter.i]
              state.output.val state.cursor.iter.i prefixLength
              (ordered state.cursor.iter.i inside))
        have maskCovers : ∀ mask ∈ frozen,
            state.cursor.iter.slice.val.length ≤ mask.val.length := by
          simpa only [invariant.source] using covered
        obtain ⟨nextCount, nextWork, extended, _, workAdvanced, advanced,
            exactPrefix, prefixAdvanced, synchronized⟩ :=
          EvaluationProgress.present_step candidate frozen state.cursor state.output
            state.limits control state.statistics invariant.counter prefixLength
            sliceInside stored children maskCovers observed remaining
        have nextValues : extended.val = EvaluationSpecification.prefixValues
            candidate frozen table (state.cursor.iter.i + 1) := by
          rw [EvaluationSpecification.prefix_succ candidate frozen table
            state.cursor.iter.i inside]
          simpa only [EvaluationSpecification.masked_exact, invariant.counter,
            invariant.values, invariant.source] using exactPrefix
        refine ⟨.cont ({ iter := { state.cursor.iter with i := state.cursor.iter.i + 1 }, count := nextCount },
          extended, state.limits, control,
          { state.statistics with work := nextWork }), advanced, ?_⟩
        refine ⟨?_, rfl, rfl, rfl, workAdvanced⟩
        refine ⟨invariant.source, Nat.succ_le_of_lt inside, ?_, nextValues⟩
        exact synchronized.trans prefixAdvanced
      · have exhausted : state.limits.max_work.val ≤ state.statistics.work.val :=
          Nat.le_of_not_gt remaining
        have stopped : oracle.evaluate_loop.body candidate frozen state.cursor
            state.output state.limits control state.statistics =
            ok (.done (.Err .WorkLimit, state.output,
              ⟨state.limits, control, state.statistics⟩)) :=
          EvaluationProgress.work_limit_stops candidate frozen state.cursor
            state.output state.limits control state.statistics
            invariant.counter sliceInside observed exhausted
        refine ⟨.done (.Err .WorkLimit, state.output,
          ⟨state.limits, control, state.statistics⟩), stopped, ?_⟩
        exact ⟨rfl, rfl, rfl, inside⟩
  · have exhausted : state.cursor.iter.slice.val.length ≤ state.cursor.iter.i := by
      rw [invariant.source]
      exact Nat.le_of_not_gt inside
    have atEnd : state.cursor.iter.i = table.length := by
      have within : state.cursor.iter.i ≤ table.length := invariant.bounded
      omega
    have finished : oracle.evaluate_loop.body candidate frozen state.cursor
        state.output state.limits control state.statistics =
        ok (.done (.Ok (), state.output,
          ⟨state.limits, control, state.statistics⟩)) :=
      Evaluation.exhausted_step candidate frozen state.cursor
        state.output state.limits control state.statistics exhausted
    refine ⟨.done (.Ok (), state.output,
      ⟨state.limits, control, state.statistics⟩), finished, ?_⟩
    exact ⟨rfl, rfl, rfl, atEnd⟩

/-- A finite trace retains the exact visited truth prefix and charges one work
unit per continuation. Its typed result distinguishes a full visit from a
strictly shorter stopped visit. No temporal relation between control inputs is
part of this record. -/
structure Preserved (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (table : List theory.Node) (state : State) (count : Nat) (outcome : Outcome) : Prop where
  values : outcome.output.val = EvaluationSpecification.prefixValues
    candidate frozen table (state.cursor.iter.i + count)
  limits : outcome.work.limits = state.limits
  subsets : outcome.work.statistics.subsets = state.statistics.subsets
  work : outcome.work.statistics.work.val = state.statistics.work.val + count
  bounded : state.cursor.iter.i + count ≤ table.length
  boundary : match outcome.result with
    | .Ok _ => state.cursor.iter.i + count = table.length
    | .Err _ => state.cursor.iter.i + count < table.length

/-- A finite trace of actual generated invocations preserves exact truth prefixes
and exact work accounting, even when successive invocations receive different
external control values. Only continuation steps contribute to the count.

Proof: classify the first actual body result with `invocation_refines`. A finish
preserves the current state. A continuation supplies the next invariant and one
work increment; apply induction to the remaining actual calls and add the counts.
The typed final result retains exhaustion versus refusal throughout. -/
theorem trace_refines (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (table : List theory.Node)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered table)
    (covered : ∀ mask ∈ frozen, table.length ≤ mask.val.length)
    {state : State} {count : Nat} {outcome : Outcome}
    (trace : Trace candidate frozen state count outcome)
    (invariant : Invariant candidate frozen table state) :
    Preserved candidate frozen table state count outcome := by
  revert invariant
  induction trace with
  | @finish state outcome control step =>
    intro invariant
    obtain ⟨transition, actual, effect⟩ := invocation_refines candidate frozen
      table state control stored ordered covered invariant
    have same : transition = .done (outcome.result, outcome.output, outcome.work) := by
      rw [step] at actual
      exact (Result.ok_injective actual).symm
    rw [same] at effect
    change Finished table state outcome at effect
    dsimp only [Finished] at effect
    obtain ⟨sameOutput, sameLimits, sameStatistics, boundary⟩ := effect
    refine ⟨?_, sameLimits, ?_, ?_, ?_, ?_⟩
    · simpa only [Nat.add_zero, sameOutput] using invariant.values
    · rw [sameStatistics]
    · simp only [sameStatistics, Nat.add_zero]
    · simpa only [Nat.add_zero] using invariant.bounded
    · simpa only [Nat.add_zero] using boundary
  | @next state nextState count outcome control returnedControl step rest ih =>
    intro invariant
    obtain ⟨transition, actual, effect⟩ := invocation_refines candidate frozen
      table state control stored ordered covered invariant
    have same : transition = .cont (nextState.cursor, nextState.output,
        nextState.limits, returnedControl, nextState.statistics) := by
      rw [step] at actual
      exact (Result.ok_injective actual).symm
    rw [same] at effect
    change Continued candidate frozen table state nextState at effect
    dsimp only [Continued] at effect
    obtain ⟨nextInvariant, advanced, sameLimits, sameSubsets, charged⟩ := effect
    have remaining : Preserved candidate frozen table nextState count outcome :=
      ih nextInvariant
    have positions : nextState.cursor.iter.i + count =
        state.cursor.iter.i + (count + 1) := by
      omega
    refine ⟨?_, remaining.limits.trans sameLimits,
      remaining.subsets.trans sameSubsets, ?_, ?_, ?_⟩
    · simpa only [positions] using remaining.values
    · rw [remaining.work, charged]
      omega
    · simpa only [positions] using remaining.bounded
    · simpa only [positions] using remaining.boundary

/-- A successfully exhausted actual trace returns the complete mathematical
value sequence. The initial state may already hold a proved prefix; in particular
this law applies to the empty prefix supplied by generated evaluator setup. -/
theorem completed_values (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (table : List theory.Node)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered table)
    (covered : ∀ mask ∈ frozen, table.length ≤ mask.val.length)
    {state : State} {count : Nat} {outcome : Outcome}
    (trace : Trace candidate frozen state count outcome)
    (invariant : Invariant candidate frozen table state)
    (completed : outcome.result = .Ok ()) :
    outcome.output.val = EvaluationSpecification.values candidate frozen table := by
  have preserved : Preserved candidate frozen table state count outcome :=
    trace_refines candidate frozen table stored ordered covered trace invariant
  have exhausted : state.cursor.iter.i + count = table.length := by
    simpa only [completed] using preserved.boundary
  rw [preserved.values, exhausted]
  exact EvaluationSpecification.prefix_complete candidate frozen table

/-- A typed stop returns fewer values than the full table, so its preserved
prefix cannot be mistaken for complete evaluation. This concerns prefix extent;
it does not reconstruct the stop reason from a history of control objects. -/
theorem stopped_prefix_shorter (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (table : List theory.Node)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered table)
    (covered : ∀ mask ∈ frozen, table.length ≤ mask.val.length)
    {state : State} {count : Nat} {outcome : Outcome}
    (trace : Trace candidate frozen state count outcome)
    (invariant : Invariant candidate frozen table state)
    (reason : zetesis_cpu.cancellation.Stop)
    (stopped : outcome.result = .Err reason) :
    outcome.output.val.length < table.length := by
  have preserved : Preserved candidate frozen table state count outcome :=
    trace_refines candidate frozen table stored ordered covered trace invariant
  have beforeEnd : state.cursor.iter.i + count < table.length := by
    simpa only [stopped] using preserved.boundary
  rw [preserved.values, EvaluationSpecification.prefix_length candidate frozen table
    (state.cursor.iter.i + count) preserved.bounded]
  exact beforeEnd

/-- Admitted state has a finite trace witness using no more successful
continuations than unvisited nodes. The construction supplies the given control
value afresh at each invocation; it is one possible witness in this abstract
trace relation, not a claim that a runtime control object keeps fixed values.
Universal trace soundness is supplied separately by `trace_refines`.

Proof: classify the actual invocation. A done result immediately gives a trace
of count zero. A continuation increases the bounded cursor by one, strictly
reducing the number of unvisited nodes; recursively construct the remaining
trace and add one continuation. Thus the trace population is constructively
inhabited, without an assumed terminating result or whole-loop equation. -/
theorem trace_exists (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (table : List theory.Node)
    (control : zetesis_cpu.cancellation.Cancellation)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered table)
    (covered : ∀ mask ∈ frozen, table.length ≤ mask.val.length)
    (state : State) (invariant : Invariant candidate frozen table state) :
    ∃ count : Nat, ∃ outcome : Outcome,
      Trace candidate frozen state count outcome ∧
      count ≤ table.length - state.cursor.iter.i := by
  have construct (remaining : Nat) :
      ∀ current : State, Invariant candidate frozen table current →
        table.length - current.cursor.iter.i = remaining →
        ∃ count : Nat, ∃ outcome : Outcome,
          Trace candidate frozen current count outcome ∧ count ≤ remaining := by
    induction remaining using Nat.strong_induction_on with
    | h remaining ih =>
      intro current currentInvariant unvisited
      obtain ⟨transition, actual, effect⟩ := invocation_refines candidate frozen
        table current control stored ordered covered currentInvariant
      cases transition with
      | done result =>
        obtain ⟨result, output, work⟩ := result
        exact ⟨0, ⟨result, output, work⟩, Trace.finish control actual, Nat.zero_le remaining⟩
      | cont result =>
        obtain ⟨cursor, output, limits, returnedControl, statistics⟩ := result
        let nextState : State := ⟨cursor, output, limits, statistics⟩
        change Continued candidate frozen table current nextState at effect
        obtain ⟨nextInvariant, advanced, _, _, _⟩ := effect
        have lessUnvisited : table.length - nextState.cursor.iter.i < remaining := by
          have within : nextState.cursor.iter.i ≤ table.length := nextInvariant.bounded
          omega
        obtain ⟨count, outcome, rest, bounded⟩ := ih
          (table.length - nextState.cursor.iter.i) lessUnvisited nextState nextInvariant rfl
        refine ⟨count + 1, outcome, Trace.next control returnedControl actual rest, ?_⟩
        have within : nextState.cursor.iter.i ≤ table.length := nextInvariant.bounded
        omega
  exact construct (table.length - state.cursor.iter.i) state invariant rfl

end EvaluationTrace
