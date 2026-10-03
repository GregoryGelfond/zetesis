import TightClassificationLoop
import ReservedStorage

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Completed classification from the actual reservation wrapper

The generated wrapper reserves class storage and starts the real scan at the
first node. Successful sequence preservation for that one empty reservation
supplies the initialized empty class vector. The loop proof then determines the
entire returned fold and work count. Allocation success is never assumed, and
backend failure, divergence and typed refusal cannot establish completion.

The supplied reservation provider and fixed control observations retain their
per-invocation scope. This is one classification phase, not a complete tight-plan
certificate, runtime history or answer-set decision.
-/
namespace TightClassification

variable [VectorReservation]
/-- Tight-plan reservation uses the existing reservation operation and wraps
only its source refusal. Backend failure and divergence pass through the bind. -/
theorem reserve_from_oracle (T : Type) (count : Usize) :
    tight.reserve T count = (do
      let reserved ← oracle.reserve T count
      match reserved with
      | .Ok vector => ok (.Ok vector)
      | .Err reason => ok (.Err (.Stopped reason))) := by
  rw [ReservedStorage.reserve_exact]
  unfold tight.reserve
  simp only [bind_assoc_eq]
  congr 1
  funext returned
  rcases returned with ⟨verdict, vector⟩
  cases verdict with
  | Ok value =>
      cases value
      simp [core.result.Result.map_err, core.result.Result.Insts.CoreOpsTry.branch]
  | Err error =>
      simp [core.result.Result.map_err,
        tight.reserve.closure.Insts.CoreOpsFunctionFnOnceTupleTryReserveErrorStop.call_once,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
        tight.TightError.Insts.CoreConvertFromStop.from]

/-- The wrapper uses the actual reservation result and starts the slice cursor
at zero. The bind retains typed refusal, backend failure and divergence. -/
theorem classify_setup (program : theory.Theory) (work : tight.Work) :
    tight.compile.classify program work = (do
      let reserved ← tight.reserve tight.compile.Body (Slice.len (alloc.vec.Vec.deref program.value.nodes))
      match reserved with
      | .Ok initial =>
          tight.compile.classify_loop { slice := (alloc.vec.Vec.deref program.value.nodes), i := 0 }
            program work initial
      | .Err reason => ok (.Err reason, work)) := by
  unfold tight.compile.classify
  simp only [theory.Theory.nodes, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
    bind_tc_ok]
  congr 1
  funext reserved
  cases reserved with
  | Ok initial =>
      simp [core.result.Result.Insts.CoreOpsTry.branch,
        SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter]
  | Err reason =>
      simp [core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual,
        core.convert.FromSame.from]

/-- A successful actual classification returns the complete finite class fold
and charges one tick per stored node. Its reservation must preserve the logical
empty input on success; no reservation-success premise is supplied.

Proof: recover the actual successful reservation from the wrapper, initialize
the prefix invariant at zero, and identify the completed generated scan with
the loop's constructed execution and exact work receipt. -/
theorem completed_classes (program : theory.Theory) (work returned : tight.Work)
    (output : alloc.vec.Vec tight.compile.Body)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (budget : work.used.val ≤ work.max.val)
    (preservesEmpty : ∀ initial : alloc.vec.Vec tight.compile.Body,
      alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new tight.compile.Body)
        (Slice.len (alloc.vec.Vec.deref program.value.nodes)) = ok (.Ok (), initial) →
          initial.val = [])
    (completed : tight.compile.classify program work = ok (.Ok output, returned)) :
    output.val = TightClassificationSemantics.classes program.value.nodes.val ∧
      returned.used.val = work.used.val + program.value.nodes.val.length ∧
      returned.max = work.max ∧ returned.cancellation = work.cancellation := by
  rw [classify_setup, reserve_from_oracle] at completed
  simp only [bind_assoc_eq] at completed
  cases reserved : oracle.reserve tight.compile.Body
      (Slice.len (alloc.vec.Vec.deref program.value.nodes)) with
  | ret attempt =>
    cases attempt with
    | Err reason => simp [reserved] at completed
    | Ok initial =>
      have empty : initial.val = [] := preservesEmpty initial
        (ReservedStorage.completed_reservation tight.compile.Body
          (Slice.len (alloc.vec.Vec.deref program.value.nodes)) initial reserved)
      have invariant : TightClassificationLoop.Invariant program
          { slice := alloc.vec.Vec.deref program.value.nodes, i := 0 } initial := by
        refine ⟨by simp [alloc.vec.Vec.deref], Nat.zero_le _, ?_⟩
        simp [empty, TightClassificationSemantics.scan]
      have loopComplete : tight.compile.classify_loop
          { slice := alloc.vec.Vec.deref program.value.nodes, i := 0 } program work initial =
          ok (.Ok output, returned) := by
        simpa only [reserved, bind_tc_ok] using completed
      simpa only [Nat.sub_zero] using TightClassificationLoop.completed_classes program
        { slice := alloc.vec.Vec.deref program.value.nodes, i := 0 } work initial output returned
        ordered invariant budget loopComplete
  | vis effect continuation => simp only [reserved, bind_tc_vis, vis_not_ok] at completed
  | div => simp only [reserved, bind_tc_div, div_not_ok] at completed

/-- Every class returned by the actual classifier is the structural class of
its corresponding original formula. A full successful scan supplies all
positions, with one work unit per node and unchanged allowance and control.

Proof: the generated wrapper/loop theorem derives the complete finite fold;
the ordered-DAG theorem identifies that fold with exact body recognition.
`formulaClass_opaque`, `formulaClass_frozen` and `formulaClass_positive` then
state its grammar and positive-occurrence meaning. No rank certificate or
answer-set decision follows from classification alone. -/
theorem completed_semantics (program : theory.Theory) (work returned : tight.Work)
    (output : alloc.vec.Vec tight.compile.Body)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (budget : work.used.val ≤ work.max.val)
    (preservesEmpty : ∀ initial : alloc.vec.Vec tight.compile.Body,
      alloc.vec.Vec.try_reserve_exact Global (alloc.vec.Vec.new tight.compile.Body)
        (Slice.len (alloc.vec.Vec.deref program.value.nodes)) = ok (.Ok (), initial) →
          initial.val = [])
    (completed : tight.compile.classify program work = ok (.Ok output, returned)) :
    output.val = (Zetesis.DagSharing.meanings
        (program.value.nodes.val.map EvaluationSemantics.node)).map
          TightClassificationSemantics.formulaClass ∧
      returned.used.val = work.used.val + program.value.nodes.val.length ∧
      returned.max = work.max ∧ returned.cancellation = work.cancellation := by
  obtain ⟨computed, charged, allowance, control⟩ :=
    completed_classes program work returned output ordered budget preservesEmpty completed
  have recognized := TightClassificationSemantics.classes_exact program.value.nodes.val ordered
  exact ⟨computed.trans recognized, charged, allowance, control⟩

end TightClassification
