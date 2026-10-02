import FixedLoop

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Accounting for actual generated evaluation

Every appended truth value corresponds to one admitted node tick. This module
extracts the work receipt from the proved generated evaluator trace, for both
completed and stopped source results. It takes an actual return equation, not
an assumed trace or agreement with mathematical evaluation.

The current fixed-observation and sequence-model boundaries remain: charged
work is not elapsed time, output length is not physical capacity, and these
statements do not establish allocator or concurrent atomic behavior.
-/
namespace EvaluationAccounting

/-- The logical work receipt of evaluation from its actual empty-prefix setup.
The output length counts admitted ticks. Success fills the entire node table;
a typed stop leaves a strictly shorter prefix. Limits and subset count are
unchanged, regardless of the source result. -/
structure Receipt (program : theory.Theory) (before : oracle.Work)
    (result : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work) : Prop where
  limits : after.limits = before.limits
  subsets : after.statistics.subsets = before.statistics.subsets
  work : after.statistics.work.val = before.statistics.work.val + output.val.length
  bounded : output.val.length ≤ program.value.nodes.val.length
  boundary : match result with
    | .Ok _ => output.val.length = program.value.nodes.val.length
    | .Err _ => output.val.length < program.value.nodes.val.length

/-- Every actual typed evaluator result has the exact logical work receipt.
Old output contents need no invariant because generated setup clears them.

Proof: construct the actual evaluation trace and its preservation result using
`evaluate_refines`. Identify its result with the supplied actual return equation.
The preserved truth-prefix length is precisely the trace's charged step count;
substitute that equality into the resource and completion fields.
-/
theorem returned_receipt (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (oldOutput : alloc.vec.Vec Bool) (before : oracle.Work)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (covered : ∀ mask ∈ frozen, program.value.nodes.val.length ≤ mask.val.length)
    (result : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (returned : oracle.evaluate program candidate frozen oldOutput before =
      ok (result, output, after)) :
    Receipt program before result output after := by
  obtain ⟨count, outcome, executed, _, preserved⟩ :=
    FixedEvaluationLoop.evaluate_refines program candidate frozen oldOutput before
      stored ordered covered
  have same : (outcome.result, outcome.output, outcome.work) = (result, output, after) :=
    Result.ok_injective (executed.symm.trans returned)
  have sameResult : outcome.result = result := congrArg Prod.fst same
  have sameOutput : outcome.output = output := congrArg (fun value => value.2.1) same
  have sameWork : outcome.work = after := congrArg (fun value => value.2.2) same
  have countBound : count ≤ program.value.nodes.val.length := by
    simpa only [ReductTrace.initialState, EvaluatorSetup.initialCursor, Nat.zero_add]
      using preserved.bounded
  have outputPrefix : output.val = EvaluationSpecification.prefixValues candidate frozen
      program.value.nodes.val count := by
    simpa only [sameOutput, ReductTrace.initialState, EvaluatorSetup.initialCursor,
      Nat.zero_add] using preserved.values
  have countLength : output.val.length = count := by
    rw [outputPrefix]
    exact EvaluationSpecification.prefix_length candidate frozen program.value.nodes.val
      count countBound
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  · simpa only [sameWork, ReductTrace.initialState] using preserved.limits
  · simpa only [sameWork, ReductTrace.initialState] using preserved.subsets
  · simpa only [sameWork, ReductTrace.initialState, countLength] using preserved.work
  · omega
  · have boundary := preserved.boundary
    rw [sameResult] at boundary
    cases result <;>
      simpa only [ReductTrace.initialState, EvaluatorSetup.initialCursor,
        Nat.zero_add, countLength] using boundary

/-- A completed actual evaluation charges exactly one unit per node, independent
of old output contents and node truth. This is an accounting result conditional
on actual completion, not a claim that every supplied allowance completes. -/
theorem completed_work (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (oldOutput : alloc.vec.Vec Bool) (before : oracle.Work)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (covered : ∀ mask ∈ frozen, program.value.nodes.val.length ≤ mask.val.length)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (completed : oracle.evaluate program candidate frozen oldOutput before =
      ok (core.result.Result.Ok (), output, after)) :
    after.statistics.work.val = before.statistics.work.val + program.value.nodes.val.length := by
  have receipt : Receipt program before (.Ok ()) output after :=
    returned_receipt program candidate frozen oldOutput before stored ordered covered
      (.Ok ()) output after completed
  exact receipt.work.trans (congrArg (before.statistics.work.val + ·) receipt.boundary)

/-- Every actual typed return charges at most the node count and never reduces
work. This includes stopped prefixes, without converting a stop into completion.
-/
theorem returned_work_bound (program : theory.Theory) (candidate : theory.Interpretation)
    (frozen : Option (Slice Bool)) (oldOutput : alloc.vec.Vec Bool) (before : oracle.Work)
    (stored : Membership.Represented candidate)
    (ordered : EvaluationSpecification.Ordered program.value.nodes.val)
    (covered : ∀ mask ∈ frozen, program.value.nodes.val.length ≤ mask.val.length)
    (result : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (output : alloc.vec.Vec Bool) (after : oracle.Work)
    (returned : oracle.evaluate program candidate frozen oldOutput before =
      ok (result, output, after)) :
    before.statistics.work.val ≤ after.statistics.work.val ∧
      after.statistics.work.val ≤ before.statistics.work.val + program.value.nodes.val.length := by
  have receipt : Receipt program before result output after :=
    returned_receipt program candidate frozen oldOutput before stored ordered covered
      result output after returned
  have counted : after.statistics.work.val = before.statistics.work.val + output.val.length :=
    receipt.work
  have bounded : output.val.length ≤ program.value.nodes.val.length := receipt.bounded
  omega

end EvaluationAccounting
