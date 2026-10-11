import Zetesis.ValueExtrema
import Zetesis.OrderedBounds

/-!
# Finite rank quotients for extrema

An extremum can be reduced over integer ranks when selection and the represented
guards agree with the source order. Ranks need not distinguish values lying in
the same guard interval. The value returned here is a rank, not a reconstruction
of the selected source value.

The finite carrier must include every possible contribution and the actual empty
endpoint. Comparator compatibility and guard partitions are explicit premises;
this module does not verify sorting, typed comparison or a Rust preparation
routine. Complete tuple positions and their eligibility formulas are unchanged.
The final laws cover original truth and every frozen interpretation, not partial
propagation. Extending the guard family requires establishing its agreement too.
-/

namespace Zetesis.RankedExtrema

open Ferraris ValueExtrema AggregateReduct

universe u v w
variable {κ : Type u} {ρ : Type v} {α : Type w} {n : Nat}

/-- Mapping commutes with finite selection if it commutes with each choice on
    the carrier. The selected tail remains in that carrier, by selection itself.
    No injectivity or source-order comparison reflection is required. -/
theorem extreme_map (before : κ → κ → Bool) (after : ρ → ρ → Bool)
    (code : κ → ρ) (carrier values : List κ)
    (choice : ∀ left ∈ carrier, ∀ right ∈ carrier,
      choose after (code left) (code right) = code (choose before left right))
    (coverage : ∀ value ∈ values, value ∈ carrier) :
    extreme after (values.map code) = (extreme before values).map code := by
  revert coverage
  induction values with
  | nil => intro _; rfl
  | cons head tail ih =>
    intro coverage
    have headIn : head ∈ carrier := coverage head (by simp)
    have tailIn : ∀ value ∈ tail, value ∈ carrier := by
      intro value member
      exact coverage value (List.mem_cons_of_mem head member)
    have mappedTail := ih tailIn
    cases selected : extreme before tail with
    | none => simp [extreme, mappedTail, selected]
    | some best =>
      have bestIn : best ∈ carrier :=
        tailIn best (extreme_some_mem before tail best selected)
      have mappedChoice := choice head headIn best bestIn
      simp [extreme, mappedTail, selected, mappedChoice]

/-- The empty result is mapped from its source endpoint. It is not replaced by
    an arbitrary integer sentinel. -/
theorem encoded_extreme_map (before : κ → κ → Bool) (after : ρ → ρ → Bool)
    (code : κ → ρ) (carrier values : List κ) (empty : κ)
    (choice : ∀ left ∈ carrier, ∀ right ∈ carrier,
      choose after (code left) (code right) = code (choose before left right))
    (coverage : ∀ value ∈ values, value ∈ carrier) :
    encode (code empty) (extreme after (values.map code)) =
      code (encode empty (extreme before values)) := by
  rw [extreme_map before after code carrier values choice coverage]
  cases extreme before values <;> rfl

inductive Direction where
  | minimum
  | maximum

def prefer : Direction → Ordering → Bool
  | .minimum, .gt => false
  | .minimum, _ => true
  | .maximum, .lt => false
  | .maximum, _ => true

def rankedBefore : Direction → Nat → Nat → Bool
  | .minimum, left, right => decide (left ≤ right)
  | .maximum, left, right => !(decide (left ≤ right))

/-- Weak compatibility allows distinct values to share a rank. Equality must
    share a rank; either strict ordering may collapse to rank equality. -/
def Compatible (compare : κ → κ → Ordering) (rank : κ → Nat)
    (carrier : List κ) : Prop :=
  ∀ left ∈ carrier, ∀ right ∈ carrier,
    match compare left right with
    | .lt => rank left ≤ rank right
    | .eq => rank left = rank right
    | .gt => rank right ≤ rank left

/-- Choosing a rank is the ordinary integer minimum or maximum. The tie choice
    is immaterial because the result is the common rank. -/
theorem ranked_choice (direction : Direction) (left right : Nat) :
    choose (rankedBefore direction) left right =
      match direction with
      | .minimum => min left right
      | .maximum => max left right := by
  cases direction <;> by_cases ordered : left ≤ right <;>
    simp [choose, rankedBefore, Nat.min_def, Nat.max_def, ordered]

/-- A weakly compatible rank quotient commutes with a single source selection.
    A collapsed strict comparison produces equal ranks on either branch. -/
theorem compatible_choice (direction : Direction) (compare : κ → κ → Ordering)
    (rank : κ → Nat) (carrier : List κ) (compatible : Compatible compare rank carrier)
    (left : κ) (leftIn : left ∈ carrier) (right : κ) (rightIn : right ∈ carrier) :
    choose (rankedBefore direction) (rank left) (rank right) =
      rank (choose (fun a b => prefer direction (compare a b)) left right) := by
  have ordered := compatible left leftIn right rightIn
  rw [ranked_choice]
  cases relation : compare left right <;> cases direction <;>
    simp_all [prefer, choose, Nat.min_eq_left, Nat.min_eq_right,
      Nat.max_eq_left, Nat.max_eq_right]

/-- Consequently either extremum can be selected entirely on ranks. This says
    nothing about choosing a representative source value from a merged rank. -/
theorem ranked_extreme (direction : Direction) (compare : κ → κ → Ordering)
    (rank : κ → Nat) (carrier values : List κ) (empty : κ)
    (compatible : Compatible compare rank carrier)
    (coverage : ∀ value ∈ values, value ∈ carrier) :
    encode (rank empty) (extreme (rankedBefore direction) (values.map rank)) =
      rank (encode empty
        (extreme (fun a b => prefer direction (compare a b)) values)) := by
  exact encoded_extreme_map _ _ rank carrier values empty
    (compatible_choice direction compare rank carrier compatible) coverage

/-- The two cuts delimit values strictly below and at or below one guard bound.
    The bound itself need not occur among the contributions. -/
structure Cuts where
  below : Nat
  atOrBelow : Nat

def cutAccepts : OrderedBounds.Relation → Cuts → Nat → Bool
  | .less, cuts, rank => decide (rank < cuts.below)
  | .lessEqual, cuts, rank => decide (rank < cuts.atOrBelow)
  | .equal, cuts, rank => decide (cuts.below ≤ rank ∧ rank < cuts.atOrBelow)
  | .notEqual, cuts, rank => decide (rank < cuts.below ∨ cuts.atOrBelow ≤ rank)
  | .greaterEqual, cuts, rank => decide (cuts.below ≤ rank)
  | .greater, cuts, rank => decide (cuts.atOrBelow ≤ rank)

/-- A represented guard has the correct strict and inclusive cuts for each
    possible source result. This is stronger than weak order compatibility. -/
def CutsAgree (compare : κ → κ → Ordering) (rank : κ → Nat)
    (carrier : List κ) (bound : κ) (cuts : Cuts) : Prop :=
  ∀ value ∈ carrier,
    (compare value bound = .lt ↔ rank value < cuts.below) ∧
    (compare value bound ≠ .gt ↔ rank value < cuts.atOrBelow)

/-- Strict and inclusive cut agreement preserves all six guard relations,
    including equality when values inside an unobserved interval share a rank.
    The three source comparison cases determine the result. -/
theorem cut_agreement (relation : OrderedBounds.Relation)
    (compare : κ → κ → Ordering) (rank : κ → Nat) (carrier : List κ)
    (bound : κ) (cuts : Cuts) (agreement : CutsAgree compare rank carrier bound cuts)
    (value : κ) (member : value ∈ carrier) :
    cutAccepts relation cuts (rank value) = OrderedBounds.accepts relation (compare value bound) := by
  obtain ⟨below, atOrBelow⟩ := agreement value member
  cases ordering : compare value bound <;> cases relation <;>
    simp_all [cutAccepts, OrderedBounds.accepts] <;> omega

/-- Positions refer to complete aggregate tuples. Equal first values or equal
    eligibility formulas do not merge those positions. -/
def selectedValues (values : Fin n → κ) (mask : Mask n) : List κ :=
  ((List.finRange n).filter mask).map values

/-- Every selected value belongs to the fixed contribution carrier. -/
theorem selected_coverage (values : Fin n → κ) (mask : Mask n) (carrier : List κ)
    (coverage : ∀ index, values index ∈ carrier) :
    ∀ value ∈ selectedValues values mask, value ∈ carrier := by
  intro value member
  obtain ⟨index, _, same⟩ := List.mem_map.mp member
  exact same ▸ coverage index

def sourceMeasure (direction : Direction) (compare : κ → κ → Ordering)
    (empty : κ) (values : Fin n → κ) (mask : Mask n) : κ :=
  encode empty (extreme (fun a b => prefer direction (compare a b))
    (selectedValues values mask))

def rankMeasure (direction : Direction) (rank : κ → Nat) (empty : κ)
    (values : Fin n → κ) (mask : Mask n) : Nat :=
  encode (rank empty) (extreme (rankedBefore direction)
    (selectedValues (fun index => rank (values index)) mask))

/-- Reducing selected ranks agrees with ranking the selected source extremum
    for every mask, including the empty mask. -/
theorem measure_agreement (direction : Direction) (compare : κ → κ → Ordering)
    (rank : κ → Nat) (carrier : List κ) (empty : κ) (values : Fin n → κ)
    (compatible : Compatible compare rank carrier)
    (coverage : ∀ index, values index ∈ carrier) (mask : Mask n) :
    rankMeasure direction rank empty values mask =
      rank (sourceMeasure direction compare empty values mask) := by
  have selection := ranked_extreme direction compare rank carrier
    (selectedValues values mask) empty compatible
    (selected_coverage values mask carrier coverage)
  simpa only [rankMeasure, sourceMeasure, selectedValues, List.map_map,
    Function.comp_def] using selection

/-- Each result is either the actual empty endpoint or a selected contribution.
    Therefore a guard need only be certified on that complete finite carrier. -/
theorem measure_in_carrier (direction : Direction) (compare : κ → κ → Ordering)
    (carrier : List κ) (empty : κ) (values : Fin n → κ)
    (emptyIn : empty ∈ carrier) (coverage : ∀ index, values index ∈ carrier)
    (mask : Mask n) : sourceMeasure direction compare empty values mask ∈ carrier := by
  unfold sourceMeasure
  cases selected : extreme (fun a b => prefer direction (compare a b))
      (selectedValues values mask) with
  | none => exact emptyIn
  | some best =>
    exact selected_coverage values mask carrier coverage best
      (extreme_some_mem _ _ best selected)

def sourceGuard (direction : Direction) (compare : κ → κ → Ordering)
    (relation : OrderedBounds.Relation) (bound empty : κ)
    (values : Fin n → κ) (mask : Mask n) : Bool :=
  OrderedBounds.accepts relation (compare (sourceMeasure direction compare empty values mask) bound)

def rankGuard (direction : Direction) (rank : κ → Nat)
    (relation : OrderedBounds.Relation) (cuts : Cuts) (empty : κ)
    (values : Fin n → κ) (mask : Mask n) : Bool :=
  cutAccepts relation cuts (rankMeasure direction rank empty values mask)

/-- Selection agreement followed by guard-cut agreement proves the complete
    evaluator contract. The empty endpoint is covered by the same guard law. -/
theorem guard_agreement (direction : Direction) (compare : κ → κ → Ordering)
    (rank : κ → Nat) (carrier : List κ) (relation : OrderedBounds.Relation)
    (bound empty : κ) (cuts : Cuts) (values : Fin n → κ)
    (compatible : Compatible compare rank carrier)
    (agreement : CutsAgree compare rank carrier bound cuts)
    (emptyIn : empty ∈ carrier) (coverage : ∀ index, values index ∈ carrier)
    (mask : Mask n) :
    rankGuard direction rank relation cuts empty values mask =
      sourceGuard direction compare relation bound empty values mask := by
  unfold rankGuard sourceGuard
  rw [measure_agreement direction compare rank carrier empty values compatible coverage]
  exact cut_agreement relation compare rank carrier bound cuts agreement _
    (measure_in_carrier direction compare carrier empty values emptyIn coverage mask)

/-- Original aggregate truth is the rank evaluator over the original eligible
    tuple positions. The reference aggregate includes every possible mask. -/
theorem original (M : Atoms α) (conditions : Fin n → Formula α)
    (direction : Direction) (compare : κ → κ → Ordering) (rank : κ → Nat)
    (carrier : List κ) (relation : OrderedBounds.Relation) (bound empty : κ)
    (cuts : Cuts) (values : Fin n → κ)
    (compatible : Compatible compare rank carrier)
    (agreement : CutsAgree compare rank carrier bound cuts)
    (emptyIn : empty ∈ carrier) (coverage : ∀ index, values index ∈ carrier) :
    Satisfies M (aggregate conditions
      (sourceGuard direction compare relation bound empty values) (masks n)) ↔
      rankGuard direction rank relation cuts empty values (active M conditions) = true := by
  rw [AggregateReduct.original M conditions _ (masks n) (masks_complete n)]
  rw [guard_agreement direction compare rank carrier relation bound empty cuts values
    compatible agreement emptyIn coverage]

/-- Frozen truth retains the original guard and separately reduces recursively
    frozen eligibility. J is arbitrary: neither a subset premise nor original
    satisfaction is assumed. A rank measure cannot replace the original gate. -/
theorem frozen (M J : Atoms α) (conditions : Fin n → Formula α)
    (direction : Direction) (compare : κ → κ → Ordering) (rank : κ → Nat)
    (carrier : List κ) (relation : OrderedBounds.Relation) (bound empty : κ)
    (cuts : Cuts) (values : Fin n → κ)
    (compatible : Compatible compare rank carrier)
    (agreement : CutsAgree compare rank carrier bound cuts)
    (emptyIn : empty ∈ carrier) (coverage : ∀ index, values index ∈ carrier) :
    Satisfies J (Reduct M (aggregate conditions
      (sourceGuard direction compare relation bound empty values) (masks n))) ↔
      rankGuard direction rank relation cuts empty values (active M conditions) = true ∧
      rankGuard direction rank relation cuts empty values
        (active J (fun index => Reduct M (conditions index))) = true := by
  exact evaluator_refinement M J conditions _ _
    (guard_agreement direction compare rank carrier relation bound empty cuts values
      compatible agreement emptyIn coverage)

end Zetesis.RankedExtrema
