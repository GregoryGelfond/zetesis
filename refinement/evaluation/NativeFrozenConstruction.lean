import NativeWorkInitialization
import NativeFrozenQuery
import VectorReservation
import NativeControl

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract

/-!
# Construction of the stored reduct

The actual generated freeze reserves a workspace, evaluates original truth and
publishes the candidate with that truth table. `completed_phases` derives both
successful calls and candidate retention from the constructor's actual return.
`completed_represents` then establishes the invariant consumed by `NativeFrozenQuery`.
No original-model, root-validity or subset premise belongs to this producer.

The supplied reservation need not succeed or preserve logical contents for
these semantic laws: evaluation clears its workspace before scanning. Physical
capacity and allocator behavior remain trusted library contracts outside the
sequence model. Fixed observation tokens do not establish changing concurrent
control behavior, and extracted candidate-field equality is not a lifetime or
aliasing proof about the runtime borrow.
-/
namespace NativeFrozenConstruction

variable [reservation : VectorReservation]

/-- The actual node count passed to reservation. -/
abbrev nodeCount (candidate : theory.Interpretation) : Usize :=
  Slice.len (alloc.vec.Vec.deref candidate.theory.value.parts.nodes)

/-- A reservation stop is returned unchanged, preserving the entire work record.
No evaluator result is required because this source branch does not call it. -/
theorem reservation_stopped (candidate : theory.Interpretation) (before : oracle.Work)
    (reason : zetesis_cpu.cancellation.Stop)
    (reserved : oracle.reserve Bool (nodeCount candidate) = ok (.Err reason)) :
    reduct.FrozenReduct.freeze candidate before = ok (.Err reason, before) := by
  simp [reduct.FrozenReduct.freeze, theory.Interpretation.impl.theory,
    theory.Theory.nodes, theory.FormulaParts.impl.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
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
      theory.Theory.nodes, theory.FormulaParts.impl.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
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
      have refused : reduct.FrozenReduct.freeze candidate before = ok (.Err reason, before) := by
        exact reservation_stopped candidate before reason reserved
      rw [refused] at completed
      simp at completed
    | Ok workspace =>
      cases evaluation : oracle.evaluate candidate.theory candidate none workspace before with
      | ret outcome =>
        obtain ⟨answer, truth, finalWork⟩ := outcome
        have actual : reduct.FrozenReduct.freeze candidate before =
            ok ((match answer with
              | .Ok _ => core.result.Result.Ok { candidate, truth }
              | .Err reason => .Err reason), finalWork) := by
          exact evaluated candidate before workspace truth finalWork answer reserved evaluation
        cases answer with
        | Err reason =>
          rw [actual] at completed
          simp at completed
        | Ok value =>
          cases value
          have same : ((core.result.Result.Ok { candidate, truth } :
              core.result.Result reduct.FrozenReduct zetesis_cpu.cancellation.Stop),
              finalWork) = (core.result.Result.Ok frozen, after) := by
            exact Result.ok_injective (actual.symm.trans completed)
          have sameFrozen : ({ candidate, truth } : reduct.FrozenReduct) = frozen := by
            exact core.result.Result.Ok.inj (congrArg Prod.fst same)
          have sameWork : finalWork = after := by
            exact congrArg Prod.snd same
          cases sameFrozen
          cases sameWork
          exact ⟨rfl, workspace, rfl, evaluation⟩
      | vis effect continuation =>
        simp [reduct.FrozenReduct.freeze, theory.Interpretation.impl.theory,
          theory.Theory.nodes, theory.FormulaParts.impl.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
          reserved, evaluation, core.result.Result.Insts.CoreOpsTry.branch]
          at completed
      | div =>
        simp [reduct.FrozenReduct.freeze, theory.Interpretation.impl.theory,
          theory.Theory.nodes, theory.FormulaParts.impl.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
          reserved, evaluation, core.result.Result.Insts.CoreOpsTry.branch]
          at completed
  | vis effect continuation =>
    simp [reduct.FrozenReduct.freeze, theory.Interpretation.impl.theory,
      theory.Theory.nodes, theory.FormulaParts.impl.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
      reserved] at completed
  | div =>
    simp [reduct.FrozenReduct.freeze, theory.Interpretation.impl.theory,
      theory.Theory.nodes, theory.FormulaParts.impl.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
      reserved] at completed

/-- The public initial poll precedes reservation, including for an empty table.
A supplied observed stop is returned without requiring any reservation outcome. -/
theorem new_stopped (candidate : theory.Interpretation) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (reason : zetesis_cpu.cancellation.Stop)
    (observed : NativeControl.observation control = some reason) :
    reduct.FrozenReduct.new candidate limits control = ok (.Err reason) := by
  simp [reduct.FrozenReduct.new, NativeControl.poll_exact, observed,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- A completed public constructor passed the initial poll and completed the
actual private freeze from zero statistics. Its discarded final work record is
recovered existentially rather than assumed as another successful call. -/
theorem new_completed_phases (candidate : theory.Interpretation) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (frozen : reduct.FrozenReduct)
    (completed : reduct.FrozenReduct.new candidate limits control = ok (.Ok frozen)) :
    NativeControl.observation control = none ∧
      ∃ after : oracle.Work, reduct.FrozenReduct.freeze candidate (NativeWorkInitialization.initial limits control) =
        ok (.Ok frozen, after) := by
  cases observed : NativeControl.observation control with
  | some reason =>
    rw [new_stopped candidate limits control reason observed] at completed
    simp at completed
  | none =>
    have fromFreeze : reduct.FrozenReduct.new candidate limits control =
        (do let (answer, _) ← reduct.FrozenReduct.freeze candidate (NativeWorkInitialization.initial limits control)
            ok answer) := by
      simp [reduct.FrozenReduct.new, NativeControl.poll_exact, observed,
        core.result.Result.Insts.CoreOpsTry.branch,
        NativeWorkInitialization.statistics_default, NativeWorkInitialization.initial]
    rw [fromFreeze] at completed
    cases frozenResult : reduct.FrozenReduct.freeze candidate (NativeWorkInitialization.initial limits control) with
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

/-- Actual successful freezing establishes the original-mask invariant, without
requiring that the candidate models its asserted roots. -/
theorem completed_represents (candidate : theory.Interpretation) (before : oracle.Work)
    (valid : NativeStructure.WellFormed candidate.theory.value.atoms.val (NativeExecution.view candidate.theory))
    (stored : NativeMembership.Represented candidate)
    (frozen : reduct.FrozenReduct) (after : oracle.Work)
    (completed : reduct.FrozenReduct.freeze candidate before = ok (.Ok frozen, after)) :
    NativeFrozenQuery.Represents frozen := by
  obtain ⟨sameCandidate, workspace, _, evaluated⟩ := completed_phases candidate before frozen after completed
  have original : frozen.truth.val = NativeSpecification.values candidate none
      (NativeTable.rows (NativeExecution.view candidate.theory)) := by
    exact NativeExecution.completed_values candidate.theory candidate none workspace before
      valid stored (by simp) frozen.truth after evaluated
  unfold NativeFrozenQuery.Represents
  rw [sameCandidate]
  exact original

/-- Successful freezing charges exactly N+E and publishes exactly N truth cells.
The partial evaluator's separate receipt remains available for refused freezes;
reservation adds no logical formula ticks under the supplied library model. -/
theorem completed_work (candidate : theory.Interpretation) (before : oracle.Work)
    (valid : NativeStructure.WellFormed candidate.theory.value.atoms.val (NativeExecution.view candidate.theory))
    (stored : NativeMembership.Represented candidate)
    (frozen : reduct.FrozenReduct) (after : oracle.Work)
    (completed : reduct.FrozenReduct.freeze candidate before = ok (.Ok frozen, after)) :
    after.limits = before.limits ∧ after.cancellation = before.cancellation ∧
      after.statistics.subsets = before.statistics.subsets ∧
      after.statistics.work.val = before.statistics.work.val +
        NativeSpecification.cost (NativeTable.rows (NativeExecution.view candidate.theory)) ∧
      frozen.truth.val.length = (NativeTable.rows (NativeExecution.view candidate.theory)).length := by
  obtain ⟨_, workspace, _, evaluated⟩ := completed_phases candidate before frozen after completed
  obtain ⟨limits, control, subsets, _, _⟩ := NativeEvaluationAccounting.returned_work_bound
    candidate.theory candidate none workspace before valid stored (by simp) (.Ok ()) frozen.truth after evaluated
  have charged : after.statistics.work.val = before.statistics.work.val +
      NativeSpecification.cost (NativeTable.rows (NativeExecution.view candidate.theory)) := by
    simpa only [NativeSpecification.cost_dimensions, Nat.add_assoc] using
      NativeExecution.completed_work candidate.theory candidate none workspace before
        valid stored (by simp) frozen.truth after evaluated
  have values : frozen.truth.val = NativeSpecification.values candidate none
      (NativeTable.rows (NativeExecution.view candidate.theory)) := by
    exact NativeExecution.completed_values candidate.theory candidate none workspace before
      valid stored (by simp) frozen.truth after evaluated
  exact ⟨limits, control, subsets, charged, by rw [values, NativeSpecification.values_length]⟩

/-- Public construction retains the supplied candidate and derives its mask
invariant. Its successful private freeze starts at zero and charges N+E. No
original modelhood, root validity or tested-subset relation is required. -/
theorem new_represents (candidate : theory.Interpretation) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation)
    (valid : NativeStructure.WellFormed candidate.theory.value.atoms.val (NativeExecution.view candidate.theory))
    (stored : NativeMembership.Represented candidate)
    (frozen : reduct.FrozenReduct)
    (completed : reduct.FrozenReduct.new candidate limits control = ok (.Ok frozen)) :
    frozen.candidate = candidate ∧ NativeFrozenQuery.Represents frozen ∧
      ∃ after : oracle.Work,
        reduct.FrozenReduct.freeze candidate (NativeWorkInitialization.initial limits control) = ok (.Ok frozen, after) ∧
        after.statistics.work.val = NativeSpecification.cost (NativeTable.rows (NativeExecution.view candidate.theory)) ∧
        after.statistics.subsets = 0#u64 := by
  obtain ⟨_, after, frozenResult⟩ := new_completed_phases candidate limits control frozen completed
  have sameCandidate : frozen.candidate = candidate := by
    exact (completed_phases candidate (NativeWorkInitialization.initial limits control) frozen after frozenResult).1
  have represented : NativeFrozenQuery.Represents frozen := by
    exact completed_represents candidate (NativeWorkInitialization.initial limits control) valid stored frozen after frozenResult
  obtain ⟨_, _, subsets, work, _⟩ := completed_work candidate (NativeWorkInitialization.initial limits control)
    valid stored frozen after frozenResult
  exact ⟨sameCandidate, represented, after, frozenResult,
    by simpa [NativeWorkInitialization.initial] using work, subsets⟩

end NativeFrozenConstruction
