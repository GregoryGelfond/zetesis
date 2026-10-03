import WorkInitialization
import FrozenQuery
import VectorReservation
import Control

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Construction of the stored reduct

The actual generated freeze reserves a workspace, evaluates original truth and
publishes the candidate with that truth table. `completed_phases` derives both
successful calls and candidate retention from the constructor's actual return.
`completed_represents` then establishes the invariant consumed by `FrozenQuery`.
No original-model, root-validity or subset premise belongs to this producer.

The supplied reservation need not succeed or preserve logical contents for
these semantic laws: evaluation clears its workspace before scanning. Physical
capacity and allocator behavior remain trusted library contracts outside the
sequence model. Fixed observation tokens do not establish changing concurrent
control behavior, and extracted candidate-field equality is not a lifetime or
aliasing proof about the runtime borrow.
-/
namespace FrozenConstruction

variable [reservation : VectorReservation]

/-- The actual node count passed to reservation. -/
abbrev nodeCount (candidate : theory.Interpretation) : Usize :=
  Slice.len (alloc.vec.Vec.deref candidate.theory.value.nodes)

/-- A reservation stop is returned unchanged, preserving the entire work record.
No evaluator result is required because this source branch does not call it. -/
theorem reservation_stopped (candidate : theory.Interpretation) (before : oracle.Work)
    (reason : zetesis_cpu.cancellation.Stop)
    (reserved : oracle.reserve Bool (nodeCount candidate) = ok (.Err reason)) :
    reduct.FrozenReduct.freeze candidate before = ok (.Err reason, before) := by
  simp [reduct.FrozenReduct.freeze, theory.Interpretation.impl.theory,
    theory.Theory.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
    reserved, core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- After successful reservation, the actual evaluation result determines the
published result. A typed evaluation stop retains its reason and returned work;
only success publishes a reduct, with the supplied candidate and computed truth.
-/
theorem evaluated (candidate : theory.Interpretation) (before : oracle.Work)
    (workspace truth : alloc.vec.Vec Bool) (after : oracle.Work)
    (answer : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (reserved : oracle.reserve Bool (nodeCount candidate) = ok (.Ok workspace))
    (evaluation : oracle.evaluate candidate.theory candidate none workspace before =
      ok (answer, truth, after)) :
    reduct.FrozenReduct.freeze candidate before =
      ok ((match answer with
        | .Ok _ => core.result.Result.Ok { candidate, truth }
        | .Err reason => .Err reason), after) := by
  cases answer <;>
    simp [reduct.FrozenReduct.freeze, theory.Interpretation.impl.theory,
      theory.Theory.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
      reserved, evaluation, core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- A completed actual freeze retained this candidate and successfully reserved
and evaluated a workspace. These inner equations are conclusions, not premises.
No condition on the reservation's returned contents is needed.

Proof: a reservation stop, backend failure or divergence cannot produce the
supplied success. In the successful reservation branch, the same distinction
applies to evaluation. The actual published record identifies the candidate,
truth table and final work record. -/
theorem completed_phases (candidate : theory.Interpretation) (before : oracle.Work)
    (frozen : reduct.FrozenReduct) (after : oracle.Work)
    (completed : reduct.FrozenReduct.freeze candidate before = ok (.Ok frozen, after)) :
    frozen.candidate = candidate ∧
      ∃ workspace : alloc.vec.Vec Bool,
        oracle.reserve Bool (nodeCount candidate) = ok (.Ok workspace) ∧
        oracle.evaluate candidate.theory candidate none workspace before =
          ok (.Ok (), frozen.truth, after) := by
  cases reserved : oracle.reserve Bool (nodeCount candidate) with
  | ret outcome =>
    cases outcome with
    | Err reason =>
      have refused := reservation_stopped candidate before reason reserved
      rw [refused] at completed
      simp at completed
    | Ok workspace =>
      cases evaluation : oracle.evaluate candidate.theory candidate none workspace before with
      | ret outcome =>
        obtain ⟨answer, truth, finalWork⟩ := outcome
        have actual := evaluated candidate before workspace truth finalWork answer reserved evaluation
        cases answer with
        | Err reason =>
          rw [actual] at completed
          simp at completed
        | Ok value =>
          cases value
          have same : (core.result.Result.Ok ({ candidate, truth } : reduct.FrozenReduct),
              finalWork) = (core.result.Result.Ok frozen, after) :=
            Result.ok_injective (actual.symm.trans completed)
          have sameFrozen : ({ candidate, truth } : reduct.FrozenReduct) = frozen :=
            core.result.Result.Ok.inj (congrArg Prod.fst same)
          have sameWork : finalWork = after := congrArg Prod.snd same
          cases sameFrozen
          cases sameWork
          exact ⟨rfl, workspace, rfl, evaluation⟩
      | vis effect continuation =>
        simp [reduct.FrozenReduct.freeze, theory.Interpretation.impl.theory,
          theory.Theory.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
          reserved, evaluation, core.result.Result.Insts.CoreOpsTry.branch]
          at completed
      | div =>
        simp [reduct.FrozenReduct.freeze, theory.Interpretation.impl.theory,
          theory.Theory.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
          reserved, evaluation, core.result.Result.Insts.CoreOpsTry.branch]
          at completed
  | vis effect continuation =>
    simp [reduct.FrozenReduct.freeze, theory.Interpretation.impl.theory,
      theory.Theory.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
      reserved] at completed
  | div =>
    simp [reduct.FrozenReduct.freeze, theory.Interpretation.impl.theory,
      theory.Theory.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
      reserved] at completed

/-- Actual successful construction establishes the stored-mask invariant used
by the query proof. The candidate need not satisfy the original theory; freezing
records its original truth, including false asserted roots. -/
theorem completed_represents (candidate : theory.Interpretation) (before : oracle.Work)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered candidate.theory.value.nodes.val)
    (frozen : reduct.FrozenReduct) (after : oracle.Work)
    (completed : reduct.FrozenReduct.freeze candidate before = ok (.Ok frozen, after)) :
    FrozenQuery.Represents frozen := by
  obtain ⟨sameCandidate, workspace, _, evaluation⟩ :=
    completed_phases candidate before frozen after completed
  have original : frozen.truth.val = EvaluationSpecification.values candidate none
      candidate.theory.value.nodes.val :=
    FixedEvaluationLoop.completed_values candidate.theory candidate none workspace before
      stored ordered (by intro mask member; cases member) frozen.truth after evaluation
  unfold FrozenQuery.Represents
  rw [sameCandidate]
  exact original

/-- Completed freezing stores one Boolean and charges one work unit per node.
Limits and the subset count are retained. Reservation does not contribute a
logical node tick; the count is not a claim about physical memory or time. -/
theorem completed_work (candidate : theory.Interpretation) (before : oracle.Work)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered candidate.theory.value.nodes.val)
    (frozen : reduct.FrozenReduct) (after : oracle.Work)
    (completed : reduct.FrozenReduct.freeze candidate before = ok (.Ok frozen, after)) :
    after.limits = before.limits ∧
      after.statistics.subsets = before.statistics.subsets ∧
      after.statistics.work.val = before.statistics.work.val +
        candidate.theory.value.nodes.val.length ∧
      frozen.truth.val.length = candidate.theory.value.nodes.val.length := by
  obtain ⟨_, workspace, _, evaluation⟩ := completed_phases candidate before frozen after completed
  have receipt := EvaluationAccounting.returned_receipt candidate.theory candidate none workspace
    before stored ordered (by intro mask member; cases member) (.Ok ()) frozen.truth after evaluation
  exact ⟨receipt.limits, receipt.subsets,
    receipt.work.trans (congrArg (before.statistics.work.val + ·) receipt.boundary),
    receipt.boundary⟩


/-- The public initial poll precedes reservation, including for an empty table.
A supplied observed stop is returned without requiring any reservation outcome. -/
theorem new_stopped (candidate : theory.Interpretation) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (reason : zetesis_cpu.cancellation.Stop)
    (observed : EvaluatorControl.observation control = some reason) :
    reduct.FrozenReduct.new candidate limits control = ok (.Err reason) := by
  simp [reduct.FrozenReduct.new, EvaluatorControl.poll_exact, observed,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- A completed public constructor passed the initial poll and completed the
actual private freeze from zero statistics. Its discarded final work record is
recovered existentially rather than assumed as another successful call. -/
theorem new_completed_phases (candidate : theory.Interpretation) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (frozen : reduct.FrozenReduct)
    (completed : reduct.FrozenReduct.new candidate limits control = ok (.Ok frozen)) :
    EvaluatorControl.observation control = none ∧
      ∃ after : oracle.Work, reduct.FrozenReduct.freeze candidate (WorkInitialization.initial limits control) =
        ok (.Ok frozen, after) := by
  cases observed : EvaluatorControl.observation control with
  | some reason =>
    rw [new_stopped candidate limits control reason observed] at completed
    simp at completed
  | none =>
    have fromFreeze : reduct.FrozenReduct.new candidate limits control =
        (do let (answer, _) ← reduct.FrozenReduct.freeze candidate (WorkInitialization.initial limits control)
            ok answer) := by
      simp [reduct.FrozenReduct.new, EvaluatorControl.poll_exact, observed,
        core.result.Result.Insts.CoreOpsTry.branch,
        WorkInitialization.statistics_default, WorkInitialization.initial]
    rw [fromFreeze] at completed
    cases frozenResult : reduct.FrozenReduct.freeze candidate (WorkInitialization.initial limits control) with
    | ret outcome =>
      obtain ⟨answer, after⟩ := outcome
      have same : answer = .Ok frozen := by
        rw [frozenResult] at completed
        simp only [bind_tc_ok] at completed
        exact Result.ok_injective completed
      cases same
      exact ⟨rfl, after, rfl⟩
    | vis effect continuation => simp only [frozenResult, bind_tc_vis, vis_not_ok] at completed
    | div => simp only [frozenResult, bind_tc_div, div_not_ok] at completed

/-- The actual public constructor produces the query's mask invariant and
retains the supplied candidate. Its private freeze charges exactly the node
count from zero; no modelhood, root or subset assumption is introduced. -/
theorem new_represents (candidate : theory.Interpretation) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered candidate.theory.value.nodes.val)
    (frozen : reduct.FrozenReduct)
    (completed : reduct.FrozenReduct.new candidate limits control = ok (.Ok frozen)) :
    frozen.candidate = candidate ∧ FrozenQuery.Represents frozen ∧
      ∃ after : oracle.Work,
        reduct.FrozenReduct.freeze candidate (WorkInitialization.initial limits control) = ok (.Ok frozen, after) ∧
        after.statistics.work.val = candidate.theory.value.nodes.val.length ∧
        after.statistics.subsets = 0#u64 := by
  obtain ⟨_, after, frozenResult⟩ := new_completed_phases candidate limits control frozen completed
  have sameCandidate := (completed_phases candidate (WorkInitialization.initial limits control)
    frozen after frozenResult).1
  have represented := completed_represents candidate (WorkInitialization.initial limits control)
    stored ordered frozen after frozenResult
  obtain ⟨_, subsets, work, _⟩ := completed_work candidate (WorkInitialization.initial limits control)
    stored ordered frozen after frozenResult
  exact ⟨sameCandidate, represented, after, frozenResult,
    by simpa [WorkInitialization.initial] using work, subsets⟩

end FrozenConstruction
