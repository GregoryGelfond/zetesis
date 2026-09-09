import Zetesis.WeightedHeadActivity
import Zetesis.ExtremumCandidates

/-!
# Numeric extrema over independently selected head tuples

A complete tuple key, its numeric value, a positive head atom and its eligibility
are separate objects. A tuple is selected by any one of its own eligible head
occurrences. Neither direction of a tuple/atom bijection is required. Repeated
numeric values do not identify their complete tuples or their permissions.

The canonical aggregate formula uses a complete mask expansion and agrees with
the direct numeric minimum or maximum. An empty selection has an explicit
supremum or infimum value. The activated head bound inspects only the candidate;
atom permissions retain their original eligibility reducts. Row-preserving
transformations therefore preserve stable models in arbitrary contexts.

These are laws over finite complete row tables and mathematical integers. They
do not establish source enumeration, optimized comparison compilation, numeric
endpoint admission, machine arithmetic, Rust maps or device refinement.
-/

namespace Zetesis.ExtremumHeadActivity

open Ferraris
open CountEligibility (Row tupleActivity eligibility)
open CountHeadActivity (Carrier)
open WeightedHeadActivity (selected)

universe u v
variable {A : Type u} {K : Type v}

/-- One numeric value per selected complete-key position. Equal numeric values
    may occur at several positions; no reduction of the key identity is made. -/
def values (keys : List K) (value : K → Int)
    (selection : AggregateReduct.Mask keys.length) : List Int :=
  ((List.finRange keys.length).filter selection).map (fun index => value keys[index])

/-- Every realized numeric value has a selected complete-key witness. -/
theorem mem_values (keys : List K) (value : K → Int)
    (selection : AggregateReduct.Mask keys.length) (number : Int) :
    number ∈ values keys value selection ↔
      ∃ index, selection index = true ∧ value keys[index] = number := by
  simp only [values, List.mem_map, List.mem_filter, List.mem_finRange, true_and]

/-- `true` selects minimum and `false` maximum. Empty extrema use genuine
    constructors, distinct from every finite integer. -/
def reduce (minimum : Bool) (keys : List K) (value : K → Int)
    (selection : AggregateReduct.Mask keys.length) : ExtremumCandidates.Value :=
  ExtremumCandidates.encode minimum
    (ExtremumCandidates.extreme minimum (values keys value selection))

/-- Direct extremum of the candidate's eligible, selected complete tuples. -/
noncomputable def measure (minimum : Bool) (M : Atoms A) (rows : List (Row K A))
    (keys : List K) (value : K → Int) : ExtremumCandidates.Value :=
  reduce minimum keys value (selected M rows keys)

/-- Canonical finite reference for an arbitrary fixed guard on the extremum. -/
noncomputable def formula (minimum : Bool) (rows : List (Row K A)) (keys : List K)
    (value : K → Int) (accepts : ExtremumCandidates.Value → Bool) : Formula A :=
  AggregateReduct.aggregate (fun index => tupleActivity rows keys[index])
    (fun mask => accepts (reduce minimum keys value mask))
    (AggregateReduct.masks keys.length)

/-- Canonical original truth is direct min/max evaluation over the complete
    unique tuple carrier. Aliases affect activity, never carrier identity. -/
theorem formula_original (minimum : Bool) (M : Atoms A) (rows : List (Row K A))
    (carrier : Carrier rows) (value : K → Int)
    (accepts : ExtremumCandidates.Value → Bool) :
    Satisfies M (formula minimum rows carrier.keys value accepts) ↔
      accepts (measure minimum M rows carrier.keys value) = true := by
  rw [formula, AggregateReduct.original _ _ _ _ (AggregateReduct.masks_complete _)]
  rw [WeightedHeadActivity.active_is_selected]
  rfl

open Classical in
/-- A tuple's frozen activity requires its original and tested head membership
    together with the reduct of its own eligibility, including recursion. -/
noncomputable def frozenSelection (M J : Atoms A) (rows : List (Row K A))
    (keys : List K) : AggregateReduct.Mask keys.length :=
  fun index => decide (∃ row ∈ rows, row.identity.key = keys[index] ∧
    (M row.identity.atom ∧ J row.identity.atom) ∧
      Satisfies J (Reduct M row.eligible))

/-- The aggregate formula retains original truth and recursively frozen tuple
    activity. J is arbitrary; no proper-subset assumption is used in this law. -/
theorem formula_frozen (minimum : Bool) (M J : Atoms A) (rows : List (Row K A))
    (carrier : Carrier rows) (value : K → Int)
    (accepts : ExtremumCandidates.Value → Bool) :
    Satisfies J (Reduct M (formula minimum rows carrier.keys value accepts)) ↔
      accepts (measure minimum M rows carrier.keys value) = true ∧
        accepts (reduce minimum carrier.keys value
          (frozenSelection M J rows carrier.keys)) = true := by
  have activity : AggregateReduct.active J (fun index : Fin carrier.keys.length =>
      Reduct M (tupleActivity rows carrier.keys[index])) =
      frozenSelection M J rows carrier.keys := by
    classical
    funext index
    simp only [AggregateReduct.active, frozenSelection,
      CountHeadActivity.activity_frozen, atom_reduct]
  rw [formula, AggregateReduct.direct_reduct,
    WeightedHeadActivity.active_is_selected, activity]
  rfl

/-- An empty activity set produces the appropriate infinite extremum rather
    than an invented finite numeric value. Possible rows alone do not select it. -/
theorem inactive_measure (minimum : Bool) (M : Atoms A) (rows : List (Row K A))
    (keys : List K) (value : K → Int)
    (inactive : ∀ row ∈ rows, ¬ (M row.identity.atom ∧ Satisfies M row.eligible)) :
    measure minimum M rows keys value = ExtremumCandidates.encode minimum none := by
  have no_selection : selected M rows keys = fun _ => false := by
    classical
    funext index
    apply decide_eq_false
    rintro ⟨row, member, _, atom, eligible⟩
    exact inactive row member ⟨atom, eligible⟩
  have empty_values : values keys value (selected M rows keys) = [] := by
    apply List.eq_nil_iff_forall_not_mem.mpr
    intro number member
    obtain ⟨index, active, _⟩ := (mem_values keys value _ number).mp member
    rw [no_selection] at active
    contradiction
  change ExtremumCandidates.encode minimum
    (ExtremumCandidates.extreme minimum _) = _
  rw [empty_values]
  rfl

/-- A finite extremum comes from an actually eligible selected row. Sharing a
    key with an ineligible row cannot make that row's atom a witness. -/
theorem finite_measure_has_witness (minimum : Bool) (M : Atoms A)
    (rows : List (Row K A)) (keys : List K) (value : K → Int) (number : Int)
    (finite : measure minimum M rows keys value = .number number) :
    ∃ row ∈ rows, M row.identity.atom ∧ Satisfies M row.eligible ∧
      value row.identity.key = number := by
  have realized : ExtremumCandidates.extreme minimum
      (values keys value (selected M rows keys)) = some number := by
    cases result : ExtremumCandidates.extreme minimum
      (values keys value (selected M rows keys)) with
    | none =>
      cases minimum <;> simp [measure, reduce, result, ExtremumCandidates.encode] at finite
    | some actual =>
      have same : actual = number := by
        simpa only [measure, reduce, result, ExtremumCandidates.encode,
          ExtremumCandidates.Value.number.injEq] using finite
      exact congrArg some same
  have member := ExtremumCandidates.extreme_some_mem minimum _ number realized
  obtain ⟨index, active, numeric⟩ := (mem_values keys value _ number).mp member
  have witness : ∃ row ∈ rows, row.identity.key = keys[index] ∧
      M row.identity.atom ∧ Satisfies M row.eligible := by
    simpa only [selected, decide_eq_true_eq] using active
  obtain ⟨row, belongs, key, atom, eligible⟩ := witness
  exact ⟨row, belongs, atom, eligible, by rw [key]; exact numeric⟩

/-- Duplicate or reordered complete rows leave the numerical extremum unchanged.
    This preserves complete keys rather than merging them by numeric value. -/
theorem same_rows_preserve_measure (minimum : Bool) (M : Atoms A)
    (first second : List (Row K A)) (same : ∀ row, row ∈ first ↔ row ∈ second)
    (keys : List K) (value : K → Int) :
    measure minimum M first keys value = measure minimum M second keys value := by
  have selection : selected M first keys = selected M second keys := by
    classical
    funext index
    simp only [selected, same]
  unfold measure
  rw [selection]

/-- Activated extremum bounds constrain M without granting an atom support in J. -/
theorem bound_frozen (minimum : Bool) (M J : Atoms A) (rows : List (Row K A))
    (carrier : Carrier rows) (value : K → Int)
    (accepts : ExtremumCandidates.Value → Bool) (body : Formula A) :
    Satisfies J (Reduct M (HeadMeasures.bound body
      (formula minimum rows carrier.keys value accepts))) ↔
      (Satisfies M body →
        accepts (measure minimum M rows carrier.keys value) = true) := by
  rw [HeadMeasures.bound_frozen, formula_original]

/-- Row-preserving changes preserve both independent atom permissions and the
    extremum bound in every context, including when several heads share a tuple. -/
theorem stable_in_context (minimum : Bool) (M : Atoms A)
    (first second : List (Row K A)) (same : ∀ row, row ∈ first ↔ row ∈ second)
    (first_carrier : Carrier first) (second_carrier : Carrier second)
    (same_keys : first_carrier.keys = second_carrier.keys)
    (value : K → Int) (accepts : ExtremumCandidates.Value → Bool)
    (body : Formula A) (heads : List A) (context : Theory A) :
    Stable M (HeadMeasures.group body heads (eligibility first)
      (formula minimum first first_carrier.keys value accepts) :: context) ↔
    Stable M (HeadMeasures.group body heads (eligibility second)
      (formula minimum second second_carrier.keys value accepts) :: context) := by
  have numeric (candidate : Atoms A) :
      Satisfies candidate (formula minimum first first_carrier.keys value accepts) ↔
        Satisfies candidate (formula minimum second second_carrier.keys value accepts) := by
    rw [formula_original, formula_original, same_keys,
      same_rows_preserve_measure minimum candidate first second same]
  exact HeadMeasures.stable_in_context M body heads _ _ _ _
    (CountHeadActivity.eligibility_of_same_rows first second same) numeric context

end Zetesis.ExtremumHeadActivity
