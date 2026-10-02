import SubsetQuery
import Zetesis.SubsetCounter

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisExtract

/-!
# Transitions of the extracted countermodel search

The outer loop first tests the population guard. A proper-subset query either
finds a witness, stops, or returns false and advances the counter. These laws
retain the actual intermediate work records and partial carry results. They do
not assume that a selected coordinate list covers the candidate; the coverage
argument belongs to the subsequent counter invariant.
-/
namespace CountermodelSteps

/-- Search stops before polling or querying when the population guard is false.
    This includes the empty candidate. Only the counted-selection invariant can
    justify interpreting this branch as full-state exhaustion. -/
theorem exhausted (program : theory.Theory) (frozen : Slice Bool)
    (selected : Slice Usize) (subset : theory.Interpretation)
    (values : alloc.vec.Vec Bool) (work : oracle.Work) (present : Usize)
    (full : selected.val.length ≤ present.val) :
    oracle.find_countermodel_loop.body program frozen selected subset values work present =
      ok (.done (subset, values, work, core.result.Result.Ok false)) := by
  simp [oracle.find_countermodel_loop.body, UScalar.lt_equiv,
    Nat.not_lt.mpr full]

/-- An admitted query that returns a typed stop terminates the outer loop with
    that exact stop, output and work record. No carry is executed. -/
theorem query_stopped (program : theory.Theory) (frozen : Slice Bool)
    (selected : Slice Usize) (subset : theory.Interpretation)
    (values output : alloc.vec.Vec Bool) (work after : oracle.Work) (present : Usize)
    (reason : zetesis_cpu.cancellation.Stop)
    (proper : present.val < selected.val.length)
    (queried : oracle.check_subset program subset frozen values work =
      ok (core.result.Result.Err reason, output, after)) :
    oracle.find_countermodel_loop.body program frozen selected subset values work present =
      ok (.done (subset, output, after, core.result.Result.Err reason)) := by
  simp [oracle.find_countermodel_loop.body, UScalar.lt_equiv,
    proper, queried]

/-- A true query returns its tested interpretation immediately. It does not
    require work or quota for a following carry or another query. -/
theorem witness (program : theory.Theory) (frozen : Slice Bool)
    (selected : Slice Usize) (subset : theory.Interpretation)
    (values output : alloc.vec.Vec Bool) (work after : oracle.Work) (present : Usize)
    (proper : present.val < selected.val.length)
    (queried : oracle.check_subset program subset frozen values work =
      ok (core.result.Result.Ok true, output, after)) :
    oracle.find_countermodel_loop.body program frozen selected subset values work present =
      ok (.done (subset, output, after, core.result.Result.Ok true)) := by
  simp [oracle.find_countermodel_loop.body, UScalar.lt_equiv,
    proper, queried]

/-- A stopped carry returns its partially updated interpretation and final work.
    The preceding query's output is retained. A false query followed by a stop
    is not a successful claim that every proper subset has been refuted. -/
theorem carry_stopped (program : theory.Theory) (frozen : Slice Bool)
    (selected : Slice Usize) (subset interrupted : theory.Interpretation)
    (values output : alloc.vec.Vec Bool) (work middle after : oracle.Work)
    (present partialCount : Usize) (reason : zetesis_cpu.cancellation.Stop)
    (proper : present.val < selected.val.length)
    (queried : oracle.check_subset program subset frozen values work =
      ok (core.result.Result.Ok false, output, middle))
    (carried : oracle.advance_subset selected subset present middle =
      ok (core.result.Result.Err reason, interrupted, partialCount, after)) :
    oracle.find_countermodel_loop.body program frozen selected subset values work present =
      ok (.done (interrupted, output, after, core.result.Result.Err reason)) := by
  simp [oracle.find_countermodel_loop.body, UScalar.lt_equiv,
    proper, queried, carried]

/-- Only a false completed query and a successful carry continue the search.
    Every returned field is passed to the next invocation without reset. -/
theorem continued (program : theory.Theory) (frozen : Slice Bool)
    (selected : Slice Usize) (subset next : theory.Interpretation)
    (values output : alloc.vec.Vec Bool) (work middle after : oracle.Work)
    (present nextCount : Usize)
    (proper : present.val < selected.val.length)
    (queried : oracle.check_subset program subset frozen values work =
      ok (core.result.Result.Ok false, output, middle))
    (carried : oracle.advance_subset selected subset present middle =
      ok (core.result.Result.Ok (), next, nextCount, after)) :
    oracle.find_countermodel_loop.body program frozen selected subset values work present =
      ok (.cont (next, output, after, nextCount)) := by
  simp [oracle.find_countermodel_loop.body, UScalar.lt_equiv,
    proper, queried, carried]

end CountermodelSteps
