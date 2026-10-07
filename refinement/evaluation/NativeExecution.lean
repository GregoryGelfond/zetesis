import NativeLoop
import NativeDenotation

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract
open Zetesis Ferraris TightEvaluation

/-!
# Original and frozen results of the actual native evaluator

The generated setup clears the old output and runs the proved native range loop.
A completed first pass establishes the mask consumed by the second pass, so the
frozen theorem assumes no agreement between stored Boolean cells and semantics.
Both results are about the actual extracted evaluator under its imported scalar,
sequence, ownership and fixed atomic-observation models. Runtime allocation,
concurrent observations and hardware execution remain separate boundaries.
-/
namespace NativeExecution

/-- The paired slices returned by the generated theory getter. -/
def view (program : theory.Theory) : theory.FormulaView :=
  ⟨alloc.vec.Vec.deref program.value.parts.nodes, alloc.vec.Vec.deref program.value.parts.operands⟩

/-- The generated range begins at zero and ends at the actual logical-node count. -/
def initialCursor (program : theory.Theory) : NativeStep.Cursor :=
  ⟨0#usize, Slice.len (view program).nodes⟩

/-- Generated setup discards old output values and supplies the actual paired
view to the actual loop; no assumed setup equation or old-prefix premise is used. -/
theorem evaluate_from_empty (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (oldOutput : alloc.vec.Vec Bool) (work : oracle.Work) :
    oracle.evaluate program candidate frozen oldOutput work =
      oracle.evaluate_loop (initialCursor program) candidate frozen (alloc.vec.Vec.new Bool)
        work (view program).nodes (view program).operands := by
  simp [oracle.evaluate, alloc.vec.Vec.clear, theory.Theory.view,
    alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, theory.FormulaParts.view,
    theory.FormulaView.len, view, initialCursor]

/-- Setup establishes the loop's empty-prefix invariant for every supplied
candidate and mask, independently of the discarded output's previous contents. -/
theorem initial_invariant (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) :
    NativeLoop.Invariant (view program) candidate frozen (initialCursor program)
      (alloc.vec.Vec.new Bool) := by
  constructor
  · simp [initialCursor, NativeTable.length]
  · simp [initialCursor]
  · simp [initialCursor, NativeSpecification.prefixValues, NativeSpecification.values]

/-- The actual evaluator returns an exact truth prefix and a complete receipt,
including work spent inside the final refused node. Table validity is structural;
no evaluator-result agreement is a hypothesis. -/
theorem evaluate_refines (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (oldOutput : alloc.vec.Vec Bool) (work : oracle.Work)
    (valid : NativeStructure.WellFormed program.value.atoms.val (view program))
    (stored : NativeMembership.Represented candidate)
    (covered : ∀ mask ∈ frozen, (NativeTable.rows (view program)).length ≤ mask.val.length) :
    ∃ outcome : NativeLoop.Outcome,
      oracle.evaluate program candidate frozen oldOutput work =
        ok (outcome.result, outcome.output, outcome.work) ∧
      NativeLoop.Preserved (view program) candidate frozen (initialCursor program) work outcome := by
  rw [evaluate_from_empty]
  exact NativeLoop.loop_refines program.value.atoms.val (view program) candidate frozen
    (initialCursor program) (alloc.vec.Vec.new Bool) work valid stored covered
    (initial_invariant program candidate frozen)

/-- Source success from the generated evaluator returns the complete specified
native truth table. A typed refusal cannot establish this conclusion. -/
theorem completed_values (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (oldOutput : alloc.vec.Vec Bool) (work : oracle.Work)
    (valid : NativeStructure.WellFormed program.value.atoms.val (view program))
    (stored : NativeMembership.Represented candidate)
    (covered : ∀ mask ∈ frozen, (NativeTable.rows (view program)).length ≤ mask.val.length)
    (output : alloc.vec.Vec Bool) (returned : oracle.Work)
    (completed : oracle.evaluate program candidate frozen oldOutput work =
      ok (.Ok (), output, returned)) :
    output.val = NativeSpecification.values candidate frozen (NativeTable.rows (view program)) := by
  obtain ⟨outcome, actual, preserved⟩ :=
    evaluate_refines program candidate frozen oldOutput work valid stored covered
  have same : (outcome.result, outcome.output, outcome.work) = (.Ok (), output, returned) := by
    exact Result.ok_injective (actual.symm.trans completed)
  have resultSame : outcome.result = .Ok () := by
    exact congrArg Prod.fst same
  have outputSame : outcome.output = output := by
    exact congrArg (fun result => result.2.1) same
  have boundary : outcome.position = (NativeTable.rows (view program)).length ∧
      outcome.pendingWork = 0 := by
    simpa only [resultSame] using preserved.boundary
  rw [← outputSame, preserved.values, boundary.1, NativeSpecification.prefix_complete]

/-- A complete native evaluation charges exactly one tick per logical node and
per operand occurrence, in addition to the caller's prior work. -/
theorem completed_work (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (oldOutput : alloc.vec.Vec Bool) (work : oracle.Work)
    (valid : NativeStructure.WellFormed program.value.atoms.val (view program))
    (stored : NativeMembership.Represented candidate)
    (covered : ∀ mask ∈ frozen, (NativeTable.rows (view program)).length ≤ mask.val.length)
    (output : alloc.vec.Vec Bool) (returned : oracle.Work)
    (completed : oracle.evaluate program candidate frozen oldOutput work =
      ok (.Ok (), output, returned)) :
    returned.statistics.work.val = work.statistics.work.val +
      (NativeTable.rows (view program)).length +
      ((NativeTable.rows (view program)).map NativeEvaluation.occurrences).sum := by
  obtain ⟨outcome, actual, preserved⟩ :=
    evaluate_refines program candidate frozen oldOutput work valid stored covered
  have same : (outcome.result, outcome.output, outcome.work) = (.Ok (), output, returned) := by
    exact Result.ok_injective (actual.symm.trans completed)
  have resultSame : outcome.result = .Ok () := by
    exact congrArg Prod.fst same
  have workSame : outcome.work = returned := by
    exact congrArg (fun result => result.2.2) same
  have boundary : outcome.position = (NativeTable.rows (view program)).length ∧
      outcome.pendingWork = 0 := by
    simpa only [resultSame] using preserved.boundary
  have charged : outcome.work.statistics.work.val +
      NativeSpecification.cost ((NativeTable.rows (view program)).take (initialCursor program).start.val) =
    work.statistics.work.val +
      NativeSpecification.cost ((NativeTable.rows (view program)).take outcome.position) +
      outcome.pendingWork := by
    exact preserved.accounting
  have started : (initialCursor program).start.val = 0 := by
    rfl
  rw [started, List.take_zero, boundary.1, List.take_length, boundary.2, workSame] at charged
  simpa only [show NativeSpecification.cost [] = 0 by rfl, Nat.add_zero,
    NativeSpecification.cost_dimensions, Nat.add_assoc] using charged

/-- A completed generated original pass computes each decoded formula's truth.
The reference table comes from the same paired storage that the evaluator reads. -/
theorem completed_original (program : theory.Theory) (candidate : theory.Interpretation)
    (oldOutput : alloc.vec.Vec Bool) (work : oracle.Work)
    (valid : NativeStructure.WellFormed program.value.atoms.val (view program))
    (stored : NativeMembership.Represented candidate)
    (output : alloc.vec.Vec Bool) (returned : oracle.Work)
    (completed : oracle.evaluate program candidate none oldOutput work =
      ok (.Ok (), output, returned)) :
    output.val = (NativeDenotation.meanings (NativeTable.rows (view program))).map
      (formulaValue (NativeMembership.denotes candidate)) := by
  rw [completed_values program candidate none oldOutput work valid stored
    (by simp) output returned completed]
  exact NativeDenotation.original candidate (NativeTable.rows (view program))
    (NativeTable.ordered program.value.atoms.val (view program) valid)

/-- A completed generated first pass establishes its own original mask. A
completed generated second pass using that exact mask computes the explicit
Ferraris reduct at every node for arbitrary outer and tested interpretations.
Neither candidate equality nor a tested-subset premise is required. -/
theorem completed_frozen (program : theory.Theory) (outer tested : theory.Interpretation)
    (oldOriginal oldFrozen : alloc.vec.Vec Bool) (originalWork frozenWork : oracle.Work)
    (valid : NativeStructure.WellFormed program.value.atoms.val (view program))
    (outerStored : NativeMembership.Represented outer)
    (testedStored : NativeMembership.Represented tested)
    (original output : alloc.vec.Vec Bool) (returnedOriginal returnedFrozen : oracle.Work)
    (first : oracle.evaluate program outer none oldOriginal originalWork =
      ok (.Ok (), original, returnedOriginal))
    (second : oracle.evaluate program tested (some (alloc.vec.Vec.deref original)) oldFrozen frozenWork =
      ok (.Ok (), output, returnedFrozen)) :
    output.val = (NativeDenotation.meanings (NativeTable.rows (view program))).map
      (fun formula => formulaValue (NativeMembership.denotes tested)
        (Reduct (interpretation (NativeMembership.denotes outer)) formula)) := by
  have originalValues : original.val =
      NativeSpecification.values outer none (NativeTable.rows (view program)) := by
    exact completed_values program outer none oldOriginal originalWork valid
      outerStored (by simp) original returnedOriginal first
  have ordered : NativeSpecification.Ordered (NativeTable.rows (view program)) := by
    exact NativeTable.ordered program.value.atoms.val (view program) valid
  have coverage : ∀ mask ∈ some (alloc.vec.Vec.deref original),
      (NativeTable.rows (view program)).length ≤ mask.val.length := by
    intro mask member
    have maskSame : mask = alloc.vec.Vec.deref original := by
      exact (show alloc.vec.Vec.deref original = mask by simpa using member).symm
    rw [maskSame]
    simp [alloc.vec.Vec.deref, originalValues, NativeSpecification.values_length]
  rw [completed_values program tested (some (alloc.vec.Vec.deref original)) oldFrozen frozenWork
    valid testedStored coverage output returnedFrozen second]
  exact NativeDenotation.frozen outer tested (alloc.vec.Vec.deref original)
    (NativeTable.rows (view program)) ordered
    (NativeDenotation.original_mask outer (alloc.vec.Vec.deref original)
      (NativeTable.rows (view program)) ordered (by simpa [alloc.vec.Vec.deref] using originalValues))

end NativeExecution
