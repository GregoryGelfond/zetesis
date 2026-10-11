# Bounded values with zero remainder

[BoundedMultiples](../Zetesis/BoundedMultiples.lean) gives the arithmetic identity
behind selecting multiples from an inclusive integer interval. For `d > 0`,

```text
lower ≤ value ≤ upper and value.tmod d = 0
  iff
value = d*k for an integer k with
  (lower + d - 1) / d ≤ k ≤ upper / d.
```

The endpoint divisions are floor division, so the first quotient is the ceiling
of `lower / d`. The tested remainder is truncating remainder. A zero truncating
remainder is divisibility even for a negative dividend; the theorem therefore
needs no sign restriction on the interval endpoints. The implementation's
positive numeric domain is a separate restriction.

The proof of `bounded_multiple_iff` has three steps. Zero remainder provides a
factor with `value = d*k`. Positive-divisor rounding translates each value bound
into the corresponding factor bound. Conversely, those bounds recover the value
interval, and the product has zero truncating remainder.

`multiple_membership` applies the existing finite enumeration law
`IntegerEnvelopes.integer_membership` to the factor interval and maps each factor
to its product. Divisor one gives every interval value. Reversed endpoints give
none, as does a nonempty interval with no multiple, such as `5..5` for divisor 2.
Both interval endpoints remain inclusive.

These are laws of mathematical integers. They do not verify Rust's finite-width
addition, division or product, the source totality certificate, its diagnostics,
relation postings, occurrence interval intersection, row order, snapshot lifetime
or charged work and storage. Composition with
`DomainBindings.guarded_continuations_exact` still requires the executable
selector to establish that every row with a contributing continuation survives.
The laws neither certify generic computed-table selection in a support round nor
replace source-family admission or the reduct.
