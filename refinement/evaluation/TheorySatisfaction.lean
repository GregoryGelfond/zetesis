import RootScan
import RootSemantics

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Satisfaction from generated evaluation and root checking

A successful evaluator call supplies one truth per stored node. Root admission
then justifies every read of the actual generated root scan. When that scan
completes, absence of a failed root is equivalent to satisfaction of the asserted
theory, originally or under the Ferraris reduct.

Completion equations identify actual generated results; they do not assume
semantic correctness. Typed refusal outcomes cannot satisfy those equations
and are never converted into negative model verdicts. These
proofs retain the fixed-observation and library-model boundaries of FixedLoop.
They do not verify owner checks, allocation, subset minimality or enumeration.
-/
namespace TheorySatisfaction

/-- A completed generated original evaluation followed by a completed generated
root scan returns no failed root exactly when the interpretation models the
represented theory. Root bounds refer to stored nodes, not supplied truth data.

Proof: derive truth-table length from evaluation, justify root reads, identify
the scan result with truth of every root, and apply the original ASP root law.
-/
theorem completed_original (program : theory.Theory) (candidate : theory.Interpretation)
    (oldOutput : alloc.vec.Vec Bool) (evaluationWork scanWork : oracle.Work)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (rootsBounded : ∀ root ∈ program.value.roots.val,
      root.val < program.value.nodes.val.length)
    (output : alloc.vec.Vec Bool) (evaluationReturned scanReturned : oracle.Work)
    (answer : Option Usize)
    (evaluated : oracle.evaluate program candidate none oldOutput evaluationWork =
      ok (core.result.Result.Ok (), output, evaluationReturned))
    (scanned : oracle.failed_root program output.slice scanWork =
      ok (core.result.Result.Ok answer, scanReturned)) :
    answer = none ↔ Zetesis.Ferraris.Models
      (Zetesis.TightEvaluation.interpretation (Membership.denotes candidate))
      (RootSemantics.assertions program) := by
  have outputValues : output.val =
      EvaluationSpecification.values candidate none program.value.nodes.val :=
    FixedEvaluationLoop.completed_values program candidate none oldOutput evaluationWork
      stored ordered (by intro mask member; cases member) output evaluationReturned evaluated
  have outputLength : output.val.length = program.value.nodes.val.length := by
    rw [outputValues, EvaluationSpecification.values_length]
  have rootsCovered : ∀ root ∈ program.value.roots.val,
      root.val < output.slice.val.length := by
    intro root member
    change root.val < output.val.length
    rw [outputLength]
    exact rootsBounded root member
  calc
    answer = none ↔ ∀ root ∈ program.value.roots.val,
        FixedRootScan.truth output.slice root = true :=
      FixedRootScan.completed_none_iff program output.slice scanWork scanReturned answer
        rootsCovered scanned
    _ ↔ Zetesis.TightEvaluation.rootsTrue output.val
        (program.value.roots.val.map (fun root => root.val)) = true := by
      simp [Zetesis.TightEvaluation.rootsTrue, List.all_eq_true, FixedRootScan.truth,
        List.getD_eq_getElem?_getD]
      rfl
    _ ↔ _ := RootSemantics.original_roots program candidate oldOutput evaluationWork
      stored ordered output evaluationReturned evaluated

/-- Two successful generated evaluations followed by the completed generated
root scan decide satisfaction of the frozen Ferraris reduct theory. The first
evaluation's actual output fixes the mask; the tested interpretation is arbitrary.

Proof: the two-call value theorem establishes the frozen truth table and its
length. Stored root bounds justify the scan; the scan's exact conjunction agrees
with the existing ASP reduct-theory law. This is satisfaction, not minimality.
-/
theorem completed_reduct (program : theory.Theory) (outer tested : theory.Interpretation)
    (oldOriginal oldTested : alloc.vec.Vec Bool)
    (outerWork testedWork scanWork : oracle.Work)
    (outerStored : Membership.Represented outer) (testedStored : Membership.Represented tested)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (rootsBounded : ∀ root ∈ program.value.roots.val,
      root.val < program.value.nodes.val.length)
    (original result : alloc.vec.Vec Bool)
    (outerReturned testedReturned scanReturned : oracle.Work) (answer : Option Usize)
    (originalComplete : oracle.evaluate program outer none oldOriginal outerWork =
      ok (core.result.Result.Ok (), original, outerReturned))
    (testedComplete : oracle.evaluate program tested (some original.slice) oldTested testedWork =
      ok (core.result.Result.Ok (), result, testedReturned))
    (scanned : oracle.failed_root program result.slice scanWork =
      ok (core.result.Result.Ok answer, scanReturned)) :
    answer = none ↔ Zetesis.Ferraris.Models
      (Zetesis.TightEvaluation.interpretation (Membership.denotes tested))
      (Zetesis.Ferraris.ReductTheory
        (Zetesis.TightEvaluation.interpretation (Membership.denotes outer))
        (RootSemantics.assertions program)) := by
  have resultValues : result.val = Zetesis.ReductEvaluation.values
      (Membership.denotes outer) (Membership.denotes tested)
      (program.value.nodes.val.map EvaluationSemantics.node) :=
    FixedReductEvaluation.completed_values program outer tested oldOriginal oldTested
      outerWork testedWork outerStored testedStored ordered original result outerReturned
      testedReturned originalComplete testedComplete
  have resultLength : result.val.length = program.value.nodes.val.length := by
    rw [resultValues, Zetesis.ReductEvaluation.values_length, List.length_map]
  have rootsCovered : ∀ root ∈ program.value.roots.val,
      root.val < result.slice.val.length := by
    intro root member
    change root.val < result.val.length
    rw [resultLength]
    exact rootsBounded root member
  calc
    answer = none ↔ ∀ root ∈ program.value.roots.val,
        FixedRootScan.truth result.slice root = true :=
      FixedRootScan.completed_none_iff program result.slice scanWork scanReturned answer
        rootsCovered scanned
    _ ↔ Zetesis.TightEvaluation.rootsTrue result.val
        (program.value.roots.val.map (fun root => root.val)) = true := by
      simp [Zetesis.TightEvaluation.rootsTrue, List.all_eq_true, FixedRootScan.truth,
        List.getD_eq_getElem?_getD]
      rfl
    _ ↔ _ := RootSemantics.reduct_roots program outer tested oldOriginal oldTested outerWork
      testedWork outerStored testedStored ordered original result outerReturned
      testedReturned originalComplete testedComplete

end TheorySatisfaction
