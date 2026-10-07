import NativeSubsetQuery

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisNativeExtract

/-!
# Total typed outcomes of an actual subset query

The generated subset query either refuses admission or evaluates and scans roots
using one cumulative work record. These statements construct its actual typed
return under represented storage and covered reads. They do not assume successful
evaluation, successful root scanning, or sufficient work and subset allowances.

Control preservation refers to the existing fixed observation-token model. No
allocator, owner-identity or changing concurrent-atomic correspondence is added.
The queried interpretation and frozen mask remain immutable supplied inputs;
their reduct meaning is established separately by `NativeSubsetQuery.completed_reduct`.
-/
namespace NativeSubsetQueryTotal

/-- The source admits a subset exactly when its initial poll is clear and its
    subset count is strictly below the stored ceiling. Work admission occurs
    later, once the query has already charged this subset. -/
def admitted (before : oracle.Work) : Bool :=
  (NativeControl.observation before.cancellation).isNone &&
    decide (before.statistics.subsets.val < before.limits.max_subsets.val)

/-- Every typed query return preserves limits and controls. Admission charges
    exactly one subset even if evaluation or roots subsequently stop. Before
    admission, refusal preserves the old output and the entire work record.
    Logical work grows by at most N+E+R, including work inside a refused native node. -/
structure Receipt (program : theory.Theory) (old : alloc.vec.Vec Bool)
    (before : oracle.Work) (output : alloc.vec.Vec Bool) (after : oracle.Work) : Prop where
  limits : after.limits = before.limits
  control : after.cancellation = before.cancellation
  subsets : after.statistics.subsets.val = before.statistics.subsets.val +
    if admitted before then 1 else 0
  workLower : before.statistics.work.val ≤ after.statistics.work.val
  workUpper : after.statistics.work.val ≤ before.statistics.work.val +
    NativeSpecification.cost (NativeTable.rows (NativeExecution.view program)) + program.value.roots.val.length
  uncharged : admitted before = false → output = old ∧ after = before

/-- The actual generated subset query returns either a completed Boolean or a
    typed refusal, with its precise admission receipt. No inner-call success is
    assumed. Represented storage, ordered nodes and covered mask/root reads rule
    out backend read failures; actual control and resource branches remain.

    Proof: classify initial control and quota admission; derive the checked count
    increment from the quota; construct the actual evaluator return; on success,
    construct the root scan using the proved full output length. Compose their
    work receipts without charging either phase twice. -/
theorem query_refines (program : theory.Theory) (tested : theory.Interpretation)
    (frozen : Slice Bool) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (stored : NativeMembership.Represented tested)
    (valid : NativeStructure.WellFormed program.value.atoms.val (NativeExecution.view program))
    (covered : (NativeTable.rows (NativeExecution.view program)).length ≤ frozen.val.length)
    (rootsBounded : ∀ root ∈ program.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view program)).length) :
    ∃ result : core.result.Result Bool zetesis_cpu.cancellation.Stop,
      ∃ output : alloc.vec.Vec Bool, ∃ after : oracle.Work,
        oracle.check_subset program tested frozen old before = ok (result, output, after) ∧
        Receipt program old before output after := by
  cases observed : NativeControl.observation before.cancellation with
  | some reason =>
      refine ⟨.Err reason, old, before, ?_, ?_⟩
      · simp [oracle.check_subset, NativeControl.poll_exact, observed,
          core.result.Result.Insts.CoreOpsTry.branch,
          core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
      · refine ⟨rfl, rfl, ?_, Nat.le_refl _, ?_, ?_⟩
        · simp [admitted, observed]
        · omega
        · intro _
          exact ⟨rfl, rfl⟩
  | none =>
      by_cases quota : before.statistics.subsets.val < before.limits.max_subsets.val
      · have incrementFits : before.statistics.subsets.val + (1#u64).val ≤ U64.max := by
          have limitFits : before.limits.max_subsets.val ≤ U64.max := by
            simpa only [U64.max_eq] using U64.le_max before.limits.max_subsets
          change before.statistics.subsets.val + 1 ≤ U64.max
          omega
        obtain ⟨charged, increment, countExact⟩ := WP.spec_imp_exists
          (U64.add_spec (x := before.statistics.subsets) (y := 1#u64) incrementFits)
        have countValue : charged.val = before.statistics.subsets.val + 1 := by
          simpa using countExact
        let started : oracle.Work :=
          { before with statistics := { before.statistics with subsets := charged } }
        have wasAdmitted : admitted before = true := by simp [admitted, observed, quota]
        have maskCovered : ∀ mask ∈ some frozen,
            (NativeTable.rows (NativeExecution.view program)).length ≤ mask.val.length := by
          intro mask member
          cases member
          exact covered
        obtain ⟨outcome, evaluated, _⟩ := NativeExecution.evaluate_refines
          program tested (some frozen) old started valid stored maskCovered
        obtain ⟨limits, control, subsets, evaluationLower, evaluationUpper⟩ :=
          NativeEvaluationAccounting.returned_work_bound program tested (some frozen) old started
            valid stored maskCovered outcome.result outcome.output outcome.work evaluated
        cases resultCase : outcome.result with
        | Err reason =>
            have stopped : oracle.evaluate program tested (some frozen) old started =
                ok (.Err reason, outcome.output, outcome.work) := by
              simpa only [resultCase] using evaluated
            refine ⟨.Err reason, outcome.output, outcome.work, ?_, ?_⟩
            · dsimp only [started] at stopped
              simp [oracle.check_subset, NativeControl.poll_exact, observed,
                core.result.Result.Insts.CoreOpsTry.branch, UScalar.le_equiv,
                Nat.not_le.mpr quota, increment, stopped,
                core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
            · refine ⟨limits, control, ?_, ?_, ?_, ?_⟩
              · simpa only [wasAdmitted, ↓reduceIte] using
                  (congrArg UScalar.val subsets).trans countValue
              · exact evaluationLower
              · dsimp only [started] at evaluationUpper
                omega
              · simp [wasAdmitted]
        | Ok value =>
            cases value
            have completed : oracle.evaluate program tested (some frozen) old started =
                ok (.Ok (), outcome.output, outcome.work) := by
              simpa only [resultCase] using evaluated
            have rootsCovered : ∀ root ∈ program.value.roots.val,
                root.val < outcome.output.slice.val.length := by
              intro root member
              change root.val < outcome.output.val.length
              have outputValues : outcome.output.val = NativeSpecification.values tested (some frozen)
                  (NativeTable.rows (NativeExecution.view program)) := by
                exact NativeExecution.completed_values program tested (some frozen) old started
                  valid stored maskCovered outcome.output outcome.work completed
              have completeLength : outcome.output.val.length = (NativeTable.rows (NativeExecution.view program)).length := by
                rw [outputValues, NativeSpecification.values_length]
              rw [completeLength]
              exact rootsBounded root member
            obtain ⟨answer, after, scanned, _⟩ := NativeRootScan.failed_root_refines
              program outcome.output.slice outcome.work rootsCovered
            obtain ⟨frame, lower, upper⟩ := NativeRootScan.returned_work_bound
              program outcome.output.slice outcome.work after answer rootsCovered scanned
            let result : core.result.Result Bool zetesis_cpu.cancellation.Stop :=
              match answer with
              | .Ok failed => .Ok failed.isNone
              | .Err reason => .Err reason
            refine ⟨result, outcome.output, after, ?_, ?_⟩
            · have sliceView : alloc.vec.Vec.deref outcome.output = outcome.output.slice := by
                apply Slice.ext
                simp [alloc.vec.Vec.deref, alloc.vec.Vec.val]
              dsimp only [started] at completed
              cases answer <;> simp [oracle.check_subset, NativeControl.poll_exact, observed,
                core.result.Result.Insts.CoreOpsTry.branch, UScalar.le_equiv,
                Nat.not_le.mpr quota, increment, completed, sliceView, scanned, result,
                core.option.Option.is_none,
                core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
            · refine ⟨frame.1.trans limits, frame.2.1.trans control, ?_, ?_, ?_, ?_⟩
              · simpa only [wasAdmitted, ↓reduceIte] using
                  (congrArg UScalar.val (frame.2.2.trans subsets)).trans countValue
              · dsimp only [started] at evaluationLower
                omega
              · dsimp only [started] at evaluationUpper
                omega
              · simp [wasAdmitted]
      · refine ⟨.Err .CandidateLimit, old, before, ?_, ?_⟩
        · simp [oracle.check_subset, NativeControl.poll_exact, observed,
            core.result.Result.Insts.CoreOpsTry.branch, UScalar.le_equiv,
            Nat.le_of_not_gt quota]
        · refine ⟨rfl, rfl, ?_, Nat.le_refl _, ?_, ?_⟩
          · simp [admitted, observed, quota]
          · omega
          · intro _
            exact ⟨rfl, rfl⟩

/-- Every actual typed subset-query return has the constructed receipt. The
    premise names the returned values only; termination and the accounting
    contract come from `query_refines`, not from an assumed phase history. -/
theorem returned_receipt (program : theory.Theory) (tested : theory.Interpretation)
    (frozen : Slice Bool) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (stored : NativeMembership.Represented tested)
    (valid : NativeStructure.WellFormed program.value.atoms.val (NativeExecution.view program))
    (covered : (NativeTable.rows (NativeExecution.view program)).length ≤ frozen.val.length)
    (rootsBounded : ∀ root ∈ program.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view program)).length)
    (result : core.result.Result Bool zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (returned : oracle.check_subset program tested frozen old before =
      ok (result, output, after)) :
    Receipt program old before output after := by
  obtain ⟨actualResult, actualOutput, actualWork, executed, receipt⟩ :=
    query_refines program tested frozen old before stored valid covered rootsBounded
  have same : (actualResult, actualOutput, actualWork) = (result, output, after) := by
    exact Result.ok_injective (executed.symm.trans returned)
  have sameOutput : actualOutput = output := by
    exact congrArg (fun value => value.2.1) same
  have sameWork : actualWork = after := by
    exact congrArg (fun value => value.2.2) same
  exact sameOutput ▸ sameWork ▸ receipt

end NativeSubsetQueryTotal
