import TickProjection

open Aeneas Aeneas.Std Result ControlFlow Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# Causes of returning-event control and work refusals

These receipts characterize the reads that actually produced a typed stop in
the source-checked poll and tick contexts. Local cancellation is read first, then
optional slot membership, then optional expiry. Only clear controls reach the
work limit. A refused tick retains every work field and charges no unit.
No stored-token observation, successful poll or allocation premise is assumed.
Backend failure, divergence and nonreturning requests remain outside finite Runs.
-/
namespace RuntimeRefusal

/-- Exactly the reads made by a poll that observes no control stop. -/
def clearReads (control : zetesis_cpu.cancellation.Cancellation) : List Event :=
  ⟨.read (ContextEvents.cancelObject control) .Relaxed, false⟩ ::
    (ControlReads.matchingSlot (ContextEvents.slotRead control) ++
      ControlReads.clearDeadline (ContextEvents.deadlineRead control))

/-- A refusal receipt retains the actual local bit or mismatching U64 response,
or the expiry bit reached after clear cancellation checks. Its result index
excludes the success constructor of the shared exact receipt relation. -/
abbrev PollRefusal (control : zetesis_cpu.cancellation.Cancellation)
    (events : List Event) (reason : zetesis_cpu.cancellation.Stop) : Prop :=
  ControlReads.Receipt (ContextEvents.cancelObject control)
    (ContextEvents.slotRead control) (ContextEvents.deadlineRead control) events (.Err reason)

/-- A stopped source-checked event poll determines the exact response sequence
and cause. The normal-form receipt is derived by inverting the executed reads. -/
theorem poll_refused (control : zetesis_cpu.cancellation.Cancellation)
    (events : List Event) (reason : zetesis_cpu.cancellation.Stop)
    (run : Runs (ContextEvents.poll control) events (.Err reason)) :
    PollRefusal control events reason := by
  rw [ContextEvents.poll_reads] at run
  exact ControlReads.receipt_of_run _ _ _ events (.Err reason) run

/-- Each refusal receipt is realized by the source-checked event poll. Together
with `poll_refused`, this characterizes exact causes without a supplied verdict. -/
theorem poll_receipt_runs (control : zetesis_cpu.cancellation.Cancellation)
    (events : List Event) (reason : zetesis_cpu.cancellation.Stop)
    (receipt : PollRefusal control events reason) :
    Runs (ContextEvents.poll control) events (.Err reason) := by
  rw [ContextEvents.poll_reads]
  exact ControlReads.run_of_receipt _ _ _ events (.Err reason) receipt

/-- A tick stops either at a control read, before testing its limit, or at its
work ceiling after exactly the clear-control reads. -/
inductive TickRefusal (before : oracle.Work) :
    List Event → zetesis_cpu.cancellation.Stop → Prop where
  | control {events : List Event} {reason : zetesis_cpu.cancellation.Stop}
      (receipt : PollRefusal before.cancellation events reason) : TickRefusal before events reason
  | work (exhausted : before.limits.max_work.val ≤ before.statistics.work.val) :
      TickRefusal before (clearReads before.cancellation) .WorkLimit

/-- Every typed event tick refusal retains the whole original work record and
has one of the precise source causes. In particular cancellation and expiry
precede even an already exhausted work allowance.

Proof: split the actual poll run from the embedded backend suffix. A refused
poll is propagated without a scalar increment. A successful poll either meets
the work ceiling or reaches an increment whose successful continuation cannot
return a typed refusal. No backend failure is reclassified as a source stop. -/
theorem tick_refused (before after : oracle.Work) (events : List Event)
    (reason : zetesis_cpu.cancellation.Stop)
    (run : Runs (ContextEvents.tick before) events (.Err reason, after)) :
    after = before ∧ TickRefusal before events reason := by
  have takeReturn {A B : Type} (operation : Result A) (following : A → Result B)
      (result : B) (completed : (operation >>= following) = ok result) :
      ∃ value, operation = ok value ∧ following value = ok result := by
    cases actual : operation with
    | ret value => exact ⟨value, rfl, by simpa only [actual, bind_tc_ok] using completed⟩
    | vis effect continuation => simp only [actual, bind_tc_vis, vis_not_ok] at completed
    | div => simp only [actual, bind_tc_div, div_not_ok] at completed
  rw [TickProjection.tick_factors] at run
  obtain ⟨pollEvents, suffix, answer, partition, polled, continued⟩ :=
    RuntimeRuns.bind_inv _ _ _ _ run
  obtain ⟨computed, silent⟩ := (RuntimeRuns.embed_iff _ _ _).mp continued
  have sameEvents : events = pollEvents := by simpa only [silent, List.append_nil] using partition
  rw [sameEvents]
  cases answer with
  | Err stopped =>
    have exactSuffix : RuntimeContexts.tickContext (M := Result) (fun _ => ok (.Err stopped)) before =
        ok (.Err stopped, before) := by
      simp [RuntimeContexts.tickContext, liftM, monadLift_self,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
        core.convert.FromSame.from]
    have same := Result.ok_injective (exactSuffix.symm.trans computed)
    have stopSame : stopped = reason := core.result.Result.Err.inj (congrArg Prod.fst same)
    have workSame : before = after := congrArg Prod.snd same
    subst reason
    exact ⟨workSame.symm, .control (poll_refused before.cancellation pollEvents stopped polled)⟩
  | Ok value =>
    cases value
    have reads : pollEvents = clearReads before.cancellation :=
      TickProjection.completed_poll before.cancellation pollEvents polled
    simp only [RuntimeContexts.tickContext, liftM, monadLift_self, bind_tc_ok,
      core.result.Result.Insts.CoreOpsTry.branch, UScalar.le_equiv] at computed
    split at computed
    · have same := Result.ok_injective computed
      have stopSame : zetesis_cpu.cancellation.Stop.WorkLimit = reason :=
        core.result.Result.Err.inj (congrArg Prod.fst same)
      have workSame : before = after := congrArg Prod.snd same
      subst reason
      exact ⟨workSame.symm, reads ▸ TickRefusal.work (by assumption)⟩
    · obtain ⟨incremented, _, impossible⟩ := takeReturn _ _ _ computed
      have same := congrArg Prod.fst (Result.ok_injective impossible)
      contradiction

end RuntimeRefusal
