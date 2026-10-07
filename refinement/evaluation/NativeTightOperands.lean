import NativeTightWork
import NativeTightClassFold
import Iteration

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisNativeExtract
open NativeTightClassFold

/-!
# Actual native tight operand classification

The generated loop visits the entire ordered occurrence row. Each admitted tick
precedes the child-class read, even when the accumulator is already Opaque.
Completion returns the finite class fold and charges every occurrence. A typed
stop retains exactly the earlier admitted work and publishes no completed class.

The child-prefix and valid-counter premises justify reads and checked addition.
Fixed control observations and imported scalar/sequence operations remain the
existing execution boundary. This module does not classify complete theories,
construct rank certificates or establish runtime observation histories.
-/
namespace NativeTightOperands

/-- A row's returned work retains allowance and controls, and charges exactly
the recorded number of admitted occurrence visits. -/
structure Receipt (before after : tight.Work) (used : Nat) : Prop where
  allowance : after.max = before.max
  control : after.cancellation = before.cancellation
  charged : after.used.val = before.used.val + used

/-- Exhaustion returns the current class without polling or charging a tick. -/
theorem exhausted (classes : Slice tight.compile.Body)
    (iter : core.slice.iter.Iter Usize) (classification : tight.compile.Body)
    (work : tight.Work) (ended : iter.slice.val.length ≤ iter.i) :
    tight.compile.classify_operands_loop.body classes iter work classification =
      ok (.done (.Ok classification, work)) := by
  simp [tight.compile.classify_operands_loop.body,
    EvaluatorIteration.slice_next_exhausted iter ended]

/-- Refusal of the next tick returns the unchanged current work and no class.
No child lookup follows refusal, so this law needs no child-storage premise. -/
theorem stopped (classes : Slice tight.compile.Body)
    (iter : core.slice.iter.Iter Usize) (classification : tight.compile.Body)
    (work : tight.Work) (reason : tight.TightError)
    (inside : iter.i < iter.slice.val.length)
    (tick : tight.Work.tick work = ok (.Err reason, work)) :
    tight.compile.classify_operands_loop.body classes iter work classification =
      ok (.done (.Err reason, work)) := by
  simp [tight.compile.classify_operands_loop.body,
    EvaluatorIteration.slice_next_present iter inside, tick,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
    core.convert.FromSame.from]

/-- An admitted visit reads the next present class and incorporates it before
continuing. Opaque accumulators and repeated child IDs still take this step. -/
theorem advanced (classes : Slice tight.compile.Body)
    (iter : core.slice.iter.Iter Usize) (classification : tight.compile.Body)
    (work next : tight.Work) (inside : iter.i < iter.slice.val.length)
    (present : iter.slice.val[iter.i].val < classes.val.length)
    (tick : tight.Work.tick work = ok (.Ok (), next)) :
    tight.compile.classify_operands_loop.body classes iter work classification =
      ok (.cont ({ iter with i := iter.i + 1 }, next,
        combine classification classes.val[iter.slice.val[iter.i].val])) := by
  obtain ⟨child, read, exactChild⟩ := WP.spec_imp_exists
    (Slice.index_usize_spec classes iter.slice.val[iter.i] present)
  cases classification <;> cases child <;>
    simp [tight.compile.classify_operands_loop.body,
      EvaluatorIteration.slice_next_present iter inside, tick,
      core.result.Result.Insts.CoreOpsTry.branch, read, ← exactChild, combine]

/-- Unfold the actual generated loop with its returned cursor, work and tag. -/
theorem loop_unfold (classes : Slice tight.compile.Body)
    (iter : core.slice.iter.Iter Usize) (classification : tight.compile.Body)
    (work : tight.Work) :
    tight.compile.classify_operands_loop iter classes work classification = (do
      let transition ← tight.compile.classify_operands_loop.body classes iter work classification
      match transition with
      | .done result => ok result
      | .cont (cursor, returned, accumulated) =>
        tight.compile.classify_operands_loop cursor classes returned accumulated) := by
  rw [tight.compile.classify_operands_loop, loop]
  dsimp only [uncurry]
  congr 1
  funext transition
  cases transition with
  | done result => rfl
  | cont result =>
    obtain ⟨cursor, returned, accumulated⟩ := result
    rfl

/-- The returned row result distinguishes completed coverage from a strict
partial prefix. Refusal records the exact reason from the next actual tick;
it gives no class meaning for the unfinished row. -/
def Boundary (classes : Slice tight.compile.Body) (operands : List Usize)
    (initial : tight.compile.Body) (used : Nat) (returned : tight.Work) :
    core.result.Result tight.compile.Body tight.TightError → Prop
  | .Ok classification => used = operands.length ∧ classification = fold classes initial operands
  | .Err reason => used < operands.length ∧
      tight.Work.tick returned = ok (.Err reason, returned)

/-- Every valid initialized row state has a finite actual result with exact
work. Success folds every remaining occurrence; refusal retains a strict prefix
and the reached typed error. No control-clear, spare-budget or success premise
is supplied, and no opaque-tag short circuit is assumed.

Proof: induct on the unvisited row length. Exhaustion returns without a tick.
Otherwise the actual tick either refuses unchanged or admits one bounded read.
Only that successful visit recurses, carrying its incremented work and tag. -/
theorem loop_refines (classes : Slice tight.compile.Body)
    (iter : core.slice.iter.Iter Usize) (initial : tight.compile.Body) (work : tight.Work)
    (children : ∀ child ∈ iter.slice.val, child.val < classes.val.length)
    (budget : work.used.val ≤ work.max.val) :
    ∃ result returned used,
      tight.compile.classify_operands_loop iter classes work initial = ok (result, returned) ∧
      Receipt work returned used ∧ returned.used.val ≤ returned.max.val ∧
      used ≤ iter.slice.val.length - iter.i ∧
      Boundary classes (iter.slice.val.drop iter.i) initial used returned result := by
  have construct (remaining : Nat) :
      ∀ (cursor : core.slice.iter.Iter Usize) (accumulated : tight.compile.Body) (current : tight.Work),
      (∀ child ∈ cursor.slice.val, child.val < classes.val.length) →
      current.used.val ≤ current.max.val → cursor.slice.val.length - cursor.i = remaining →
      ∃ result returned used,
        tight.compile.classify_operands_loop cursor classes current accumulated = ok (result, returned) ∧
        Receipt current returned used ∧ returned.used.val ≤ returned.max.val ∧
        used ≤ remaining ∧ Boundary classes (cursor.slice.val.drop cursor.i)
          accumulated used returned result := by
    induction remaining using Nat.strong_induction_on with
    | h remaining ih =>
      intro cursor accumulated current bounded counterBound remainingCount
      by_cases inside : cursor.i < cursor.slice.val.length
      · rcases NativeTightWork.refines current counterBound with refused | admitted
        · obtain ⟨reason, tick⟩ := refused
          have step : tight.compile.classify_operands_loop.body classes cursor current accumulated =
              ok (.done (.Err reason, current)) := by
            exact stopped classes cursor accumulated current reason inside tick
          refine ⟨.Err reason, current, 0, ?_, ⟨rfl, rfl, by omega⟩,
            counterBound, Nat.zero_le _, ?_⟩
          · rw [loop_unfold, step]
            simp only [bind_tc_ok]
          · exact ⟨by simp only [List.length_drop]; omega, tick⟩
        · obtain ⟨counter, increment, counterWithin, tick⟩ := admitted
          let next : tight.Work := { current with used := counter }
          have present : cursor.slice.val[cursor.i].val < classes.val.length := by
            exact bounded _ (List.getElem_mem inside)
          have step : tight.compile.classify_operands_loop.body classes cursor current accumulated =
              ok (.cont ({ cursor with i := cursor.i + 1 }, next,
                combine accumulated classes.val[cursor.slice.val[cursor.i].val])) := by
            exact advanced classes cursor accumulated current next inside present tick
          have decreased : cursor.slice.val.length - (cursor.i + 1) < remaining := by
            omega
          obtain ⟨result, returned, used, actual, receipt, returnedBound, within, boundary⟩ := ih
            (cursor.slice.val.length - (cursor.i + 1)) decreased
            { cursor with i := cursor.i + 1 }
            (combine accumulated classes.val[cursor.slice.val[cursor.i].val])
            next bounded counterWithin rfl
          refine ⟨result, returned, used + 1, ?_, ?_, returnedBound, ?_, ?_⟩
          · rw [loop_unfold, step]
            simpa only [bind_tc_ok] using actual
          · refine ⟨receipt.allowance, receipt.control, ?_⟩
            have counted : returned.used.val = counter.val + used := by
              exact receipt.charged
            omega
          · omega
          · cases result with
            | Err reason =>
              obtain ⟨prefixShort, tick⟩ := boundary
              refine ⟨?_, tick⟩
              simp only [List.length_drop] at prefixShort ⊢
              omega
            | Ok classification =>
              obtain ⟨counted, classExact⟩ := boundary
              refine ⟨?_, ?_⟩
              · simp only [List.length_drop] at counted ⊢
                omega
              · rw [List.drop_eq_getElem_cons inside]
                simpa only [fold, List.foldl_cons, List.getElem?_eq_getElem present,
                  Option.getD_some] using classExact
      · have ended : cursor.slice.val.length ≤ cursor.i := by
          exact Nat.le_of_not_gt inside
        refine ⟨.Ok accumulated, current, 0, ?_, ⟨rfl, rfl, by omega⟩,
          counterBound, Nat.zero_le _, ?_⟩
        · rw [loop_unfold, exhausted classes cursor accumulated current ended]
          simp only [bind_tc_ok]
        · simp [Boundary, List.drop_eq_nil_of_le ended, fold]
  exact construct (iter.slice.val.length - iter.i) iter initial work children budget rfl

/-- Generated setup starts at the first operand with the Frozen identity tag.
The complete wrapper has the same exact receipt and refusal boundary as its
actual loop, with one charge for each visited occurrence. -/
theorem classify_refines (operands : Slice Usize) (classes : Slice tight.compile.Body)
    (work : tight.Work) (children : ∀ child ∈ operands.val, child.val < classes.val.length)
    (budget : work.used.val ≤ work.max.val) :
    ∃ result returned used,
      tight.compile.classify_operands operands classes work = ok (result, returned) ∧
      Receipt work returned used ∧ returned.used.val ≤ returned.max.val ∧
      used ≤ operands.val.length ∧ Boundary classes operands.val .Frozen used returned result := by
  simpa only [tight.compile.classify_operands,
    SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter,
    bind_tc_ok, Nat.sub_zero, List.drop_zero] using
    loop_refines classes ⟨operands, 0⟩ .Frozen work children budget

/-- Any actual returned result has the constructed row receipt. Completion is
not assumed; an Err retains the next refused tick after a strict visited prefix. -/
theorem returned_receipt (operands : Slice Usize) (classes : Slice tight.compile.Body)
    (work : tight.Work) (children : ∀ child ∈ operands.val, child.val < classes.val.length)
    (budget : work.used.val ≤ work.max.val)
    (result : core.result.Result tight.compile.Body tight.TightError)
    (returned : tight.Work)
    (actual : tight.compile.classify_operands operands classes work = ok (result, returned)) :
    ∃ used, Receipt work returned used ∧ returned.used.val ≤ returned.max.val ∧
      used ≤ operands.val.length ∧ Boundary classes operands.val .Frozen used returned result := by
  obtain ⟨outcome, after, used, executed, receipt, bound, within, boundary⟩ :=
    classify_refines operands classes work children budget
  have same : (outcome, after) = (result, returned) := by
    exact Result.ok_injective (executed.symm.trans actual)
  have sameResult : outcome = result := by
    exact congrArg Prod.fst same
  have sameWork : after = returned := by
    exact congrArg Prod.snd same
  exact ⟨used, sameWork ▸ receipt, sameWork ▸ bound, within,
    sameResult ▸ sameWork ▸ boundary⟩

/-- A completed row returns its entire ordered class fold and charges its whole
length, even if its tag became Opaque before the last occurrence. -/
theorem completed (operands : Slice Usize) (classes : Slice tight.compile.Body)
    (work returned : tight.Work) (classification : tight.compile.Body)
    (children : ∀ child ∈ operands.val, child.val < classes.val.length)
    (budget : work.used.val ≤ work.max.val)
    (actual : tight.compile.classify_operands operands classes work = ok (.Ok classification, returned)) :
    classification = fold classes .Frozen operands.val ∧
      returned.used.val = work.used.val + operands.val.length ∧
      returned.max = work.max ∧ returned.cancellation = work.cancellation := by
  obtain ⟨used, receipt, _, _, boundary⟩ :=
    returned_receipt operands classes work children budget (.Ok classification) returned actual
  obtain ⟨counted, classExact⟩ := boundary
  exact ⟨classExact, counted ▸ receipt.charged, receipt.allowance, receipt.control⟩

end NativeTightOperands
