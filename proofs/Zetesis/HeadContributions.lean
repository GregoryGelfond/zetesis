import Zetesis.SignedHeadElements

/-!
# Neutral aggregate-head contributions

Abstract Gringo gives missing or nonnumeric sum tuples weight zero and generates
positive head choices independently of tuple weights. A positive-only sum also
ignores nonpositive numeric weights without removing their head permissions.
Unbounded heads consist of those choices without a measure constraint.

The finite numeric laws below concern the already coalesced active tuple family.
The frozen permission witness distinguishes a neutral contribution from deleting
its head. An absent bound can be removed in arbitrary original and frozen worlds
and therefore in every surrounding theory. Existing signed-activity laws retain
eligibility and its reduct; numeric contribution does not filter their rows.

Source syntax, complete binding and tuple carriers, checked arithmetic, provenance,
resource accounting and Rust refinement remain separate obligations. A missing
first value in a bounded extremum has no total measure specified here. Nothing in
these laws turns failed source evaluation into a neutral contribution.
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

end Zetesis.HeadContributions
