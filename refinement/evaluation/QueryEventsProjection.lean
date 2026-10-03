import ReferenceEvents
import TickProjection

open Aeneas Aeneas.Std Result ControlFlow Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# Composing a completed reduct query

The query polls before its subset budget, increments once, evaluates the frozen
reduct and checks its roots. A successful event execution identifies the actual
calls in that order. This module composes explicit phase projections; consumers
must supply the concrete evaluation and root-scan theorems.
-/
namespace QueryEventsProjection

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

/-- Success of the query retains its exact result, workspace and work record in
the generated checker. The query's own poll supplies a nonempty observation
prefix, even when evaluation and root checking have no elements. -/
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
    (program : theory.Theory) (subset : theory.Interpretation) (frozen : Slice Bool)
    (input output : alloc.vec.Vec Bool) (before after : oracle.Work) (answer : Bool)
    (events : List Event)
    (clear : EvaluatorControl.observation before.cancellation = none)
    (run : Runs (ReferenceEvents.checkSubset program subset frozen input before)
      events (.Ok answer, output, after)) :
    oracle.check_subset program subset frozen input before = ok (.Ok answer, output, after) ∧
      after.cancellation = before.cancellation ∧ events ≠ [] := by
  unfold ReferenceEvents.checkSubset CheckerContexts.subsetQuery at run
  change Runs (ITree.bind (ContextEvents.poll before.cancellation) _) _ _ at run
  obtain ⟨pollEvents, tail, polled, partition, pollRun, following⟩ :=
    RuntimeRuns.bind_inv _ _ _ _ run
  cases polled with
  | Err reason =>
    simp [core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
      core.convert.FromSame.from, ContextEvents.lift_result, embed_ok, Bind.bind,
      itree_ret_bind] at following
    have impossible := (RuntimeRuns.returned_inv _ _ _ following).1
    cases impossible
  | Ok success =>
    cases success
    have reads := TickProjection.completed_poll before.cancellation pollEvents pollRun
    have consumed : events ≠ [] := by
      rw [partition, reads]
      simp only [List.cons_append, ne_eq, List.cons_ne_nil, not_false_eq_true]
    simp only [core.result.Result.Insts.CoreOpsTry.branch,
      ContextEvents.lift_result, embed_ok, Bind.bind, itree_ret_bind] at following
    split at following
    next exhausted =>
      have impossible := (RuntimeRuns.returned_inv _ _ _ following).1
      cases impossible
    next remaining =>
      obtain ⟨increment, incremented, evaluated⟩ := RuntimeRuns.embedded_bind_inv _ _ _ _ following
      obtain ⟨evaluationEvents, rest, evaluatedResult, evaluationPartition, evaluationRun, roots⟩ :=
        RuntimeRuns.bind_inv _ _ _ _ evaluated
      rcases evaluatedResult with ⟨evaluatedAnswer, values, evaluatedWork⟩
      cases evaluatedAnswer with
      | Err reason =>
        simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
          core.convert.FromSame.from, embed_ok, Bind.bind, itree_ret_bind] at roots
        have impossible := (RuntimeRuns.returned_inv _ _ _ roots).1
        cases impossible
      | Ok success =>
        cases success
        obtain ⟨actualEvaluation, evaluationControl⟩ :=
          evaluationProjection program subset (some frozen) input
            {before with statistics := {before.statistics with subsets := increment}}
            values evaluatedWork evaluationEvents clear evaluationRun
        have clearEvaluation : EvaluatorControl.observation evaluatedWork.cancellation = none := by
          rw [evaluationControl]
          exact clear
        simp only [embed_ok, itree_ret_bind] at roots
        obtain ⟨rootEvents, suffix, rootResult, rootPartition, rootRun, finished⟩ :=
          RuntimeRuns.bind_inv _ _ _ _ roots
        rcases rootResult with ⟨rootAnswer, finalWork⟩
        cases rootAnswer with
        | Err reason =>
          simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
            core.convert.FromSame.from, embed_ok, Bind.bind, itree_ret_bind] at finished
          have impossible := (RuntimeRuns.returned_inv _ _ _ finished).1
          cases impossible
        | Ok failed =>
          obtain ⟨actualRoots, rootControl⟩ :=
            rootProjection program (alloc.vec.Vec.deref values) evaluatedWork failed
              finalWork rootEvents clearEvaluation rootRun
          simp only [embed_ok, itree_ret_bind] at finished
          have same := (RuntimeRuns.returned_inv _ _ _ finished).1
          have finalEqual : finalWork = after := congrArg (fun result => result.2.2) same
          have outputEqual : values = output := congrArg (fun result => result.2.1) same
          have answerEqual : core.option.Option.is_none failed = answer :=
            core.result.Result.Ok.inj (congrArg Prod.fst same)
          have actual : oracle.check_subset program subset frozen input before =
              ok (.Ok (core.option.Option.is_none failed), values, finalWork) := by
            simp only [oracle.check_subset, EvaluatorControl.poll_exact, clear, bind_tc_ok,
              core.result.Result.Insts.CoreOpsTry.branch, remaining, ↓reduceIte,
              incremented, actualEvaluation, uncurry, actualRoots]
          exact ⟨by simpa only [finalEqual, outputEqual, answerEqual] using actual,
            by rw [← finalEqual, rootControl, evaluationControl], consumed⟩

end QueryEventsProjection
