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

/-- The borrowed representation carries the same supplied atomic tokens and
immutable slot identity. This is a value relation, not a Rust lifetime theorem. -/
def pollingView (control : zetesis_cpu.cancellation.Cancellation) :
    zetesis_cpu.cancellation.CancellationPoll :=
  { cancelled := control.cancelled.value
    expired := control.deadline.map (fun owner => owner.value.deadline.value.expired)
    membership := control.slot.map (fun member =>
      { state := member.state.value, active := member.active }) }

/-- The reason observed by one borrowed poll, with the original read order. -/
def borrowedObservation (control : zetesis_cpu.cancellation.CancellationPoll) :
    Option zetesis_cpu.cancellation.Stop :=
  if control.cancelled.nextRead then some .Cancelled
  else if (control.membership.map
      (fun member => member.state.nextRead != member.active)).getD false
    then some .Cancelled
  else if (control.expired.map (fun expired => expired.nextRead)).getD false
    then some .Deadline else none

/-- Preparing the view preserves each represented atomic token and the slot's
immutable expected generation without asking the external model for a read. -/
theorem polling_exact (control : zetesis_cpu.cancellation.Cancellation) :
    zetesis_cpu.cancellation.Cancellation.polling control = ok (pollingView control) := by
  cases membership : control.slot <;> cases deadline : control.deadline <;>
    simp [zetesis_cpu.cancellation.Cancellation.polling, pollingView,
      zetesis_cpu.cancellation.DeadlineOwner.expiry,
      zetesis_cpu.cancellation.slot.Membership.polling,
      alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
      core.option.Option.as_ref,
      ZetesisNativeExtract.core.option.Option.Insts.CoreOpsTry_traitTry.branch,
      ZetesisNativeExtract.core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual,
      membership, deadline]

/-- Borrowing preserves the single-invocation observation represented by the
owning token; no changing observation history is assumed. -/
theorem polling_observation (control : zetesis_cpu.cancellation.Cancellation) :
    borrowedObservation (pollingView control) = observation control := by
  cases membership : control.slot <;> cases deadline : control.deadline <;>
    simp [borrowedObservation, pollingView, observation, membership, deadline]

/-- The actual borrowed poll returns the supplied observations in the same
cancellation, membership and expiry order using only Relaxed reads. -/
theorem borrowed_poll_exact (control : zetesis_cpu.cancellation.CancellationPoll) :
    zetesis_cpu.cancellation.CancellationPoll.poll control =
      ok (match borrowedObservation control with
        | some reason => core.result.Result.Err reason
        | none => core.result.Result.Ok ()) := by
  cases cancelled : control.cancelled.nextRead <;>
    cases membership : control.membership <;> cases expiry : control.expired <;>
    simp [zetesis_cpu.cancellation.CancellationPoll.poll, borrowedObservation,
      core.sync.atomic.AtomicBoolAlign1U8.load,
      core.sync.atomic.AtomicU64Align8U64.load,
      zetesis_cpu.cancellation.slot.MembershipPoll.is_cancelled,
      core.option.Option.is_some_and,
      zetesis_cpu.cancellation.CancellationPoll.poll.closure.Insts.CoreOpsFunctionFnOnceTupleSharedAtomicBoolAlign1U8Bool.call_once,
      cancelled, membership, expiry] <;>
    repeat' split <;> simp_all
  all_goals split_ifs <;> rfl

/-- The owning poll delegates through a view preserving the same observations.
Its original contract remains available to the evaluator's work proofs. -/
theorem poll_exact (control : zetesis_cpu.cancellation.Cancellation) :
    zetesis_cpu.cancellation.Cancellation.poll control =
      ok (match observation control with
        | some reason => core.result.Result.Err reason
        | none => core.result.Result.Ok ()) := by
  simp [zetesis_cpu.cancellation.Cancellation.poll, polling_exact,
    borrowed_poll_exact, polling_observation]

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
