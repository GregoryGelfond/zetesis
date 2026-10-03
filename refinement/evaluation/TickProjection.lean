import ContextEvents
import RuntimeRuns

open Aeneas Aeneas.Std Result ControlFlow Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# Successful returning-event ticks

The event contexts keep their original limits and control handles while reads
obtain new responses. A successful finite tick must have observed clear control
and passed the existing work ceiling. It therefore increments exactly once and
preserves the other work fields. When the represented input control also has
clear fixed observations, that result agrees with the actual generated tick.

This is a projection of completed context runs. It does not equate arbitrary
changing event histories with the unchanged generated whole checker, or assume
that a requested read returns. The context-generation and handle interpretation
boundaries remain those stated by RuntimeContexts and ContextEvents.
-/
namespace TickProjection

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

/-- With a deadline present, the event poll first reads cancellation. A true
response stops immediately; only a false response reaches the expiry read.
This equation follows the retained contexts and preserves their short circuit. -/
theorem poll_with_deadline (control : zetesis_cpu.cancellation.Cancellation)
    (owner : alloc.sync.Arc zetesis_cpu.cancellation.DeadlineOwner)
    (present : control.deadline = some owner) :
    ContextEvents.poll control =
      ITree.vis (E := effects) (.read (ContextEvents.cancelObject control) .Relaxed)
        (fun cancelled => if cancelled then ITree.ret (.Err .Cancelled)
          else ITree.vis (.read (2 * owner.value.deadline.owner + 1) .Relaxed)
            (fun expired => ITree.ret (if expired then .Err .Deadline else .Ok ()))) := by
  simp [ContextEvents.poll, RuntimeContexts.pollContext, ContextEvents.readAt,
    RuntimeEffects.load, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
    core.option.Option.as_ref, ContextEvents.lift_result, RuntimeEffects.embed_ok,
    ContextEvents.deadline, RuntimeContexts.deadlineContext, present, Bind.bind, Pure.pure]
  congr 1
  funext cancelled
  cases cancelled <;> simp
  congr 1
  funext expired
  cases expired <;> rfl

/-- Isolate the eventful poll from the unchanged backend suffix of the tick.
The suffix is the same context specialized to the backend monad with that actual
poll result. Its embedding preserves scalar failure and divergence; it does not
replace either outcome with a source Stop. -/
theorem tick_factors (work : oracle.Work) :
    ContextEvents.tick work =
      ITree.bind (ContextEvents.poll work.cancellation)
        (fun answer => RuntimeEffects.embed
          (RuntimeContexts.tickContext (M := Result) (fun _ => ok answer) work)) := by
  unfold ContextEvents.tick RuntimeContexts.tickContext
  simp only [bind_tc_ok, ContextEvents.lift_result]
  congr 1
  funext answer
  cases answer with
  | Err reason =>
    simp [core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
      core.convert.FromSame.from, liftM, monadLift_self, RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind, bind_ok]
  | Ok value =>
    cases value
    simp only [core.result.Result.Insts.CoreOpsTry.branch, liftM, monadLift_self,
      RuntimeEffects.embed_ok, Bind.bind, itree_ret_bind, bind_ok]
    split
    · rfl
    · rw [RuntimeEffects.embed_bind]
      simp only [RuntimeEffects.embed_ok]

/-- A successful event poll consumes a clear cancellation read and, exactly
when configured, a clear deadline read. This is derived from the run constructors,
not assumed as a poll verdict. In particular, every successful poll consumes a
read even when no deadline exists. -/
theorem completed_poll (control : zetesis_cpu.cancellation.Cancellation)
    (events : List RuntimeEffects.Event)
    (run : Runs (ContextEvents.poll control) events (.Ok ())) :
    events = ⟨.read (ContextEvents.cancelObject control) .Relaxed, false⟩ ::
      match control.deadline with
      | none => []
      | some owner => [⟨.read (2 * owner.value.deadline.owner + 1) .Relaxed, false⟩] := by
  cases present : control.deadline with
  | none =>
    rw [ContextEvents.poll_without_deadline control present] at run
    obtain ⟨cancelled, tail, consumed, continued⟩ := RuntimeRuns.observed_inv _ _ _ _ run
    cases cancelled with
    | false =>
      have finished : tail = [] := (RuntimeRuns.returned_inv _ _ _ continued).2
      simpa only [finished] using consumed
    | true =>
      have impossible := (RuntimeRuns.returned_inv _ _ _ continued).1
      contradiction
  | some owner =>
    rw [poll_with_deadline control owner present] at run
    obtain ⟨cancelled, tail, consumed, continued⟩ := RuntimeRuns.observed_inv _ _ _ _ run
    cases cancelled with
    | true =>
      have impossible := (RuntimeRuns.returned_inv _ _ _ continued).1
      contradiction
    | false =>
      obtain ⟨expired, rest, readExpiry, finished⟩ := RuntimeRuns.observed_inv _ _ _ _ continued
      cases expired with
      | true =>
        have impossible := (RuntimeRuns.returned_inv _ _ _ finished).1
        contradiction
      | false =>
        have ended : rest = [] := (RuntimeRuns.returned_inv _ _ _ finished).2
        simpa only [readExpiry, ended] using consumed

/-- A successful finite event tick agrees with the actual generated tick when
its represented input tokens are clear, and consumes at least one read. First
invert the eventful poll followed by the embedded backend suffix. A stopped poll
cannot yield successful tick output. The successful poll's read receipt supplies
strict event progress; the fixed-clear poll equation identifies the backend
suffix with the actual call. No tick-success premise is used. -/
theorem completed_tick (before after : oracle.Work)
    (events : List RuntimeEffects.Event)
    (clear : EvaluatorControl.observation before.cancellation = none)
    (run : Runs (ContextEvents.tick before) events (.Ok (), after)) :
    oracle.Work.tick before = ok (.Ok (), after) ∧ events ≠ [] := by
  rw [tick_factors] at run
  obtain ⟨pollEvents, suffix, answer, consumed, polled, continued⟩ :=
    RuntimeRuns.bind_inv _ _ _ _ run
  obtain ⟨completed, silent⟩ := (RuntimeRuns.embed_iff _ _ _).mp continued
  have successfulPoll : answer = .Ok () := by
    cases answer with
    | Ok value => cases value; rfl
    | Err reason =>
      simp [RuntimeContexts.tickContext, liftM, monadLift_self,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
        core.convert.FromSame.from, Bind.bind, bind_ok] at completed
  subst answer
  have actualSuffix :
      RuntimeContexts.tickContext (M := Result) (fun _ => ok (.Ok ())) before =
        oracle.Work.tick before := by
    rw [← RuntimeContexts.tick_reconstruct]
    simp only [RuntimeContexts.tickContext, EvaluatorControl.poll_exact, clear]
  have generated : oracle.Work.tick before = ok (.Ok (), after) := by
    rw [← actualSuffix]
    exact completed
  have reads : pollEvents =
      ⟨.read (ContextEvents.cancelObject before.cancellation) .Relaxed, false⟩ ::
        match before.cancellation.deadline with
        | none => []
        | some owner => [⟨.read (2 * owner.value.deadline.owner + 1) .Relaxed, false⟩] :=
    completed_poll before.cancellation pollEvents polled
  have progress : events ≠ [] := by
    rw [consumed, silent, List.append_nil, reads]
    exact List.cons_ne_nil _ _
  exact ⟨generated, progress⟩

/-- A completed event tick admits exactly one work unit below the configured
ceiling. It preserves the limits, control handles and subset count. The earlier
projection identifies the actual generated tick; its existing checked scalar
laws then supply the bound and increment without assuming arithmetic success. -/
theorem completed_receipt (before after : oracle.Work)
    (events : List RuntimeEffects.Event)
    (clear : EvaluatorControl.observation before.cancellation = none)
    (run : Runs (ContextEvents.tick before) events (.Ok (), after)) :
    before.statistics.work.val < before.limits.max_work.val ∧
      after.statistics.work.val = before.statistics.work.val + 1 ∧
      after.limits = before.limits ∧ after.cancellation = before.cancellation ∧
      after.statistics.subsets = before.statistics.subsets := by
  have generated : oracle.Work.tick before = ok (.Ok (), after) :=
    (completed_tick before after events clear run).1
  have remaining : before.statistics.work.val < before.limits.max_work.val := by
    by_contra exhausted
    have stopped := EvaluatorControl.tick_at_limit before clear (Nat.le_of_not_gt exhausted)
    rw [generated] at stopped
    simp only [Result.ok.injEq, Prod.mk.injEq] at stopped
    have impossible := stopped.1
    contradiction
  obtain ⟨next, count, advanced⟩ := EvaluatorControl.tick_advances before clear remaining
  have exactAfter : after =
      {before with statistics := {before.statistics with work := next}} := by
    rw [generated] at advanced
    simpa only [Result.ok.injEq, Prod.mk.injEq, true_and] using advanced
  rw [exactAfter]
  exact ⟨remaining, count, rfl, rfl, rfl⟩

end TickProjection
