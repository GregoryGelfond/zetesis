import Zetesis.ObjectiveEligibility

/-!
# Immutable selected rows, current-round Activity

The objects are ordered source occurrences, their scalar selection and a stateful
producer consumer. Exact occurrence and selection agreement preserves replay into
the same current state, then each complete producer pass and every fixed finite
trace. Duplicate occurrences and producer order remain significant. The final
counterexample shows why equal rows do not justify retaining evaluated Activity.

The only imported capability is `ObjectiveEligibility`; it supplies the activity
values used in that counterexample. Rust must establish completed immutable
support, exact selected occurrence order, ownership and decoded row identity,
current-round Activity, and successful sealing. No theorem here establishes
scalar purity, allocation, resource receipts, cancellation, convergence, or an
end-to-end grounding refinement. See the producer-row-reuse reading guide and
the manual's implementation correspondence section.
-/

namespace Zetesis.SourceActivity.CyclicRows

universe u v w
variable {Row : Type u} {State : Type v} {Receipt : Type w}

/-- Selection depends only on a fixed source and its scalar predicate. Activity
is intentionally absent from this function's inputs. Duplicate rows remain. -/
def selected (source : List Row) (keep : Row → Bool) : List Row :=
  source.filter keep

/-- Equal source occurrences and scalar decisions give exactly the same selected
stream, not merely the same set. The actual Join must establish both premises. -/
theorem selected_occurrences_exact (source retained : List Row)
    (keep keepAgain : Row → Bool) (sameSource : retained = source)
    (sameScalar : ∀ row, keepAgain row = keep row) :
    selected retained keepAgain = selected source keep := by
  subst retained
  have sameFunction : keepAgain = keep := funext sameScalar
  rw [sameFunction]

/-- An exact decoded stream can be fed to the same ordered stateful consumer.
Its state can include errors and staged Activity; no cached truth is assumed. -/
theorem replay_current_consumer_exact (original decoded : List Row)
    (exactRows : decoded = original) (step : State → Row → State) (current : State) :
    decoded.foldl step current = original.foldl step current := by
  rw [exactRows]

/-- The entire ordered producer pass recomputes each fold from the current old
Activity. Its output is the same when every selected occurrence stream is exact. -/
def producerPass (rows : Nat → List Row) (order : List Nat)
    (step : State → Nat → Row → State) (current : State) : State :=
  order.foldl (fun state producer => (rows producer).foldl
    (fun state row => step state producer row) state) current

theorem whole_pass_exact (original decoded : Nat → List Row)
    (exactRows : ∀ producer, decoded producer = original producer)
    (order : List Nat) (step : State → Nat → Row → State) (current : State) :
    producerPass decoded order step current = producerPass original order step current := by
  have sameFunction : decoded = original := funext exactRows
  rw [sameFunction]

/-- Fixed-point progress and stopping inspect the current state after every
complete pass. Extensional equality of the pass preserves every finite trace. -/
def trace (next : State → State) : Nat → State → List State
  | 0, current => [current]
  | Nat.succ rounds, current => current :: trace next rounds (next current)

theorem refinement_trace_exact (ordinary replayed : State → State)
    (samePass : ∀ current, replayed current = ordinary current)
    (rounds : Nat) (initial : State) :
    trace replayed rounds initial = trace ordinary rounds initial := by
  have sameFunction : replayed = ordinary := funext samePass
  rw [sameFunction]

/-- Equal rows cannot justify caching their evaluated Activity. Even one exact
source occurrence can read a different current-round classification. -/
theorem rows_do_not_determine_activity :
    ([()] : List Unit).foldl
      (fun _ _ => ObjectiveEligibility.Activity.required) ObjectiveEligibility.Activity.absent ≠
    ([()] : List Unit).foldl
      (fun _ _ => ObjectiveEligibility.Activity.optional) ObjectiveEligibility.Activity.absent := by
  decide

end Zetesis.SourceActivity.CyclicRows
