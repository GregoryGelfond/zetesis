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

/-- With no required keys, the count carrier contains exactly the natural
numbers up to the number of possible occurrences. A selection cannot be longer
than its source; conversely, taking the first `value` occurrences realizes every
value within that bound.

This law counts list occurrences. An aggregate caller must first provide distinct
complete tuple keys to interpret that length as ASP count; equal measured values
alone do not identify keys. Checked numeric conversion and resource limits remain
implementation obligations. -/
theorem count_carrier_membership [DecidableEq K] (possible : List K) (value : Nat) :
    value ∈ carrier [] possible List.length ↔ value ≤ possible.length := by
  rw [carrier_membership]
  constructor
  · rintro ⟨selected, ⟨inside, _⟩, measured⟩
    have count_bounded : selected.length ≤ possible.length := inside.length_le
    simpa only [measured] using count_bounded
  · intro bounded
    have prefix_admissible : Admissible [] possible (possible.take value) :=
      ⟨List.take_sublist value possible, List.nil_subset _⟩
    exact ⟨possible.take value, prefix_admissible, List.length_take_of_le bounded⟩

/-- With no required occurrences and no possible occurrences, the only
optional integer sum is zero. This is the initial carrier for either a sorted
or a packed finite-sum operation; it specifies no storage representation. -/
theorem sum_carrier_nil :
    carrier [] ([] : List Int) List.sum = [0] := by
  simp [carrier, selections]

/-- Adding one optional integer occurrence keeps every previous sum and adds
its translation by that weight. No sign, nonzero or distinctness premise is
needed. Equal weights remain separate occurrences, so two occurrences of weight
w can contribute twice even though repeated result values need not be retained.

Split selections into those omitting the new occurrence and those retaining it.
Mapping List.sum turns retention into addition by the new weight. The final
membership statement is extensional: it does not count witnesses or prescribe
an enumeration order. Correlated source conditions may make some selections
unrealizable; ASP callers must separately supply one contribution per complete
tuple key and retain the original aggregate condition. -/
theorem sum_carrier_cons (weight : Int) (possible : List Int) (value : Int) :
    value ∈ carrier [] (weight :: possible) List.sum ↔
      value ∈ carrier [] possible List.sum ∨
        ∃ previous ∈ carrier [] possible List.sum, weight + previous = value := by
  have split : carrier [] (weight :: possible) List.sum =
      carrier [] possible List.sum ++
        (carrier [] possible List.sum).map (fun previous => weight + previous) := by
    have noRequiredKeys (rows : List (List Int)) :
        rows.filter (fun _ => true) = rows := by
      apply List.filter_eq_self.mpr
      intro row member
      rfl
    simp [carrier, selections, noRequiredKeys, List.map_append, List.map_map,
      Function.comp_def]
  rw [split, List.mem_append]
  simp only [List.mem_map]

/-- A selection's total under a key-to-weight map: the sum measure over complete
keys, where equal weights on distinct keys stay separate contributions. -/
def weighted (weight : K → Int) (selected : List K) : Int :=
  (selected.map weight).sum

/-- Splitting a list by a predicate and its negation splits its total. -/
theorem weighted_filter_split (weight : K → Int) (keep : K → Bool) (keys : List K) :
    weighted weight keys =
      weighted weight (keys.filter keep) + weighted weight (keys.filter (fun key => !keep key)) := by
  induction keys with
  | nil => simp [weighted]
  | cons head rest ih =>
    simp only [weighted, List.map_cons, List.sum_cons] at ih ⊢
    cases decision : keep head <;> simp [decision, ih] <;> omega

/-- Permuting a selection does not change its total. -/
theorem weighted_perm (weight : K → Int) {left right : List K}
    (permutation : left.Perm right) : weighted weight left = weighted weight right := by
  induction permutation with
  | nil => rfl
  | cons head permutation ih => simp [weighted] at ih ⊢; omega
  | swap first second rest => simp [weighted]; omega
  | trans first second ihFirst ihSecond => exact ihFirst.trans ihSecond

/-- Splitting a list by a predicate and its negation splits its length. -/
theorem length_filter_split (keep : K → Bool) (keys : List K) :
    keys.length = (keys.filter keep).length + (keys.filter (fun key => !keep key)).length := by
  induction keys with
  | nil => simp
  | cons head rest ih => cases decision : keep head <;> simp [decision] <;> omega

/-- Over distinct possible keys, a selection that contains every required key
keeps exactly the required keys when filtered by membership in them, up to
order. -/
theorem required_part_perm [DecidableEq K] (required possible selected : List K)
    (distinct : possible.Nodup) (requiredDistinct : required.Nodup)
    (inside : selected.Sublist possible) (mandatory : required ⊆ selected) :
    (selected.filter (fun key => decide (key ∈ required))).Perm required := by
  apply (List.perm_ext_iff_of_nodup
    ((inside.nodup distinct).sublist List.filter_sublist) requiredDistinct).mpr
  intro key
  simp only [List.mem_filter, decide_eq_true_eq]
  exact ⟨And.right, fun member => ⟨mandatory member, member⟩⟩

/-- The selection made of the required keys and a choice of optional keys, in
the possible keys' order. -/
def assembled [DecidableEq K] (required possible chosen : List K) : List K :=
  possible.filter (fun key => decide (key ∈ required ∨ key ∈ chosen))

/-- Over distinct possible keys with the required keys among them, the selection
assembled from the required keys and distinct optional keys `chosen` is
admissible, and it splits back into its parts: its required part is a
permutation of the required keys and its optional part a permutation of
`chosen`. -/
theorem assembled_split [DecidableEq K] (required possible chosen : List K)
    (distinct : possible.Nodup) (requiredDistinct : required.Nodup)
    (included : required ⊆ possible) (chosenDistinct : chosen.Nodup)
    (optional : ∀ key ∈ chosen, key ∈ possible ∧ key ∉ required) :
    ((assembled required possible chosen).Sublist possible ∧
        required ⊆ assembled required possible chosen) ∧
      ((assembled required possible chosen).filter
          (fun key => decide (key ∈ required))).Perm required ∧
      ((assembled required possible chosen).filter
          (fun key => !decide (key ∈ required))).Perm chosen := by
  have mandatory : required ⊆ assembled required possible chosen := fun key member =>
    List.mem_filter.mpr ⟨included member, by simp [member]⟩
  refine ⟨⟨List.filter_sublist, mandatory⟩, ?_, ?_⟩
  · exact required_part_perm required possible _ distinct requiredDistinct
      List.filter_sublist mandatory
  · apply (List.perm_ext_iff_of_nodup
      ((distinct.sublist List.filter_sublist).sublist List.filter_sublist) chosenDistinct).mpr
    intro key
    simp only [List.mem_filter, decide_eq_true_eq, Bool.not_eq_true', decide_eq_false_iff_not]
    constructor
    · rintro ⟨⟨_, required | picked⟩, absent⟩
      · exact absurd required absent
      · exact picked
    · intro picked
      exact ⟨⟨(optional key picked).1, Or.inr picked⟩, (optional key picked).2⟩

/-- With distinct possible keys and the required keys among them, a sum carrier
value is the required keys' total plus a subset sum of the optional keys'
weights. A selection splits into its required part, a permutation of the
required keys, and its optional part, a selection of the optional keys;
conversely the possible keys filtered by the required keys and an optional
selection form a selection. This is the denotation of the seeded sum
operation; over weights used as keys `Admissible` would not require each
occurrence. -/
theorem required_sum_carrier_membership [DecidableEq K] (weight : K → Int)
    (required possible : List K) (distinct : possible.Nodup)
    (requiredDistinct : required.Nodup) (included : required ⊆ possible) (value : Int) :
    value ∈ carrier required possible (weighted weight) ↔
      ∃ optional ∈ carrier [] (possible.filter (fun key => decide (key ∉ required)))
          (weighted weight),
        value = weighted weight required + optional := by
  rw [carrier_membership]
  constructor
  · rintro ⟨selected, ⟨inside, mandatory⟩, measured⟩
    refine ⟨weighted weight (selected.filter (fun key => decide (key ∉ required))), ?_, ?_⟩
    · apply (carrier_membership _ _ _ _).mpr
      exact ⟨_, ⟨inside.filter _, List.nil_subset _⟩, rfl⟩
    · rw [← measured, weighted_filter_split weight (fun key => decide (key ∈ required)) selected,
        weighted_perm weight
          (required_part_perm required possible selected distinct requiredDistinct inside mandatory)]
      congr 2
      apply List.filter_congr
      intro key _
      simp
  · rintro ⟨optional, present, rfl⟩
    obtain ⟨chosen, ⟨within, _⟩, measured⟩ := (carrier_membership _ _ _ _).mp present
    have chosenDistinct : chosen.Nodup := within.nodup (distinct.sublist List.filter_sublist)
    have optionalKeys : ∀ key ∈ chosen, key ∈ possible ∧ key ∉ required := by
      intro key picked
      have optionalKey := within.subset picked
      simp only [List.mem_filter, decide_eq_true_eq] at optionalKey
      exact optionalKey
    obtain ⟨admissible, requiredPart, optionalPart⟩ := assembled_split required possible chosen
      distinct requiredDistinct included chosenDistinct optionalKeys
    refine ⟨assembled required possible chosen, admissible, ?_⟩
    rw [weighted_filter_split weight (fun key => decide (key ∈ required))
        (assembled required possible chosen),
      weighted_perm weight requiredPart, weighted_perm weight optionalPart, measured]

/-- With distinct possible keys and the required keys among them, the count
carrier contains exactly the naturals from the number of required keys to the
number of possible keys. A selection contains the required keys, which its
membership filter keeps up to order, and lies within the possible keys;
conversely the required keys together with a prefix of the optional keys
realize each value between the bounds. -/
theorem required_count_carrier_membership [DecidableEq K] (required possible : List K)
    (distinct : possible.Nodup) (requiredDistinct : required.Nodup)
    (included : required ⊆ possible) (value : Nat) :
    value ∈ carrier required possible List.length ↔
      required.length ≤ value ∧ value ≤ possible.length := by
  rw [carrier_membership]
  have split := length_filter_split (fun key => decide (key ∈ required)) possible
  have requiredLength :
      (possible.filter (fun key => decide (key ∈ required))).length = required.length :=
    (required_part_perm required possible possible distinct requiredDistinct
      (List.Sublist.refl possible) included).length_eq
  constructor
  · rintro ⟨selected, ⟨inside, mandatory⟩, rfl⟩
    have own := (required_part_perm required possible selected distinct requiredDistinct inside
      mandatory).length_eq
    exact ⟨own ▸ List.length_filter_le _ _, inside.length_le⟩
  · rintro ⟨atLeast, atMost⟩
    let optional := possible.filter (fun key => !decide (key ∈ required))
    let chosen := optional.take (value - required.length)
    have chosenDistinct : chosen.Nodup :=
      (List.take_sublist _ _).nodup (distinct.sublist List.filter_sublist)
    have optionalKeys : ∀ key ∈ chosen, key ∈ possible ∧ key ∉ required := by
      intro key picked
      have optionalKey := (List.take_sublist _ _).subset picked
      simp only [optional, List.mem_filter, Bool.not_eq_true', decide_eq_false_iff_not]
        at optionalKey
      exact optionalKey
    obtain ⟨admissible, requiredPart, optionalPart⟩ := assembled_split required possible chosen
      distinct requiredDistinct included chosenDistinct optionalKeys
    refine ⟨assembled required possible chosen, admissible, ?_⟩
    have total := length_filter_split (fun key => decide (key ∈ required))
      (assembled required possible chosen)
    have optionalLength : optional.length + required.length = possible.length := by
      simp only [optional]
      omega
    rw [total, requiredPart.length_eq, optionalPart.length_eq, List.length_take]
    omega

end Zetesis.SourceMeasures
