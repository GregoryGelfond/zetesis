import Zetesis.ThresholdPropagation
import Zetesis.OrdinalExtrema
import Zetesis.PossibleFoundedness

/-!
# False aggregate bodies after removing an unsupported set

A body witness states that a formula's reduct is false outside a specified set
of atoms, for every original interpretation satisfying the witness condition.
The condition may be a conjunction of signed atom requirements; the mathematical
statement does not depend on that representation. Removing more atoms preserves
this property because it restricts the tested interpretations.

A nonnegative threshold is false if an upper reading, which keeps every
contribution without a false-reduct witness, remains below its bound. An
interior extremum cut is false if every qualifying contribution has such a witness.
Conditions may be arbitrary formulas. Their recursively frozen truth, rather
than their original truth or direct evaluation after removal, is decisive.

These laws compose with PossibleFoundedness's complete producer-coverage law.
They do not certify an executable witness collector, rank construction, signed
literal storage, finite-width arithmetic, allocation, cancellation or reuse
between different original theories. An objective or candidate restriction
cannot supply an original-theory witness merely because it rejects a region.
-/

namespace Zetesis.AggregateBodyWitnesses

open Ferraris

universe u v
variable {α : Type u} {β : Type v} {n : Nat}

/-- Every matching original interpretation has a false body reduct when tested
    outside the removed atoms. The tested interpretation need not be a subset
    of the original; this stronger statement matches the body-relaxation law. -/
def RefutesOutside (condition : Atoms α → Prop) (removed : Atoms α)
    (body : Formula α) : Prop :=
  ∀ M, condition M → ∀ J, Sub J (fun atom => ¬ removed atom) →
    ¬ Satisfies J (Reduct M body)

/-- Enlarging the removed set preserves every previously established witness.
    The proof transfers the smaller tested interpretation to the earlier
    complement. New removed atoms still need their own producer coverage. -/
theorem refutes_outside_mono (condition : Atoms α → Prop)
    (before after : Atoms α) (body : Formula α) (grows : Sub before after)
    (witness : RefutesOutside condition before body) :
    RefutesOutside condition after body := by
  intro M matching J outside
  have previouslyOutside : Sub J (fun atom => ¬ before atom) := by
    intro atom present previouslyRemoved
    exact outside atom present (grows atom previouslyRemoved)
  exact witness M matching J previouslyOutside

/-- A formula not refuted by these witnesses is a sound upper body relaxation.
    If a reduct were true outside the removed set, a witness refuting it would
    contradict that truth. Complete producer coverage remains a separate premise
    of `PossibleFoundedness.conditional_unsupported_absent`. -/
theorem refutation_upper_sound (condition : Atoms α → Prop) (removed : Atoms α)
    (M : Atoms α) (matching : condition M) :
    PossibleFoundedness.UpperSound M (fun atom => ¬ removed atom)
      (fun body => ¬ RefutesOutside condition removed body) := by
  intro body J outside reduced witness
  exact witness M matching J outside reduced

/-- An upper reading below the threshold excludes its reduct. Every omitted
    condition must have a false-reduct witness; all remaining conditions may be
    counted as possible, whether or not they can hold together.

    Frozen threshold satisfaction requires a sufficient sum of recursively
    frozen conditions. Their truth mask is contained in the supplied upper
    reading, so nonnegative sum monotonicity contradicts the insufficient bound.
    Each input position retains its weight, including repeated conditions. -/
theorem threshold_reduct_absent (M J : Atoms α) (conditions : β → Formula α)
    (inputs : List (Nat × β)) (bound : Nat) (upper : β → Bool)
    (omitted : ∀ index, upper index = false →
      ¬ Satisfies J (Reduct M (conditions index)))
    (insufficient : ThresholdReduct.maskedSum upper inputs < bound) :
    ¬ Satisfies J (Reduct M (ThresholdReduct.formula conditions inputs bound)) := by
  classical
  intro reduced
  have included : ∀ index,
      ThresholdReduct.active J (fun key => Reduct M (conditions key)) index = true →
        upper index = true := by
    intro index enabled
    have conditionTrue : Satisfies J (Reduct M (conditions index)) := by
      simpa only [ThresholdReduct.active, decide_eq_true_eq] using enabled
    cases kept : upper index with
    | false => exact False.elim (omitted index kept conditionTrue)
    | true => rfl
  have bounded :
      ThresholdReduct.maskedSum
          (ThresholdReduct.active J (fun key => Reduct M (conditions key))) inputs ≤
        ThresholdReduct.maskedSum upper inputs :=
    ThresholdPropagation.masked_sum_mono _ upper inputs included
  have required : bound ≤ ThresholdReduct.maskedSum
      (ThresholdReduct.active J (fun key => Reduct M (conditions key))) inputs :=
    ((ThresholdReduct.frozen M J conditions inputs bound).mp reduced).2
  have contradiction : bound < bound :=
    Nat.lt_of_le_of_lt (Nat.le_trans required bounded) insufficient
  exact (Nat.lt_irrefl bound) contradiction

/-- False contribution witnesses combine into a threshold body witness under
    the same condition and removed set. In particular, a contribution may be
    false because its positive atom is removed; original falsity is not required. -/
theorem threshold_refutes_outside (condition : Atoms α → Prop) (removed : Atoms α)
    (conditions : β → Formula α) (inputs : List (Nat × β)) (bound : Nat)
    (upper : β → Bool)
    (omitted : ∀ index, upper index = false →
      RefutesOutside condition removed (conditions index))
    (insufficient : ThresholdReduct.maskedSum upper inputs < bound) :
    RefutesOutside condition removed (ThresholdReduct.formula conditions inputs bound) := by
  intro M matching J outside
  have contributions : ∀ index, upper index = false →
      ¬ Satisfies J (Reduct M (conditions index)) := by
    intro index excluded
    exact omitted index excluded M matching J outside
  exact threshold_reduct_absent M J conditions inputs bound upper contributions insufficient

/-- A guard requiring an active qualifying contribution cannot hold in the
    reduct when every such contribution has a false-reduct witness. The guard
    premise is explicit, so this law does not silently treat an empty extremum
    sentinel as a selected contribution or apply an interior-cut rule at an endpoint. -/
theorem aggregate_reduct_absent (M J : Atoms α) (conditions : Fin n → Formula α)
    (guard : AggregateReduct.Mask n → Bool) (qualifies : Fin n → Prop)
    (requiresWitness : ∀ mask, guard mask = true →
      ∃ index, qualifies index ∧ mask index = true)
    (omitted : ∀ index, qualifies index →
      ¬ Satisfies J (Reduct M (conditions index))) :
    ¬ Satisfies J (Reduct M
      (AggregateReduct.aggregate conditions guard (AggregateReduct.masks n))) := by
  classical
  intro reduced
  have accepted : guard
      (AggregateReduct.active J (fun index => Reduct M (conditions index))) = true :=
    ((AggregateReduct.direct_reduct M J conditions guard).mp reduced).2
  obtain ⟨index, qualifying, enabled⟩ := requiresWitness _ accepted
  have conditionTrue : Satisfies J (Reduct M (conditions index)) := by
    simpa only [AggregateReduct.active, decide_eq_true_eq] using enabled
  exact omitted index qualifying conditionTrue

/-- Qualifying contribution witnesses combine under one original condition and
    removed set. The complete-mask formula denotes the aggregate semantics;
    no runtime mask enumeration is proposed or required by this statement. -/
theorem aggregate_refutes_outside (condition : Atoms α → Prop) (removed : Atoms α)
    (conditions : Fin n → Formula α) (guard : AggregateReduct.Mask n → Bool)
    (qualifies : Fin n → Prop)
    (requiresWitness : ∀ mask, guard mask = true →
      ∃ index, qualifies index ∧ mask index = true)
    (omitted : ∀ index, qualifies index →
      RefutesOutside condition removed (conditions index)) :
    RefutesOutside condition removed
      (AggregateReduct.aggregate conditions guard (AggregateReduct.masks n)) := by
  intro M matching J outside
  have contributions : ∀ index, qualifies index →
      ¬ Satisfies J (Reduct M (conditions index)) := by
    intro index qualifying
    exact omitted index qualifying M matching J outside
  exact aggregate_reduct_absent M J conditions guard qualifies requiresWitness contributions

/-- An interior prefix cut supplies the qualifying-position premise of the
    preceding aggregate law. Bounds exclude the greatest empty-selection rank;
    selected rank membership preserves its original contribution position. -/
theorem prefix_requires_witness (greatest cut : Nat) (ranks : Fin n → Nat)
    (bounded : ∀ index, ranks index ≤ greatest)
    (interior : 0 < cut ∧ cut ≤ greatest) (mask : AggregateReduct.Mask n)
    (accepted : (OrdinalExtrema.measure greatest
      (RankedExtrema.selectedValues ranks mask)).1 < cut) :
    ∃ index, ranks index < cut ∧ mask index = true := by
  have selectedBounded :
      ∀ value ∈ RankedExtrema.selectedValues ranks mask, value ≤ greatest := by
    intro value member
    obtain ⟨index, _, same⟩ := List.mem_map.mp member
    exact same ▸ bounded index
  obtain ⟨value, member, qualifying⟩ :=
    (OrdinalExtrema.prefix_witness greatest cut _ selectedBounded interior).mp accepted
  obtain ⟨index, selected, same⟩ := List.mem_map.mp member
  exact ⟨index, by simpa only [same] using qualifying, (List.mem_filter.mp selected).2⟩

/-- An interior suffix cut likewise requires a qualifying selected position.
    A positive cut excludes the empty maximum zero. Endpoint cuts instead use
    `OrdinalExtrema.suffix_zero` and `OrdinalExtrema.suffix_above`. -/
theorem suffix_requires_witness (greatest cut : Nat) (ranks : Fin n → Nat)
    (bounded : ∀ index, ranks index ≤ greatest)
    (interior : 0 < cut ∧ cut ≤ greatest) (mask : AggregateReduct.Mask n)
    (accepted : cut ≤ (OrdinalExtrema.measure greatest
      (RankedExtrema.selectedValues ranks mask)).2) :
    ∃ index, cut ≤ ranks index ∧ mask index = true := by
  have selectedBounded :
      ∀ value ∈ RankedExtrema.selectedValues ranks mask, value ≤ greatest := by
    intro value member
    obtain ⟨index, _, same⟩ := List.mem_map.mp member
    exact same ▸ bounded index
  obtain ⟨value, member, qualifying⟩ :=
    (OrdinalExtrema.suffix_witness greatest cut _ selectedBounded interior).mp accepted
  obtain ⟨index, selected, same⟩ := List.mem_map.mp member
  exact ⟨index, by simpa only [same] using qualifying, (List.mem_filter.mp selected).2⟩

end Zetesis.AggregateBodyWitnesses
