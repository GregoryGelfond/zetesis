import RuntimeRuns

open Aeneas Aeneas.Std Aeneas.Data.Coinductive
open RuntimeEffects

/-!
# The reached origin of a returned loop result

A finite returning execution is separated into its actually executed
continuations and its terminal body call. Prefix reachability records each
intermediate state and the exact ordered observations; it does not permit a
terminal body at an arbitrary state. The progress premise is explicit: every
continuation consumes an observation. The reference checker's ticked loops
satisfy it independently of storage or semantic invariants.
-/
namespace RuntimeLoopOrigin

/-- An executed loop prefix contains only actual continuation results, preserving
all states and observation order. A zero-step prefix reaches its starting state.
The final done invocation is deliberately kept separate. -/
inductive Prefix {State Value : Type}
    (body : State → Computation (ControlFlow State Value)) :
    State → List Event → State → Prop
  | start (state : State) : Prefix body state [] state
  | step {state next reached : State} {before after : List Event}
      (called : Runs (body state) before (.cont next))
      (rest : Prefix body next after reached) :
      Prefix body state (before ++ after) reached

/-- Every finite returned loop result originates in a done body call at an
actually reached state. Its history splits exactly into the continuation prefix
and the terminal call. No condition is placed on the returned value, so the law
also preserves a typed refusal and its mutated state.

Proof: unfold one loop iteration and invert its bind. A done transition is the
terminal witness. A continuation consumes a nonempty prefix; apply induction to
the strictly shorter remaining history and prepend that actual call. -/
theorem completed_origin {State Value : Type}
    (body : State → Computation (ControlFlow State Value))
    (progress : ∀ state events next, Runs (body state) events (.cont next) → events ≠ [])
    (state : State) (events : List Event) (value : Value)
    (run : Runs (RuntimeEffects.loop body state) events value) :
    ∃ before last reached, events = before ++ last ∧
      Prefix body state before reached ∧ Runs (body reached) last (.done value) := by
  generalize size : events.length = count
  induction count using Nat.strong_induction_on generalizing state events with
  | h count inductionHypothesis =>
    rw [RuntimeEffects.loop] at run
    obtain ⟨before, after, transition, partition, step, following⟩ :=
      RuntimeRuns.bind_inv _ _ _ _ run
    cases transition with
    | done result =>
      obtain ⟨same, empty⟩ := RuntimeRuns.returned_inv _ _ _ following
      subst result
      have history : events = before := by simpa only [empty, List.append_nil] using partition
      exact ⟨[], before, state, history, .start state, step⟩
    | cont next =>
      have smaller : after.length < count := by
        rw [partition, List.length_append] at size
        have positive : 0 < before.length := by
          cases before with
          | nil => exact False.elim (progress state [] next step rfl)
          | cons event rest => simp only [List.length_cons]; omega
        omega
      obtain ⟨middle, last, reached, remainder, prefixRun, terminal⟩ :=
        inductionHypothesis after.length smaller next after following rfl
      refine ⟨before ++ middle, last, reached, ?_, .step step prefixRun, terminal⟩
      rw [partition, remainder, List.append_assoc]

end RuntimeLoopOrigin
