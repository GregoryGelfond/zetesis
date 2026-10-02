import Aeneas

open Aeneas Aeneas.Std Result

/-!
# Exact slice enumeration steps

These laws unfold the restored Aeneas slice/enumerate primitives used by the
extracted evaluator. They do not define a replacement iterator or assume a
Rust-to-Lean output equation. The backend model and its scalar/storage contracts
remain the imported boundary. No cancellation or atomic-read premise is needed.
-/
namespace EvaluatorIteration

/-- An in-range slice step returns that precise element and advances only its
index. The bound justifies the dependent list read. -/
theorem slice_next_present {α : Type} (iter : core.slice.iter.Iter α)
    (inside : iter.i < iter.slice.val.length) :
    core.slice.iter.IteratorSliceIter.next iter =
      ok (some iter.slice.val[iter.i], { iter with i := iter.i + 1 }) := by
  simp [core.slice.iter.IteratorSliceIter.next, inside]
  rfl

/-- At or beyond the end, the slice iterator returns no element and keeps its
state; it performs no increment. -/
theorem slice_next_exhausted {α : Type} (iter : core.slice.iter.Iter α)
    (exhausted : iter.slice.val.length ≤ iter.i) :
    core.slice.iter.IteratorSliceIter.next iter = ok (none, iter) := by
  simp [core.slice.iter.IteratorSliceIter.next, Nat.not_lt.mpr exhausted]

/-- When enumeration and slice positions agree, a remaining element guarantees
room for the checked enumeration increment. The returned index is the old
position, the element is the actual slice element, and both positions advance
by one. This derives success from the slice's stored length bound. -/
theorem enumerate_next_present {α : Type}
    (iter : core.iter.adapters.enumerate.Enumerate (core.slice.iter.Iter α))
    (aligned : iter.count.val = iter.iter.i)
    (inside : iter.iter.i < iter.iter.slice.val.length) :
    ∃ count : Usize,
      count.val = iter.count.val + 1 ∧
      core.iter.adapters.enumerate.IteratorEnumerate.next
        (core.iter.traits.iterator.IteratorSliceIter α) iter =
        ok (some (iter.count, iter.iter.slice.val[iter.iter.i]),
          { iter := { iter.iter with i := iter.iter.i + 1 }, count := count }) := by
  have room : iter.count.val + (1#usize).val ≤ Usize.max := by
    have stored := Slice.length_ineq iter.iter.slice
    simp only [UScalar.ofNatCore_val_eq] at *
    omega
  obtain ⟨count, increment, count_value⟩ := WP.spec_imp_exists
    (Usize.add_spec (x := iter.count) (y := 1#usize) room)
  refine ⟨count, ?_, ?_⟩
  · simpa using count_value
  · simp [core.iter.adapters.enumerate.IteratorEnumerate.next,
      slice_next_present iter.iter inside, increment]

/-- An exhausted enumerated slice returns no element and leaves the complete
iterator unchanged, regardless of its current enumeration count. -/
theorem enumerate_next_exhausted {α : Type}
    (iter : core.iter.adapters.enumerate.Enumerate (core.slice.iter.Iter α))
    (exhausted : iter.iter.slice.val.length ≤ iter.iter.i) :
    core.iter.adapters.enumerate.IteratorEnumerate.next
      (core.iter.traits.iterator.IteratorSliceIter α) iter = ok (none, iter) := by
  simp [core.iter.adapters.enumerate.IteratorEnumerate.next,
    slice_next_exhausted iter.iter exhausted]

/-- A checked vector read inside the initialized prefix returns its precise
stored element. This is the actual generic indexing operation emitted by the
extractor, reduced to the backend's existing checked-index specification. -/
theorem vector_read {α : Type} (values : alloc.vec.Vec α) (index : Usize)
    (inside : index.val < values.val.length) :
    alloc.vec.Vec.index (core.slice.index.SliceIndexUsizeSlice α) values index =
      ok values.val[index.val] := by
  obtain ⟨value, read, exact_value⟩ := WP.spec_imp_exists
    (alloc.vec.Vec.index_usize_spec values index inside)
  rw [alloc.vec.Vec.index_slice_index, read, exact_value]

/-- Appending to a prefix with at least one representable remaining position
succeeds in the imported vector model and preserves all prior values. The
returned list is exactly the previous prefix followed by the new value. -/
theorem vector_append {α : Type} (values : alloc.vec.Vec α) (value : α)
    (room : values.val.length < Usize.max) :
    ∃ extended : alloc.vec.Vec α,
      alloc.vec.Vec.push values value = ok extended ∧
      extended.val = values.val ++ [value] := by
  exact WP.spec_imp_exists (alloc.vec.Vec.push_spec values value room)

end EvaluatorIteration
