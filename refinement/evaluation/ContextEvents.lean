import RuntimeContexts
import Iteration
import Step
import ControlReads

open Aeneas Aeneas.Std Result ControlFlow Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# Returning events in the reconstructed contexts

This module instantiates the same structurally checked contexts with
returning reads. Owner/field keys identify unchanged handles; the stored old
observation value is not the result of a new runtime read. Cancellation reads use
the enclosing cancellation Arc owner, while expiry reads use the nested deadline
Arc owner and its expired-field tag. These keys are not derived from the extracted
Atomic value, which has no address. Slot reads use their state Arc owner in
the separate U64 request space and retain the captured active word. Their relation to live Rust objects and the
checked context generalization remain explicit trusted adaptations. The loop and public-check correspondences are proved separately in
`RuntimeProjection` and `RuntimePublicRefusal`.
-/
namespace ContextEvents

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

/-- The local monad lift is exactly the failure-preserving existing embedding. -/
theorem lift_result {T : Type} (operation : Result T) :
    (liftM operation : Computation T) = RuntimeEffects.embed operation := by rfl

/-- A read site retains an object key and ordering while obtaining a new value.
The supplied token's old bit is deliberately not consulted. -/
def readAt (object : Nat)
    (_token : core.sync.atomic.Atomic Bool (core.sync.atomic.private.Align1 U8))
    (ordering : core.sync.atomic.Ordering) : Computation Bool :=
  RuntimeEffects.load ⟨object⟩ ordering

/-- The word adapter preserves the actual returned U64. The membership context
itself compares that response with the token's captured active word. -/
def readWordAt (object : Nat)
    (_token : core.sync.atomic.Atomic U64 (core.sync.atomic.private.Align8 U64))
    (ordering : core.sync.atomic.Ordering) : Computation U64 :=
  RuntimeEffects.loadU64 ⟨object⟩ ordering

/-- The even key names the cancellation Arc allocation itself. The odd key
used below names the expired field inside a deadline Arc allocation. The trusted
object mapping preserves live owners and field identity; this arithmetic encoding
is neither a machine address nor a theorem about physical memory. -/
def cancelObject (control : zetesis_cpu.cancellation.Cancellation) : Nat :=
  2 * control.cancelled.owner

/-- The source slot's read owner and captured active word, without consulting
its old fixed observation. Absence remains explicit and consumes no word read. -/
def slotRead (control : zetesis_cpu.cancellation.Cancellation) : Option (Nat × U64) :=
  control.slot.map fun member => (member.state.owner, member.active)

/-- The configured deadline's existing odd field key. -/
def deadlineRead (control : zetesis_cpu.cancellation.Cancellation) : Option Nat :=
  control.deadline.map fun owner => 2 * owner.value.deadline.owner + 1

/-- Instantiate the exact generated membership comparison with a returning U64
read. This operation does not assume whether the token remains active. -/
def membership (member : zetesis_cpu.cancellation.slot.Membership) : Computation Bool :=
  RuntimeContexts.membershipContext (readWordAt member.state.owner) member

/-- The actual option-model branching invokes the reconstructed deadline
callback only when a deadline exists. -/
def deadline (value : Option (alloc.sync.Arc zetesis_cpu.cancellation.DeadlineOwner)) :
    Computation Bool :=
  match value with
  | none => pure false
  | some owner =>
      RuntimeContexts.deadlineContext (readAt (2 * owner.value.deadline.owner + 1)) () owner

def poll (control : zetesis_cpu.cancellation.Cancellation) :
    Computation (core.result.Result Unit zetesis_cpu.cancellation.Stop) :=
  RuntimeContexts.pollContext (readAt (cancelObject control)) membership deadline control

def tick (work : oracle.Work) :
    Computation ((core.result.Result Unit zetesis_cpu.cancellation.Stop) × oracle.Work) :=
  RuntimeContexts.tickContext poll work

/-- The source-checked context has exactly the explicit control-read normal
form. The slot comparison is unfolded from its generated body; no membership
verdict, successful read or poll outcome is supplied as a premise. -/
theorem poll_reads (control : zetesis_cpu.cancellation.Cancellation) :
    poll control = ControlReads.poll (cancelObject control)
      (slotRead control) (deadlineRead control) := by
  cases configured : control.slot <;> cases timed : control.deadline <;>
    simp [poll, RuntimeContexts.pollContext, readAt, readWordAt,
      RuntimeEffects.load, RuntimeEffects.loadU64,
      alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, core.option.Option.as_ref,
      lift_result, RuntimeEffects.embed_ok, membership, RuntimeContexts.membershipContext,
      deadline, RuntimeContexts.deadlineContext, slotRead, deadlineRead,
      ControlReads.poll, ControlReads.afterLocal, ControlReads.finish,
      configured, timed, Bind.bind, Pure.pure]
  all_goals
    congr 1
    funext cancelled
    cases cancelled <;> simp
  all_goals
    congr 1
    funext observed
    simp only [apply_ite]
    split <;> simp_all

/-- Two calls on the same unchanged handle may consume different responses.
The second returns the actual cancellation Stop; it is not backend failure. -/
theorem two_polls_observe_change (control : zetesis_cpu.cancellation.Cancellation)
    (absent : control.deadline = none) (unbound : control.slot = none) :
    Runs (do let first ← poll control; let second ← poll control; pure (first, second))
      [⟨.read (cancelObject control) .Relaxed, false⟩,
       ⟨.read (cancelObject control) .Relaxed, true⟩]
      (.Ok (), .Err .Cancelled) := by
  rw [poll_reads]
  simp only [slotRead, deadlineRead, unbound, absent, Option.map_none,
    ControlReads.poll, ControlReads.afterLocal, ControlReads.finish]
  simp only [Bind.bind, itree_vis_bind]
  refine Runs.observed (.read (cancelObject control) .Relaxed) false _ _ _ ?_
  simp only [Bool.false_eq_true, ↓reduceIte, itree_ret_bind]
  refine Runs.observed (.read (cancelObject control) .Relaxed) true _ _ _ ?_
  simp only [↓reduceIte, Pure.pure, itree_ret_bind]
  exact Runs.returned _

/-- A cancellation response at the actual tick context preserves every work
field and returns before the quota test, even if that quota is already spent. -/
theorem tick_cancelled (work : oracle.Work) :
    Runs (tick work) [⟨.read (cancelObject work.cancellation) .Relaxed, true⟩]
      (.Err .Cancelled, work) := by
  simp only [tick, RuntimeContexts.tickContext,
    poll_reads, ControlReads.poll, Bind.bind, itree_vis_bind]
  refine Runs.observed (.read (cancelObject work.cancellation) .Relaxed) true _ _ _ ?_
  simp [lift_result, RuntimeEffects.embed_ok, itree_ret_bind,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
    core.convert.FromSame.from]
  exact Runs.returned _

/-- An exhausted actual iterator completes the generalized evaluator context
without reaching its tick hole and therefore consumes no read event. -/
theorem exhausted_body (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (cursor : core.iter.adapters.enumerate.Enumerate (core.slice.iter.Iter theory.Node))
    (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (exhausted : cursor.iter.slice.val.length ≤ cursor.iter.i) :
    Runs (RuntimeContexts.evaluationContext tick candidate frozen cursor output
      limits control statistics) []
      (.done (.Ok (), output, ⟨limits, control, statistics⟩)) := by
  simp only [RuntimeContexts.evaluationContext,
    EvaluatorIteration.enumerate_next_exhausted cursor exhausted]
  change Runs (ITree.bind (RuntimeEffects.embed (ok (none, cursor))) _) _ _
  rw [RuntimeEffects.embed_ok, itree_ret_bind]
  change Runs (RuntimeEffects.embed (ok (.done (.Ok (), output,
    ⟨limits, control, statistics⟩)) : Result Evaluation.Transition)) [] _
  rw [RuntimeEffects.embed_ok]
  exact Runs.returned _

end ContextEvents
