import Zetesis.PartitionCapacities
import Zetesis.EntailedPruning

/-!
# Families of counts

The number of an interpretation's atoms among a list of atoms is a count in the
ordered commutative monoid (ℕ, +, ≤), and counting adds over concatenation:
a monoid homomorphism from lists under append. Rows bound such counts, an
at-least row from below and an at-most row from above. When two families of rows
each partition the same atoms, the atoms are counted twice, once by each family,
and the two totals agree. If the first family's lower bounds add up to more than
the second family's upper bounds, no interpretation satisfies every row: the
pigeonhole principle, refuted by one comparison, where resolution needs
exponentially many steps.

The laws:
- `count_perm` and `count_partition_additive`: a count does not depend on order,
  and a partition's counts add up to the whole;
- `double_count_refutes`: two partitions of the same atoms whose lower bounds
  exceed their upper bounds admit no interpretation;
- `cut_refutes`: for a bipartite system, a cut certificate, a set of rows of
  each family with the atoms they share and those the second set misses, refutes
  a region when the first set's lower bounds exceed the second's upper bounds
  plus the missed atoms the region still allows;
- `count_lower_in_region` and `count_upper_in_region`: the region-adjusted bounds,
  held atoms counting toward every member's count and cut atoms leaving it;
- `probe_forces`: a region whose probe of an atom is refuted has that atom cut
  in every member of the class the refutation concerns.

Lists count occurrences. A recognizer must establish that its rows' atom lists
are the partitions the premises state, that the bounds are the theory's, and
that a certificate's shared and missed atoms split the rows as stated. Max-flow
and min-cut themselves are not proved: only a certificate's inequality is
checked. Rows hold in every model of the constraints they come from, so in every
answer set (`EntailedPruning`).
-/

namespace Zetesis.CountingFamilies

open ChoiceIntervals PartitionCapacities

universe u
variable {α : Type u}

/-- A count does not depend on the order of the list. -/
theorem count_perm (selected : α → Prop) {l₁ l₂ : List α} (same : l₁.Perm l₂) :
    count selected l₁ = count selected l₂ := by
  classical
  exact (same.filter (fun value => decide (selected value))).length_eq

/-- Counting adds over the parts of a flattened list. -/
theorem count_flatten (selected : α → Prop) (parts : List (List α)) :
    count selected parts.flatten = (parts.map (count selected)).sum := by
  induction parts with
  | nil => simp [count]
  | cons part rest ih => simp [List.flatten_cons, count_append, ih]

/-- The counts of a partition's parts add up to the count of the whole. -/
theorem count_partition_additive (selected : α → Prop) (S : List α)
    (parts : List (List α)) (covers : parts.flatten.Perm S) :
    count selected S = (parts.map (count selected)).sum := by
  rw [← count_perm selected covers]
  exact count_flatten selected parts

/-- The lower bounds of a family's rows add up to at most the count of their
atoms. -/
theorem lower_bounds_sum (selected : α → Prop) (rows : List (Group α))
    (bounded : ∀ row ∈ rows, row.2 ≤ count selected row.1) :
    (rows.map Prod.snd).sum ≤ count selected (members rows) := by
  rw [count_members]
  induction rows with
  | nil => simp
  | cons row rest ih =>
    have head := bounded row (by simp)
    have tail := ih (fun other inside => bounded other (by simp [inside]))
    simpa using Nat.add_le_add head tail

/-- Two families each partitioning the same atoms, the first with lower bounds and
the second with upper bounds that the first's exceed, admit no interpretation
satisfying every row.

Proof. Count the atoms twice. The first family's lower bounds add up to at most
the count (`lower_bounds_sum`), which the second family's upper bounds bound
from above (`capacity_bounds`). -/
theorem double_count_refutes (S : List α) (first second : List (Group α))
    (firstCovers : (members first).Perm S) (secondCovers : (members second).Perm S)
    (exceeds : capacity second < (first.map Prod.snd).sum) (M : Atoms α) :
    ¬ ((∀ row ∈ first, row.2 ≤ count M row.1) ∧
      (∀ row ∈ second, count M row.1 ≤ row.2)) := by
  rintro ⟨lower, upper⟩
  have fromBelow : (first.map Prod.snd).sum ≤ count M S := by
    rw [← count_perm M firstCovers]
    exact lower_bounds_sum M first lower
  have fromAbove : count M S ≤ capacity second := by
    rw [← count_perm M secondCovers]
    exact capacity_bounds M second upper
  omega

/-- A count can only grow with the interpretation. -/
theorem count_mono {X Y : Atoms α} (included : Sub X Y) (l : List α) :
    count X l ≤ count Y l := by
  classical
  simp only [count, ← List.countP_eq_length_filter]
  apply List.countP_mono_left
  intro a _ inX
  exact decide_eq_true (included a (of_decide_eq_true inX))

/-- In a region, every member counts at least the held atoms. -/
theorem count_lower_in_region (c : Cube α) {z : Atoms α} (inside : c.Contains z)
    (l : List α) : count c.lower l ≤ count z l :=
  count_mono inside.1 l

/-- In a region, every member counts at most the atoms not cut. -/
theorem count_upper_in_region (c : Cube α) {z : Atoms α} (inside : c.Contains z)
    (l : List α) : count z l ≤ count c.upper l :=
  count_mono inside.2 l

/-- A cut certificate refutes a region of a bipartite system. The certificate
names some rows of the first family, with lower bounds, and some rows of the
second, with upper bounds. Its shared atoms lie in both sets of rows; its missed
atoms lie in the first set's rows outside the second's. If the first set's lower
bounds exceed the second set's upper bounds plus the number of missed atoms the
region has not cut, no interpretation in the region satisfies those rows.

Proof. A member's count over the first set's atoms is at least their lower
bounds. That count is its count over the shared atoms plus its count over the
missed ones. The shared atoms are among the second set's atoms, whose count the
upper bounds bound, and the missed atoms count at most as many as the region has
not cut. -/
theorem cut_refutes (first second : List (Group α))
    (shared missed rest : List α)
    (firstSplit : (members first).Perm (shared ++ missed))
    (secondSplit : (members second).Perm (shared ++ rest))
    (c : Cube α)
    (exceeds : capacity second + count c.upper missed < (first.map Prod.snd).sum) :
    ∀ z, c.Contains z →
      ¬ ((∀ row ∈ first, row.2 ≤ count z row.1) ∧
        (∀ row ∈ second, count z row.1 ≤ row.2)) := by
  rintro z inside ⟨lower, upper⟩
  have fromBelow : (first.map Prod.snd).sum ≤ count z shared + count z missed := by
    rw [← count_append, ← count_perm z firstSplit]
    exact lower_bounds_sum z first lower
  have sharedBound : count z shared ≤ capacity second := by
    have whole : count z (shared ++ rest) ≤ capacity second := by
      rw [← count_perm z secondSplit]
      exact capacity_bounds z second upper
    rw [count_append] at whole
    omega
  have missedBound : count z missed ≤ count c.upper missed :=
    count_upper_in_region c inside missed
  omega

/-- Probe and refute: if the region with an atom held contains no member of a
class, every member of the class in the region has the atom cut. Any certified
refutation of the probe serves, a counting cut among them. -/
theorem probe_forces (valid : Atoms α → Prop) (c : Cube α) (a : α)
    (refuted : ∀ z, (c.splitTrue a).Contains z → ¬ valid z) :
    ∀ z, c.Contains z → valid z → ¬ z a := by
  intro z inside member present
  exact refuted z (Cube.contains_splitTrue.mpr ⟨inside, present⟩) member

end Zetesis.CountingFamilies
