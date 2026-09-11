import Zetesis.SignedHeadElements
import Zetesis.OrderedHeadActivity

/-!
# Neutral aggregate-head contributions

Abstract Gringo gives missing or nonnumeric sum tuples weight zero and generates
positive head choices independently of tuple weights. A positive-only sum also
ignores nonpositive numeric weights without removing their head permissions.
Unbounded heads consist of those choices without a measure constraint.

For extrema, the declared finite extension projects the present first values
before ordered reduction. Missing first values contribute no measure, while
complete values retain the existing comparator and empty endpoint. The laws
establish conservation on complete-value families, exact selected-value coverage,
and candidate-only bound preservation with unchanged permissions. This extension
is not claimed to be uniquely determined by Abstract Gringo's extrema clauses.

The finite numeric laws below concern the already coalesced active tuple family.
The frozen permission witness distinguishes a neutral contribution from deleting
its head. An absent bound can be removed in arbitrary original and frozen worlds
and therefore in every surrounding theory. Existing signed-activity laws retain
eligibility and its reduct; numeric contribution does not filter their rows.

Source syntax, complete binding and tuple carriers, checked arithmetic, provenance,
resource accounting and Rust refinement remain separate obligations. The optional
value represents absence of a tuple component, never failed source evaluation.
-/

namespace Zetesis.HeadContributions

open Ferraris ChoiceIntervals
universe u
variable {A : Type u}

/-- Discarding missing numeric contributions adds exactly the same sum as giving
those tuples weight zero. The list represents active complete tuple keys, not
source occurrences. Proof outline: the first optional value either adds its
integer on both sides or contributes zero; apply the remaining-list equality. -/
theorem neutral_values_are_zero (values : List (Option Int)) :
    (values.filterMap id).sum = (values.map (fun value => value.getD 0)).sum := by
  induction values with
  | nil => rfl
  | cons value rest remaining =>
    cases value with
    | none => simpa using remaining
    | some integer => simpa using congrArg (integer + ·) remaining

/-- Positive-only selection gives missing values and nonpositive integers no
numeric contribution. Their independently generated permissions remain present. -/
def positiveContribution (value : Option Int) : Option Int :=
  value.filter (fun integer => integer > 0)

/-- A nonpositive numeric value is neutral for positive-only summation. -/
theorem nonpositive_is_neutral (value : Int) (nonpositive : value ≤ 0) :
    positiveContribution (some value) = none := by
  have not_positive : ¬ value > 0 := Int.not_lt.mpr nonpositive
  simp [positiveContribution, Option.filter, not_positive]

/-- Selecting positive numeric contributions gives exactly the positive part of
the tuple-weight sum, where absent numeric values have weight zero.
Proof outline: split the first optional value and its sign, then add the identical
remaining sum. No distinct-weight premise is required. -/
theorem positive_measure (values : List (Option Int)) :
    (values.filterMap positiveContribution).sum =
      (values.map (fun value => if value.getD 0 > 0 then value.getD 0 else 0)).sum := by
  induction values with
  | nil => rfl
  | cons value rest remaining =>
    cases value with
    | none =>
      change (rest.filterMap positiveContribution).sum =
        0 + (rest.map (fun value => if value.getD 0 > 0 then value.getD 0 else 0)).sum
      simpa only [Int.zero_add] using remaining
    | some integer =>
      by_cases positive : integer > 0
      · simpa [positiveContribution, Option.filter, positive] using congrArg (integer + ·) remaining
      · simpa [positiveContribution, Option.filter, positive] using remaining

/-- An unbounded head has no candidate measure constraint. Replacing that absent
constraint by truth changes neither original satisfaction nor any frozen reduct.
The arbitrary permission formula is retained intact, including its eligibility.
Proof outline: a true measure makes the candidate-only bound true in both worlds;
conjoining that truth leaves the supplied permission formula unchanged. -/
theorem unbounded_equivalent (body permissions : Formula A) :
    Equivalent (.conj permissions (HeadMeasures.bound body top)) permissions := by
  constructor
  · intro M
    change (Satisfies M permissions ∧ Satisfies M (HeadMeasures.bound body top)) ↔ _
    rw [HeadMeasures.bound_original]
    simp [Satisfies, top]
  · intro M J
    rw [RuleFactorization.reduct_conj, HeadMeasures.bound_frozen]
    simp [Satisfies, top]

/-- Removing an absent bound preserves answer sets in arbitrary context.
Apply original and frozen equivalence to the model and proper-subset clauses
of stability; all supplied head permissions remain in the theory. -/
theorem unbounded_in_context (M : Atoms A) (body permissions : Formula A)
    (context : Theory A) :
    Stable M (.conj permissions (HeadMeasures.bound body top) :: context) ↔
      Stable M (permissions :: context) := by
  have preservation := unbounded_equivalent body permissions
  simp only [Stable, models_cons, ReductTheory, List.map_cons,
    preservation.1, preservation.2]

/-- A head permission cannot be erased just because its numeric contribution is
zero. With body and eligibility true, selecting the atom in M requires it in J.
Truth alone has no such obligation. These concrete worlds distinguish the two
formulas even when an aggregate bound is absent or identically true. -/
theorem neutral_permission_is_not_truth :
    ¬ Satisfies Empty (Reduct Full
      (SignedHeadElements.permission (top : Formula Unit) top ⟨.positive, .atom ()⟩)) ∧
    Satisfies Empty (Reduct Full (top : Formula Unit)) := by
  simp [SignedHeadElements.permission, SignedHeadElements.operand,
    BooleanHeadElements.operand, top, Ferraris.Neg, Reduct, Satisfies, Empty, Full]

section Extrema

universe v w
variable {K : Type v} {V : Type w}

/-- Reduce present first components using the existing ordered selector. Absence
does not insert a numeric proxy or remove the source row's separate permission.
The empty endpoint is supplied explicitly: supremum for min, infimum for max. -/
def extremum (before : V → V → Bool) (empty : V) (values : List (Option V)) : V :=
  ValueExtrema.encode empty (ValueExtrema.extreme before (values.filterMap id))

/-- Adding a missing first component leaves the measure unchanged. The proof
removes that absent value from the projected list, without changing any row. -/
theorem missing_extremum_is_neutral (before : V → V → Bool) (empty : V)
    (values : List (Option V)) :
    extremum before empty (none :: values) = extremum before empty values := by
  simp [extremum]

/-- On complete-value families the extension is exactly the prior reduction,
including its empty-family result. Projecting the present values recovers the
original list in order; no order-law assumption is needed for this equality. -/
theorem complete_extremum_conservative (before : V → V → Bool) (empty : V)
    (values : List V) :
    extremum before empty (values.map some) =
      ValueExtrema.encode empty (ValueExtrema.extreme before values) := by
  simp [extremum]

/-- Any finite all-missing family yields the supplied logical empty endpoint.
This concerns the measure only, not whether the head has any permissions. -/
theorem only_missing_extremum (before : V → V → Bool) (empty : V) (count : Nat) :
    extremum before empty (List.replicate count none) = empty := by
  simp [extremum, ValueExtrema.extreme, ValueExtrema.encode]

/-- Projected values have exactly the active, eligible source-row witnesses with
a present first component. Complete key coverage supplies the reverse direction;
equal values need not identify keys. The proof first witnesses the projection,
then uses the existing signed whole-key selection correspondence. -/
theorem selected_extremum_values (M : Atoms A) (rows : List (SignedHeadElements.Row K A))
    (keys : List K) (value : K → Option V)
    (coverage : ∀ row ∈ rows, row.key ∈ keys) (result : V) :
    result ∈ (OrderedHeadActivity.values keys value
      (OrderedHeadActivity.selected M rows keys)).filterMap id ↔
      ∃ row ∈ rows, SignedHeadElements.Holds M row.head ∧
        Satisfies M row.eligible ∧ value row.key = some result := by
  have selected_values : ∀ optional,
      optional ∈ OrderedHeadActivity.values keys value
        (OrderedHeadActivity.selected M rows keys) ↔
        ∃ row ∈ rows, SignedHeadElements.Holds M row.head ∧
          Satisfies M row.eligible ∧ value row.key = optional := by
    intro optional
    exact OrderedHeadActivity.selected_values M rows keys value coverage optional
  constructor
  · intro present
    obtain ⟨optional, member, same⟩ := List.mem_filterMap.mp present
    have witness : ∃ row ∈ rows, SignedHeadElements.Holds M row.head ∧
        Satisfies M row.eligible ∧ value row.key = optional :=
      (selected_values optional).mp member
    simpa only [id_eq] using same ▸ witness
  · rintro ⟨row, member, head, eligible, same⟩
    have selected : some result ∈ OrderedHeadActivity.values keys value
        (OrderedHeadActivity.selected M rows keys) :=
      (selected_values (some result)).mpr ⟨row, member, head, eligible, same⟩
    exact List.mem_filterMap.mpr ⟨some result, selected, rfl⟩

/-- The canonical finite aggregate formula enumerates the existing complete
activity masks. Only measure projection omits absent first components; signed
row activity and the source's independent permissions are unchanged. -/
noncomputable def extremumFormula (before : V → V → Bool) (empty : V)
    (rows : List (SignedHeadElements.Row K A)) (keys : List K)
    (value : K → Option V) (accepts : V → Bool) : Formula A :=
  SignedHeadElements.formula rows keys (fun mask =>
    accepts (extremum before empty (OrderedHeadActivity.values keys value mask)))

/-- Original canonical truth is exactly the declared projected-value guard.
The complete mask enumeration law selects the actual original activity mask. -/
theorem extremum_formula_original (before : V → V → Bool) (empty : V)
    (M : Atoms A) (rows : List (SignedHeadElements.Row K A)) (keys : List K)
    (value : K → Option V) (accepts : V → Bool) :
    Satisfies M (extremumFormula before empty rows keys value accepts) ↔
      accepts (extremum before empty (OrderedHeadActivity.values keys value
        (OrderedHeadActivity.selected M rows keys))) = true := by
  rw [extremumFormula, SignedHeadElements.formula,
    AggregateReduct.original _ _ _ _ (AggregateReduct.masks_complete _)]
  rfl

/-- A head bound checks the projected measure only in the fixed candidate M.
Its reduct never supplies a new permission to J. Apply the existing bound law
then the original measure correspondence; no subset assumption on J is needed. -/
theorem extremum_bound_frozen (before : V → V → Bool) (empty : V)
    (M J : Atoms A) (rows : List (SignedHeadElements.Row K A)) (keys : List K)
    (value : K → Option V) (accepts : V → Bool) (body : Formula A) :
    Satisfies J (Reduct M (HeadMeasures.bound body
      (extremumFormula before empty rows keys value accepts))) ↔
      (Satisfies M body → accepts (extremum before empty
        (OrderedHeadActivity.values keys value
          (OrderedHeadActivity.selected M rows keys))) = true) := by
  rw [HeadMeasures.bound_frozen, extremum_formula_original]

/-- An implementation with exact original measure truth preserves the complete
head group in every surrounding theory when its permission formula is retained.
The proof lifts original agreement through the candidate-only bound, conjoins
unchanged permissions, then applies both equivalences to stable membership.
This law does not justify removing activities from arbitrary body aggregates. -/
theorem extremum_head_in_context (before : V → V → Bool) (empty : V)
    (M : Atoms A) (rows : List (SignedHeadElements.Row K A)) (keys : List K)
    (value : K → Option V) (accepts : V → Bool)
    (body permissions compiled : Formula A)
    (implementation : ∀ candidate, Satisfies candidate compiled ↔
      accepts (extremum before empty (OrderedHeadActivity.values keys value
        (OrderedHeadActivity.selected candidate rows keys))) = true)
    (context : Theory A) :
    Stable M (.conj permissions (HeadMeasures.bound body
      (extremumFormula before empty rows keys value accepts)) :: context) ↔
      Stable M (.conj permissions (HeadMeasures.bound body compiled) :: context) := by
  have original : ∀ candidate,
      Satisfies candidate (extremumFormula before empty rows keys value accepts) ↔
        Satisfies candidate compiled := by
    intro candidate
    rw [extremum_formula_original, implementation candidate]
  have group : Equivalent
      (.conj permissions (HeadMeasures.bound body
        (extremumFormula before empty rows keys value accepts)))
      (.conj permissions (HeadMeasures.bound body compiled)) :=
    equivalent_conj _ _ _ _ (equivalent_refl permissions)
      (HeadMeasures.bound_equivalent body _ _ original)
  simp only [Stable, models_cons, ReductTheory, List.map_cons, group.1, group.2]

end Extrema

end Zetesis.HeadContributions
