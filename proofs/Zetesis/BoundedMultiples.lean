import Zetesis.IntegerEnvelopes

/-!
# Bounded multiples of a positive divisor

For a positive integer divisor, the values in an inclusive integer interval
whose truncating remainder is zero are exactly the products obtained by taking
integer factors between the rounded-up lower endpoint and the rounded-down
upper endpoint. `bounded_multiple_iff` states that arithmetic identity;
`multiple_membership` composes it with finite interval enumeration.

The divisor may be one. Neither endpoint order nor positive endpoints are
required: empty intervals and intervals containing no multiple give no values.
The divisions in the factor bounds are Euclidean division, hence floor division
for the positive divisor. The tested remainder is explicitly truncating
remainder. Over mathematical integers its zero test is divisibility regardless
of the dividend's sign.

These laws do not establish finite-width arithmetic, source definedness or
error preservation, posting membership, occurrence clipping, row ordering,
snapshot identity or resource accounting. They are arithmetic premises for a
selector, not a refinement proof of an executable join.
-/

namespace Zetesis.BoundedMultiples

/-- A zero truncating remainder within the interval is equivalent to a product
whose integer factor lies between the ceiling and floor endpoint quotients.

A zero remainder supplies a divisor witness. Positive-divisor rounding converts
both value bounds to factor bounds. Conversely, the same rounding laws recover
the value bounds, and the product has zero truncating remainder. -/
theorem bounded_multiple_iff (lower upper value divisor : Int) (positive : 0 < divisor) :
    (lower ≤ value ∧ value ≤ upper ∧ value.tmod divisor = 0) ↔
      ∃ factor : Int, (lower + divisor - 1) / divisor ≤ factor ∧
        factor ≤ upper / divisor ∧ value = divisor * factor := by
  have lowerEndpoint (factor : Int) :
      (lower + divisor - 1) / divisor ≤ factor ↔ lower ≤ divisor * factor := by
    rw [Int.ediv_le_iff_le_mul positive, Int.mul_comm factor divisor]
    omega
  have upperEndpoint (factor : Int) :
      factor ≤ upper / divisor ↔ divisor * factor ≤ upper := by
    rw [Int.le_ediv_iff_mul_le positive, Int.mul_comm factor divisor]
  constructor
  · rintro ⟨above, below, remainder⟩
    obtain ⟨factor, equation⟩ := Int.dvd_of_tmod_eq_zero remainder
    exact ⟨factor, (lowerEndpoint factor).mpr (equation ▸ above),
      (upperEndpoint factor).mpr (equation ▸ below), equation⟩
  · rintro ⟨factor, above, below, equation⟩
    have lowerBound : lower ≤ value := equation ▸ (lowerEndpoint factor).mp above
    have upperBound : value ≤ upper := equation ▸ (upperEndpoint factor).mp below
    have divisible : divisor ∣ value := ⟨factor, equation⟩
    exact ⟨lowerBound, upperBound, Int.tmod_eq_zero_of_dvd divisible⟩

/-- Enumerating the inclusive factor interval and multiplying each factor
selects exactly the bounded values with zero truncating remainder.

Finite interval membership supplies precisely the two factor bounds used by
`bounded_multiple_iff`, including when that interval is empty. -/
theorem multiple_membership (lower upper value divisor : Int) (positive : 0 < divisor) :
    value ∈ (IntegerEnvelopes.integers ((lower + divisor - 1) / divisor)
      (upper / divisor)).map (fun factor => divisor * factor) ↔
      lower ≤ value ∧ value ≤ upper ∧ value.tmod divisor = 0 := by
  rw [bounded_multiple_iff lower upper value divisor positive]
  constructor
  · intro member
    obtain ⟨factor, inside, equation⟩ := List.mem_map.mp member
    have bounds : (lower + divisor - 1) / divisor ≤ factor ∧
        factor ≤ upper / divisor :=
      (IntegerEnvelopes.integer_membership _ _ factor).mp inside
    exact ⟨factor, bounds.1, bounds.2, equation.symm⟩
  · rintro ⟨factor, above, below, equation⟩
    exact List.mem_map.mpr ⟨factor,
      (IntegerEnvelopes.integer_membership _ _ factor).mpr ⟨above, below⟩, equation.symm⟩

end Zetesis.BoundedMultiples
