import Iteration
import Semantics
import Zetesis.TheoryAdmission

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisExtract
open Zetesis Zetesis.Refinement

/-!
# The generated admission validators

`Theory::new` refuses a proposed formula table before it allocates the shared
theory. After its dimension and padded-count checks it calls two private
validators. `validate_nodes` checks every node, in stored order, against the
atom universe and the nodes before it. `validate_roots` checks that every
asserted root names a stored node. Each stops at its first refusal.

This module proves that the generated validators compute exactly the authored
checks of `TheoryAdmission`: the node scan `scan` started at position zero, and
the root scan `rootScan`, which `TheoryAdmission.validate_phases` identifies as
the root clause of `validate`. The structural conversion `EvaluationSemantics.node`
reads extracted nodes and machine indices as natural numbers, and enumeration
positions are list positions. The equations hold for every represented slice,
with no premise: the generated functions always return, no enumeration step
overflows, and no validator reports `Allocation`. Acceptance therefore supplies
the evaluator's ordered-children and bounded-root premises, and a node refusal
reports the error of the first refused node.

The dimension checks, the checked padded-count addition, the transfer of the
vectors into `Data`, the `Arc` allocation and its owner, and `Interpretation::new`
remain outside this module. The Charon and Aeneas translations and the backend
slice and enumeration models remain trusted correspondence boundaries.
-/

namespace AdmissionValidation

/-- The generated refusal for each authored refusal kind. No validator reports
`Allocation`: storage failure is not a structural refusal. -/
def refusal : TheoryAdmission.Error → theory.AdmissionError
  | .limit => .Limit
  | .atom => .Atom
  | .edge => .Edge
  | .root => .Root

/-- An authored check's outcome in the generated result type. -/
def verdict : Except TheoryAdmission.Error Unit →
    core.result.Result Unit theory.AdmissionError
  | .ok _ => .Ok ()
  | .error reason => .Err (refusal reason)

/-- A converted verdict accepts exactly when the authored check accepts. -/
theorem verdict_accepts_iff (check : Except TheoryAdmission.Error Unit) :
    verdict check = .Ok () ↔ check = .ok () := by
  cases check <;> simp [verdict]

/-- A converted verdict refuses with a generated error exactly when the
authored check refuses with the corresponding authored error. -/
theorem verdict_refuses_iff (check : Except TheoryAdmission.Error Unit)
    (reason : theory.AdmissionError) :
    verdict check = .Err reason ↔
      ∃ authored, check = .error authored ∧ reason = refusal authored := by
  cases check with
  | ok value => simp [verdict]
  | error authored =>
    simp only [verdict, core.result.Result.Err.injEq, Except.error.injEq]
    exact ⟨fun same => ⟨authored, rfl, same.symm⟩,
      fun ⟨_, sameAuthored, sameReason⟩ => sameAuthored ▸ sameReason.symm⟩

/-- No authored refusal kind becomes an allocation refusal. -/
theorem refusal_ne_allocation (reason : TheoryAdmission.Error) :
    refusal reason ≠ .Allocation := by
  cases reason <;> simp [refusal]

/-! ## One node -/

/-- The generated single-node check computes the authored check at the same
position, after the structural conversion of the node. An atom must lie in the
universe, both children of a connective must precede the node, and falsum
always passes. -/
theorem node_exact (atoms index : Usize) (entry : theory.Node) :
    theory.validate_node atoms index entry =
      ok (verdict (TheoryAdmission.node atoms.val index.val
        (EvaluationSemantics.node entry))) := by
  cases entry with
  | Atom atom =>
    by_cases inside : atom.val < atoms.val <;>
      simp [theory.validate_node, TheoryAdmission.node, EvaluationSemantics.node,
        verdict, refusal, UScalar.le_equiv, inside]
  | False => simp [theory.validate_node, TheoryAdmission.node, EvaluationSemantics.node, verdict]
  | And left right | Or left right | Implies left right =>
    by_cases leftAfter : index.val ≤ left.val <;>
      by_cases rightAfter : index.val ≤ right.val <;>
      simp [theory.validate_node, TheoryAdmission.node, EvaluationSemantics.node,
        verdict, refusal, UScalar.le_equiv, ← Nat.not_le, leftAfter, rightAfter]

/-- Under the structural conversion, the authored edge condition is exactly the
evaluator's ordered-children condition at the same position. -/
theorem valid_node_iff (position : Nat) (entry : theory.Node) :
    DagSharing.ValidNode position (EvaluationSemantics.node entry) ↔
      EvaluationSpecification.ChildrenBefore position entry := by
  cases entry <;> rfl

/-- A node passes exactly when its children precede it and its atom lies in
the universe. -/
theorem node_accepts_iff (atoms index : Usize) (entry : theory.Node) :
    theory.validate_node atoms index entry = ok (.Ok ()) ↔
      EvaluationSpecification.ChildrenBefore index.val entry ∧
        TheoryAdmission.AtomBound atoms.val (EvaluationSemantics.node entry) := by
  rw [node_exact, Result.ok.injEq, verdict_accepts_iff, TheoryAdmission.node_exact,
    valid_node_iff]

/-! ## The node loop -/

/-- At or beyond the end of the stored nodes, the generated body finishes with
acceptance. -/
theorem nodes_body_exhausted (atoms : Usize) (cursor : Evaluation.Cursor)
    (exhausted : cursor.iter.slice.val.length ≤ cursor.iter.i) :
    theory.validate_nodes_loop.body atoms cursor = ok (.done (.Ok ())) := by
  simp [theory.validate_nodes_loop.body,
    EvaluatorIteration.enumerate_next_exhausted cursor exhausted]

/-- At an aligned position inside the stored nodes, the generated body checks
that node at its position. Acceptance continues with both positions advanced by
one; a refusal finishes with that node's error. -/
theorem nodes_body_present (atoms : Usize) (cursor : Evaluation.Cursor)
    (aligned : cursor.count.val = cursor.iter.i)
    (inside : cursor.iter.i < cursor.iter.slice.val.length) :
    ∃ next : Evaluation.Cursor,
      next.iter = { cursor.iter with i := cursor.iter.i + 1 } ∧
      next.count.val = cursor.iter.i + 1 ∧
      theory.validate_nodes_loop.body atoms cursor =
        match verdict (TheoryAdmission.node atoms.val cursor.iter.i
            (EvaluationSemantics.node cursor.iter.slice.val[cursor.iter.i])) with
        | .Ok () => ok (.cont next)
        | .Err reason => ok (.done (.Err reason)) := by
  obtain ⟨count, counted, stepped⟩ :=
    EvaluatorIteration.enumerate_next_present cursor aligned inside
  refine ⟨{ iter := { cursor.iter with i := cursor.iter.i + 1 }, count := count },
    rfl, by dsimp only; omega, ?_⟩
  have checked := node_exact atoms cursor.count cursor.iter.slice.val[cursor.iter.i]
  rw [aligned] at checked
  cases verdictCase : verdict (TheoryAdmission.node atoms.val cursor.iter.i
      (EvaluationSemantics.node cursor.iter.slice.val[cursor.iter.i])) with
  | Ok value =>
    cases value
    simp [theory.validate_nodes_loop.body, stepped, checked, verdictCase,
      core.result.Result.Insts.CoreOpsTry.branch]
  | Err reason =>
    simp [theory.validate_nodes_loop.body, stepped, checked, verdictCase,
      core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- One unfolding of the generated loop runs its actual body once. -/
theorem nodes_loop_unfold (atoms : Usize) (cursor : Evaluation.Cursor) :
    theory.validate_nodes_loop cursor atoms = (do
      let transition ← theory.validate_nodes_loop.body atoms cursor
      match transition with
      | .done result => ok result
      | .cont next => theory.validate_nodes_loop next atoms) := by
  rw [theory.validate_nodes_loop, loop]
  congr 1
  funext transition
  cases transition <;> rfl

/-- From an aligned cursor, the generated loop returns the authored scan's
verdict on the remaining nodes, starting at the cursor's position. It always
returns: every continuation advances the position, and the slice length bounds
the enumeration count.

Proof: induct on the number of remaining nodes. With none left, the body
accepts and the scan of the empty suffix accepts. Otherwise the body checks the
node at the cursor exactly as the scan checks the suffix's head; a refusal ends
both, and an acceptance continues both one position later. -/
theorem nodes_loop_exact (atoms : Usize) (cursor : Evaluation.Cursor)
    (aligned : cursor.count.val = cursor.iter.i)
    (within : cursor.iter.i ≤ cursor.iter.slice.val.length) :
    theory.validate_nodes_loop cursor atoms =
      ok (verdict (TheoryAdmission.scan atoms.val cursor.iter.i
        ((cursor.iter.slice.val.drop cursor.iter.i).map EvaluationSemantics.node))) := by
  have construct (remaining : Nat) :
      ∀ current : Evaluation.Cursor,
        current.count.val = current.iter.i →
        current.iter.i ≤ current.iter.slice.val.length →
        current.iter.slice.val.length - current.iter.i = remaining →
        theory.validate_nodes_loop current atoms =
          ok (verdict (TheoryAdmission.scan atoms.val current.iter.i
            ((current.iter.slice.val.drop current.iter.i).map EvaluationSemantics.node))) := by
    induction remaining with
    | zero =>
      intro current _ bounded exhaustedCount
      have exhausted : current.iter.slice.val.length ≤ current.iter.i := by omega
      have emptySuffix : current.iter.slice.val.drop current.iter.i = [] :=
        List.drop_eq_nil_of_le exhausted
      rw [nodes_loop_unfold, nodes_body_exhausted atoms current exhausted, emptySuffix]
      simp [TheoryAdmission.scan, verdict]
    | succ remaining inductionHypothesis =>
      intro current currentAligned bounded counted
      have inside : current.iter.i < current.iter.slice.val.length := by omega
      obtain ⟨next, nextIter, nextCount, stepped⟩ :=
        nodes_body_present atoms current currentAligned inside
      have suffix : current.iter.slice.val.drop current.iter.i =
          current.iter.slice.val[current.iter.i] ::
            current.iter.slice.val.drop (current.iter.i + 1) :=
        List.drop_eq_getElem_cons inside
      have scanned : TheoryAdmission.scan atoms.val current.iter.i
          ((current.iter.slice.val.drop current.iter.i).map EvaluationSemantics.node) =
          (TheoryAdmission.node atoms.val current.iter.i
            (EvaluationSemantics.node current.iter.slice.val[current.iter.i])).bind
            (fun _ => TheoryAdmission.scan atoms.val (current.iter.i + 1)
              ((current.iter.slice.val.drop (current.iter.i + 1)).map
                EvaluationSemantics.node)) := by
        rw [suffix]
        rfl
      rw [nodes_loop_unfold, stepped, scanned]
      cases checked : TheoryAdmission.node atoms.val current.iter.i
          (EvaluationSemantics.node current.iter.slice.val[current.iter.i]) with
      | error authored =>
        simp [verdict, Except.bind]
      | ok value =>
        cases value
        have nextAligned : next.count.val = next.iter.i := by
          rw [nextCount, nextIter]
        have nextWithin : next.iter.i ≤ next.iter.slice.val.length := by
          rw [nextIter]
          exact inside
        have nextRemaining : next.iter.slice.val.length - next.iter.i = remaining := by
          rw [nextIter]
          dsimp only
          omega
        have continued := inductionHypothesis next nextAligned nextWithin nextRemaining
        rw [nextIter] at continued
        simpa [verdict, Except.bind] using continued
  exact construct _ cursor aligned within rfl

/-- The generated node validator returns the authored scan's verdict on the
whole stored sequence, from position zero, for every represented slice. -/
theorem validate_nodes_exact (atoms : Usize) (nodes : Slice theory.Node) :
    theory.validate_nodes atoms nodes =
      ok (verdict (TheoryAdmission.scan atoms.val 0
        (nodes.val.map EvaluationSemantics.node))) := by
  have started : core.slice.Slice.iter nodes = ok { slice := nodes, i := 0 } := rfl
  have enumerated : core.iter.traits.iterator.Iterator.enumerate.trait_default
      (core.iter.traits.iterator.IteratorSliceIter theory.Node) { slice := nodes, i := 0 } =
      ok { iter := { slice := nodes, i := 0 }, count := 0#usize } := rfl
  simp only [theory.validate_nodes, started, enumerated, bind_tc_ok]
  simpa using nodes_loop_exact atoms { iter := { slice := nodes, i := 0 }, count := 0#usize }
    rfl (Nat.zero_le _)

/-- Node validation accepts exactly a topologically ordered table whose atoms
lie in the universe. -/
theorem validate_nodes_accepts_iff (atoms : Usize) (nodes : Slice theory.Node) :
    theory.validate_nodes atoms nodes = ok (.Ok ()) ↔
      DagSharing.WellFormed (nodes.val.map EvaluationSemantics.node) ∧
        ∀ entry ∈ nodes.val,
          TheoryAdmission.AtomBound atoms.val (EvaluationSemantics.node entry) := by
  rw [validate_nodes_exact, Result.ok.injEq, verdict_accepts_iff, TheoryAdmission.scan_exact]
  simp only [List.forall_mem_map]

/-- A refusal reports the first refused node in stored order: every earlier
node passes its check at its position, and the reported error is that node's
own. -/
theorem validate_nodes_refuses_iff (atoms : Usize) (nodes : Slice theory.Node)
    (reason : theory.AdmissionError) :
    theory.validate_nodes atoms nodes = ok (.Err reason) ↔
      ∃ position, ∃ inside : position < nodes.val.length,
        (∀ earlier, ∀ before : earlier < position,
          TheoryAdmission.node atoms.val earlier
            (EvaluationSemantics.node nodes.val[earlier]) = .ok ()) ∧
        ∃ authored, TheoryAdmission.node atoms.val position
            (EvaluationSemantics.node nodes.val[position]) = .error authored ∧
          reason = refusal authored := by
  rw [validate_nodes_exact, Result.ok.injEq, verdict_refuses_iff]
  constructor
  · rintro ⟨authored, refused, sameReason⟩
    obtain ⟨position, inside, earlierPass, refusedAt⟩ :=
      (TheoryAdmission.scan_refusal_exact _ _ _ _).mp refused
    refine ⟨position, by simpa using inside, ?_, authored, ?_, sameReason⟩
    · intro earlier before
      simpa using earlierPass earlier before
    · simpa using refusedAt
  · rintro ⟨position, inside, earlierPass, authored, refusedAt, sameReason⟩
    refine ⟨authored, (TheoryAdmission.scan_refusal_exact _ _ _ _).mpr
      ⟨position, by simpa using inside, ?_, by simpa using refusedAt⟩, sameReason⟩
    intro earlier before
    simpa using earlierPass earlier before

/-- Accepted node validation supplies the evaluator's ordered-children premise:
every connective reads only nodes stored before it. -/
theorem accepted_ordered (atoms : Usize) (nodes : Slice theory.Node)
    (accepted : theory.validate_nodes atoms nodes = ok (.Ok ())) :
    EvaluationSpecification.Ordered nodes.val := by
  intro index inside
  have wellFormed : DagSharing.WellFormed (nodes.val.map EvaluationSemantics.node) :=
    ((validate_nodes_accepts_iff atoms nodes).mp accepted).1
  have reference := DagSharing.well_formed_reference wellFormed index (by simpa using inside)
  have stored : (nodes.val.map EvaluationSemantics.node).getD index .bot =
      EvaluationSemantics.node nodes.val[index] := by
    simp [List.getD_eq_getElem?_getD, inside]
  rw [stored] at reference
  exact (valid_node_iff index nodes.val[index]).mp reference

/-- Node validation never reports an allocation refusal. -/
theorem validate_nodes_never_allocation (atoms : Usize) (nodes : Slice theory.Node) :
    theory.validate_nodes atoms nodes ≠ ok (.Err .Allocation) := by
  intro reported
  obtain ⟨_, _, _, authored, _, sameReason⟩ :=
    (validate_nodes_refuses_iff atoms nodes .Allocation).mp reported
  exact refusal_ne_allocation authored sameReason.symm

/-! ## Roots -/

/-- The generated root check computes the authored root check: it accepts
exactly a root that names a stored node. -/
theorem root_exact (count root : Usize) :
    theory.validate_root count root =
      ok (verdict (TheoryAdmission.root count.val root.val)) := by
  by_cases inside : root.val < count.val <;>
    simp [theory.validate_root, TheoryAdmission.root, verdict, refusal,
      UScalar.le_equiv, inside]

/-- At or beyond the end of the asserted roots, the generated body finishes
with acceptance. -/
theorem roots_body_exhausted (count : Usize) (cursor : core.slice.iter.Iter Usize)
    (exhausted : cursor.slice.val.length ≤ cursor.i) :
    theory.validate_roots_loop.body count cursor = ok (.done (.Ok ())) := by
  simp [theory.validate_roots_loop.body,
    EvaluatorIteration.slice_next_exhausted cursor exhausted]

/-- Inside the asserted roots, the generated body checks the root at the
cursor. A stored node continues one position later; any other identifier
finishes with a root refusal. -/
theorem roots_body_present (count : Usize) (cursor : core.slice.iter.Iter Usize)
    (inside : cursor.i < cursor.slice.val.length) :
    theory.validate_roots_loop.body count cursor =
      if cursor.slice.val[cursor.i].val < count.val then
        ok (.cont { cursor with i := cursor.i + 1 })
      else ok (.done (.Err .Root)) := by
  by_cases stored : cursor.slice.val[cursor.i].val < count.val <;>
    simp [theory.validate_roots_loop.body,
      EvaluatorIteration.slice_next_present cursor inside, root_exact, stored,
      TheoryAdmission.root, verdict, refusal,
      core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- One unfolding of the generated loop runs its actual body once. -/
theorem roots_loop_unfold (count : Usize) (cursor : core.slice.iter.Iter Usize) :
    theory.validate_roots_loop cursor count = (do
      let transition ← theory.validate_roots_loop.body count cursor
      match transition with
      | .done result => ok result
      | .cont next => theory.validate_roots_loop next count) := by
  rw [theory.validate_roots_loop, loop]
  congr 1
  funext transition
  cases transition <;> rfl

/-- From any cursor position, the generated loop returns the authored root
scan's verdict on the remaining roots, in stored order.

Proof: induct on the number of remaining roots, as for nodes. The body checks
the root at the cursor exactly as the root scan checks the suffix's head. -/
theorem roots_loop_exact (count : Usize) (cursor : core.slice.iter.Iter Usize)
    (within : cursor.i ≤ cursor.slice.val.length) :
    theory.validate_roots_loop cursor count =
      ok (verdict (TheoryAdmission.rootScan count.val
        ((cursor.slice.val.drop cursor.i).map UScalar.val))) := by
  have construct (remaining : Nat) :
      ∀ current : core.slice.iter.Iter Usize,
        current.i ≤ current.slice.val.length →
        current.slice.val.length - current.i = remaining →
        theory.validate_roots_loop current count =
          ok (verdict (TheoryAdmission.rootScan count.val
            ((current.slice.val.drop current.i).map UScalar.val))) := by
    induction remaining with
    | zero =>
      intro current bounded exhaustedCount
      have exhausted : current.slice.val.length ≤ current.i := by omega
      rw [roots_loop_unfold, roots_body_exhausted count current exhausted,
        List.drop_eq_nil_of_le exhausted]
      simp [TheoryAdmission.rootScan, verdict]
    | succ remaining inductionHypothesis =>
      intro current bounded counted
      have inside : current.i < current.slice.val.length := by omega
      have suffix : current.slice.val.drop current.i =
          current.slice.val[current.i] :: current.slice.val.drop (current.i + 1) :=
        List.drop_eq_getElem_cons inside
      have scanned : TheoryAdmission.rootScan count.val
          ((current.slice.val.drop current.i).map UScalar.val) =
          (TheoryAdmission.root count.val current.slice.val[current.i].val).bind
            (fun _ => TheoryAdmission.rootScan count.val
              ((current.slice.val.drop (current.i + 1)).map UScalar.val)) := by
        rw [suffix]
        rfl
      rw [roots_loop_unfold, roots_body_present count current inside, scanned]
      by_cases stored : current.slice.val[current.i].val < count.val
      · have continued := inductionHypothesis { current with i := current.i + 1 }
          (by dsimp only; omega) (by dsimp only; omega)
        simpa [stored, TheoryAdmission.root, Except.bind] using continued
      · simp [stored, TheoryAdmission.root, verdict, refusal, Except.bind]
  exact construct _ cursor within rfl

/-- The generated root validator returns the authored root scan's verdict on
the asserted roots, read over machine identifiers. `TheoryAdmission.validate_phases`
identifies this scan as the root clause of `TheoryAdmission.validate`. -/
theorem validate_roots_exact (count : Usize) (roots : Slice Usize) :
    theory.validate_roots count roots =
      ok (verdict (TheoryAdmission.rootScan count.val (roots.val.map UScalar.val))) := by
  have started : SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter roots =
      ok { slice := roots, i := 0 } := rfl
  simp only [theory.validate_roots, started, bind_tc_ok]
  simpa using roots_loop_exact count { slice := roots, i := 0 } (Nat.zero_le _)

/-- Root validation accepts exactly when every asserted root names a stored
node. Empty roots are accepted. -/
theorem validate_roots_accepts_iff (count : Usize) (roots : Slice Usize) :
    theory.validate_roots count roots = ok (.Ok ()) ↔
      ∀ root ∈ roots.val, root.val < count.val := by
  rw [validate_roots_exact, Result.ok.injEq, verdict_accepts_iff,
    TheoryAdmission.rootScan_exact, List.forall_mem_map]

/-- Root validation refuses only with a root refusal, and exactly when some
asserted root names no stored node. -/
theorem validate_roots_refuses_iff (count : Usize) (roots : Slice Usize)
    (reason : theory.AdmissionError) :
    theory.validate_roots count roots = ok (.Err reason) ↔
      reason = .Root ∧ ∃ root ∈ roots.val, count.val ≤ root.val := by
  rw [validate_roots_exact, Result.ok.injEq, verdict_refuses_iff]
  constructor
  · rintro ⟨authored, refused, sameReason⟩
    obtain ⟨rootReason, unstored, member, outside⟩ :=
      (TheoryAdmission.rootScan_refusal_exact _ _ _).mp refused
    obtain ⟨root, rootMember, sameValue⟩ := List.mem_map.mp member
    refine ⟨by rw [sameReason, rootReason]; rfl, root, rootMember, ?_⟩
    omega
  · rintro ⟨sameReason, root, member, outside⟩
    refine ⟨.root, (TheoryAdmission.rootScan_refusal_exact _ _ _).mpr
      ⟨rfl, root.val, List.mem_map_of_mem member, outside⟩, by rw [sameReason]; rfl⟩

/-- Root validation never reports an allocation refusal. -/
theorem validate_roots_never_allocation (count : Usize) (roots : Slice Usize) :
    theory.validate_roots count roots ≠ ok (.Err .Allocation) := by
  intro reported
  have rootReason := ((validate_roots_refuses_iff count roots .Allocation).mp reported).1
  cases rootReason

/-! ## The constructor's calls -/

/-- Accepted node and root validation, applied as `Theory::new` applies them to
its input vectors, supplies the evaluator's structural premises for those
vectors: ordered children, bounded roots and atoms inside the universe. When the
vectors are a theory's stored data, the first two are the `ordered` and
`rootsBounded` premises of `CountermodelSemantics.FrozenEvaluation`. -/
theorem accepted_structure (atoms : Usize) (nodes : alloc.vec.Vec theory.Node)
    (roots : alloc.vec.Vec Usize)
    (nodesAccepted : theory.validate_nodes atoms (alloc.vec.Vec.deref nodes) = ok (.Ok ()))
    (rootsAccepted : theory.validate_roots (alloc.vec.Vec.len nodes)
      (alloc.vec.Vec.deref roots) = ok (.Ok ())) :
    EvaluationSpecification.Ordered nodes.val ∧
      (∀ root ∈ roots.val, root.val < nodes.val.length) ∧
      ∀ entry ∈ nodes.val,
        TheoryAdmission.AtomBound atoms.val (EvaluationSemantics.node entry) := by
  have sameNodes : (alloc.vec.Vec.deref nodes).val = nodes.val := by
    simp [alloc.vec.Vec.deref]
  have ordered : EvaluationSpecification.Ordered nodes.val := by
    rw [← sameNodes]
    exact accepted_ordered atoms (alloc.vec.Vec.deref nodes) nodesAccepted
  have atomsInside : ∀ entry ∈ nodes.val,
      TheoryAdmission.AtomBound atoms.val (EvaluationSemantics.node entry) := by
    rw [← sameNodes]
    exact ((validate_nodes_accepts_iff atoms (alloc.vec.Vec.deref nodes)).mp nodesAccepted).2
  have rootsInside : ∀ root ∈ roots.val, root.val < nodes.val.length := by
    have bounded := (validate_roots_accepts_iff (alloc.vec.Vec.len nodes)
      (alloc.vec.Vec.deref roots)).mp rootsAccepted
    simpa [alloc.vec.Vec.deref, alloc.vec.Vec.len] using bounded
  exact ⟨ordered, rootsInside, atomsInside⟩

end AdmissionValidation
