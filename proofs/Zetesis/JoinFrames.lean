import Zetesis.WorldMasks

/-!
Reusable prefix frames for positive source joins.

The frame at depth `d` represents the worlds satisfying the first `d` chosen
positive rows. Preparing a new join replaces the root with all worlds. Extending
one row replaces its child frame with the intersection of its current parent and
that row's current membership. Previous contents of other frames may remain in
storage, but cannot affect the completed prefix under this write-before-read
schedule.

These executable definitions operate on mathematical predicate stores. They
justify retaining storage while replacing truth, not caching previous membership.
Fresh world snapshots are explicit. Array bounds, machine bit operations, packed
tail masks, allocation lifetime/budgets and the Rust visitor's scheduling are
separate, unproved implementation correspondences.
-/

namespace Zetesis.JoinFrames

open WorldMasks

universe u v

variable {α : Type u} {ω : Type v}

/-- A depth-indexed store of world predicates. Unused frames may be arbitrary. -/
abbrev Frames (ω : Type v) := Nat → ω → Prop

/-- Replace only the root. No previous root truth survives a new join. -/
def reset (frames : Frames ω) : Frames ω :=
  fun depth world => if depth = 0 then True else frames depth world

/-- Replace the next frame using the current parent and current row membership.
The old child is deliberately not an operand of the intersection. -/
def extend (frames : Frames ω) (depth : Nat) (row : ω → Prop) : Frames ω :=
  fun target world =>
    if target = depth + 1 then frames depth world ∧ row world else frames target world

/-- A finite schedule advances one depth after each complete child overwrite. -/
def walk (worlds : ω → Atoms α) :
    List α → Nat → Frames ω → Frames ω
  | [], _, frames => frames
  | row :: rest, depth, frames =>
      walk worlds rest (depth + 1) (extend frames depth (fun world => worlds world row))

/-- Preparing a join gives its empty positive prefix every world. -/
theorem reset_root_exact (frames : Frames ω) (world : ω) :
    reset frames 0 world ↔ True := by
  simp only [reset, ↓reduceIte]

/-- A completed child write depends on its current parent and row, not on the
previous contents at the child depth. -/
theorem child_overwrite_exact (frames : Frames ω) (depth : Nat)
    (row : ω → Prop) (world : ω) :
    extend frames depth row (depth + 1) world ↔ frames depth world ∧ row world := by
  simp only [extend, ↓reduceIte]

/-- After a complete prefix schedule, the final frame is the initial parent
intersected with all newly supplied row memberships. Arbitrary deeper contents
are overwritten before they can contribute to that result. -/
theorem completed_walk_exact (worlds : ω → Atoms α) (rows : List α)
    (depth : Nat) (frames : Frames ω) (world : ω) :
    walk worlds rows depth frames (depth + rows.length) world ↔
      frames depth world ∧ WorldMasks.Membership worlds rows world := by
  induction rows generalizing depth frames with
  | nil =>
    simp only [walk, List.length_nil, Nat.add_zero, WorldMasks.Membership, and_true]
  | cons row rest induction =>
    have tailExact :
        walk worlds rest (depth + 1)
            (extend frames depth (fun candidate => worlds candidate row))
            (depth + 1 + rest.length) world ↔
          (frames depth world ∧ worlds world row) ∧ WorldMasks.Membership worlds rest world := by
      simpa only [child_overwrite_exact] using
        induction (depth + 1) (extend frames depth (fun candidate => worlds candidate row))
    have sameIndex : depth + (row :: rest).length = depth + 1 + rest.length := by
      simp only [List.length_cons]
      omega
    simpa only [walk, sameIndex, WorldMasks.Membership, and_assoc] using tailExact

/-- Reset plus complete overwrite gives exactly the current positive prefix,
even when all retained frame contents came from another join or snapshot. -/
theorem prepared_prefix_exact (worlds : ω → Atoms α) (rows : List α)
    (frames : Frames ω) (world : ω) :
    walk worlds rows 0 (reset frames) rows.length world ↔
      WorldMasks.Membership worlds rows world := by
  have completed :
      walk worlds rows 0 (reset frames) (0 + rows.length) world ↔
        reset frames 0 world ∧ WorldMasks.Membership worlds rows world :=
    completed_walk_exact worlds rows 0 (reset frames) world
  simpa only [Nat.zero_add, reset_root_exact, true_and] using completed

/-- Two different retained stores give the same completed positive membership
when both are reset and traverse the same current world snapshots. -/
theorem retained_frames_irrelevant (worlds : ω → Atoms α) (rows : List α)
    (before other : Frames ω) (world : ω) :
    walk worlds rows 0 (reset before) rows.length world ↔
      walk worlds rows 0 (reset other) rows.length world := by
  exact (prepared_prefix_exact worlds rows before world).trans
    (prepared_prefix_exact worlds rows other world).symm

/-- Negative control: a stale empty root erases a currently true row if a join
does not reset it. Storage reuse alone is not a semantic preservation argument. -/
theorem stale_root_can_erase_current_truth :
    WorldMasks.Membership (fun _ _ : Unit => True) [()] () ∧
      ¬ walk (fun _ _ : Unit => True) [()] 0 (fun _ _ => False) 1 () := by
  have current : WorldMasks.Membership (fun _ _ : Unit => True) [()] () :=
    ⟨True.intro, True.intro⟩
  have erased : ¬ walk (fun _ _ : Unit => True) [()] 0 (fun _ _ => False) 1 () := by
    simp only [walk, extend, ↓reduceIte, false_and, not_false_eq_true]
  exact ⟨current, erased⟩

end Zetesis.JoinFrames
