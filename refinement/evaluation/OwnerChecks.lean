import Evaluator.Funs
import RuntimeOwnership

/-!
# Extracted theory-owner checks

The actual generated identity check accepts precisely matching allocation
identities. Common immutable-heap consistency then supplies equal theory data,
including atoms, nodes and roots. The actual Theory clone retains that identity.

The external Arc operations use the explicit owner model. This is not an
allocator, reference-count or concurrent-memory proof. The public checker and
its reservation/setup operations remain separate from these extracted calls.
-/

namespace OwnerChecks

open Aeneas Aeneas.Std Result
open ZetesisExtract RuntimeOwnership

/-- The actual identity check accepts exactly a matching theory owner.
It reads no candidate word and evaluates no formula. -/
theorem identities_accept_iff (program : theory.Theory)
    (candidate : theory.Interpretation) :
    oracle.identities program candidate = ok (core.result.Result.Ok ()) ↔
      program.owner = candidate.theory.owner := by
  simp [oracle.identities, theory.Interpretation.impl.theory,
    theory.Theory.same_instance, alloc.sync.Arc.ptr_eq]

/-- A distinct owner produces the actual WrongProgram refusal, independently
of whether the stored theories happen to have equal values. -/
theorem identities_reject (program : theory.Theory) (candidate : theory.Interpretation)
    (different : program.owner ≠ candidate.theory.owner) :
    oracle.identities program candidate =
      ok (core.result.Result.Err zetesis_cpu.cancellation.Stop.WrongProgram) := by
  simp [oracle.identities, theory.Interpretation.impl.theory,
    theory.Theory.same_instance, alloc.sync.Arc.ptr_eq, different]

/-- A successful actual identity check transports immutable theory data under
one common-heap consistency contract, not merely numeric atom-count agreement. -/
theorem accepted_data (heap : Heap theory.Data) (program : theory.Theory)
    (candidate : theory.Interpretation)
    (programValid : Consistent heap program)
    (candidateValid : Consistent heap candidate.theory)
    (accepted : oracle.identities program candidate = ok (core.result.Result.Ok ())) :
    program.value = candidate.theory.value := by
  have same : program.owner = candidate.theory.owner :=
    (identities_accept_iff program candidate).mp accepted
  exact same_owner_value heap program candidate.theory programValid candidateValid same

/-- The complete stored vocabulary and formula table agree after an accepted
actual owner check with consistent inputs. These are equal fields, not a claim
about independently allocated but equal-valued theories. -/
theorem accepted_fields (heap : Heap theory.Data) (program : theory.Theory)
    (candidate : theory.Interpretation)
    (programValid : Consistent heap program)
    (candidateValid : Consistent heap candidate.theory)
    (accepted : oracle.identities program candidate = ok (core.result.Result.Ok ())) :
    program.value.atoms = candidate.theory.value.atoms ∧
      program.value.nodes = candidate.theory.value.nodes ∧
      program.value.roots = candidate.theory.value.roots := by
  have same : program.value = candidate.theory.value :=
    accepted_data heap program candidate programValid candidateValid accepted
  exact ⟨congrArg theory.Data.atoms same,
    congrArg theory.Data.nodes same, congrArg theory.Data.roots same⟩

/-- The actual generated Theory clone returns the same owner and immutable data
under the selected global-allocator external contract. -/
theorem theory_clone_exact (program : theory.Theory) :
    theory.Theory.Insts.CoreCloneClone.clone program = ok program := by
  simp [theory.Theory.Insts.CoreCloneClone.clone,
    alloc.sync.Arc.Insts.CoreCloneClone.clone]

/-- A completed actual Theory clone preserves heap consistency and owner identity.
The clone result, rather than an assumed owner equality, determines the copy. -/
theorem theory_clone_consistent (heap : Heap theory.Data) (program copied : theory.Theory)
    (valid : Consistent heap program)
    (returned : theory.Theory.Insts.CoreCloneClone.clone program = ok copied) :
    copied.owner = program.owner ∧ copied.value = program.value ∧ Consistent heap copied := by
  have same : program = copied := by
    simpa only [theory_clone_exact, Result.ok.injEq] using returned
  subst copied
  exact ⟨rfl, rfl, valid⟩

end OwnerChecks
