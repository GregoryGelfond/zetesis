import InsertionLoop

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Boundaries of the generic insertion loop

These supplied iterator dictionaries exercise the actual generated helper. An
invalid atom must stop before a nonreturning tail, and the first None must finish
even when another next call could yield an atom. The dictionaries are explicit
proof witnesses, not claims about an allocator or a particular Rust caller's
borrowed state.
-/
namespace InsertionBoundaryExample

/-- The first call yields atom zero; every later call diverges. -/
def invalidThenDiverging : core.iter.traits.iterator.Iterator Bool Usize := {
  next := fun later => if later then div else ok (some 0#usize, true) }

/-- The zero universe rejects its first atom without consulting the diverging
tail or reading the empty storage. No finite-input premise is needed. -/
theorem invalid_atom_avoids_diverging_tail :
    theory.insert_atoms invalidThenDiverging 0#usize false (Slice.new U64) =
      ok (.Err .Atom, Slice.new U64) := by
  unfold theory.insert_atoms
  rw [InsertionLoop.loop_unfold]
  have refused := InsertionLoop.body_refused invalidThenDiverging
    0#usize false true 0#usize (Slice.new U64) rfl (by decide)
  rw [refused]
  simp only [bind_tc_ok]

/-- The first call returns None; a subsequent call would yield atom zero. -/
def nonfused : core.iter.traits.iterator.Iterator Bool Usize := {
  next := fun later => if later then ok (some 0#usize, true) else ok (none, true) }

/-- The first None completes insertion without an extra next call. The helper
therefore needs no fused-iterator assumption, even for an empty universe. -/
theorem first_none_finishes_nonfused_input :
    theory.insert_atoms nonfused 0#usize false (Slice.new U64) =
      ok (.Ok (), Slice.new U64) := by
  unfold theory.insert_atoms
  rw [InsertionLoop.loop_unfold]
  have exhausted := InsertionLoop.body_exhausted nonfused
    0#usize false true (Slice.new U64) rfl
  rw [exhausted]
  simp only [bind_tc_ok]

end InsertionBoundaryExample
