import NativePublicMembership
import NativeCheckedResults
import NativeTheoryConstruction
import NativeInterpretationStorage

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract
open Zetesis Zetesis.Refinement

/-!
# Membership of a constructed native subject

Actual successful theory and owned-vector interpretation construction establish
ordered nodes, bounded roots, exact packed storage and retention of the theory.
The actual reference check against that same retained theory can therefore use
the completed membership theorem without separate structural or heap premises.

The singleton heap below is a logical consistency witness for two occurrences
of the identical immutable Arc record. It makes no claim about a physical heap,
allocation freshness or unrelated views. Existing successful-value and
successful-empty-reservation contracts remain explicit, as do completion of the
actual operations. Fixed mathematical observations are retained. Reusing a
reservation contract at equal logical arguments makes no assertion of identical
physical allocation or of an allocator history.

The exact generated subject-bound wrapper supplies its original candidate and
native check result. It cannot turn a source stop into a completed decision.
The constructor composition connects that retained subject to the reusable ASP
answer-set definition.
-/
namespace NativeSubjectMembership

/-- Successful actual construction supplies the structural and storage premises
needed by membership checking, and retains the exact supplied theory. The owned
atom vector needs no assumed admission, distinctness or iterator trace.

Proof: actual theory construction supplies node order and root bounds. Actual
interpretation construction supplies the packed input and admitted atom bounds;
the shared packing law then gives exact storage. -/
theorem constructed_input [allocation : ArcAllocation] [reservation : VectorReservation]
    (atomCount : Usize) (parts : theory.FormulaParts) (roots : alloc.vec.Vec Usize)
    (admission : theory.AdmissionLimits) (program : theory.Theory)
    (atoms : alloc.vec.Vec Usize) (candidate : theory.Interpretation)
    (preservesValue : ∀ (data : theory.Data) (view : theory.Theory),
      alloc.sync.Arc.new data = ok view → view.value = data)
    (preservesEmpty : NativeInterpretationStorage.PreservesEmpty program.value.atoms)
    (theoryCompleted : theory.Theory.new atomCount parts roots admission = ok (.Ok program))
    (candidateCompleted : theory.Interpretation.new
      (core.iter.traits.collect.IntoIteratorVec Usize) program atoms = ok (.Ok candidate)) :
    candidate.theory = program ∧ NativePackedStorage.ExactStorage candidate ∧
      NativeStructure.WellFormed program.value.atoms.val (NativeExecution.view program) ∧
      ∀ root ∈ program.value.roots.val, root.val < (NativeTable.rows (NativeExecution.view program)).length := by
  obtain ⟨valid, rootsBounded, _, _, _⟩ :=
    NativeTheoryConstruction.returned_structure atomCount parts roots admission program
      preservesValue theoryCompleted
  obtain ⟨owner, _, shape, _, _⟩ := NativeInterpretationStorage.completed_vector_queries
    program atoms candidate preservesEmpty candidateCompleted
  refine ⟨owner, shape, valid, ?_⟩
  intro root member
  simpa only [NativeTable.length, NativeExecution.view, alloc.vec.Vec.deref,
    Slice.from_val, alloc.vec.Vec.val] using rootsBounded root member

/-- A completed actual check of a constructed candidate against its own retained
theory returns Stable exactly for an answer set. Structure, storage and common
ownership are derived; they are not additional caller hypotheses.

The successful Arc-value and empty-reservation contracts do not promise any
operation completes. The three actual completed calls are explicit premises.
One supplied reservation operation is used throughout. The constructor's word
reservation contract also covers the checker's same-sized zero subset; only
preservation of the separate selected-atom buffer is supplied additionally.

Proof: construction establishes the required input invariants. A singleton table
witnesses consistency of the identical theory argument and candidate owner.
The completed public-check theorem then gives the semantic equivalence. -/
theorem completed_constructed_check
    [allocation : ArcAllocation] [reservation : VectorReservation]
    (atomCount : Usize) (parts : theory.FormulaParts) (roots : alloc.vec.Vec Usize)
    (admission : theory.AdmissionLimits) (program : theory.Theory)
    (atoms : alloc.vec.Vec Usize) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation)
    (checked : oracle.Check)
    (preservesValue : ∀ (data : theory.Data) (view : theory.Theory),
      alloc.sync.Arc.new data = ok view → view.value = data)
    (preservesEmpty : NativeInterpretationStorage.PreservesEmpty program.value.atoms)
    (selectionPreserved : ∀ returned : alloc.vec.Vec Usize,
      alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new Usize) program.value.atoms =
        ok (.Ok (), returned) → returned.val = [])
    (theoryCompleted : theory.Theory.new atomCount parts roots admission = ok (.Ok program))
    (candidateCompleted : theory.Interpretation.new
      (core.iter.traits.collect.IntoIteratorVec Usize) program atoms = ok (.Ok candidate))
    (checkCompleted : oracle.check candidate.theory candidate limits control = ok (.Ok checked)) :
    checked.verdict = .Stable ↔ Ferraris.Stable
      (TightEvaluation.interpretation (NativeMembership.denotes candidate))
      (NativeRootSemantics.assertions program) := by
  obtain ⟨owner, shape, ordered, boundedRoots⟩ := constructed_input atomCount parts roots
    admission program atoms candidate preservesValue preservesEmpty theoryCompleted candidateCompleted
  let heap : RuntimeOwnership.Heap theory.Data := fun identity =>
    if identity = candidate.theory.owner then some candidate.theory.value else none
  have consistent : RuntimeOwnership.Consistent heap candidate.theory := by
    simp only [RuntimeOwnership.Consistent, heap, ite_true]
  have ownOrder : NativeStructure.WellFormed candidate.theory.value.atoms.val (NativeExecution.view candidate.theory) := by
    simpa only [owner] using ordered
  have ownRoots : ∀ root ∈ candidate.theory.value.roots.val,
      root.val < (NativeTable.rows (NativeExecution.view candidate.theory)).length := by
    simpa only [owner] using boundedRoots
  obtain ⟨count, counted, countExact⟩ := UsizeCeiling.word_count64 program.value.atoms
  have wordCount : alloc.vec.Vec.len candidate.words = count := by
    apply UScalar.eq_of_val_eq
    change candidate.words.val.length = count.val
    rw [countExact]
    simpa only [NativePackedStorage.ExactStorage, owner] using shape
  have countedWords : core.num.Usize.div_ceil program.value.atoms 64#usize =
      ok (alloc.vec.Vec.len candidate.words) := by
    rw [wordCount]
    exact counted
  have checkReservations : NativePublicMembership.EmptyReservations program candidate := by
    exact ⟨selectionPreserved,
        fun returned reserved => preservesEmpty _ returned countedWords reserved⟩
  have ownReservations : NativePublicMembership.EmptyReservations candidate.theory candidate := by
    simpa only [owner] using checkReservations
  have equivalent : checked.verdict = .Stable ↔ Ferraris.Stable
      (TightEvaluation.interpretation (NativeMembership.denotes candidate))
      (NativeRootSemantics.assertions candidate.theory) := by
    exact NativePublicMembership.completed_answer_set candidate.theory candidate limits control checked
        heap consistent consistent shape ownOrder ownRoots ownReservations checkCompleted
  simpa only [owner] using equivalent

/-- The actual owned wrapper changes only the native result's successful
payload. A source stop keeps its reason; outer backend failure or divergence is
retained by the same bind. This equation promises neither completion nor a
membership verdict. -/
theorem subject_exact [reservation : VectorReservation]
    (candidate : theory.Interpretation) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) :
    checked.check_interpretation candidate limits control =
      (do let answer ← oracle.check candidate.theory candidate limits control
          match answer with
          | .Ok native => ok (core.result.Result.Ok { candidate, check := native })
          | .Err reason => ok (core.result.Result.Err reason)) := by
  simp only [checked.check_interpretation, theory.Interpretation.impl.theory, bind_tc_ok]
  congr 1
  funext answer
  cases answer <;>
    simp only [core.result.Result.Insts.CoreOpsTry.branch, bind_tc_ok,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
      core.convert.FromSame.from]

/-- A native source stop is propagated by the owned subject wrapper. No decision
record is published, and no membership conclusion is drawn from the stop. -/
theorem subject_refused [reservation : VectorReservation]
    (candidate : theory.Interpretation) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (reason : zetesis_cpu.cancellation.Stop)
    (refused : oracle.check candidate.theory candidate limits control = ok (.Err reason)) :
    checked.check_interpretation candidate limits control = ok (.Err reason) := by
  simp only [subject_exact, refused, bind_tc_ok]

/-- A completed owned wrapper retains exactly its input candidate and an actual
native check of that candidate against its own theory. The native call equation
is derived from wrapper completion; no caller-supplied decision is attached.

Proof: unfold the generated borrow and wrapper. Backend nonreturn and native
source refusal cannot publish a decision; an actual native success determines
both fields of the returned record. -/
theorem completed_subject_phases [reservation : VectorReservation]
    (candidate : theory.Interpretation) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation)
    (decision : checked.CheckedInterpretation)
    (completed : checked.check_interpretation candidate limits control = ok (.Ok decision)) :
    decision.candidate = candidate ∧
      oracle.check candidate.theory candidate limits control = ok (.Ok decision.check) := by
  rw [subject_exact] at completed
  cases actual : oracle.check candidate.theory candidate limits control with
  | ret outcome =>
      cases outcome with
      | Err reason =>
          simp only [actual, bind_tc_ok, Result.ok.injEq, reduceCtorEq] at completed
      | Ok native =>
          have same : ({ candidate, check := native } : checked.CheckedInterpretation) =
              decision := by
            simpa only [actual, bind_tc_ok, Result.ok.injEq,
                core.result.Result.Ok.injEq] using completed
          constructor
          · rw [← same]
          · rw [← same]
  | vis effect continuation =>
      simp only [actual, bind_tc_vis, vis_not_ok] at completed
  | div => simp only [actual, bind_tc_div, div_not_ok] at completed

/-- A completed owned check of an actually constructed subject retains that
subject, and its actual acceptance accessor is true exactly when the subject
is an answer set of the constructed theory. No stored verdict supplied by a
caller, structural invariant or semantic oracle agreement is assumed.

Proof: wrapper completion recovers its exact candidate and native call. The
constructor composition proves membership for that native call; the actual
acceptance accessor reads precisely its Stable verdict. -/
theorem completed_subject
    [allocation : ArcAllocation] [reservation : VectorReservation]
    (atomCount : Usize) (parts : theory.FormulaParts) (roots : alloc.vec.Vec Usize)
    (admission : theory.AdmissionLimits) (program : theory.Theory)
    (atoms : alloc.vec.Vec Usize) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation)
    (decision : checked.CheckedInterpretation)
    (preservesValue : ∀ (data : theory.Data) (view : theory.Theory),
      alloc.sync.Arc.new data = ok view → view.value = data)
    (preservesEmpty : NativeInterpretationStorage.PreservesEmpty program.value.atoms)
    (selectionPreserved : ∀ returned : alloc.vec.Vec Usize,
      alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new Usize) program.value.atoms =
        ok (.Ok (), returned) → returned.val = [])
    (theoryCompleted : theory.Theory.new atomCount parts roots admission = ok (.Ok program))
    (candidateCompleted : theory.Interpretation.new
      (core.iter.traits.collect.IntoIteratorVec Usize) program atoms = ok (.Ok candidate))
    (checkCompleted : checked.check_interpretation candidate limits control = ok (.Ok decision)) :
    decision.candidate = candidate ∧
      (checked.CheckedInterpretation.accepted decision = ok true ↔ Ferraris.Stable
        (TightEvaluation.interpretation (NativeMembership.denotes candidate))
        (NativeRootSemantics.assertions program)) := by
  obtain ⟨retained, actual⟩ := completed_subject_phases candidate limits control decision checkCompleted
  have membership : decision.check.verdict = .Stable ↔ Ferraris.Stable
      (TightEvaluation.interpretation (NativeMembership.denotes candidate))
      (NativeRootSemantics.assertions program) := by
    exact completed_constructed_check atomCount parts roots admission program atoms candidate
        limits control decision.check preservesValue preservesEmpty selectionPreserved
        theoryCompleted candidateCompleted actual
  exact ⟨retained, (NativeCheckedResults.accepted_iff decision).trans membership⟩

/-- An actual successful conversion of the completed decision returns the exact
constructed subject as an answer set. This is a semantic result about a typed
interpretation obtained through the actual constructors, check and conversion;
it does not assert that every freely constructed Lean record is an answer set.

Proof: the owned-check theorem supplies subject identity and membership from
acceptance. Actual conversion supplies a Stable verdict and retains the subject,
so its returned interpretation has that same identity and membership. -/
theorem completed_stable
    [allocation : ArcAllocation] [reservation : VectorReservation]
    (atomCount : Usize) (parts : theory.FormulaParts) (roots : alloc.vec.Vec Usize)
    (admission : theory.AdmissionLimits) (program : theory.Theory)
    (atoms : alloc.vec.Vec Usize) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation)
    (decision : checked.CheckedInterpretation) (result : checked.StableInterpretation)
    (preservesValue : ∀ (data : theory.Data) (view : theory.Theory),
      alloc.sync.Arc.new data = ok view → view.value = data)
    (preservesEmpty : NativeInterpretationStorage.PreservesEmpty program.value.atoms)
    (selectionPreserved : ∀ returned : alloc.vec.Vec Usize,
      alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new Usize) program.value.atoms =
        ok (.Ok (), returned) → returned.val = [])
    (theoryCompleted : theory.Theory.new atomCount parts roots admission = ok (.Ok program))
    (candidateCompleted : theory.Interpretation.new
      (core.iter.traits.collect.IntoIteratorVec Usize) program atoms = ok (.Ok candidate))
    (checkCompleted : checked.check_interpretation candidate limits control = ok (.Ok decision))
    (converted : checked.CheckedInterpretation.into_stable_interpretation decision = ok (.Ok result)) :
    result.interpretation = candidate ∧ Ferraris.Stable
      (TightEvaluation.interpretation (NativeMembership.denotes result.interpretation))
      (NativeRootSemantics.assertions program) := by
  obtain ⟨retained, membership⟩ := completed_subject atomCount parts roots admission program
    atoms candidate limits control decision preservesValue preservesEmpty selectionPreserved
    theoryCompleted candidateCompleted checkCompleted
  obtain ⟨verdict, transferred⟩ := (NativeCheckedResults.conversion_success_iff decision result).mp converted
  have accepted : checked.CheckedInterpretation.accepted decision = ok true := by
    exact (NativeCheckedResults.accepted_iff decision).mpr verdict
  have stable : Ferraris.Stable
      (TightEvaluation.interpretation (NativeMembership.denotes candidate))
      (NativeRootSemantics.assertions program) := by
    exact membership.mp accepted
  have same : result.interpretation = candidate := by
    exact transferred.trans retained
  exact ⟨same, by simpa only [same] using stable⟩

end NativeSubjectMembership
