import FrozenQuery

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisExtract

/-!
# Boundaries of the actual stored-reduct query

These examples execute the generated private query under fixed clear observation
tokens. Work already charged before the call remains charged at the root scan.
The query neither enforces a subset relation nor requires its stored candidate
to model the original theory. None of these examples establishes construction,
allocation, owner checks or concurrent-runtime correspondence.
-/
namespace FrozenQueryExample

/-- A singleton Boolean vector spells the concrete masks and outputs. -/
def truth (value : Bool) : alloc.vec.Vec Bool :=
  { slice := Slice.from [value] (by scalar_tac) }

/-- Only the charged work varies within an example. Clear fixed observations,
no deadline and a nonzero subset count make the retained fields explicit. -/
def work (limit charged : U64) : oracle.Work := {
  limits := { max_work := limit, max_subsets := 7#u64 }
  cancellation := { cancelled := { owner := 1, value := { nextRead := false } }, deadline := none, slot := none }
  statistics := { work := charged, subsets := 5#u64 } }

/-- On a single admitted leaf, the actual evaluator performs one successful
body transition and then exhausts its source slice. Its computed output is the
leaf value masked by the supplied bit; the old output is cleared first.

This helper derives the actual loop call from its body and setup laws. It does
not assume an evaluator result or a mask/value correspondence. -/
theorem singleton_evaluation (program : theory.Theory)
    (tested : theory.Interpretation) (node : theory.Node) (mask : Bool)
    (limit before after : U64)
    (nodes : program.value.nodes.val = [node])
    (stored : Membership.Represented tested)
    (leaf : Evaluation.ChildrenPresent [] node)
    (remaining : before.val < limit.val) (increment : after.val = before.val + 1) :
    oracle.evaluate program tested (some (truth mask).slice) (truth true)
      (work limit before) =
      ok (core.result.Result.Ok (), truth (Evaluation.value tested [] node && mask),
        work limit after) := by
  let initial : EvaluationTrace.State := {
    cursor := EvaluatorSetup.initialCursor program
    output := alloc.vec.Vec.new Bool
    limits := (work limit before).limits
    statistics := (work limit before).statistics }
  obtain ⟨nextCount, nextWork, output, countNext, workNext, stepped,
      outputValues, _, _⟩ := EvaluationProgress.present_step tested
    (some (truth mask).slice) initial.cursor initial.output initial.limits
    (work limit before).cancellation initial.statistics
    rfl rfl (by simp [initial, EvaluatorSetup.initialCursor, alloc.vec.Vec.deref, nodes])
    stored (by simpa [initial, EvaluatorSetup.initialCursor, alloc.vec.Vec.deref, nodes] using leaf)
    (by intro values member; cases member; simp [initial, EvaluatorSetup.initialCursor,
        alloc.vec.Vec.deref, nodes, truth]) rfl remaining
  have countOne : nextCount = 1#usize := by
    apply UScalar.eq_of_val_eq
    simpa [initial, EvaluatorSetup.initialCursor] using countNext
  have workAfter : nextWork = after :=
    UScalar.eq_of_val_eq (workNext.trans increment.symm)
  have outputExact : output = truth (Evaluation.value tested [] node && mask) := by
    apply alloc.vec.Vec.ext
    change output.val = [Evaluation.value tested [] node && mask]
    simpa [initial, EvaluatorSetup.initialCursor, alloc.vec.Vec.deref, nodes,
      Evaluation.masked, truth] using outputValues
  let finalState : EvaluationTrace.State := {
    cursor := { iter := { initial.cursor.iter with i := 1 }, count := 1#usize }
    output := truth (Evaluation.value tested [] node && mask)
    limits := initial.limits
    statistics := (work limit after).statistics }
  let completed : EvaluationTrace.Outcome := {
    result := .Ok ()
    output := finalState.output
    work := work limit after }
  have first : oracle.evaluate_loop.body tested (some (truth mask).slice)
      initial.cursor initial.output initial.limits (work limit before).cancellation
      initial.statistics = ok (.cont (finalState.cursor, finalState.output,
        finalState.limits, (work limit before).cancellation, finalState.statistics)) := by
    simpa only [countOne, workAfter, outputExact, finalState, initial,
      EvaluatorSetup.initialCursor, work] using stepped
  have finish : oracle.evaluate_loop.body tested (some (truth mask).slice)
      finalState.cursor finalState.output finalState.limits (work limit before).cancellation
      finalState.statistics = ok (.done (completed.result, completed.output, completed.work)) := by
    apply Evaluation.exhausted_step
    simp [finalState, initial, EvaluatorSetup.initialCursor, alloc.vec.Vec.deref, nodes]
  have calls : FixedEvaluationLoop.Calls tested (some (truth mask).slice) initial
      (work limit before).cancellation 1 completed :=
    FixedEvaluationLoop.Calls.next first (FixedEvaluationLoop.Calls.finish finish)
  rw [EvaluatorSetup.evaluate_from_empty]
  exact FixedEvaluationLoop.calls_execute tested (some (truth mask).slice) calls

/-- A singleton asserted root consumes one tick and either returns that false
root or exhausts after its true value. The helper uses the actual scan body. -/
theorem singleton_scan (program : theory.Theory) (value : Bool)
    (limit before after : U64)
    (roots : program.value.roots.val = [0#usize])
    (remaining : before.val < limit.val) (increment : after.val = before.val + 1) :
    oracle.failed_root program (truth value).slice (work limit before) =
      ok (core.result.Result.Ok (if value then none else some 0#usize), work limit after) := by
  obtain ⟨next, advanced, ticked⟩ := EvaluatorControl.tick_advances
    (work limit before) rfl remaining
  have same : next = after := UScalar.eq_of_val_eq (advanced.trans increment.symm)
  have tick : oracle.Work.tick (work limit before) = ok (.Ok (), work limit after) := by
    simpa only [same, work] using ticked
  let cursor := FixedRootScan.initialCursor program
  have body := FixedRootScan.body_tested (truth value).slice cursor
    (work limit before) (work limit after)
    (by simp [cursor, FixedRootScan.initialCursor, alloc.vec.Vec.deref, roots])
    (by simp [cursor, FixedRootScan.initialCursor, alloc.vec.Vec.deref, roots, truth]) tick
  have first : oracle.failed_root_loop.body (truth value).slice cursor (work limit before) =
      ok (if value then .cont ({ cursor with i := 1 }, work limit after)
        else .done (.Ok (some 0#usize), work limit after)) := by
    cases value <;>
      simpa [cursor, FixedRootScan.initialCursor, alloc.vec.Vec.deref, roots,
        FixedRootScan.truth, truth] using body
  rw [FixedRootScan.failed_root_from_start, FixedRootScan.loop_unfold, first]
  cases value with
  | false => simp only [Bool.false_eq_true, ↓reduceIte, bind_tc_ok]
  | true =>
    simp only [↓reduceIte, bind_tc_ok]
    rw [FixedRootScan.loop_unfold, FixedRootScan.body_exhausted _ _ _
      (by simp [cursor, FixedRootScan.initialCursor, alloc.vec.Vec.deref, roots])]
    simp only [bind_tc_ok]

/-- Falsum is the one asserted formula; there are no atoms to store. -/
def falseCandidate : theory.Interpretation := {
  theory := { owner := 0, value := {
    atoms := 0#usize
    nodes := alloc.vec.Vec.from [.False] (by scalar_tac)
    roots := alloc.vec.Vec.from [0#usize] (by scalar_tac) } }
  words := alloc.vec.Vec.new U64 }

/-- Original evaluation of the falsum candidate has one false mask bit. -/
def falseFrozen : reduct.FrozenReduct := ⟨falseCandidate, truth false⟩

/-- With one unit already charged and a ceiling of two, evaluation consumes the
remaining unit. The root scan then returns the typed work limit at exactly two,
retaining the computed output and the other work fields. It does not restart the
allowance or turn the untested false root into a satisfaction verdict. -/
theorem root_scan_retains_prior_work :
    reduct.FrozenReduct.satisfied_by falseFrozen falseCandidate (truth true)
      (work 2#u64 1#u64) =
      ok (core.result.Result.Err .WorkLimit, truth false, work 2#u64 2#u64) := by
  have evaluated : oracle.evaluate falseCandidate.theory falseCandidate
      (some (truth false).slice) (truth true) (work 2#u64 1#u64) =
      ok (.Ok (), truth false, work 2#u64 2#u64) := by
    simpa [Evaluation.value] using singleton_evaluation falseCandidate.theory
      falseCandidate .False false 2#u64 1#u64 2#u64 rfl
      (by intro atom inside; simp [falseCandidate] at inside) trivial (by decide) (by decide)
  have scanned : oracle.failed_root falseCandidate.theory (truth false).slice
      (work 2#u64 2#u64) = ok (.Err .WorkLimit, work 2#u64 2#u64) := by
    rw [FixedRootScan.failed_root_from_start, FixedRootScan.loop_unfold,
      FixedRootScan.body_stopped _ _ _ _
        (by simp [FixedRootScan.initialCursor, falseCandidate, alloc.vec.Vec.deref]) .WorkLimit
        (EvaluatorControl.tick_at_limit _ rfl (by decide))]
    simp only [bind_tc_ok]
  exact FrozenQuery.execution_returns falseFrozen falseCandidate (truth true)
    (work 2#u64 1#u64) (.Err .WorkLimit) (truth false) (work 2#u64 2#u64)
    (FrozenQuery.Execution.scanned (truth false) (work 2#u64 2#u64)
      (work 2#u64 2#u64) (.Err .WorkLimit) evaluated scanned)

/-- A correct stored mask need not come from an original model: falsum evaluates
false under this candidate, and the actual query completes with false. -/
theorem represented_candidate_need_not_be_a_model :
    FrozenQuery.Represents falseFrozen ∧
      ¬ Zetesis.Ferraris.Models
        (Zetesis.TightEvaluation.interpretation (Membership.denotes falseCandidate))
        (RootSemantics.assertions falseCandidate.theory) ∧
      reduct.FrozenReduct.satisfied_by falseFrozen falseCandidate (truth true)
        (work 2#u64 0#u64) = ok (.Ok false, truth false, work 2#u64 2#u64) := by
  have represented : FrozenQuery.Represents falseFrozen := by
    rfl
  have notModel : ¬ Zetesis.Ferraris.Models
      (Zetesis.TightEvaluation.interpretation (Membership.denotes falseCandidate))
      (RootSemantics.assertions falseCandidate.theory) := by
    simp [RootSemantics.assertions, falseCandidate, Zetesis.DagSharing.assertions,
      Zetesis.DagSharing.meanings, Zetesis.DagSharing.decode, EvaluationSemantics.node,
      Zetesis.Ferraris.Models, Zetesis.Ferraris.Satisfies]
  have evaluated : oracle.evaluate falseCandidate.theory falseCandidate
      (some (truth false).slice) (truth true) (work 2#u64 0#u64) =
      ok (.Ok (), truth false, work 2#u64 1#u64) := by
    simpa [Evaluation.value] using singleton_evaluation falseCandidate.theory
      falseCandidate .False false 2#u64 0#u64 1#u64 rfl
      (by intro atom inside; simp [falseCandidate] at inside) trivial (by decide) (by decide)
  have scanned := singleton_scan falseCandidate.theory false 2#u64 1#u64 2#u64
    rfl (by decide) (by decide)
  refine ⟨represented, notModel, ?_⟩
  exact FrozenQuery.execution_returns falseFrozen falseCandidate (truth true)
    (work 2#u64 0#u64) (.Ok false) (truth false) (work 2#u64 2#u64)
    (FrozenQuery.Execution.scanned (truth false) (work 2#u64 1#u64)
      (work 2#u64 2#u64) (.Ok (some 0#usize)) evaluated scanned)

/-- The asserted atom is zero in a two-atom universe. Atom one is unasserted;
the packed word selects whether it additionally belongs to the interpretation. -/
def atomCandidate (bits : U64) : theory.Interpretation := {
  theory := { owner := 0, value := {
    atoms := 2#usize
    nodes := alloc.vec.Vec.from [.Atom 0#usize] (by scalar_tac)
    roots := alloc.vec.Vec.from [0#usize] (by scalar_tac) } }
  words := alloc.vec.Vec.from [bits] (by scalar_tac) }

/-- The actual private query accepts `{0,1}` against the reduct fixed at `{0}`.
Atom one witnesses failure of the subset relation; the single asserted atom zero
is true in both interpretations. The stored mask is original truth, not an
arbitrary mask used to manufacture acceptance. -/
theorem successful_query_need_not_test_a_subset :
    FrozenQuery.Represents ⟨atomCandidate 1#u64, truth true⟩ ∧
      Membership.denotes (atomCandidate 3#u64) 1 = true ∧
      Membership.denotes (atomCandidate 1#u64) 1 = false ∧
      reduct.FrozenReduct.satisfied_by ⟨atomCandidate 1#u64, truth true⟩
        (atomCandidate 3#u64) (truth true) (work 2#u64 0#u64) =
        ok (.Ok true, truth true, work 2#u64 2#u64) := by
  have represented : FrozenQuery.Represents ⟨atomCandidate 1#u64, truth true⟩ := by
    rfl
  have evaluated : oracle.evaluate (atomCandidate 1#u64).theory (atomCandidate 3#u64)
      (some (truth true).slice) (truth true) (work 2#u64 0#u64) =
      ok (.Ok (), truth true, work 2#u64 1#u64) := by
    have stored : Membership.Represented (atomCandidate 3#u64) := by
      intro atom inside
      simp only [atomCandidate] at inside ⊢
      have small : atom < 2 := inside
      change atom / 64 < 1
      omega
    have atomTrue : Evaluation.value (atomCandidate 3#u64) [] (.Atom 0#usize) = true := by decide
    simpa only [atomTrue, Bool.and_self] using
      singleton_evaluation (atomCandidate 1#u64).theory (atomCandidate 3#u64)
        (.Atom 0#usize) true 2#u64 0#u64 1#u64 rfl stored trivial (by decide) (by decide)
  have scanned := singleton_scan (atomCandidate 1#u64).theory true
    2#u64 1#u64 2#u64 rfl (by decide) (by decide)
  refine ⟨represented, ?_, ?_, ?_⟩
  · decide
  · decide
  · exact FrozenQuery.execution_returns ⟨atomCandidate 1#u64, truth true⟩
      (atomCandidate 3#u64) (truth true) (work 2#u64 0#u64)
      (.Ok true) (truth true) (work 2#u64 2#u64)
      (FrozenQuery.Execution.scanned (truth true) (work 2#u64 1#u64)
        (work 2#u64 2#u64) (.Ok none) evaluated scanned)

end FrozenQueryExample
