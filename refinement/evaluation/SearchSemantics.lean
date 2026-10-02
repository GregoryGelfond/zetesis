import SearchRepresentation
import SubsetQueryTotal

open Aeneas Aeneas.Std Result
open ZetesisExtract
open Zetesis Zetesis.Refinement

/-!
# Meaning of a search over an actually frozen reduct

The frozen truth vector comes from the actual original evaluator. This record
retains that call and its preconditions, so later search theorems do not assume
an oracle's correctness or an arbitrary mask's semantic meaning.
-/
namespace CountermodelSemantics

/-- Evidence that the original evaluation completed. Owner identity and source
    admission are not represented by these value-level fields. -/
structure FrozenEvaluation where
  program : theory.Theory
  candidate : theory.Interpretation
  old : alloc.vec.Vec Bool
  before : oracle.Work
  values : alloc.vec.Vec Bool
  after : oracle.Work
  stored : Membership.Represented candidate
  ordered : EvaluationSpecification.Ordered program.value.nodes.val
  rootsBounded : ∀ root ∈ program.value.roots.val, root.val < program.value.nodes.val.length
  completed : oracle.evaluate program candidate none old before =
    ok (core.result.Result.Ok (), values, after)

/-- The semantic reduct of the original roots with respect to the candidate. -/
noncomputable def FrozenEvaluation.reduct (frozen : FrozenEvaluation) : Ferraris.Theory Nat :=
  Ferraris.ReductTheory (TightEvaluation.interpretation (Membership.denotes frozen.candidate))
    (RootSemantics.assertions frozen.program)

/-- Original completion supplies mask coverage from the actual evaluator's
    output-length law. It is not a separate assumption about frozen storage. -/
theorem frozen_covered (frozen : FrozenEvaluation) :
    frozen.program.value.nodes.val.length ≤ frozen.values.slice.val.length := by
  have exactValues := FixedEvaluationLoop.completed_values frozen.program frozen.candidate
    none frozen.old frozen.before frozen.stored frozen.ordered
    (by intro mask member; cases member) frozen.values frozen.after frozen.completed
  change frozen.program.value.nodes.val.length ≤ frozen.values.val.length
  rw [exactValues, EvaluationSpecification.values_length]

/-- Each actual completed query is the satisfaction test for the frozen reduct.
    The tested interpretation need not satisfy the original program. -/
theorem query_meaning (frozen : FrozenEvaluation) (tested : theory.Interpretation)
    (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (stored : Membership.Represented tested) (accepted : Bool)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (completed : oracle.check_subset frozen.program tested frozen.values.slice old before =
      ok (core.result.Result.Ok accepted, output, after)) :
    accepted = true ↔ Ferraris.Models (TightEvaluation.interpretation
      (Membership.denotes tested)) frozen.reduct := by
  exact SubsetQuery.completed_reduct frozen.program frozen.candidate tested frozen.old old
    frozen.before before frozen.stored stored frozen.ordered frozen.rootsBounded
    frozen.values output frozen.after after accepted frozen.completed completed

/-- A completed false query refutes exactly its represented positional state.
    It does not refute supersets, unrelated candidates or an unfinished query. -/
theorem query_refutes (frozen : FrozenEvaluation) {size : Nat}
    (atoms : List (Fin size)) (bits : List Bool) (tested : theory.Interpretation)
    (old : alloc.vec.Vec Bool) (before : oracle.Work)
    (sizeExact : tested.theory.value.atoms.val = size)
    (represented : PackedSubsets.Represents (ScalarSubsets.raw tested.words)
      (SubsetCounter.selected atoms bits))
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (completed : oracle.check_subset frozen.program tested frozen.values.slice old before =
      ok (core.result.Result.Ok false, output, after)) :
    ¬ Ferraris.Models (TightEvaluation.interpretation
      (FiniteMembership.candidate ((SubsetCounter.selected atoms bits).map Fin.val)))
      frozen.reduct := by
  have exactMeaning := query_meaning frozen tested old before
    (SearchRepresentation.stored _ tested sizeExact represented) false output after completed
  rw [SearchRepresentation.denotes _ tested sizeExact represented] at exactMeaning
  intro modeled
  exact Bool.false_ne_true (exactMeaning.mpr modeled)

end CountermodelSemantics
