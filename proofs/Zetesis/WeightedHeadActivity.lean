import Zetesis.CountHeadActivity
import Zetesis.AggregateReduct

/-!
# Weighted head activity over distinct complete tuples

Rows independently associate tuples, head atoms and eligibility formulas.
The complete, duplicate-free tuple carrier from CountHeadActivity is reused for
signed sums. A tuple contributes its integer weight once when any of its own
eligible selected rows is true. Several tuples may share an atom.

The reference numeric formula uses AggregateReduct's complete mask expansion;
it is a mathematical specification, not the Rust lowering algorithm. Its original
truth agrees with direct tuple selection, and its activated head bound depends
only on the candidate. Row coalescing preserves the complete group in context.
Integer weights are unbounded here. Finite-width arithmetic, source binding,
optimized weighted compilation and machine execution remain unproved refinements.
-/

namespace Zetesis.WeightedHeadActivity
open Ferraris
open CountEligibility (Row tupleActivity eligibility)
open CountHeadActivity (Carrier)
universe u v
variable {A : Type u} {K : Type v}

/-- The weight of each selected tuple position is included exactly once. The
    carrier, not this reducer, establishes distinct complete tuple identity. -/
def sum (keys : List K) (weight : K → Int)
    (selected : AggregateReduct.Mask keys.length) : Int :=
  ((List.finRange keys.length).map fun index =>
    if selected index then weight keys[index] else 0).sum

open Classical in
/-- Select a tuple from its own row witnesses. Possible support alone does not
    establish either head membership or eligibility in the candidate. -/
noncomputable def selected (M : Atoms A) (rows : List (Row K A))
    (keys : List K) : AggregateReduct.Mask keys.length :=
  fun index => decide (∃ row ∈ rows, row.identity.key = keys[index] ∧
    M row.identity.atom ∧ Satisfies M row.eligible)

open Classical in
/-- Direct signed measure over the carrier's selected complete tuples. Numeric
    zero changes no permission; nonnegative weights give the admitted sum+ case. -/
noncomputable def measure (M : Atoms A) (rows : List (Row K A))
    (keys : List K) (weight : K → Int) : Int :=
  sum keys weight (selected M rows keys)

/-- Canonical formula for any Boolean guard on the signed tuple sum. Bounds,
    equality and disequality can share this semantic construction. -/
noncomputable def formula (rows : List (Row K A)) (keys : List K)
    (weight : K → Int) (accepts : Int → Bool) : Formula A :=
  AggregateReduct.aggregate (fun index => tupleActivity rows keys[index])
    (fun mask => accepts (sum keys weight mask))
    (AggregateReduct.masks keys.length)

/-- Formula activity selects precisely the same row witnesses as direct
    evaluation, with no atom/tuple bijection requirement. -/
theorem active_is_selected (M : Atoms A) (rows : List (Row K A)) (keys : List K) :
    AggregateReduct.active M (fun index : Fin keys.length =>
      tupleActivity rows keys[index]) = selected M rows keys := by
  classical
  funext index
  simp only [AggregateReduct.active, selected,
    CountHeadActivity.activity_original]

/-- The canonical guard agrees with the direct signed measure on a complete
    unique carrier, including shared atoms, shared tuples and zero weights. -/
theorem formula_original (M : Atoms A) (rows : List (Row K A))
    (carrier : Carrier rows) (weight : K → Int) (accepts : Int → Bool) :
    Satisfies M (formula rows carrier.keys weight accepts) ↔
      accepts (measure M rows carrier.keys weight) = true := by
  rw [formula, AggregateReduct.original _ _ _ _ (AggregateReduct.masks_complete _)]
  rw [active_is_selected]
  rfl

/-- Repeated or reordered identical rows leave selection unchanged. Distinct
    complete keys remain distinct even if their numeric weights coincide. -/
theorem same_rows_preserve_measure (M : Atoms A) (first second : List (Row K A))
    (same : ∀ row, row ∈ first ↔ row ∈ second) (keys : List K) (weight : K → Int) :
    measure M first keys weight = measure M second keys weight := by
  have selected_equal : selected M first keys = selected M second keys := by
    classical
    funext index
    simp only [selected, same]
  unfold measure
  rw [selected_equal]

/-- The activated bound inspects M and contributes no support to J, even when
    negative contributions make the measure nonmonotone. -/
theorem bound_frozen (M J : Atoms A) (rows : List (Row K A))
    (carrier : Carrier rows) (weight : K → Int) (accepts : Int → Bool)
    (body : Formula A) :
    Satisfies J (Reduct M (HeadMeasures.bound body
      (formula rows carrier.keys weight accepts))) ↔
      (Satisfies M body → accepts (measure M rows carrier.keys weight) = true) := by
  rw [HeadMeasures.bound_frozen, formula_original]

/-- Preserving complete rows preserves both eligibility permissions and the
    weighted candidate constraint in every surrounding theory. Numeric weights
    never replace the eligibility reduct or grant a selected atom support. -/
theorem stable_in_context (M : Atoms A) (first second : List (Row K A))
    (same : ∀ row, row ∈ first ↔ row ∈ second)
    (first_carrier : Carrier first) (second_carrier : Carrier second)
    (same_keys : first_carrier.keys = second_carrier.keys)
    (weight : K → Int) (accepts : Int → Bool) (body : Formula A)
    (heads : List A) (context : Theory A) :
    Stable M (HeadMeasures.group body heads (eligibility first)
      (formula first first_carrier.keys weight accepts) :: context) ↔
    Stable M (HeadMeasures.group body heads (eligibility second)
      (formula second second_carrier.keys weight accepts) :: context) := by
  have numeric (candidate : Atoms A) :
      Satisfies candidate (formula first first_carrier.keys weight accepts) ↔
        Satisfies candidate (formula second second_carrier.keys weight accepts) := by
    rw [formula_original, formula_original, same_keys,
      same_rows_preserve_measure candidate first second same]
  exact HeadMeasures.stable_in_context M body heads _ _ _ _
    (CountHeadActivity.eligibility_of_same_rows first second same) numeric context

end Zetesis.WeightedHeadActivity
