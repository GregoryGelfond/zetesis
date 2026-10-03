import SearchEventsProjection
import RuntimeLoopOrigin
import RuntimeTickOrigin

open Aeneas Aeneas.Std Result ControlFlow Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# Where a subset-search step stops

An actual terminal source stop comes from its reached reduct query, or from the
carry after a completed false query. The receipt retains the same reason and
every intermediate field, with the exact consumed observations in source order.
Exhaustion and a found countermodel cannot produce a typed stop.
-/
namespace SearchRefusal

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

/-- The two possible reached source refusals in one search body. The carry
case retains the actual preceding false query; it cannot invent a carry reached
after a true query, exhaustion or an earlier refusal. -/
inductive Receipt (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Usize)
    (subset : theory.Interpretation) (values : alloc.vec.Vec Bool)
    (work : oracle.Work) (present : Usize) (reason : zetesis_cpu.cancellation.Stop) :
    List Event → theory.Interpretation → alloc.vec.Vec Bool → oracle.Work → Prop where
  | query (events : List Event) (output : alloc.vec.Vec Bool) (after : oracle.Work)
      (proper : present < Slice.len selected)
      (refused : Runs (ReferenceEvents.checkSubset program subset frozen values work)
        events (.Err reason, output, after)) :
      Receipt program frozen selected subset values work present reason events subset output after
  | carry (before after : List Event) (output : alloc.vec.Vec Bool) (middle finalWork : oracle.Work)
      (returned : theory.Interpretation) (count : Usize)
      (proper : present < Slice.len selected)
      (queried : Runs (ReferenceEvents.checkSubset program subset frozen values work)
        before (.Ok false, output, middle))
      (refused : Runs (ReferenceEvents.advanceSubset selected subset present middle)
        after (.Err reason, returned, count, finalWork)) :
      Receipt program frozen selected subset values work present reason (before ++ after)
        returned output finalWork

/-- A stopped body exposes precisely one of the two source-order receipts.
Its final return consumes no extra observation after the refused phase. -/
theorem body_refused (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Usize)
    (subset returned : theory.Interpretation) (values output : alloc.vec.Vec Bool)
    (work after : oracle.Work) (present : Usize) (reason : zetesis_cpu.cancellation.Stop)
    (events : List Event)
    (run : Runs (CheckerContexts.searchBody ReferenceEvents.checkSubset ReferenceEvents.advanceSubset
      program frozen selected subset values work present) events
      (.done (returned, output, after, .Err reason))) :
    Receipt program frozen selected subset values work present reason events returned output after := by
  unfold CheckerContexts.searchBody at run
  dsimp only at run
  split at run
  next proper =>
    obtain ⟨queryEvents, tail, queried, partition, queryRun, following⟩ :=
      RuntimeRuns.bind_inv _ _ _ _ run
    rcases queried with ⟨answer, retained, middle⟩
    cases answer with
    | Err stop =>
      simp only [ContextEvents.lift_result, embed_ok] at following
      obtain ⟨same, silent⟩ := RuntimeRuns.returned_inv _ _ _ following
      have resultSame : (subset, retained, middle, core.result.Result.Err stop) =
          (returned, output, after, core.result.Result.Err reason) := ControlFlow.done.inj same
      cases resultSame
      rw [partition, silent, List.append_nil]
      exact .query queryEvents output after proper queryRun
    | Ok answer =>
      cases answer with
      | true =>
        simp only [↓reduceIte, ContextEvents.lift_result, embed_ok] at following
        have impossible := (RuntimeRuns.returned_inv _ _ _ following).1
        cases impossible
      | false =>
        simp only [Bool.false_eq_true, ↓reduceIte] at following
        obtain ⟨carryEvents, suffix, carried, carryPartition, carryRun, finished⟩ :=
          RuntimeRuns.bind_inv _ _ _ _ following
        rcases carried with ⟨answer, next, count, finalWork⟩
        cases answer with
        | Ok success =>
          simp only [ContextEvents.lift_result, embed_ok] at finished
          have impossible := (RuntimeRuns.returned_inv _ _ _ finished).1
          cases impossible
        | Err stop =>
          simp only [ContextEvents.lift_result, embed_ok] at finished
          obtain ⟨same, silent⟩ := RuntimeRuns.returned_inv _ _ _ finished
          have resultSame : (next, retained, finalWork, core.result.Result.Err stop) =
              (returned, output, after, core.result.Result.Err reason) := ControlFlow.done.inj same
          cases resultSame
          rw [partition, carryPartition, silent, List.append_nil]
          exact .carry queryEvents carryEvents output middle after returned count proper queryRun carryRun
  next exhausted =>
    have impossible := (RuntimeRuns.returned_inv _ _ _ run).1
    cases impossible

/-- Any returning reduct query has consumed its initial cancellation read,
regardless of its verdict or refusal. -/
theorem query_nonempty (program : theory.Theory) (subset : theory.Interpretation)
    (frozen : Slice Bool) (input output : alloc.vec.Vec Bool) (work after : oracle.Work)
    (answer : core.result.Result Bool zetesis_cpu.cancellation.Stop) (events : List Event)
    (run : Runs (ReferenceEvents.checkSubset program subset frozen input work) events (answer, output, after)) :
    events ≠ [] := by
  unfold ReferenceEvents.checkSubset CheckerContexts.subsetQuery at run
  obtain ⟨before, tail, polled, partition, pollRun, _⟩ := RuntimeRuns.bind_inv _ _ _ _ run
  have consumed := RuntimeTickOrigin.poll_nonempty work.cancellation before polled pollRun
  intro vacant
  exact consumed (List.append_eq_nil_iff.mp (partition.symm.trans vacant)).1

/-- Every outer continuation includes a returning reduct query and therefore
consumes observations, without a semantic or fixed-control premise. -/
theorem body_nonempty (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Usize)
    (subset : theory.Interpretation) (values : alloc.vec.Vec Bool) (work : oracle.Work)
    (present : Usize) (events : List Event) (next : SearchEventsProjection.State)
    (run : Runs (CheckerContexts.searchBody ReferenceEvents.checkSubset ReferenceEvents.advanceSubset
      program frozen selected subset values work present) events (.cont next)) : events ≠ [] := by
  unfold CheckerContexts.searchBody at run
  dsimp only at run
  split at run
  next proper =>
    obtain ⟨before, tail, queried, partition, queryRun, _⟩ := RuntimeRuns.bind_inv _ _ _ _ run
    rcases queried with ⟨answer, output, after⟩
    have consumed := query_nonempty program subset frozen values output work after answer before queryRun
    intro vacant
    exact consumed (List.append_eq_nil_iff.mp (partition.symm.trans vacant)).1
  next exhausted =>
    have impossible := (RuntimeRuns.returned_inv _ _ _ run).1
    cases impossible

/-- A typed search refusal comes from a terminal body at an actually reached
state. The receipt retains the entire executed prefix and the same terminal
query/carry stop, so an unreachable local refusal cannot explain the run. -/
theorem loop_refused (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Usize)
    (subset returned : theory.Interpretation) (values output : alloc.vec.Vec Bool)
    (work after : oracle.Work) (present : Usize) (reason : zetesis_cpu.cancellation.Stop)
    (events : List Event)
    (run : Runs (CheckerContexts.searchLoop RuntimeEffects.loop
      (CheckerContexts.searchBody ReferenceEvents.checkSubset ReferenceEvents.advanceSubset)
      program frozen selected subset values work present) events (returned, output, after, .Err reason)) :
    let body := fun state : SearchEventsProjection.State =>
      CheckerContexts.searchBody ReferenceEvents.checkSubset ReferenceEvents.advanceSubset
        program frozen selected state.1 state.2.1 state.2.2.1 state.2.2.2
    ∃ before last reached, events = before ++ last ∧
      RuntimeLoopOrigin.Prefix body (subset, values, work, present) before reached ∧
      Receipt program frozen selected reached.1 reached.2.1 reached.2.2.1 reached.2.2.2 reason
        last returned output after := by
  dsimp only
  let body := fun state : SearchEventsProjection.State =>
    CheckerContexts.searchBody ReferenceEvents.checkSubset ReferenceEvents.advanceSubset
      program frozen selected state.1 state.2.1 state.2.2.1 state.2.2.2
  have progress : ∀ state observations next, Runs (body state) observations (.cont next) → observations ≠ [] := by
    intro state observations next called
    exact body_nonempty program frozen selected state.1 state.2.1 state.2.2.1 state.2.2.2 observations next called
  obtain ⟨before, last, reached, history, executed, terminal⟩ :=
    RuntimeLoopOrigin.completed_origin body progress (subset, values, work, present)
      events (returned, output, after, .Err reason) run
  exact ⟨before, last, reached, history, executed,
    body_refused program frozen selected reached.1 returned reached.2.1 output reached.2.2.1 after
      reached.2.2.2 reason last terminal⟩

/-- A refused search wrapper exposes the same refused loop run and observations.
Its source-level result rearrangement cannot change the reason or add work. -/
theorem search_refused (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Usize)
    (subset returned : theory.Interpretation) (values output : alloc.vec.Vec Bool)
    (work after : oracle.Work) (reason : zetesis_cpu.cancellation.Stop) (events : List Event)
    (run : Runs (ReferenceEvents.findCountermodel program frozen selected subset values work)
      events (.Err reason, returned, output, after)) :
    Runs (CheckerContexts.searchLoop RuntimeEffects.loop
      (CheckerContexts.searchBody ReferenceEvents.checkSubset ReferenceEvents.advanceSubset)
      program frozen selected subset values work 0#usize) events (returned, output, after, .Err reason) := by
  unfold ReferenceEvents.findCountermodel CheckerContexts.findCountermodel at run
  obtain ⟨before, suffix, result, partition, loopRun, following⟩ := RuntimeRuns.bind_inv _ _ _ _ run
  rcases result with ⟨tested, retained, counted, verdict⟩
  obtain ⟨same, silent⟩ := (RuntimeRuns.embed_iff _ _ _).mp following
  have exactResult : (verdict, tested, retained, counted) = (.Err reason, returned, output, after) :=
    Result.ok_injective same
  cases exactResult
  rw [partition, silent, List.append_nil]
  exact loopRun

end SearchRefusal
