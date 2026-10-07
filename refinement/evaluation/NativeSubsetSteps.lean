import NativeControl
import Iteration
import Aeneas.Std.RangeIter

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisNativeExtract

/-!
# Exhaustion and refusal in the generated subset helpers

These laws unfold the unchanged generated selection and carry bodies. They
establish their empty and interrupted boundaries without assuming a body-result
equation. A present coordinate is fetched before the work tick; a typed refusal
returns the current mutable state and work, with no iterator in the result.

No selected-prefix, packed-storage or population invariant is needed for these
branches: membership and bit reads happen only after a successful tick.
Observation premises describe the supplied mathematical control tokens, not
concurrent runtime histories. Successful transitions, whole helper iteration,
allocation and complete membership checking remain separate obligations.
-/
namespace NativeSubsetSteps

/-- An exhausted atom range finishes selection without polling, reading the
    candidate or changing the selected vector or work record. This includes
    an empty range even when its supplied controls or work limit would refuse. -/
theorem selection_exhausted (candidate : theory.Interpretation)
    (iter : core.ops.range.Range Usize) (selected : alloc.vec.Vec Usize)
    (work : oracle.Work) (exhausted : iter.end.val ≤ iter.start.val) :
    oracle.select_atoms_loop.body candidate iter selected work =
      ok (.done (core.result.Result.Ok (), selected, work)) := by
  have comparisonExact : ∀ first second : Usize,
      core.cmp.PartialOrdUsize.lt first second = ok (decide (first.val < second.val)) := by
    intro first second
    rfl
  obtain ⟨⟨atom, next⟩, advanced, absent, unchanged⟩ := WP.spec_imp_exists
    (core.iter.range.IteratorRange.next_UScalar_none_spec
      (cloneInst := core.clone.CloneUsize) comparisonExact iter exhausted)
  have emptyRange : core.iter.range.IteratorRange.next core.iter.range.StepUsize iter =
      ok (none, iter) := by
    simpa only [absent, unchanged] using advanced
  simp [oracle.select_atoms_loop.body, emptyRange]

/-- A present selection coordinate is fetched before the tick. A control stop,
    or an exhausted allowance after clear controls, returns that typed reason
    with the current selected vector and every work field unchanged. The range
    has advanced internally, but the done result contains no cursor. -/
theorem selection_refused (candidate : theory.Interpretation)
    (iter : core.ops.range.Range Usize) (selected : alloc.vec.Vec Usize)
    (work : oracle.Work) (reason : zetesis_cpu.cancellation.Stop)
    (inside : iter.start.val < iter.end.val)
    (refused : NativeControl.observation work.cancellation = some reason ∨
      (NativeControl.observation work.cancellation = none ∧
        work.limits.max_work.val ≤ work.statistics.work.val ∧ reason = .WorkLimit)) :
    oracle.select_atoms_loop.body candidate iter selected work =
      ok (.done (core.result.Result.Err reason, selected, work)) := by
  have cloneExact : ∀ value : Usize, core.clone.CloneUsize.clone value = ok value := by
    intro value
    rfl
  have comparisonExact : ∀ first second : Usize,
      core.cmp.PartialOrdUsize.lt first second = ok (decide (first.val < second.val)) := by
    intro first second
    rfl
  obtain ⟨⟨atom, next⟩, advanced, returned, _, _⟩ := WP.spec_imp_exists
    (core.iter.range.IteratorRange.next_UScalar_some_spec cloneExact comparisonExact iter inside)
  have presentRange : core.iter.range.IteratorRange.next core.iter.range.StepUsize iter =
      ok (some iter.start, next) := by
    simpa only [returned] using advanced
  have tickRefused : oracle.Work.tick work = ok (core.result.Result.Err reason, work) := by
    rcases refused with observed | ⟨clear, exhausted, rfl⟩
    · exact NativeControl.tick_stopped work reason observed
    · exact NativeControl.tick_at_limit work clear exhausted
  simp [oracle.select_atoms_loop.body, presentRange, tickRefused,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- An exhausted selected-coordinate iterator finishes the carry without
    polling or touching words, population or work. The invariant required for
    a successful carry is unnecessary for this empty boundary. -/
theorem carry_exhausted (iter : core.slice.iter.Iter Usize)
    (subset : theory.Interpretation) (present : Usize) (work : oracle.Work)
    (exhausted : iter.slice.val.length ≤ iter.i) :
    oracle.advance_subset_loop.body iter subset present work =
      ok (.done (core.result.Result.Ok (), subset, present, work)) := by
  simp [oracle.advance_subset_loop.body,
    EvaluatorIteration.slice_next_exhausted iter exhausted]

/-- A present carry coordinate is fetched before its tick. A typed refusal
    retains the current packed words, population and complete work record, so
    any updates from earlier successful carry steps remain visible. No packed
    coordinate is indexed on this branch, and the done result has no cursor. -/
theorem carry_refused (iter : core.slice.iter.Iter Usize)
    (subset : theory.Interpretation) (present : Usize) (work : oracle.Work)
    (reason : zetesis_cpu.cancellation.Stop) (inside : iter.i < iter.slice.val.length)
    (refused : NativeControl.observation work.cancellation = some reason ∨
      (NativeControl.observation work.cancellation = none ∧
        work.limits.max_work.val ≤ work.statistics.work.val ∧ reason = .WorkLimit)) :
    oracle.advance_subset_loop.body iter subset present work =
      ok (.done (core.result.Result.Err reason, subset, present, work)) := by
  have tickRefused : oracle.Work.tick work = ok (core.result.Result.Err reason, work) := by
    rcases refused with observed | ⟨clear, exhausted, rfl⟩
    · exact NativeControl.tick_stopped work reason observed
    · exact NativeControl.tick_at_limit work clear exhausted
  simp [oracle.advance_subset_loop.body,
    EvaluatorIteration.slice_next_present iter inside, tickRefused,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

end NativeSubsetSteps
