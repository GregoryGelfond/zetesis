import Specification
import Zetesis.ReductEvaluation

open Aeneas Aeneas.Std
open ZetesisExtract

/-!
# Connecting concrete truth sequences to Ferraris reduct truth

A structural conversion reads each extracted node constructor and each Usize
index as a natural number. The concrete finite folds then equal the existing
mathematical folds: an unmasked pass computes original truth, and a masked pass
computes reduct truth when its mask is the concrete original pass's output.
The mask equation is a data provenance premise to be supplied by a completed
original trace, not an assumed evaluator or semantic-mask agreement.

This module imports the existing semantic sources, checked without changes by
the same Lean 4.31 toolchain as the extraction package. It does not identify
Lean object files across compiler versions. Generated-step trace refinement,
Rust owner identity, read observations across a whole loop and source grounding
remain separate obligations. These fold identities use matching false defaults;
they do not permit an extracted implementation to read outside its bounds.
-/

namespace EvaluationSemantics

/-- Preserve a node's Boolean constructor and numeric atom/child identities.
This conversion introduces no fresh atoms, formula rewrites or owner relation. -/
def node : theory.Node → Zetesis.DagSharing.Node Nat
  | .Atom atom => .atom atom.val
  | .False => .bot
  | .And left right => .conj left.val right.val
  | .Or left right => .disj left.val right.val
  | .Implies left right => .imp left.val right.val

/-- The concrete one-node specification is exactly the existing Boolean DAG
layer after structural conversion. Both specifications totalize absent children
as false; actual bounded reads are justified separately by the step proof. -/
theorem node_value_exact (candidate : theory.Interpretation) (earlier : List Bool)
    (current : theory.Node) :
    Evaluation.value candidate earlier current =
      Zetesis.TightEvaluation.nodeValue (Membership.denotes candidate) earlier
        (node current) := by
  cases current <;> rfl

/-- An unmasked concrete fold equals the mathematical original-truth fold.
The induction identifies each appended layer and preserves the entire prefix. -/
theorem original_values (candidate : theory.Interpretation) (table : List theory.Node) :
    EvaluationSpecification.values candidate none table =
      Zetesis.TightEvaluation.values (Membership.denotes candidate) (table.map node) := by
  have foldExact (remaining : List theory.Node) (earlier : List Bool) :
      remaining.foldl (fun truths current => truths ++
        [EvaluationSpecification.maskedAt none truths.length
          (Evaluation.value candidate truths current)]) earlier =
      (remaining.map node).foldl (fun truths current => truths ++
        [Zetesis.TightEvaluation.nodeValue (Membership.denotes candidate) truths current])
        earlier := by
    induction remaining generalizing earlier with
    | nil => rfl
    | cons current rest inductionHypothesis =>
      simpa only [List.foldl_cons, List.map_cons, EvaluationSpecification.maskedAt,
        Option.map_none, Option.getD_none, Bool.and_true, node_value_exact] using
        inductionHypothesis (earlier ++ [Evaluation.value candidate earlier current])
  exact foldExact table []

/-- A frozen concrete pass equals the existing reduct fold when its supplied
mask is exactly the completed concrete original-value sequence. The original
fold theorem discharges the mathematical mask provenance; the second fold's
induction uses the same stored mask at every layer. No reduct-evaluator result
or subset relation between the interpretations is assumed. -/
theorem frozen_values (outer tested : theory.Interpretation) (frozen : Slice Bool)
    (table : List theory.Node)
    (computed : frozen.val = EvaluationSpecification.values outer none table) :
    EvaluationSpecification.values tested (some frozen) table =
      Zetesis.ReductEvaluation.values (Membership.denotes outer)
        (Membership.denotes tested) (table.map node) := by
  have maskExact : frozen.val =
      Zetesis.TightEvaluation.values (Membership.denotes outer) (table.map node) :=
    computed.trans (original_values outer table)
  have foldExact (remaining : List theory.Node) (earlier : List Bool) :
      remaining.foldl (fun truths current => truths ++
        [EvaluationSpecification.maskedAt (some frozen) truths.length
          (Evaluation.value tested truths current)]) earlier =
      (remaining.map node).foldl (fun truths current => truths ++
        [Zetesis.TightEvaluation.nodeValue (Membership.denotes tested) truths current &&
          frozen.val.getD truths.length false]) earlier := by
    induction remaining generalizing earlier with
    | nil => rfl
    | cons current rest inductionHypothesis =>
      simpa only [List.foldl_cons, List.map_cons, EvaluationSpecification.maskedAt,
        Option.map_some, Option.getD_some, node_value_exact, List.getD_eq_getElem?_getD] using
        inductionHypothesis (earlier ++
          [EvaluationSpecification.maskedAt (some frozen) earlier.length
            (Evaluation.value tested earlier current)])
  change EvaluationSpecification.values tested (some frozen) table =
    (table.map node).foldl (fun truths current => truths ++
      [Zetesis.TightEvaluation.nodeValue (Membership.denotes tested) truths current &&
        (Zetesis.TightEvaluation.values (Membership.denotes outer) (table.map node)).getD
          truths.length false]) []
  rw [← maskExact]
  exact foldExact table []

/-- Every value in the concrete frozen sequence is the truth of the corresponding
explicit Ferraris reduct. The mask is supplied by a concrete original fold;
`ReductEvaluation.values_correspond` supplies the existing general semantic law. -/
theorem frozen_values_correspond (outer tested : theory.Interpretation)
    (frozen : Slice Bool) (table : List theory.Node)
    (computed : frozen.val = EvaluationSpecification.values outer none table) :
    EvaluationSpecification.values tested (some frozen) table =
      (Zetesis.DagSharing.meanings (table.map node)).map (fun formula =>
        Zetesis.TightEvaluation.formulaValue (Membership.denotes tested)
          (Zetesis.Ferraris.Reduct
            (Zetesis.TightEvaluation.interpretation (Membership.denotes outer)) formula)) := by
  rw [frozen_values outer tested frozen table computed]
  exact Zetesis.ReductEvaluation.values_correspond _ _ _

/-- A concrete frozen-value lookup equals the truth of that stored formula's
explicit reduct. Unavailable positions have the common falsum meaning; a caller
connecting a successful extracted read must also provide its index bound. -/
theorem frozen_value_at (outer tested : theory.Interpretation) (frozen : Slice Bool)
    (table : List theory.Node) (index : Nat)
    (computed : frozen.val = EvaluationSpecification.values outer none table) :
    (EvaluationSpecification.values tested (some frozen) table).getD index false =
      Zetesis.TightEvaluation.formulaValue (Membership.denotes tested)
        (Zetesis.Ferraris.Reduct
          (Zetesis.TightEvaluation.interpretation (Membership.denotes outer))
          ((Zetesis.DagSharing.meanings (table.map node)).getD index .bot)) := by
  rw [frozen_values outer tested frozen table computed]
  exact Zetesis.ReductEvaluation.value_at _ _ _ _

end EvaluationSemantics
