import TheorySatisfaction
import EvaluationAccounting

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# The stored-reduct query

The actual generated private query evaluates the tested interpretation with the
stored mask, then checks the roots using the returned work record. The mask must
represent original truth for the stored candidate. That agreement is explicit:
the extracted record type alone does not establish its producer invariant.

These proofs cover the private `satisfied_by` operation under the existing fixed
observation and sequence models. Construction, fallible reservation, public owner
checks and concurrent observations remain separate obligations. In particular,
the record formed from a proved original evaluation is not a proof of `freeze`.
-/
namespace FrozenQuery

/-- The stored mask is original truth for the stored candidate and its theory.
This says nothing about original satisfaction, minimality or runtime ownership. -/
def Represents (frozen : reduct.FrozenReduct) : Prop :=
  frozen.truth.val = EvaluationSpecification.values frozen.candidate none
    frozen.candidate.theory.value.nodes.val

/-- Actual completed original evaluation supplies the mask agreement for the
corresponding record. This does not model reservation or the `freeze` constructor.
-/
theorem represents_from_evaluation (candidate : theory.Interpretation)
    (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered candidate.theory.value.nodes.val)
    (truth : alloc.vec.Vec Bool) (after : oracle.Work)
    (evaluated : oracle.evaluate candidate.theory candidate none old before =
      ok (core.result.Result.Ok (), truth, after)) :
    Represents ⟨candidate, truth⟩ := by
  exact FixedEvaluationLoop.completed_values candidate.theory candidate none old before
    stored ordered (by intro mask member; cases member) truth after evaluated

/-- Original truth has one value per stored node, so the query mask covers the
whole table. Its extent follows from agreement rather than a second assumption.
-/
theorem mask_coverage (frozen : reduct.FrozenReduct) (represented : Represents frozen) :
    ∀ mask ∈ some frozen.truth.slice,
      frozen.candidate.theory.value.nodes.val.length ≤ mask.val.length := by
  intro mask member
  cases member
  change frozen.candidate.theory.value.nodes.val.length ≤ frozen.truth.val.length
  rw [represented, EvaluationSpecification.values_length]

/-- The query converts only a completed root-scan option to a Boolean. A typed
stop remains that same stop, rather than becoming false. -/
def verdict : core.result.Result (Option Usize) zetesis_cpu.cancellation.Stop →
    core.result.Result Bool zetesis_cpu.cancellation.Stop
  | .Ok root => .Ok root.isNone
  | .Err reason => .Err reason

/-- The two actual phases of the private query. Root scanning starts with the
work returned by successful evaluation. An evaluation stop has no root phase.
This records generated call equations, not assumed semantic truth. -/
inductive Execution (frozen : reduct.FrozenReduct) (tested : theory.Interpretation)
    (old : alloc.vec.Vec Bool) (before : oracle.Work) :
    core.result.Result Bool zetesis_cpu.cancellation.Stop → alloc.vec.Vec Bool →
      oracle.Work → Prop where
  | evaluationStopped (reason : zetesis_cpu.cancellation.Stop)
      (output : alloc.vec.Vec Bool) (after : oracle.Work)
      (evaluated : oracle.evaluate frozen.candidate.theory tested
        (some frozen.truth.slice) old before = ok (.Err reason, output, after)) :
      Execution frozen tested old before (.Err reason) output after
  | scanned (output : alloc.vec.Vec Bool) (middle after : oracle.Work)
      (answer : core.result.Result (Option Usize) zetesis_cpu.cancellation.Stop)
      (evaluated : oracle.evaluate frozen.candidate.theory tested
        (some frozen.truth.slice) old before = ok (.Ok (), output, middle))
      (scanned : oracle.failed_root frozen.candidate.theory output.slice middle =
        ok (answer, after)) :
      Execution frozen tested old before (verdict answer) output after

/-- An execution of these two phases is the result of the unchanged generated
private query. Unfold its accessor, source-result branches and Boolean selector;
substitute the actual calls without resetting the returned work record. -/
theorem execution_returns (frozen : reduct.FrozenReduct)
    (tested : theory.Interpretation) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (answer : core.result.Result Bool zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (execution : Execution frozen tested old before answer output after) :
    reduct.FrozenReduct.satisfied_by frozen tested old before = ok (answer, output, after) := by
  have sliceView (values : alloc.vec.Vec Bool) : alloc.vec.Vec.deref values = values.slice := by
    apply Slice.ext
    simp [alloc.vec.Vec.deref, alloc.vec.Vec.val]
  cases execution with
  | evaluationStopped reason output after evaluated =>
    simp [reduct.FrozenReduct.satisfied_by, reduct.FrozenReduct.theory,
      theory.Interpretation.impl.theory, sliceView, evaluated,
      core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
  | scanned output middle after answer evaluated scanned =>
    cases answer <;>
      simp [reduct.FrozenReduct.satisfied_by, reduct.FrozenReduct.theory,
        theory.Interpretation.impl.theory, sliceView, evaluated, scanned,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
        verdict, core.option.Option.is_none]

/-- Admitted storage, an ordered table and a covered mask give a finite actual
query execution. A stopped evaluation returns immediately. Otherwise its proved
length covers the admitted roots, and the actual root scan returns a typed result.
No successful query or available-work premise is required. -/
theorem execution_exists (frozen : reduct.FrozenReduct)
    (tested : theory.Interpretation) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (stored : Membership.Represented tested)
    (ordered : EvaluationSpecification.Ordered frozen.candidate.theory.value.nodes.val)
    (covered : ∀ mask ∈ some frozen.truth.slice,
      frozen.candidate.theory.value.nodes.val.length ≤ mask.val.length)
    (rootsBounded : ∀ root ∈ frozen.candidate.theory.value.roots.val,
      root.val < frozen.candidate.theory.value.nodes.val.length) :
    ∃ answer, ∃ output, ∃ after, Execution frozen tested old before answer output after := by
  obtain ⟨_, outcome, evaluated, _, _⟩ := FixedEvaluationLoop.evaluate_refines
    frozen.candidate.theory tested (some frozen.truth.slice) old before stored ordered covered
  cases sourceResult : outcome.result with
  | Err reason =>
    have stopped : oracle.evaluate frozen.candidate.theory tested
        (some frozen.truth.slice) old before = ok (.Err reason, outcome.output, outcome.work) := by
      simpa only [sourceResult] using evaluated
    exact ⟨.Err reason, outcome.output, outcome.work,
      Execution.evaluationStopped reason outcome.output outcome.work stopped⟩
  | Ok value =>
    cases value
    have completed : oracle.evaluate frozen.candidate.theory tested
        (some frozen.truth.slice) old before = ok (.Ok (), outcome.output, outcome.work) := by
      simpa only [sourceResult] using evaluated
    have receipt := EvaluationAccounting.returned_receipt frozen.candidate.theory tested
      (some frozen.truth.slice) old before stored ordered covered (.Ok ())
      outcome.output outcome.work completed
    have rootsCovered : ∀ root ∈ frozen.candidate.theory.value.roots.val,
        root.val < outcome.output.slice.val.length := by
      intro root member
      change root.val < outcome.output.val.length
      rw [receipt.boundary]
      exact rootsBounded root member
    obtain ⟨answer, after, scanned, _⟩ := FixedRootScan.failed_root_refines
      frozen.candidate.theory outcome.output.slice outcome.work rootsCovered
    exact ⟨verdict answer, outcome.output, after,
      Execution.scanned outcome.output outcome.work after answer completed scanned⟩

/-- Every actual typed query result has the two-phase execution above. Identify
the supplied return with the constructed generated result; no phase correctness
or successful inner call is required from the caller. -/
theorem returned_execution (frozen : reduct.FrozenReduct)
    (tested : theory.Interpretation) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (stored : Membership.Represented tested)
    (ordered : EvaluationSpecification.Ordered frozen.candidate.theory.value.nodes.val)
    (covered : ∀ mask ∈ some frozen.truth.slice,
      frozen.candidate.theory.value.nodes.val.length ≤ mask.val.length)
    (rootsBounded : ∀ root ∈ frozen.candidate.theory.value.roots.val,
      root.val < frozen.candidate.theory.value.nodes.val.length)
    (answer : core.result.Result Bool zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (returned : reduct.FrozenReduct.satisfied_by frozen tested old before =
      ok (answer, output, after)) :
    Execution frozen tested old before answer output after := by
  obtain ⟨actualAnswer, actualOutput, actualWork, execution⟩ :=
    execution_exists frozen tested old before stored ordered covered rootsBounded
  have executed := execution_returns frozen tested old before actualAnswer actualOutput
    actualWork execution
  have same : (actualAnswer, actualOutput, actualWork) = (answer, output, after) :=
    Result.ok_injective (executed.symm.trans returned)
  cases same
  exact execution

/-- A completed private query returns true exactly when the tested interpretation
models the Ferraris reduct fixed by the stored candidate. Neither original
modelhood of that candidate nor a tested-subset relation is required.

Proof: success rules out both typed-stop branches of the execution. Evaluation
computes frozen truth using the represented mask; its length justifies the root
reads. The actual scan and the general ASP root law identify the Boolean.
-/
theorem execution_satisfaction (frozen : reduct.FrozenReduct)
    (tested : theory.Interpretation) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (represented : Represents frozen) (stored : Membership.Represented tested)
    (ordered : EvaluationSpecification.Ordered frozen.candidate.theory.value.nodes.val)
    (rootsBounded : ∀ root ∈ frozen.candidate.theory.value.roots.val,
      root.val < frozen.candidate.theory.value.nodes.val.length)
    (answer : core.result.Result Bool zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (execution : Execution frozen tested old before answer output after)
    (accepted : Bool) (completed : answer = .Ok accepted) :
    accepted = true ↔ Zetesis.Ferraris.Models
      (Zetesis.TightEvaluation.interpretation (Membership.denotes tested))
      (Zetesis.Ferraris.ReductTheory
        (Zetesis.TightEvaluation.interpretation (Membership.denotes frozen.candidate))
        (RootSemantics.assertions frozen.candidate.theory)) := by
  have covered := mask_coverage frozen represented
  cases execution with
  | evaluationStopped reason output after evaluated => cases completed
  | scanned output middle after scanAnswer evaluated scanned =>
    cases scanAnswer with
    | Err reason => cases completed
    | Ok root =>
      have same : root.isNone = accepted := core.result.Result.Ok.inj completed
      subst accepted
      have outputValues : output.val = Zetesis.ReductEvaluation.values
          (Membership.denotes frozen.candidate) (Membership.denotes tested)
          (frozen.candidate.theory.value.nodes.val.map EvaluationSemantics.node) := by
        rw [FixedEvaluationLoop.completed_values frozen.candidate.theory tested
          (some frozen.truth.slice) old before stored ordered covered output middle evaluated]
        exact EvaluationSemantics.frozen_values frozen.candidate tested frozen.truth.slice
          frozen.candidate.theory.value.nodes.val represented
      have outputLength : output.val.length = frozen.candidate.theory.value.nodes.val.length := by
        rw [outputValues, Zetesis.ReductEvaluation.values_length, List.length_map]
      have rootsCovered : ∀ root ∈ frozen.candidate.theory.value.roots.val,
          root.val < output.slice.val.length := by
        intro root member
        change root.val < output.val.length
        rw [outputLength]
        exact rootsBounded root member
      have scanMeaning := FixedRootScan.completed_none_iff frozen.candidate.theory
        output.slice middle after root rootsCovered scanned
      have logical : Zetesis.TightEvaluation.rootsTrue output.val
          (frozen.candidate.theory.value.roots.val.map (fun root => root.val)) = true ↔
          Zetesis.Ferraris.Models
            (Zetesis.TightEvaluation.interpretation (Membership.denotes tested))
            (Zetesis.Ferraris.ReductTheory
              (Zetesis.TightEvaluation.interpretation (Membership.denotes frozen.candidate))
              (RootSemantics.assertions frozen.candidate.theory)) := by
        rw [outputValues]
        exact Zetesis.ReductEvaluation.roots_true_iff _ _ _ _
      calc
        root.isNone = true ↔ root = none := by cases root <;> simp
        _ ↔ _ := scanMeaning
        _ ↔ Zetesis.TightEvaluation.rootsTrue output.val
            (frozen.candidate.theory.value.roots.val.map (fun root => root.val)) = true := by
          simp [Zetesis.TightEvaluation.rootsTrue, List.all_eq_true, FixedRootScan.truth,
            List.getD_eq_getElem?_getD]
          rfl
        _ ↔ _ := logical

/-- An actual successful private query decides the represented Ferraris reduct.
The single query return determines both internal calls and their shared work;
semantic truth is derived from those calls and the stored-mask invariant. -/
theorem completed_satisfaction (frozen : reduct.FrozenReduct)
    (tested : theory.Interpretation) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (represented : Represents frozen) (stored : Membership.Represented tested)
    (ordered : EvaluationSpecification.Ordered frozen.candidate.theory.value.nodes.val)
    (rootsBounded : ∀ root ∈ frozen.candidate.theory.value.roots.val,
      root.val < frozen.candidate.theory.value.nodes.val.length)
    (answer : Bool) (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (returned : reduct.FrozenReduct.satisfied_by frozen tested old before =
      ok (core.result.Result.Ok answer, output, after)) :
    answer = true ↔ Zetesis.Ferraris.Models
      (Zetesis.TightEvaluation.interpretation (Membership.denotes tested))
      (Zetesis.Ferraris.ReductTheory
        (Zetesis.TightEvaluation.interpretation (Membership.denotes frozen.candidate))
        (RootSemantics.assertions frozen.candidate.theory)) := by
  have execution := returned_execution frozen tested old before stored ordered
    (mask_coverage frozen represented) rootsBounded (.Ok answer) output after returned
  exact execution_satisfaction frozen tested old before represented stored ordered rootsBounded
    (.Ok answer) output after execution answer rfl

/-- Both phases retain limits and subset statistics and together charge at most
the node count plus the root count. The bound includes typed stops and repeated
root occurrences. It concerns logical work, not physical storage or elapsed time.

Proof: an evaluation stop uses its evaluator receipt alone. Otherwise completed
evaluation establishes root coverage; add its charged nodes to the actual scan's
bound using the same intermediate work record.
-/
theorem execution_work_bound (frozen : reduct.FrozenReduct)
    (tested : theory.Interpretation) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (stored : Membership.Represented tested)
    (ordered : EvaluationSpecification.Ordered frozen.candidate.theory.value.nodes.val)
    (covered : ∀ mask ∈ some frozen.truth.slice,
      frozen.candidate.theory.value.nodes.val.length ≤ mask.val.length)
    (rootsBounded : ∀ root ∈ frozen.candidate.theory.value.roots.val,
      root.val < frozen.candidate.theory.value.nodes.val.length)
    (answer : core.result.Result Bool zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (execution : Execution frozen tested old before answer output after) :
    after.limits = before.limits ∧
      after.statistics.subsets = before.statistics.subsets ∧
      before.statistics.work.val ≤ after.statistics.work.val ∧
      after.statistics.work.val ≤ before.statistics.work.val +
        frozen.candidate.theory.value.nodes.val.length +
        frozen.candidate.theory.value.roots.val.length := by
  cases execution with
  | evaluationStopped reason output after evaluated =>
    have receipt := EvaluationAccounting.returned_receipt frozen.candidate.theory tested
      (some frozen.truth.slice) old before stored ordered covered (.Err reason)
      output after evaluated
    refine ⟨receipt.limits, receipt.subsets, ?_, ?_⟩
    · have counted := receipt.work
      omega
    · have counted := receipt.work
      have bounded := receipt.bounded
      omega
  | scanned output middle after answer evaluated scanned =>
    have receipt := EvaluationAccounting.returned_receipt frozen.candidate.theory tested
      (some frozen.truth.slice) old before stored ordered covered (.Ok ())
      output middle evaluated
    have rootsCovered : ∀ root ∈ frozen.candidate.theory.value.roots.val,
        root.val < output.slice.val.length := by
      intro root member
      change root.val < output.val.length
      rw [receipt.boundary]
      exact rootsBounded root member
    obtain ⟨frame, monotone, bounded⟩ := FixedRootScan.returned_work_bound
      frozen.candidate.theory output.slice middle after answer rootsCovered scanned
    refine ⟨frame.1.trans receipt.limits, frame.2.2.trans receipt.subsets, ?_, ?_⟩
    · have counted := receipt.work
      omega
    · have counted := receipt.work
      have full := receipt.boundary
      omega

/-- An actual typed query result obeys the composed work bound, including when
either phase stops. The supplied result equation identifies the actual outcome;
the generated phases and their accounting are derived from it. -/
theorem returned_work_bound (frozen : reduct.FrozenReduct)
    (tested : theory.Interpretation) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (stored : Membership.Represented tested)
    (ordered : EvaluationSpecification.Ordered frozen.candidate.theory.value.nodes.val)
    (covered : ∀ mask ∈ some frozen.truth.slice,
      frozen.candidate.theory.value.nodes.val.length ≤ mask.val.length)
    (rootsBounded : ∀ root ∈ frozen.candidate.theory.value.roots.val,
      root.val < frozen.candidate.theory.value.nodes.val.length)
    (answer : core.result.Result Bool zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (returned : reduct.FrozenReduct.satisfied_by frozen tested old before =
      ok (answer, output, after)) :
    after.limits = before.limits ∧
      after.statistics.subsets = before.statistics.subsets ∧
      before.statistics.work.val ≤ after.statistics.work.val ∧
      after.statistics.work.val ≤ before.statistics.work.val +
        frozen.candidate.theory.value.nodes.val.length +
        frozen.candidate.theory.value.roots.val.length := by
  exact execution_work_bound frozen tested old before stored ordered covered rootsBounded
    answer output after (returned_execution frozen tested old before stored ordered covered
      rootsBounded answer output after returned)

end FrozenQuery
