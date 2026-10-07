import NativeMembershipSearch
import NativePackedStorage

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract
open Zetesis Zetesis.Refinement

/-!
# Zero storage for the reference subset search

The public checker reserves an empty vector and resizes it with zero words.
The backend's actual resize operation establishes the empty interpretation
required by the search proof. Exact candidate storage is a constructor
invariant, stronger than the read-safety premise `NativeMembership.Represented`.

These laws use the sequence model of resize. Successful reservation, sufficient
physical capacity and program ownership remain separate runtime contracts.
No allocation success or constructor correspondence is inferred here.
-/
namespace NativePackedSetup

/-- Every successful resize used for the initial subset denotes the empty set,
including zero padding. The requested length comes from the candidate's exact
storage invariant, not from an assumed semantic meaning of the output. -/
theorem resized_empty (candidate : theory.Interpretation)
    (shape : NativePackedStorage.ExactStorage candidate) (vacant : alloc.vec.Vec U64)
    (empty : vacant.val = []) (count : Usize)
    (countExact : count.val = candidate.words.val.length)
    (words : alloc.vec.Vec U64)
    (resized : alloc.vec.Vec.resize core.clone.CloneU64 vacant count (0#u64) = ok words) :
    PackedSubsets.Represents (ScalarSubsets.raw words)
      ([] : List (Fin candidate.theory.value.atoms.val)) := by
  obtain ⟨produced, actual, contents⟩ := NativePackedStorage.resize_zero vacant count empty
  have same : produced = words := by
    exact Result.ok_injective (actual.symm.trans resized)
  have countShape : count.val = PackedInterpretations.count64
      candidate.theory.value.atoms.val := by
    exact countExact.trans shape
  have rawZero : ScalarSubsets.raw words = List.replicate
      (PackedInterpretations.count64 candidate.theory.value.atoms.val) (0#64) := by
    rw [← same]
    simp only [ScalarSubsets.raw, contents, List.map_replicate, countShape]
    rfl
  rw [rawZero]
  simpa [PackedInterpretations.pack, PackedInterpretations.insertAll] using
    (PackedSubsets.pack_represents ([] : List (Fin candidate.theory.value.atoms.val)))

/-- Initializing the search's words establishes both its exact storage and its
empty semantic interpretation. The caller supplies the program value retained
by the owner clone; identity preservation is a distinct ownership obligation. -/
theorem initialized_interpretation (candidate : theory.Interpretation)
    (shape : NativePackedStorage.ExactStorage candidate) (program : theory.Theory)
    (sameUniverse : program.value.atoms = candidate.theory.value.atoms)
    (vacant : alloc.vec.Vec U64) (empty : vacant.val = []) (count : Usize)
    (countExact : count.val = candidate.words.val.length)
    (words : alloc.vec.Vec U64)
    (resized : alloc.vec.Vec.resize core.clone.CloneU64 vacant count (0#u64) = ok words) :
    let subset : theory.Interpretation := { theory := program, words := words }
    NativePackedStorage.ExactStorage subset ∧ NativeMembership.Represented subset ∧
      NativeMembership.denotes subset = FiniteMembership.candidate [] := by
  have represented : PackedSubsets.Represents (ScalarSubsets.raw words)
      ([] : List (Fin candidate.theory.value.atoms.val)) := by
    exact resized_empty candidate shape vacant empty count countExact words resized
  have sizeExact : program.value.atoms.val = candidate.theory.value.atoms.val := by
    exact congrArg UScalar.val sameUniverse
  have exact : NativePackedStorage.ExactStorage { theory := program, words := words } := by
    simpa only [NativePackedStorage.ExactStorage, ScalarSubsets.raw, List.length_map, sameUniverse] using represented.1
  exact ⟨exact, NativePackedStorage.exact_storage_readable _ exact,
    NativeSearchRepresentation.denotes [] { theory := program, words := words } sizeExact represented⟩

/-- The actual zero-resize result discharges the empty-subset premise in the
completed membership theorem. Selection and search still refer to the generated
functions, with work threaded through the original root scan. Reservation and
owner checks are not represented by these completion equations. -/
theorem initialized_membership (frozen : NativeCountermodelSemantics.FrozenEvaluation)
    (shape : NativePackedStorage.ExactStorage frozen.candidate) (rootAfter : oracle.Work)
    (originalModel : oracle.failed_root frozen.program frozen.values.slice frozen.after =
      ok (core.result.Result.Ok none, rootAfter))
    (destination : alloc.vec.Vec Usize)
    (sameUniverse : frozen.program.value.atoms = frozen.candidate.theory.value.atoms)
    (vacant : destination.val = [])
    (selected : alloc.vec.Vec Usize) (selectionAfter : oracle.Work)
    (selectedComplete : oracle.select_atoms frozen.program frozen.candidate destination rootAfter =
      ok (core.result.Result.Ok (), selected, selectionAfter))
    (wordDestination : alloc.vec.Vec U64) (wordVacant : wordDestination.val = [])
    (count : Usize) (countExact : count.val = frozen.candidate.words.val.length)
    (words : alloc.vec.Vec U64)
    (resized : alloc.vec.Vec.resize core.clone.CloneU64 wordDestination count (0#u64) = ok words)
    (old output : alloc.vec.Vec Bool) (found : Bool)
    (subset : theory.Interpretation) (after : oracle.Work)
    (completed : oracle.find_countermodel frozen.program frozen.values.slice selected.slice
      { theory := frozen.program, words := words } old selectionAfter =
      ok (core.result.Result.Ok found, subset, output, after)) :
    found = false ↔ Ferraris.Stable
      (TightEvaluation.interpretation (NativeMembership.denotes frozen.candidate))
      (NativeRootSemantics.assertions frozen.program) := by
  have zero : PackedSubsets.Represents (ScalarSubsets.raw words)
      ([] : List (Fin frozen.candidate.theory.value.atoms.val)) := by
    exact resized_empty frozen.candidate shape wordDestination wordVacant
      count countExact words resized
  exact NativeMembershipSearch.completed_answer_set frozen rootAfter originalModel destination
    sameUniverse vacant selected selectionAfter selectedComplete
    { theory := frozen.program, words := words } old
    (congrArg UScalar.val sameUniverse) zero found subset output after completed

end NativePackedSetup
