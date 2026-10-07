import Aeneas

open Aeneas Aeneas.Std Result

/-!
# Exact native machine-range steps

These specializations expose the existing Aeneas range model used by native
admission and evaluation. Success follows from a remaining coordinate, not from
an assumed increment; exhausted ranges perform no increment. No new iterator
implementation or extraction/library correspondence is assumed here.
-/
namespace NativeRange

/-- An in-range machine cursor advances by one without overflow. Its remaining
coordinate itself supplies room for the successor; no successful next is assumed. -/
theorem range_present (cursor : core.ops.range.Range Usize)
    (inside : cursor.start.val < cursor.end.val) :
    ∃ next : core.ops.range.Range Usize,
      core.iter.range.IteratorRange.next core.iter.range.StepUsize cursor =
        ok (some cursor.start, next) ∧
      next.start.val = cursor.start.val + 1 ∧ next.end = cursor.end := by
  have cloneExact : ∀ value : Usize, core.clone.CloneUsize.clone value = ok value := by
    intro value
    rfl
  have comparisonExact : ∀ first second : Usize,
      core.cmp.PartialOrdUsize.lt first second = ok (decide (first.val < second.val)) := by
    intro first second
    rfl
  obtain ⟨⟨value, next⟩, step, returned, increment, ending⟩ := WP.spec_imp_exists
    (core.iter.range.IteratorRange.next_UScalar_some_spec cloneExact comparisonExact cursor inside)
  exact ⟨next, by simpa only [returned] using step, increment, ending⟩

/-- An exhausted machine range returns no coordinate and keeps its cursor. -/
theorem range_exhausted (cursor : core.ops.range.Range Usize)
    (exhausted : cursor.end.val ≤ cursor.start.val) :
    core.iter.range.IteratorRange.next core.iter.range.StepUsize cursor = ok (none, cursor) := by
  have comparisonExact : ∀ first second : Usize,
      core.cmp.PartialOrdUsize.lt first second = ok (decide (first.val < second.val)) := by
    intro first second
    rfl
  obtain ⟨⟨value, next⟩, step, absent, unchanged⟩ := WP.spec_imp_exists
    (core.iter.range.IteratorRange.next_UScalar_none_spec
      (cloneInst := core.clone.CloneUsize) comparisonExact cursor exhausted)
  simpa only [absent, unchanged] using step

end NativeRange
