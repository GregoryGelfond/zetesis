import RootScan

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisExtract

/-!
# Root order and empty scans in the actual generated function

These examples use fixed observation tokens and the unchanged generated root
scan. They make no concurrent-runtime or owner-identity claim. The first checks
that empty roots require no poll; the second checks that order and repeated root
occurrences determine both the witness and charged work.
-/
namespace RootScanExample

/-- Empty node and root storage supplies no asserted formula to scan. -/
def emptyProgram : theory.Theory := {
  value := {
    atoms := 0#usize
    nodes := alloc.vec.Vec.new theory.Node
    roots := alloc.vec.Vec.new Usize } }

/-- Both cancellation and an exhausted work allowance would refuse a tick. -/
def refusedWork : oracle.Work := {
  limits := { max_work := 0#u64, max_subsets := 0#u64 }
  cancellation := { cancelled := { value := { nextRead := true } }, deadline := none }
  statistics := { work := 0#u64, subsets := 7#u64 } }

/-- The actual entry function returns no failed root on empty roots, retaining
the entire work record despite cancellation and no remaining allowance. -/
theorem empty_roots_do_not_poll :
    oracle.failed_root emptyProgram (Slice.new Bool) refusedWork =
      ok (core.result.Result.Ok none, refusedWork) := by
  rw [FixedRootScan.failed_root_from_start]
  rw [FixedRootScan.loop_unfold]
  rw [FixedRootScan.body_exhausted _ _ _
    (by simp [FixedRootScan.initialCursor, emptyProgram, alloc.vec.Vec.deref])]
  simp only [bind_tc_ok]

/-- Nodes zero, one and three are false; node two is the true implication
false→false. The roots repeat node two before visiting three, with false node zero
last. No sorting or deduplication is part of this stored root sequence. -/
def orderedProgram : theory.Theory := {
  value := {
    atoms := 0#usize
    nodes := alloc.vec.Vec.from [.False, .False, .Implies 0#usize 0#usize, .False] (by scalar_tac)
    roots := alloc.vec.Vec.from [2#usize, 2#usize, 3#usize, 0#usize] (by scalar_tac) } }

/-- The supplied complete truth table has true only at node two. -/
def values : Slice Bool := Slice.from [false, false, true, false] (by scalar_tac)

/-- The root iterator retains the entire source sequence while advancing its
position. This constructor is only for spelling the concrete example states. -/
def cursor (position : Nat) : core.slice.iter.Iter Usize := {
  slice := alloc.vec.Vec.deref orderedProgram.value.roots
  i := position }

/-- Clear fixed observations and three available total work units. The subset
counter is unrelated to root visits and must remain seven. -/
def clearWork (charged : U64) : oracle.Work := {
  limits := { max_work := 3#u64, max_subsets := 0#u64 }
  cancellation := { cancelled := { value := { nextRead := false } }, deadline := none }
  statistics := { work := charged, subsets := 7#u64 } }

/-- Two occurrences of true root two consume two ticks; false root three consumes
the third and is returned before false root zero. Thus the witness follows source
order rather than the smallest failing node identifier or its position two, and
duplicates retain their work cost. The inclusive allowance of three admits this
result. -/
theorem first_failure_follows_root_order :
    oracle.failed_root orderedProgram values (clearWork 0#u64) =
      ok (core.result.Result.Ok (some 3#usize), clearWork 3#u64) := by
  have tickNext (before after : U64) (within : before.val < 3)
      (increment : after.val = before.val + 1) :
      oracle.Work.tick (clearWork before) =
        ok (core.result.Result.Ok (), clearWork after) := by
    obtain ⟨next, advanced, ticked⟩ := EvaluatorControl.tick_advances
      (clearWork before) rfl (by simpa [clearWork] using within)
    have same : next = after := UScalar.eq_of_val_eq (advanced.trans increment.symm)
    simpa only [same, clearWork] using ticked
  have tickZero : oracle.Work.tick (clearWork 0#u64) =
      ok (core.result.Result.Ok (), clearWork 1#u64) := tickNext _ _ (by decide) (by decide)
  have tickOne : oracle.Work.tick (clearWork 1#u64) =
      ok (core.result.Result.Ok (), clearWork 2#u64) := tickNext _ _ (by decide) (by decide)
  have tickTwo : oracle.Work.tick (clearWork 2#u64) =
      ok (core.result.Result.Ok (), clearWork 3#u64) := tickNext _ _ (by decide) (by decide)
  have first : oracle.failed_root_loop.body values (cursor 0) (clearWork 0#u64) =
      ok (.cont (cursor 1, clearWork 1#u64)) := by
    have step := FixedRootScan.body_tested values (cursor 0)
      (clearWork 0#u64) (clearWork 1#u64)
      (by simp [cursor, orderedProgram, alloc.vec.Vec.deref])
      (by simp [cursor, orderedProgram, values, alloc.vec.Vec.deref]) tickZero
    simpa [cursor, orderedProgram, FixedRootScan.truth, values, alloc.vec.Vec.deref] using step
  have second : oracle.failed_root_loop.body values (cursor 1) (clearWork 1#u64) =
      ok (.cont (cursor 2, clearWork 2#u64)) := by
    have step := FixedRootScan.body_tested values (cursor 1)
      (clearWork 1#u64) (clearWork 2#u64)
      (by simp [cursor, orderedProgram, alloc.vec.Vec.deref])
      (by simp [cursor, orderedProgram, values, alloc.vec.Vec.deref]) tickOne
    simpa [cursor, orderedProgram, FixedRootScan.truth, values, alloc.vec.Vec.deref] using step
  have third : oracle.failed_root_loop.body values (cursor 2) (clearWork 2#u64) =
      ok (.done (.Ok (some 3#usize), clearWork 3#u64)) := by
    have step := FixedRootScan.body_tested values (cursor 2)
      (clearWork 2#u64) (clearWork 3#u64)
      (by simp [cursor, orderedProgram, alloc.vec.Vec.deref])
      (by simp [cursor, orderedProgram, values, alloc.vec.Vec.deref]) tickTwo
    simpa [cursor, orderedProgram, FixedRootScan.truth, values, alloc.vec.Vec.deref] using step
  rw [FixedRootScan.failed_root_from_start]
  change oracle.failed_root_loop (cursor 0) values (clearWork 0#u64) = _
  rw [FixedRootScan.loop_unfold, first]
  simp only [bind_tc_ok]
  rw [FixedRootScan.loop_unfold, second]
  simp only [bind_tc_ok]
  rw [FixedRootScan.loop_unfold, third]
  simp only [bind_tc_ok]

end RootScanExample
