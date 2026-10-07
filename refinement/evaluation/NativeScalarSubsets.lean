import NativeMembership
import ScalarSubsetWords

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract

/-!
# Native interpretation storage for subset carries

Generic scalar/vector/population laws retain their names in ScalarSubsetWords.
This interpretation-specific bridge uses the actual native extracted type;
all bit updates and population arithmetic remain shared with historical proofs.
-/
namespace NativeScalarSubsets

open Zetesis.Refinement ScalarSubsets

/-- A bounded atom of the current extracted interpretation names an existing
word. Its raw bit view is exactly the interpretation's membership predicate;
no default value is used to hide missing storage. -/
theorem interpretation_storage (candidate : theory.Interpretation) (atom : Usize)
    (represented : NativeMembership.Represented candidate)
    (inside : atom.val < candidate.theory.value.atoms.val) :
    atom.val / 64 < candidate.words.val.length ∧
      PackedInterpretations.bit64 (raw candidate.words) atom.val =
        NativeMembership.denotes candidate atom.val := by
  have stored : atom.val / 64 < candidate.words.val.length := by
    exact represented atom.val inside
  refine ⟨stored, ?_⟩
  simp [PackedInterpretations.bit64, PackedInterpretations.word, raw,
    NativeMembership.denotes, inside, List.getElem?_eq_getElem stored]

end NativeScalarSubsets
