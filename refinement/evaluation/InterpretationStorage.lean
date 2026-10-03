import InterpretationConstruction
import PackedSetup
import VectorInput

open Aeneas Aeneas.Std Result
open ZetesisExtract
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
namespace InterpretationStorage

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
      ScalarSubsets.raw words = PackedInterpretations.pack atoms.val [] ∧
      PackedSubsets.Represents (ScalarSubsets.raw words) ([] : List (Fin atoms.val)) := by
  obtain ⟨actualCount, actualCounted, countValue⟩ := UsizeCeiling.word_count64 atoms
  have sameCount : actualCount = count :=
    Result.ok_injective (actualCounted.symm.trans counted)
  have countExact : count.val = PackedInterpretations.count64 atoms.val := by
    simpa only [sameCount] using countValue
  obtain ⟨words, resized, contents⟩ := PackedSetup.resize_zero vacant count empty
  have rawZero : ScalarSubsets.raw words = PackedInterpretations.pack atoms.val [] := by
    simp only [ScalarSubsets.raw, contents, List.map_replicate, countExact,
      PackedInterpretations.pack, PackedInterpretations.insertAll]
    rfl
  have represented : PackedSubsets.Represents (ScalarSubsets.raw words)
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
      ScalarSubsets.raw initialized = PackedInterpretations.pack program.value.atoms.val [] ∧
      PackedSubsets.Represents (ScalarSubsets.raw initialized)
        ([] : List (Fin program.value.atoms.val)) ∧
      theory.insert_atoms source.iteratorInst program.value.atoms iterator initialized.slice =
        ok (.Ok (), stored) ∧
      result = { theory := program, words := { slice := stored } } := by
  obtain ⟨count, vacant, initialized, iterator, stored, counted, _, reserved,
    resized, converted, inserted, resultExact⟩ :=
    InterpretationConstruction.completed_phases source program atoms result completed
  have empty : vacant.val = [] := preserved count vacant counted reserved
  obtain ⟨zeroWords, zeroResized, rawZero, represented⟩ :=
    counted_zero program.value.atoms count counted vacant empty
  have sameWords : zeroWords = initialized :=
    Result.ok_injective (zeroResized.symm.trans resized)
  have initialRaw : ScalarSubsets.raw initialized =
      PackedInterpretations.pack program.value.atoms.val [] := by
    simpa only [sameWords] using rawZero
  have initialRepresents : PackedSubsets.Represents (ScalarSubsets.raw initialized)
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
    (packed : ScalarSubsets.raw result.words =
      PackedInterpretations.pack result.theory.value.atoms.val atoms) :
    PackedSetup.ExactStorage result ∧
      PackedInterpretations.ZeroPadding result.theory.value.atoms.val
        (ScalarSubsets.raw result.words) ∧
      ∀ atom : Usize, theory.Interpretation.contains result atom =
        ok (decide (atom.val ∈ atoms)) := by
  obtain ⟨length, membership, padding⟩ :=
    PackedInterpretations.pack_exact result.theory.value.atoms.val atoms inside
  have shape : PackedSetup.ExactStorage result := by
    have rawLength := congrArg List.length packed
    simpa only [PackedSetup.ExactStorage, ScalarSubsets.raw, List.length_map, length]
      using rawLength
  have represented : Membership.Represented result :=
    PackedSetup.exact_storage_readable result shape
  have zeroPadding : PackedInterpretations.ZeroPadding result.theory.value.atoms.val
      (ScalarSubsets.raw result.words) := by
    rw [packed]
    exact padding
  have queried (atom : Usize) : theory.Interpretation.contains result atom =
      ok (decide (atom.val ∈ atoms)) := by
    rw [Membership.contains_refines result atom represented, SelectedAtoms.packed_denotation]
    change ok (PackedInterpretations.contains result.theory.value.atoms.val
      (ScalarSubsets.raw result.words) atom.val) = _
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
    (finite : InsertionLoop.FiniteInput source.iteratorInst iterator atoms)
    (result : theory.Interpretation)
    (preserved : PreservesEmpty program.value.atoms)
    (completed : theory.Interpretation.new source program input = ok (.Ok result)) :
    result.theory = program ∧
      (∀ atom ∈ atoms, atom.val < program.value.atoms.val) ∧
      ScalarSubsets.raw result.words =
        PackedInterpretations.pack program.value.atoms.val (atoms.map UScalar.val) := by
  obtain ⟨initialized, actualIterator, stored, actuallyConverted, initialRaw,
    initialRepresents, inserted, resultExact⟩ :=
    completed_initialization source program input result preserved completed
  have sameIterator : actualIterator = iterator :=
    Result.ok_injective (actuallyConverted.symm.trans converted)
  have covered : InsertionLoop.Covers program.value.atoms initialized.slice := by
    intro atom inside
    have length : initialized.val.length =
        PackedInterpretations.count64 program.value.atoms.val := by
      simpa only [ScalarSubsets.raw, List.length_map] using initialRepresents.1
    change atom / 64 < initialized.val.length
    rw [length]
    unfold PackedInterpretations.count64
    omega
  have actualInsertion : theory.insert_atoms source.iteratorInst program.value.atoms
      iterator initialized.slice = ok (.Ok (), stored) := by
    simpa only [sameIterator] using inserted
  obtain ⟨inside, _, contents⟩ := InsertionLoop.completed_exact source.iteratorInst
    program.value.atoms iterator atoms initialized.slice stored finite covered actualInsertion
  have owner : result.theory = program := by rw [resultExact]
  have packed : ScalarSubsets.raw result.words =
      PackedInterpretations.pack program.value.atoms.val (atoms.map UScalar.val) := by
    rw [resultExact]
    change InsertionLoop.raw stored = _
    rw [contents]
    change PackedInterpretations.insertAll (ScalarSubsets.raw initialized)
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
      ScalarSubsets.raw result.words =
        PackedInterpretations.pack program.value.atoms.val (atoms.val.map UScalar.val) := by
  obtain ⟨iterator, converted, finite⟩ := VectorInput.converted atoms
  exact completed_pack (core.iter.traits.collect.IntoIteratorVec Usize)
    program atoms iterator atoms.val converted finite result preserved completed

end InterpretationStorage
