import RuntimeEffects

open Aeneas Aeneas.Std Aeneas.Data.Coinductive

/-!
# Decomposing finite executions with returning observations

A completed sequential execution consists of the first operation's finite
execution followed by its actual continuation. An embedded backend operation
consumes no runtime observations and completes only with its actual value.
These laws expose calls rather than assume their outcomes. They concern the
explicit event model; source correspondence is a separate obligation.
-/
namespace RuntimeRuns

/-- A returned computation consumes no observations and retains its value. -/
theorem returned_inv {A : Type} (value result : A) (events : List RuntimeEffects.Event)
    (run : RuntimeEffects.Runs (ITree.ret value) events result) :
    value = result ∧ events = [] := by
  generalize actual : ITree.ret (E := RuntimeEffects.effects) value = operation at run
  cases run with
  | returned other => exact ⟨ret_inj.mp actual, rfl⟩
  | observed request response next tail result rest =>
      exact False.elim (not_vis_ret actual)

/-- Divergence has no finite returned execution. -/
theorem diverged_not_run {A : Type} (events : List RuntimeEffects.Event) (value : A) :
    ¬ RuntimeEffects.Runs ITree.div events value := by
  intro run
  generalize actual : (ITree.div : RuntimeEffects.Computation A) = operation at run
  cases run with
  | returned other => exact not_ret_div actual.symm
  | observed request response next tail result rest => exact not_div_vis actual

/-- A visible request consumes its matching response before its continuation.
The dependent response type is preserved; a nonreturning failure admits none. -/
theorem observed_inv {A : Type} (request : RuntimeEffects.Request)
    (next : RuntimeEffects.Response request → RuntimeEffects.Computation A)
    (events : List RuntimeEffects.Event) (result : A)
    (run : RuntimeEffects.Runs (ITree.vis request next) events result) :
    ∃ response tail, events = ⟨request, response⟩ :: tail ∧
      RuntimeEffects.Runs (next response) tail result := by
  generalize actual : ITree.vis (E := RuntimeEffects.effects) request next = operation at run
  cases run with
  | returned value => exact False.elim (not_vis_ret actual.symm)
  | observed other response continuation tail value rest =>
      have sameRequest : request = other := vis_inj_effect actual
      subst other
      have unfolded := congrArg ITree.unfold actual
      simp only [unfold_vis] at unfolded
      have sameNext : next = continuation := by
        cases unfolded
        rfl
      subst continuation
      exact ⟨response, tail, rfl, rest⟩

/-- Completion of a bind identifies the actual intermediate value and partitions
its observations in source order. Neither component's completion is assumed.

Proof: inspect the first computation. Return transfers control immediately;
divergence has no finite run. A visible request consumes the first observation;
apply induction to the remaining observations and retain that response in the
first operation's prefix. -/
theorem bind_inv {A B : Type} (first : RuntimeEffects.Computation A)
    (next : A → RuntimeEffects.Computation B) (events : List RuntimeEffects.Event)
    (result : B) (run : RuntimeEffects.Runs (ITree.bind first next) events result) :
    ∃ before after value,
      events = before ++ after ∧ RuntimeEffects.Runs first before value ∧
        RuntimeEffects.Runs (next value) after result := by
  induction events generalizing first with
  | nil =>
      cases first using ITree.cases with
      | ret value =>
          exact ⟨[], [], value, rfl, .returned value, by
            change RuntimeEffects.Runs (ITree.bind (ITree.ret value) next) _ _ at run
            simpa only [itree_ret_bind] using run⟩
      | div =>
          rw [itree_div_bind] at run
          exact False.elim (diverged_not_run _ _ run)
      | vis request continuation =>
          rw [itree_vis_bind] at run
          obtain ⟨response, tail, impossible, _⟩ := observed_inv _ _ _ _ run
          cases impossible
  | cons event events inductionHypothesis =>
      cases first using ITree.cases with
      | ret value =>
          exact ⟨[], event :: events, value, rfl, .returned value, by
            change RuntimeEffects.Runs (ITree.bind (ITree.ret value) next) _ _ at run
            simpa only [itree_ret_bind] using run⟩
      | div =>
          rw [itree_div_bind] at run
          exact False.elim (diverged_not_run _ _ run)
      | vis request continuation =>
          rw [itree_vis_bind] at run
          obtain ⟨response, tail, partition, following⟩ := observed_inv _ _ _ _ run
          have sameTail : events = tail := List.cons.inj partition |>.2
          have sameEvent : event = ⟨request, response⟩ := List.cons.inj partition |>.1
          subst tail
          obtain ⟨before, after, value, split, initial, rest⟩ :=
            inductionHypothesis _ following
          refine ⟨⟨request, response⟩ :: before, after, value, ?_, ?_, rest⟩
          · simp only [List.cons_append, split, sameEvent]
          · exact RuntimeEffects.Runs.observed request response _ before value initial

/-- An embedded backend call completes exactly when that call returns the same
value, consuming no runtime observation. Failure and divergence have no completed
run; the backend failure event admits no response. -/
theorem embed_iff {A : Type} (operation : Result A)
    (events : List RuntimeEffects.Event) (value : A) :
    RuntimeEffects.Runs (RuntimeEffects.embed operation) events value ↔
      operation = Result.ok value ∧ events = [] := by
  cases operation with
  | ret actual =>
      rw [RuntimeEffects.embed_ok]
      constructor
      · intro run
        obtain ⟨same, vacant⟩ := returned_inv _ _ _ run
        exact ⟨congrArg Result.ok same, vacant⟩
      · rintro ⟨same, rfl⟩
        have retained : actual = value := Result.ok_injective same
        subst actual
        exact .returned value
  | div =>
      rw [RuntimeEffects.embed_divergence]
      constructor
      · intro run
        exact False.elim (diverged_not_run _ _ run)
      · rintro ⟨impossible, _⟩
        exact False.elim (Aeneas.Std.div_not_ok impossible)
  | vis request continuation =>
      cases request with
      | fail error =>
          constructor
          · intro run
            simp only [RuntimeEffects.embed, Result.match.vis] at run
            obtain ⟨response, _, _, _⟩ := observed_inv _ _ _ _ run
            cases response
          · rintro ⟨impossible, _⟩
            exact False.elim (Aeneas.Std.vis_not_ok impossible)

/-- A completed embedded call followed by an eventful continuation exposes the
actual backend return. The embedded prefix consumes no observations, so the
continuation receives the complete original history. -/
theorem embedded_bind_inv {A B : Type} (first : Result A)
    (next : A → RuntimeEffects.Computation B) (events : List RuntimeEffects.Event)
    (result : B)
    (run : RuntimeEffects.Runs (ITree.bind (RuntimeEffects.embed first) next) events result) :
    ∃ value, first = Result.ok value ∧ RuntimeEffects.Runs (next value) events result := by
  obtain ⟨before, after, value, partition, initial, following⟩ :=
    bind_inv _ _ _ _ run
  obtain ⟨returned, silent⟩ := (embed_iff first before value).mp initial
  have same : events = after := by simpa only [silent, List.nil_append] using partition
  subst after
  exact ⟨value, returned, following⟩

end RuntimeRuns
