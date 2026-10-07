import NativeExecution
import NativeRootScan

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract
open Zetesis Ferraris TightEvaluation

/-!
# Asserted formulas of the native paired table

Roots select decoded native formulas in stored order, including duplicate roots.
The table decoder is the same one justified by NativeExecution. The finite root
selection below only connects that decoder to the existing Ferraris Models and
ReductTheory definitions; it introduces no second formula graph or semantics.
-/
namespace NativeRootSemantics

/-- The formulas named by the actual stored root occurrences. -/
def assertions (program : theory.Theory) : Theory Nat :=
  program.value.roots.val.map (fun root =>
    (NativeDenotation.meanings (NativeTable.rows (NativeExecution.view program))).getD root.val .bot)

/-- Selecting original truth cells is satisfaction of precisely those formulas.
The totalized missing cell denotes falsum on both sides; admitted roots separately
establish that the runtime scan never performs a missing read. -/
theorem original_values (program : theory.Theory) (candidate : Nat → Bool) :
    rootsTrue ((NativeDenotation.meanings (NativeTable.rows (NativeExecution.view program))).map
      (formulaValue candidate)) (program.value.roots.val.map UScalar.val) = true ↔
      Models (interpretation candidate) (assertions program) := by
  have cell (root : Usize) :
      (((NativeDenotation.meanings (NativeTable.rows (NativeExecution.view program))).map
        (formulaValue candidate)).getD root.val false) =
      formulaValue candidate ((NativeDenotation.meanings
        (NativeTable.rows (NativeExecution.view program))).getD root.val .bot) := by
    simp only [List.getD_eq_getElem?_getD, List.getElem?_map]
    cases (NativeDenotation.meanings (NativeTable.rows (NativeExecution.view program)))[root.val]? <;> rfl
  simp only [rootsTrue, List.all_map, Function.comp_def, List.all_eq_true, cell,
    formula_value_true, Models, assertions, List.mem_map]
  constructor
  · intro checked formula member
    obtain ⟨root, member, rfl⟩ := member
    exact checked root member
  · intro models root member
    exact models _ ⟨root, member, rfl⟩

/-- Selecting frozen truth cells is satisfaction of the explicit Ferraris
reducts of the selected formulas. Outer and tested interpretations are arbitrary. -/
theorem frozen_values (program : theory.Theory) (outer tested : Nat → Bool) :
    rootsTrue ((NativeDenotation.meanings (NativeTable.rows (NativeExecution.view program))).map
      (fun formula => formulaValue tested (Reduct (interpretation outer) formula)))
      (program.value.roots.val.map UScalar.val) = true ↔
      Models (interpretation tested) (ReductTheory (interpretation outer) (assertions program)) := by
  have cell (root : Usize) :
      (((NativeDenotation.meanings (NativeTable.rows (NativeExecution.view program))).map
        (fun formula => formulaValue tested (Reduct (interpretation outer) formula))).getD root.val false) =
      formulaValue tested (Reduct (interpretation outer) ((NativeDenotation.meanings
        (NativeTable.rows (NativeExecution.view program))).getD root.val .bot)) := by
    simp only [List.getD_eq_getElem?_getD, List.getElem?_map]
    cases (NativeDenotation.meanings (NativeTable.rows (NativeExecution.view program)))[root.val]? <;> rfl
  simp only [rootsTrue, List.all_map, Function.comp_def, List.all_eq_true, cell,
    formula_value_true, Models, ReductTheory, assertions, List.mem_map]
  constructor
  · intro checked formula member
    obtain ⟨original, ⟨root, member, rfl⟩, rfl⟩ := member
    exact checked root member
  · intro models root member
    exact models _ ⟨_, ⟨root, member, rfl⟩, rfl⟩

/-- Actual completed original evaluation supplies the root truth table. -/
theorem original_roots (program : theory.Theory) (candidate : theory.Interpretation)
    (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (valid : NativeStructure.WellFormed program.value.atoms.val (NativeExecution.view program))
    (stored : NativeMembership.Represented candidate)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (completed : oracle.evaluate program candidate none old before = ok (.Ok (), output, after)) :
    rootsTrue output.val (program.value.roots.val.map UScalar.val) = true ↔
      Models (interpretation (NativeMembership.denotes candidate)) (assertions program) := by
  rw [NativeExecution.completed_original program candidate old before valid stored output after completed]
  exact original_values program (NativeMembership.denotes candidate)

/-- The actual two completed evaluator calls establish arbitrary-J frozen
root satisfaction, without a supplied mask-agreement or subset hypothesis. -/
theorem reduct_roots (program : theory.Theory) (outer tested : theory.Interpretation)
    (oldOriginal oldTested : alloc.vec.Vec Bool) (outerWork testedWork : oracle.Work)
    (valid : NativeStructure.WellFormed program.value.atoms.val (NativeExecution.view program))
    (outerStored : NativeMembership.Represented outer) (testedStored : NativeMembership.Represented tested)
    (original result : alloc.vec.Vec Bool) (outerReturned testedReturned : oracle.Work)
    (first : oracle.evaluate program outer none oldOriginal outerWork =
      ok (.Ok (), original, outerReturned))
    (second : oracle.evaluate program tested (some (alloc.vec.Vec.deref original)) oldTested testedWork =
      ok (.Ok (), result, testedReturned)) :
    rootsTrue result.val (program.value.roots.val.map UScalar.val) = true ↔
      Models (interpretation (NativeMembership.denotes tested))
        (ReductTheory (interpretation (NativeMembership.denotes outer)) (assertions program)) := by
  rw [NativeExecution.completed_frozen program outer tested oldOriginal oldTested outerWork testedWork
    valid outerStored testedStored original result outerReturned testedReturned first second]
  exact frozen_values program (NativeMembership.denotes outer) (NativeMembership.denotes tested)

/-- Actual original evaluation followed by the actual root scan decides
satisfaction of the asserted native theory. The two phase records remain
explicit; neither typed refusal can establish this completed verdict. -/
theorem completed_original (program : theory.Theory) (candidate : theory.Interpretation)
    (old : alloc.vec.Vec Bool) (evaluationWork scanWork : oracle.Work)
    (valid : NativeStructure.WellFormed program.value.atoms.val (NativeExecution.view program))
    (stored : NativeMembership.Represented candidate)
    (rootsBounded : ∀ root ∈ program.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view program)).length)
    (output : alloc.vec.Vec Bool) (evaluationReturned scanReturned : oracle.Work) (answer : Option Usize)
    (evaluated : oracle.evaluate program candidate none old evaluationWork = ok (.Ok (), output, evaluationReturned))
    (scanned : oracle.failed_root program output.slice scanWork = ok (.Ok answer, scanReturned)) :
    answer = none ↔ Models (interpretation (NativeMembership.denotes candidate)) (assertions program) := by
  have outputValues : output.val = NativeSpecification.values candidate none
      (NativeTable.rows (NativeExecution.view program)) := by
    exact NativeExecution.completed_values program candidate none old evaluationWork valid stored
      (by simp) output evaluationReturned evaluated
  have rootsCovered : ∀ root ∈ program.value.roots.val, root.val < output.slice.val.length := by
    intro root member
    change root.val < output.val.length
    rw [outputValues, NativeSpecification.values_length]
    exact rootsBounded root member
  calc
    answer = none ↔ ∀ root ∈ program.value.roots.val, NativeRootScan.truth output.slice root = true :=
      NativeRootScan.completed_none_iff program output.slice scanWork scanReturned answer rootsCovered scanned
    _ ↔ rootsTrue output.val (program.value.roots.val.map UScalar.val) = true := by
      simp [rootsTrue, List.all_eq_true, NativeRootScan.truth, List.getD_eq_getElem?_getD]
      rfl
    _ ↔ _ := original_roots program candidate old evaluationWork valid stored output evaluationReturned evaluated

/-- The actual original pass, arbitrary-J frozen pass and root scan jointly
decide Ferraris reduct satisfaction. The first pass supplies the exact mask;
the second pass and root bounds justify all actual root reads. -/
theorem completed_reduct (program : theory.Theory) (outer tested : theory.Interpretation)
    (oldOriginal oldTested : alloc.vec.Vec Bool) (outerWork testedWork scanWork : oracle.Work)
    (valid : NativeStructure.WellFormed program.value.atoms.val (NativeExecution.view program))
    (outerStored : NativeMembership.Represented outer) (testedStored : NativeMembership.Represented tested)
    (rootsBounded : ∀ root ∈ program.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view program)).length)
    (original result : alloc.vec.Vec Bool) (outerReturned testedReturned scanReturned : oracle.Work)
    (answer : Option Usize)
    (first : oracle.evaluate program outer none oldOriginal outerWork = ok (.Ok (), original, outerReturned))
    (second : oracle.evaluate program tested (some (alloc.vec.Vec.deref original)) oldTested testedWork =
      ok (.Ok (), result, testedReturned))
    (scanned : oracle.failed_root program result.slice scanWork = ok (.Ok answer, scanReturned)) :
    answer = none ↔ Models (interpretation (NativeMembership.denotes tested))
      (ReductTheory (interpretation (NativeMembership.denotes outer)) (assertions program)) := by
  have resultValues : result.val = (NativeDenotation.meanings (NativeTable.rows (NativeExecution.view program))).map
      (fun formula => formulaValue (NativeMembership.denotes tested)
        (Reduct (interpretation (NativeMembership.denotes outer)) formula)) := by
    exact NativeExecution.completed_frozen program outer tested oldOriginal oldTested outerWork testedWork
      valid outerStored testedStored original result outerReturned testedReturned first second
  have rootsCovered : ∀ root ∈ program.value.roots.val, root.val < result.slice.val.length := by
    intro root member
    change root.val < result.val.length
    rw [resultValues, List.length_map, NativeDenotation.meanings_length]
    exact rootsBounded root member
  calc
    answer = none ↔ ∀ root ∈ program.value.roots.val, NativeRootScan.truth result.slice root = true :=
      NativeRootScan.completed_none_iff program result.slice scanWork scanReturned answer rootsCovered scanned
    _ ↔ rootsTrue result.val (program.value.roots.val.map UScalar.val) = true := by
      simp [rootsTrue, List.all_eq_true, NativeRootScan.truth, List.getD_eq_getElem?_getD]
      rfl
    _ ↔ _ := reduct_roots program outer tested oldOriginal oldTested outerWork testedWork valid
      outerStored testedStored original result outerReturned testedReturned first second

end NativeRootSemantics
