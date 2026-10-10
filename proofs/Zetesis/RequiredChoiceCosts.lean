import Zetesis.ObjectiveDirections

/-!
# Necessary objective bounds from required choices

An active choice group requires at least one eligible objective key. Its least
weight therefore bounds that group's cost from below, provided the allocated
weights are nonnegative. Disjoint allocation matters: the partition below is a
permutation of the complete, already-coalesced key list, so a key cannot be
charged to two groups. Unallocated keys retain their exact contributions.
An allocated key can also retain its weight above the group's minimum. This
residual cost strengthens the floor without charging the required minimum twice.

These are candidate restrictions. They do not replace original satisfaction or
reduct minimality. Source choice capture, implication checking, finite-width
arithmetic, formula compilation and Rust execution remain separate obligations.
-/

namespace Zetesis.RequiredChoiceCosts

open AggregateAssignment (weightedSum)
open ObjectiveDirections (compareVectors)

universe u v w
variable {κ : Type u} {π : Type v} {α : Type w}

/-- A required group allocates complete objective keys, after duplicate-key
    coalescing. Activation belongs to the current interpretation. -/
structure Group (κ : Type u) where
  keys : List κ
  minimum : Int
  active : Bool

def contribution (weight : κ → Int) (eligible : κ → Bool) (key : κ) : Int :=
  if eligible key then weight key else 0

def floor (group : Group κ) : Int :=
  if group.active then group.minimum else 0

/-- A finite sum of nonnegative contributions is nonnegative. -/
theorem sum_nonnegative (value : κ → Int) (keys : List κ)
    (nonnegative : ∀ key ∈ keys, 0 ≤ value key) : 0 ≤ weightedSum value keys := by
  induction keys with
  | nil => simp [weightedSum]
  | cons key keys ih =>
    have first : 0 ≤ value key := nonnegative key (by simp)
    have rest : 0 ≤ weightedSum value keys :=
      ih (fun other member => nonnegative other (by simp [member]))
    simp only [weightedSum]
    omega

/-- Every member's nonnegative contribution is bounded by the complete sum. -/
theorem member_le_sum (value : κ → Int) (keys : List κ) (key : κ)
    (present : key ∈ keys) (nonnegative : ∀ other ∈ keys, 0 ≤ value other) :
    value key ≤ weightedSum value keys := by
  induction keys with
  | nil => simp at present
  | cons first rest ih =>
    have restNonnegative : ∀ other ∈ rest, 0 ≤ value other :=
      fun other member => nonnegative other (by simp [member])
    have restSum : 0 ≤ weightedSum value rest := sum_nonnegative value rest restNonnegative
    have firstValue : 0 ≤ value first := nonnegative first (by simp)
    rcases List.mem_cons.mp present with same | inRest
    · subst key
      simp only [weightedSum]
      omega
    · have inSum := ih inRest restNonnegative
      simp only [weightedSum]
      omega

/-- An active group has an eligible witness whose weight is at least the
    group's floor. The remaining contributions are nonnegative. An inactive
    group contributes the zero lower bound. -/
theorem group_floor_le (group : Group κ) (weight : κ → Int) (eligible : κ → Bool)
    (nonnegative : ∀ key ∈ group.keys, 0 ≤ weight key)
    (minimum : ∀ key ∈ group.keys, group.minimum ≤ weight key)
    (required : group.active = true → ∃ key ∈ group.keys, eligible key = true) :
    floor group ≤ weightedSum (contribution weight eligible) group.keys := by
  have termsNonnegative : ∀ key ∈ group.keys, 0 ≤ contribution weight eligible key := by
    intro key member
    have value := nonnegative key member
    unfold contribution
    split <;> omega
  cases active : group.active with
  | false =>
    simpa [floor, active] using sum_nonnegative _ group.keys termsNonnegative
  | true =>
    obtain ⟨key, member, enabled⟩ := required active
    have witness : contribution weight eligible key ≤
        weightedSum (contribution weight eligible) group.keys :=
      member_le_sum _ group.keys key member termsNonnegative
    have least := minimum key member
    simp only [contribution, enabled, ↓reduceIte] at witness
    simp only [floor, active, ↓reduceIte]
    omega

/-- Concatenating key lists adds their costs without removing occurrences. -/
theorem sum_append (value : κ → Int) (left right : List κ) :
    weightedSum value (left ++ right) = weightedSum value left + weightedSum value right := by
  induction left with
  | nil => simp [weightedSum]
  | cons key keys ih => simp [weightedSum, ih, Int.add_assoc]

/-- Summing pointwise sums is the sum of the two finite sums. -/
theorem sum_add (left right : κ → Int) (keys : List κ) :
    weightedSum (fun key => left key + right key) keys =
      weightedSum left keys + weightedSum right keys := by
  induction keys with
  | nil => simp [weightedSum]
  | cons key keys ih =>
    simp only [weightedSum, ih]
    omega

def residual (group : Group κ) (weight : κ → Int) (eligible : κ → Bool) : Int :=
  weightedSum (contribution (fun key => weight key - group.minimum) eligible) group.keys

def lowerCost (group : Group κ) (weight : κ → Int) (eligible : κ → Bool) : Int :=
  floor group + residual group weight eligible

/-- Subtracting a certified minimum from every allocated weight leaves
    nonnegative residual contributions. -/
theorem residual_nonnegative (group : Group κ) (weight : κ → Int) (eligible : κ → Bool)
    (minimum : ∀ key ∈ group.keys, group.minimum ≤ weight key) :
    0 ≤ residual group weight eligible := by
  apply sum_nonnegative
  intro key member
  have least := minimum key member
  dsimp only [contribution]
  split <;> omega

/-- Every eligible key pays one minimum plus its residual weight. An active
    group has at least one such key, so replacing their minimum payments by
    one required payment cannot increase cost. Inactive groups pay no floor. -/
theorem group_residual_le (group : Group κ) (weight : κ → Int) (eligible : κ → Bool)
    (nonnegative : 0 ≤ group.minimum)
    (required : group.active = true → ∃ key ∈ group.keys, eligible key = true) :
    lowerCost group weight eligible ≤
      weightedSum (contribution weight eligible) group.keys := by
  have minimumCost := group_floor_le group (fun _ => group.minimum) eligible
    (fun _ _ => nonnegative) (fun _ _ => Int.le_refl _) required
  have decomposition : contribution weight eligible =
      fun key => contribution (fun key => weight key - group.minimum) eligible key +
        contribution (fun _ => group.minimum) eligible key := by
    funext key
    dsimp only [contribution]
    split <;> omega
  rw [decomposition, sum_add]
  unfold lowerCost residual
  omega

/-- Required payments and selected residual costs can be summed across the
    allocated groups. The following partition law establishes disjointness. -/
theorem residuals_le_allocated (groups : List (Group κ))
    (weight : κ → Int) (eligible : κ → Bool)
    (nonnegative : ∀ group ∈ groups, 0 ≤ group.minimum)
    (required : ∀ group ∈ groups,
      group.active = true → ∃ key ∈ group.keys, eligible key = true) :
    weightedSum (fun group => lowerCost group weight eligible) groups ≤
      weightedSum (contribution weight eligible) (groups.flatMap Group.keys) := by
  induction groups with
  | nil => simp [weightedSum]
  | cons group groups ih =>
    have first := group_residual_le group weight eligible
      (nonnegative group (by simp)) (required group (by simp))
    have rest := ih (fun other member => nonnegative other (by simp [member]))
      (fun other member => required other (by simp [member]))
    simp only [weightedSum, List.flatMap_cons, sum_append]
    omega

/-- Disjoint groups may retain residual costs as well as their required
    minima. Exact unallocated contributions, including signed weights, complete
    a lower bound on the objective's already-coalesced key list. -/
theorem partition_residual_bound (keys remaining : List κ) (groups : List (Group κ))
    (weight : κ → Int) (eligible : κ → Bool)
    (partition : (groups.flatMap Group.keys ++ remaining).Perm keys)
    (nonnegative : ∀ group ∈ groups, 0 ≤ group.minimum)
    (required : ∀ group ∈ groups,
      group.active = true → ∃ key ∈ group.keys, eligible key = true) :
    weightedSum (fun group => lowerCost group weight eligible) groups +
      weightedSum (contribution weight eligible) remaining ≤
      weightedSum (contribution weight eligible) keys := by
  have allocated := residuals_le_allocated groups weight eligible nonnegative required
  have exactPartition := AggregateAssignment.perm_weighted_sum
    (contribution weight eligible) partition
  rw [sum_append] at exactPartition
  omega

/-- Sum the group inequalities before relating their allocated keys to the
    complete objective. No disjointness is needed until that next step. -/
theorem floors_le_allocated (groups : List (Group κ))
    (weight : κ → Int) (eligible : κ → Bool)
    (nonnegative : ∀ group ∈ groups, ∀ key ∈ group.keys, 0 ≤ weight key)
    (minimum : ∀ group ∈ groups, ∀ key ∈ group.keys, group.minimum ≤ weight key)
    (required : ∀ group ∈ groups,
      group.active = true → ∃ key ∈ group.keys, eligible key = true) :
    weightedSum floor groups ≤
      weightedSum (contribution weight eligible) (groups.flatMap Group.keys) := by
  induction groups with
  | nil => simp [weightedSum]
  | cons group groups ih =>
    have first := group_floor_le group weight eligible
      (nonnegative group (by simp)) (minimum group (by simp)) (required group (by simp))
    have rest := ih
      (fun other member => nonnegative other (by simp [member]))
      (fun other member => minimum other (by simp [member]))
      (fun other member => required other (by simp [member]))
    simp only [weightedSum, List.flatMap_cons, sum_append]
    omega

/-- Allocate keys without duplication, replace each required group's exact
    cost by its floor, and leave the other keys unchanged. The resulting sum
    cannot exceed the exact objective cost. The partition premise counts every
    occurrence once; for a coalesced key list this also forbids shared keys. -/
theorem partition_lower_bound (keys remaining : List κ) (groups : List (Group κ))
    (weight : κ → Int) (eligible : κ → Bool)
    (partition : (groups.flatMap Group.keys ++ remaining).Perm keys)
    (nonnegative : ∀ group ∈ groups, ∀ key ∈ group.keys, 0 ≤ weight key)
    (minimum : ∀ group ∈ groups, ∀ key ∈ group.keys, group.minimum ≤ weight key)
    (required : ∀ group ∈ groups,
      group.active = true → ∃ key ∈ group.keys, eligible key = true) :
    weightedSum floor groups + weightedSum (contribution weight eligible) remaining ≤
      weightedSum (contribution weight eligible) keys := by
  have allocated := floors_le_allocated groups weight eligible nonnegative minimum required
  have exactPartition := AggregateAssignment.perm_weighted_sum
    (contribution weight eligible) partition
  rw [sum_append] at exactPartition
  omega

/-- Componentwise lower bounds are also necessary under lexicographic
    minimization. If an exact cost vector meets the incumbent bound, its lower
    vector does too. All three vectors use the same priority order. -/
theorem lexicographic_lower_bound (priorities : List π) (lower exact ceiling : π → Int)
    (bounded : ∀ priority ∈ priorities, lower priority ≤ exact priority)
    (within : compareVectors (priorities.map exact) (priorities.map ceiling) ≠ .gt) :
    compareVectors (priorities.map lower) (priorities.map ceiling) ≠ .gt := by
  induction priorities with
  | nil => simp [compareVectors]
  | cons priority priorities ih =>
    have first := bounded priority (by simp)
    simp only [List.map_cons, compareVectors] at within ⊢
    by_cases less : lower priority < ceiling priority
    · simp [less]
    · have exactWithin : exact priority ≤ ceiling priority := by
        by_cases upper : ceiling priority < exact priority
        · have notLower : ¬ exact priority < ceiling priority := by omega
          simp [notLower, upper] at within
        · omega
      have equalLower : lower priority = ceiling priority := by omega
      have equalExact : exact priority = ceiling priority := by omega
      have tail := ih (fun other member => bounded other (by simp [member]))
      simp only [equalExact, Int.lt_irrefl, ↓reduceIte] at within
      simpa [equalLower] using tail within

/-- Conjoining the necessary lower bound leaves the accepted exact-cost
    candidates unchanged. This equality includes ties with the incumbent. -/
theorem lower_bound_preserves_candidates (priorities : List π)
    (lower exact ceiling : π → Int)
    (bounded : ∀ priority ∈ priorities, lower priority ≤ exact priority) :
    (compareVectors (priorities.map exact) (priorities.map ceiling) ≠ .gt ∧
      compareVectors (priorities.map lower) (priorities.map ceiling) ≠ .gt) ↔
      compareVectors (priorities.map exact) (priorities.map ceiling) ≠ .gt := by
  constructor
  · exact And.left
  · intro within
    exact ⟨within, lexicographic_lower_bound priorities lower exact ceiling bounded within⟩

/-- A lower-bound restriction is still conjoined with stability of the
    original theory. No different reduct is introduced by objective pruning. -/
theorem lower_bound_preserves_answer_sets (theory : Ferraris.Theory α)
    (candidate : Atoms α) (priorities : List π) (lower exact ceiling : π → Int)
    (bounded : ∀ priority ∈ priorities, lower priority ≤ exact priority) :
    (Ferraris.Stable candidate theory ∧
      compareVectors (priorities.map exact) (priorities.map ceiling) ≠ .gt ∧
      compareVectors (priorities.map lower) (priorities.map ceiling) ≠ .gt) ↔
    (Ferraris.Stable candidate theory ∧
      compareVectors (priorities.map exact) (priorities.map ceiling) ≠ .gt) := by
  rw [lower_bound_preserves_candidates priorities lower exact ceiling bounded]

end Zetesis.RequiredChoiceCosts
