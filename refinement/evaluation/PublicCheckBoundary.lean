import OwnerChecks
import WorkInitialization
import Control
import ReservedStorage

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Refusals before reference checking

The actual public wrapper checks ownership, then control, then prepares its
first evaluation buffer. These laws retain that order without assuming any
later operation returns. They describe the supplied reservation and fixed
control interpretation; changing allocation or observation histories remain
separate correspondence obligations. A stop yields no public partial statistics.
-/
namespace PublicCheckBoundary

variable [reservation : VectorReservation]

/-- A foreign candidate is refused before polling or reserving storage. Equal
formula contents do not override distinct owners. No storage invariant or
returning-provider premise is needed. -/
theorem wrong_owner (program : theory.Theory) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (cancellation : zetesis_cpu.cancellation.Cancellation)
    (different : program.owner ≠ candidate.theory.owner) :
    oracle.check program candidate limits cancellation = ok (.Err .WrongProgram) := by
  simp [oracle.check, OwnerChecks.identities_reject program candidate different,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- After matching ownership, a stopped initial poll returns before the first
reservation. Empty theories and candidates do not bypass this control boundary. -/
theorem stopped (program : theory.Theory) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (cancellation : zetesis_cpu.cancellation.Cancellation)
    (same : program.owner = candidate.theory.owner)
    (reason : zetesis_cpu.cancellation.Stop)
    (observed : EvaluatorControl.observation cancellation = some reason) :
    oracle.check program candidate limits cancellation = ok (.Err reason) := by
  have owned := (OwnerChecks.identities_accept_iff program candidate).mpr same
  simp [oracle.check, owned, EvaluatorControl.poll_exact, observed,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- A typed refusal of the first actual reservation becomes Allocation before
formula evaluation. Its reason is derived from the reservation wrapper, not
supplied as an assumed allocation diagnosis. -/
theorem reservation_refused (program : theory.Theory) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (cancellation : zetesis_cpu.cancellation.Cancellation)
    (same : program.owner = candidate.theory.owner)
    (clear : EvaluatorControl.observation cancellation = none)
    (reason : zetesis_cpu.cancellation.Stop)
    (refused : oracle.reserve Bool program.value.nodes.len = ok (.Err reason)) :
    oracle.check program candidate limits cancellation = ok (.Err .Allocation) := by
  have owned := (OwnerChecks.identities_accept_iff program candidate).mpr same
  have allocation := (ReservedStorage.returned_refusal Bool program.value.nodes.len reason refused).1
  subst reason
  have refusedSlice : oracle.reserve Bool program.value.nodes.deref.len =
      ok (.Err zetesis_cpu.cancellation.Stop.Allocation) := by
    simpa only [alloc.vec.Vec.len, alloc.vec.Vec.deref, Slice.len, Slice.from_val] using refused
  simp [oracle.check, owned, EvaluatorControl.poll_exact, clear,
    WorkInitialization.statistics_default, theory.Theory.nodes,
    alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, refusedSlice,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

end PublicCheckBoundary
