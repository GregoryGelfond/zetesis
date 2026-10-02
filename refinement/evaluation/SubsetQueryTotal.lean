import SubsetQuery

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisExtract

/-!
# Total typed outcomes of an actual subset query

The generated subset query either refuses admission or evaluates and scans roots
using one cumulative work record. These statements construct its actual typed
return under represented storage and covered reads. They do not assume successful
evaluation, successful root scanning, or sufficient work and subset allowances.

Control preservation refers to the existing fixed observation-token model. No
allocator, owner-identity or changing concurrent-atomic correspondence is added.
The queried interpretation and frozen mask remain immutable supplied inputs;
their reduct meaning is established separately by `SubsetQuery.completed_reduct`.
-/
namespace SubsetQueryTotal

/-- An actual evaluator return retains its supplied control record. The proof
    follows generated calls, whose present, stopped and exhausted transitions
    all return the same token. This is fixed-token preservation, not a claim
    that concurrent Rust flags cannot change. -/
theorem evaluation_control (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (covered : ∀ mask ∈ frozen, program.value.nodes.val.length ≤ mask.val.length)
    (result : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (returned : oracle.evaluate program candidate frozen old before =
      ok (result, output, after)) :
    after.cancellation = before.cancellation := by
  let table := program.value.nodes.val
  let transitionControl : Evaluation.Transition → zetesis_cpu.cancellation.Cancellation
    | .cont (_, _, _, control, _) => control
    | .done (_, _, work) => work.cancellation
  have invocationControl (state : EvaluationTrace.State)
      (control : zetesis_cpu.cancellation.Cancellation)
      (invariant : EvaluationTrace.Invariant candidate frozen table state)
      (transition : Evaluation.Transition)
      (executed : oracle.evaluate_loop.body candidate frozen state.cursor state.output
        state.limits control state.statistics = ok transition) :
      transitionControl transition = control := by
    by_cases inside : state.cursor.iter.i < table.length
    · have sliceInside : state.cursor.iter.i < state.cursor.iter.slice.val.length := by
        rw [invariant.source]
        exact inside
      cases observed : EvaluatorControl.observation control with
      | some reason =>
          have stopped := EvaluationProgress.control_stops candidate frozen state.cursor
            state.output state.limits control state.statistics reason
            invariant.counter sliceInside observed
          have same := Result.ok_injective (executed.symm.trans stopped)
          exact congrArg transitionControl same
      | none =>
          by_cases remaining : state.statistics.work.val < state.limits.max_work.val
          · have prefixLength : state.output.val.length = state.cursor.iter.i := by
              rw [invariant.values]
              exact EvaluationSpecification.prefix_length candidate frozen table
                state.cursor.iter.i invariant.bounded
            have children : Evaluation.ChildrenPresent state.output.val
                state.cursor.iter.slice.val[state.cursor.iter.i] := by
              simpa only [invariant.source] using
                (EvaluationSpecification.children_present table[state.cursor.iter.i]
                  state.output.val state.cursor.iter.i prefixLength
                  (ordered state.cursor.iter.i inside))
            have maskCovered : ∀ mask ∈ frozen,
                state.cursor.iter.slice.val.length ≤ mask.val.length := by
              simpa only [invariant.source] using covered
            obtain ⟨_, _, _, _, _, advanced, _, _, _⟩ :=
              EvaluationProgress.present_step candidate frozen state.cursor state.output
                state.limits control state.statistics invariant.counter prefixLength
                sliceInside stored children maskCovered observed remaining
            have same := Result.ok_injective (executed.symm.trans advanced)
            exact congrArg transitionControl same
          · have stopped := EvaluationProgress.work_limit_stops candidate frozen state.cursor
              state.output state.limits control state.statistics invariant.counter
              sliceInside observed (Nat.le_of_not_gt remaining)
            have same := Result.ok_injective (executed.symm.trans stopped)
            exact congrArg transitionControl same
    · have exhausted : state.cursor.iter.slice.val.length ≤ state.cursor.iter.i := by
        rw [invariant.source]
        exact Nat.le_of_not_gt inside
      have finished := Evaluation.exhausted_step candidate frozen state.cursor
        state.output state.limits control state.statistics exhausted
      have same := Result.ok_injective (executed.symm.trans finished)
      exact congrArg transitionControl same
  have callsControl {state : EvaluationTrace.State}
      {control : zetesis_cpu.cancellation.Cancellation} {count : Nat}
      {outcome : EvaluationTrace.Outcome}
      (calls : FixedEvaluationLoop.Calls candidate frozen state control count outcome)
      (invariant : EvaluationTrace.Invariant candidate frozen table state) :
      outcome.work.cancellation = control := by
    induction calls with
    | finish step =>
        exact invocationControl _ _ invariant _ step
    | @next state nextState control returnedControl count outcome step rest ih =>
        have sameControl : returnedControl = control :=
          invocationControl state control invariant _ step
        obtain ⟨transition, actual, effect⟩ := EvaluationTrace.invocation_refines
          candidate frozen table state control stored ordered covered invariant
        have same : transition = .cont (nextState.cursor, nextState.output,
            nextState.limits, returnedControl, nextState.statistics) :=
          Result.ok_injective (actual.symm.trans step)
        rw [same] at effect
        exact (ih effect.1).trans sameControl
  let initial := ReductTrace.initialState program before.limits before.statistics
  have invariant : EvaluationTrace.Invariant candidate frozen table initial :=
    ReductTrace.initial_invariant program candidate frozen before.limits before.statistics
  obtain ⟨_, outcome, calls, _⟩ := FixedEvaluationLoop.calls_exist candidate frozen
    table initial before.cancellation stored ordered covered invariant
  have executed : oracle.evaluate program candidate frozen old before =
      ok (outcome.result, outcome.output, outcome.work) := by
    rw [EvaluatorSetup.evaluate_from_empty]
    exact FixedEvaluationLoop.calls_execute candidate frozen calls
  have same : (outcome.result, outcome.output, outcome.work) = (result, output, after) :=
    Result.ok_injective (executed.symm.trans returned)
  have sameWork : outcome.work = after := congrArg (fun value => value.2.2) same
  exact sameWork ▸ callsControl calls invariant

/-- The source admits a subset exactly when its initial poll is clear and its
    subset count is strictly below the stored ceiling. Work admission occurs
    later, once the query has already charged this subset. -/
def admitted (before : oracle.Work) : Bool :=
  (EvaluatorControl.observation before.cancellation).isNone &&
    decide (before.statistics.subsets.val < before.limits.max_subsets.val)

/-- Every typed query return preserves limits and controls. Admission charges
    exactly one subset even if evaluation or roots subsequently stop. Before
    admission, refusal preserves the old output and the entire work record.
    Logical work grows by at most all nodes plus all root occurrences. -/
structure Receipt (program : theory.Theory) (old : alloc.vec.Vec Bool)
    (before : oracle.Work) (output : alloc.vec.Vec Bool) (after : oracle.Work) : Prop where
  limits : after.limits = before.limits
  control : after.cancellation = before.cancellation
  subsets : after.statistics.subsets.val = before.statistics.subsets.val +
    if admitted before then 1 else 0
  workLower : before.statistics.work.val ≤ after.statistics.work.val
  workUpper : after.statistics.work.val ≤ before.statistics.work.val +
    program.value.nodes.val.length + program.value.roots.val.length
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
    (stored : Membership.Represented tested)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (covered : program.value.nodes.val.length ≤ frozen.val.length)
    (rootsBounded : ∀ root ∈ program.value.roots.val,
      root.val < program.value.nodes.val.length) :
    ∃ result : core.result.Result Bool zetesis_cpu.cancellation.Stop,
      ∃ output : alloc.vec.Vec Bool, ∃ after : oracle.Work,
        oracle.check_subset program tested frozen old before = ok (result, output, after) ∧
        Receipt program old before output after := by
  cases observed : EvaluatorControl.observation before.cancellation with
  | some reason =>
      refine ⟨.Err reason, old, before, ?_, ?_⟩
      · simp [oracle.check_subset, EvaluatorControl.poll_exact, observed,
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
            program.value.nodes.val.length ≤ mask.val.length := by
          intro mask member
          cases member
          exact covered
        obtain ⟨_, outcome, evaluated, _, _⟩ := FixedEvaluationLoop.evaluate_refines
          program tested (some frozen) old started stored ordered maskCovered
        have receipt : EvaluationAccounting.Receipt program started outcome.result
            outcome.output outcome.work := EvaluationAccounting.returned_receipt
          program tested (some frozen) old started stored ordered maskCovered
          outcome.result outcome.output outcome.work evaluated
        have control : outcome.work.cancellation = before.cancellation :=
          evaluation_control program tested (some frozen) old started stored ordered
            maskCovered outcome.result outcome.output outcome.work evaluated
        cases resultCase : outcome.result with
        | Err reason =>
            have stopped : oracle.evaluate program tested (some frozen) old started =
                ok (.Err reason, outcome.output, outcome.work) := by
              simpa only [resultCase] using evaluated
            refine ⟨.Err reason, outcome.output, outcome.work, ?_, ?_⟩
            · dsimp only [started] at stopped
              simp [oracle.check_subset, EvaluatorControl.poll_exact, observed,
                core.result.Result.Insts.CoreOpsTry.branch, UScalar.le_equiv,
                Nat.not_le.mpr quota, increment, stopped,
                core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
            · refine ⟨receipt.limits, control, ?_, ?_, ?_, ?_⟩
              · simpa only [wasAdmitted, ↓reduceIte] using
                  (congrArg UScalar.val receipt.subsets).trans countValue
              · have counted := receipt.work
                dsimp only [started] at counted
                omega
              · have counted := receipt.work
                have bounded := receipt.bounded
                dsimp only [started] at counted
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
              have completeLength : outcome.output.val.length = program.value.nodes.val.length := by
                simpa only [resultCase] using receipt.boundary
              rw [completeLength]
              exact rootsBounded root member
            obtain ⟨answer, after, scanned, _⟩ := FixedRootScan.failed_root_refines
              program outcome.output.slice outcome.work rootsCovered
            obtain ⟨frame, lower, upper⟩ := FixedRootScan.returned_work_bound
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
              cases answer <;> simp [oracle.check_subset, EvaluatorControl.poll_exact, observed,
                core.result.Result.Insts.CoreOpsTry.branch, UScalar.le_equiv,
                Nat.not_le.mpr quota, increment, completed, sliceView, scanned, result,
                core.option.Option.is_none,
                core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
            · refine ⟨frame.1.trans receipt.limits, frame.2.1.trans control, ?_, ?_, ?_, ?_⟩
              · simpa only [wasAdmitted, ↓reduceIte] using
                  (congrArg UScalar.val (frame.2.2.trans receipt.subsets)).trans countValue
              · have counted := receipt.work
                dsimp only [started] at counted
                omega
              · have counted := receipt.work
                have bounded := receipt.bounded
                dsimp only [started] at counted
                omega
              · simp [wasAdmitted]
      · refine ⟨.Err .CandidateLimit, old, before, ?_, ?_⟩
        · simp [oracle.check_subset, EvaluatorControl.poll_exact, observed,
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
    (stored : Membership.Represented tested)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (covered : program.value.nodes.val.length ≤ frozen.val.length)
    (rootsBounded : ∀ root ∈ program.value.roots.val,
      root.val < program.value.nodes.val.length)
    (result : core.result.Result Bool zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (returned : oracle.check_subset program tested frozen old before =
      ok (result, output, after)) :
    Receipt program old before output after := by
  obtain ⟨actualResult, actualOutput, actualWork, executed, receipt⟩ :=
    query_refines program tested frozen old before stored ordered covered rootsBounded
  have same : (actualResult, actualOutput, actualWork) = (result, output, after) :=
    Result.ok_injective (executed.symm.trans returned)
  have sameOutput : actualOutput = output := congrArg (fun value => value.2.1) same
  have sameWork : actualWork = after := congrArg (fun value => value.2.2) same
  exact sameOutput ▸ sameWork ▸ receipt

end SubsetQueryTotal
