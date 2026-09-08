import Zetesis.ChoiceIntervals

/-!
# Cardinality consequences of finite group capacities

A complete distinct member set partitioned into groups has an additive selected
count. If the total is at least L, the chosen group must contribute at least
L minus the sum of all other capacities. Natural subtraction clips at zero.

These laws use lists and therefore count occurrences. A source or runtime
partition must separately establish unique identity and complete coverage before
these counts represent set cardinalities. Capacities are explicit logical
premises; validating a partition's shape does not prove that a theory entails
them. Common activation, concrete source recognition, machine arithmetic,
resource accounting and Rust candidate-search refinement remain unproved.
-/

namespace Zetesis.PartitionCapacities

open ChoiceIntervals
universe u
variable {α : Type u}

/-- Each group retains its member carrier and stated upper capacity. -/
abbrev Group (α : Type u) := List α × Nat

/-- Total stated capacity, including capacities of empty groups. -/
def capacity (groups : List (Group α)) : Nat := (groups.map Prod.snd).sum

/-- Complete member occurrences in the supplied group order. -/
def members (groups : List (Group α)) : List α :=
  (groups.map Prod.fst).flatten

/-- Selected occurrences add across concatenated member carriers. -/
theorem count_append (selected : α → Prop) (left right : List α) :
    count selected (left ++ right) = count selected left + count selected right := by
  classical
  simp [count, List.filter_append, List.length_append]

/-- Flattening a group list preserves the sum of its selected occurrences. -/
theorem count_members (selected : α → Prop) (groups : List (Group α)) :
    count selected (members groups) =
      (groups.map (fun group => count selected group.1)).sum := by
  induction groups with
  | nil => simp [members, count]
  | cons group groups ih =>
    simpa [members, count_append] using congrArg (count selected group.1 + ·) ih

/-- The sum of group capacities bounds the complete selected count. -/
theorem capacity_bounds (selected : α → Prop) (groups : List (Group α))
    (bounded : ∀ group ∈ groups, count selected group.1 ≤ group.2) :
    count selected (members groups) ≤ capacity groups := by
  rw [count_members]
  induction groups with
  | nil => simp [capacity]
  | cons group groups ih =>
    have headBound := bounded group (by simp)
    have restBound := ih (fun other inside => bounded other (by simp [inside]))
    simpa [capacity] using Nat.add_le_add headBound restBound

/-- A distinguished group's lower bound follows from the other capacities.
    The carrier equation is an explicit coverage/association premise. -/
theorem local_lower (selected : α → Prop) (all chosen : List α)
    (others : List (Group α)) (lower : Nat)
    (coverage : count selected all =
      count selected chosen + count selected (members others))
    (total : lower ≤ count selected all)
    (bounded : ∀ group ∈ others, count selected group.1 ≤ group.2) :
    lower - capacity others ≤ count selected chosen := by
  have otherBound := capacity_bounds selected others bounded
  omega

/-- An exceeded total capacity contradicts the stated premises.
    This is not a claim that an unexamined original theory is inconsistent. -/
theorem inconsistent_premises (selected : α → Prop)
    (groups : List (Group α)) (lower : Nat)
    (bounded : ∀ group ∈ groups, count selected group.1 ≤ group.2)
    (total : lower ≤ count selected (members groups))
    (exceeded : capacity groups < lower) : False := by
  have upper := capacity_bounds selected groups bounded
  omega

/-- A total filling every capacity forces the chosen group to fill its own. -/
theorem full_capacity (selected : α → Prop) (all chosen : List α)
    (others : List (Group α)) (upper : Nat)
    (coverage : count selected all =
      count selected chosen + count selected (members others))
    (total : upper + capacity others ≤ count selected all)
    (chosenBound : count selected chosen ≤ upper)
    (bounded : ∀ group ∈ others, count selected group.1 ≤ group.2) :
    count selected chosen = upper := by
  have lower := local_lower selected all chosen others
    (upper + capacity others) coverage total bounded
  omega

end Zetesis.PartitionCapacities
