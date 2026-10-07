import NativeAdmittedData
import ArcAllocation

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract

/-!
# Native shared-theory construction

The generated constructor invokes admission before the supplied Arc operation.
A successful return supplies actual successful phase equations; none is assumed.
Value preservation and any fresh heap transition remain explicit library
contracts for that invocation. The provider may fail or diverge. A single pure
provider does not model repeated fresh allocations or reference-count effects.
-/
namespace NativeTheoryConstruction

variable [allocation : ArcAllocation]

/-- A typed admission refusal is returned before any allocation operation. -/
theorem refused (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits)
    (reason : theory.AdmissionError)
    (admitted : theory.admit atoms parts roots limits = ok (.Err reason)) :
    theory.Theory.new atoms parts roots limits = ok (.Err reason) := by
  simp [theory.Theory.new, admitted, core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- On admission success the actual wrapper preserves the supplied allocator's
outer result, including failure and divergence, rather than fabricating success. -/
theorem admitted_allocation (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits) (data : theory.Data)
    (admitted : theory.admit atoms parts roots limits = ok (.Ok data)) :
    theory.Theory.new atoms parts roots limits =
      (do let program ← alloc.sync.Arc.new data
          ok (core.result.Result.Ok program)) := by
  simp [theory.Theory.new, admitted, core.result.Result.Insts.CoreOpsTry.branch]

/-- Actual constructor success entails completed admission and allocation of
one and the same admitted data. The stored cache is derived by admission. -/
theorem completed_phases (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits)
    (program : theory.Theory)
    (completed : theory.Theory.new atoms parts roots limits = ok (.Ok program)) :
    ∃ data : theory.Data,
      theory.admit atoms parts roots limits = ok (.Ok data) ∧
      alloc.sync.Arc.new data = ok program := by
  cases validated : NativeAdmittedData.validate atoms parts roots limits with
  | Err reason =>
    have rejected : theory.admit atoms parts roots limits = ok (.Err reason) := by
      rw [NativeAdmittedData.admit_exact, validated]
    rw [refused atoms parts roots limits reason rejected] at completed
    simp at completed
  | Ok data =>
    have admitted : theory.admit atoms parts roots limits = ok (.Ok data) := by
      rw [NativeAdmittedData.admit_exact, validated]
    rw [admitted_allocation atoms parts roots limits data admitted] at completed
    have allocated : alloc.sync.Arc.new data = ok program := by
      cases outcome : alloc.sync.Arc.new data with
      | ret value =>
        have same : value = program := by
          simpa only [outcome, bind_tc_ok, Result.ok.injEq,
            core.result.Result.Ok.injEq] using completed
        simp only [same]
      | vis effect continuation =>
        simp only [outcome, bind_tc_vis, vis_not_ok] at completed
      | div => simp only [outcome, bind_tc_div, div_not_ok] at completed
    exact ⟨data, admitted, allocated⟩

/-- Typed constructor refusals arise exactly from admission. The external Arc
operation's backend failures are not recast as typed admission errors. -/
theorem refused_iff (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits)
    (reason : theory.AdmissionError) :
    theory.Theory.new atoms parts roots limits = ok (.Err reason) ↔
      theory.admit atoms parts roots limits = ok (.Err reason) := by
  constructor
  · intro completed
    cases validated : NativeAdmittedData.validate atoms parts roots limits with
    | Err authored =>
      have rejected : theory.admit atoms parts roots limits = ok (.Err authored) := by
        rw [NativeAdmittedData.admit_exact, validated]
      have stopped : theory.Theory.new atoms parts roots limits = ok (.Err authored) :=
        refused atoms parts roots limits authored rejected
      have same : authored = reason := by
        simpa only [Result.ok.injEq, core.result.Result.Err.injEq] using
          stopped.symm.trans completed
      simpa only [same] using rejected
    | Ok data =>
      have admitted : theory.admit atoms parts roots limits = ok (.Ok data) := by
        rw [NativeAdmittedData.admit_exact, validated]
      rw [admitted_allocation atoms parts roots limits data admitted] at completed
      cases outcome : alloc.sync.Arc.new data with
      | ret value => simp only [outcome, bind_tc_ok, Result.ok.injEq, reduceCtorEq] at completed
      | vis effect continuation => simp only [outcome, bind_tc_vis, vis_not_ok] at completed
      | div => simp only [outcome, bind_tc_div, div_not_ok] at completed
  · exact refused atoms parts roots limits reason

/-- Under the explicit successful-value contract, the returned owner stores
actual admitted data, including the recomputed count. No allocation success or
admission premise is supplied by the caller. -/
theorem returned_value (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits)
    (program : theory.Theory)
    (preservesValue : ∀ (data : theory.Data) (view : theory.Theory),
      alloc.sync.Arc.new data = ok view → view.value = data)
    (completed : theory.Theory.new atoms parts roots limits = ok (.Ok program)) :
    NativeAdmittedData.Accepted atoms parts roots limits program.value := by
  obtain ⟨data, admitted, allocated⟩ := completed_phases atoms parts roots limits program completed
  rw [preservesValue data program allocated]
  exact (NativeAdmittedData.admit_accepts_iff atoms parts roots limits data).mp admitted

/-- The same contract gives complete native topology, atom bounds, roots and
both operand resource invariants to subsequent evaluator and query consumers. -/
theorem returned_structure (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits)
    (program : theory.Theory)
    (preservesValue : ∀ (data : theory.Data) (view : theory.Theory),
      alloc.sync.Arc.new data = ok view → view.value = data)
    (completed : theory.Theory.new atoms parts roots limits = ok (.Ok program)) :
    NativeStructure.WellFormed program.value.atoms.val (NativeAdmittedData.view program.value.parts) ∧
      (∀ root ∈ program.value.roots.val, root.val < program.value.parts.nodes.val.length) ∧
      program.value.parts.occurrences.val = NativeOccurrenceCount.occurrences program.value.parts.nodes.val ∧
      program.value.parts.operands.val.length ≤ limits.max_operands.val ∧
      program.value.parts.occurrences.val ≤ limits.max_operands.val := by
  obtain ⟨data, admitted, allocated⟩ := completed_phases atoms parts roots limits program completed
  rw [preservesValue data program allocated]
  exact NativeAdmittedData.admitted_structure atoms parts roots limits data admitted

/-- An explicit immutable heap transition establishes consistency of the new
owner and preserves every previously consistent view under a different owner.
Freshness is a premise about this one invocation, never a provider axiom. -/
theorem returned_heap (atoms : Usize) (parts : theory.FormulaParts)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits)
    (program : theory.Theory)
    (preservesValue : ∀ (data : theory.Data) (view : theory.Theory),
      alloc.sync.Arc.new data = ok view → view.value = data)
    (completed : theory.Theory.new atoms parts roots limits = ok (.Ok program))
    (before after : Nat → Option theory.Data)
    (fresh : before program.owner = none)
    (stored : ∀ data : theory.Data,
      theory.admit atoms parts roots limits = ok (.Ok data) →
      alloc.sync.Arc.new data = ok program → after program.owner = some data)
    (otherEntries : ∀ owner, owner ≠ program.owner → after owner = before owner) :
    after program.owner = some program.value ∧
      ∀ previous : theory.Theory, before previous.owner = some previous.value →
        previous.owner ≠ program.owner ∧ after previous.owner = some previous.value := by
  obtain ⟨data, admitted, allocated⟩ := completed_phases atoms parts roots limits program completed
  have value : program.value = data := preservesValue data program allocated
  constructor
  · rw [value]; exact stored data admitted allocated
  · intro previous consistent
    have distinct : previous.owner ≠ program.owner := by
      intro same
      have impossible : (none : Option theory.Data) = some previous.value := by
        calc
          none = before program.owner := fresh.symm
          _ = before previous.owner := congrArg before same.symm
          _ = some previous.value := consistent
      cases impossible
    exact ⟨distinct, by rw [otherEntries previous.owner distinct]; exact consistent⟩

end NativeTheoryConstruction
