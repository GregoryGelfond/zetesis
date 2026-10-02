import Step
import Control

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisExtract

/-!
# One complete generated evaluator transition

The result composes the real generated iterator, work tick and node continuation.
Its inputs are cursor/prefix alignment, storage and read bounds, the explicitly
supplied observations for this one invocation, and a remaining work allowance.
No iterator, tick, node-evaluation or output-agreement equation is assumed.

The mask is supplied data here; its derivation by original evaluation is a later
composition obligation. The imported atomic observation model is valid only for
this invocation. These laws make no temporal, allocation or whole-loop claim.
-/
namespace EvaluationProgress

/-- An admitted present-node transition appends exactly one computed masked
value, advances the enumerated slice once and charges exactly one unit of work.
The subset count, limits and control record are unchanged. Cursor and output
alignment are preserved, so the next node has a prefix of the correct length.

Proof: derive iterator success from its synchronized position and stored slice
bound; derive tick success from the actual observed control and U64 limit;
derive append room and mask bounds; then apply the generated node-step law. -/
theorem present_step (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (cursor : Evaluation.Cursor)
    (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (aligned : cursor.count.val = cursor.iter.i)
    (prefixLength : output.val.length = cursor.iter.i)
    (inside : cursor.iter.i < cursor.iter.slice.val.length)
    (stored : Membership.Represented candidate)
    (children : Evaluation.ChildrenPresent output.val
      cursor.iter.slice.val[cursor.iter.i])
    (covered : ∀ mask ∈ frozen, cursor.iter.slice.val.length ≤ mask.val.length)
    (clear : EvaluatorControl.observation control = none)
    (remaining : statistics.work.val < limits.max_work.val) :
    ∃ nextCount : Usize, ∃ nextWork : U64, ∃ extended : alloc.vec.Vec Bool,
      nextCount.val = cursor.count.val + 1 ∧
      nextWork.val = statistics.work.val + 1 ∧
      oracle.evaluate_loop.body candidate frozen cursor output limits control statistics =
        ok (.cont ({ iter := { cursor.iter with i := cursor.iter.i + 1 }, count := nextCount },
          extended, limits, control,
          { statistics with work := nextWork })) ∧
      extended.val = output.val ++ [Evaluation.masked frozen cursor.count
        (Evaluation.value candidate output.val cursor.iter.slice.val[cursor.iter.i])] ∧
      extended.val.length = cursor.iter.i + 1 ∧
      nextCount.val = extended.val.length := by
  obtain ⟨nextCount, countAdvanced, advanced⟩ :=
    EvaluatorIteration.enumerate_next_present cursor aligned inside
  obtain ⟨nextWork, workAdvanced, ticked⟩ :=
    EvaluatorControl.tick_advances ⟨limits, control, statistics⟩ clear remaining
  have room : output.val.length < Usize.max := by
    have sliceFits : cursor.iter.slice.val.length ≤ Usize.max :=
      Slice.length_ineq cursor.iter.slice
    omega
  have maskPresent : Evaluation.MaskPresent frozen cursor.count := by
    intro mask member
    have maskCovers : cursor.iter.slice.val.length ≤ mask.val.length :=
      covered mask member
    omega
  obtain ⟨extended, stepped, exactPrefix⟩ := Evaluation.step_after_tick
    candidate frozen cursor
    { iter := { cursor.iter with i := cursor.iter.i + 1 }, count := nextCount }
    output limits control statistics cursor.count
    cursor.iter.slice.val[cursor.iter.i]
    { limits := limits, cancellation := control,
      statistics := { statistics with work := nextWork } }
    advanced ticked stored children maskPresent room
  have prefixAdvanced : extended.val.length = cursor.iter.i + 1 := by
    rw [exactPrefix, List.length_append, List.length_singleton, prefixLength]
  have synchronized : nextCount.val = extended.val.length := by
    omega
  exact ⟨nextCount, nextWork, extended, countAdvanced, workAdvanced, stepped,
    exactPrefix, prefixAdvanced, synchronized⟩

/-- With a present node, a supplied control-stop observation exits the actual
body before evaluating that node or appending. Output and the entire work record
are unchanged. No packed-storage, child-read, mask or output-prefix premise is
needed for this stopped branch. Cancellation/expiry priority is determined by
`EvaluatorControl.observation`, not by an assumed tick result. -/
theorem control_stops (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (cursor : Evaluation.Cursor)
    (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (reason : zetesis_cpu.cancellation.Stop)
    (aligned : cursor.count.val = cursor.iter.i)
    (inside : cursor.iter.i < cursor.iter.slice.val.length)
    (observed : EvaluatorControl.observation control = some reason) :
    oracle.evaluate_loop.body candidate frozen cursor output limits control statistics =
      ok (.done (core.result.Result.Err reason, output,
        ⟨limits, control, statistics⟩)) := by
  obtain ⟨nextCount, _, advanced⟩ :=
    EvaluatorIteration.enumerate_next_present cursor aligned inside
  have stopped : oracle.Work.tick ⟨limits, control, statistics⟩ =
      ok (core.result.Result.Err reason, ⟨limits, control, statistics⟩) :=
    EvaluatorControl.tick_stopped ⟨limits, control, statistics⟩ reason observed
  exact Evaluation.step_stops candidate frozen cursor
    { iter := { cursor.iter with i := cursor.iter.i + 1 }, count := nextCount }
    output limits control statistics cursor.count
    cursor.iter.slice.val[cursor.iter.i] ⟨limits, control, statistics⟩ reason
    advanced stopped

/-- With a present node and no control stop, an exhausted work allowance exits
before node evaluation and preserves output and all work fields. Equality with
the configured limit already refuses this transition. -/
theorem work_limit_stops (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (cursor : Evaluation.Cursor)
    (output : alloc.vec.Vec Bool) (limits : oracle.Limits)
    (control : zetesis_cpu.cancellation.Cancellation) (statistics : oracle.Statistics)
    (aligned : cursor.count.val = cursor.iter.i)
    (inside : cursor.iter.i < cursor.iter.slice.val.length)
    (clear : EvaluatorControl.observation control = none)
    (exhausted : limits.max_work.val ≤ statistics.work.val) :
    oracle.evaluate_loop.body candidate frozen cursor output limits control statistics =
      ok (.done (core.result.Result.Err .WorkLimit, output,
        ⟨limits, control, statistics⟩)) := by
  obtain ⟨nextCount, _, advanced⟩ :=
    EvaluatorIteration.enumerate_next_present cursor aligned inside
  have stopped : oracle.Work.tick ⟨limits, control, statistics⟩ =
      ok (core.result.Result.Err .WorkLimit, ⟨limits, control, statistics⟩) :=
    EvaluatorControl.tick_at_limit ⟨limits, control, statistics⟩ clear exhausted
  exact Evaluation.step_stops candidate frozen cursor
    { iter := { cursor.iter with i := cursor.iter.i + 1 }, count := nextCount }
    output limits control statistics cursor.count
    cursor.iter.slice.val[cursor.iter.i] ⟨limits, control, statistics⟩ .WorkLimit
    advanced stopped

end EvaluationProgress
