import NativePublicFrozenQuery
import NativeTheoryConstruction
import NativeAdmissionConsequences
import NativeInterpretationStorage

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract

/-!
# From native admission to frozen query semantics and dimensions

The actual theory constructor establishes the structural premises of native
execution under the explicit Arc successful-value contract. Its independently
recomputed occurrence metadata agrees with the decoded rows' complete work.
No premise asserts cached metadata correctness or evaluator agreement.
-/
namespace NativeAdmittedQuery

/-- Successful native decoding preserves each row's complete logical width. -/
theorem view_occurrences (arena : Slice Usize) (raw : theory.Node) (view : theory.NodeView)
    (decoded : NativeRows.storedView arena raw = .Ok view) :
    NativeEvaluation.occurrences view = (NativeOccurrenceCount.width raw).val := by
  have width : (NativeAdmissionValidation.children view).length = (NativeOccurrenceCount.width raw).val := by
    exact NativeAdmissionConsequences.decoded_width arena raw view decoded
  cases view <;> simpa only [NativeEvaluation.occurrences, NativeAdmissionValidation.children,
    List.length_cons, List.length_nil] using width

/-- Actual well-formed paired storage makes the decoded work sum equal to the
raw occurrence count, preserving repeated children and overlapping raw spans. -/
theorem table_occurrences (atoms : Nat) (view : theory.FormulaView)
    (valid : NativeStructure.WellFormed atoms view) :
    ((NativeTable.rows view).map NativeEvaluation.occurrences).sum =
      NativeOccurrenceCount.occurrences view.nodes.val := by
  have entries : (NativeTable.rows view).map NativeEvaluation.occurrences =
      view.nodes.val.map (fun raw => (NativeOccurrenceCount.width raw).val) := by
    simp only [NativeTable.rows, List.map_map]
    apply List.map_congr_left
    intro raw member
    obtain ⟨index, inside, same⟩ := List.mem_iff_getElem.mp member
    obtain ⟨decoded, accepted, _, _⟩ := valid index inside
    rw [← same]
    simp only [Function.comp_def, NativeTable.row, accepted]
    exact view_occurrences view.operands view.nodes.val[index] decoded accepted
  have rawSum (nodes : List theory.Node) :
      (nodes.map (fun raw => (NativeOccurrenceCount.width raw).val)).sum =
        NativeOccurrenceCount.occurrences nodes := by
    induction nodes with
    | nil => rfl
    | cons raw rest inductionHypothesis =>
      simp only [List.map_cons, List.sum_cons, NativeOccurrenceCount.occurrences, inductionHypothesis]
  rw [entries]
  exact rawSum view.nodes.val

/-- Actual construction supplies native topology and root coverage, and its
checked occurrence metadata names exactly the evaluator's N+E work dimension.
Only the explicitly supplied Arc library value-preservation contract is assumed. -/
theorem returned_dimensions [allocation : ArcAllocation]
    (atoms : Usize) (parts : theory.FormulaParts) (roots : alloc.vec.Vec Usize)
    (limits : theory.AdmissionLimits) (program : theory.Theory)
    (preservesValue : ∀ (data : theory.Data) (view : theory.Theory),
      alloc.sync.Arc.new data = ok view → view.value = data)
    (completed : theory.Theory.new atoms parts roots limits = ok (.Ok program)) :
    NativeStructure.WellFormed program.value.atoms.val (NativeExecution.view program) ∧
      (∀ root ∈ program.value.roots.val,
        root.val < (NativeTable.rows (NativeExecution.view program)).length) ∧
      NativeSpecification.cost (NativeTable.rows (NativeExecution.view program)) =
        program.value.parts.nodes.val.length + program.value.parts.occurrences.val := by
  obtain ⟨valid, rootsBounded, recounted, _, _⟩ :=
    NativeTheoryConstruction.returned_structure atoms parts roots limits program preservesValue completed
  have readable : NativeStructure.WellFormed program.value.atoms.val (NativeExecution.view program) := by
    exact valid
  refine ⟨readable, ?_, ?_⟩
  · intro root member
    simpa [NativeTable.length, NativeExecution.view, alloc.vec.Vec.deref] using rootsBounded root member
  · rw [NativeSpecification.cost_dimensions, table_occurrences _ _ readable, NativeTable.length, recounted]
    simp [NativeExecution.view, alloc.vec.Vec.deref]

/-- Admission and public frozen construction/query jointly establish the exact
Ferraris verdict. Representation of candidate and tested packed words is supplied
by their separate constructor boundary, never an assumed semantic verdict.
The tested interpretation need not be a subset of the candidate. -/
theorem constructed_satisfaction [allocation : ArcAllocation]
    (construction query : VectorReservation)
    (atoms : Usize) (parts : theory.FormulaParts) (roots : alloc.vec.Vec Usize)
    (admission : theory.AdmissionLimits) (program : theory.Theory)
    (preservesValue : ∀ (data : theory.Data) (view : theory.Theory),
      alloc.sync.Arc.new data = ok view → view.value = data)
    (admitted : theory.Theory.new atoms parts roots admission = ok (.Ok program))
    (candidate tested : theory.Interpretation) (candidateTheory : candidate.theory = program)
    (candidateStored : NativeMembership.Represented candidate)
    (testedStored : NativeMembership.Represented tested)
    (constructionLimits queryLimits : oracle.Limits)
    (constructionControl queryControl : zetesis_cpu.cancellation.Cancellation)
    (frozen : reduct.FrozenReduct)
    (constructed : @reduct.FrozenReduct.new construction candidate constructionLimits constructionControl =
      ok (.Ok frozen))
    (answer : Bool)
    (queried : @reduct.FrozenReduct.is_satisfied_by query frozen tested queryLimits queryControl = ok (.Ok answer)) :
    answer = true ↔ Zetesis.Ferraris.Models
      (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes tested))
      (Zetesis.Ferraris.ReductTheory
        (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes candidate))
        (NativeRootSemantics.assertions program)) := by
  obtain ⟨valid, rootsBounded, _⟩ := returned_dimensions atoms parts roots admission program preservesValue admitted
  have candidateValid : NativeStructure.WellFormed candidate.theory.value.atoms.val
      (NativeExecution.view candidate.theory) := by
    simpa only [candidateTheory] using valid
  have candidateRoots : ∀ root ∈ candidate.theory.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view candidate.theory)).length := by
    simpa only [candidateTheory] using rootsBounded
  have verdict : answer = true ↔ Zetesis.Ferraris.Models
      (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes tested))
      (Zetesis.Ferraris.ReductTheory
        (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes candidate))
        (NativeRootSemantics.assertions candidate.theory)) := by
    exact NativePublicFrozenQuery.constructed_satisfaction (reservation := query) construction
      candidate tested constructionLimits queryLimits constructionControl queryControl candidateStored testedStored
      candidateValid candidateRoots frozen constructed answer queried
  simpa only [candidateTheory] using verdict

/-- Starting with actual theory and owned-vector interpretation construction,
the public frozen query decides Ferraris reduct satisfaction. The constructors
supply topology, root coverage, owners and readable packed words; none is an
additional caller premise. Candidate modelhood and a tested-subset relation are
not required. Reservation and Arc value preservation remain library contracts,
with separate providers for each invocation and no assumed allocation success.

Proof: recover each interpretation's retained theory and packed representation
from its constructor. Apply the admission-to-query law with those derived facts.
The returned Boolean therefore has the meaning of the original public query. -/
theorem constructed_vector_satisfaction [allocation : ArcAllocation]
    (candidateStorage testedStorage construction query : VectorReservation)
    (atoms : Usize) (parts : theory.FormulaParts) (roots : alloc.vec.Vec Usize)
    (admission : theory.AdmissionLimits) (program : theory.Theory)
    (preservesValue : ∀ (data : theory.Data) (view : theory.Theory),
      alloc.sync.Arc.new data = ok view → view.value = data)
    (admitted : theory.Theory.new atoms parts roots admission = ok (.Ok program))
    (candidateInput testedInput : alloc.vec.Vec Usize)
    (candidate tested : theory.Interpretation)
    (candidatePreserved : @NativeInterpretationStorage.PreservesEmpty
      candidateStorage program.value.atoms)
    (testedPreserved : @NativeInterpretationStorage.PreservesEmpty
      testedStorage program.value.atoms)
    (candidateConstructed : @theory.Interpretation.new candidateStorage _ _
      (core.iter.traits.collect.IntoIteratorVec Usize) program candidateInput = ok (.Ok candidate))
    (testedConstructed : @theory.Interpretation.new testedStorage _ _
      (core.iter.traits.collect.IntoIteratorVec Usize) program testedInput = ok (.Ok tested))
    (constructionLimits queryLimits : oracle.Limits)
    (constructionControl queryControl : zetesis_cpu.cancellation.Cancellation)
    (frozen : reduct.FrozenReduct)
    (constructed : @reduct.FrozenReduct.new construction candidate constructionLimits constructionControl =
      ok (.Ok frozen))
    (answer : Bool)
    (queried : @reduct.FrozenReduct.is_satisfied_by query frozen tested queryLimits queryControl = ok (.Ok answer)) :
    answer = true ↔ Zetesis.Ferraris.Models
      (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes tested))
      (Zetesis.Ferraris.ReductTheory
        (Zetesis.TightEvaluation.interpretation (NativeMembership.denotes candidate))
        (NativeRootSemantics.assertions program)) := by
  obtain ⟨candidateTheory, candidateStored, _⟩ :=
    NativeInterpretationStorage.completed_vector_queries (reservation := candidateStorage)
      program candidateInput candidate candidatePreserved candidateConstructed
  obtain ⟨_, testedStored, _⟩ :=
    NativeInterpretationStorage.completed_vector_queries (reservation := testedStorage)
      program testedInput tested testedPreserved testedConstructed
  exact constructed_satisfaction construction query atoms parts roots admission program
    preservesValue admitted candidate tested candidateTheory candidateStored testedStored
    constructionLimits queryLimits constructionControl queryControl frozen constructed answer queried

end NativeAdmittedQuery
