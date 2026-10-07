import Membership
import ScalarSubsetWords

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Legacy interpretation storage for subset carries

Generic scalar/vector/population laws retain their names in ScalarSubsetWords.
Only this interpretation-specific storage bridge depends on the historical
extracted type. NativeScalarSubsets supplies the corresponding native bridge.
-/
namespace ScalarSubsets

open Zetesis.Refinement

/-- A bounded atom of the current extracted interpretation names an existing
word. Its raw bit view is exactly the interpretation's membership predicate;
no default value is used to hide missing storage. -/
theorem interpretation_storage (candidate : theory.Interpretation) (atom : Usize)
    (represented : Membership.Represented candidate)
    (inside : atom.val < candidate.theory.value.atoms.val) :
    atom.val / 64 < candidate.words.val.length ∧
      PackedInterpretations.bit64 (raw candidate.words) atom.val =
        Membership.denotes candidate atom.val := by
  have stored : atom.val / 64 < candidate.words.val.length := by
    exact represented atom.val inside
  refine ⟨stored, ?_⟩
  simp [PackedInterpretations.bit64, PackedInterpretations.word, raw,
    Membership.denotes, inside, List.getElem?_eq_getElem stored]

end ScalarSubsets
