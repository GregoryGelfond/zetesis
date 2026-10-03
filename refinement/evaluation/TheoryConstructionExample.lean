import TheoryConstruction

open Aeneas Aeneas.Std Result
open ZetesisExtract
open Zetesis.Refinement

/-!
# Allocation boundaries of actual theory construction

These examples supply proof-only operations explicitly, with no global instance.
They establish behavior under those operations, not allocator implementations.
The final example describes two separately interpreted invocations; it does not
claim that one pure provider produces fresh owners on repeated equal inputs.
-/
namespace TheoryConstructionExample

/-- A supplied operation that never returns. -/
abbrev diverging : ArcAllocation := { allocate := fun {_} _ => div }

/-- A supplied operation returning its input at the specified identity. -/
abbrev atOwner (owner : Nat) : ArcAllocation :=
  { allocate := fun {_} value => ok { value, owner } }

/-- Empty input with an empty atom universe. -/
def emptyData : theory.Data :=
  { atoms := 0#usize, nodes := alloc.vec.Vec.new theory.Node, roots := alloc.vec.Vec.new Usize }

/-- No dimension may exceed zero. The padded count still fits the host. -/
def zeroLimits : theory.AdmissionLimits :=
  { max_atoms := 0#usize, max_nodes := 0#usize, max_roots := 0#usize }

/-- An oversized atom universe is refused even when allocation would diverge. -/
theorem refused_input_does_not_allocate :
    @theory.Theory.new diverging 1#usize emptyData.nodes emptyData.roots zeroLimits =
      ok (.Err .Limit) := by
  apply TheoryConstruction.refused (allocation := diverging)
  simp [theory.admit, zeroLimits]

/-- Admitted empty input does not fabricate success when allocation diverges. -/
theorem admitted_input_can_diverge :
    @theory.Theory.new diverging emptyData.atoms emptyData.nodes emptyData.roots zeroLimits =
      div := by
  have admitted : theory.admit emptyData.atoms emptyData.nodes emptyData.roots zeroLimits =
      ok (.Ok emptyData) := by
    have padded : ¬ Usize.max < 63 := by scalar_tac
    rw [AdmittedData.admit_exact]
    simp [padded, emptyData, zeroLimits, AdmittedData.limitsOf, TheoryAdmission.validate,
      TheoryAdmission.scan, alloc.vec.Vec.new, Except.bind]
  rw [TheoryConstruction.admitted_allocation (allocation := diverging) _ _ _ _ emptyData admitted]
  simp [alloc.sync.Arc.new, ArcAllocation.allocate]

/-- Equal admitted inputs can return different owners when their invocations
supply different operations. Both store the same data, and pointer comparison
still distinguishes their supplied identities. -/
theorem separate_invocations_can_return_distinct_owners :
    @theory.Theory.new (atOwner 7) emptyData.atoms emptyData.nodes emptyData.roots zeroLimits =
      ok (.Ok { value := emptyData, owner := 7 }) ∧
    @theory.Theory.new (atOwner 9) emptyData.atoms emptyData.nodes emptyData.roots zeroLimits =
      ok (.Ok { value := emptyData, owner := 9 }) ∧
    alloc.sync.Arc.ptr_eq Unit ({ value := emptyData, owner := 7 } : theory.Theory)
      { value := emptyData, owner := 9 } = ok false := by
  have admitted : theory.admit emptyData.atoms emptyData.nodes emptyData.roots zeroLimits =
      ok (.Ok emptyData) := by
    have padded : ¬ Usize.max < 63 := by scalar_tac
    rw [AdmittedData.admit_exact]
    simp [padded, emptyData, zeroLimits, AdmittedData.limitsOf, TheoryAdmission.validate,
      TheoryAdmission.scan, alloc.vec.Vec.new, Except.bind]
  refine ⟨?_, ?_, ?_⟩
  · rw [TheoryConstruction.admitted_allocation (allocation := atOwner 7) _ _ _ _ emptyData admitted]
    simp [alloc.sync.Arc.new, ArcAllocation.allocate]
  · rw [TheoryConstruction.admitted_allocation (allocation := atOwner 9) _ _ _ _ emptyData admitted]
    simp [alloc.sync.Arc.new, ArcAllocation.allocate]
  · exact RuntimeOwnership.equal_values_distinct_owners Unit emptyData 7 9 (by decide)

end TheoryConstructionExample
