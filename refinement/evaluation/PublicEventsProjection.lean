import ReferenceEvents
import PublicCheckPhases
import TickProjection

open Aeneas Aeneas.Std Result Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# Composing a completed public check with returning observations

The source-checked public context retains every reached phase and refusal branch.
This module projects a completed event execution to the actual fixed checker,
using explicit successful evaluation, root, selection and search projections.
Those phase contracts must be instantiated by their independent concrete laws;
this composition alone does not complete runtime correspondence.

Clear stored bits choose the logical fixed interpretation of the unchanged
control handles. They do not constrain the values returned by runtime reads.
Every reservation projection follows a reached grant, including repeated equal
requests. No allocation-success or eventual-return premise is introduced.
-/
namespace PublicEventsProjection

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

/-- Successful runtime checking publishes the exact verdict and statistics of
the actual generated checker under the logical reservation provider. Inner calls
are recovered from the event execution; stopped branches cannot supply a Check.

Proof: decompose the actual sequential contexts. Each successful reached phase
projects to its generated call, retaining the control handle. A failed original
root publishes immediately. Otherwise the same reservations, resize, clone and
countermodel result determine the final record. -/
theorem completed
    (evaluationProjection : ∀ program candidate frozen input before output after events,
      EvaluatorControl.observation before.cancellation = none →
      Runs (ReferenceEvents.evaluate program candidate frozen input before) events (.Ok (), output, after) →
      oracle.evaluate program candidate frozen input before = ok (.Ok (), output, after) ∧
        after.cancellation = before.cancellation)
    (rootProjection : ∀ program values before failed after events,
      EvaluatorControl.observation before.cancellation = none →
      Runs (ReferenceEvents.failedRoot program values before) events (.Ok failed, after) →
      oracle.failed_root program values before = ok (.Ok failed, after) ∧
        after.cancellation = before.cancellation)
    (selectionProjection : ∀ program candidate input before output after events,
      EvaluatorControl.observation before.cancellation = none →
      Runs (ReferenceEvents.selectAtoms program candidate input before) events (.Ok (), output, after) →
      oracle.select_atoms program candidate input before = ok (.Ok (), output, after) ∧
        after.cancellation = before.cancellation)
    (searchProjection : ∀ program frozen selected subset input before found returned output after events,
      EvaluatorControl.observation before.cancellation = none →
      Runs (ReferenceEvents.findCountermodel program frozen selected subset input before)
        events (.Ok found, returned, output, after) →
      oracle.find_countermodel program frozen selected subset input before =
        ok (.Ok found, returned, output, after) ∧ after.cancellation = before.cancellation)
    (program : theory.Theory) (candidate : theory.Interpretation) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (checked : oracle.Check)
    (events : List Event) (clear : EvaluatorControl.observation control = none)
    (run : Runs (ReferenceEvents.check program candidate limits control) events (.Ok checked)) :
    @oracle.check ReservationEvents.fixed program candidate limits control = ok (.Ok checked) := by
  have takeSource {A : Type}
      (operation : Computation (core.result.Result A zetesis_cpu.cancellation.Stop))
      (next : core.result.Result A zetesis_cpu.cancellation.Stop →
        Computation (core.result.Result oracle.Check zetesis_cpu.cancellation.Stop))
      (history : List Event)
      (refused : ∀ reason, next (.Err reason) = ITree.ret (.Err reason))
      (returned : Runs (ITree.bind operation next) history (.Ok checked)) :
      ∃ value before after,
        Runs operation before (.Ok value) ∧ Runs (next (.Ok value)) after (.Ok checked) := by
    obtain ⟨before, after, answer, _, called, following⟩ := RuntimeRuns.bind_inv _ _ _ _ returned
    cases answer with
    | Err reason =>
        rw [refused] at following
        have impossible := (RuntimeRuns.returned_inv _ _ _ following).1
        cases impossible
    | Ok value => exact ⟨value, before, after, called, following⟩
  have takePhase {A Frame : Type}
      (operation : Computation ((core.result.Result A zetesis_cpu.cancellation.Stop) × Frame))
      (next : ((core.result.Result A zetesis_cpu.cancellation.Stop) × Frame) →
        Computation (core.result.Result oracle.Check zetesis_cpu.cancellation.Stop))
      (history : List Event)
      (refused : ∀ reason frame, next (.Err reason, frame) = ITree.ret (.Err reason))
      (returned : Runs (ITree.bind operation next) history (.Ok checked)) :
      ∃ value frame before after,
        Runs operation before (.Ok value, frame) ∧
          Runs (next (.Ok value, frame)) after (.Ok checked) := by
    obtain ⟨before, after, answer, _, called, following⟩ := RuntimeRuns.bind_inv _ _ _ _ returned
    rcases answer with ⟨answer, frame⟩
    cases answer with
    | Err reason =>
        rw [refused] at following
        have impossible := (RuntimeRuns.returned_inv _ _ _ following).1
        cases impossible
    | Ok value => exact ⟨value, frame, before, after, called, following⟩
  unfold ReferenceEvents.check CheckerContexts.check at run
  change Runs (ITree.bind (embed (oracle.identities program candidate)) _) _ _ at run
  obtain ⟨ownership, owned, phase⟩ := RuntimeRuns.embedded_bind_inv _ _ _ _ run
  cases ownership with
  | Err reason =>
      simp [core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
        core.convert.FromSame.from, ContextEvents.lift_result, embed_ok, Bind.bind,
        itree_ret_bind] at phase
      have impossible := (RuntimeRuns.returned_inv _ _ _ phase).1
      cases impossible
  | Ok accepted =>
      cases accepted
      simp only [core.result.Result.Insts.CoreOpsTry.branch,
        ContextEvents.lift_result, embed_ok, Bind.bind, itree_ret_bind] at phase
      obtain ⟨polled, _, _, _, phase⟩ := takeSource (ContextEvents.poll control) _ _
        (by intro reason; simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
          core.convert.FromSame.from, embed_ok, Bind.bind,
          itree_ret_bind]) phase
      cases polled
      simp only [WorkInitialization.statistics_default, theory.Theory.nodes, theory.Theory.atom_count,
        alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, OwnerChecks.theory_clone_exact,
        embed_ok, bind_ok, Bind.bind, itree_ret_bind] at phase
      obtain ⟨old, reserveEvents, _, reserved, phase⟩ := takeSource
        (ReservationEvents.reserve Bool program.value.nodes.deref.len) _ _
        (by intro reason; simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual, core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase
      have actualReserved := ReservationEvents.completed_wrapper _ _ _ _ reserved
      simp only [embed_ok, itree_ret_bind] at phase
      obtain ⟨evaluated, evaluationFrame, evaluationEvents, _, evaluationRun, phase⟩ :=
        takePhase (ReferenceEvents.evaluate program candidate none old
          (WorkInitialization.initial limits control)) _ _
          (by intro reason frame; rcases frame with ⟨values, work⟩; simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual, core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase
      cases evaluated
      rcases evaluationFrame with ⟨values, evaluationWork⟩
      obtain ⟨actualEvaluation, evaluationControl⟩ := evaluationProjection program candidate none
        old (WorkInitialization.initial limits control) values evaluationWork evaluationEvents
        clear evaluationRun
      change oracle.evaluate program candidate none old
        { limits, cancellation := control, statistics := { work := 0#u64, subsets := 0#u64 } } =
        ok (.Ok (), values, evaluationWork) at actualEvaluation
      have evaluationClear : EvaluatorControl.observation evaluationWork.cancellation = none := by
        rw [evaluationControl]
        exact clear
      simp only [embed_ok, itree_ret_bind] at phase
      obtain ⟨failed, rootWork, rootEvents, _, rootRun, phase⟩ :=
        takePhase (ReferenceEvents.failedRoot program values.deref evaluationWork) _ _
          (by intro reason frame; simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual, core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase
      obtain ⟨actualRoots, rootControl⟩ := rootProjection program values.deref evaluationWork
        failed rootWork rootEvents evaluationClear rootRun
      have rootClear : EvaluatorControl.observation rootWork.cancellation = none := by
        rw [rootControl]
        exact evaluationClear
      cases failed with
      | some root =>
          simp only [embed_ok, itree_ret_bind] at phase
          have same := core.result.Result.Ok.inj (RuntimeRuns.returned_inv _ _ _ phase).1
          have actual : @oracle.check ReservationEvents.fixed program candidate limits control =
              ok (.Ok { verdict := .NotModel root, statistics := rootWork.statistics }) := by
            simp only [oracle.check, owned, EvaluatorControl.poll_exact, clear, bind_tc_ok,
              core.result.Result.Insts.CoreOpsTry.branch, WorkInitialization.statistics_default,
              theory.Theory.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
              actualReserved, actualEvaluation, uncurry, actualRoots]
          simpa only [same] using actual
      | none =>
          simp only [embed_ok, itree_ret_bind] at phase
          obtain ⟨destination, selectionReserveEvents, _, selectionReserved, phase⟩ :=
            takeSource (ReservationEvents.reserve Usize program.value.atoms) _ _
              (by intro reason; simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual, core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase
          have actualSelectionReserved := ReservationEvents.completed_wrapper _ _ _ _ selectionReserved
          simp only [embed_ok, itree_ret_bind] at phase
          obtain ⟨selectedAnswer, selectionFrame, selectionEvents, _, selectionRun, phase⟩ :=
            takePhase (ReferenceEvents.selectAtoms program candidate destination rootWork) _ _
              (by intro reason frame; rcases frame with ⟨selected, work⟩; simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual, core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase
          cases selectedAnswer
          rcases selectionFrame with ⟨selected, selectionWork⟩
          obtain ⟨actualSelection, selectionControl⟩ := selectionProjection program candidate
            destination rootWork selected selectionWork selectionEvents rootClear selectionRun
          have selectionClear : EvaluatorControl.observation selectionWork.cancellation = none := by
            rw [selectionControl]
            exact rootClear
          simp only [embed_ok, itree_ret_bind] at phase
          obtain ⟨wordDestination, wordEvents, _, wordReserved, phase⟩ :=
            takeSource (ReservationEvents.reserve U64 (alloc.vec.Vec.len candidate.words)) _ _
              (by intro reason; simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual, core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase
          have actualWordReserved := ReservationEvents.completed_wrapper _ _ _ _ wordReserved
          simp only [embed_ok, itree_ret_bind] at phase
          obtain ⟨words, resized, phase⟩ := RuntimeRuns.embedded_bind_inv _ _ _ _ phase
          obtain ⟨scratch, scratchEvents, _, scratchReserved, phase⟩ :=
            takeSource (ReservationEvents.reserve Bool program.value.nodes.deref.len) _ _
              (by intro reason; simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual, core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase
          have actualScratchReserved := ReservationEvents.completed_wrapper _ _ _ _ scratchReserved
          have sameScratch : scratch = old := core.result.Result.Ok.inj
            (Result.ok_injective (actualScratchReserved.symm.trans actualReserved))
          subst scratch
          simp only [embed_ok, itree_ret_bind] at phase
          obtain ⟨found, searchFrame, searchEvents, _, searchRun, phase⟩ :=
            takePhase (ReferenceEvents.findCountermodel program values.deref selected.deref
              { theory := program, words } old selectionWork) _ _
              (by intro reason frame; rcases frame with ⟨subset, output, work⟩; simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual, core.convert.FromSame.from, embed_ok, itree_ret_bind]) phase
          rcases searchFrame with ⟨subset, output, finalWork⟩
          obtain ⟨actualSearch, _⟩ := searchProjection program values.deref selected.deref
            { theory := program, words } old selectionWork found subset output finalWork
            searchEvents selectionClear searchRun
          simp only [embed_ok, itree_ret_bind] at phase
          have same : checked =
              { verdict := (if found then .NonMinimal subset else .Stable),
                statistics := finalWork.statistics } := by
            cases found <;>
              exact (core.result.Result.Ok.inj (RuntimeRuns.returned_inv _ _ _ phase).1).symm
          have actual : @oracle.check ReservationEvents.fixed program candidate limits control =
              ok (.Ok { verdict := (if found then .NonMinimal subset else .Stable), statistics := finalWork.statistics }) := by
            simp only [oracle.check, owned, EvaluatorControl.poll_exact, clear, bind_tc_ok,
              core.result.Result.Insts.CoreOpsTry.branch, WorkInitialization.statistics_default,
              theory.Theory.nodes, theory.Theory.atom_count,
              alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, OwnerChecks.theory_clone_exact,
              actualReserved, actualEvaluation, uncurry, actualRoots, actualSelectionReserved,
              actualSelection, actualWordReserved, resized, actualSearch]
            cases found <;> rfl
          simpa only [same] using actual

end PublicEventsProjection
