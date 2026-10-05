import TickProjection

open Aeneas Aeneas.Std Result Aeneas.Data.Coinductive
open ZetesisExtract RuntimeEffects

/-!
# Observation progress independent of poll outcome

Every returning poll reads cancellation, even when it refuses. Every returning
tick reaches that poll before its quota test. These structural progress facts
therefore apply to typed refusals without fixed-clear or semantic assumptions.
-/
namespace RuntimeTickOrigin

/-- A returning poll has consumed its initial cancellation read. This statement
does not constrain the response or assume the poll succeeded. -/
theorem poll_nonempty (control : zetesis_cpu.cancellation.Cancellation)
    (events : List Event) (answer : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (run : Runs (ContextEvents.poll control) events answer) : events ≠ [] := by
  rw [ContextEvents.poll_reads] at run
  exact ControlReads.nonempty _ _ _ events answer run

/-- Every returning tick consumes a read, whether it succeeds, observes a stop,
or refuses its work allowance. Invert the checked tick's initial poll and retain
its nonempty prefix in the complete event history. -/
theorem tick_nonempty (before after : oracle.Work) (events : List Event)
    (answer : core.result.Result Unit zetesis_cpu.cancellation.Stop)
    (run : Runs (ContextEvents.tick before) events (answer, after)) : events ≠ [] := by
  rw [TickProjection.tick_factors] at run
  obtain ⟨first, last, polled, history, pollRun, _⟩ := RuntimeRuns.bind_inv _ _ _ _ run
  have progress : first ≠ [] := poll_nonempty before.cancellation first polled pollRun
  intro empty
  have impossible : first = [] := (List.append_eq_nil_iff.mp (history.symm.trans empty)).1
  exact progress impossible

end RuntimeTickOrigin
