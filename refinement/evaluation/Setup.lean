import Progress

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Setup of the generated formula evaluator

The generated evaluator clears the supplied Boolean output, reads the theory's
node slice, and starts the slice iterator and its enumeration at zero before
calling the generated loop. This module derives that setup directly from the
generated body and the imported library models; it assumes no setup equation.

The resulting cursor is aligned with the empty truth prefix independently of
the candidate, mask and work record. No loop execution, candidate-owner check,
formula admission, allocation or temporal atomic behavior is established here.
The pure external models and the Aeneas slice/iterator models remain explicit
correspondence boundaries.
-/
namespace EvaluatorSetup

/-- The generated loop's initial cursor enumerates the actual stored node slice
from position zero. The scalar count and slice index both start at zero. -/
def initialCursor (program : theory.Theory) : Evaluation.Cursor :=
  { iter := { slice := alloc.vec.Vec.deref program.value.nodes, i := 0 },
    count := 0#usize }

/-- The actual generated evaluator calls its actual loop with an empty Boolean
output, the stored node slice at position zero, and the supplied work fields
unchanged. The old output is discarded in the logical vector model. The
candidate and optional frozen mask are passed through unchanged, without an
owner or mask-validity premise.

Proof: compute the generated clear and node-read operations, then compute the
backend slice and enumeration constructors. Substitute these four successful
results into the generated evaluator; the remaining expression is its loop
call. This establishes setup only and makes no claim that the loop completes. -/
theorem evaluate_from_empty (program : theory.Theory)
    (candidate : theory.Interpretation) (frozen : Option (Slice Bool))
    (output : alloc.vec.Vec Bool) (work : oracle.Work) :
    oracle.evaluate program candidate frozen output work =
      oracle.evaluate_loop (initialCursor program) candidate frozen
        (alloc.vec.Vec.new Bool) work.limits work.cancellation work.statistics := by
  have cleared : alloc.vec.Vec.clear Global output =
      ok (alloc.vec.Vec.new Bool) := by
    rfl
  have nodesRead : theory.Theory.nodes program =
      ok (alloc.vec.Vec.deref program.value.nodes) := by
    simp [theory.Theory.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref]
  have sliceStarted : core.slice.Slice.iter (alloc.vec.Vec.deref program.value.nodes) =
      ok { slice := alloc.vec.Vec.deref program.value.nodes, i := 0 } := by
    rfl
  have enumerated : core.iter.traits.iterator.Iterator.enumerate.trait_default
      (core.iter.traits.iterator.IteratorSliceIter theory.Node)
      { slice := alloc.vec.Vec.deref program.value.nodes, i := 0 } =
      ok (initialCursor program) := by
    rfl
  simp only [oracle.evaluate, cleared, bind_tc_ok, nodesRead, sliceStarted, enumerated]

/-- Setup retains exactly the theory's node sequence, initializes both cursor
positions to zero, and aligns them with the empty Boolean truth prefix. The
cursor is within the stored slice even for an empty theory. These facts require
no candidate, mask, control observation or work allowance. -/
theorem initial_alignment (program : theory.Theory) :
    (initialCursor program).iter.slice.val = program.value.nodes.val ∧
    (initialCursor program).iter.i = 0 ∧
    (initialCursor program).count.val = 0 ∧
    (alloc.vec.Vec.new Bool).val = [] ∧
    (initialCursor program).count.val = (initialCursor program).iter.i ∧
    (alloc.vec.Vec.new Bool).val.length = (initialCursor program).iter.i ∧
    (initialCursor program).iter.i ≤ (initialCursor program).iter.slice.val.length := by
  have sourceRetained : (initialCursor program).iter.slice.val =
      program.value.nodes.val := by
    simp [initialCursor, alloc.vec.Vec.deref]
  have positionsZero : (initialCursor program).iter.i = 0 ∧
      (initialCursor program).count.val = 0 := by
    exact ⟨rfl, rfl⟩
  have emptyPrefix : (alloc.vec.Vec.new Bool).val = [] := by
    rfl
  refine ⟨sourceRetained, positionsZero.1, positionsZero.2, emptyPrefix, ?_, ?_, ?_⟩
  · rw [positionsZero.1, positionsZero.2]
  · rw [emptyPrefix, positionsZero.1]
    rfl
  · rw [positionsZero.1]
    exact Nat.zero_le _

end EvaluatorSetup
