import Zetesis.AggregateInvariants
import Zetesis.AggregateDependencies

/-!
# Finite source measure carriers

Required and possible complete tuple keys define a source abstraction. Its
measure carrier contains the result of every key selection between those bounds.
It need not be the set of results realized by answer sets: two optional keys may
share one condition, while the source carrier still selects them separately.
The original aggregate equality and objective conditions retain that correlation.

Lists represent finite keys in their supplied order. Distinct possible keys are
a caller obligation when these lists denote sets; equal measured values do not
identify keys. The enumerator is a mathematical finite reference, not a requested
implementation strategy. Runtime count intervals, subset sums and ordered extrema
must separately establish the same carrier membership.

These laws do not prove source classification, joins, checked i32 reduction,
ASP term comparison, allocation limits or Rust/WGSL refinement. A stopped carrier
construction supplies no completeness premise. Numeric priority presence is
source-relative; it is not reconstructed from accepted or optimal answers.

Several observers compose using `AggregateDependencies.rows`: a constant second
family represents the product of two source carriers. `row_membership` and
`covered_extension` provide product membership and coverage. Shared conditions
still constrain answer activity; they do not remove source-product slots.
-/

namespace Zetesis.SourceMeasures

universe u v w
variable {K : Type u} {V : Type v} {B : Type w}

/-- Every subsequence selects occurrences from the original finite key order. -/
def selections : List K → List (List K)
  | [] => [[]]
  | key :: rest => selections rest ++ (selections rest).map (key :: ·)

/-- Membership and subsequence selection coincide. At each key, omission and
retention are the two cases on both sides of the induction. -/
theorem selection_iff_sublist (possible selected : List K) :
    selected ∈ selections possible ↔ selected.Sublist possible := by
  induction possible generalizing selected with
  | nil => simp [selections]
  | cons key rest ih =>
    simp only [selections, List.mem_append, List.mem_map]
    constructor
    · rintro (without | ⟨tail, member, rfl⟩)
      · exact (ih selected).mp without |>.cons key
      · exact (ih tail).mp member |>.cons_cons key
    · intro contained
      cases contained with
      | cons _ inside => exact Or.inl ((ih selected).mpr inside)
      | cons_cons _ inside => exact Or.inr ⟨_, (ih _).mpr inside, rfl⟩

/-- A legal source selection contains every required key and no extra key. -/
def Admissible (required possible selected : List K) : Prop :=
  selected.Sublist possible ∧ required ⊆ selected

/-- Finite enumeration retains all measure values, including unrealized values.
Duplicate results do not affect its extensional membership contract. -/
def carrier [DecidableEq K] (required possible : List K) (measure : List K → V) : List V :=
  ((selections possible).filter (fun selected =>
    required.all (fun key => decide (key ∈ selected)))).map measure

/-- A value is retained precisely when a required/possible selection produces
it. Filtering supplies required membership; mapping supplies the result equality.
Several selections can have the same value. -/
theorem carrier_membership [DecidableEq K] (required possible : List K)
    (measure : List K → V) (value : V) :
    value ∈ carrier required possible measure ↔
      ∃ selected, Admissible required possible selected ∧ measure selected = value := by
  simp [carrier, Admissible, selection_iff_sublist, List.subset_def]

/-- Retaining actual active keys in possible-key order gives a legal selection.
No independence assumption is imposed on the activity predicate. -/
theorem actual_value_covered [DecidableEq K] (required possible : List K)
    (measure : List K → V) (active : K → Bool)
    (included : required ⊆ possible)
    (mandatory : ∀ key ∈ required, active key = true) :
    measure (possible.filter active) ∈ carrier required possible measure := by
  apply (carrier_membership required possible measure _).mpr
  refine ⟨possible.filter active, ⟨List.filter_sublist, ?_⟩, rfl⟩
  intro key member
  exact List.mem_filter.mpr ⟨included member, mandatory key member⟩

/-- The previous fixed-value certificate is the singleton special case. -/
theorem invariant_carrier [DecidableEq K] (required possible : List K)
    (measure : List K → V) (included : required.Sublist possible)
    (invariant : ∀ selected, Admissible required possible selected →
      measure selected = measure required) (value : V) :
    value ∈ carrier required possible measure ↔ value = measure required := by
  rw [carrier_membership]
  constructor
  · rintro ⟨selected, admissible, measured⟩
    exact measured.symm.trans (invariant selected admissible)
  · intro same
    exact ⟨required, ⟨included, List.Subset.refl required⟩, same.symm⟩

/-- More possible keys may enlarge the source carrier without changing the
required keys. This is source coverage, not additional producer permission. -/
theorem possible_extension [DecidableEq K] (required first second : List K)
    (measure : List K → V) (extension : first.Sublist second) :
    carrier required first measure ⊆ carrier required second measure := by
  intro value present
  obtain ⟨selected, ⟨inside, mandatory⟩, measured⟩ :=
    (carrier_membership required first measure value).mp present
  exact (carrier_membership required second measure value).mpr
    ⟨selected, ⟨inside.trans extension, mandatory⟩, measured⟩

/-- Source membership filters whole completed bindings before numeric selection.
The resolver retains weight, priority, tuple and original condition in that row;
neither accepted-answer truth nor independently projected fields occur here. -/
theorem source_priority_presence [DecidableEq V] {σ χ : Type u}
    (priority : Int) (bindings : List B) (eligible : B → Prop)
    (complete : ∀ binding, binding ∈ bindings ↔ eligible binding)
    (input : B → V) (values : List V)
    (resolve : B → ObjectiveValues.ResolvedEntry σ χ (Option Int)) :
    ObjectivePriorities.Present priority
      ((bindings.filter (fun binding => decide (input binding ∈ values))).map resolve) ↔
      ∃ binding, eligible binding ∧ input binding ∈ values ∧
        (resolve binding).priority = some priority ∧ (resolve binding).weight.isSome = true := by
  have filtered_complete (binding : B) :
      binding ∈ bindings.filter (fun row => decide (input row ∈ values)) ↔
        eligible binding ∧ input binding ∈ values := by
    simp [complete]
  rw [ObjectivePriorities.completed_presence priority _ _ filtered_complete resolve]
  simp only [and_assoc]

end Zetesis.SourceMeasures
