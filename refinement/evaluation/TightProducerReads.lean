import TightClassificationStep

open Aeneas Aeneas.Std Result
open ZetesisExtract
open Zetesis

/-!
# Reads made by the actual single-root producer

A typed return from the extracted producer has one of its reached source
branches: a skipped falsum or default negation, an accepted row, an unsupported
head, or an unsupported explicit body. The receipt retains exact node and class
reads, source root, and the actual atomic-choice call. It neither assumes the
recognizer's correctness nor treats an unavailable read as typed refusal.

This helper performs no allocation, work charge or control read. The surrounding
complete root scan has separate coverage, resource and publication obligations.
-/
namespace TightProducerReads

/-- Absence retains a fact's original head; an explicit body retains the
original implication and its two occurrence indices. -/
def Parts (program : theory.Theory) (root : Usize) (body : Option Usize)
    (head : Usize) : Prop :=
  (body = none ∧ head = root) ∨
    ∃ index : Usize, body = some index ∧
      program.value.nodes.val[root.val]? = some (.Implies index head)

/-- A normal head is read directly; a choice head is supplied by the actual
atomic-choice recognizer. Formula correctness is composed separately. -/
def Head (program : theory.Theory) (index atom : Usize)
    (kind : tight.TightProducerKind) : Prop :=
  (kind = .Normal ∧ program.value.nodes.val[index.val]? = some (.Atom atom)) ∨
    (kind = .Choice ∧ atomic_choice.atom program index = ok (some atom))

/-- A fact does not read the class table. An explicit body must read a
nonopaque class before a row is returned. -/
def Body (classes : Slice tight.compile.Body) : Option Usize → Prop
  | none => True
  | some index => ∃ classification : tight.compile.Body,
      classes.val[index.val]? = some classification ∧ classification ≠ .Opaque

/-- Every typed return retains its reached branch. Unsupported-head refusal
precedes the body-class read; unsupported-body refusal retains a recognized head.
Backend failure and divergence are not constructors of this receipt. -/
inductive Receipt (program : theory.Theory) (root : Usize)
    (classes : Slice tight.compile.Body) :
    core.result.Result (Option tight.TightProducer) tight.TightError → Prop where
  | falsum
      (read : program.value.nodes.val[root.val]? = some .False) :
      Receipt program root classes (.Ok none)
  | negation (left right : Usize)
      (read : program.value.nodes.val[root.val]? = some (.Implies left right))
      (consequent : program.value.nodes.val[right.val]? = some .False) :
      Receipt program root classes (.Ok none)
  | produced (output : tight.TightProducer) (head : Usize)
      (source : output.root = root)
      (parts : Parts program root output.body head)
      (recognized : Head program head output.head output.kind)
      (body : Body classes output.body) :
      Receipt program root classes (.Ok (some output))
  | unsupportedHead (body : Option Usize) (head : Usize) (node : theory.Node)
      (parts : Parts program root body head)
      (read : program.value.nodes.val[head.val]? = some node)
      (notAtom : ∀ atom : Usize,
        program.value.nodes.val[head.val]? ≠ some (.Atom atom))
      (declined : atomic_choice.atom program head = ok none) :
      Receipt program root classes (.Err (.UnsupportedRoot root))
  | unsupportedBody (body head atom : Usize) (kind : tight.TightProducerKind)
      (parts : Parts program root (some body) head)
      (recognized : Head program head atom kind)
      (read : classes.val[body.val]? = some .Opaque) :
      Receipt program root classes (.Err (.UnsupportedBody root body))

/-- Comparing a class with Opaque tests precisely that constructor. -/
theorem opaque_test (classification : tight.compile.Body) :
    tight.compile.Body.Insts.CoreCmpPartialEqBody.eq classification .Opaque =
      ok (match classification with | .Opaque => true | _ => false) := by
  cases classification <;>
    simp [tight.compile.Body.Insts.CoreCmpPartialEqBody.eq,
      tight.compile.Body.read_discriminant]

/-- Any typed return of the actual producer has its exact reached-source
receipt. The proof follows the stored root, then the head, then the body class;
only a declined head invokes the source's unsupported-root branch. -/
theorem completed_reads (program : theory.Theory) (root : Usize)
    (classes : Slice tight.compile.Body)
    (answer : core.result.Result (Option tight.TightProducer) tight.TightError)
    (completed : tight.compile.producer program root classes = ok answer) :
    Receipt program root classes answer := by
  cases rootRead : program.value.nodes.val[root.val]? with
  | none =>
      simp [tight.compile.producer, theory.Theory.nodes,
        alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
        Slice.index_usize, rootRead] at completed
  | some rootNode =>
      cases rootNode with
      | Atom atom =>
          simp [tight.compile.producer, theory.Theory.nodes,
            alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
            Slice.index_usize, rootRead] at completed
          subst answer
          exact .produced _ root rfl (.inl ⟨rfl, rfl⟩) (.inl ⟨rfl, rootRead⟩) trivial
      | False =>
          simp [tight.compile.producer, theory.Theory.nodes,
            alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
            Slice.index_usize, rootRead] at completed
          subst answer
          exact .falsum rootRead
      | And _ _ | Or _ _ =>
          have notAtom : ∀ atom : Usize,
              program.value.nodes.val[root.val]? ≠ some (.Atom atom) := by
            intro atom
            simp [rootRead]
          cases choice : atomic_choice.atom program root with
          | ret selected =>
              cases selected with
              | none =>
                  simp [tight.compile.producer, theory.Theory.nodes,
                    alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                    Slice.index_usize, rootRead, choice, core.option.Option.ok_or,
                    core.result.Result.Insts.CoreOpsTry.branch,
                    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
                    core.convert.FromSame.from] at completed
                  subst answer
                  exact .unsupportedHead none root _ (.inl ⟨rfl, rfl⟩) rootRead notAtom choice
              | some atom =>
                  simp [tight.compile.producer, theory.Theory.nodes,
                    alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                    Slice.index_usize, rootRead, choice, core.option.Option.ok_or,
                    core.result.Result.Insts.CoreOpsTry.branch] at completed
                  subst answer
                  exact .produced _ root rfl (.inl ⟨rfl, rfl⟩) (.inr ⟨rfl, choice⟩) trivial
          | vis effect continuation =>
              simp [tight.compile.producer, theory.Theory.nodes,
                alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                Slice.index_usize, rootRead, choice] at completed
          | div =>
              simp [tight.compile.producer, theory.Theory.nodes,
                alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                Slice.index_usize, rootRead, choice] at completed
      | Implies body head =>
          have parts : Parts program root (some body) head := .inr ⟨body, rfl, rootRead⟩
          cases headRead : program.value.nodes.val[head.val]? with
          | none =>
              simp [tight.compile.producer, theory.Theory.nodes,
                alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                Slice.index_usize, rootRead, headRead] at completed
          | some headNode =>
              cases headNode with
              | False =>
                  simp [tight.compile.producer, theory.Theory.nodes,
                    alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                    Slice.index_usize, rootRead, headRead,
                    TightClassificationStep.false_test] at completed
                  subst answer
                  exact .negation body head rootRead headRead
              | Atom atom =>
                  have recognized : Head program head atom .Normal := .inl ⟨rfl, headRead⟩
                  cases classRead : classes.val[body.val]? with
                  | none =>
                      simp [tight.compile.producer, theory.Theory.nodes,
                        alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                        Slice.index_usize, rootRead, headRead, classRead,
                        TightClassificationStep.false_test] at completed
                  | some classification =>
                      cases classification with
                      | Opaque =>
                          simp [tight.compile.producer, theory.Theory.nodes,
                            alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                            Slice.index_usize, rootRead, headRead, classRead,
                            TightClassificationStep.false_test, opaque_test] at completed
                          subst answer
                          exact .unsupportedBody body head atom .Normal parts recognized classRead
                      | Frozen | Positive =>
                          simp [tight.compile.producer, theory.Theory.nodes,
                            alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                            Slice.index_usize, rootRead, headRead, classRead,
                            TightClassificationStep.false_test, opaque_test] at completed
                          subst answer
                          exact .produced _ head rfl parts recognized ⟨_, classRead, by simp⟩
              | And _ _ | Or _ _ | Implies _ _ =>
                  have notAtom : ∀ atom : Usize,
                      program.value.nodes.val[head.val]? ≠ some (.Atom atom) := by
                    intro atom
                    simp [headRead]
                  cases choice : atomic_choice.atom program head with
                  | ret selected =>
                      cases selected with
                      | none =>
                          simp [tight.compile.producer, theory.Theory.nodes,
                            alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                            Slice.index_usize, rootRead, headRead, choice,
                            TightClassificationStep.false_test, core.option.Option.ok_or,
                            core.result.Result.Insts.CoreOpsTry.branch,
                            core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
                            core.convert.FromSame.from] at completed
                          subst answer
                          exact .unsupportedHead (some body) head _ parts headRead notAtom choice
                      | some atom =>
                          have recognized : Head program head atom .Choice := .inr ⟨rfl, choice⟩
                          cases classRead : classes.val[body.val]? with
                          | none =>
                              simp [tight.compile.producer, theory.Theory.nodes,
                                alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                                Slice.index_usize, rootRead, headRead, choice, classRead,
                                TightClassificationStep.false_test, core.option.Option.ok_or,
                                core.result.Result.Insts.CoreOpsTry.branch] at completed
                          | some classification =>
                              cases classification with
                              | Opaque =>
                                  simp [tight.compile.producer, theory.Theory.nodes,
                                    alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                                    Slice.index_usize, rootRead, headRead, choice, classRead,
                                    TightClassificationStep.false_test, core.option.Option.ok_or,
                                    core.result.Result.Insts.CoreOpsTry.branch, opaque_test] at completed
                                  subst answer
                                  exact .unsupportedBody body head atom .Choice parts recognized classRead
                              | Frozen | Positive =>
                                  simp [tight.compile.producer, theory.Theory.nodes,
                                    alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                                    Slice.index_usize, rootRead, headRead, choice, classRead,
                                    TightClassificationStep.false_test, core.option.Option.ok_or,
                                    core.result.Result.Insts.CoreOpsTry.branch, opaque_test] at completed
                                  subst answer
                                  exact .produced _ head rfl parts recognized ⟨_, classRead, by simp⟩
                  | vis effect continuation =>
                      simp [tight.compile.producer, theory.Theory.nodes,
                        alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                        Slice.index_usize, rootRead, headRead, choice,
                        TightClassificationStep.false_test] at completed
                  | div =>
                      simp [tight.compile.producer, theory.Theory.nodes,
                        alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                        Slice.index_usize, rootRead, headRead, choice,
                        TightClassificationStep.false_test] at completed

end TightProducerReads
