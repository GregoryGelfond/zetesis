import Zetesis.ObjectivePriorities
import Zetesis.ValueExtrema

/-!
# Invariant values between required and possible aggregate tuples

A finite carrier contains complete tuple keys. Required keys are active in every
admitted realization; other keys are optional, possibly correlated. Optional
zero contributions cannot change the required sum. Optional values dominated by
the required extremum cannot change that extremum. Min and max share the ordered
law by reversing the comparison; their empty endpoints are explicit values.

These invariants justify a singleton aggregate-value carrier without enumerating
optional choices or answer sets. The objective corollary composes a fixed resolved
priority with complete eligible rows, preserving the same-row numeric weight
witness. It does not replace aggregate equalities or model-relative conditions.

The source adapter must establish complete-key coalescing, required/possible
classification, exact eligible rows and unique predicate transport. Integer sums
are mathematical Int; term ordering, checked i32 evaluation and intermediate
overflow, source errors, resource completion and Rust/WGSL refinement are not
proved here. A numerical invariant does not itself prove source eligibility.
-/

namespace Zetesis.AggregateInvariants

universe u v w
variable {K : Type u} {V : Type v} {B : Type w}

/-- Select contributions from one finite complete-key carrier. An adapter uses
    distinct keys; equal measured values need not denote the same key. -/
def sum (keys : List K) (weight : K → Int) (selected : K → Bool) : Int :=
  (keys.map (fun key => if selected key then weight key else 0)).sum

/-- Every required key survives; every other contribution is zero. The result
    is independent of correlations among the optional selections. -/
theorem sum_invariant (keys : List K) (weight : K → Int)
    (required actual : K → Bool)
    (mandatory : ∀ key ∈ keys, required key = true → actual key = true)
    (inert : ∀ key ∈ keys, required key = false → weight key = 0) :
    sum keys weight actual = sum keys weight required := by
  have contributions : keys.map (fun key => if actual key then weight key else 0) =
      keys.map (fun key => if required key then weight key else 0) := by
    apply List.map_congr_left
    intro key member
    cases fixed : required key with
    | false => simp [inert key member fixed]
    | true => simp [mandatory key member fixed]
  exact congrArg List.sum contributions

/-- Ordered selection retains the specified empty value, not a numeric proxy. -/
def extremum (before : V → V → Bool) (empty : V) (value : K → V)
    (keys : List K) : V :=
  ValueExtrema.encode empty (ValueExtrema.extreme before (keys.map value))

/-- Every realization contains the required keys and lies within the possible
    carrier. Optional values are dominated by the required result. Explicit
    ordered-selection laws make this one theorem apply to both min and max. -/
theorem extremum_invariant (before : V → V → Bool) (empty : V)
    (dominates : V → V → Prop)
    (reflexive : ∀ value, dominates value value)
    (antisymmetric : ∀ left right,
      dominates left right → dominates right left → left = right)
    (selection : ∀ bound left right,
      dominates (ValueExtrema.choose before left right) bound ↔
        dominates left bound ∨ dominates right bound)
    (empty_bound : ∀ value, dominates value empty)
    (value : K → V) (required actual possible : List K)
    (mandatory : ∀ key ∈ required, key ∈ actual)
    (covered : ∀ key ∈ actual, key ∈ possible)
    (inert : ∀ key ∈ possible, key ∉ required →
      dominates (extremum before empty value required) (value key)) :
    extremum before empty value actual = extremum before empty value required := by
  classical
  have below_member (keys : List K) (key : K) (member : key ∈ keys) :
      dominates (extremum before empty value keys) (value key) := by
    cases selected : ValueExtrema.extreme before (keys.map value) with
    | none =>
      have absent := (ValueExtrema.extreme_none_iff before (keys.map value)).mp selected
      have present : value key ∈ keys.map value := List.mem_map.mpr ⟨key, member, rfl⟩
      simp [absent] at present
    | some result =>
      have bounded : dominates result (value key) :=
        (ValueExtrema.selected_predicate before (fun item => dominates item (value key))
          (selection (value key)) (keys.map value) result selected).mpr
          ⟨value key, List.mem_map.mpr ⟨key, member, rfl⟩, reflexive (value key)⟩
      simpa [extremum, selected, ValueExtrema.encode] using bounded
  have required_bounds : ∀ key ∈ actual,
      dominates (extremum before empty value required) (value key) := by
    intro key member
    by_cases fixed : key ∈ required
    · exact below_member required key fixed
    · exact inert key (covered key member) fixed
  have required_dominates : dominates (extremum before empty value required)
      (extremum before empty value actual) := by
    cases selected : ValueExtrema.extreme before (actual.map value) with
    | none => simpa [extremum, selected, ValueExtrema.encode] using
        empty_bound (extremum before empty value required)
    | some result =>
      obtain ⟨key, member, same⟩ := List.mem_map.mp
        (ValueExtrema.extreme_some_mem before (actual.map value) result selected)
      have bounded := required_bounds key member
      simpa [extremum, selected, ValueExtrema.encode, same] using bounded
  have actual_dominates : dominates (extremum before empty value actual)
      (extremum before empty value required) := by
    cases selected : ValueExtrema.extreme before (required.map value) with
    | none => simpa [extremum, selected, ValueExtrema.encode] using
        empty_bound (extremum before empty value actual)
    | some result =>
      obtain ⟨key, member, same⟩ := List.mem_map.mp
        (ValueExtrema.extreme_some_mem before (required.map value) result selected)
      have bounded := below_member actual key (mandatory key member)
      simpa [extremum, selected, ValueExtrema.encode, same] using bounded
  exact antisymmetric _ _ actual_dominates required_dominates

/-- A fixed resolved priority does not remove the same-row numeric-weight
    requirement. Complete eligibility and resolution are supplied separately
    from the aggregate invariant and from truth in an accepted answer set. -/
theorem fixed_priority_presence {σ : Type u} {χ : Type v}
    (priority : Int) (fixed : Option Int) (bindings : List B) (eligible : B → Prop)
    (complete : ∀ binding, binding ∈ bindings ↔ eligible binding)
    (resolve : B → ObjectiveValues.ResolvedEntry σ χ (Option Int))
    (invariant : ∀ binding, eligible binding → (resolve binding).priority = fixed) :
    ObjectivePriorities.Present priority (bindings.map resolve) ↔
      fixed = some priority ∧ ∃ binding, eligible binding ∧ (resolve binding).weight.isSome = true := by
  rw [ObjectivePriorities.completed_presence priority bindings eligible complete resolve]
  constructor
  · rintro ⟨binding, admitted, atPriority, numeric⟩
    exact ⟨(invariant binding admitted).symm.trans atPriority, binding, admitted, numeric⟩
  · rintro ⟨atPriority, binding, admitted, numeric⟩
    exact ⟨binding, admitted, (invariant binding admitted).trans atPriority, numeric⟩

end Zetesis.AggregateInvariants
