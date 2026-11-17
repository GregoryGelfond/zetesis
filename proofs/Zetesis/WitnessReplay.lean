import Zetesis.DeltaRounds

/-!
# Ordered witness suffix transport

The join already identifies a prefix unchanged from its preceding complete row.
These laws justify retaining that prefix and transporting only the changed
suffix. They do not authenticate a relation owner, a row coordinate, a Rust
lifetime or the join's claimed prefix. The adapter must establish those separate
premises, including a zero prefix at each independently executed task boundary.
No allocation, cancellation, receipt or thread correctness is asserted here.
-/

namespace Zetesis.ParallelGrounding.WitnessReplay

universe u v
variable {Row : Type u} {Atom : Type v}

/-- Rebuild a witness without changing the order or multiplicity of occurrences. -/
def replay (previous : List Row) (unchanged : Nat) (suffix : List Row) : List Row :=
  previous.take unchanged ++ suffix

/-- The actual shared-prefix premise suffices to reconstruct the next complete
witness. Equality of row values must follow from authenticated source identity;
mere equality of integer positions across unrelated sources is not this premise. -/
theorem unchanged_prefix_reconstructs (previous next : List Row) (unchanged : Nat)
    (sharedPrefix : previous.take unchanged = next.take unchanged) :
    replay previous unchanged (next.drop unchanged) = next := by
  unfold replay
  rw [sharedPrefix]
  exact List.take_append_drop unchanged next

/-- The existing projection receives precisely its original witness. This is a
value law: effectful atom interning still occurs once, in the existing owner. -/
theorem unchanged_projection_exact (previous next : List Row) (unchanged : Nat)
    (sharedPrefix : previous.take unchanged = next.take unchanged)
    (project : List Row → Atom) :
    project (replay previous unchanged (next.drop unchanged)) = project next := by
  rw [unchanged_prefix_reconstructs previous next unchanged sharedPrefix]

/-- A new task's first completion must reset its prefix; it cannot depend on the
last witness produced by another task or on completion order. -/
theorem task_boundary_reset (previous next : List Row) :
    replay previous 0 next = next := by
  simp [replay]

/-- A replay may carry no changed rows, preserving duplicate completions exactly
when the whole preceding witness is retained. Deduplication belongs elsewhere. -/
theorem unchanged_completion_retained (previous : List Row) :
    replay previous previous.length [] = previous := by
  simp [replay]

/-- A claimed prefix without the shared-prefix premise can change the projected
witness, even though lengths and all numeric coordinates fit their bounds. -/
theorem unchecked_prefix_is_unsound :
    replay ([1, 2] : List Nat) 1 ([9, 3].drop 1) ≠ [9, 3] := by
  decide

end Zetesis.ParallelGrounding.WitnessReplay
