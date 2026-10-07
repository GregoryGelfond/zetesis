import NativeRootSemantics
import NativeEvaluationAccounting

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract
open Zetesis Ferraris TightEvaluation

/-!
# Actual native frozen queries

The private query evaluates arbitrary tested interpretations against the stored
original mask, then scans the actual roots with the returned work record. The
mask invariant below is established from actual construction in the companion
module; it does not assert candidate modelhood. Typed stops preserve partial
native work, including operand visits in an unpublished final node.
-/
namespace NativeFrozenQuery

/-- The stored mask is the original native truth table of its retained candidate. -/
def Represents (frozen : reduct.FrozenReduct) : Prop :=
  frozen.truth.val = NativeSpecification.values frozen.candidate none
    (NativeTable.rows (NativeExecution.view frozen.candidate.theory))

/-- The producer invariant supplies all mask cells used by native evaluation. -/
theorem mask_coverage (frozen : reduct.FrozenReduct) (represented : Represents frozen) :
    ∀ mask ∈ some frozen.truth.slice,
      (NativeTable.rows (NativeExecution.view frozen.candidate.theory)).length ≤ mask.val.length := by
  intro mask member
  cases member
  change (NativeTable.rows (NativeExecution.view frozen.candidate.theory)).length ≤ frozen.truth.val.length
  rw [represented, NativeSpecification.values_length]

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

/-- Valid paired storage and a covered mask yield a finite actual private query,
even when control or work refuses. Completion of evaluation supplies root extent. -/
theorem execution_exists (frozen : reduct.FrozenReduct) (tested : theory.Interpretation)
    (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (valid : NativeStructure.WellFormed frozen.candidate.theory.value.atoms.val
      (NativeExecution.view frozen.candidate.theory))
    (stored : NativeMembership.Represented tested)
    (covered : ∀ mask ∈ some frozen.truth.slice,
      (NativeTable.rows (NativeExecution.view frozen.candidate.theory)).length ≤ mask.val.length)
    (rootsBounded : ∀ root ∈ frozen.candidate.theory.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view frozen.candidate.theory)).length) :
    ∃ answer output after, Execution frozen tested old before answer output after := by
  obtain ⟨outcome, evaluated, _⟩ := NativeExecution.evaluate_refines
    frozen.candidate.theory tested (some frozen.truth.slice) old before valid stored covered
  cases result : outcome.result with
  | Err reason =>
    have stopped : oracle.evaluate frozen.candidate.theory tested (some frozen.truth.slice) old before =
        ok (.Err reason, outcome.output, outcome.work) := by
      simpa only [result] using evaluated
    exact ⟨.Err reason, outcome.output, outcome.work, .evaluationStopped reason _ _ stopped⟩
  | Ok value =>
    cases value
    have completed : oracle.evaluate frozen.candidate.theory tested (some frozen.truth.slice) old before =
        ok (.Ok (), outcome.output, outcome.work) := by
      simpa only [result] using evaluated
    have outputValues : outcome.output.val = NativeSpecification.values tested
        (some frozen.truth.slice) (NativeTable.rows (NativeExecution.view frozen.candidate.theory)) := by
      exact NativeExecution.completed_values _ _ _ _ _ valid stored covered _ _ completed
    have rootsCovered : ∀ root ∈ frozen.candidate.theory.value.roots.val,
        root.val < outcome.output.slice.val.length := by
      intro root member
      change root.val < outcome.output.val.length
      rw [outputValues, NativeSpecification.values_length]
      exact rootsBounded root member
    obtain ⟨answer, after, scanned, _⟩ := NativeRootScan.failed_root_refines
      frozen.candidate.theory outcome.output.slice outcome.work rootsCovered
    exact ⟨verdict answer, outcome.output, after, .scanned _ _ _ answer completed scanned⟩

/-- Every actual typed private return has the derived evaluation/root execution.
No successful inner call, reference result or available budget is a premise. -/
theorem returned_execution (frozen : reduct.FrozenReduct) (tested : theory.Interpretation)
    (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (valid : NativeStructure.WellFormed frozen.candidate.theory.value.atoms.val
      (NativeExecution.view frozen.candidate.theory))
    (stored : NativeMembership.Represented tested)
    (covered : ∀ mask ∈ some frozen.truth.slice,
      (NativeTable.rows (NativeExecution.view frozen.candidate.theory)).length ≤ mask.val.length)
    (rootsBounded : ∀ root ∈ frozen.candidate.theory.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view frozen.candidate.theory)).length)
    (answer : core.result.Result Bool zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (returned : reduct.FrozenReduct.satisfied_by frozen tested old before = ok (answer, output, after)) :
    Execution frozen tested old before answer output after := by
  obtain ⟨actualAnswer, actualOutput, actualWork, execution⟩ :=
    execution_exists frozen tested old before valid stored covered rootsBounded
  have actual : reduct.FrozenReduct.satisfied_by frozen tested old before =
      ok (actualAnswer, actualOutput, actualWork) := by
    exact execution_returns frozen tested old before _ _ _ execution
  have same : (actualAnswer, actualOutput, actualWork) = (answer, output, after) := by
    exact Result.ok_injective (actual.symm.trans returned)
  cases same
  exact execution

/-- A completed actual private query decides Ferraris reduct satisfaction for
arbitrary J. The original-mask invariant and native evaluator supply truth; the
actual root scan supplies the Boolean, including its first-false meaning. -/
theorem execution_satisfaction (frozen : reduct.FrozenReduct) (tested : theory.Interpretation)
    (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (represented : Represents frozen)
    (valid : NativeStructure.WellFormed frozen.candidate.theory.value.atoms.val
      (NativeExecution.view frozen.candidate.theory))
    (stored : NativeMembership.Represented tested)
    (rootsBounded : ∀ root ∈ frozen.candidate.theory.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view frozen.candidate.theory)).length)
    (answer : core.result.Result Bool zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (execution : Execution frozen tested old before answer output after)
    (accepted : Bool) (completed : answer = .Ok accepted) :
    accepted = true ↔ Models (interpretation (NativeMembership.denotes tested))
      (ReductTheory (interpretation (NativeMembership.denotes frozen.candidate))
        (NativeRootSemantics.assertions frozen.candidate.theory)) := by
  have covered : ∀ mask ∈ some frozen.truth.slice,
      (NativeTable.rows (NativeExecution.view frozen.candidate.theory)).length ≤ mask.val.length := by
    exact mask_coverage frozen represented
  cases execution with
  | evaluationStopped reason output after evaluated => cases completed
  | scanned output middle after scanAnswer evaluated scanned =>
    cases scanAnswer with
    | Err reason => cases completed
    | Ok root =>
      have same : root.isNone = accepted := by exact core.result.Result.Ok.inj completed
      subst accepted
      have ordered : NativeSpecification.Ordered
          (NativeTable.rows (NativeExecution.view frozen.candidate.theory)) := by
        exact NativeTable.ordered _ _ valid
      have outputValues : output.val = (NativeDenotation.meanings
          (NativeTable.rows (NativeExecution.view frozen.candidate.theory))).map
          (fun formula => formulaValue (NativeMembership.denotes tested)
            (Reduct (interpretation (NativeMembership.denotes frozen.candidate)) formula)) := by
        rw [NativeExecution.completed_values _ _ _ _ _ valid stored covered _ _ evaluated]
        exact NativeDenotation.frozen frozen.candidate tested frozen.truth.slice _ ordered
          (NativeDenotation.original_mask frozen.candidate frozen.truth.slice _ ordered represented)
      have rootsCovered : ∀ root ∈ frozen.candidate.theory.value.roots.val,
          root.val < output.slice.val.length := by
        intro root member
        change root.val < output.val.length
        rw [outputValues, List.length_map, NativeDenotation.meanings_length]
        exact rootsBounded root member
      have scannedMeaning : root = none ↔ ∀ root ∈ frozen.candidate.theory.value.roots.val,
          NativeRootScan.truth output.slice root = true := by
        exact NativeRootScan.completed_none_iff _ _ middle after root rootsCovered scanned
      have logical : rootsTrue output.val (frozen.candidate.theory.value.roots.val.map UScalar.val) = true ↔
          Models (interpretation (NativeMembership.denotes tested))
            (ReductTheory (interpretation (NativeMembership.denotes frozen.candidate))
              (NativeRootSemantics.assertions frozen.candidate.theory)) := by
        rw [outputValues]
        exact NativeRootSemantics.frozen_values _ _ _
      calc
        root.isNone = true ↔ root = none := by cases root <;> simp
        _ ↔ _ := scannedMeaning
        _ ↔ rootsTrue output.val (frozen.candidate.theory.value.roots.val.map UScalar.val) = true := by
          simp [rootsTrue, List.all_eq_true, NativeRootScan.truth, List.getD_eq_getElem?_getD]
          rfl
        _ ↔ _ := logical

/-- Actual successful return derives its internal execution and the precise
Ferraris satisfaction verdict; stopped results cannot supply this premise. -/
theorem completed_satisfaction (frozen : reduct.FrozenReduct) (tested : theory.Interpretation)
    (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (represented : Represents frozen)
    (valid : NativeStructure.WellFormed frozen.candidate.theory.value.atoms.val
      (NativeExecution.view frozen.candidate.theory))
    (stored : NativeMembership.Represented tested)
    (rootsBounded : ∀ root ∈ frozen.candidate.theory.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view frozen.candidate.theory)).length)
    (answer : Bool) (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (returned : reduct.FrozenReduct.satisfied_by frozen tested old before = ok (.Ok answer, output, after)) :
    answer = true ↔ Models (interpretation (NativeMembership.denotes tested))
      (ReductTheory (interpretation (NativeMembership.denotes frozen.candidate))
        (NativeRootSemantics.assertions frozen.candidate.theory)) := by
  have execution : Execution frozen tested old before (.Ok answer) output after := by
    exact returned_execution frozen tested old before valid stored (mask_coverage frozen represented)
      rootsBounded (.Ok answer) output after returned
  exact execution_satisfaction frozen tested old before represented valid stored rootsBounded
    (.Ok answer) output after execution answer rfl

/-- A returned query preserves limits, control and subsets, and charges at most
N+E+R. Refused evaluation includes pending operand work; root scanning counts
stored occurrences, including duplicates, until its first false root or stop. -/
theorem returned_work_bound (frozen : reduct.FrozenReduct) (tested : theory.Interpretation)
    (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (valid : NativeStructure.WellFormed frozen.candidate.theory.value.atoms.val
      (NativeExecution.view frozen.candidate.theory))
    (stored : NativeMembership.Represented tested)
    (covered : ∀ mask ∈ some frozen.truth.slice,
      (NativeTable.rows (NativeExecution.view frozen.candidate.theory)).length ≤ mask.val.length)
    (rootsBounded : ∀ root ∈ frozen.candidate.theory.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view frozen.candidate.theory)).length)
    (answer : core.result.Result Bool zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (returned : reduct.FrozenReduct.satisfied_by frozen tested old before = ok (answer, output, after)) :
    after.limits = before.limits ∧ after.cancellation = before.cancellation ∧
      after.statistics.subsets = before.statistics.subsets ∧
      before.statistics.work.val ≤ after.statistics.work.val ∧
      after.statistics.work.val ≤ before.statistics.work.val +
        NativeSpecification.cost (NativeTable.rows (NativeExecution.view frozen.candidate.theory)) +
        frozen.candidate.theory.value.roots.val.length := by
  have execution : Execution frozen tested old before answer output after := by
    exact returned_execution frozen tested old before valid stored covered rootsBounded
      answer output after returned
  cases execution with
  | evaluationStopped reason output after evaluated =>
    obtain ⟨limits, control, subsets, monotone, bound⟩ := NativeEvaluationAccounting.returned_work_bound
      _ _ _ _ _ valid stored covered (.Err reason) output after evaluated
    exact ⟨limits, control, subsets, monotone, by omega⟩
  | scanned output middle after answer evaluated scanned =>
    obtain ⟨limits, control, subsets, monotone, bound⟩ := NativeEvaluationAccounting.returned_work_bound
      _ _ _ _ _ valid stored covered (.Ok ()) output middle evaluated
    have outputValues : output.val = NativeSpecification.values tested
        (some frozen.truth.slice) (NativeTable.rows (NativeExecution.view frozen.candidate.theory)) := by
      exact NativeExecution.completed_values _ _ _ _ _ valid stored covered _ _ evaluated
    have rootsCovered : ∀ root ∈ frozen.candidate.theory.value.roots.val,
        root.val < output.slice.val.length := by
      intro root member
      change root.val < output.val.length
      rw [outputValues, NativeSpecification.values_length]
      exact rootsBounded root member
    obtain ⟨frame, rootMonotone, rootBound⟩ := NativeRootScan.returned_work_bound
      _ output.slice middle after answer rootsCovered scanned
    exact ⟨frame.1.trans limits, frame.2.1.trans control, frame.2.2.trans subsets,
      by omega, by omega⟩

end NativeFrozenQuery
