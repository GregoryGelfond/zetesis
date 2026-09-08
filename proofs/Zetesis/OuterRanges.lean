import Zetesis.ChoiceIntervals

/-!
# Finite ranges indexed by completed outer values

Each complete outer value selects its own mathematical integer interval.
Expansion retains the outer value together with every inclusive member. Equal
intervals in different outer rows do not share an exhausted cursor; reversed
intervals simply contribute no row. Clauses keep the original equality and
activation supplied for that exact row under both original and frozen truth.

These are denotational expansion laws with a complete outer carrier and total
integer endpoint functions as premises. They do not prove Rust source safety,
topological scheduling, cursor reset, fixed-width evaluation, nonnumeric empty
interval handling, possible-support completeness or resource limits. In
particular, no Rust-to-Lean refinement is established by these results.
-/

namespace Zetesis.OuterRanges
open Ferraris
universe u v
variable {V : Type u} {A : Type v}

def rows (carrier : List V) (bounds : V → ChoiceIntervals.Interval) : List (V × Int) :=
  carrier.flatMap (fun outer => (ChoiceIntervals.interval (bounds outer)).map
    (fun value => (outer, value)))

/-- An emitted integer belongs to the interval of its own complete outer row. -/
theorem row_membership (carrier : List V) (bounds : V → ChoiceIntervals.Interval)
    (outer : V) (value : Int) :
    (outer, value) ∈ rows carrier bounds ↔
      outer ∈ carrier ∧ (bounds outer).lower ≤ value ∧ value ≤ (bounds outer).upper := by
  simp only [rows, List.mem_flatMap, List.mem_map]
  constructor
  · rintro ⟨original, member, integer, inside, same⟩
    cases same
    exact ⟨member, (ChoiceIntervals.mem_interval (bounds outer) value).mp inside⟩
  · intro ⟨member, inside⟩
    exact ⟨outer, member, value,
      (ChoiceIntervals.mem_interval (bounds outer) value).mpr inside, rfl⟩

/-- Coverage of the actual outer value retains every one of its integer rows. -/
theorem covered_range (carrier : List V) (bounds : V → ChoiceIntervals.Interval)
    (actual : V) (value : Int) (covered : actual ∈ carrier)
    (inside : (bounds actual).lower ≤ value ∧ value ≤ (bounds actual).upper) :
    (actual, value) ∈ rows carrier bounds :=
  (row_membership carrier bounds actual value).mpr ⟨covered, inside⟩

/-- Each new outer prefix contributes its complete expansion, even when it
    repeats an earlier value or has equal endpoints. No state is shared. -/
theorem appended_frames (before after : List V) (bounds : V → ChoiceIntervals.Interval) :
    rows (before ++ after) bounds = rows before bounds ++ rows after bounds := by
  simp [rows, List.flatMap_append]

/-- The clause function includes the original equalities and activation of
    its row. Expansion changes only which finite rows instantiate it. -/
def clauses (carrier : List V) (bounds : V → ChoiceIntervals.Interval)
    (clause : V → Int → Formula A) : Theory A :=
  (rows carrier bounds).map (fun row => clause row.1 row.2)

theorem original_rows (M : Atoms A) (carrier : List V)
    (bounds : V → ChoiceIntervals.Interval) (clause : V → Int → Formula A) :
    Models M (clauses carrier bounds clause) ↔
      ∀ outer ∈ carrier, ∀ value,
        (bounds outer).lower ≤ value ∧ value ≤ (bounds outer).upper →
          Satisfies M (clause outer value) := by
  constructor
  · intro model outer member value inside
    have row := covered_range carrier bounds outer value member inside
    exact model (clause outer value) (List.mem_map.mpr ⟨(outer, value), row, rfl⟩)
  · intro all formula member
    obtain ⟨⟨outer, value⟩, row, rfl⟩ := List.mem_map.mp member
    have covered := (row_membership carrier bounds outer value).mp row
    exact all outer covered.1 value covered.2

/-- The exact same row association holds for arbitrary frozen M/J; endpoint
    evaluation never substitutes success for a clause's equality or gate. -/
theorem frozen_rows (M J : Atoms A) (carrier : List V)
    (bounds : V → ChoiceIntervals.Interval) (clause : V → Int → Formula A) :
    Models J (ReductTheory M (clauses carrier bounds clause)) ↔
      ∀ outer ∈ carrier, ∀ value,
        (bounds outer).lower ≤ value ∧ value ≤ (bounds outer).upper →
          Satisfies J (Reduct M (clause outer value)) := by
  have expanded := original_rows J carrier bounds
    (fun outer value => Reduct M (clause outer value))
  simpa only [ReductTheory, clauses, List.map_map, Function.comp_def] using expanded

end Zetesis.OuterRanges
