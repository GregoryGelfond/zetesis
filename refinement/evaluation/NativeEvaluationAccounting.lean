import NativeExecution

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract

/-!
# Complete and partial native evaluation receipts

A refused native node can have consumed operand ticks without publishing its
truth value. The receipt therefore retains the completed-prefix cost and the
pending node's partial cost separately. Both are obtained from the actual
NativeExecution theorem, including on source-level refusal. These counts concern
logical work under fixed observation tokens, not capacity or elapsed time.
-/
namespace NativeEvaluationAccounting

/-- Every actual typed evaluator return has its proved native prefix receipt.
The caller supplies only the returned value, not a successful inner execution. -/
theorem returned_receipt (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (valid : NativeStructure.WellFormed program.value.atoms.val (NativeExecution.view program))
    (stored : NativeMembership.Represented candidate)
    (covered : ∀ mask ∈ frozen, (NativeTable.rows (NativeExecution.view program)).length ≤ mask.val.length)
    (result : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (returned : oracle.evaluate program candidate frozen old before = ok (result, output, after)) :
    ∃ position pending,
      NativeLoop.Preserved (NativeExecution.view program) candidate frozen
        (NativeExecution.initialCursor program) before ⟨result, output, after, position, pending⟩ := by
  obtain ⟨outcome, actual, preserved⟩ :=
    NativeExecution.evaluate_refines program candidate frozen old before valid stored covered
  have same : (outcome.result, outcome.output, outcome.work) = (result, output, after) := by
    exact Result.ok_injective (actual.symm.trans returned)
  have resultSame : outcome.result = result := by exact congrArg Prod.fst same
  have outputSame : outcome.output = output := by exact congrArg (fun value => value.2.1) same
  have workSame : outcome.work = after := by exact congrArg (fun value => value.2.2) same
  refine ⟨outcome.position, outcome.pendingWork, ?_⟩
  simpa only [← resultSame, ← outputSame, ← workSame] using preserved

/-- A native prefix can consume no more work than its complete table. -/
theorem prefix_cost_le (table : List theory.NodeView) (count : Nat) :
    NativeSpecification.cost (table.take count) ≤ NativeSpecification.cost table := by
  have splitCost : NativeSpecification.cost table =
      NativeSpecification.cost (table.take count) + NativeSpecification.cost (table.drop count) := by
    conv_lhs => rw [← List.take_append_drop count table]
    simp only [NativeSpecification.cost, List.map_append, List.sum_append]
  omega

/-- Both completion and refusal preserve the work frame and charge at most N+E.
A partial node's already-admitted ticks are included in the upper bound. -/
theorem returned_work_bound (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (valid : NativeStructure.WellFormed program.value.atoms.val (NativeExecution.view program))
    (stored : NativeMembership.Represented candidate)
    (covered : ∀ mask ∈ frozen, (NativeTable.rows (NativeExecution.view program)).length ≤ mask.val.length)
    (result : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (returned : oracle.evaluate program candidate frozen old before = ok (result, output, after)) :
    after.limits = before.limits ∧ after.cancellation = before.cancellation ∧
      after.statistics.subsets = before.statistics.subsets ∧
      before.statistics.work.val ≤ after.statistics.work.val ∧
      after.statistics.work.val ≤ before.statistics.work.val +
        NativeSpecification.cost (NativeTable.rows (NativeExecution.view program)) := by
  obtain ⟨position, pending, receipt⟩ := returned_receipt program candidate frozen old before
    valid stored covered result output after returned
  have charged : after.statistics.work.val = before.statistics.work.val +
      NativeSpecification.cost ((NativeTable.rows (NativeExecution.view program)).take position) + pending := by
    simpa [NativeExecution.initialCursor, NativeSpecification.cost] using receipt.accounting
  refine ⟨receipt.limits, receipt.control, receipt.subsets, by omega, ?_⟩
  cases result with
  | Ok value =>
    obtain ⟨atEnd, finished⟩ := receipt.boundary
    change position = _ at atEnd
    change pending = 0 at finished
    rw [atEnd, List.take_length, finished] at charged
    omega
  | Err reason =>
    obtain ⟨inside, partialReceipt⟩ := receipt.boundary
    change position < _ at inside
    change pending < _ at partialReceipt
    have partialBound : pending < 1 + NativeEvaluation.occurrences
        ((NativeTable.rows (NativeExecution.view program))[position]'inside) := by
      simpa only [List.getElem?_eq_getElem inside, Option.getD_some] using partialReceipt
    have nextCost : NativeSpecification.cost ((NativeTable.rows (NativeExecution.view program)).take
        (position + 1)) =
      NativeSpecification.cost ((NativeTable.rows (NativeExecution.view program)).take position) +
        (1 + NativeEvaluation.occurrences ((NativeTable.rows (NativeExecution.view program))[position]'inside)) := by
      exact NativeSpecification.cost_prefix_succ _ position inside
    have bound : NativeSpecification.cost ((NativeTable.rows (NativeExecution.view program)).take
        (position + 1)) ≤ NativeSpecification.cost (NativeTable.rows (NativeExecution.view program)) := by
      exact prefix_cost_le _ _
    omega

end NativeEvaluationAccounting
