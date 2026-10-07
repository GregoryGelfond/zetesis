import NativeStep
import NativeTable

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisNativeExtract

/-!
# The actual native evaluator loop under fixed observations

The theorem unfolds the generated loop itself and derives a finite result from
its bounded range. Its complete output is the specified truth table. Its stopped
output is the exact completed prefix, with a separate count for work spent in
the next unfinished node. Atomic read tokens retain the fixed-observation scope
of the imported external model; no concurrent-runtime trace is claimed here.
-/
namespace NativeLoop

/-- The range names the whole paired table and its initialized output is the
truth prefix ending immediately before the next logical node. -/
structure Invariant (view : theory.FormulaView) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (cursor : NativeStep.Cursor)
    (output : alloc.vec.Vec Bool) : Prop where
  endPosition : cursor.end.val = (NativeTable.rows view).length
  bounded : cursor.start.val ≤ (NativeTable.rows view).length
  values : output.val = NativeSpecification.prefixValues candidate frozen
    (NativeTable.rows view) cursor.start.val

/-- A typed result with the position of its completed prefix and the work spent
inside its following unfinished node. Successful exhaustion has no partial work. -/
structure Outcome where
  result : core.result.Result Unit zetesis_cpu.cancellation.Stop
  output : alloc.vec.Vec Bool
  work : oracle.Work
  position : Nat
  pendingWork : Nat

/-- Complete prefix cost and partial-node cost together explain every charged
tick. The additive equation avoids truncated subtraction when comparing prefixes.
A typed stop remains a stop, strictly before the table ends. -/
structure Preserved (view : theory.FormulaView) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (cursor : NativeStep.Cursor) (work : oracle.Work)
    (outcome : Outcome) : Prop where
  values : outcome.output.val = NativeSpecification.prefixValues candidate frozen
    (NativeTable.rows view) outcome.position
  start : cursor.start.val ≤ outcome.position
  bounded : outcome.position ≤ (NativeTable.rows view).length
  limits : outcome.work.limits = work.limits
  control : outcome.work.cancellation = work.cancellation
  subsets : outcome.work.statistics.subsets = work.statistics.subsets
  accounting : outcome.work.statistics.work.val +
      NativeSpecification.cost ((NativeTable.rows view).take cursor.start.val) =
    work.statistics.work.val +
      NativeSpecification.cost ((NativeTable.rows view).take outcome.position) + outcome.pendingWork
  boundary : match outcome.result with
    | .Ok _ => outcome.position = (NativeTable.rows view).length ∧ outcome.pendingWork = 0
    | .Err _ => outcome.position < (NativeTable.rows view).length ∧
        outcome.pendingWork < 1 + NativeEvaluation.occurrences
          ((NativeTable.rows view)[outcome.position]?.getD .False)

/-- One unfolding retains the actual cursor, output and complete work record
returned by the generated body, including its observation tokens. -/
theorem loop_unfold (view : theory.FormulaView) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (cursor : NativeStep.Cursor)
    (output : alloc.vec.Vec Bool) (work : oracle.Work) :
    oracle.evaluate_loop cursor candidate frozen output work view.nodes view.operands = (do
      let transition ← oracle.evaluate_loop.body candidate frozen view.nodes view.operands
        cursor output work
      match transition with
      | .done result => ok result
      | .cont (next, extended, returned) =>
        oracle.evaluate_loop next candidate frozen extended returned view.nodes view.operands) := by
  rw [oracle.evaluate_loop, loop]
  dsimp only [uncurry]
  congr 1
  funext transition
  cases transition with
  | done result => rfl
  | cont result =>
    obtain ⟨next, extended, returned⟩ := result
    rfl

/-- Under actual paired-table validity, packed atom storage and mask coverage,
the generated loop terminates and preserves the exact truth prefix and all work.
No remaining-work or successful-completion premise is required.

Proof: an exhausted range returns directly. A present invocation either refuses
with its exact partial receipt or publishes one complete node. Publication
preserves the truth invariant, adds the node's complete N+E charge and strictly
decreases the number of unvisited logical nodes, permitting finite induction. -/
theorem loop_refines (atoms : Nat) (view : theory.FormulaView)
    (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (cursor : NativeStep.Cursor) (output : alloc.vec.Vec Bool) (work : oracle.Work)
    (valid : NativeStructure.WellFormed atoms view)
    (stored : NativeMembership.Represented candidate)
    (covered : ∀ mask ∈ frozen, (NativeTable.rows view).length ≤ mask.val.length)
    (invariant : Invariant view candidate frozen cursor output) :
    ∃ outcome : Outcome,
      oracle.evaluate_loop cursor candidate frozen output work view.nodes view.operands =
        ok (outcome.result, outcome.output, outcome.work) ∧
      Preserved view candidate frozen cursor work outcome := by
  have construct (remaining : Nat) :
      ∀ (current : NativeStep.Cursor) (prefixValues : alloc.vec.Vec Bool) (currentWork : oracle.Work),
      Invariant view candidate frozen current prefixValues →
      (NativeTable.rows view).length - current.start.val = remaining →
      ∃ outcome : Outcome,
        oracle.evaluate_loop current candidate frozen prefixValues currentWork view.nodes view.operands =
          ok (outcome.result, outcome.output, outcome.work) ∧
        Preserved view candidate frozen current currentWork outcome := by
    induction remaining using Nat.strong_induction_on with
    | h remaining ih =>
      intro current prefixValues currentWork currentInvariant remainingCount
      by_cases inside : current.start.val < (NativeTable.rows view).length
      · have rangeInside : current.start.val < current.end.val := by
          rw [currentInvariant.endPosition]
          exact inside
        have prefixLength : prefixValues.val.length = current.start.val := by
          rw [currentInvariant.values]
          exact NativeSpecification.prefix_length candidate frozen (NativeTable.rows view)
            current.start.val currentInvariant.bounded
        obtain ⟨lookup, backwards⟩ := NativeTable.lookup atoms view valid current.start inside
        have children : NativeStructure.ChildrenBefore prefixValues.val.length
            (NativeTable.rows view)[current.start.val] := by
          rw [prefixLength]
          exact backwards
        have mask : NativeStep.MaskPresent frozen current.start := by
          intro selected member
          have coverage : (NativeTable.rows view).length ≤ selected.val.length := by
            exact covered selected member
          omega
        have room : prefixValues.val.length < Usize.max := by
          have storage : view.nodes.val.length ≤ Usize.max := by
            exact Slice.length_ineq view.nodes
          have length : (NativeTable.rows view).length = view.nodes.val.length := by
            exact NativeTable.length view
          omega
        obtain ⟨transition, used, actual, effect⟩ := NativeStep.present_refines candidate frozen
          view current prefixValues currentWork (NativeTable.rows view)[current.start.val]
          rangeInside lookup stored children mask room
        cases transition with
        | done result =>
          obtain ⟨result, retained, returned⟩ := result
          obtain ⟨⟨reason, resultIsError⟩, sameOutput, receipt, partialBound⟩ := effect
          let outcome : Outcome := ⟨result, retained, returned, current.start.val, used⟩
          refine ⟨outcome, ?_, ?_⟩
          · rw [loop_unfold, actual]
            simp only [bind_tc_ok, outcome]
          · refine ⟨?_, Nat.le_refl _, currentInvariant.bounded, receipt.limits,
              receipt.control, receipt.subsets, ?_, ?_⟩
            · change retained.val = _
              rw [sameOutput]
              exact currentInvariant.values
            · dsimp only [outcome]
              have counted : returned.statistics.work.val = currentWork.statistics.work.val + used := by
                exact receipt.work
              omega
            · dsimp only [outcome]
              rw [resultIsError]
              exact ⟨inside, by simpa only [List.getElem?_eq_getElem inside,
                Option.getD_some] using partialBound⟩
        | cont result =>
          obtain ⟨next, extended, returned⟩ := result
          obtain ⟨progressed, sameEnd, extendedValues, receipt, charged⟩ := effect
          have nextInvariant : Invariant view candidate frozen next extended := by
            refine ⟨?_, ?_, ?_⟩
            · rw [sameEnd]
              exact currentInvariant.endPosition
            · omega
            · rw [progressed, NativeSpecification.prefix_succ candidate frozen
                (NativeTable.rows view) current.start.val inside]
              simpa only [NativeStep.masked, NativeSpecification.maskedAt,
                currentInvariant.values] using extendedValues
          have decreased : (NativeTable.rows view).length - next.start.val < remaining := by
            omega
          obtain ⟨outcome, executed, preserved⟩ := ih
            ((NativeTable.rows view).length - next.start.val) decreased
            next extended returned nextInvariant rfl
          refine ⟨outcome, ?_, preserved.values, ?_, preserved.bounded,
            preserved.limits.trans receipt.limits,
            preserved.control.trans receipt.control,
            preserved.subsets.trans receipt.subsets, ?_, preserved.boundary⟩
          · rw [loop_unfold, actual]
            simpa only [bind_tc_ok] using executed
          · have lower : next.start.val ≤ outcome.position := by
              exact preserved.start
            omega
          · have future : outcome.work.statistics.work.val +
                NativeSpecification.cost ((NativeTable.rows view).take next.start.val) =
              returned.statistics.work.val +
                NativeSpecification.cost ((NativeTable.rows view).take outcome.position) +
                outcome.pendingWork := by
              exact preserved.accounting
            have counted : returned.statistics.work.val = currentWork.statistics.work.val + used := by
              exact receipt.work
            have nextCost : NativeSpecification.cost ((NativeTable.rows view).take next.start.val) =
                NativeSpecification.cost ((NativeTable.rows view).take current.start.val) +
                  (1 + NativeEvaluation.occurrences (NativeTable.rows view)[current.start.val]) := by
              rw [progressed]
              exact NativeSpecification.cost_prefix_succ _ _ inside
            rw [nextCost] at future
            omega
      · have ended : current.end.val ≤ current.start.val := by
          rw [currentInvariant.endPosition]
          exact Nat.le_of_not_gt inside
        have position : current.start.val = (NativeTable.rows view).length := by
          have bounded : current.start.val ≤ (NativeTable.rows view).length := by
            exact currentInvariant.bounded
          omega
        let outcome : Outcome := ⟨.Ok (), prefixValues, currentWork, current.start.val, 0⟩
        refine ⟨outcome, ?_, currentInvariant.values, Nat.le_refl _, currentInvariant.bounded,
          rfl, rfl, rfl, by simp [outcome], position, rfl⟩
        rw [loop_unfold, NativeStep.exhausted candidate frozen view current prefixValues currentWork ended]
        simp only [bind_tc_ok, outcome]
  exact construct ((NativeTable.rows view).length - cursor.start.val)
    cursor output work invariant rfl

end NativeLoop
