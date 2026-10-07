import Native.Funs

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract

/-!
# One observed control boundary in the extracted evaluator

The theorems unfold the actual generated poll and work-tick definitions. The
separately supplied atomic tokens describe the reads in one invocation only.
Cancellation takes precedence over expiry; either takes precedence over the
work limit. An admitted tick increments the machine counter without overflow
and preserves every other field.

The result is conditional on the external observation model, not on an assumed
poll or tick verdict. It establishes no observation history across iterations,
concurrent atomic semantics, timer behavior or eventual cancellation response.
-/

namespace NativeControl

/-- The reason observed at this single poll. A cancellation observation wins
before the optional slot comparison and deadline are consulted. A mismatching
slot word also cancels; absent controls perform no corresponding read. -/
def observation (control : zetesis_cpu.cancellation.Cancellation) :
    Option zetesis_cpu.cancellation.Stop :=
  if control.cancelled.value.nextRead then some .Cancelled
  else if (control.slot.map
      (fun member => member.state.value.nextRead != member.active)).getD false
    then some .Cancelled
  else if (control.deadline.map
      (fun owner => owner.value.deadline.value.expired.nextRead)).getD false
    then some .Deadline else none

/-- The actual extracted poll returns precisely the supplied single-invocation
observations, in cancellation-before-deadline order. All reads it requests use
Relaxed ordering, so the external model's unsupported-order branch is absent. -/
theorem poll_exact (control : zetesis_cpu.cancellation.Cancellation) :
    zetesis_cpu.cancellation.Cancellation.poll control =
      ok (match observation control with
        | some reason => core.result.Result.Err reason
        | none => core.result.Result.Ok ()) := by
  cases cancelled : control.cancelled.value.nextRead <;>
    cases membership : control.slot <;> cases deadline : control.deadline <;>
    simp [zetesis_cpu.cancellation.Cancellation.poll, observation,
      alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
      core.sync.atomic.AtomicBoolAlign1U8.load,
      core.sync.atomic.AtomicU64Align8U64.load,
      zetesis_cpu.cancellation.slot.Membership.is_cancelled,
      core.option.Option.as_ref, core.option.Option.is_some_and,
      zetesis_cpu.cancellation.Cancellation.poll.closure.Insts.CoreOpsFunctionFnOnceTupleSharedArcDeadlineOwnerBool.call_once,
      cancelled, membership, deadline] <;>
    split <;> simp_all
  all_goals split <;> rfl

/-- A control stop is returned before any work-limit test or counter update.
The entire work record is preserved, even if its counter has reached its limit. -/
theorem tick_stopped (work : oracle.Work) (reason : zetesis_cpu.cancellation.Stop)
    (stopped : observation work.cancellation = some reason) :
    oracle.Work.tick work = ok (core.result.Result.Err reason, work) := by
  simp [oracle.Work.tick, poll_exact, stopped,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- With no control stop, an exhausted work allowance refuses the tick and
preserves every field. Equality with the limit already exhausts the allowance. -/
theorem tick_at_limit (work : oracle.Work)
    (clear : observation work.cancellation = none)
    (exhausted : work.limits.max_work.val ≤ work.statistics.work.val) :
    oracle.Work.tick work = ok (core.result.Result.Err .WorkLimit, work) := by
  simp [oracle.Work.tick, poll_exact, clear,
    core.result.Result.Insts.CoreOpsTry.branch, UScalar.le_equiv, exhausted]

/-- With no control stop and remaining allowance, the actual tick succeeds and
increments by exactly one. The stored U64 limit bounds the old value strictly,
so addition fits before the backend addition theorem is applied. Only the work
counter changes; the subset count, limits and observations remain unchanged. -/
theorem tick_advances (work : oracle.Work)
    (clear : observation work.cancellation = none)
    (remaining : work.statistics.work.val < work.limits.max_work.val) :
    ∃ next : U64, next.val = work.statistics.work.val + 1 ∧
      oracle.Work.tick work = ok (core.result.Result.Ok (),
        { work with statistics := { work.statistics with work := next } }) := by
  have limitFits : work.limits.max_work.val ≤ U64.max := by
    simpa only [U64.max_eq] using U64.le_max work.limits.max_work
  have incrementFits : work.statistics.work.val + (1#u64).val ≤ U64.max := by
    change work.statistics.work.val + 1 ≤ U64.max
    omega
  obtain ⟨next, added, incremented⟩ := WP.spec_imp_exists
    (U64.add_spec (x := work.statistics.work) (y := 1#u64) incrementFits)
  refine ⟨next, ?_, ?_⟩
  · simpa using incremented
  · simp [oracle.Work.tick, poll_exact, clear,
      core.result.Result.Insts.CoreOpsTry.branch, UScalar.le_equiv,
      Nat.not_le.mpr remaining, added]

end NativeControl
