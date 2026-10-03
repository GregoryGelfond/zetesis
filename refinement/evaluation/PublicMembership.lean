import OwnedMembership
import MembershipVerdicts
import ReservedStorage
import SearchFrame
import PublicCheckPhases

open Aeneas Aeneas.Std Result
open ZetesisExtract
open Zetesis Zetesis.Refinement

/-!
# Semantic evidence for the public reference checker

The completed actual `oracle.check` returns Stable exactly for an answer set of
its asserted formula theory. A NotModel result identifies an asserted root with
a present false original value; NonMinimal returns the actual program-owned
proper-subset reduct model.

The proof recovers the generated wrapper's calls. Evaluation supplies the frozen
mask, selection supplies the candidate carrier, and search supplies reduct
minimality. Their correctness is derived, not included in the wrapper premises.
The remaining contracts describe exact candidate storage, admitted node/root
structure, a consistent immutable heap, and the preservation of empty sequences
by the two successful reservations consumed without clearing.

These are completed results of the generated function under the recorded pure
library operations and fixed observation values. Allocation success, termination
of an arbitrary reservation operation, changing runtime observation histories,
source grounding and other solver paths are not claimed.
-/
namespace PublicMembership

open CountermodelSemantics

/-- A returned original-model failure names an asserted root whose computed
original value is false. The lookup is present: missing storage cannot serve as
false-root evidence.

Proof: the actual root scan supplies the root's stored occurrence and false
value. Original evaluation supplies the complete value sequence; admitted root
bounds justify the corresponding present lookup in that sequence. -/
theorem original_counterexample (frozen : FrozenEvaluation)
    (root : Usize) (after : oracle.Work)
    (failed : oracle.failed_root frozen.program frozen.values.slice frozen.after =
      ok (.Ok (some root), after)) :
    root ∈ frozen.program.value.roots.val ∧
      (EvaluationSpecification.values frozen.candidate none
        frozen.program.value.nodes.val)[root.val]? = some false := by
  have valuesExact : frozen.values.val = EvaluationSpecification.values
      frozen.candidate none frozen.program.value.nodes.val :=
    FixedEvaluationLoop.completed_values frozen.program frozen.candidate none
      frozen.old frozen.before frozen.stored frozen.ordered
      (by intro mask member; cases member) frozen.values frozen.after frozen.completed
  have covered : ∀ root ∈ frozen.program.value.roots.val,
      root.val < frozen.values.slice.val.length := by
    intro tested member
    change tested.val < frozen.values.val.length
    rw [valuesExact, EvaluationSpecification.values_length]
    exact frozen.rootsBounded tested member
  have receipt := FixedRootScan.returned_report frozen.program frozen.values.slice
    frozen.after after (.Ok (some root)) covered failed
  obtain ⟨index, inside, _, rootAt, falseValue, _, _⟩ := receipt.2
  have member : root ∈ frozen.program.value.roots.val := by
    rw [rootAt]
    exact List.getElem_mem inside
  have bounded : root.val < frozen.values.val.length := covered root member
  have falseLookup : frozen.values.val[root.val]? = some false := by
    unfold FixedRootScan.truth at falseValue
    change (frozen.values.val[root.val]?).getD false = false at falseValue
    rw [List.getElem?_eq_getElem bounded] at falseValue ⊢
    simpa only [Option.getD_some] using congrArg some falseValue
  exact ⟨member, by simpa only [valuesExact] using falseLookup⟩

/-- Only the two buffers consumed without clearing need an empty-sequence
contract. Each implication concerns the actual reservation on an empty input at
its exact count. Source refusal, backend failure and divergence remain possible;
no physical capacity or changing allocator history is asserted. -/
structure EmptyReservations [reservation : VectorReservation]
    (program : theory.Theory) (candidate : theory.Interpretation) : Prop where
  selection : ∀ returned : alloc.vec.Vec Usize,
    alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new Usize) program.value.atoms =
      ok (.Ok (), returned) → returned.val = []
  words : ∀ returned : alloc.vec.Vec U64,
    alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new U64)
      (alloc.vec.Vec.len candidate.words) = ok (.Ok (), returned) → returned.val = []

/-- The recovered original call and input storage/structure contracts establish
an actually evaluated frozen interpretation. This only packages an existing call;
it assumes neither original satisfaction nor meaning of a supplied mask. -/
def originalEvaluation [reservation : VectorReservation]
    {program : theory.Theory} {candidate : theory.Interpretation}
    {limits : oracle.Limits} {control : zetesis_cpu.cancellation.Cancellation}
    (original : PublicCheckPhases.Original program candidate limits control)
    (shape : PackedSetup.ExactStorage candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (roots : ∀ root ∈ program.value.roots.val, root.val < program.value.nodes.val.length) :
    FrozenEvaluation :=
  { program, candidate, old := original.old, before := WorkInitialization.initial limits control,
    values := original.values, after := original.after,
    stored := PackedSetup.exact_storage_readable candidate shape,
    ordered, rootsBounded := roots, completed := original.evaluated }

/-- A recovered searched branch decides answer-set membership. The actual
reservation returns and the two empty-sequence contracts establish its initial
selection and zero subset. Actual ownership supplies agreement of universes.
The existing membership composition then derives mask meaning, candidate coverage
and proper-subset completeness from the actual phase calls. -/
theorem searched_answer_set [reservation : VectorReservation]
    {program : theory.Theory} {candidate : theory.Interpretation}
    {limits : oracle.Limits} {control : zetesis_cpu.cancellation.Cancellation}
    (original : PublicCheckPhases.Original program candidate limits control)
    (checked : oracle.Check) (searched : PublicCheckPhases.Searched original checked)
    (heap : RuntimeOwnership.Heap theory.Data)
    (programValid : RuntimeOwnership.Consistent heap program)
    (candidateValid : RuntimeOwnership.Consistent heap candidate.theory)
    (shape : PackedSetup.ExactStorage candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (roots : ∀ root ∈ program.value.roots.val, root.val < program.value.nodes.val.length)
    (preserved : EmptyReservations program candidate) :
    checked.verdict = .Stable ↔ Ferraris.Stable
      (TightEvaluation.interpretation (Membership.denotes candidate))
      (RootSemantics.assertions program) := by
  let frozen := originalEvaluation original shape ordered roots
  have vacant : searched.destination.val = [] := preserved.selection searched.destination
    (ReservedStorage.completed_reservation Usize program.value.atoms
      searched.destination searched.selectedReserved)
  have wordsVacant : searched.wordDestination.val = [] := preserved.words searched.wordDestination
    (ReservedStorage.completed_reservation U64 (alloc.vec.Vec.len candidate.words)
      searched.wordDestination searched.wordsReserved)
  have countExact : (alloc.vec.Vec.len candidate.words).val = candidate.words.val.length := by
    simp
  have membership : searched.found = false ↔ Ferraris.Stable
      (TightEvaluation.interpretation (Membership.denotes candidate))
      (RootSemantics.assertions program) :=
    OwnedMembership.completed frozen heap programValid candidateValid original.owned shape
      searched.rootAfter searched.originalModel searched.destination vacant searched.selected
      searched.selectionAfter searched.selectedComplete searched.wordDestination wordsVacant
      (alloc.vec.Vec.len candidate.words) countExact searched.words searched.resized
      searched.old searched.output searched.found searched.subset searched.after searched.completed
  have published : checked.verdict = .Stable ↔ searched.found = false := by
    have verdict := congrArg oracle.Check.verdict searched.published
    simp only [verdict]
    cases searched.found <;> simp
  exact published.trans membership

/-- A recovered NonMinimal branch returns its own program-owned proper-subset
model of the candidate's reduct. The witness is the actual search output, not a
separately proposed interpretation.

Proof: derive empty setup and the selected carrier from the recovered calls.
The existing witness theorem supplies properness and reduct satisfaction; the
actual search frame supplies retention of the initial program owner. -/
theorem searched_witness [reservation : VectorReservation]
    {program : theory.Theory} {candidate : theory.Interpretation}
    {limits : oracle.Limits} {control : zetesis_cpu.cancellation.Cancellation}
    (original : PublicCheckPhases.Original program candidate limits control)
    (checked : oracle.Check) (searched : PublicCheckPhases.Searched original checked)
    (heap : RuntimeOwnership.Heap theory.Data)
    (programValid : RuntimeOwnership.Consistent heap program)
    (candidateValid : RuntimeOwnership.Consistent heap candidate.theory)
    (shape : PackedSetup.ExactStorage candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (roots : ∀ root ∈ program.value.roots.val, root.val < program.value.nodes.val.length)
    (preserved : EmptyReservations program candidate)
    (witness : theory.Interpretation)
    (published : checked.verdict = .NonMinimal witness) :
    witness.theory = program ∧
      Ferraris.ProperSub (TightEvaluation.interpretation (Membership.denotes witness))
        (TightEvaluation.interpretation (Membership.denotes candidate)) ∧
      Ferraris.Models (TightEvaluation.interpretation (Membership.denotes witness))
        (Ferraris.ReductTheory (TightEvaluation.interpretation (Membership.denotes candidate))
          (RootSemantics.assertions program)) := by
  let frozen := originalEvaluation original shape ordered roots
  have selectedEmpty : searched.destination.val = [] := preserved.selection searched.destination
    (ReservedStorage.completed_reservation Usize program.value.atoms
      searched.destination searched.selectedReserved)
  have wordsEmpty : searched.wordDestination.val = [] := preserved.words searched.wordDestination
    (ReservedStorage.completed_reservation U64 (alloc.vec.Vec.len candidate.words)
      searched.wordDestination searched.wordsReserved)
  have sameUniverse : program.value.atoms = candidate.theory.value.atoms :=
    congrArg theory.Data.atoms
      (OwnerChecks.accepted_data heap program candidate programValid candidateValid original.owned)
  have countExact : (alloc.vec.Vec.len candidate.words).val = candidate.words.val.length := by simp
  have zero : PackedSubsets.Represents (ScalarSubsets.raw searched.words)
      ([] : List (Fin candidate.theory.value.atoms.val)) :=
    PackedSetup.resized_empty candidate shape searched.wordDestination wordsEmpty
      (alloc.vec.Vec.len candidate.words) countExact searched.words searched.resized
  have found : searched.found = true ∧ searched.subset = witness := by
    cases foundValue : searched.found with
    | false => simp [searched.published, foundValue] at published
    | true =>
        refine ⟨rfl, ?_⟩
        simpa [searched.published, foundValue] using published
  have completed : oracle.find_countermodel program original.values.slice searched.selected.slice
      { theory := program, words := searched.words } searched.old searched.selectionAfter =
      ok (.Ok true, witness, searched.output, searched.after) := by
    simpa only [found.1, found.2] using searched.completed
  have semantic :
      Ferraris.ProperSub (TightEvaluation.interpretation (Membership.denotes witness))
        (TightEvaluation.interpretation (Membership.denotes candidate)) ∧
      Ferraris.Models (TightEvaluation.interpretation (Membership.denotes witness))
        frozen.reduct :=
    MembershipVerdicts.returned_witness frozen searched.destination searched.rootAfter
    sameUniverse selectedEmpty searched.selected searched.selectionAfter searched.selectedComplete
    { theory := program, words := searched.words } searched.old
    (congrArg UScalar.val sameUniverse) zero witness searched.output searched.after completed
  let atoms := PackedSubsets.selectedAtoms candidate.theory.value.atoms.val
    (ScalarSubsets.raw candidate.words)
  have coordinates : searched.selected.val.map UScalar.val = atoms.map Fin.val :=
    FixedSelection.completed_selection program candidate searched.destination searched.rootAfter
      sameUniverse frozen.stored selectedEmpty searched.selected searched.selectionAfter
      searched.selectedComplete
  have frame : SearchFrame.Frame { theory := program, words := searched.words }
      searched.selectionAfter witness searched.after :=
    SearchFrame.returned_frame frozen atoms searched.selected.slice coordinates
    (PackedSubsets.selected_atoms_nodup _ _) { theory := program, words := searched.words }
    searched.old searched.selectionAfter (congrArg UScalar.val sameUniverse) zero
    (.Ok true) witness searched.output searched.after completed
  exact ⟨frame.theory, semantic⟩

/-- A completed actual public check returns Stable exactly when its candidate
is an answer set of the asserted formula theory. No mask agreement, correct
selection or completed search is assumed: the actual wrapper return supplies
its phase calls, whose existing refinements establish those properties.

The premises concern candidate storage, admitted node/root structure, a common
immutable heap, and successful reservation's preservation of empty sequences.
They assume neither allocation success nor termination of an arbitrary provider.
The generated library interpretation retains fixed observations and one supplied
reservation operation throughout this invocation.

Proof: recover the wrapper's actual branches. Original failure excludes
membership; the searched branch has the equivalence established above. -/
theorem completed_answer_set [reservation : VectorReservation]
    (program : theory.Theory) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation)
    (checked : oracle.Check) (heap : RuntimeOwnership.Heap theory.Data)
    (programValid : RuntimeOwnership.Consistent heap program)
    (candidateValid : RuntimeOwnership.Consistent heap candidate.theory)
    (shape : PackedSetup.ExactStorage candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (roots : ∀ root ∈ program.value.roots.val, root.val < program.value.nodes.val.length)
    (preserved : EmptyReservations program candidate)
    (completed : oracle.check program candidate limits control = ok (.Ok checked)) :
    checked.verdict = .Stable ↔ Ferraris.Stable
      (TightEvaluation.interpretation (Membership.denotes candidate))
      (RootSemantics.assertions program) := by
  obtain ⟨original, branch⟩ := PublicCheckPhases.completed_phases
    program candidate limits control checked completed
  rcases branch with rejected | available
  · obtain ⟨root, after, failed, published⟩ := rejected
    let frozen := originalEvaluation original shape ordered roots
    have absent : ¬ Ferraris.Stable
        (TightEvaluation.interpretation (Membership.denotes candidate))
        (RootSemantics.assertions program) :=
      MembershipVerdicts.original_failure frozen root after failed
    have notStable : checked.verdict ≠ .Stable := by
      rw [published]
      intro impossible
      cases impossible
    exact ⟨fun accepted => False.elim (notStable accepted),
      fun stable => False.elim (absent stable)⟩
  · cases available with
    | intro searched =>
        exact searched_answer_set original checked searched heap programValid candidateValid
          shape ordered roots preserved

/-- A completed public NotModel result carries concrete original-model failure:
its node ID is asserted, its present original value is false, and the candidate
is not an answer set. This branch requires no later reservation contract because
those setup calls are not reached. The evidence is recovered from the wrapper;
no failed-root call is a theorem premise. -/
theorem completed_not_model [reservation : VectorReservation]
    (program : theory.Theory) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation)
    (checked : oracle.Check) (shape : PackedSetup.ExactStorage candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (roots : ∀ root ∈ program.value.roots.val, root.val < program.value.nodes.val.length)
    (completed : oracle.check program candidate limits control = ok (.Ok checked))
    (root : Usize) (notModel : checked.verdict = .NotModel root) :
    ¬ Ferraris.Stable (TightEvaluation.interpretation (Membership.denotes candidate))
        (RootSemantics.assertions program) ∧
      root ∈ program.value.roots.val ∧
      (EvaluationSpecification.values candidate none program.value.nodes.val)[root.val]? =
        some false := by
  obtain ⟨original, branch⟩ := PublicCheckPhases.completed_phases
    program candidate limits control checked completed
  rcases branch with rejected | available
  · obtain ⟨actualRoot, after, failed, published⟩ := rejected
    have sameRoot : actualRoot = root := by
      simpa only [published, oracle.Verdict.NotModel.injEq] using notModel
    subst actualRoot
    let frozen := originalEvaluation original shape ordered roots
    exact ⟨MembershipVerdicts.original_failure frozen root after failed,
      original_counterexample frozen root after failed⟩
  · cases available with
    | intro searched =>
        cases found : searched.found <;>
          simp [searched.published, found] at notModel

/-- A completed public NonMinimal result returns the actual program-owned
proper-subset model of the candidate's Ferraris reduct. Its conversion from
packed storage and its semantic properties follow from the wrapper's recovered
calls, rather than from an assumed countermodel or search oracle. -/
theorem completed_nonminimal [reservation : VectorReservation]
    (program : theory.Theory) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation)
    (checked : oracle.Check) (heap : RuntimeOwnership.Heap theory.Data)
    (programValid : RuntimeOwnership.Consistent heap program)
    (candidateValid : RuntimeOwnership.Consistent heap candidate.theory)
    (shape : PackedSetup.ExactStorage candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (roots : ∀ root ∈ program.value.roots.val, root.val < program.value.nodes.val.length)
    (preserved : EmptyReservations program candidate)
    (completed : oracle.check program candidate limits control = ok (.Ok checked))
    (witness : theory.Interpretation) (nonminimal : checked.verdict = .NonMinimal witness) :
    witness.theory = program ∧
      Ferraris.ProperSub (TightEvaluation.interpretation (Membership.denotes witness))
        (TightEvaluation.interpretation (Membership.denotes candidate)) ∧
      Ferraris.Models (TightEvaluation.interpretation (Membership.denotes witness))
        (Ferraris.ReductTheory (TightEvaluation.interpretation (Membership.denotes candidate))
          (RootSemantics.assertions program)) := by
  obtain ⟨original, branch⟩ := PublicCheckPhases.completed_phases
    program candidate limits control checked completed
  rcases branch with rejected | available
  · obtain ⟨root, after, failed, published⟩ := rejected
    simp [published] at nonminimal
  · cases available with
    | intro searched =>
        exact searched_witness original checked searched heap programValid candidateValid
          shape ordered roots preserved witness nonminimal

end PublicMembership
