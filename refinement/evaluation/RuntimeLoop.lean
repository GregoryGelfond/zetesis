import RuntimeRuns

open Aeneas Aeneas.Std Aeneas.Data.Coinductive

/-!
# Projecting completed loops

A successful eventful loop can reuse a fixed-observation proof when each reached
continuation preserves a state invariant and consumes an observation. The latter
is a finite-execution measure, not a fairness or cancellation-delivery premise.
The projection retains the actual intermediate states and final value.
-/
namespace RuntimeLoop

/-- A finite completed loop has a sequence of actual fixed-body calls when its
continuations and accepted exits have the supplied local correspondences.

Proof: unfold one loop step and recover its actual body run and continuation.
A terminal result gives the final fixed call. A continuation consumes a nonempty
prefix of the event history; apply induction to the shorter remaining history.
The proof does not assume that either loop terminates on arbitrary input. -/
theorem completed_calls {State Value : Type}
    (body : State → RuntimeEffects.Computation (ControlFlow State Value))
    (fixed : State → Result (ControlFlow State Value))
    (invariant : State → Prop) (accepted : Value → Prop)
    (continues : ∀ state events next, invariant state →
      RuntimeEffects.Runs (body state) events (.cont next) →
      fixed state = Result.ok (.cont next) ∧ invariant next ∧ events ≠ [])
    (finishes : ∀ state events value, invariant state → accepted value →
      RuntimeEffects.Runs (body state) events (.done value) →
      fixed state = Result.ok (.done value))
    (state : State) (events : List RuntimeEffects.Event) (value : Value)
    (initial : invariant state) (final : accepted value)
    (run : RuntimeEffects.Runs (RuntimeEffects.loop body state) events value) :
    RuntimeEffects.PureCalls fixed state value := by
  generalize size : events.length = count
  induction count using Nat.strong_induction_on generalizing state events with
  | h count inductionHypothesis =>
      rw [RuntimeEffects.loop] at run
      obtain ⟨before, after, transition, partition, step, following⟩ :=
        RuntimeRuns.bind_inv _ _ _ _ run
      cases transition with
      | done returned =>
          obtain ⟨same, _⟩ := RuntimeRuns.returned_inv _ _ _ following
          subst returned
          exact .done (finishes state before value initial final step)
      | cont next =>
          obtain ⟨projected, retained, consumed⟩ :=
            continues state before next initial step
          have smaller : after.length < count := by
            rw [partition, List.length_append] at size
            have positive : 0 < before.length := by
              cases before with
              | nil => exact False.elim (consumed rfl)
              | cons event rest => simp only [List.length_cons]; omega
            omega
          have remaining : RuntimeEffects.PureCalls fixed next value :=
            inductionHypothesis after.length smaller next after retained following rfl
          exact .next projected remaining

/-- The projected finite body calls are an actual completed fixed loop with the
same result. All local correspondence and progress conditions remain explicit. -/
theorem completed_loop {State Value : Type}
    (body : State → RuntimeEffects.Computation (ControlFlow State Value))
    (fixed : State → Result (ControlFlow State Value))
    (invariant : State → Prop) (accepted : Value → Prop)
    (continues : ∀ state events next, invariant state →
      RuntimeEffects.Runs (body state) events (.cont next) →
      fixed state = Result.ok (.cont next) ∧ invariant next ∧ events ≠ [])
    (finishes : ∀ state events value, invariant state → accepted value →
      RuntimeEffects.Runs (body state) events (.done value) →
      fixed state = Result.ok (.done value))
    (state : State) (events : List RuntimeEffects.Event) (value : Value)
    (initial : invariant state) (final : accepted value)
    (run : RuntimeEffects.Runs (RuntimeEffects.loop body state) events value) :
    Aeneas.Std.loop fixed state = Result.ok value := by
  have calls : RuntimeEffects.PureCalls fixed state value :=
    completed_calls body fixed invariant accepted continues finishes state events
      value initial final run
  exact (RuntimeEffects.pure_loop_compatible fixed calls).1

/-- A completed event loop retains a postcondition established at its accepted
exit when each continuation preserves the state invariant and consumes an event.
This framing law does not depend on a fixed-observation interpretation. -/
theorem completed_post {State Value : Type}
    (body : State → RuntimeEffects.Computation (ControlFlow State Value))
    (invariant : State → Prop) (accepted postcondition : Value → Prop)
    (continues : ∀ state events next, invariant state →
      RuntimeEffects.Runs (body state) events (.cont next) →
      invariant next ∧ events ≠ [])
    (finishes : ∀ state events value, invariant state → accepted value →
      RuntimeEffects.Runs (body state) events (.done value) → postcondition value)
    (state : State) (events : List RuntimeEffects.Event) (value : Value)
    (initial : invariant state) (final : accepted value)
    (run : RuntimeEffects.Runs (RuntimeEffects.loop body state) events value) :
    postcondition value := by
  generalize size : events.length = count
  induction count using Nat.strong_induction_on generalizing state events with
  | h count inductionHypothesis =>
      rw [RuntimeEffects.loop] at run
      obtain ⟨before, after, transition, partition, step, following⟩ :=
        RuntimeRuns.bind_inv _ _ _ _ run
      cases transition with
      | done returned =>
          obtain ⟨same, _⟩ := RuntimeRuns.returned_inv _ _ _ following
          subst returned
          exact finishes state before value initial final step
      | cont next =>
          obtain ⟨retained, consumed⟩ := continues state before next initial step
          have smaller : after.length < count := by
            rw [partition, List.length_append] at size
            have positive : 0 < before.length := by
              cases before with
              | nil => exact False.elim (consumed rfl)
              | cons event rest => simp only [List.length_cons]; omega
            omega
          exact inductionHypothesis after.length smaller next after retained following rfl

end RuntimeLoop
