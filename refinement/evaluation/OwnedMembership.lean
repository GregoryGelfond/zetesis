import PackedSetup
import OwnerChecks

open Aeneas Aeneas.Std Result
open ZetesisExtract
open Zetesis Zetesis.Refinement

/-!
# Owner-checked membership setup

The generated owner check connects the candidate's universe to the original
theory. The actual zero-resize result then supplies the initial subset's
representation. The remaining semantic phases are the generated operations
already refined to the answer-set definition.

Immutable-heap consistency and exact candidate word length remain admission
contracts. This composition does not execute the public wrapper's reservations
or model changing cancellation observations.
-/
namespace OwnedMembership

/-- Successful actual owner checking and zero initialization discharge the
universe and empty-subset premises of completed membership. Original evaluation,
root checking, atom selection and search retain the same returned work records.

Proof: a common immutable heap turns the checked owner identity into equal
theory data. Hence atom universes agree. The zero-resize theorem supplies the
empty packed interpretation, and the completed-search theorem applies the
general answer-set definition. No reservation call is modeled; existing vector
inputs and actual resize success are explicit premises.
-/
theorem completed (frozen : CountermodelSemantics.FrozenEvaluation)
    (heap : RuntimeOwnership.Heap theory.Data)
    (programValid : RuntimeOwnership.Consistent heap frozen.program)
    (candidateValid : RuntimeOwnership.Consistent heap frozen.candidate.theory)
    (owned : oracle.identities frozen.program frozen.candidate = ok (core.result.Result.Ok ()))
    (shape : PackedSetup.ExactStorage frozen.candidate) (rootAfter : oracle.Work)
    (originalModel : oracle.failed_root frozen.program frozen.values.slice frozen.after =
      ok (core.result.Result.Ok none, rootAfter))
    (destination : alloc.vec.Vec Usize) (vacant : destination.val = [])
    (selected : alloc.vec.Vec Usize) (selectionAfter : oracle.Work)
    (selectedComplete : oracle.select_atoms frozen.program frozen.candidate destination rootAfter =
      ok (core.result.Result.Ok (), selected, selectionAfter))
    (wordDestination : alloc.vec.Vec U64) (wordVacant : wordDestination.val = [])
    (count : Usize) (countExact : count.val = frozen.candidate.words.val.length)
    (words : alloc.vec.Vec U64)
    (resized : alloc.vec.Vec.resize core.clone.CloneU64 wordDestination count (0#u64) = ok words)
    (old output : alloc.vec.Vec Bool) (found : Bool)
    (subset : theory.Interpretation) (after : oracle.Work)
    (completedSearch : oracle.find_countermodel frozen.program frozen.values.slice selected.slice
      { theory := frozen.program, words := words } old selectionAfter =
      ok (core.result.Result.Ok found, subset, output, after)) :
    found = false ↔ Ferraris.Stable
      (TightEvaluation.interpretation (Membership.denotes frozen.candidate))
      (RootSemantics.assertions frozen.program) := by
  have dataEqual : frozen.program.value = frozen.candidate.theory.value :=
    OwnerChecks.accepted_data heap frozen.program frozen.candidate
      programValid candidateValid owned
  have sameUniverse : frozen.program.value.atoms = frozen.candidate.theory.value.atoms :=
    congrArg (fun data : theory.Data => data.atoms) dataEqual
  exact PackedSetup.initialized_membership frozen shape rootAfter originalModel destination
    sameUniverse vacant selected selectionAfter selectedComplete wordDestination wordVacant
    count countExact words resized old output found subset after completedSearch

end OwnedMembership
