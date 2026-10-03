import RuntimeRefusal
import ReferenceEvents

open Aeneas Aeneas.Std Result ControlFlow Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# The reached phase of a refused reduct query

A query polls, tests its subset allowance, increments the subset count, evaluates
the frozen reduct and checks its roots in that order. A finite typed refusal
comes from exactly one reached phase below. The receipt retains the event prefix,
checked increment, reached inner call and its original stop/output/work. It does
not claim that a stopped evaluation or root scan completed its semantic task.
-/
namespace QueryRefusal

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

/-- The actual phase that returns a query's typed stop. Early refusals retain
the supplied workspace and all work fields. Later refusals retain the admitted
subset increment and the actual partial state returned by the reached phase. -/
inductive Origin (program : theory.Theory) (subset : theory.Interpretation)
    (frozen : Slice Bool) (input : alloc.vec.Vec Bool) (before : oracle.Work) :
    List Event → alloc.vec.Vec Bool → oracle.Work → zetesis_cpu.cancellation.Stop → Prop where
  | poll {events : List Event} {reason : zetesis_cpu.cancellation.Stop}
      (receipt : RuntimeRefusal.PollRefusal before.cancellation events reason) :
      Origin program subset frozen input before events input before reason
  | quota (exhausted : before.limits.max_subsets.val ≤ before.statistics.subsets.val) :
      Origin program subset frozen input before
        (RuntimeRefusal.clearReads before.cancellation) input before .CandidateLimit
  | evaluation (increment : U64)
      (remaining : before.statistics.subsets.val < before.limits.max_subsets.val)
      (incremented : before.statistics.subsets + 1#u64 = ok increment)
      (events : List Event) (output : alloc.vec.Vec Bool) (after : oracle.Work)
      (reason : zetesis_cpu.cancellation.Stop)
      (run : Runs (ReferenceEvents.evaluate program subset (some frozen) input
        {before with statistics := {before.statistics with subsets := increment}})
        events (.Err reason, output, after)) :
      Origin program subset frozen input before
        (RuntimeRefusal.clearReads before.cancellation ++ events) output after reason
  | roots (increment : U64)
      (remaining : before.statistics.subsets.val < before.limits.max_subsets.val)
      (incremented : before.statistics.subsets + 1#u64 = ok increment)
      (evaluationEvents rootEvents : List Event) (values : alloc.vec.Vec Bool)
      (evaluatedWork after : oracle.Work) (reason : zetesis_cpu.cancellation.Stop)
      (evaluated : Runs (ReferenceEvents.evaluate program subset (some frozen) input
        {before with statistics := {before.statistics with subsets := increment}})
        evaluationEvents (.Ok (), values, evaluatedWork))
      (checked : Runs (ReferenceEvents.failedRoot program (alloc.vec.Vec.deref values) evaluatedWork)
        rootEvents (.Err reason, after)) :
      Origin program subset frozen input before
        (RuntimeRefusal.clearReads before.cancellation ++ evaluationEvents ++ rootEvents)
        values after reason

/-- A finite refused query determines its actual reached phase and exact event
prefix. Poll refusal precedes the quota comparison; the quota precedes the
checked increment and evaluation; root checking requires successful evaluation.

Proof: invert the binds in source order. Every embedded backend operation must
have actually returned and contributes no event. A source refusal is propagated
unchanged, while the final successful root branch contradicts the supplied stop.
No successful poll, increment, evaluation or root result is assumed. -/
theorem refused (program : theory.Theory) (subset : theory.Interpretation)
    (frozen : Slice Bool) (input output : alloc.vec.Vec Bool) (before after : oracle.Work)
    (events : List Event) (reason : zetesis_cpu.cancellation.Stop)
    (run : Runs (ReferenceEvents.checkSubset program subset frozen input before)
      events (.Err reason, output, after)) :
    Origin program subset frozen input before events output after reason := by
  unfold ReferenceEvents.checkSubset CheckerContexts.subsetQuery at run
  change Runs (ITree.bind (ContextEvents.poll before.cancellation) _) _ _ at run
  obtain ⟨pollEvents, tail, polled, partition, pollRun, following⟩ :=
    RuntimeRuns.bind_inv _ _ _ _ run
  cases polled with
  | Err stopped =>
    simp [core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
      core.convert.FromSame.from, ContextEvents.lift_result, embed_ok, Bind.bind,
      itree_ret_bind] at following
    obtain ⟨same, silent⟩ := RuntimeRuns.returned_inv _ _ _ following
    have stopSame : stopped = reason := core.result.Result.Err.inj (congrArg Prod.fst same)
    have outputSame : input = output := congrArg (fun result => result.2.1) same
    have workSame : before = after := congrArg (fun result => result.2.2) same
    rw [← stopSame, ← outputSame, ← workSame, partition, silent, List.append_nil]
    exact .poll (RuntimeRefusal.poll_refused before.cancellation pollEvents stopped pollRun)
  | Ok success =>
    cases success
    have reads : pollEvents = RuntimeRefusal.clearReads before.cancellation :=
      TickProjection.completed_poll before.cancellation pollEvents pollRun
    simp only [core.result.Result.Insts.CoreOpsTry.branch,
      ContextEvents.lift_result, embed_ok, Bind.bind, itree_ret_bind] at following
    split at following
    next exhausted =>
      obtain ⟨same, silent⟩ := RuntimeRuns.returned_inv _ _ _ following
      have stopSame : zetesis_cpu.cancellation.Stop.CandidateLimit = reason :=
        core.result.Result.Err.inj (congrArg Prod.fst same)
      have outputSame : input = output := congrArg (fun result => result.2.1) same
      have workSame : before = after := congrArg (fun result => result.2.2) same
      rw [← stopSame, ← outputSame, ← workSame, partition, silent, List.append_nil, reads]
      exact .quota (by simpa only [UScalar.le_equiv] using exhausted)
    next remaining =>
      have room : before.statistics.subsets.val < before.limits.max_subsets.val := by
        simpa only [UScalar.le_equiv, Nat.not_le] using remaining
      obtain ⟨increment, incremented, evaluated⟩ := RuntimeRuns.embedded_bind_inv _ _ _ _ following
      obtain ⟨evaluationEvents, rest, evaluatedResult, evaluationPartition, evaluationRun, roots⟩ :=
        RuntimeRuns.bind_inv _ _ _ _ evaluated
      rcases evaluatedResult with ⟨evaluatedAnswer, values, evaluatedWork⟩
      cases evaluatedAnswer with
      | Err stopped =>
        simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
          core.convert.FromSame.from, embed_ok, Bind.bind, itree_ret_bind] at roots
        obtain ⟨same, silent⟩ := RuntimeRuns.returned_inv _ _ _ roots
        have stopSame : stopped = reason := core.result.Result.Err.inj (congrArg Prod.fst same)
        have outputSame : values = output := congrArg (fun result => result.2.1) same
        have workSame : evaluatedWork = after := congrArg (fun result => result.2.2) same
        rw [← stopSame, ← outputSame, ← workSame, partition, evaluationPartition,
          silent, List.append_nil, reads]
        exact .evaluation increment room incremented evaluationEvents values evaluatedWork stopped evaluationRun
      | Ok success =>
        cases success
        simp only [embed_ok, itree_ret_bind] at roots
        obtain ⟨rootEvents, suffix, rootResult, rootPartition, rootRun, finished⟩ :=
          RuntimeRuns.bind_inv _ _ _ _ roots
        rcases rootResult with ⟨rootAnswer, finalWork⟩
        cases rootAnswer with
        | Err stopped =>
          simp [core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
            core.convert.FromSame.from, embed_ok, Bind.bind, itree_ret_bind] at finished
          obtain ⟨same, silent⟩ := RuntimeRuns.returned_inv _ _ _ finished
          have stopSame : stopped = reason := core.result.Result.Err.inj (congrArg Prod.fst same)
          have outputSame : values = output := congrArg (fun result => result.2.1) same
          have workSame : finalWork = after := congrArg (fun result => result.2.2) same
          rw [← stopSame, ← outputSame, ← workSame, partition, evaluationPartition,
            rootPartition, silent, List.append_nil, reads, ← List.append_assoc]
          exact .roots increment room incremented evaluationEvents rootEvents values evaluatedWork
            finalWork stopped evaluationRun rootRun
        | Ok failed =>
          simp only [embed_ok, itree_ret_bind] at finished
          have impossible := (RuntimeRuns.returned_inv _ _ _ finished).1
          cases impossible

end QueryRefusal
