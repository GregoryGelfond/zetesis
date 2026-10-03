import TightClassificationSemantics
import TightClassificationStep

open Aeneas Aeneas.Std Result
open ZetesisExtract
open Zetesis

/-!
# Exact atomic-choice recognition

The actual extracted recognizer accepts an atom together with its default
negation, in either disjunction order. Distinct atom-node occurrences may name
the same semantic atom. Successful reads establish the exact stored syntax;
ordered DAG decoding then gives formula equality without classical rewriting.

The operation performs no allocation, work charge or control read. Its callers
own their inspection charge. These laws do not establish producer coverage, rank
validation, a complete tight plan or source-grounding correctness.
-/
namespace AtomicChoice

/-- The four stored nodes read by one successful orientation. Occurrence
identity is retained for links; only the two atomic payloads must agree. -/
def PairShape (program : theory.Theory) (positive negative atom : Usize) : Prop :=
  program.value.nodes.val[positive.val]? = some (.Atom atom) ∧
    ∃ left right : Usize,
      program.value.nodes.val[negative.val]? = some (.Implies left right) ∧
      program.value.nodes.val[left.val]? = some (.Atom atom) ∧
      program.value.nodes.val[right.val]? = some .False

/-- Comparing with an atom tests the exact constructor and semantic atom index. -/
theorem atom_test (node : theory.Node) (atom : Usize) :
    theory.Node.Insts.CoreCmpPartialEqNode.eq node (.Atom atom) =
      ok (match node with | .Atom found => decide (found = atom) | _ => false) := by
  cases node <;>
    simp [theory.Node.Insts.CoreCmpPartialEqNode.eq, theory.Node.read_discriminant,
      core.cmp.impls.PartialEqUsize.eq, lift]

/-- A successful actual pair recognition is exactly its four stored-node
witnesses. Failure of an unchecked index cannot establish recognition. -/
theorem pair_shape_iff (program : theory.Theory) (positive negative atom : Usize) :
    atomic_choice.pair program positive negative = ok (some atom) ↔
      PairShape program positive negative atom := by
  cases positiveRead : program.value.nodes.val[positive.val]? with
  | none =>
    simp [atomic_choice.pair, theory.Theory.nodes,
      alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref, Slice.index_usize,
      positiveRead, PairShape]
  | some positiveNode =>
    cases positiveNode with
    | False | And _ _ | Or _ _ | Implies _ _ =>
      simp [atomic_choice.pair, theory.Theory.nodes,
        alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref, Slice.index_usize,
        positiveRead, PairShape]
    | Atom found =>
      cases negativeRead : program.value.nodes.val[negative.val]? with
      | none =>
        simp [atomic_choice.pair, theory.Theory.nodes,
          alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref, Slice.index_usize,
          positiveRead, negativeRead, PairShape]
      | some negativeNode =>
        cases negativeNode with
        | Atom _ | False | And _ _ | Or _ _ =>
          simp [atomic_choice.pair, theory.Theory.nodes,
            alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref, Slice.index_usize,
            positiveRead, negativeRead, PairShape]
        | Implies left right =>
          cases leftRead : program.value.nodes.val[left.val]? with
          | none =>
            simp [atomic_choice.pair, theory.Theory.nodes,
              alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref, Slice.index_usize,
              positiveRead, negativeRead, leftRead, PairShape]
          | some leftNode =>
            cases leftNode with
            | False | And _ _ | Or _ _ | Implies _ _ =>
              simp [atomic_choice.pair, theory.Theory.nodes,
                alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref, Slice.index_usize,
                positiveRead, negativeRead, leftRead, atom_test, core.bool.Bool.then_some,
                PairShape]
            | Atom repeated =>
              by_cases same : repeated = found
              · subst repeated
                cases rightRead : program.value.nodes.val[right.val]? with
                | none =>
                  simp [atomic_choice.pair, theory.Theory.nodes,
                    alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                    Slice.index_usize, positiveRead, negativeRead, leftRead, rightRead,
                    atom_test, PairShape]
                | some rightNode =>
                  cases rightNode <;>
                    simp [atomic_choice.pair, theory.Theory.nodes,
                      alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                      Slice.index_usize, positiveRead, negativeRead, leftRead, rightRead,
                      atom_test, TightClassificationStep.false_test, core.bool.Bool.then_some,
                      PairShape]
              · simp [atomic_choice.pair, theory.Theory.nodes,
                  alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
                  Slice.index_usize, positiveRead, negativeRead, leftRead,
                  atom_test, same, core.bool.Bool.then_some, PairShape]
                intro foundEqual repeatedEqual
                exact False.elim (same (repeatedEqual.trans foundEqual.symm))

/-- A witnessed pair makes its reverse decline immediately: the reverse's
positive position stores an implication, not an atom. No later read is needed. -/
theorem reversed_declines (program : theory.Theory) (positive negative atom : Usize)
    (shape : PairShape program positive negative atom) :
    atomic_choice.pair program negative positive = ok none := by
  obtain ⟨_, left, right, negativeRead, _, _⟩ := shape
  simp [atomic_choice.pair, theory.Theory.nodes,
    alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
    Slice.index_usize, negativeRead]

/-- Trying the second orientation only after the first declines recognizes
exactly the union of their syntax witnesses. Failure or divergence of a reached
read is not a successful recognition. -/
theorem either_pair_iff (program : theory.Theory) (left right atom : Usize) :
    (do
      let first ← atomic_choice.pair program left right
      core.option.Option.or_else
        atomic_choice.atom.closure.Insts.CoreOpsFunctionFnOnceTupleOptionUsize first
        (program, right, left)) = ok (some atom) ↔
      PairShape program left right atom ∨ PairShape program right left atom := by
  cases first : atomic_choice.pair program left right with
  | ret option =>
    cases option with
    | none =>
      rw [← pair_shape_iff program left right atom, ← pair_shape_iff program right left atom]
      simp [first, core.option.Option.or_else,
        atomic_choice.atom.closure.Insts.CoreOpsFunctionFnOnceTupleOptionUsize.call_once]
    | some found =>
      have shape : PairShape program left right found :=
        (pair_shape_iff program left right found).mp first
      have reverse := reversed_declines program left right found shape
      rw [← pair_shape_iff program left right atom, ← pair_shape_iff program right left atom]
      simp [first, reverse, core.option.Option.or_else]
  | vis effect continuation =>
    constructor
    · intro impossible
      simp only [bind_tc_vis, vis_not_ok] at impossible
    · intro shapes
      rcases shapes with forward | backward
      · have success := (pair_shape_iff program left right atom).mpr forward
        simp only [first, vis_not_ok] at success
      · have declined := reversed_declines program right left atom backward
        simp only [first, vis_not_ok] at declined
  | div =>
    constructor
    · intro impossible
      simp only [bind_tc_div, div_not_ok] at impossible
    · intro shapes
      rcases shapes with forward | backward
      · have success := (pair_shape_iff program left right atom).mpr forward
        simp only [first, div_not_ok] at success
      · have declined := reversed_declines program right left atom backward
        simp only [first, div_not_ok] at declined

/-- A disjunction of the two exact atomic-choice orientations in stored syntax. -/
def Shape (program : theory.Theory) (head atom : Usize) : Prop :=
  ∃ left right : Usize,
    program.value.nodes.val[head.val]? = some (.Or left right) ∧
      (PairShape program left right atom ∨ PairShape program right left atom)

/-- Successful actual head recognition is exactly the stored choice shape.
The proof recovers the head read and the reached orientation from execution;
there is no assumption that the recognizer agrees with its intended meaning. -/
theorem atom_shape_iff (program : theory.Theory) (head atom : Usize) :
    atomic_choice.atom program head = ok (some atom) ↔ Shape program head atom := by
  cases headRead : program.value.nodes.val[head.val]? with
  | none =>
    simp [atomic_choice.atom, theory.Theory.nodes,
      alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
      Slice.index_usize, headRead, Shape]
  | some headNode =>
    cases headNode with
    | Atom _ | False | And _ _ | Implies _ _ =>
      simp [atomic_choice.atom, theory.Theory.nodes,
        alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
        Slice.index_usize, headRead, Shape]
    | Or left right =>
      simpa [atomic_choice.atom, theory.Theory.nodes,
        alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, alloc.vec.Vec.deref,
        Slice.index_usize, headRead, Shape] using either_pair_iff program left right atom

/-- At a bounded position in an ordered DAG, an atomic unfolded formula comes
from an atom node with that exact payload. This inverse excludes lookup defaults
and does not identify classically equivalent formulas. -/
theorem meaning_atom_iff (program : theory.Theory) (index atom : Usize)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (inside : index.val < program.value.nodes.val.length) :
    TightDag.meaning program index = .atom atom.val ↔
      program.value.nodes.val[index.val]? = some (.Atom atom) := by
  have read := List.getElem?_eq_getElem inside
  have decoded := TightDag.stored_meaning program index
    program.value.nodes.val[index.val] ordered read
  rw [decoded, read]
  cases program.value.nodes.val[index.val] with
  | False | And _ _ | Or _ _ | Implies _ _ =>
      simp [EvaluationSemantics.node, DagSharing.decode]
  | Atom found =>
      simp only [EvaluationSemantics.node, DagSharing.decode,
        Ferraris.Formula.atom.injEq, Option.some.injEq, theory.Node.Atom.injEq]
      exact ⟨UScalar.eq_of_val_eq, congrArg UScalar.val⟩

/-- Exact default negation of an atom has an implication node with an atom
antecedent and a falsum consequent. Child bounds follow from the ordered DAG;
these are syntactic equalities, not truth simplifications. -/
theorem meaning_negation_iff (program : theory.Theory) (index atom : Usize)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (inside : index.val < program.value.nodes.val.length) :
    TightDag.meaning program index = Ferraris.Neg (.atom atom.val) ↔
      ∃ left right : Usize,
        program.value.nodes.val[index.val]? = some (.Implies left right) ∧
        program.value.nodes.val[left.val]? = some (.Atom atom) ∧
        program.value.nodes.val[right.val]? = some .False := by
  have read := List.getElem?_eq_getElem inside
  have decoded := TightDag.stored_meaning program index
    program.value.nodes.val[index.val] ordered read
  have before := ordered index.val inside
  cases entry : program.value.nodes.val[index.val] with
  | Atom _ | False | And _ _ | Or _ _ =>
      simp [entry, EvaluationSemantics.node, DagSharing.decode] at decoded
      simp [decoded, read, entry, Ferraris.Neg]
  | Implies left right =>
      have children : left.val < index.val ∧ right.val < index.val := by
        simpa [entry, EvaluationSpecification.ChildrenBefore] using before
      have leftInside : left.val < program.value.nodes.val.length := by omega
      have rightInside : right.val < program.value.nodes.val.length := by omega
      have layer : TightDag.meaning program index =
          .imp (TightDag.meaning program left) (TightDag.meaning program right) := by
        simpa [entry, TightDag.meaning, EvaluationSemantics.node, DagSharing.decode]
          using decoded
      have atomMeaning := meaning_atom_iff program left atom ordered leftInside
      have bottomMeaning := TightClassificationSemantics.meaning_bot_iff
        program.value.nodes.val ordered right.val rightInside
      rw [layer]
      simpa [read, entry, Ferraris.Neg, TightDag.meaning] using
        and_congr atomMeaning bottomMeaning

/-- Each actual pair succeeds exactly when its bounded children decode to an
atom and that atom's default negation. Admission supplies only structural order;
the read and equality tests are derived from the generated function. -/
theorem pair_meaning_iff (program : theory.Theory) (positive negative atom : Usize)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (positiveInside : positive.val < program.value.nodes.val.length)
    (negativeInside : negative.val < program.value.nodes.val.length) :
    atomic_choice.pair program positive negative = ok (some atom) ↔
      TightDag.meaning program positive = .atom atom.val ∧
      TightDag.meaning program negative = Ferraris.Neg (.atom atom.val) := by
  rw [pair_shape_iff]
  exact (and_congr (meaning_atom_iff program positive atom ordered positiveInside)
    (meaning_negation_iff program negative atom ordered negativeInside)).symm

/-- Successful actual atomic-choice recognition is equivalent to either exact
choice orientation at an admitted head position. The first step recovers the
stored disjunction from execution; ordered child links and exact DAG decoding
then establish both directions. Richer tautologies are outside this grammar. -/
theorem atom_meaning_iff (program : theory.Theory) (head atom : Usize)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (inside : head.val < program.value.nodes.val.length) :
    atomic_choice.atom program head = ok (some atom) ↔
      TightDag.meaning program head =
        .disj (.atom atom.val) (Ferraris.Neg (.atom atom.val)) ∨
      TightDag.meaning program head =
        .disj (Ferraris.Neg (.atom atom.val)) (.atom atom.val) := by
  have read := List.getElem?_eq_getElem inside
  have decoded := TightDag.stored_meaning program head
    program.value.nodes.val[head.val] ordered read
  have before := ordered head.val inside
  rw [atom_shape_iff]
  cases entry : program.value.nodes.val[head.val] with
  | Atom _ | False | And _ _ | Implies _ _ =>
      simp [entry, EvaluationSemantics.node, DagSharing.decode] at decoded
      simp [Shape, read, entry, decoded]
  | Or left right =>
      have children : left.val < head.val ∧ right.val < head.val := by
        simpa [entry, EvaluationSpecification.ChildrenBefore] using before
      have leftInside : left.val < program.value.nodes.val.length := by omega
      have rightInside : right.val < program.value.nodes.val.length := by omega
      have layer : TightDag.meaning program head =
          .disj (TightDag.meaning program left) (TightDag.meaning program right) := by
        simpa [entry, TightDag.meaning, EvaluationSemantics.node, DagSharing.decode]
          using decoded
      have forward := pair_meaning_iff program left right atom ordered leftInside rightInside
      have backward := pair_meaning_iff program right left atom ordered rightInside leftInside
      rw [pair_shape_iff] at forward backward
      simpa [Shape, read, entry, layer, and_comm] using or_congr forward backward

end AtomicChoice
