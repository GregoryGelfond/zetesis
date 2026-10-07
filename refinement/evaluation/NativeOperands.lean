import NativeControl
import Iteration
import Zetesis.OperandArena

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisNativeExtract

/-!
# Actual native connective operand traversal

These laws concern the generated operand loop, including every occurrence of a
repeated child. The imported scalar, slice and atomic models are the execution
boundary. There is no assumed equation between the loop and formula semantics.
A complete row computes its ordered Boolean fold and charges its full length;
a refused row keeps the work already performed and returns a typed stop.
-/
namespace NativeOperands

/-- The one Boolean operation selected by the generated connective flag. -/
def combine (conjunction accumulated child : Bool) : Bool :=
  if conjunction then accumulated && child else accumulated || child

/-- The remaining ordered row, interpreted in the initialized truth prefix. -/
def fold (values : Slice Bool) (conjunction initial : Bool)
    (operands : List Usize) : Bool :=
  operands.foldl (fun accumulated child =>
    combine conjunction accumulated (values.val[child.val]?.getD false)) initial

/-- Work observations and limits are retained; only the work count advances. -/
structure Receipt (before after : oracle.Work) (used : Nat) : Prop where
  limits : after.limits = before.limits
  control : after.cancellation = before.cancellation
  subsets : after.statistics.subsets = before.statistics.subsets
  work : after.statistics.work.val = before.statistics.work.val + used

/-- An exhausted operand cursor returns the current accumulator without a tick. -/
theorem exhausted (values : Slice Bool) (iter : core.slice.iter.Iter Usize)
    (conjunction value : Bool) (work : oracle.Work)
    (ended : iter.slice.val.length ≤ iter.i) :
    oracle.evaluate_operands_loop.body values iter conjunction work value =
      ok (.done (.Ok value, work)) := by
  simp [oracle.evaluate_operands_loop.body,
    EvaluatorIteration.slice_next_exhausted iter ended]

/-- A refused operand tick returns the actual unchanged work record and no
continuation. The child value is not read after refusal. -/
theorem stopped (values : Slice Bool) (iter : core.slice.iter.Iter Usize)
    (conjunction value : Bool) (work : oracle.Work)
    (reason : zetesis_cpu.cancellation.Stop)
    (inside : iter.i < iter.slice.val.length)
    (tick : oracle.Work.tick work = ok (.Err reason, work)) :
    oracle.evaluate_operands_loop.body values iter conjunction work value =
      ok (.done (.Err reason, work)) := by
  simp [oracle.evaluate_operands_loop.body,
    EvaluatorIteration.slice_next_present iter inside, tick,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- One admitted tick visits exactly the next occurrence and incorporates its
stored truth. The Boolean accumulator does not short-circuit the visit. -/
theorem advanced (values : Slice Bool) (iter : core.slice.iter.Iter Usize)
    (conjunction value : Bool) (work next : oracle.Work)
    (inside : iter.i < iter.slice.val.length)
    (present : iter.slice.val[iter.i].val < values.val.length)
    (tick : oracle.Work.tick work = ok (.Ok (), next)) :
    oracle.evaluate_operands_loop.body values iter conjunction work value =
      ok (.cont ({ iter with i := iter.i + 1 }, conjunction, next,
        combine conjunction value values.val[iter.slice.val[iter.i].val])) := by
  obtain ⟨child, read, exactChild⟩ := WP.spec_imp_exists
    (Slice.index_usize_spec values iter.slice.val[iter.i] present)
  cases conjunction <;>
    simp [oracle.evaluate_operands_loop.body,
      EvaluatorIteration.slice_next_present iter inside, tick,
      core.result.Result.Insts.CoreOpsTry.branch, read, exactChild, combine]

/-- Unfold the actual generated loop, retaining every returned state field. -/
theorem loop_unfold (values : Slice Bool) (iter : core.slice.iter.Iter Usize)
    (conjunction value : Bool) (work : oracle.Work) :
    oracle.evaluate_operands_loop iter values conjunction work value = (do
      let transition ← oracle.evaluate_operands_loop.body values iter conjunction work value
      match transition with
      | .done result => ok result
      | .cont (cursor, connective, returned, accumulated) =>
        oracle.evaluate_operands_loop cursor values connective returned accumulated) := by
  rw [oracle.evaluate_operands_loop, loop]
  dsimp only [uncurry]
  congr 1
  funext transition
  cases transition with
  | done result => rfl
  | cont result =>
    obtain ⟨cursor, connective, returned, accumulated⟩ := result
    rfl

/-- The actual operand loop terminates with an exact work receipt. On success
it folds every remaining occurrence; on refusal it has consumed a strict prefix
of the row. Duplicates count separately, and no remaining-budget or success
assumption is required. Atomic observations are fixed by the imported model.

Proof: induct on the number of unvisited operands. An exhausted cursor returns
without polling. Otherwise a stopped tick finishes immediately; an admitted tick
reads the bounded child and decreases the remaining count by one. -/
theorem loop_refines (values : Slice Bool) (iter : core.slice.iter.Iter Usize)
    (conjunction value : Bool) (work : oracle.Work)
    (children : ∀ child ∈ iter.slice.val, child.val < values.val.length) :
    ∃ result returned used,
      oracle.evaluate_operands_loop iter values conjunction work value =
        ok (result, returned) ∧
      Receipt work returned used ∧
      used ≤ iter.slice.val.length - iter.i ∧
      match result with
      | .Ok truth => used = iter.slice.val.length - iter.i ∧
          truth = fold values conjunction value (iter.slice.val.drop iter.i)
      | .Err _ => used < iter.slice.val.length - iter.i := by
  have construct (remaining : Nat) :
      ∀ (cursor : core.slice.iter.Iter Usize) (accumulated : Bool) (current : oracle.Work),
      (∀ child ∈ cursor.slice.val, child.val < values.val.length) →
      cursor.slice.val.length - cursor.i = remaining →
      ∃ result returned used,
        oracle.evaluate_operands_loop cursor values conjunction current accumulated =
          ok (result, returned) ∧
        Receipt current returned used ∧ used ≤ remaining ∧
        match result with
        | .Ok truth => used = remaining ∧
            truth = fold values conjunction accumulated (cursor.slice.val.drop cursor.i)
        | .Err _ => used < remaining := by
    induction remaining using Nat.strong_induction_on with
    | h remaining ih =>
      intro cursor accumulated current bounded remainingCount
      by_cases inside : cursor.i < cursor.slice.val.length
      · have positive : 0 < remaining := by omega
        cases observed : NativeControl.observation current.cancellation with
        | some reason =>
          have step : oracle.evaluate_operands_loop.body values cursor conjunction current accumulated =
              ok (.done (.Err reason, current)) := by
            exact stopped values cursor conjunction accumulated current reason inside
              (NativeControl.tick_stopped current reason observed)
          refine ⟨.Err reason, current, 0, ?_, ⟨rfl, rfl, rfl, by omega⟩,
            Nat.zero_le _, positive⟩
          rw [loop_unfold, step]
          simp only [bind_tc_ok]
        | none =>
          by_cases allowance : current.statistics.work.val < current.limits.max_work.val
          · obtain ⟨counter, increment, tick⟩ :=
              NativeControl.tick_advances current observed allowance
            let next : oracle.Work :=
              { current with statistics := { current.statistics with work := counter } }
            have present : cursor.slice.val[cursor.i].val < values.val.length := by
              exact bounded _ (List.getElem_mem inside)
            have step : oracle.evaluate_operands_loop.body values cursor conjunction current accumulated =
                ok (.cont ({ cursor with i := cursor.i + 1 }, conjunction, next,
                  combine conjunction accumulated values.val[cursor.slice.val[cursor.i].val])) := by
              exact advanced values cursor conjunction accumulated current next inside present tick
            have decreased : cursor.slice.val.length - (cursor.i + 1) < remaining := by
              omega
            obtain ⟨result, returned, used, actual, receipt, within, boundary⟩ := ih
              (cursor.slice.val.length - (cursor.i + 1)) decreased
              { cursor with i := cursor.i + 1 }
              (combine conjunction accumulated values.val[cursor.slice.val[cursor.i].val])
              next bounded rfl
            refine ⟨result, returned, used + 1, ?_, ?_, ?_, ?_⟩
            · rw [loop_unfold, step]
              simpa only [bind_tc_ok] using actual
            · refine ⟨receipt.limits, receipt.control, receipt.subsets, ?_⟩
              have counted : returned.statistics.work.val = next.statistics.work.val + used := by
                exact receipt.work
              change returned.statistics.work.val = counter.val + used at counted
              omega
            · omega
            · cases result with
              | Err reason => change used < _ at boundary; omega
              | Ok truth =>
                obtain ⟨counted, exactTruth⟩ := boundary
                refine ⟨by omega, ?_⟩
                rw [List.drop_eq_getElem_cons inside]
                simpa only [fold, List.foldl_cons, List.getElem?_eq_getElem present,
                  Option.getD_some] using exactTruth
          · have step : oracle.evaluate_operands_loop.body values cursor conjunction current accumulated =
                ok (.done (.Err .WorkLimit, current)) := by
              exact stopped values cursor conjunction accumulated current .WorkLimit inside
                (NativeControl.tick_at_limit current observed (Nat.le_of_not_gt allowance))
            refine ⟨.Err .WorkLimit, current, 0, ?_, ⟨rfl, rfl, rfl, by omega⟩,
              Nat.zero_le _, positive⟩
            rw [loop_unfold, step]
            simp only [bind_tc_ok]
      · have ended : cursor.slice.val.length ≤ cursor.i := by
          exact Nat.le_of_not_gt inside
        have zero : remaining = 0 := by omega
        refine ⟨.Ok accumulated, current, 0, ?_, ⟨rfl, rfl, rfl, by omega⟩,
          Nat.zero_le _, ?_⟩
        · rw [loop_unfold, exhausted values cursor conjunction accumulated current ended]
          simp only [bind_tc_ok]
        · simp [zero, List.drop_eq_nil_of_le ended, fold]
  exact construct (iter.slice.val.length - iter.i) iter value work children rfl

/-- The unchanged work record has a zero-length receipt. -/
theorem Receipt.refl (work : oracle.Work) : Receipt work work 0 :=
  ⟨rfl, rfl, rfl, by omega⟩

/-- Consecutive visits compose their actual counters without losing partial
work from an earlier visit. The sum is natural arithmetic over stored counters. -/
theorem Receipt.trans {before middle after : oracle.Work} {first second : Nat}
    (left : Receipt before middle first) (right : Receipt middle after second) :
    Receipt before after (first + second) := by
  refine ⟨right.limits.trans left.limits, right.control.trans left.control,
    right.subsets.trans left.subsets, ?_⟩
  have one : middle.statistics.work.val = before.statistics.work.val + first := by
    exact left.work
  have two : after.statistics.work.val = middle.statistics.work.val + second := by
    exact right.work
  omega

/-- The operand wrapper initializes the actual slice iterator and identity
accumulator. Its complete result folds the whole row, with one unit per stored
occurrence; typed refusal retains exactly the consumed strict prefix. -/
theorem evaluate_refines (operands : Slice Usize) (values : Slice Bool)
    (conjunction : Bool) (work : oracle.Work)
    (children : ∀ child ∈ operands.val, child.val < values.val.length) :
    ∃ result returned used,
      oracle.evaluate_operands operands values conjunction work = ok (result, returned) ∧
      Receipt work returned used ∧ used ≤ operands.val.length ∧
      match result with
      | .Ok truth => used = operands.val.length ∧
          truth = fold values conjunction conjunction operands.val
      | .Err _ => used < operands.val.length := by
  simpa only [oracle.evaluate_operands,
    SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter,
    bind_tc_ok, Nat.sub_zero, List.drop_zero] using
    loop_refines values ⟨operands, 0⟩ conjunction conjunction work children

/-- The generated flag selects the existing general connective definition. -/
def kind (conjunction : Bool) : Zetesis.Refinement.OperandArena.Connective :=
  if conjunction then .conj else .disj

/-- The ordered accumulator fold computes the existing finite-connective
reduction. This is a Boolean representation law, with duplicates and empty rows
preserved, rather than a new formula semantics. -/
theorem fold_reduce (values : Slice Bool) (conjunction initial : Bool)
    (operands : List Usize) :
    fold values conjunction initial operands =
      combine conjunction initial (Zetesis.Refinement.OperandArena.reduce (kind conjunction)
        (operands.map (fun child => values.val[child.val]?.getD false))) := by
  induction operands generalizing initial with
  | nil => cases conjunction <;> cases initial <;> rfl
  | cons child rest inductionHypothesis =>
    simp only [fold, List.foldl_cons] at *
    rw [inductionHypothesis]
    cases conjunction <;> cases initial <;>
      cases value : values.val[child.val]?.getD false <;>
      simp [combine, kind, Zetesis.Refinement.OperandArena.reduce, value]

/-- Starting at the connective's identity removes the accumulator from the
result and gives precisely the reusable finite all/any interpretation. -/
theorem identity_fold (values : Slice Bool) (conjunction : Bool)
    (operands : List Usize) :
    fold values conjunction conjunction operands =
      Zetesis.Refinement.OperandArena.reduce (kind conjunction)
        (operands.map (fun child => values.val[child.val]?.getD false)) := by
  rw [fold_reduce]
  cases conjunction <;> simp [combine]

end NativeOperands
