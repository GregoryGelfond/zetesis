import TightClassificationStep

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisExtract

/-!
# The actual node-classification scan

The generated classifier visits an ordered node table once. Its initialized
class vector is the classification of exactly the visited prefix, while every
implication reads its consequent from the same immutable full table. A completed
scan therefore returns the full classification fold, with one charged tick per
visited node. Typed refusals remain distinct from a completed class table.

The proof follows the actual generated body and loop under the fixed-observation
external definitions. It does not assume classifier correctness or claim a
runtime observation history, allocator implementation, complete tight-plan
certificate or answer-set decision.
-/
namespace TightClassificationLoop

/-- The actual cursor and class vector describe the same visited prefix. -/
structure Invariant (program : theory.Theory) (iter : core.slice.iter.Iter theory.Node)
    (classes : alloc.vec.Vec tight.compile.Body) : Prop where
  source : iter.slice.val = program.value.nodes.val
  bounded : iter.i ≤ program.value.nodes.val.length
  classified : classes.val = TightClassificationSemantics.scan program.value.nodes.val
    (program.value.nodes.val.take iter.i)

/-- The visited cursor position is exactly the initialized class length. -/
theorem prefix_length (program : theory.Theory) (iter : core.slice.iter.Iter theory.Node)
    (classes : alloc.vec.Vec tight.compile.Body) (invariant : Invariant program iter classes) :
    classes.val.length = iter.i := by
  rw [invariant.classified, TightClassificationSemantics.scan_length, List.length_take]
  exact Nat.min_eq_left invariant.bounded

/-- Appending the next class preserves the cursor-prefix invariant. -/
theorem advance_invariant (program : theory.Theory) (iter : core.slice.iter.Iter theory.Node)
    (classes extended : alloc.vec.Vec tight.compile.Body)
    (invariant : Invariant program iter classes)
    (inside : iter.i < program.value.nodes.val.length)
    (appended : extended.val = classes.val ++
      [TightClassificationSemantics.classifyNode program.value.nodes.val classes.val
        program.value.nodes.val[iter.i]]) :
    Invariant program { iter with i := iter.i + 1 } extended := by
  refine ⟨invariant.source, by dsimp; omega, ?_⟩
  simp only [List.take_add_one, List.getElem?_eq_getElem inside, Option.toList_some,
    TightClassificationSemantics.scan_snoc]
  rw [← invariant.classified]
  exact appended

/-- The returned work counts the nodes actually classified. A successful
answer additionally accounts for every remaining node and contains the full
class table. A typed refusal carries no completed-table assertion. -/
structure Report (program : theory.Theory) (position : Nat) (before after : tight.Work)
    (processed : Nat) (answer : core.result.Result (alloc.vec.Vec tight.compile.Body)
      tight.TightError) : Prop where
  within : processed ≤ program.value.nodes.val.length - position
  allowance : after.max = before.max
  control : after.cancellation = before.cancellation
  charged : after.used.val = before.used.val + processed
  bounded : after.used.val ≤ after.max.val
  completed : ∀ output, answer = .Ok output →
    output.val = TightClassificationSemantics.classes program.value.nodes.val ∧
      processed = program.value.nodes.val.length - position

/-- The actual loop either returns the body's final result or continues with
all three fields that body returned. No loop-result equation is assumed. -/
theorem loop_unfold (program : theory.Theory) (iter : core.slice.iter.Iter theory.Node)
    (work : tight.Work) (classes : alloc.vec.Vec tight.compile.Body) :
    tight.compile.classify_loop iter program work classes = (do
      let transition ← tight.compile.classify_loop.body program iter work classes
      match transition with
      | .done result => ok result
      | .cont (next, charged, extended) =>
          tight.compile.classify_loop next program charged extended) := by
  rw [tight.compile.classify_loop, loop]
  dsimp only [uncurry]
  congr 1
  funext transition
  cases transition with
  | done result => rfl
  | cont result =>
      obtain ⟨next, charged, extended⟩ := result
      rfl

/-- Every invariant state has an actual finite generated execution. Each
continuation classifies one remaining node; a refused tick ends immediately.
The returned receipt counts exactly those continuations and preserves the
allowance and control. No successful-completion premise is needed. -/
theorem loop_refines (program : theory.Theory) (iter : core.slice.iter.Iter theory.Node)
    (work : tight.Work) (classes : alloc.vec.Vec tight.compile.Body)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (invariant : Invariant program iter classes) (budget : work.used.val ≤ work.max.val) :
    ∃ answer returned processed,
      tight.compile.classify_loop iter program work classes = ok (answer, returned) ∧
        Report program iter.i work returned processed answer := by
  have construct (remaining : Nat) :
      ∀ cursor : core.slice.iter.Iter theory.Node, ∀ before : tight.Work,
        ∀ entries : alloc.vec.Vec tight.compile.Body,
        Invariant program cursor entries → before.used.val ≤ before.max.val →
        program.value.nodes.val.length - cursor.i = remaining →
        ∃ answer returned processed,
          tight.compile.classify_loop cursor program before entries = ok (answer, returned) ∧
            Report program cursor.i before returned processed answer := by
    induction remaining using Nat.strong_induction_on with
    | h remaining inductionHypothesis =>
      intro cursor before entries current budget unvisited
      by_cases inside : cursor.i < program.value.nodes.val.length
      · have advanced : core.slice.iter.IteratorSliceIter.next cursor =
            ok (some program.value.nodes.val[cursor.i], { cursor with i := cursor.i + 1 }) := by
          simpa only [current.source] using EvaluatorIteration.slice_next_present cursor
            (by rw [current.source]; exact inside)
        rcases TightWork.refines before budget with refused | permitted
        · obtain ⟨reason, ticked⟩ := refused
          have stopped := TightClassificationStep.step_stopped program cursor
            { cursor with i := cursor.i + 1 } before before entries
            program.value.nodes.val[cursor.i] reason advanced ticked
          refine ⟨.Err reason, before, 0, ?_, ?_⟩
          · rw [loop_unfold, stopped]
            simp only [bind_tc_ok]
          · exact ⟨Nat.zero_le _, rfl, rfl, by simp, budget,
              by intro output impossible; cases impossible⟩
        · obtain ⟨used, incremented, withinBudget, ticked⟩ := permitted
          have initialized : entries.val.length = cursor.i :=
            prefix_length program cursor entries current
          have children : EvaluationSpecification.ChildrenBefore entries.val.length
              program.value.nodes.val[cursor.i] := by
            rw [initialized]
            exact ordered cursor.i inside
          have room : entries.val.length < Usize.max := by
            have stored := Slice.length_ineq cursor.slice
            rw [current.source] at stored
            omega
          obtain ⟨extended, stepped, appended⟩ := TightClassificationStep.step_after_tick
            program cursor { cursor with i := cursor.i + 1 } before
            { before with used := used } entries program.value.nodes.val[cursor.i]
            advanced ticked children (by rw [initialized]; exact Nat.le_of_lt inside) room
          have nextInvariant : Invariant program { cursor with i := cursor.i + 1 } extended :=
            advance_invariant program cursor entries extended current inside appended
          have less : program.value.nodes.val.length - (cursor.i + 1) < remaining := by omega
          obtain ⟨answer, returned, processed, executed, receipt⟩ := inductionHypothesis
            (program.value.nodes.val.length - (cursor.i + 1)) less
            { cursor with i := cursor.i + 1 } { before with used := used } extended
            nextInvariant withinBudget rfl
          refine ⟨answer, returned, processed + 1, ?_, ?_⟩
          · rw [loop_unfold, stepped]
            simpa only [bind_tc_ok] using executed
          · refine ⟨?_, receipt.allowance, receipt.control, ?_, receipt.bounded, ?_⟩
            · have bounded := receipt.within
              dsimp only at bounded
              omega
            · have charged := receipt.charged
              dsimp only at charged
              omega
            · intro output completed
              obtain ⟨exactClasses, exactCount⟩ := receipt.completed output completed
              refine ⟨exactClasses, ?_⟩
              dsimp only at exactCount
              omega
      · have finished : cursor.i = program.value.nodes.val.length := by
          have bounded := current.bounded
          omega
        have exhausted := TightClassificationStep.exhausted program cursor before entries
          (by rw [current.source]; omega)
        refine ⟨.Ok entries, before, 0, ?_, ?_⟩
        · rw [loop_unfold, exhausted]
          simp only [bind_tc_ok]
        · refine ⟨Nat.zero_le _, rfl, rfl, by simp, budget, ?_⟩
          intro output completed
          cases completed
          refine ⟨?_, by omega⟩
          simpa [finished, TightClassificationSemantics.classes] using current.classified
  exact construct (program.value.nodes.val.length - iter.i) iter work classes invariant budget rfl

/-- A successful generated scan returns the full class fold and charges exactly
one unit for every node remaining at its initial cursor. Its allowance and
control are unchanged. The result is identified from the constructed execution,
not assumed to agree with the finite fold. -/
theorem completed_classes (program : theory.Theory) (iter : core.slice.iter.Iter theory.Node)
    (work : tight.Work) (classes output : alloc.vec.Vec tight.compile.Body) (returned : tight.Work)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (invariant : Invariant program iter classes) (budget : work.used.val ≤ work.max.val)
    (completed : tight.compile.classify_loop iter program work classes =
      ok (.Ok output, returned)) :
    output.val = TightClassificationSemantics.classes program.value.nodes.val ∧
      returned.used.val = work.used.val + (program.value.nodes.val.length - iter.i) ∧
      returned.max = work.max ∧ returned.cancellation = work.cancellation := by
  obtain ⟨answer, after, processed, executed, receipt⟩ :=
    loop_refines program iter work classes ordered invariant budget
  have same : (answer, after) = (.Ok output, returned) :=
    Result.ok_injective (executed.symm.trans completed)
  cases same
  obtain ⟨exactClasses, exactCount⟩ := receipt.completed output rfl
  exact ⟨exactClasses, exactCount ▸ receipt.charged, receipt.allowance, receipt.control⟩

end TightClassificationLoop
