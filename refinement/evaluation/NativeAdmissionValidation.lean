import NativeBoundsValidation
import NativeRange

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisNativeExtract
open NativeStructure NativeBoundsValidation NativeRange

/-!
# Actual native node admission

The generated validator first decodes each raw row from its own operand arena,
then checks atoms and every child occurrence. The finite specification below
preserves that order and the first typed refusal. Its successful scan supplies
the shared NativeStructure premises without assuming prior admission.
-/
namespace NativeAdmissionValidation

/-- The inspected occurrences of an already decoded row, in their stored order.
This mathematical list describes inspection; it is not another runtime graph. -/
def children : theory.NodeView → List Usize
  | .Atom _ | .False => []
  | .Implies left right => [left, right]
  | .And row | .Or row => row.val

/-- The finite child check is exactly the common complete-row bound predicate. -/
theorem children_accepts_iff (position : Nat) (node : theory.NodeView) :
    bounds position .Edge (children node) = .Ok () ↔ ChildrenBefore position node := by
  rw [bounds_accepts_iff]
  cases node <;> simp [children, ChildrenBefore]

/-- The actual generated child validator returns the complete ordered scan.
The two native connective loops share the same occurrence contract. -/
theorem validate_children_exact (index : Usize) (node : theory.NodeView) :
    theory.validate_children index node =
      ok (bounds index.val .Edge (children node)) := by
  cases node with
  | Atom atom => rfl
  | False => rfl
  | Implies left right =>
    by_cases leftOutside : index.val ≤ left.val
    · have leftInside : ¬ left.val < index.val := Nat.not_lt.mpr leftOutside
      simp [theory.validate_children, children, bounds, UScalar.le_equiv,
        leftOutside, leftInside]
    · have leftInside : left.val < index.val := Nat.lt_of_not_ge leftOutside
      by_cases rightOutside : index.val ≤ right.val
      · have rightInside : ¬ right.val < index.val := Nat.not_lt.mpr rightOutside
        simp [theory.validate_children, children, bounds, UScalar.le_equiv,
          leftOutside, leftInside, rightOutside, rightInside]
      · have rightInside : right.val < index.val := Nat.lt_of_not_ge rightOutside
        simp [theory.validate_children, children, bounds, UScalar.le_equiv,
          leftOutside, leftInside, rightOutside, rightInside]
  | And row =>
    have started : SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter row =
        ok { slice := row, i := 0 } := rfl
    simpa only [theory.validate_children, started, bind_tc_ok, children, List.drop_zero] using
      children_loop0_exact index { slice := row, i := 0 } (Nat.zero_le _)
  | Or row =>
    have started : SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter row =
        ok { slice := row, i := 0 } := rfl
    simpa only [theory.validate_children, started, bind_tc_ok, children, List.drop_zero] using
      children_loop1_exact index { slice := row, i := 0 } (Nat.zero_le _)

/-- Check the row's atom universe and then its complete child sequence. -/
def viewCheck (atoms position : Nat) (node : theory.NodeView) :
    core.result.Result Unit theory.AdmissionError :=
  match node with
  | .Atom atom => if atom.val < atoms then .Ok () else .Err .Atom
  | _ => bounds position .Edge (children node)

/-- The generated one-row validator has the exact atom and child verdict. -/
theorem validate_node_exact (atoms index : Usize) (node : theory.NodeView) :
    theory.validate_node atoms index node = ok (viewCheck atoms.val index.val node) := by
  cases node with
  | Atom atom =>
    by_cases inside : atom.val < atoms.val <;>
      simp [theory.validate_node, theory.validate_children, viewCheck,
        UScalar.le_equiv, inside]
  | False => rfl
  | Implies left right => exact validate_children_exact index (.Implies left right)
  | And row => exact validate_children_exact index (.And row)
  | Or row => exact validate_children_exact index (.Or row)

/-- Row acceptance supplies both structural premises, with no atom or child
condition hidden in a successful-getter assumption. -/
theorem viewCheck_accepts_iff (atoms position : Nat) (node : theory.NodeView) :
    viewCheck atoms position node = .Ok () ↔
      ChildrenBefore position node ∧ AtomBound atoms node := by
  cases node with
  | Atom atom =>
    by_cases inside : atom.val < atoms <;>
      simp [viewCheck, ChildrenBefore, AtomBound, inside]
  | False => simp [viewCheck, children, bounds, ChildrenBefore, AtomBound]
  | Implies left right =>
    simpa only [viewCheck, AtomBound, and_true] using
      children_accepts_iff position (.Implies left right)
  | And row =>
    simpa only [viewCheck, AtomBound, and_true] using children_accepts_iff position (.And row)
  | Or row =>
    simpa only [viewCheck, AtomBound, and_true] using children_accepts_iff position (.Or row)

/-- A raw node is decoded before its atom or child checks. Getter refusal
therefore wins over an invalid child hidden behind an unavailable span. -/
def storedCheck (atoms position : Nat) (arena : Slice Usize) (node : theory.Node) :
    core.result.Result Unit theory.AdmissionError :=
  match NativeRows.storedView arena node with
  | .Err reason => .Err reason
  | .Ok view => viewCheck atoms position view

/-- Raw-node acceptance is precisely successful full decoding and valid atoms
and children in the shared structural predicate. -/
theorem storedCheck_accepts_iff (atoms position : Nat) (arena : Slice Usize)
    (node : theory.Node) :
    storedCheck atoms position arena node = .Ok () ↔ ValidNode atoms position arena node := by
  cases decoded : NativeRows.storedView arena node with
  | Err reason => simp [storedCheck, ValidNode, decoded]
  | Ok view => simp [storedCheck, ValidNode, decoded, viewCheck_accepts_iff]

/-- Inspect a suffix at its original offset, stopping at its first typed refusal. -/
def scan (atoms first : Nat) (arena : Slice Usize) :
    List theory.Node → core.result.Result Unit theory.AdmissionError
  | [] => .Ok ()
  | entry :: rest =>
    match storedCheck atoms first arena entry with
    | .Err reason => .Err reason
    | .Ok _ => scan atoms (first + 1) arena rest

/-- Complete scan acceptance is exactly validity at every stored position. -/
theorem scan_accepts_iff (atoms first : Nat) (arena : Slice Usize)
    (nodes : List theory.Node) :
    scan atoms first arena nodes = .Ok () ↔ ValidNodes atoms first arena nodes := by
  induction nodes generalizing first with
  | nil => simp only [scan, true_iff]; exact validNodes_nil atoms first arena
  | cons entry rest inductionHypothesis =>
    rw [validNodes_cons]
    cases checked : storedCheck atoms first arena entry with
    | Err reason =>
      have invalid : ¬ ValidNode atoms first arena entry := by
        rw [← storedCheck_accepts_iff, checked]
        simp
      simp [scan, checked, invalid]
    | Ok value =>
      cases value
      have valid : ValidNode atoms first arena entry :=
        (storedCheck_accepts_iff atoms first arena entry).mp checked
      simp only [scan, checked, valid, true_and, inductionHypothesis]

/-- A present generated body decodes the actual raw node at its cursor and
then validates that row. Getter failures and row failures both stop immediately. -/
theorem nodes_body_present (atoms : Usize) (nodes : Slice theory.Node)
    (arena : Slice Usize) (cursor next : core.ops.range.Range Usize)
    (advanced : core.iter.range.IteratorRange.next core.iter.range.StepUsize cursor =
      ok (some cursor.start, next))
    (inside : cursor.start.val < nodes.val.length) :
    theory.validate_nodes_loop.body atoms nodes arena cursor =
      match storedCheck atoms.val cursor.start.val arena nodes.val[cursor.start.val] with
      | .Ok _ => ok (.cont next)
      | .Err reason => ok (.done (.Err reason)) := by
  cases decoded : NativeRows.storedView arena nodes.val[cursor.start.val] with
  | Err reason =>
    simp [theory.validate_nodes_loop.body, advanced, NativeRows.formula_view_exact,
      NativeRows.tableResult, List.getElem?_eq_getElem inside, storedCheck, decoded,
      core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
  | Ok view =>
    cases checked : viewCheck atoms.val cursor.start.val view with
    | Ok value =>
      cases value
      simp [theory.validate_nodes_loop.body, advanced, NativeRows.formula_view_exact,
        NativeRows.tableResult, List.getElem?_eq_getElem inside, storedCheck, decoded,
        validate_node_exact, checked, core.result.Result.Insts.CoreOpsTry.branch]
    | Err reason =>
      simp [theory.validate_nodes_loop.body, advanced, NativeRows.formula_view_exact,
        NativeRows.tableResult, List.getElem?_eq_getElem inside, storedCheck, decoded,
        validate_node_exact, checked, core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- Exhaustion does not inspect another node or operand. -/
theorem nodes_body_exhausted (atoms : Usize) (nodes : Slice theory.Node)
    (arena : Slice Usize) (cursor : core.ops.range.Range Usize)
    (exhausted : cursor.end.val ≤ cursor.start.val) :
    theory.validate_nodes_loop.body atoms nodes arena cursor = ok (.done (.Ok ())) := by
  simp [theory.validate_nodes_loop.body, range_exhausted cursor exhausted]

/-- Unfold the actual generated node loop by one reached body. -/
theorem nodes_loop_unfold (atoms : Usize) (nodes : Slice theory.Node)
    (arena : Slice Usize) (cursor : core.ops.range.Range Usize) :
    theory.validate_nodes_loop cursor atoms nodes arena = (do
      let transition ← theory.validate_nodes_loop.body atoms nodes arena cursor
      match transition with
      | .done result => ok result
      | .cont next => theory.validate_nodes_loop next atoms nodes arena) := by
  rw [theory.validate_nodes_loop, loop]
  congr 1
  funext transition
  cases transition <;> rfl

/-- Every aligned native node scan terminates with the exact suffix verdict.
The range end is the table length; induction decreases the number of remaining
nodes. Every successful row inspection keeps the same paired operand arena. -/
theorem nodes_loop_exact (atoms : Usize) (nodes : Slice theory.Node)
    (arena : Slice Usize) (cursor : core.ops.range.Range Usize)
    (ending : cursor.end.val = nodes.val.length)
    (within : cursor.start.val ≤ nodes.val.length) :
    theory.validate_nodes_loop cursor atoms nodes arena =
      ok (scan atoms.val cursor.start.val arena (nodes.val.drop cursor.start.val)) := by
  have construct (remaining : Nat) :
      ∀ current : core.ops.range.Range Usize,
        current.end.val = nodes.val.length → current.start.val ≤ nodes.val.length →
        nodes.val.length - current.start.val = remaining →
        theory.validate_nodes_loop current atoms nodes arena =
          ok (scan atoms.val current.start.val arena (nodes.val.drop current.start.val)) := by
    induction remaining with
    | zero =>
      intro current endAt _ exhaustedCount
      have exhausted : nodes.val.length ≤ current.start.val := by omega
      rw [nodes_loop_unfold, nodes_body_exhausted atoms nodes arena current (by omega),
        List.drop_eq_nil_of_le exhausted]
      simp [scan]
    | succ remaining inductionHypothesis =>
      intro current endAt _ counted
      have inside : current.start.val < nodes.val.length := by omega
      obtain ⟨next, advanced, increment, sameEnd⟩ := range_present current (by omega)
      have suffix : nodes.val.drop current.start.val =
          nodes.val[current.start.val] :: nodes.val.drop (current.start.val + 1) :=
        List.drop_eq_getElem_cons inside
      rw [nodes_loop_unfold, nodes_body_present atoms nodes arena current next advanced inside,
        suffix, scan]
      cases checked : storedCheck atoms.val current.start.val arena nodes.val[current.start.val] with
      | Err reason => simp
      | Ok value =>
        cases value
        have continued : theory.validate_nodes_loop next atoms nodes arena =
            ok (scan atoms.val next.start.val arena (nodes.val.drop next.start.val)) :=
          inductionHypothesis next (by rw [sameEnd]; exact endAt)
            (by omega) (by omega)
        simpa only [bind_tc_ok, increment] using continued
  exact construct _ cursor ending within rfl

/-- The complete generated validator always returns the exact ordered raw-node
scan. Arity, span, atom and edge refusals preserve their first reached position. -/
theorem validate_nodes_exact (atoms : Usize) (view : theory.FormulaView) :
    theory.validate_nodes atoms view =
      ok (scan atoms.val 0 view.operands view.nodes.val) := by
  have completed : theory.validate_nodes_loop
      { start := 0#usize, «end» := Slice.len view.nodes } atoms view.nodes view.operands =
      ok (scan atoms.val 0 view.operands view.nodes.val) := by
    simpa only [UScalar.ofNatCore_val_eq, List.drop_zero] using
      nodes_loop_exact atoms view.nodes view.operands
        { start := 0#usize, «end» := Slice.len view.nodes } (by simp) (by simp)
  simpa only [theory.validate_nodes, theory.FormulaView.len, bind_tc_ok] using completed

/-- Actual complete acceptance supplies all decoded rows with their atom bounds
and backward children, exactly the common evaluator structural premise. -/
theorem validate_nodes_accepts_iff (atoms : Usize) (view : theory.FormulaView) :
    theory.validate_nodes atoms view = ok (.Ok ()) ↔ WellFormed atoms.val view := by
  rw [validate_nodes_exact, Result.ok.injEq, scan_accepts_iff]
  rfl

end NativeAdmissionValidation
