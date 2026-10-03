import RuntimeProjection
import RuntimeControl
import RuntimeSubject
import SubjectMembership

open Aeneas Aeneas.Std Result
open ZetesisExtract RuntimeEffects
open Zetesis Zetesis.Refinement

/-!
# Answer-set membership under runtime observations

Successful eventful public checks project to the actual fixed checker. Its
existing Ferraris theorem then establishes answer-set membership and explains
both negative verdicts. The projection's library provider preserves sequences;
there is no premise that physical allocation always succeeds or that runtime
reads remain clear. The chosen control representative only clears unused ghost
bits; returning reads retain their original handles and deadline configuration.

These results concern the source-checked scalar reference checker. They do not
verify grounding, candidate enumeration, optimized checking, parallel schedules
or shaders. Rust, extraction, the audited context generalization and runtime
library/handle correspondences remain explicit trust boundaries.
-/
namespace RuntimeMembership

/-- The logical reservation projection supplies the two sequence-preservation
contracts required by the existing membership proof. -/
theorem empty_reservations (program : theory.Theory) (candidate : theory.Interpretation) :
    @PublicMembership.EmptyReservations ReservationEvents.fixed program candidate := by
  refine @PublicMembership.EmptyReservations.mk ReservationEvents.fixed program candidate ?_ ?_
  · intro returned reserved
    exact ReservationEvents.fixed_preserves Global (alloc.vec.Vec.new Usize) returned
      program.value.atoms reserved
  · intro returned reserved
    exact ReservationEvents.fixed_preserves Global (alloc.vec.Vec.new U64) returned
      (alloc.vec.Vec.len candidate.words) reserved

/-- A completed public runtime check accepts exactly the answer sets of the
stored theory. Actual setup, original satisfaction and proper-subset reduct
search are derived from the completed call, not supplied as oracle premises. -/
theorem completed_answer_set (program : theory.Theory) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation)
    (decision : oracle.Check) (events : List Event) (heap : RuntimeOwnership.Heap theory.Data)
    (programValid : RuntimeOwnership.Consistent heap program)
    (candidateValid : RuntimeOwnership.Consistent heap candidate.theory)
    (shape : PackedSetup.ExactStorage candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (roots : ∀ root ∈ program.value.roots.val, root.val < program.value.nodes.val.length)
    (run : Runs (ReferenceEvents.check program candidate limits (RuntimeControl.representative control))
      events (.Ok decision)) :
    decision.verdict = .Stable ↔ Ferraris.Stable
      (TightEvaluation.interpretation (Membership.denotes candidate))
      (RootSemantics.assertions program) := by
  have projected := RuntimeProjection.completed_check program candidate limits
    (RuntimeControl.representative control) decision events (RuntimeControl.fixed_clear control) run
  exact @PublicMembership.completed_answer_set ReservationEvents.fixed program candidate limits
    (RuntimeControl.representative control) decision heap programValid candidateValid shape ordered roots
    (empty_reservations program candidate) projected

/-- A runtime NotModel verdict names an asserted root with a present false
original value and excludes answer-set membership. The evidence belongs to the
actual returned record, not a separately supplied counterexample. -/
theorem completed_not_model (program : theory.Theory) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation)
    (decision : oracle.Check) (events : List Event)
    (shape : PackedSetup.ExactStorage candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (roots : ∀ root ∈ program.value.roots.val, root.val < program.value.nodes.val.length)
    (run : Runs (ReferenceEvents.check program candidate limits (RuntimeControl.representative control))
      events (.Ok decision))
    (root : Usize) (rejected : decision.verdict = .NotModel root) :
    ¬ Ferraris.Stable (TightEvaluation.interpretation (Membership.denotes candidate))
        (RootSemantics.assertions program) ∧ root ∈ program.value.roots.val ∧
      (EvaluationSpecification.values candidate none program.value.nodes.val)[root.val]? = some false := by
  have projected := RuntimeProjection.completed_check program candidate limits
    (RuntimeControl.representative control) decision events (RuntimeControl.fixed_clear control) run
  exact @PublicMembership.completed_not_model ReservationEvents.fixed program candidate limits
    (RuntimeControl.representative control) decision shape ordered roots projected root rejected

/-- A runtime NonMinimal verdict retains the actual program-owned proper subset
that models the candidate's Ferraris reduct. -/
theorem completed_nonminimal (program : theory.Theory) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation)
    (decision : oracle.Check) (events : List Event) (heap : RuntimeOwnership.Heap theory.Data)
    (programValid : RuntimeOwnership.Consistent heap program)
    (candidateValid : RuntimeOwnership.Consistent heap candidate.theory)
    (shape : PackedSetup.ExactStorage candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (roots : ∀ root ∈ program.value.roots.val, root.val < program.value.nodes.val.length)
    (run : Runs (ReferenceEvents.check program candidate limits (RuntimeControl.representative control))
      events (.Ok decision))
    (witness : theory.Interpretation) (rejected : decision.verdict = .NonMinimal witness) :
    witness.theory = program ∧
      Ferraris.ProperSub (TightEvaluation.interpretation (Membership.denotes witness))
        (TightEvaluation.interpretation (Membership.denotes candidate)) ∧
      Ferraris.Models (TightEvaluation.interpretation (Membership.denotes witness))
        (Ferraris.ReductTheory (TightEvaluation.interpretation (Membership.denotes candidate))
          (RootSemantics.assertions program)) := by
  have projected := RuntimeProjection.completed_check program candidate limits
    (RuntimeControl.representative control) decision events (RuntimeControl.fixed_clear control) run
  exact @PublicMembership.completed_nonminimal ReservationEvents.fixed program candidate limits
    (RuntimeControl.representative control) decision heap programValid candidateValid shape ordered roots
    (empty_reservations program candidate) projected witness rejected

/-- Actual successful construction and the runtime owned check establish
membership of the retained candidate. Constructor allocation operations have
their own supplied contracts; the checking history may contain independently
observed reservations. No shared deterministic allocation history is assumed. -/
theorem completed_subject [allocation : ArcAllocation] [reservation : VectorReservation]
    (atomCount : Usize) (nodes : alloc.vec.Vec theory.Node) (roots : alloc.vec.Vec Usize)
    (admission : theory.AdmissionLimits) (program : theory.Theory)
    (atoms : alloc.vec.Vec Usize) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation)
    (decision : checked.CheckedInterpretation) (events : List Event)
    (preservesValue : ∀ (data : theory.Data) (view : theory.Theory),
      alloc.sync.Arc.new data = ok view → view.value = data)
    (preservesEmpty : InterpretationStorage.PreservesEmpty program.value.atoms)
    (theoryCompleted : theory.Theory.new atomCount nodes roots admission = ok (.Ok program))
    (candidateCompleted : theory.Interpretation.new
      (core.iter.traits.collect.IntoIteratorVec Usize) program atoms = ok (.Ok candidate))
    (run : Runs (ReferenceEvents.checkInterpretation candidate limits (RuntimeControl.representative control))
      events (.Ok decision)) :
    decision.candidate = candidate ∧
      (checked.CheckedInterpretation.accepted decision = ok true ↔ Ferraris.Stable
        (TightEvaluation.interpretation (Membership.denotes candidate))
        (RootSemantics.assertions program)) := by
  obtain ⟨owner, shape, ordered, boundedRoots⟩ := SubjectMembership.constructed_input atomCount nodes
    roots admission program atoms candidate preservesValue preservesEmpty theoryCompleted candidateCompleted
  obtain ⟨retained, checked⟩ := RuntimeSubject.completed_subject candidate limits
    (RuntimeControl.representative control) events decision run
  let heap : RuntimeOwnership.Heap theory.Data := fun identity =>
    if identity = candidate.theory.owner then some candidate.theory.value else none
  have consistent : RuntimeOwnership.Consistent heap candidate.theory := by
    simp only [RuntimeOwnership.Consistent, heap, ite_true]
  have ownOrder : EvaluationSpecification.Ordered candidate.theory.value.nodes.val := by
    simpa only [owner] using ordered
  have ownRoots : ∀ root ∈ candidate.theory.value.roots.val,
      root.val < candidate.theory.value.nodes.val.length := by
    simpa only [owner] using boundedRoots
  have equivalent := completed_answer_set candidate.theory candidate limits control decision.check
    events heap consistent consistent shape ownOrder ownRoots checked
  exact ⟨retained, (CheckedResults.accepted_iff decision).trans (by simpa only [owner] using equivalent)⟩

/-- Successful conversion after a constructed subject's runtime check retains
that exact candidate as an answer set. The conversion is the generated consuming
operation, not a separate assertion that its stored verdict is correct. -/
theorem completed_stable [allocation : ArcAllocation] [reservation : VectorReservation]
    (atomCount : Usize) (nodes : alloc.vec.Vec theory.Node) (roots : alloc.vec.Vec Usize)
    (admission : theory.AdmissionLimits) (program : theory.Theory)
    (atoms : alloc.vec.Vec Usize) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation)
    (decision : checked.CheckedInterpretation) (result : checked.StableInterpretation)
    (events : List Event)
    (preservesValue : ∀ (data : theory.Data) (view : theory.Theory),
      alloc.sync.Arc.new data = ok view → view.value = data)
    (preservesEmpty : InterpretationStorage.PreservesEmpty program.value.atoms)
    (theoryCompleted : theory.Theory.new atomCount nodes roots admission = ok (.Ok program))
    (candidateCompleted : theory.Interpretation.new
      (core.iter.traits.collect.IntoIteratorVec Usize) program atoms = ok (.Ok candidate))
    (run : Runs (ReferenceEvents.checkInterpretation candidate limits (RuntimeControl.representative control))
      events (.Ok decision))
    (converted : checked.CheckedInterpretation.into_stable_interpretation decision = ok (.Ok result)) :
    result.interpretation = candidate ∧ Ferraris.Stable
      (TightEvaluation.interpretation (Membership.denotes result.interpretation))
      (RootSemantics.assertions program) := by
  obtain ⟨retained, equivalent⟩ := completed_subject atomCount nodes roots admission program atoms
    candidate limits control decision events preservesValue preservesEmpty theoryCompleted candidateCompleted run
  obtain ⟨verdict, transferred⟩ := (CheckedResults.conversion_success_iff decision result).mp converted
  have accepted : checked.CheckedInterpretation.accepted decision = ok true :=
    (CheckedResults.accepted_iff decision).mpr verdict
  have member : Ferraris.Stable
      (TightEvaluation.interpretation (Membership.denotes candidate))
      (RootSemantics.assertions program) := equivalent.mp accepted
  have same : result.interpretation = candidate := transferred.trans retained
  exact ⟨same, by simpa only [same] using member⟩

end RuntimeMembership
