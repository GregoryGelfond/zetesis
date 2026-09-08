import Std

/-!
# Signed aggregate ranges under parallel reassociation

The carrier is a finite list of contribution occurrences. Equal weights remain
separate list occurrences; selecting by value membership alone is insufficient.
A subcollection is a permutation of a sublist, so it cannot increase any
weight's multiplicity. The sum of all negative contributions bounds every
selected sum below, and the sum of all positive contributions bounds it above.
Bounding only the final signed total does not bound partial sums.

A finite addition tree supplies an abstract parallel reduction. If its leaves
form a subcollection of the admitted carrier, every intermediate result lies
in the admitted interval, for arbitrary tree shape and leaf order. The leaf
premise expresses the required disjoint occurrence coverage; the tree is not
allowed to duplicate contributions merely because the final total cancels.

These laws use mathematical Int. Applying them to Rust or WGSL requires a
separate correspondence for contributions, occurrence ownership, the actual
reduction tree, integer encoding and checked arithmetic. No source entailment,
aggregate eligibility, shader partition correctness, resource accounting or
device execution is proved here. The module imports only Std.
-/

namespace Zetesis.AggregateRanges

/-- All strictly positive contributions, with neutral zeros elsewhere. -/
def positiveTotal (weights : List Int) : Int :=
  (weights.map (fun weight => if 0 < weight then weight else 0)).sum

/-- All strictly negative contributions, with neutral zeros elsewhere. -/
def negativeTotal (weights : List Int) : Int :=
  (weights.map (fun weight => if weight < 0 then weight else 0)).sum

/-- Inclusive mathematical bounds, independent of a particular machine width. -/
def Within (lower upper value : Int) : Prop := lower ≤ value ∧ value ≤ upper

/-- Reordering a selected sublist preserves occurrence multiplicity. This is
stronger than requiring each selected value to occur somewhere in the carrier. -/
def Subcollection (selected weights : List Int) : Prop :=
  ∃ ordered, ordered.Sublist weights ∧ selected.Perm ordered

/-- Removing occurrences cannot escape the full negative/positive envelope. -/
theorem sublist_envelope {selected weights : List Int}
    (selection : selected.Sublist weights) :
    Within (negativeTotal weights) (positiveTotal weights) selected.sum := by
  induction selection with
  | slnil => simp [Within, negativeTotal, positiveTotal]
  | cons weight selection tail =>
    have tail_bounds := tail
    simp only [Within, negativeTotal, positiveTotal, List.map_cons,
      List.sum_cons] at tail_bounds ⊢
    by_cases negative : weight < 0 <;> by_cases positive : 0 < weight <;>
      simp only [negative, positive, if_true, if_false] <;> omega
  | cons_cons weight selection tail =>
    have tail_bounds := tail
    simp only [Within, negativeTotal, positiveTotal, List.map_cons,
      List.sum_cons] at tail_bounds ⊢
    by_cases negative : weight < 0 <;> by_cases positive : 0 < weight <;>
      simp only [negative, positive, if_true, if_false] <;> omega

/-- Permutation changes neither the contribution multiplicity nor the sum. -/
theorem permutation_sum {left right : List Int} (reordered : left.Perm right) :
    left.sum = right.sum := by
  induction reordered with
  | nil => rfl
  | cons weight reordered tail => simp only [List.sum_cons, tail]
  | swap first second rest => simp only [List.sum_cons]; omega
  | trans first second left_equal right_equal => exact left_equal.trans right_equal

/-- The signed envelope bounds every selected subcollection in any order. The
two separate admission premises are stronger than a bound on the final total. -/
theorem subcollection_range (weights selected : List Int) (lower upper : Int)
    (selection : Subcollection selected weights)
    (negative_bound : lower ≤ negativeTotal weights)
    (positive_bound : positiveTotal weights ≤ upper) :
    Within lower upper selected.sum := by
  obtain ⟨ordered, included, reordered⟩ := selection
  have envelope : Within (negativeTotal weights) (positiveTotal weights) ordered.sum :=
    sublist_envelope included
  have same_total : selected.sum = ordered.sum := permutation_sum reordered
  have admitted : Within lower upper ordered.sum := by
    exact ⟨Int.le_trans negative_bound envelope.1, Int.le_trans envelope.2 positive_bound⟩
  exact same_total ▸ admitted

/-- A partial selection from an already valid subcollection remains valid. -/
theorem subcollection_of_sublist {part selected weights : List Int}
    (partial_selection : part.Sublist selected)
    (selection : Subcollection selected weights) : Subcollection part weights := by
  obtain ⟨ordered, included, reordered⟩ := selection
  obtain ⟨partial_ordered, partial_permutation, partial_included⟩ :=
    List.exists_perm_sublist partial_selection reordered
  have carrier_inclusion : partial_ordered.Sublist weights :=
    partial_included.trans included
  exact ⟨partial_ordered, carrier_inclusion, partial_permutation.symm⟩

/-- A finite mathematical reduction; each join adds two disjoint leaf groups
only when the caller establishes the root occurrence-selection premise. -/
inductive Reduction where
  | empty
  | contribution : Int → Reduction
  | join : Reduction → Reduction → Reduction

/-- Left-to-right contribution occurrences of a tree, retaining duplicates. -/
def Reduction.leaves : Reduction → List Int
  | .empty => []
  | .contribution weight => [weight]
  | .join left right => left.leaves ++ right.leaves

/-- The mathematical result at the root, independently of machine addition. -/
def Reduction.value : Reduction → Int
  | .empty => 0
  | .contribution weight => weight
  | .join left right => left.value + right.value

/-- Every subtree result, including the root and empty neutral reductions, fits
the same admitted interval. This says more than the root alone being in range. -/
def Reduction.Safe (lower upper : Int) : Reduction → Prop
  | .empty => Within lower upper 0
  | .contribution weight => Within lower upper weight
  | .join left right =>
      left.Safe lower upper ∧ right.Safe lower upper ∧
        Within lower upper (left.value + right.value)

/-- Reassociation preserves the exact sum of a tree's leaf occurrences. -/
theorem reduction_sum (tree : Reduction) : tree.value = tree.leaves.sum := by
  induction tree with
  | empty => rfl
  | contribution weight => simp [Reduction.value, Reduction.leaves]
  | join left right left_equal right_equal =>
    have combined : (left.leaves ++ right.leaves).sum =
        left.leaves.sum + right.leaves.sum := List.sum_append_int
    simp only [Reduction.value, Reduction.leaves, combined, left_equal, right_equal]

/-- An admitted positive/negative envelope bounds every intermediate result of
any finite reassociation of a selected subcollection. The leaf premise must be
proved for the actual execution; separate bounds for overlapping groups do not
permit counting their shared occurrences twice. -/
theorem parallel_reduction_range (weights : List Int) (lower upper : Int)
    (negative_bound : lower ≤ negativeTotal weights)
    (positive_bound : positiveTotal weights ≤ upper)
    (tree : Reduction) (selection : Subcollection tree.leaves weights) :
    tree.Safe lower upper := by
  induction tree with
  | empty =>
    exact subcollection_range weights [] lower upper selection negative_bound positive_bound
  | contribution weight =>
    have leaf_bound := subcollection_range weights [weight] lower upper
      selection negative_bound positive_bound
    simpa only [Reduction.Safe, List.sum_singleton] using leaf_bound
  | join left right left_safe right_safe =>
    have left_selection : Subcollection left.leaves weights :=
      subcollection_of_sublist (List.sublist_append_left _ _) selection
    have right_selection : Subcollection right.leaves weights :=
      subcollection_of_sublist (List.sublist_append_right _ _) selection
    have combined_bound : Within lower upper (left.leaves ++ right.leaves).sum :=
      subcollection_range weights _ lower upper selection negative_bound positive_bound
    have root_bound : Within lower upper (left.value + right.value) := by
      simpa only [List.sum_append_int, reduction_sum] using combined_bound
    exact ⟨left_safe left_selection, right_safe right_selection, root_bound⟩

/-- Cancellation can make the final total fit while an intermediate positive
sum exceeds the same interval. No machine width is special to this obstruction. -/
theorem bounded_total_is_insufficient (bound : Int) (nonnegative : 0 ≤ bound) :
    Within (-bound) bound [bound + 1, -(bound + 1)].sum ∧
      ¬ Within (-bound) bound [bound + 1].sum := by
  simp only [List.sum_cons, List.sum_nil, Within]
  omega

end Zetesis.AggregateRanges
