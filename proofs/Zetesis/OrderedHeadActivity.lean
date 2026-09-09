import Zetesis.SignedHeadElements
import Zetesis.ValueExtrema

/-!
# Ordered values in signed extrema heads

Complete tuple identity, logical first value, signed activity and positive atom
permission remain separate. The value carrier is arbitrary: this module does not
coerce symbolic, string, structured or extremal values into mathematical integers.
An explicit selection function chooses a minimum or maximum, with a supplied
logical empty value. Its ordering laws are premises, not hidden assumptions about
the Rust comparator.

Canonical aggregate truth agrees with direct ordered selection, and the Ferraris
reduct preserves both the candidate guard and the guard over frozen signed
activity. An activated head bound then tests only the candidate. It uses the
existing SignedHeadElements permissions, which introduce producers only for
unsigned atoms; changing the value carrier changes no permission definition.

The complete-carrier law requires coverage of every row key. For source extrema,
keys are complete distinct tuples and the value map must give each nonempty
tuple's actual first logical value. Source joins, term ordering, empty min=#sup
and max=#inf, endpoint admission, formula lowering, resource completion and
Rust/WGSL refinement remain separate obligations. Missing tuples and disputed
sum contribution rules are not assigned a semantics by these laws.
-/

namespace Zetesis.OrderedHeadActivity

universe u v w
variable {A : Type u} {K : Type v} {V : Type w}
open Ferraris
open SignedHeadElements (Row activity)

/-- Selected complete-key positions retain their logical values. Equal values
    do not identify the keys or merge their permissions. -/
def values (keys : List K) (value : K → V)
    (selection : AggregateReduct.Mask keys.length) : List V :=
  ((List.finRange keys.length).filter selection).map (fun index => value keys[index])

theorem mem_values (keys : List K) (value : K → V)
    (selection : AggregateReduct.Mask keys.length) (result : V) :
    result ∈ values keys value selection ↔
      ∃ index, selection index = true ∧ value keys[index] = result := by
  simp only [values, List.mem_map, List.mem_filter, List.mem_finRange, true_and]

/-- The original selection uses signed operands and their own eligibility. -/
noncomputable def selected (M : Atoms A) (rows : List (Row K A))
    (keys : List K) : AggregateReduct.Mask keys.length :=
  AggregateReduct.active M (fun index => activity rows keys[index])

/-- Selection in a frozen reduct keeps M fixed while testing every activity at J. -/
noncomputable def frozenSelection (M J : Atoms A) (rows : List (Row K A))
    (keys : List K) : AggregateReduct.Mask keys.length :=
  AggregateReduct.active J (fun index => Reduct M (activity rows keys[index]))

/-- Signed operands freeze differently from positive operands; eligibility
    always retains its own reduct. No subset assumption on J is needed. -/
theorem frozen_selection (M J : Atoms A) (rows : List (Row K A))
    (keys : List K) (index : Fin keys.length) :
    frozenSelection M J rows keys index = true ↔
      ∃ row ∈ rows, row.key = keys[index] ∧ SignedHeadElements.FrozenHolds M J row.head ∧
        Satisfies J (Reduct M row.eligible) := by
  simp only [frozenSelection, AggregateReduct.active, decide_eq_true_eq,
    SignedHeadElements.activity_frozen]

/-- Exact value coverage follows from coverage of complete row keys. Repeated
    values are allowed: their witnesses remain distinct rows and complete keys. -/
theorem selected_values (M : Atoms A) (rows : List (Row K A)) (keys : List K)
    (value : K → V) (coverage : ∀ row ∈ rows, row.key ∈ keys) (result : V) :
    result ∈ values keys value (selected M rows keys) ↔
      ∃ row ∈ rows, SignedHeadElements.Holds M row.head ∧
        Satisfies M row.eligible ∧ value row.key = result := by
  rw [mem_values]
  constructor
  · rintro ⟨index, active, same_value⟩
    have truth : Satisfies M (activity rows keys[index]) := by
      simpa only [selected, AggregateReduct.active, decide_eq_true_eq] using active
    obtain ⟨row, member, same_key, head, eligible⟩ :=
      (SignedHeadElements.activity_original M rows keys[index]).mp truth
    exact ⟨row, member, head, eligible, by rw [same_key]; exact same_value⟩
  · rintro ⟨row, member, head, eligible, same_value⟩
    obtain ⟨index, same_key⟩ := List.mem_iff_get.mp (coverage row member)
    change keys[index] = row.key at same_key
    refine ⟨index, ?_, ?_⟩
    · simp only [selected, AggregateReduct.active, decide_eq_true_eq]
      exact (SignedHeadElements.activity_original M rows keys[index]).mpr
        ⟨row, member, same_key.symm, head, eligible⟩
    · simpa only [same_key] using same_value

/-- The empty value is an explicit logical argument, not a finite numeric proxy. -/
def reduce (before : V → V → Bool) (empty : V) (keys : List K) (value : K → V)
    (selection : AggregateReduct.Mask keys.length) : V :=
  ValueExtrema.encode empty (ValueExtrema.extreme before (values keys value selection))

noncomputable def measure (before : V → V → Bool) (empty : V)
    (M : Atoms A) (rows : List (Row K A)) (keys : List K) (value : K → V) : V :=
  reduce before empty keys value (selected M rows keys)

/-- The canonical formula enumerates complete activity masks. Efficient witness
    formulas must separately be shown to have the same original/frozen meaning. -/
noncomputable def formula (before : V → V → Bool) (empty : V)
    (rows : List (Row K A)) (keys : List K) (value : K → V)
    (accepts : V → Bool) : Formula A :=
  SignedHeadElements.formula rows keys (fun mask => accepts (reduce before empty keys value mask))

theorem formula_original (before : V → V → Bool) (empty : V)
    (M : Atoms A) (rows : List (Row K A)) (keys : List K) (value : K → V)
    (accepts : V → Bool) :
    Satisfies M (formula before empty rows keys value accepts) ↔
      accepts (measure before empty M rows keys value) = true := by
  rw [formula, SignedHeadElements.formula,
    AggregateReduct.original _ _ _ _ (AggregateReduct.masks_complete _)]
  rfl

/-- The same ordered guard is applied to original and frozen selections.
    Testing only the original extremum would lose the aggregate's reduct. -/
theorem formula_frozen (before : V → V → Bool) (empty : V)
    (M J : Atoms A) (rows : List (Row K A)) (keys : List K) (value : K → V)
    (accepts : V → Bool) :
    Satisfies J (Reduct M (formula before empty rows keys value accepts)) ↔
      accepts (measure before empty M rows keys value) = true ∧
        accepts (reduce before empty keys value (frozenSelection M J rows keys)) = true := by
  rw [formula, SignedHeadElements.formula, AggregateReduct.direct_reduct]
  rfl

/-- Ordered-selection laws justify predicate witnesses across any admitted value
    classes. The empty case is explicit; key coverage is needed in both directions. -/
theorem measure_predicate (before : V → V → Bool) (empty : V) (predicate : V → Prop)
    (selection : ∀ left right, predicate (ValueExtrema.choose before left right) ↔
      predicate left ∨ predicate right)
    (M : Atoms A) (rows : List (Row K A)) (keys : List K) (value : K → V)
    (coverage : ∀ row ∈ rows, row.key ∈ keys) :
    predicate (measure before empty M rows keys value) ↔
      ((¬ ∃ row ∈ rows, SignedHeadElements.Holds M row.head ∧ Satisfies M row.eligible) ∧
        predicate empty) ∨
      ∃ row ∈ rows, SignedHeadElements.Holds M row.head ∧ Satisfies M row.eligible ∧
        predicate (value row.key) := by
  have empty_selection : values keys value (selected M rows keys) = [] ↔
      ¬ ∃ row ∈ rows, SignedHeadElements.Holds M row.head ∧ Satisfies M row.eligible := by
    rw [List.eq_nil_iff_forall_not_mem]
    simp only [selected_values M rows keys value coverage]
    constructor
    · intro absent ⟨row, member, head, eligible⟩
      exact absent (value row.key) ⟨row, member, head, eligible, rfl⟩
    · intro absent result ⟨row, member, head, eligible, _⟩
      exact absent ⟨row, member, head, eligible⟩
  have witnesses : (∃ result ∈ values keys value (selected M rows keys), predicate result) ↔
      ∃ row ∈ rows, SignedHeadElements.Holds M row.head ∧ Satisfies M row.eligible ∧
        predicate (value row.key) := by
    simp only [selected_values M rows keys value coverage]
    constructor
    · rintro ⟨_, ⟨row, member, head, eligible, rfl⟩, accepted⟩
      exact ⟨row, member, head, eligible, accepted⟩
    · rintro ⟨row, member, head, eligible, accepted⟩
      exact ⟨value row.key, ⟨row, member, head, eligible, rfl⟩, accepted⟩
  change predicate (ValueExtrema.encode empty (ValueExtrema.extreme before _)) ↔ _
  rw [ValueExtrema.selected_or_empty_predicate before empty predicate selection,
    empty_selection, witnesses]

/-- The surrounding head constraint freezes this ordered measure in M.
    Permissions remain the independent signed-head construction. -/
theorem bound_frozen (before : V → V → Bool) (empty : V)
    (M J : Atoms A) (rows : List (Row K A)) (keys : List K) (value : K → V)
    (accepts : V → Bool) (body : Formula A) :
    Satisfies J (Reduct M (HeadMeasures.bound body
      (formula before empty rows keys value accepts))) ↔
      (Satisfies M body → accepts (measure before empty M rows keys value) = true) := by
  rw [HeadMeasures.bound_frozen, formula_original]

end Zetesis.OrderedHeadActivity
