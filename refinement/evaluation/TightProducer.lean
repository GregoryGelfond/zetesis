import TightProducerReads
import TightProducerSemantics
import AtomicChoice
import TightClassification
import Zetesis.TightEvaluation

open Aeneas Aeneas.Std Result
open ZetesisExtract
open Zetesis

/-!
# Original formulas of actual tight producers

A successful extracted single-root producer supplies an existing
`TightPlans.Producer` with the exact original formula, head, optional body and
ordinary/choice kind. Facts remain facts, and reversed choices retain their
original orientation. The completed classifier supplies body-grammar witnesses;
actual atomic-choice recognition supplies the head witnesses.

Skipped roots are falsum or arbitrary default negations. Unsupported heads and
bodies remain typed refusals, in the source's head-before-body order. These laws
concern one supplied stored root. Its membership in the asserted root list,
complete two-pass extraction, occurrence coverage, rank certificate and
answer-set check remain separate obligations.
-/
namespace TightProducer

/-- Ordinary and choice producers keep their existing semantic constructors;
the runtime tag forgets only fact/rule distinction and choice orientation. -/
def kind : TightPlans.Producer Nat → tight.TightProducerKind
  | .fact _ | .normal _ _ => .Normal
  | .choiceFact _ | .choice _ _ | .choiceFactReversed _ | .choiceReversed _ _ => .Choice

/-- An actual accepted head has exactly the formula specified by its runtime
kind. Successful stored reads supply their bounds, so a missing node's default
meaning cannot justify a choice or an ordinary atom. -/
theorem head_meaning (program : theory.Theory) (index atom : Usize)
    (tag : tight.TightProducerKind)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (recognized : TightProducerReads.Head program index atom tag) :
    (tag = .Normal ∧ TightDag.meaning program index = .atom atom.val) ∨
      (tag = .Choice ∧
        (TightDag.meaning program index = .disj (.atom atom.val) (Ferraris.Neg (.atom atom.val)) ∨
         TightDag.meaning program index = .disj (Ferraris.Neg (.atom atom.val)) (.atom atom.val))) := by
  rcases recognized with ⟨normal, read⟩ | ⟨choice, completed⟩
  · exact .inl ⟨normal, TightDag.stored_atom program index atom ordered read⟩
  · obtain ⟨left, right, read, _⟩ := (AtomicChoice.atom_shape_iff program index atom).mp completed
    have inside : index.val < program.value.nodes.val.length :=
      (List.getElem?_eq_some_iff.mp read).1
    exact .inr ⟨choice, (AtomicChoice.atom_meaning_iff program index atom ordered inside).mp completed⟩

/-- Every returned row retains the source root and has an exact original
producer witness with the same semantic head, indexed body and kind.

Proof: recover the actual source reads; distinguish a fact from an implication;
recover an explicit body from its completed class; and use the proved head
recognition for the ordinary or oriented choice constructor. No supplied
producer-correctness or candidate-truth equation is used. -/
theorem completed_producer (program : theory.Theory) (root : Usize)
    (classes : Slice tight.compile.Body) (output : tight.TightProducer)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (exactClasses : classes.val = TightClassificationSemantics.classes program.value.nodes.val)
    (completed : tight.compile.producer program root classes = ok (.Ok (some output))) :
    output.root = root ∧ ∃ rule : TightPlans.Producer Nat,
      rule.formula = TightDag.meaning program root ∧
      rule.head = output.head.val ∧
      rule.body.formula = TightEvaluation.bodyFormula
        (program.value.nodes.val.map EvaluationSemantics.node) (output.body.map UScalar.val) ∧
      kind rule = output.kind := by
  have receipt := TightProducerReads.completed_reads program root classes (.Ok (some output)) completed
  cases receipt with
  | produced _ head source parts recognized accepted =>
      refine ⟨source, ?_⟩
      have headFormula := head_meaning program head output.head output.kind ordered recognized
      rcases parts with ⟨noBody, sameHead⟩ | ⟨index, bodyAt, rootRead⟩
      · rcases headFormula with ⟨normal, formula⟩ | ⟨choice, forward | backward⟩
        · refine ⟨.fact output.head.val, ?_, rfl, ?_, normal.symm⟩
          · simpa [TightPlans.Producer.formula, sameHead] using formula.symm
          · simp [TightPlans.Producer.body, TightEvaluation.bodyFormula, noBody]
        · refine ⟨.choiceFact output.head.val, ?_, rfl, ?_, choice.symm⟩
          · simpa [TightPlans.Producer.formula, sameHead] using forward.symm
          · simp [TightPlans.Producer.body, TightEvaluation.bodyFormula, noBody]
        · refine ⟨.choiceFactReversed output.head.val, ?_, rfl, ?_, choice.symm⟩
          · simpa [TightPlans.Producer.formula, sameHead] using backward.symm
          · simp [TightPlans.Producer.body, TightEvaluation.bodyFormula, noBody]
      · obtain ⟨rootInside, entry⟩ := List.getElem?_eq_some_iff.mp rootRead
        have children : index.val < root.val ∧ head.val < root.val := by
          simpa [entry, EvaluationSpecification.ChildrenBefore] using ordered root.val rootInside
        have bodyInside : index.val < program.value.nodes.val.length := by omega
        have bodyAccepted : TightProducerReads.Body classes (some index) := bodyAt ▸ accepted
        obtain ⟨classification, classRead, notOpaque⟩ := bodyAccepted
        obtain ⟨body, bodyFormula⟩ := TightProducerSemantics.body_witness
          program classes index classification ordered bodyInside exactClasses classRead notOpaque
        have layer := TightDag.stored_implies program root index head ordered rootRead
        have indexedBody : body.formula = TightEvaluation.bodyFormula
            (program.value.nodes.val.map EvaluationSemantics.node) (output.body.map UScalar.val) := by
          simpa [TightEvaluation.bodyFormula, bodyAt, TightDag.meaning] using bodyFormula
        rcases headFormula with ⟨normal, formula⟩ | ⟨choice, forward | backward⟩
        · refine ⟨.normal body output.head.val, ?_, rfl, indexedBody, normal.symm⟩
          simp only [TightPlans.Producer.formula, bodyFormula, layer, formula]
        · refine ⟨.choice body output.head.val, ?_, rfl, indexedBody, choice.symm⟩
          simp only [TightPlans.Producer.formula, bodyFormula, layer, forward]
        · refine ⟨.choiceReversed body output.head.val, ?_, rfl, indexedBody, choice.symm⟩
          simp only [TightPlans.Producer.formula, bodyFormula, layer, backward]

/-- A skipped root is falsum or a default negation of its original
interior. No body-grammar restriction is imposed on that frozen interior. This
is the nonproducer alternative needed by complete root coverage. -/
theorem completed_none (program : theory.Theory) (root : Usize)
    (classes : Slice tight.compile.Body)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (completed : tight.compile.producer program root classes = ok (.Ok none)) :
    TightDag.meaning program root = .bot ∨
      ∃ interior, TightDag.meaning program root = Ferraris.Neg interior := by
  have receipt := TightProducerReads.completed_reads program root classes (.Ok none) completed
  cases receipt with
  | falsum read => exact .inl (TightDag.stored_false program root ordered read)
  | negation left right read consequent =>
      refine .inr ⟨TightDag.meaning program left, ?_⟩
      rw [TightDag.stored_implies program root left right ordered read,
        TightDag.stored_false program right ordered consequent]
      rfl

/-- Unsupported-root refusal preserves the queried root and a reached head
that is neither an ordinary atom nor either exact atomic-choice orientation.
The source reaches this refusal before inspecting any explicit body class. -/
theorem unsupported_root (program : theory.Theory) (root reported : Usize)
    (classes : Slice tight.compile.Body)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (completed : tight.compile.producer program root classes = ok (.Err (.UnsupportedRoot reported))) :
    reported = root ∧ ∃ body head,
      TightProducerReads.Parts program root body head ∧
      ∀ atom : Usize,
        TightDag.meaning program head ≠ .atom atom.val ∧
        TightDag.meaning program head ≠ .disj (.atom atom.val) (Ferraris.Neg (.atom atom.val)) ∧
        TightDag.meaning program head ≠ .disj (Ferraris.Neg (.atom atom.val)) (.atom atom.val) := by
  have receipt := TightProducerReads.completed_reads program root classes (.Err (.UnsupportedRoot reported)) completed
  cases receipt with
  | unsupportedHead body head node parts read notAtom declined =>
      refine ⟨rfl, body, head, parts, ?_⟩
      have inside : head.val < program.value.nodes.val.length := (List.getElem?_eq_some_iff.mp read).1
      intro atom
      have notChoice : ¬ (TightDag.meaning program head =
          .disj (.atom atom.val) (Ferraris.Neg (.atom atom.val)) ∨
          TightDag.meaning program head = .disj (Ferraris.Neg (.atom atom.val)) (.atom atom.val)) := by
        intro shape
        have recognized := (AtomicChoice.atom_meaning_iff program head atom ordered inside).mpr shape
        simp [declined] at recognized
      exact ⟨fun formula => notAtom atom
        ((AtomicChoice.meaning_atom_iff program head atom ordered inside).mp formula),
        fun formula => notChoice (.inl formula), fun formula => notChoice (.inr formula)⟩

/-- Unsupported-body refusal preserves the exact implication and a previously
recognized head, while the original body lies outside the structural grammar.
Thus an unsupported head is not replaced by a later body diagnostic. -/
theorem unsupported_body (program : theory.Theory) (root reported body : Usize)
    (classes : Slice tight.compile.Body)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (exactClasses : classes.val = TightClassificationSemantics.classes program.value.nodes.val)
    (completed : tight.compile.producer program root classes = ok (.Err (.UnsupportedBody reported body))) :
    reported = root ∧ ∃ head atom tag,
      program.value.nodes.val[root.val]? = some (.Implies body head) ∧
      TightProducerReads.Head program head atom tag ∧
      TightBodyRecognition.recognize (TightDag.meaning program body) = none := by
  have receipt := TightProducerReads.completed_reads program root classes (.Err (.UnsupportedBody reported body)) completed
  cases receipt with
  | unsupportedBody _ head atom tag parts recognized read =>
      rcases parts with ⟨impossible, _⟩ | ⟨index, sameBody, rootRead⟩
      · cases impossible
      · have same : index = body := by simpa using sameBody.symm
        subst index
        obtain ⟨rootInside, entry⟩ := List.getElem?_eq_some_iff.mp rootRead
        have children : body.val < root.val ∧ head.val < root.val := by
          simpa [entry, EvaluationSpecification.ChildrenBefore] using ordered root.val rootInside
        have bodyInside : body.val < program.value.nodes.val.length := by omega
        have classMeaning := TightProducerSemantics.class_meaning
          program classes body .Opaque ordered bodyInside exactClasses read
        refine ⟨rfl, head, atom, tag, rootRead, recognized, ?_⟩
        exact (TightClassificationSemantics.formulaClass_opaque _).mp classMeaning.symm

/-- A completed actual classification supplies the class-table premise of the
single-root theorem. Reservation preservation is only conditional on a
successful empty-vector reservation; allocation success is not assumed. -/
theorem after_classification [VectorReservation] (program : theory.Theory) (root : Usize)
    (before after : tight.Work) (classes : alloc.vec.Vec tight.compile.Body)
    (output : tight.TightProducer)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (budget : before.used.val ≤ before.max.val)
    (preservesEmpty : ∀ initial : alloc.vec.Vec tight.compile.Body,
      alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new tight.compile.Body)
        (Slice.len (alloc.vec.Vec.deref program.value.nodes)) = ok (.Ok (), initial) → initial.val = [])
    (classified : tight.compile.classify program before = ok (.Ok classes, after))
    (completed : tight.compile.producer program root (alloc.vec.Vec.deref classes) = ok (.Ok (some output))) :
    output.root = root ∧ ∃ rule : TightPlans.Producer Nat,
      rule.formula = TightDag.meaning program root ∧ rule.head = output.head.val ∧
      rule.body.formula = TightEvaluation.bodyFormula
        (program.value.nodes.val.map EvaluationSemantics.node) (output.body.map UScalar.val) ∧
      kind rule = output.kind := by
  have computed := TightClassification.completed_classes program before after classes
    ordered budget preservesEmpty classified
  exact completed_producer program root (alloc.vec.Vec.deref classes) output ordered
    (by simpa [alloc.vec.Vec.deref] using computed.1) completed

end TightProducer
