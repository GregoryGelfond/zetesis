import NativeMembership
import Zetesis.PackedSubsets

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract
open Zetesis.Refinement

/-!
# Native packed storage and zero initialization

These representation laws use the existing packed-word semantics and native
membership predicate. Exact storage is a length property, stronger than safe
reads; no semantic population is assumed from length alone. The resize law
uses the imported sequence model. Reservation success and capacity remain
separate explicit contracts of the constructor.
-/
namespace NativePackedStorage

/-- Read every represented machine word without clipping or padding repair. -/
def raw (words : alloc.vec.Vec U64) : List (BitVec 64) := words.val.map UScalar.bv

/-- The complete word count of the interpretation's own declared atom universe. -/
def ExactStorage (candidate : theory.Interpretation) : Prop :=
  candidate.words.val.length = PackedInterpretations.count64 candidate.theory.value.atoms.val

/-- Exact storage covers every in-universe word read, independently of padding. -/
theorem exact_storage_readable (candidate : theory.Interpretation)
    (shape : ExactStorage candidate) : NativeMembership.Represented candidate := by
  intro atom inside
  change candidate.words.val.length = _ at shape
  rw [shape]
  unfold PackedInterpretations.count64
  omega

/-- Resizing an empty machine-word vector with zero yields exactly the requested
zero sequence under the imported vector model. No allocation success is inferred. -/
theorem resize_zero (vacant : alloc.vec.Vec U64) (count : Usize)
    (empty : vacant.val = []) :
    ∃ words : alloc.vec.Vec U64,
      alloc.vec.Vec.resize core.clone.CloneU64 vacant count (0#u64) = ok words ∧
      words.val = List.replicate count.val (0#u64) := by
  obtain ⟨words, resized, contents⟩ := WP.spec_imp_exists
    (alloc.vec.Vec.resize_spec core.clone.CloneU64 vacant count (0#u64) (by rfl))
  refine ⟨words, resized, ?_⟩
  simpa only [empty, List.resize, Nat.zero_le, if_true, List.take_nil,
    List.length_nil, Nat.sub_zero, List.nil_append] using contents

/-- Native packed membership is the same bounded bit reading used by the
shared mathematical packing laws, including malformed short storage defaults. -/
theorem packed_denotation (candidate : theory.Interpretation) (atom : Nat) :
    NativeMembership.denotes candidate atom =
      PackedInterpretations.contains candidate.theory.value.atoms.val
        (raw candidate.words) atom := by
  rw [PackedInterpretations.contains_eq_bit]
  unfold NativeMembership.denotes PackedInterpretations.bit64 PackedInterpretations.word raw
  simp only [List.getElem?_map]
  cases stored : candidate.words.val[atom / 64]? <;> simp

end NativePackedStorage
