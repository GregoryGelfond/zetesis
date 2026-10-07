import NativeControl
import NativeRootSemantics
import NativeEvaluationAccounting

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract

/-!
# One actual subset query

The generated `check_subset` polls, admits one subset against its quota, and
shares its work record between evaluation and root scanning. A completed return
establishes those admission facts; they are not supplied as success assumptions.
The semantic endpoint uses an actual original evaluation to obtain the frozen
mask and proves that the returned Boolean decides its Ferraris reduct.

This is satisfaction of one tested interpretation, not proof that it is a proper
subset or that the search has covered every proper subset. The existing fixed
observation, scalar and sequence-model boundaries remain. Owner checks,
reservation and the complete public membership wrapper are separate obligations.
-/
namespace NativeSubsetQuery

/-- A completed subset query passed its initial control poll and had room in the
    subset quota. The proof unfolds the actual admission branches: either refusal
    would return a typed error, contradicting the supplied completed result. -/
theorem completed_admission (program : theory.Theory) (tested : theory.Interpretation)
    (frozen : Slice Bool) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (accepted : Bool) (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (completed : oracle.check_subset program tested frozen old before =
      ok (core.result.Result.Ok accepted, output, after)) :
    NativeControl.observation before.cancellation = none ∧
      before.statistics.subsets.val < before.limits.max_subsets.val := by
  cases observed : NativeControl.observation before.cancellation with
  | some reason =>
      simp [oracle.check_subset, NativeControl.poll_exact, observed,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
        at completed
  | none =>
      have quota : before.statistics.subsets.val < before.limits.max_subsets.val := by
        by_contra exhausted
        have refused : before.limits.max_subsets.val ≤ before.statistics.subsets.val := by
          exact Nat.le_of_not_gt exhausted
        simp [oracle.check_subset, NativeControl.poll_exact, observed,
          core.result.Result.Insts.CoreOpsTry.branch, UScalar.le_equiv, refused] at completed
      exact ⟨rfl, quota⟩

/-- An actual completed subset query contains a completed evaluation followed by
    a completed root scan with the returned work record. Its admission increments
    the subset count once. No inner call equation is assumed.

    Proof: derive quota room from completion, construct the checked increment,
    and use the existing evaluator and root-scan termination laws. Refused inner
    outcomes contradict completion; unique actual results identify the returned
    output, final work record and Boolean. -/
theorem completed_phases (program : theory.Theory) (tested : theory.Interpretation)
    (frozen : Slice Bool) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (stored : NativeMembership.Represented tested)
    (valid : NativeStructure.WellFormed program.value.atoms.val (NativeExecution.view program))
    (covered : (NativeTable.rows (NativeExecution.view program)).length ≤ frozen.val.length)
    (rootsBounded : ∀ root ∈ program.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view program)).length)
    (accepted : Bool) (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (completed : oracle.check_subset program tested frozen old before =
      ok (core.result.Result.Ok accepted, output, after)) :
    ∃ charged : U64, ∃ middle : oracle.Work, ∃ failed : Option Usize,
      charged.val = before.statistics.subsets.val + 1 ∧
      oracle.evaluate program tested (some frozen) old
        { before with statistics := { before.statistics with subsets := charged } } =
          ok (core.result.Result.Ok (), output, middle) ∧
      oracle.failed_root program output.slice middle =
        ok (core.result.Result.Ok failed, after) ∧
      accepted = failed.isNone := by
  obtain ⟨clear, quota⟩ := completed_admission program tested frozen old before
    accepted output after completed
  have incrementFits : before.statistics.subsets.val + (1#u64).val ≤ U64.max := by
    have limitFits : before.limits.max_subsets.val ≤ U64.max := by
      simpa only [U64.max_eq] using U64.le_max before.limits.max_subsets
    change before.statistics.subsets.val + 1 ≤ U64.max
    omega
  obtain ⟨charged, increment, chargedCount⟩ := WP.spec_imp_exists
    (U64.add_spec (x := before.statistics.subsets) (y := 1#u64) incrementFits)
  have countExact : charged.val = before.statistics.subsets.val + 1 := by
    simpa using chargedCount
  let admitted : oracle.Work :=
    { before with statistics := { before.statistics with subsets := charged } }
  have maskCovered : ∀ mask ∈ some frozen,
      (NativeTable.rows (NativeExecution.view program)).length ≤ mask.val.length := by
    intro mask member
    cases member
    exact covered
  have sliceView (values : alloc.vec.Vec Bool) : alloc.vec.Vec.deref values = values.slice := by
    apply Slice.ext
    simp [alloc.vec.Vec.deref, alloc.vec.Vec.val]
  obtain ⟨outcome, evaluated, _⟩ := NativeExecution.evaluate_refines
    program tested (some frozen) old admitted valid stored maskCovered
  cases evaluationResult : outcome.result with
  | Err reason =>
      have stopped : oracle.evaluate program tested (some frozen) old admitted =
          ok (core.result.Result.Err reason, outcome.output, outcome.work) := by
        simpa only [evaluationResult] using evaluated
      dsimp only [admitted] at stopped
      simp [oracle.check_subset, NativeControl.poll_exact, clear,
        core.result.Result.Insts.CoreOpsTry.branch, UScalar.le_equiv,
        Nat.not_le.mpr quota, increment, stopped,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
        at completed
  | Ok value =>
      cases value
      have evaluationComplete : oracle.evaluate program tested (some frozen) old admitted =
          ok (core.result.Result.Ok (), outcome.output, outcome.work) := by
        simpa only [evaluationResult] using evaluated
      dsimp only [admitted] at evaluationComplete
      have outputValues : outcome.output.val = NativeSpecification.values tested (some frozen)
          (NativeTable.rows (NativeExecution.view program)) := by
        exact NativeExecution.completed_values program tested (some frozen) old admitted
          valid stored maskCovered outcome.output outcome.work evaluationComplete
      have rootsCovered : ∀ root ∈ program.value.roots.val,
          root.val < outcome.output.slice.val.length := by
        intro root member
        change root.val < outcome.output.val.length
        rw [outputValues, NativeSpecification.values_length]
        exact rootsBounded root member
      obtain ⟨rootAnswer, finalWork, scanned, _⟩ := NativeRootScan.failed_root_refines
        program outcome.output.slice outcome.work rootsCovered
      cases rootAnswer with
      | Err reason =>
          simp [oracle.check_subset, NativeControl.poll_exact, clear,
            core.result.Result.Insts.CoreOpsTry.branch, UScalar.le_equiv,
            Nat.not_le.mpr quota, increment, evaluationComplete,
            sliceView, scanned,
            core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
            at completed
      | Ok failed =>
          have returned : oracle.check_subset program tested frozen old before =
              ok (core.result.Result.Ok failed.isNone, outcome.output, finalWork) := by
            simp [oracle.check_subset, NativeControl.poll_exact, clear,
              core.result.Result.Insts.CoreOpsTry.branch, UScalar.le_equiv,
              Nat.not_le.mpr quota, increment, evaluationComplete,
              sliceView, scanned, core.option.Option.is_none]
          have same : ((core.result.Result.Ok failed.isNone :
              core.result.Result Bool zetesis_cpu.cancellation.Stop), outcome.output, finalWork) =
              (core.result.Result.Ok accepted, output, after) := by
            exact Result.ok_injective (returned.symm.trans completed)
          have sameOutput : outcome.output = output := by
            exact congrArg (fun value => value.2.1) same
          have sameWork : finalWork = after := by
            exact congrArg (fun value => value.2.2) same
          have sameAnswer : failed.isNone = accepted := by
            exact core.result.Result.Ok.inj (congrArg Prod.fst same)
          refine ⟨charged, outcome.work, failed, countExact, ?_, ?_, sameAnswer.symm⟩
          · simpa only [sameOutput] using evaluationComplete
          · simpa only [sameOutput, sameWork] using scanned

/-- If the mask is the actual result of a successful original evaluation, a
    completed subset query returns true exactly when the tested interpretation
    satisfies that Ferraris reduct. Mask meaning and coverage are derived from
    the first call. Neither original modelhood nor a subset relation is assumed.

    Proof: recover the actual query phases, then apply the two-evaluation and
    root-scan satisfaction theorem to their shared intermediate work records. -/
theorem completed_reduct (program : theory.Theory) (outer tested : theory.Interpretation)
    (oldOriginal oldTested : alloc.vec.Vec Bool) (outerWork before : oracle.Work)
    (outerStored : NativeMembership.Represented outer) (testedStored : NativeMembership.Represented tested)
    (valid : NativeStructure.WellFormed program.value.atoms.val (NativeExecution.view program))
    (rootsBounded : ∀ root ∈ program.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view program)).length)
    (original output : alloc.vec.Vec Bool) (outerReturned after : oracle.Work) (accepted : Bool)
    (originalComplete : oracle.evaluate program outer none oldOriginal outerWork =
      ok (core.result.Result.Ok (), original, outerReturned))
    (completed : oracle.check_subset program tested original.slice oldTested before =
      ok (core.result.Result.Ok accepted, output, after)) :
    accepted = true ↔ Zetesis.Ferraris.Models
      (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes tested))
      (Zetesis.Ferraris.ReductTheory
        (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes outer))
        (NativeRootSemantics.assertions program)) := by
  have originalValues : original.val =
      NativeSpecification.values outer none (NativeTable.rows (NativeExecution.view program)) := by
    exact NativeExecution.completed_values program outer none oldOriginal outerWork valid outerStored
      (by simp) original outerReturned originalComplete
  have covered : (NativeTable.rows (NativeExecution.view program)).length ≤ original.slice.val.length := by
    change (NativeTable.rows (NativeExecution.view program)).length ≤ original.val.length
    rw [originalValues, NativeSpecification.values_length]
  obtain ⟨charged, middle, failed, _, evaluated, scanned, answerMeaning⟩ :=
    completed_phases program tested original.slice oldTested before testedStored valid
      covered rootsBounded accepted output after completed
  have evaluatedView : oracle.evaluate program tested (some (alloc.vec.Vec.deref original)) oldTested
      { before with statistics := { before.statistics with subsets := charged } } =
        ok (.Ok (), output, middle) := by
    simpa [alloc.vec.Vec.deref, alloc.vec.Vec.val] using evaluated
  have exactMeaning : failed = none ↔ Zetesis.Ferraris.Models
      (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes tested))
      (Zetesis.Ferraris.ReductTheory
        (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes outer))
        (NativeRootSemantics.assertions program)) := by
    exact NativeRootSemantics.completed_reduct program outer tested
      oldOriginal oldTested outerWork
      { before with statistics := { before.statistics with subsets := charged } } middle
      valid outerStored testedStored rootsBounded original output outerReturned middle after failed
      originalComplete evaluatedView scanned
  calc
    accepted = true ↔ failed = none := by rw [answerMeaning]; cases failed <;> simp
    _ ↔ _ := exactMeaning

/-- Every completed query charges exactly one subset, including a completed
    false result. Evaluation and root scanning retain that admitted count; the
    source quota check and checked increment cannot silently wrap. This does
    not claim that a typed refusal always charges a subset. -/
theorem completed_subset_count (program : theory.Theory) (tested : theory.Interpretation)
    (frozen : Slice Bool) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (stored : NativeMembership.Represented tested)
    (valid : NativeStructure.WellFormed program.value.atoms.val (NativeExecution.view program))
    (covered : (NativeTable.rows (NativeExecution.view program)).length ≤ frozen.val.length)
    (rootsBounded : ∀ root ∈ program.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view program)).length)
    (accepted : Bool) (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (completed : oracle.check_subset program tested frozen old before =
      ok (core.result.Result.Ok accepted, output, after)) :
    after.statistics.subsets.val = before.statistics.subsets.val + 1 := by
  obtain ⟨charged, middle, failed, countExact, evaluated, scanned, _⟩ :=
    completed_phases program tested frozen old before stored valid covered rootsBounded
      accepted output after completed
  let admitted : oracle.Work :=
    { before with statistics := { before.statistics with subsets := charged } }
  have maskCovered : ∀ mask ∈ some frozen,
      (NativeTable.rows (NativeExecution.view program)).length ≤ mask.val.length := by
    intro mask member
    cases member
    exact covered
  obtain ⟨_, _, subsets, _, _⟩ := NativeEvaluationAccounting.returned_work_bound
    program tested (some frozen) old admitted valid stored maskCovered (.Ok ()) output middle evaluated
  have outputValues : output.val = NativeSpecification.values tested (some frozen)
      (NativeTable.rows (NativeExecution.view program)) := by
    exact NativeExecution.completed_values program tested (some frozen) old admitted
      valid stored maskCovered output middle evaluated
  have rootsCovered : ∀ root ∈ program.value.roots.val, root.val < output.slice.val.length := by
    intro root member
    change root.val < output.val.length
    rw [outputValues, NativeSpecification.values_length]
    exact rootsBounded root member
  obtain ⟨frame, _, _⟩ := NativeRootScan.returned_work_bound program output.slice middle after
    (.Ok failed) rootsCovered scanned
  have preservedCount : after.statistics.subsets = charged := by
    exact frame.2.2.trans subsets
  exact (congrArg UScalar.val preservedCount).trans countExact

end NativeSubsetQuery
