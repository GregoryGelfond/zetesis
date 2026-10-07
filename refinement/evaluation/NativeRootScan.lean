import NativeControl
import Iteration

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisNativeExtract

/-!
# The generated root scan under fixed observation tokens

The root list is scanned in its stored order. A false root is a node identifier,
not a position or the smallest identifier. Repeated roots are visited repeatedly.
The statements distinguish a complete scan, its first false occurrence and a
typed stop before testing the next root; each tested root costs one admitted tick.

The supplied truth slice is immutable. Coverage of every root identifier is an
explicit premise. The theorems use the current external definitions, including
fixed observation tokens, and make no claim about concurrent Rust atomics,
pointer identity, allocation or the origin of the supplied truth values.
-/
namespace NativeRootScan

/-- The mathematical truth at one root identifier. Coverage separately justifies
that the actual checked slice read accesses this element. -/
def truth (values : Slice Bool) (root : Usize) : Bool :=
  values.val[root.val]?.getD false

/-- Every root occurrence in this half-open interval is true. Positions refer
to source order and therefore distinguish repeated root identifiers. -/
def Passed (values : Slice Bool) (roots : List Usize) (first last : Nat) : Prop :=
  ∀ index : Nat, ∀ inside : index < roots.length,
    first ≤ index → index < last → truth values roots[index] = true

/-- Extend a true prefix by the true occurrence immediately before it. -/
theorem passed_prepend (values : Slice Bool) (roots : List Usize)
    (first last : Nat) (inside : first < roots.length)
    (current : truth values roots[first] = true)
    (rest : Passed values roots (first + 1) last) :
    Passed values roots first last := by
  intro index bounded lower upper
  by_cases same : index = first
  · subst index
    exact current
  · exact rest index bounded (by omega) upper

/-- A scan preserves all work fields other than its charged work counter. -/
def Frame (before after : oracle.Work) : Prop :=
  after.limits = before.limits ∧
  after.cancellation = before.cancellation ∧
  after.statistics.subsets = before.statistics.subsets

/-- The result contract for a scan beginning at `first`. A complete true suffix
charges every remaining root. A failed root charges through its first false
occurrence. A stop charges only the preceding true occurrences and retains the
actual tick refusal at the next occurrence. -/
def Report (values : Slice Bool) (roots : List Usize) (first : Nat)
    (before : oracle.Work)
    (answer : core.result.Result (Option Usize) zetesis_cpu.cancellation.Stop)
    (after : oracle.Work) : Prop :=
  Frame before after ∧
  match answer with
  | .Ok none =>
    Passed values roots first roots.length ∧
    after.statistics.work.val = before.statistics.work.val + (roots.length - first)
  | .Ok (some root) =>
    ∃ index : Nat, ∃ inside : index < roots.length,
      first ≤ index ∧ root = roots[index] ∧ truth values root = false ∧
      Passed values roots first index ∧
      after.statistics.work.val = before.statistics.work.val + (index + 1 - first)
  | .Err reason =>
    ∃ index : Nat, ∃ inside : index < roots.length,
      first ≤ index ∧ Passed values roots first index ∧
      after.statistics.work.val = before.statistics.work.val + (index - first) ∧
      oracle.Work.tick after = ok (core.result.Result.Err reason, after)

/-- An admitted tick followed by a true root can be prepended to the suffix
report. It contributes exactly one to the work count, with no changed controls,
limits or subset count. The first false occurrence or stop remains the suffix's.
-/
theorem report_prepend (values : Slice Bool) (roots : List Usize)
    (first : Nat) (inside : first < roots.length)
    (current : truth values roots[first] = true)
    (before middle after : oracle.Work)
    (unchanged : Frame before middle)
    (incremented : middle.statistics.work.val = before.statistics.work.val + 1)
    (answer : core.result.Result (Option Usize) zetesis_cpu.cancellation.Stop)
    (rest : Report values roots (first + 1) middle answer after) :
    Report values roots first before answer after := by
  obtain ⟨middleFrame, suffix⟩ := rest
  have fullFrame : Frame before after := by
    exact ⟨middleFrame.1.trans unchanged.1,
      middleFrame.2.1.trans unchanged.2.1,
      middleFrame.2.2.trans unchanged.2.2⟩
  refine ⟨fullFrame, ?_⟩
  cases answer with
  | Ok answer =>
    cases answer with
    | none =>
      obtain ⟨passed, counted⟩ := suffix
      exact ⟨passed_prepend values roots first roots.length inside current passed, by omega⟩
    | some root =>
      obtain ⟨index, bounded, later, same, failed, passed, counted⟩ := suffix
      exact ⟨index, bounded, by omega, same, failed,
        passed_prepend values roots first index inside current passed, by omega⟩
  | Err reason =>
    obtain ⟨index, bounded, later, passed, counted, stopped⟩ := suffix
    exact ⟨index, bounded, by omega,
      passed_prepend values roots first index inside current passed, by omega, stopped⟩

/-- For a completed scan beginning at zero, absence of a failed root is
precisely truth of every stored root. The reverse direction rules out the
reported false occurrence, rather than assuming that a successful scan was full.
-/
theorem report_none_iff (values : Slice Bool) (roots : List Usize)
    (before after : oracle.Work) (answer : Option Usize)
    (reported : Report values roots 0 before (.Ok answer) after) :
    answer = none ↔ ∀ root ∈ roots, truth values root = true := by
  cases answer with
  | none =>
    have passed : Passed values roots 0 roots.length := by
      exact reported.2.1
    constructor
    · intro _ root member
      obtain ⟨index, inside, same⟩ := List.mem_iff_getElem.mp member
      rw [← same]
      exact passed index inside (Nat.zero_le index) inside
    · intro _
      rfl
  | some root =>
    obtain ⟨index, inside, _, same, failed, _, _⟩ := reported.2
    constructor
    · intro impossible
      cases impossible
    · intro allTrue
      have rootPresent : root ∈ roots := by
        exact same ▸ List.getElem_mem inside
      have rootTrue : truth values root = true := by
        exact allTrue root rootPresent
      have contradiction : false = true := by
        exact failed.symm.trans rootTrue
      cases contradiction

/-- A covered root read returns its mathematical truth using the actual backend
slice indexing operation. No defaulted read is used to justify success. -/
theorem read_exact (values : Slice Bool) (root : Usize)
    (covered : root.val < values.val.length) :
    Slice.index_usize values root = ok (truth values root) := by
  simp [Slice.index_usize, truth, List.getElem?_eq_getElem covered]

/-- The actual generated body finishes an exhausted root iterator without
polling or charging work, including when the supplied controls would refuse. -/
theorem body_exhausted (values : Slice Bool) (iter : core.slice.iter.Iter Usize)
    (work : oracle.Work) (exhausted : iter.slice.val.length ≤ iter.i) :
    oracle.failed_root_loop.body values iter work =
      ok (.done (core.result.Result.Ok none, work)) := by
  simp [oracle.failed_root_loop.body,
    EvaluatorIteration.slice_next_exhausted iter exhausted]

/-- A present root is fetched before its tick. A refused tick returns that
exact source stop and work record without reading the root's truth value. -/
theorem body_stopped (values : Slice Bool) (iter : core.slice.iter.Iter Usize)
    (work returned : oracle.Work) (inside : iter.i < iter.slice.val.length)
    (reason : zetesis_cpu.cancellation.Stop)
    (stopped : oracle.Work.tick work = ok (core.result.Result.Err reason, returned)) :
    oracle.failed_root_loop.body values iter work =
      ok (.done (core.result.Result.Err reason, returned)) := by
  simp [oracle.failed_root_loop.body,
    EvaluatorIteration.slice_next_present iter inside, stopped,
    core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- After an admitted tick, the actual body reads the covered root. A true
root advances the iterator; a false root returns this root identifier, with
its tick already charged. -/
theorem body_tested (values : Slice Bool) (iter : core.slice.iter.Iter Usize)
    (work returned : oracle.Work) (inside : iter.i < iter.slice.val.length)
    (covered : iter.slice.val[iter.i].val < values.val.length)
    (ticked : oracle.Work.tick work = ok (core.result.Result.Ok (), returned)) :
    oracle.failed_root_loop.body values iter work =
      if truth values iter.slice.val[iter.i] then
        ok (.cont ({ iter with i := iter.i + 1 }, returned))
      else ok (.done (core.result.Result.Ok (some iter.slice.val[iter.i]), returned)) := by
  simp [oracle.failed_root_loop.body,
    EvaluatorIteration.slice_next_present iter inside, ticked,
    core.result.Result.Insts.CoreOpsTry.branch, read_exact values _ covered]

/-- One unfolding of the generated loop retains both the iterator and work
record returned by its actual body. This includes the returned control token. -/
theorem loop_unfold (values : Slice Bool) (iter : core.slice.iter.Iter Usize)
    (work : oracle.Work) :
    oracle.failed_root_loop iter values work = (do
      let transition ← oracle.failed_root_loop.body values iter work
      match transition with
      | .done result => ok result
      | .cont (next, returned) => oracle.failed_root_loop next values returned) := by
  rw [oracle.failed_root_loop, loop]
  dsimp only [uncurry]
  congr 1
  funext transition
  cases transition with
  | done result => rfl
  | cont result =>
    obtain ⟨next, returned⟩ := result
    rfl

/-- A refused next tick terminates the actual loop with no tested root and no
charged work. Coverage of the unread truth entry is not needed for this branch.
-/
theorem loop_stopped (values : Slice Bool) (iter : core.slice.iter.Iter Usize)
    (work : oracle.Work) (inside : iter.i < iter.slice.val.length)
    (reason : zetesis_cpu.cancellation.Stop)
    (stopped : oracle.Work.tick work = ok (core.result.Result.Err reason, work)) :
    oracle.failed_root_loop iter values work =
      ok (core.result.Result.Err reason, work) ∧
    Report values iter.slice.val iter.i work (.Err reason) work := by
  have executed : oracle.failed_root_loop iter values work =
      ok (core.result.Result.Err reason, work) := by
    rw [loop_unfold, body_stopped values iter work work inside reason stopped]
    simp only [bind_tc_ok]
  have passed : Passed values iter.slice.val iter.i iter.i := by
    intro index _ lower upper
    omega
  exact ⟨executed, ⟨by simp [Frame], iter.i, inside, Nat.le_refl _,
    passed, by omega, stopped⟩⟩

/-- The actual generated loop terminates and returns its exact scan report.
Coverage prevents invalid truth reads; the finite unvisited-root count decreases
only after a true root. A false root or typed stop terminates immediately.

Proof: unfold the actual loop once. Exhaustion returns unchanged work. For a
present root, derive the actual tick from the supplied controls and allowance.
A refused tick yields a partial report. An admitted tick either finds the first
false occurrence or advances; recurse only in the latter case and prepend its
one charged true occurrence. Returned work, including control, is threaded
unchanged into the actual next call. No oracle-result equation is assumed.
-/
theorem loop_refines (values : Slice Bool) (iter : core.slice.iter.Iter Usize)
    (work : oracle.Work) (bounded : iter.i ≤ iter.slice.val.length)
    (covered : ∀ root ∈ iter.slice.val, root.val < values.val.length) :
    ∃ answer : core.result.Result (Option Usize) zetesis_cpu.cancellation.Stop,
      ∃ returned : oracle.Work,
        oracle.failed_root_loop iter values work = ok (answer, returned) ∧
        Report values iter.slice.val iter.i work answer returned := by
  have construct (remaining : Nat) :
      ∀ cursor : core.slice.iter.Iter Usize, ∀ current : oracle.Work,
        cursor.i ≤ cursor.slice.val.length →
        (∀ root ∈ cursor.slice.val, root.val < values.val.length) →
        cursor.slice.val.length - cursor.i = remaining →
        ∃ answer : core.result.Result (Option Usize) zetesis_cpu.cancellation.Stop,
          ∃ returned : oracle.Work,
            oracle.failed_root_loop cursor values current = ok (answer, returned) ∧
            Report values cursor.slice.val cursor.i current answer returned := by
    induction remaining using Nat.strong_induction_on with
    | h remaining ih =>
      intro cursor current within coverage unvisited
      by_cases inside : cursor.i < cursor.slice.val.length
      · have currentCovered : cursor.slice.val[cursor.i].val < values.val.length :=
          coverage _ (List.getElem_mem inside)
        cases observed : NativeControl.observation current.cancellation with
        | some reason =>
          have refused : oracle.Work.tick current =
              ok (core.result.Result.Err reason, current) := by
            exact NativeControl.tick_stopped current reason observed
          exact ⟨.Err reason, current, loop_stopped values cursor current inside reason refused⟩
        | none =>
          by_cases exhausted : current.limits.max_work.val ≤ current.statistics.work.val
          · have refused : oracle.Work.tick current =
                ok (core.result.Result.Err .WorkLimit, current) :=
              NativeControl.tick_at_limit current observed exhausted
            exact ⟨.Err .WorkLimit, current,
              loop_stopped values cursor current inside .WorkLimit refused⟩
          · obtain ⟨nextWork, incremented, ticked⟩ := NativeControl.tick_advances
              current observed (Nat.lt_of_not_ge exhausted)
            let next : oracle.Work :=
              { current with statistics := { current.statistics with work := nextWork } }
            have nextFrame : Frame current next := by simp [Frame, next]
            have tested : oracle.failed_root_loop.body values cursor current =
                if truth values cursor.slice.val[cursor.i] then
                  ok (.cont ({ cursor with i := cursor.i + 1 }, next))
                else ok (.done (core.result.Result.Ok (some cursor.slice.val[cursor.i]), next)) := by
              exact body_tested values cursor current next inside currentCovered ticked
            cases rootTruth : truth values cursor.slice.val[cursor.i] with
            | false =>
              have actual : oracle.failed_root_loop cursor values current =
                  ok (core.result.Result.Ok (some cursor.slice.val[cursor.i]), next) := by
                rw [loop_unfold, tested, rootTruth]
                simp only [Bool.false_eq_true, ↓reduceIte, bind_tc_ok]
              have passed : Passed values cursor.slice.val cursor.i cursor.i := by
                intro index _ lower upper
                omega
              refine ⟨.Ok (some cursor.slice.val[cursor.i]), next, actual,
                nextFrame, cursor.i, inside, Nat.le_refl _, rfl, rootTruth, passed, ?_⟩
              change nextWork.val = current.statistics.work.val + (cursor.i + 1 - cursor.i)
              omega
            | true =>
              let advanced : core.slice.iter.Iter Usize := { cursor with i := cursor.i + 1 }
              have fewer : advanced.slice.val.length - advanced.i < remaining := by
                dsimp only [advanced]
                omega
              have advancedWithin : advanced.i ≤ advanced.slice.val.length := by
                dsimp only [advanced]
                omega
              obtain ⟨answer, returned, executed, reported⟩ := ih
                (advanced.slice.val.length - advanced.i) fewer advanced next
                advancedWithin coverage rfl
              have actual : oracle.failed_root_loop cursor values current =
                  ok (answer, returned) := by
                rw [loop_unfold, tested, rootTruth]
                simpa only [↓reduceIte, bind_tc_ok] using executed
              have fullReport : Report values cursor.slice.val cursor.i current answer returned := by
                exact report_prepend values cursor.slice.val cursor.i inside rootTruth current next
                  returned nextFrame incremented answer reported
              exact ⟨answer, returned, actual, fullReport⟩
      · have exhausted : cursor.slice.val.length ≤ cursor.i := Nat.le_of_not_gt inside
        have actual : oracle.failed_root_loop cursor values current =
            ok (core.result.Result.Ok none, current) := by
          rw [loop_unfold, body_exhausted values cursor current exhausted]
          simp only [bind_tc_ok]
        have passed : Passed values cursor.slice.val cursor.i cursor.slice.val.length := by
          intro index _ lower upper
          omega
        exact ⟨.Ok none, current, actual, by simp [Frame], passed, by omega⟩
  exact construct (iter.slice.val.length - iter.i) iter work bounded covered rfl

/-- The actual root slice begins at position zero. Its order and repeated
identifiers are retained exactly. -/
def initialCursor (program : theory.Theory) : core.slice.iter.Iter Usize :=
  { slice := alloc.vec.Vec.deref program.value.roots, i := 0 }

/-- Generated setup reads the stored roots and initializes the real slice
iterator. It passes the truth slice and work through unchanged. -/
theorem failed_root_from_start (program : theory.Theory) (values : Slice Bool)
    (work : oracle.Work) :
    oracle.failed_root program values work =
      oracle.failed_root_loop (initialCursor program) values work := by
  simp [oracle.failed_root, theory.Theory.roots,
    alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
    SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter, initialCursor]

/-- With every root covered by the supplied truth slice, the actual generated
entry terminates with the scan report from position zero. No ordering, distinct
root, available-budget or completed-result premise is required. Stops remain
typed results and certify only the preceding true root occurrences. -/
theorem failed_root_refines (program : theory.Theory) (values : Slice Bool)
    (work : oracle.Work)
    (covered : ∀ root ∈ program.value.roots.val, root.val < values.val.length) :
    ∃ answer : core.result.Result (Option Usize) zetesis_cpu.cancellation.Stop,
      ∃ returned : oracle.Work,
        oracle.failed_root program values work = ok (answer, returned) ∧
        Report values program.value.roots.val 0 work answer returned := by
  have source : (initialCursor program).slice.val = program.value.roots.val := by
    simp [initialCursor, alloc.vec.Vec.deref]
  have sourceCovered : ∀ root ∈ (initialCursor program).slice.val,
      root.val < values.val.length := by
    simpa only [source] using covered
  obtain ⟨answer, returned, executed, reported⟩ := loop_refines values
    (initialCursor program) work (Nat.zero_le _) sourceCovered
  refine ⟨answer, returned, (failed_root_from_start program values work).trans executed, ?_⟩
  change Report values (initialCursor program).slice.val 0 work answer returned at reported
  rw [source] at reported
  exact reported

/-- Any actual generated entry result has the established scan contract.
This derives its meaning from execution; the premise supplies only which
result was returned, not an assumed correspondence with a reference scan. -/
theorem returned_report (program : theory.Theory) (values : Slice Bool)
    (before after : oracle.Work)
    (answer : core.result.Result (Option Usize) zetesis_cpu.cancellation.Stop)
    (covered : ∀ root ∈ program.value.roots.val, root.val < values.val.length)
    (returned : oracle.failed_root program values before = ok (answer, after)) :
    Report values program.value.roots.val 0 before answer after := by
  obtain ⟨actualAnswer, actualWork, executed, reported⟩ :=
    failed_root_refines program values before covered
  have same : (actualAnswer, actualWork) = (answer, after) := by
    exact Result.ok_injective (executed.symm.trans returned)
  have sameAnswer : actualAnswer = answer := by
    exact congrArg Prod.fst same
  have sameWork : actualWork = after := by
    exact congrArg Prod.snd same
  simpa only [sameAnswer, sameWork] using reported

/-- A completed actual scan returns no failed root exactly when every stored
root is true. The actual successful result is required: a cancellation or work
refusal cannot establish either satisfaction or failure of the complete theory.
The origin of `values` is deliberately separate, for composition with evaluation.
-/
theorem completed_none_iff (program : theory.Theory) (values : Slice Bool)
    (before after : oracle.Work) (answer : Option Usize)
    (covered : ∀ root ∈ program.value.roots.val, root.val < values.val.length)
    (completed : oracle.failed_root program values before =
      ok (core.result.Result.Ok answer, after)) :
    answer = none ↔ ∀ root ∈ program.value.roots.val, truth values root = true := by
  have reported : Report values program.value.roots.val 0 before (.Ok answer) after := by
    exact returned_report program values before after (.Ok answer) covered completed
  exact report_none_iff values program.value.roots.val before after answer reported

/-- The returned work increase is bounded by the number of root occurrences.
This counts repeated roots and a discovered false root; an untested stopped
root contributes nothing. Limits, controls and subset statistics are preserved.
-/
theorem returned_work_bound (program : theory.Theory) (values : Slice Bool)
    (before after : oracle.Work)
    (answer : core.result.Result (Option Usize) zetesis_cpu.cancellation.Stop)
    (covered : ∀ root ∈ program.value.roots.val, root.val < values.val.length)
    (returned : oracle.failed_root program values before = ok (answer, after)) :
    Frame before after ∧
      before.statistics.work.val ≤ after.statistics.work.val ∧
      after.statistics.work.val ≤ before.statistics.work.val + program.value.roots.val.length := by
  obtain ⟨frame, result⟩ := returned_report program values before after answer covered returned
  refine ⟨frame, ?_⟩
  cases answer with
  | Ok answer =>
    cases answer with
    | none =>
      obtain ⟨_, counted⟩ := result
      omega
    | some root =>
      obtain ⟨index, inside, _, _, _, _, counted⟩ := result
      omega
  | Err reason =>
    obtain ⟨index, inside, _, _, counted, _⟩ := result
    omega

end NativeRootScan
