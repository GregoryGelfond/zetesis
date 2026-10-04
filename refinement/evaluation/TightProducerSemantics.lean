import TightClassificationSemantics

open Aeneas Aeneas.Std Result
open ZetesisExtract
open Zetesis

/-!
# Formula witnesses for tight producer bodies

A nonopaque class from the complete classifier identifies an explicit body in
`TightPlans.Body`, with exactly the indexed original formula. No candidate truth
or classical rewriting is involved. These local representation laws are used by
the operational producer proof; they do not establish complete root coverage.
-/
namespace TightProducerSemantics

/-- A bounded read from the completed class table is the exact structural
class of that node's unfolded formula. -/
theorem class_meaning (program : theory.Theory) (classes : Slice tight.compile.Body)
    (index : Usize) (classification : tight.compile.Body)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (inside : index.val < program.value.nodes.val.length)
    (exactClasses : classes.val = TightClassificationSemantics.classes program.value.nodes.val)
    (read : classes.val[index.val]? = some classification) :
    classification = TightClassificationSemantics.formulaClass (TightDag.meaning program index) := by
  let meanings := DagSharing.meanings (program.value.nodes.val.map EvaluationSemantics.node)
  have inMeanings : index.val < meanings.length := by
    simpa [meanings, DagSharing.meanings_length] using inside
  have classesMeaning : classes.val = meanings.map TightClassificationSemantics.formulaClass :=
    exactClasses.trans (TightClassificationSemantics.classes_exact program.value.nodes.val ordered)
  show classification =
    TightClassificationSemantics.formulaClass (TightDag.meaning program index)
  rw [classesMeaning, List.getElem?_map, List.getElem?_eq_getElem inMeanings] at read
  simp only [Option.map_some, Option.some.injEq] at read
  change classification = TightClassificationSemantics.formulaClass
    (meanings.getD index.val .bot)
  rw [List.getD_eq_getElem meanings .bot inMeanings]
  exact read.symm

/-- A successful bounded read of a nonopaque completed class supplies an exact
body-grammar witness. The finite classifier correspondence first identifies the
formula's structural class, then the formula recognizer recovers its body. -/
theorem body_witness (program : theory.Theory) (classes : Slice tight.compile.Body)
    (index : Usize) (classification : tight.compile.Body)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (inside : index.val < program.value.nodes.val.length)
    (exactClasses : classes.val = TightClassificationSemantics.classes program.value.nodes.val)
    (read : classes.val[index.val]? = some classification)
    (accepted : classification ≠ .Opaque) :
    ∃ body : TightPlans.Body Nat, body.formula = TightDag.meaning program index := by
  have classAt := class_meaning program classes index classification ordered inside exactClasses read
  have recognized : TightBodyRecognition.recognize (TightDag.meaning program index) ≠ none := by
    intro absent
    have opaqueClass := (TightClassificationSemantics.formulaClass_opaque
      (TightDag.meaning program index)).mpr absent
    exact accepted (classAt.trans opaqueClass)
  cases found : TightBodyRecognition.recognize (TightDag.meaning program index) with
  | none => exact False.elim (recognized found)
  | some body =>
      exact ⟨body, TightBodyRecognition.recognized_formula _ body found⟩

end TightProducerSemantics
