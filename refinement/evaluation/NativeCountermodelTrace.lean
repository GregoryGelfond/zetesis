import NativeSearchSteps

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisNativeExtract

/-!
# Actual calls in the countermodel-search loop

A finite call sequence records the generated search's exact branches: exhausted
population guard, typed query stop, witness, typed carry stop, or continuation.
Every query and carry premise is an actual helper return equation. Intermediate
work and partial mutations are retained; no semantic oracle agreement is supplied.

The execution theorem unfolds the actual backend loop. Constructing such a
sequence from admitted input, relating the population to a counter rank, and
proving proper-subset coverage are separate obligations. These laws retain the
existing fixed-observation, scalar and sequence-model boundaries.
-/
namespace NativeCountermodelTrace

/-- The fields passed to an actual search-loop invocation. Population is a
    separate machine value; this record alone does not prove that it counts the
    current packed subset or that the selected coordinates cover a candidate. -/
structure State where
  subset : theory.Interpretation
  values : alloc.vec.Vec Bool
  work : oracle.Work
  present : Usize

/-- The actual loop's returned fields. Its done result discards the population;
    carry-stop constructors below retain that value in their helper evidence. -/
structure Outcome where
  subset : theory.Interpretation
  values : alloc.vec.Vec Bool
  work : oracle.Work
  result : core.result.Result Bool zetesis_cpu.cancellation.Stop

/-- A finite sequence of the actual query/carry calls and population guards.
    Only a false completed query and successful carry recurse, using every field
    returned by those calls. A typed stop is never a completed false verdict. -/
inductive Calls (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Usize) :
    State → Outcome → Prop where
  | exhausted (state : State) (full : selected.val.length ≤ state.present.val) :
      Calls program frozen selected state ⟨state.subset, state.values, state.work, .Ok false⟩
  | queryStopped (state : State) (output : alloc.vec.Vec Bool) (after : oracle.Work)
      (reason : zetesis_cpu.cancellation.Stop) (proper : state.present.val < selected.val.length)
      (queried : oracle.check_subset program state.subset frozen state.values state.work =
        ok (.Err reason, output, after)) :
      Calls program frozen selected state ⟨state.subset, output, after, .Err reason⟩
  | witness (state : State) (output : alloc.vec.Vec Bool) (after : oracle.Work)
      (proper : state.present.val < selected.val.length)
      (queried : oracle.check_subset program state.subset frozen state.values state.work =
        ok (.Ok true, output, after)) :
      Calls program frozen selected state ⟨state.subset, output, after, .Ok true⟩
  | carryStopped (state : State) (output : alloc.vec.Vec Bool) (middle : oracle.Work)
      (interrupted : theory.Interpretation) (partialCount : Usize) (after : oracle.Work)
      (reason : zetesis_cpu.cancellation.Stop) (proper : state.present.val < selected.val.length)
      (queried : oracle.check_subset program state.subset frozen state.values state.work =
        ok (.Ok false, output, middle))
      (carried : oracle.advance_subset selected state.subset state.present middle =
        ok (.Err reason, interrupted, partialCount, after)) :
      Calls program frozen selected state ⟨interrupted, output, after, .Err reason⟩
  | next (state nextState : State) (middle : oracle.Work) (outcome : Outcome)
      (proper : state.present.val < selected.val.length)
      (queried : oracle.check_subset program state.subset frozen state.values state.work =
        ok (.Ok false, nextState.values, middle))
      (carried : oracle.advance_subset selected state.subset state.present middle =
        ok (.Ok (), nextState.subset, nextState.present, nextState.work))
      (rest : Calls program frozen selected nextState outcome) :
      Calls program frozen selected state outcome

/-- One unfolding of the actual generated loop either returns its body's done
    fields or recurses with the precise returned interpretation, values, work and
    population. It introduces no refreshed controls or substitute operation. -/
theorem loop_unfold (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Usize)
    (state : State) :
    oracle.find_countermodel_loop program frozen selected state.subset state.values
      state.work state.present = (do
        let transition ← oracle.find_countermodel_loop.body program frozen selected
          state.subset state.values state.work state.present
        match transition with
        | .done result => ok result
        | .cont (subset, values, work, present) =>
          oracle.find_countermodel_loop program frozen selected subset values work present) := by
  rw [oracle.find_countermodel_loop, loop]
  dsimp only [uncurry]
  congr 1
  funext transition
  cases transition with
  | done result => rfl
  | cont result =>
      obtain ⟨subset, values, work, present⟩ := result
      rfl

/-- Any finite call sequence executes the actual generated search loop with its
    recorded outcome. The sequence contains operational helper returns, not an
    assumed search-result equation or semantic correctness of those helpers.

    Proof: apply the generated-body branch law for each constructor. Terminal
    branches finish on this unfolding; continuation substitutes the actual next
    state and applies the induction hypothesis to that shorter call sequence. -/
theorem calls_execute (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Usize)
    {state : State} {outcome : Outcome} (calls : Calls program frozen selected state outcome) :
    oracle.find_countermodel_loop program frozen selected state.subset state.values
      state.work state.present = ok (outcome.subset, outcome.values, outcome.work, outcome.result) := by
  induction calls with
  | exhausted state full =>
      rw [loop_unfold, NativeCountermodelSteps.exhausted program frozen selected state.subset
        state.values state.work state.present full]
      simp only [bind_tc_ok]
  | queryStopped state output after reason proper queried =>
      rw [loop_unfold, NativeCountermodelSteps.query_stopped program frozen selected state.subset
        state.values output state.work after state.present reason proper queried]
      simp only [bind_tc_ok]
  | witness state output after proper queried =>
      rw [loop_unfold, NativeCountermodelSteps.witness program frozen selected state.subset
        state.values output state.work after state.present proper queried]
      simp only [bind_tc_ok]
  | carryStopped state output middle interrupted partialCount after reason proper queried carried =>
      rw [loop_unfold, NativeCountermodelSteps.carry_stopped program frozen selected state.subset
        interrupted state.values output state.work middle after state.present partialCount
        reason proper queried carried]
      simp only [bind_tc_ok]
  | next state nextState middle outcome proper queried carried rest ih =>
      rw [loop_unfold, NativeCountermodelSteps.continued program frozen selected state.subset
        nextState.subset state.values nextState.values state.work middle nextState.work
        state.present nextState.present proper queried carried]
      simpa only [bind_tc_ok] using ih

/-- The actual entry sets population to zero and reorders the loop's returned
    tuple. A call sequence from that exact initial state therefore determines
    the actual helper return. An empty interpretation is not established by this
    wrapper law; the caller establishes its counter invariant separately. -/
theorem entry_executes (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Usize)
    (subset : theory.Interpretation) (values : alloc.vec.Vec Bool) (work : oracle.Work)
    (outcome : Outcome)
    (calls : Calls program frozen selected ⟨subset, values, work, 0#usize⟩ outcome) :
    oracle.find_countermodel program frozen selected subset values work =
      ok (outcome.result, outcome.subset, outcome.values, outcome.work) := by
  have executed : oracle.find_countermodel_loop program frozen selected subset values work
      0#usize = ok (outcome.subset, outcome.values, outcome.work, outcome.result) := by
    exact calls_execute program frozen selected calls
  simp [oracle.find_countermodel, executed]

end NativeCountermodelTrace
