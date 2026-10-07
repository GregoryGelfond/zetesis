import Native.Funs

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract

/-!
# Checked native operand rows

These laws describe the actual generated `read_span`, `Node::view` and
`FormulaView::node` operations over paired node/operand storage. Every return is
identified without assuming successful admission. Inline pairs keep their two
ordered entries; wide rows keep exactly the selected ordered entries, including
duplicates. Short arity, span failure and absent node indices remain distinct.

The scalar, slice and array operations are the imported Aeneas models. Their
correspondence with Rust remains the existing library/extraction boundary. These
laws establish no child topology, atom bounds, allocator capacity, lifetime,
formula semantics, work receipt or runtime-effect correspondence. No legacy
binary evaluator proof is imported or used to qualify this native generation.
-/
namespace NativeRows

/-- The complete mathematical row selected by a span. This total specification
clips unavailable ranges; the generated getter publishes it only after the
separate arity and range checks proved below. It is not another runtime owner. -/
def row (arena : Slice Usize) (span : theory.OperandSpan) : Slice Usize :=
  Slice.from ((arena.val.drop span.start.val).take span.length.val) (by
    have stored : arena.val.length ≤ Usize.max := Slice.length_ineq arena
    simp only [List.length_take, List.length_drop]
    omega)

/-- Row contents retain the exact list order and multiplicity. -/
theorem row_contents (arena : Slice Usize) (span : theory.OperandSpan) :
    (row arena span).val = (arena.val.drop span.start.val).take span.length.val := by
  simp only [row, Slice.from_val]

/-- A fitting range contains its complete declared number of occurrences. -/
theorem row_length (arena : Slice Usize) (span : theory.OperandSpan)
    (fits : span.start.val + span.length.val ≤ arena.val.length) :
    (row arena span).val.length = span.length.val := by
  rw [row_contents]
  simp only [List.length_take, List.length_drop]
  omega

/-- With a correctly computed endpoint, the imported checked range operation
returns exactly the complete row if it fits, and absence otherwise. The proof
uses actual SliceIndex semantics; it does not assume the range getter agrees. -/
theorem range_at_sum (arena : Slice Usize) (span : theory.OperandSpan)
    (finish : Usize) (sum : finish.val = span.start.val + span.length.val) :
    core.slice.Slice.get (core.slice.index.SliceIndexRangeUsizeSlice Usize)
      arena { start := span.start, «end» := finish } =
      ok (if span.start.val + span.length.val ≤ arena.val.length
        then some (row arena span) else none) := by
  by_cases fits : span.start.val + span.length.val ≤ arena.val.length
  · simp [core.slice.Slice.get,
      core.slice.index.SliceIndexRangeUsizeSlice.get, UScalar.le_equiv,
      Slice.length, sum, fits, row, List.slice]
  · simp [core.slice.Slice.get,
      core.slice.index.SliceIndexRangeUsizeSlice.get, UScalar.le_equiv,
      Slice.length, sum, fits]

/-- Native wide storage requires at least three entries. Its range must fit the
supplied arena; the arena's own machine bound then also excludes endpoint
overflow. This verdict preserves the generated arity-before-range order. -/
def spanResult (arena : Slice Usize) (span : theory.OperandSpan) :
    core.result.Result (Slice Usize) theory.AdmissionError :=
  if span.length.val < 3 then .Err .Arity
  else if span.start.val + span.length.val ≤ arena.val.length
    then .Ok (row arena span) else .Err .Span

/-- The actual generated span read always returns this precise typed verdict.
Short arity wins even when the range would overflow. For longer rows, checked
addition either proves the endpoint or proves it cannot fit the bounded arena;
the imported checked slice operation then supplies the exact ordered contents. -/
theorem read_span_exact (arena : Slice Usize) (span : theory.OperandSpan) :
    theory.read_span arena span = ok (spanResult arena span) := by
  by_cases short : span.length.val < 3
  · simp [theory.read_span, spanResult, UScalar.lt_equiv, short]
  · have checked : match Usize.checked_add span.start span.length with
        | some finish => span.start.val + span.length.val ≤ Usize.max ∧
            finish.val = span.start.val + span.length.val ∧
            finish.bv = span.start.bv + span.length.bv
        | none => Usize.max < span.start.val + span.length.val :=
      Usize.checked_add_bv_spec span.start span.length
    cases added : Usize.checked_add span.start span.length with
    | none =>
      rw [added] at checked
      have outside : ¬ span.start.val + span.length.val ≤ arena.val.length := by
        have stored : arena.val.length ≤ Usize.max := Slice.length_ineq arena
        omega
      simp [theory.read_span, spanResult, UScalar.lt_equiv, short, added,
        outside, lift, core.option.Option.ok_or,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
    | some finish =>
      rw [added] at checked
      have sum : finish.val = span.start.val + span.length.val := checked.2.1
      have selected : core.slice.Slice.get
          (core.slice.index.SliceIndexRangeUsizeSlice Usize) arena
          { start := span.start, «end» := finish } =
          ok (if span.start.val + span.length.val ≤ arena.val.length
            then some (row arena span) else none) :=
        range_at_sum arena span finish sum
      by_cases fits : span.start.val + span.length.val ≤ arena.val.length
      · simp [theory.read_span, spanResult, UScalar.lt_equiv, short, added,
          lift, core.option.Option.ok_or, selected, fits,
          core.result.Result.Insts.CoreOpsTry.branch]
      · simp [theory.read_span, spanResult, UScalar.lt_equiv, short, added,
          lift, core.option.Option.ok_or, selected, fits,
          core.result.Result.Insts.CoreOpsTry.branch,
          core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- Successful generated reading is exactly valid wide arity, a fitting range,
and the complete selected row. No premise about an already admitted theory is
needed, and no operand is sorted or deduplicated. -/
theorem read_span_accepts_iff (arena : Slice Usize) (span : theory.OperandSpan)
    (result : Slice Usize) :
    theory.read_span arena span = ok (.Ok result) ↔
      3 ≤ span.length.val ∧ span.start.val + span.length.val ≤ arena.val.length ∧
        row arena span = result := by
  rw [read_span_exact]
  by_cases short : span.length.val < 3
  · have notWide : ¬ 3 ≤ span.length.val := by omega
    simp [spanResult, short, notWide]
  · have wide : 3 ≤ span.length.val := by omega
    by_cases fits : span.start.val + span.length.val ≤ arena.val.length
    · simp [spanResult, short, wide, fits]
    · simp [spanResult, short, wide, fits]

/-- A successful actual read has its exact ordered contents and declared length.
Repeated values remain separate list positions. -/
theorem read_span_contents (arena : Slice Usize) (span : theory.OperandSpan)
    (result : Slice Usize)
    (returned : theory.read_span arena span = ok (.Ok result)) :
    result.val = (arena.val.drop span.start.val).take span.length.val ∧
      result.val.length = span.length.val := by
  have accepted : 3 ≤ span.length.val ∧
      span.start.val + span.length.val ≤ arena.val.length ∧ row arena span = result :=
    (read_span_accepts_iff arena span result).mp returned
  rw [← accepted.2.2]
  exact ⟨row_contents arena span, row_length arena span accepted.2.1⟩

/-- Arity refusal precedes every range/overflow consideration. -/
theorem read_span_arity (arena : Slice Usize) (span : theory.OperandSpan)
    (short : span.length.val < 3) :
    theory.read_span arena span = ok (.Err .Arity) := by
  simp [read_span_exact, spanResult, short]

/-- A sufficiently wide but unavailable span returns Span. This includes a
checked endpoint overflow, since a Slice cannot exceed the machine bound. -/
theorem read_span_outside (arena : Slice Usize) (span : theory.OperandSpan)
    (wide : 3 ≤ span.length.val)
    (outside : arena.val.length < span.start.val + span.length.val) :
    theory.read_span arena span = ok (.Err .Span) := by
  have notShort : ¬ span.length.val < 3 := by omega
  have notFits : ¬ span.start.val + span.length.val ≤ arena.val.length := by omega
  simp [read_span_exact, spanResult, notShort, notFits]

/-- A stored node's exact logical view. Inline pair storage contributes the
entire pair; wide storage propagates the checked span verdict unchanged. This
specification does not unfold children or construct a second formula graph. -/
def storedView (arena : Slice Usize) : theory.Node →
    core.result.Result theory.NodeView theory.AdmissionError
  | .Atom atom => .Ok (.Atom atom)
  | .False => .Ok .False
  | .Implies left right => .Ok (.Implies left right)
  | .AndPair pair => .Ok (.And pair.to_slice)
  | .OrPair pair => .Ok (.Or pair.to_slice)
  | .AndSpan span => match spanResult arena span with
    | .Ok operands => .Ok (.And operands)
    | .Err reason => .Err reason
  | .OrSpan span => match spanResult arena span with
    | .Ok operands => .Ok (.Or operands)
    | .Err reason => .Err reason

/-- The generated Node view equals the complete native view for every raw node
and arena. Storage failures are typed outcomes, never fabricated child rows. -/
theorem node_view_exact (arena : Slice Usize) (node : theory.Node) :
    theory.Node.view node arena = ok (storedView arena node) := by
  cases node with
  | Atom atom => rfl
  | False => rfl
  | Implies left right => rfl
  | AndPair pair => simp [theory.Node.view, storedView, lift]
  | OrPair pair => simp [theory.Node.view, storedView, lift]
  | AndSpan span =>
    cases result : spanResult arena span <;>
      simp [theory.Node.view, read_span_exact, storedView, result,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]
  | OrSpan span =>
    cases result : spanResult arena span <;>
      simp [theory.Node.view, read_span_exact, storedView, result,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- Both inline connective variants expose the same exact two-entry row. This
includes repeated child IDs; the array model retains their occurrence positions. -/
theorem inline_pair_contents (pair : Aeneas.Std.Array Usize 2#usize) :
    pair.to_slice.val = pair.val ∧ pair.to_slice.val.length = 2 := by
  constructor
  · exact Array.val_to_slice pair
  · simpa only [Array.val_to_slice, UScalar.ofNatCore_val_eq] using Array.property pair

/-- A table lookup first checks node presence, then decodes that node using the
same paired arena. An absent index therefore returns Edge before any span check. -/
def tableResult (view : theory.FormulaView) (index : Usize) :
    core.result.Result theory.NodeView theory.AdmissionError :=
  match view.nodes.val[index.val]? with
  | none => .Err .Edge
  | some node => storedView view.operands node

/-- Every actual generated FormulaView lookup returns exactly the checked table
result. The proof uses the actual bounded node lookup and the Node-view law. -/
theorem formula_view_exact (view : theory.FormulaView) (index : Usize) :
    theory.FormulaView.node view index = ok (tableResult view index) := by
  cases present : view.nodes.val[index.val]? <;>
    simp [theory.FormulaView.node, tableResult, core.slice.Slice.get,
      core.slice.index.Usize.get,
      Slice.getElem?_Usize_eq, present, core.option.Option.ok_or,
      core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
      node_view_exact]

/-- A present table entry is decoded against the table's own immutable arena. -/
theorem formula_view_present (view : theory.FormulaView) (index : Usize)
    (node : theory.Node) (present : view.nodes.val[index.val]? = some node) :
    theory.FormulaView.node view index = theory.Node.view node view.operands := by
  simp [formula_view_exact, node_view_exact, tableResult, present]

/-- Missing node indices return Edge, independently of raw arena contents. -/
theorem formula_view_missing (view : theory.FormulaView) (index : Usize)
    (missing : view.nodes.val[index.val]? = none) :
    theory.FormulaView.node view index = ok (.Err .Edge) := by
  simp [formula_view_exact, tableResult, missing]

/-- Borrowing paired parts exposes both original sequences together; cached
occurrence metadata is neither checked nor used by this view operation. -/
theorem parts_view_exact (parts : theory.FormulaParts) :
    theory.FormulaParts.view parts = ok {
      nodes := alloc.vec.Vec.deref parts.nodes,
      operands := alloc.vec.Vec.deref parts.operands } := by
  rfl

end NativeRows
