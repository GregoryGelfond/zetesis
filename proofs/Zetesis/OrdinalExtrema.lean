import Zetesis.RankedExtrema

/-!
# Ordinal extrema and cut witnesses

A finite row contains natural-number ranks bounded by its greatest rank. One
fold computes both the minimum and maximum, using the greatest rank and zero
for empty selection. The fold has no guard parameter: every later cut can read
the same pair of measures. Repeated ranks remain ordinary list occurrences.

The central laws identify this paired fold with the existing ordered-selection
operation, then characterize interior cuts by witnesses. Endpoint cuts are
constants. These are generic finite-order laws, independent of a solver, mutable
row owner, cache, propagation queue or device. Typed rank preparation still has
to establish the carrier and comparison agreements of `RankedExtrema`; no Rust
sorting, arithmetic, storage, execution cost or extraction refinement is claimed.
-/

namespace Zetesis.OrdinalExtrema

open ValueExtrema RankedExtrema

/-- Reduce both extrema in one fold. The first component is the minimum, the
    second the maximum. Empty selection returns the explicit endpoint pair. -/
def measure (greatest : Nat) (values : List Nat) : Nat × Nat :=
  values.foldr (fun value rest => (min value rest.1, max value rest.2)) (greatest, 0)

/-- An empty row has greatest-rank minimum and zero-rank maximum. -/
theorem empty_measures (greatest : Nat) : measure greatest [] = (greatest, 0) := rfl

/-- Projecting the paired fold is the same as either ordinary selection fold.
    Both components are updated by the same occurrence traversal; guards do not
    occur in this operation or its statement. -/
theorem paired_fold (greatest : Nat) (values : List Nat) :
    measure greatest values =
      (values.foldr (choose (rankedBefore .minimum)) greatest,
       values.foldr (choose (rankedBefore .maximum)) 0) := by
  induction values with
  | nil => rfl
  | cons head tail ih =>
    change (min head (measure greatest tail).1, max head (measure greatest tail).2) = _
    rw [ih]
    simp only [List.foldr_cons, ranked_choice]

/-- The paired fold computes the selected minimum and maximum. All contributions
    lie below the greatest rank, making it neutral for minimum; zero is neutral
    for maximum. Empty results use precisely these endpoints. -/
theorem selected_measures (greatest : Nat) (values : List Nat)
    (bounded : ∀ value ∈ values, value ≤ greatest) :
    measure greatest values =
      (encode greatest (extreme (rankedBefore .minimum) values),
       encode 0 (extreme (rankedBefore .maximum) values)) := by
  have fold_selection {κ : Type} (before : κ → κ → Bool) (empty : κ)
      (values : List κ)
      (neutral : ∀ value ∈ values, choose before value empty = value) :
      values.foldr (choose before) empty = encode empty (extreme before values) := by
    revert neutral
    induction values with
    | nil => intro _; rfl
    | cons head tail ih =>
      intro neutral
      have headNeutral : choose before head empty = head := neutral head (by simp)
      have tailNeutral : ∀ value ∈ tail, choose before value empty = value := by
        intro value member
        exact neutral value (List.mem_cons_of_mem head member)
      have tailSelection := ih tailNeutral
      rw [List.foldr_cons, tailSelection]
      cases selected : extreme before tail with
      | none => simpa only [extreme, selected, encode] using headNeutral
      | some best => simp only [extreme, selected, encode]
  have minimumNeutral : ∀ value ∈ values,
      choose (rankedBefore .minimum) value greatest = value := by
    intro value member
    rw [ranked_choice]
    exact Nat.min_eq_left (bounded value member)
  have maximumNeutral : ∀ value ∈ values,
      choose (rankedBefore .maximum) value 0 = value := by
    intro value _
    rw [ranked_choice]
    exact Nat.max_eq_left (Nat.zero_le value)
  have minimumSelection := fold_selection (rankedBefore .minimum) greatest values minimumNeutral
  have maximumSelection := fold_selection (rankedBefore .maximum) 0 values maximumNeutral
  rw [paired_fold, minimumSelection, maximumSelection]

/-- Both selected measures stay in the closed ordinal carrier, including the
    empty case. A nonempty selection is one of its inputs, by `ValueExtrema`. -/
theorem measures_bounded (greatest : Nat) (values : List Nat)
    (bounded : ∀ value ∈ values, value ≤ greatest) :
    (measure greatest values).1 ≤ greatest ∧ (measure greatest values).2 ≤ greatest := by
  have selectedBounded (direction : Direction) (empty : Nat) (emptyBound : empty ≤ greatest) :
      encode empty (extreme (rankedBefore direction) values) ≤ greatest := by
    cases selected : extreme (rankedBefore direction) values with
    | none => exact emptyBound
    | some best =>
      exact bounded best (extreme_some_mem (rankedBefore direction) values best selected)
  rw [selected_measures greatest values bounded]
  exact ⟨selectedBounded .minimum greatest (Nat.le_refl greatest),
    selectedBounded .maximum 0 (Nat.zero_le greatest)⟩

/-- For an interior prefix cut, the selected minimum is below the cut exactly
    when some contribution is below it. The upper bound on the cut excludes the
    empty minimum sentinel. The ordinary selection witness law supplies the
    nonempty argument; no occurrence or eligibility is deduplicated. -/
theorem prefix_witness (greatest cut : Nat) (values : List Nat)
    (bounded : ∀ value ∈ values, value ≤ greatest)
    (interior : 0 < cut ∧ cut ≤ greatest) :
    (measure greatest values).1 < cut ↔ ∃ value ∈ values, value < cut := by
  have selectionLaw : ∀ left right,
      choose (rankedBefore .minimum) left right < cut ↔ left < cut ∨ right < cut := by
    intro left right
    rw [ranked_choice]
    change min left right < cut ↔ left < cut ∨ right < cut
    by_cases ordered : left ≤ right
    · rw [Nat.min_eq_left ordered]
      omega
    · have reverse : right ≤ left := by omega
      rw [Nat.min_eq_right reverse]
      omega
  have witness := selected_or_empty_predicate (rankedBefore .minimum) greatest
    (fun value => value < cut) selectionLaw values
  have cutBound : cut ≤ greatest := interior.2
  have excludesEmpty : ¬ greatest < cut := by omega
  rw [selected_measures greatest values bounded]
  simpa only [excludesEmpty, and_false, false_or] using witness

/-- For an interior suffix cut, the selected maximum reaches the cut exactly
    when a contribution reaches it. Positivity excludes the empty maximum zero.
    The carrier upper bound records that this is an interior ordinal cut. -/
theorem suffix_witness (greatest cut : Nat) (values : List Nat)
    (bounded : ∀ value ∈ values, value ≤ greatest)
    (interior : 0 < cut ∧ cut ≤ greatest) :
    cut ≤ (measure greatest values).2 ↔ ∃ value ∈ values, cut ≤ value := by
  have selectionLaw : ∀ left right,
      cut ≤ choose (rankedBefore .maximum) left right ↔ cut ≤ left ∨ cut ≤ right := by
    intro left right
    rw [ranked_choice]
    change cut ≤ max left right ↔ cut ≤ left ∨ cut ≤ right
    by_cases ordered : left ≤ right
    · rw [Nat.max_eq_right ordered]
      omega
    · have reverse : right ≤ left := by omega
      rw [Nat.max_eq_left reverse]
      omega
  have witness := selected_or_empty_predicate (rankedBefore .maximum) 0
    (fun value => cut ≤ value) selectionLaw values
  have positiveCut : 0 < cut := interior.1
  have excludesEmpty : ¬ cut ≤ 0 := by omega
  rw [selected_measures greatest values bounded]
  simpa only [excludesEmpty, and_false, false_or] using witness

/-- A prefix at zero is false for every row, even an empty row. -/
theorem prefix_zero (greatest : Nat) (values : List Nat) :
    ¬ (measure greatest values).1 < 0 := Nat.not_lt_zero _

/-- A suffix at zero is true for every row, even an empty row. -/
theorem suffix_zero (greatest : Nat) (values : List Nat) :
    0 ≤ (measure greatest values).2 := Nat.zero_le _

/-- A prefix immediately above the carrier is true, including empty selection.
    With carrier size V, this boundary is V and the greatest rank is V - 1. -/
theorem prefix_above (greatest : Nat) (values : List Nat)
    (bounded : ∀ value ∈ values, value ≤ greatest) :
    (measure greatest values).1 < greatest + 1 := by
  have within := (measures_bounded greatest values bounded).1
  omega

/-- A suffix immediately above the carrier is false, including empty selection. -/
theorem suffix_above (greatest : Nat) (values : List Nat)
    (bounded : ∀ value ∈ values, value ≤ greatest) :
    ¬ greatest + 1 ≤ (measure greatest values).2 := by
  have within := (measures_bounded greatest values bounded).2
  omega

end Zetesis.OrdinalExtrema
