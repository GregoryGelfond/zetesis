import NativeInterpretationConstruction
import NativePackedStorage
import NativeVectorInput

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract
open Zetesis.Refinement

/-!
# Initial storage of an actually constructed interpretation

The constructor requests reservation on an empty vector, then resizes with zero.
Reservation has no sequence law in its supplied interface. These results state
that library contract explicitly for this invocation: any successful reservation
at the actual computed count preserves the empty logical input. This does not
assume reservation succeeds, or establish capacity, allocation identity or an
allocator history.

The actual count and resize operations then produce exactly the packed empty
interpretation. A successful constructor supplies its own internal calls; its
initial words are not an assumed representation. The actual finite insertion
loop then establishes the returned packed contents and admits every input atom.
The owned-vector specialization derives its iterator contract from actual
standard-library operations. Readable storage, padding and extracted membership
queries follow from the existing packed representation laws.

These results concern completed construction under supplied reservation and
iterator operations. They do not establish allocation success, termination of an
arbitrary runtime iterator, destructor behavior or a changing allocator history.
-/
namespace NativeInterpretationStorage

/-- The successful reservation for this constructor's actual word count retains
its empty input sequence. The implication permits source refusal, backend
failure and divergence; it is not an allocation-success or capacity premise. -/
def PreservesEmpty [reservation : VectorReservation] (atoms : Usize) : Prop :=
  ∀ count words,
    core.num.Usize.div_ceil atoms 64#usize = ok count →
    alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new U64) count =
      ok (.Ok (), words) →
    words.val = []

/-- Actual ceiling division followed by resize of an empty vector produces the
exact zero words for the declared universe, including all padding bits.

Proof: the word-count law identifies the count; the existing backend resize law
supplies its returned vector. Mapping those zero words to bits yields the shared
empty packed representation. No theory admission or allocation success is used. -/
theorem counted_zero (atoms count : Usize)
    (counted : core.num.Usize.div_ceil atoms 64#usize = ok count)
    (vacant : alloc.vec.Vec U64) (empty : vacant.val = []) :
    ∃ words : alloc.vec.Vec U64,
      alloc.vec.Vec.resize core.clone.CloneU64 vacant count (0#u64) = ok words ∧
      NativePackedStorage.raw words = PackedInterpretations.pack atoms.val [] ∧
      PackedSubsets.Represents (NativePackedStorage.raw words) ([] : List (Fin atoms.val)) := by
  obtain ⟨actualCount, actualCounted, countValue⟩ := UsizeCeiling.word_count64 atoms
  have sameCount : actualCount = count := by
    exact Result.ok_injective (actualCounted.symm.trans counted)
  have countExact : count.val = PackedInterpretations.count64 atoms.val := by
    simpa only [sameCount] using countValue
  obtain ⟨words, resized, contents⟩ := NativePackedStorage.resize_zero vacant count empty
  have rawZero : NativePackedStorage.raw words = PackedInterpretations.pack atoms.val [] := by
    simp only [NativePackedStorage.raw, contents, List.map_replicate, countExact,
      PackedInterpretations.pack, PackedInterpretations.insertAll]
    rfl
  have represented : PackedSubsets.Represents (NativePackedStorage.raw words)
      ([] : List (Fin atoms.val)) := by
    rw [rawZero]
    exact PackedSubsets.pack_represents []
  exact ⟨words, resized, rawZero, represented⟩

/-- A successful actual constructor reaches insertion with the exact packed
empty interpretation when its reservation obeys the stated library contract.
The constructor itself supplies conversion, insertion and returned-word equations;
no semantic property of the insertion result is assumed or concluded here.

Proof: obtain the actual phases from constructor completion, apply the reservation
contract to that call, and identify its resize output with the zero result above. -/
theorem completed_initialization [reservation : VectorReservation] {Input Iter : Type}
    (source : core.iter.traits.collect.IntoIterator Input Usize Iter)
    (program : theory.Theory) (atoms : Input) (result : theory.Interpretation)
    (preserved : PreservesEmpty program.value.atoms)
    (completed : theory.Interpretation.new source program atoms = ok (.Ok result)) :
    ∃ initialized : alloc.vec.Vec U64, ∃ iterator : Iter, ∃ stored : Slice U64,
      source.into_iter atoms = ok iterator ∧
      NativePackedStorage.raw initialized = PackedInterpretations.pack program.value.atoms.val [] ∧
      PackedSubsets.Represents (NativePackedStorage.raw initialized)
        ([] : List (Fin program.value.atoms.val)) ∧
      theory.insert_atoms source.iteratorInst program.value.atoms iterator initialized.slice =
        ok (.Ok (), stored) ∧
      result = { theory := program, words := { slice := stored } } := by
  obtain ⟨count, vacant, initialized, iterator, stored, counted, _, reserved,
    resized, converted, inserted, resultExact⟩ :=
    NativeInterpretationConstruction.completed_phases source program atoms result completed
  have empty : vacant.val = [] := by
    exact preserved count vacant counted reserved
  obtain ⟨zeroWords, zeroResized, rawZero, represented⟩ :=
    counted_zero program.value.atoms count counted vacant empty
  have sameWords : zeroWords = initialized := by
    exact Result.ok_injective (zeroResized.symm.trans resized)
  have initialRaw : NativePackedStorage.raw initialized =
      PackedInterpretations.pack program.value.atoms.val [] := by
    simpa only [sameWords] using rawZero
  have initialRepresents : PackedSubsets.Represents (NativePackedStorage.raw initialized)
      ([] : List (Fin program.value.atoms.val)) := by
    simpa only [sameWords] using represented
  exact ⟨initialized, iterator, stored, converted, initialRaw, initialRepresents,
    inserted, resultExact⟩

/-- Identifying the stored words with the shared packing operation supplies
exact storage, zero padding and the actual extracted membership answers. The
input coordinates must lie in the owner's universe; repetitions are allowed.
This is the representation consequence used after the actual insertion proof,
not an assumption about a constructor's output.

Proof: apply the existing packing law, derive read safety from exact length,
and use the extracted membership refinement for each queried machine atom. -/
theorem packed_queries (result : theory.Interpretation) (atoms : List Nat)
    (inside : ∀ atom ∈ atoms, atom < result.theory.value.atoms.val)
    (packed : NativePackedStorage.raw result.words =
      PackedInterpretations.pack result.theory.value.atoms.val atoms) :
    NativePackedStorage.ExactStorage result ∧
      PackedInterpretations.ZeroPadding result.theory.value.atoms.val
        (NativePackedStorage.raw result.words) ∧
      ∀ atom : Usize, theory.Interpretation.contains result atom =
        ok (decide (atom.val ∈ atoms)) := by
  obtain ⟨length, membership, padding⟩ :=
    PackedInterpretations.pack_exact result.theory.value.atoms.val atoms inside
  have shape : NativePackedStorage.ExactStorage result := by
    have rawLength : (NativePackedStorage.raw result.words).length =
        (PackedInterpretations.pack result.theory.value.atoms.val atoms).length := by
      exact congrArg List.length packed
    simpa only [NativePackedStorage.ExactStorage, NativePackedStorage.raw, List.length_map, length]
      using rawLength
  have represented : NativeMembership.Represented result := by
    exact NativePackedStorage.exact_storage_readable result shape
  have zeroPadding : PackedInterpretations.ZeroPadding result.theory.value.atoms.val
      (NativePackedStorage.raw result.words) := by
    rw [packed]
    exact padding
  have queried (atom : Usize) : theory.Interpretation.contains result atom =
      ok (decide (atom.val ∈ atoms)) := by
    rw [NativeMembership.contains_refines result atom represented, NativePackedStorage.packed_denotation]
    rw [packed, membership]
  exact ⟨shape, zeroPadding, queried⟩

/-- A completed actual constructor packs exactly its finite iterator input and
retains the supplied theory. Every supplied atom is in the declared universe;
input admission is derived from successful insertion rather than assumed.

Proof: recover the constructor's actual conversion and zero initialization.
Its converted iterator equals the one whose next calls justify the finite input.
The actual insertion theorem supplies both admission and all packed writes;
substitution of the derived zero words yields the shared packing operation. -/
theorem completed_pack [reservation : VectorReservation] {Input Iter : Type}
    (source : core.iter.traits.collect.IntoIterator Input Usize Iter)
    (program : theory.Theory) (input : Input) (iterator : Iter) (atoms : List Usize)
    (converted : source.into_iter input = ok iterator)
    (finite : NativeInsertionLoop.FiniteInput source.iteratorInst iterator atoms)
    (result : theory.Interpretation)
    (preserved : PreservesEmpty program.value.atoms)
    (completed : theory.Interpretation.new source program input = ok (.Ok result)) :
    result.theory = program ∧
      (∀ atom ∈ atoms, atom.val < program.value.atoms.val) ∧
      NativePackedStorage.raw result.words =
        PackedInterpretations.pack program.value.atoms.val (atoms.map UScalar.val) := by
  obtain ⟨initialized, actualIterator, stored, actuallyConverted, initialRaw,
    initialRepresents, inserted, resultExact⟩ :=
    completed_initialization source program input result preserved completed
  have sameIterator : actualIterator = iterator := by
    exact Result.ok_injective (actuallyConverted.symm.trans converted)
  have covered : NativeInsertionLoop.Covers program.value.atoms initialized.slice := by
    intro atom inside
    have length : initialized.val.length =
        PackedInterpretations.count64 program.value.atoms.val := by
      simpa only [NativePackedStorage.raw, List.length_map] using initialRepresents.1
    change atom / 64 < initialized.val.length
    rw [length]
    unfold PackedInterpretations.count64
    omega
  have actualInsertion : theory.insert_atoms source.iteratorInst program.value.atoms
      iterator initialized.slice = ok (.Ok (), stored) := by
    simpa only [sameIterator] using inserted
  obtain ⟨inside, _, contents⟩ := NativeInsertionLoop.completed_exact source.iteratorInst
    program.value.atoms iterator atoms initialized.slice stored finite covered actualInsertion
  have owner : result.theory = program := by rw [resultExact]
  have packed : NativePackedStorage.raw result.words =
      PackedInterpretations.pack program.value.atoms.val (atoms.map UScalar.val) := by
    rw [resultExact]
    change NativeInsertionLoop.raw stored = _
    rw [contents]
    change PackedInterpretations.insertAll (NativePackedStorage.raw initialized)
      (atoms.map UScalar.val) = _
    rw [initialRaw]
    rfl
  exact ⟨owner, inside, packed⟩

/-- A completed constructor from an owned atom vector packs exactly that
vector's stored coordinates and retains its theory. No separate iterator trace,
ordering, distinctness or input-admission premise is needed.

The actual vector conversion and next operations supply the generic finite-input
contract; completed_pack then establishes admission and the returned words. -/
theorem completed_vector [reservation : VectorReservation]
    (program : theory.Theory) (atoms : alloc.vec.Vec Usize)
    (result : theory.Interpretation)
    (preserved : PreservesEmpty program.value.atoms)
    (completed : theory.Interpretation.new (core.iter.traits.collect.IntoIteratorVec Usize)
      program atoms = ok (.Ok result)) :
    result.theory = program ∧
      (∀ atom ∈ atoms.val, atom.val < program.value.atoms.val) ∧
      NativePackedStorage.raw result.words =
        PackedInterpretations.pack program.value.atoms.val (atoms.val.map UScalar.val) := by
  obtain ⟨iterator, converted, finite⟩ := NativeVectorInput.converted atoms
  exact completed_pack (core.iter.traits.collect.IntoIteratorVec Usize)
    program atoms iterator atoms.val converted finite result preserved completed

/-- Actual successful finite construction supplies safe native membership
storage and retains the original owner. Iterator observations and reservation
sequence preservation are explicit; neither input admission nor output contents
are assumed. This is the evaluator's representation premise. -/
theorem completed_readable [reservation : VectorReservation] {Input Iter : Type}
    (source : core.iter.traits.collect.IntoIterator Input Usize Iter)
    (program : theory.Theory) (input : Input) (iterator : Iter) (atoms : List Usize)
    (converted : source.into_iter input = ok iterator)
    (finite : NativeInsertionLoop.FiniteInput source.iteratorInst iterator atoms)
    (result : theory.Interpretation)
    (preserved : PreservesEmpty program.value.atoms)
    (completed : theory.Interpretation.new source program input = ok (.Ok result)) :
    result.theory = program ∧ NativeMembership.Represented result := by
  obtain ⟨owner, inside, packed⟩ :=
    completed_pack source program input iterator atoms converted finite result preserved completed
  have bounded : ∀ atom ∈ atoms.map UScalar.val, atom < result.theory.value.atoms.val := by
    intro atom member
    obtain ⟨sourceAtom, sourceMember, rfl⟩ := List.mem_map.mp member
    rw [owner]
    exact inside sourceAtom sourceMember
  have contents : NativePackedStorage.raw result.words =
      PackedInterpretations.pack result.theory.value.atoms.val (atoms.map UScalar.val) := by
    rw [owner]
    exact packed
  have shape : NativePackedStorage.ExactStorage result := by
    exact (packed_queries result (atoms.map UScalar.val) bounded contents).1
  exact ⟨owner, NativePackedStorage.exact_storage_readable result shape⟩

/-- The ordinary owned-vector input derives its own finite iteration. A
successful actual constructor therefore supplies safe storage, exact word count,
zero padding and the actual membership answer for every machine coordinate.
Duplicates and input order are unrestricted. -/
theorem completed_vector_queries [reservation : VectorReservation]
    (program : theory.Theory) (atoms : alloc.vec.Vec Usize) (result : theory.Interpretation)
    (preserved : PreservesEmpty program.value.atoms)
    (completed : theory.Interpretation.new (core.iter.traits.collect.IntoIteratorVec Usize)
      program atoms = ok (.Ok result)) :
    result.theory = program ∧ NativeMembership.Represented result ∧
      NativePackedStorage.ExactStorage result ∧
      PackedInterpretations.ZeroPadding result.theory.value.atoms.val
        (NativePackedStorage.raw result.words) ∧
      ∀ atom : Usize, theory.Interpretation.contains result atom =
        ok (decide (atom.val ∈ atoms.val.map UScalar.val)) := by
  obtain ⟨owner, inside, packed⟩ := completed_vector program atoms result preserved completed
  have bounded : ∀ atom ∈ atoms.val.map UScalar.val, atom < result.theory.value.atoms.val := by
    intro atom member
    obtain ⟨sourceAtom, sourceMember, rfl⟩ := List.mem_map.mp member
    rw [owner]
    exact inside sourceAtom sourceMember
  have contents : NativePackedStorage.raw result.words =
      PackedInterpretations.pack result.theory.value.atoms.val (atoms.val.map UScalar.val) := by
    rw [owner]
    exact packed
  obtain ⟨shape, padding, queries⟩ :=
    packed_queries result (atoms.val.map UScalar.val) bounded contents
  exact ⟨owner, NativePackedStorage.exact_storage_readable result shape, shape, padding, queries⟩

/-- Once the actual reservation returns its preserved empty sequence, the
owned-vector constructor terminates with its exact admission verdict. Invalid
coordinates yield Atom after their bounded prefix; valid input returns the
complete packed interpretation. No successful insertion or constructor return
is assumed by this law. Reservation refusal and nonreturn remain separate.

Proof: actual zero resize establishes storage coverage, actual vector iteration
supplies a finite trace, and the generated insertion loop gives the verdict and
partial contents. Substituting those derived phases proves the wrapper result. -/
theorem reserved_vector_exact [reservation : VectorReservation]
    (program : theory.Theory) (atoms : alloc.vec.Vec Usize) (count : Usize)
    (vacant : alloc.vec.Vec U64)
    (counted : core.num.Usize.div_ceil program.value.atoms 64#usize = ok count)
    (reserved : alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new U64) count =
      ok (.Ok (), vacant))
    (preserved : PreservesEmpty program.value.atoms) :
    ∃ stored : Slice U64,
      stored.val.length = PackedInterpretations.count64 program.value.atoms.val ∧
      NativeInsertionLoop.raw stored = PackedInterpretations.pack program.value.atoms.val
        ((NativeInsertionLoop.validPrefix program.value.atoms atoms.val).map UScalar.val) ∧
      theory.Interpretation.new (core.iter.traits.collect.IntoIteratorVec Usize) program atoms =
        ok (match NativeInsertionLoop.verdict program.value.atoms atoms.val with
          | .Err reason => .Err reason
          | .Ok _ => .Ok { theory := program, words := { slice := stored } }) := by
  have empty : vacant.val = [] := by
    exact preserved count vacant counted reserved
  obtain ⟨initialized, resized, rawZero, represented⟩ :=
    counted_zero program.value.atoms count counted vacant empty
  have length : initialized.val.length = PackedInterpretations.count64 program.value.atoms.val := by
    simpa only [NativePackedStorage.raw, List.length_map] using represented.1
  have covered : NativeInsertionLoop.Covers program.value.atoms initialized.slice := by
    intro atom inside
    change atom / 64 < initialized.val.length
    rw [length]
    unfold PackedInterpretations.count64
    omega
  obtain ⟨stored, inserted, storedLength, contents⟩ :=
    NativeInsertionLoop.insert_exact (core.iter.traits.iterator.IteratorVecIntoIter Usize)
      program.value.atoms atoms atoms.val initialized.slice (NativeVectorInput.finite atoms) covered
  refine ⟨stored, storedLength.trans length, ?_, ?_⟩
  · rw [contents]
    change PackedInterpretations.insertAll (NativePackedStorage.raw initialized) _ = _
    rw [rawZero]
    rfl
  · rw [NativeInterpretationConstruction.phases_exact, counted]
    simp only [bind_tc_ok, reserved]
    change (do
      let zeroed ← alloc.vec.Vec.resize core.clone.CloneU64 vacant count 0#u64
      let input ← (core.iter.traits.collect.IntoIteratorVec Usize).into_iter atoms
      let (verdict, words) ← theory.insert_atoms
        (core.iter.traits.iterator.IteratorVecIntoIter Usize) program.value.atoms input zeroed.slice
      match verdict with
      | .Err reason => ok (core.result.Result.Err reason :
          core.result.Result theory.Interpretation theory.AdmissionError)
      | .Ok _ => ok (core.result.Result.Ok
          ({ theory := program, words := { slice := words } } : theory.Interpretation))) = _
    rw [resized]
    simp only [bind_tc_ok, alloc.vec.IntoIteratorVec.into_iter]
    rw [inserted, bind_tc_ok]
    cases NativeInsertionLoop.verdict program.value.atoms atoms.val <;> rfl

end NativeInterpretationStorage
