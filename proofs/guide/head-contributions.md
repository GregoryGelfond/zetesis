# Aggregate-head contributions

Head permission is independent of a tuple's measured contribution. The sum
and permission rules follow [Abstract Gringo](https://arxiv.org/pdf/1507.06576v2),
§2.1 and §3, equations (14)–(18): missing or nonnumeric sum tuples have weight zero,
`sum+` adds only positive weights, and positive head choices are generated
independently. Removing both bounds removes the aggregate constraint, leaving
those choices. Empty tuples are explicitly permitted by footnote 5.

| Head | Tuple value | zetesis contract |
| --- | --- | --- |
| `#count` | Any complete tuple, including empty | Count once when active; retain permission |
| `#sum` | Numeric first value | Add its signed value; retain permission |
| `#sum`, `#sum+` | Missing or nonnumeric first value | Zero contribution; retain permission |
| `#sum+` | Nonpositive numeric first value | Zero contribution; retain permission |
| `#sum+` | Positive numeric first value | Add its value; retain permission |
| Bounded `#min`, `#max` | Complete first value | Use ASP term order; retain permission |
| Bounded `#min`, `#max` | Missing first value | No measure contribution; retain permission |
| Any unbounded head | Admitted tuple terms | Retain choices; no aggregate result is needed |

## Missing extrema: the declared extension

For a finite set `T` of selected complete tuple keys, define

```text
V(T) = { v | there is a tail with (v :: tail) in T }
min*(T) = #sup if V(T) is empty, otherwise its least value
max*(T) = #inf if V(T) is empty, otherwise its greatest value
```

The order is the existing ASP term order. Missing means no first component;
symbols, strings, constructors, `#inf` and `#sup` are complete values. A tuple
whose first value is the structured tuple `()` is also nonempty. Adding an
empty tuple changes neither measure, while its independently eligible head
permission remains. No numeric zero stands in for an absent extremum value.

This is a declared extension. Abstract Gringo §2.1 gives ordinary finite extrema
and assumes total aggregate functions, but its nonempty-set clause does not
give a least/greatest first component when every tuple is empty. The totality
assumption alone does not choose that result. The independent permission rule
in §3 motivates retaining the choices; it does not uniquely determine this
measure convention.

The extension conserves the result on the previously defined finite domain:
the empty aggregate and families whose tuples all have first values. An empty
aggregate and one containing an empty tuple can have the same measure and
different permissions. This admission extension applies to heads; missing
extremum values in body aggregates, assignments and observations remain outside
their existing profiles.

For example, the [checked source](../../crates/zetesis-themelios/tests/fixtures/head-neutral-extrema.lp)

```asp
#min{:a;0:b}=0.
```

has measure zero exactly when `b` is selected. Its head translation is equivalent
in original and frozen interpretations to

```text
F = (a ∨ ¬a) ∧ (b ∨ ¬b) ∧ ¬¬b
```

The original models are `{b}` and `{a,b}`. At these candidates the Ferraris
reducts reduce to `b` and `a ∧ b`, respectively, so both are subset-minimal
models of their reducts. The double negation matters: the bound checks the
candidate; the choices supply permission. Deleting the neutral `a` occurrence
would delete its choice, leaving reduct `b` at `{a,b}` and the proper-subset
countermodel `{b}`. clingo 5.8.2 returns only `{b}` for this exact source; this
is a recorded difference, not a parity result or an established upstream bug.

Source syntax, safety and reached binding instructions remain checked. Neutral
weights do not bypass complete-key accounting, copied-value limits, substitution
limits or arithmetic errors. An unbounded head avoids evaluating an aggregate
result; it still validates and accounts for its source terms and bindings.

## Mathematical and executable evidence

[HeadContributions](../Zetesis/HeadContributions.lean) proves that absent numeric
contributions equal zero weights, and that positive-only selection computes the
positive part of the finite sum. The input family already coalesces complete
active tuple keys; repeated numeric weights are allowed.

`missing_extremum_is_neutral`, `complete_extremum_conservative` and
`only_missing_extremum` establish the projection rule and its conservation
domain using the existing `ValueExtrema` reduction. `selected_extremum_values`
requires complete key coverage and retains the exact eligible signed-row
witness for every present first value. `extremum_formula_original` relates the
canonical activity-mask formula to the projected measure.

`extremum_bound_frozen` uses the existing candidate-only head-bound law.
`extremum_head_in_context` assumes an implementation's exact original measure
truth, retains the same permission formula, and preserves stable membership
under any surrounding theory. It does not remove conditions from arbitrary
body aggregates or prove that the Rust compiler meets those hypotheses.

`neutral_permission_is_not_truth` gives a frozen-reduct witness showing why zero
contribution does not justify deleting permission. `unbounded_equivalent` and
`unbounded_in_context` preserve an arbitrary permission formula when its absent
bound is dropped, in original and frozen interpretations and surrounding theory.
The existing signed-head laws still govern activity and eligibility. Source
recognition, complete binding coverage, checked arithmetic, exact provenance and
Rust refinement remain separate obligations.

The [source matrix](../../crates/zetesis-themelios/tests/integration/head_contributions/cases.rs)
pairs every admitted example with explicit choices and constraints. Tests compare
complete native answer-set families, subset enumeration, original interpretations
and arbitrary frozen interpretation pairs. Further tests cover remaining refusals,
source identity, undefined arithmetic and inclusive resource limits.

## Comparison with clingo 5.8.2

These observed differences are recorded explicitly; they are not parity passes
or an established claim of an upstream defect.

| Source | zetesis formal translation | clingo 5.8.2 |
| --- | --- | --- |
| `#sum{:a}<=0.` | `{}`, `{a}` | `{}` |
| `#sum{word:a}<=0.` | `{}`, `{a}` | `{}` |
| `#sum{:a}.` | `{}`, `{a}` | `{}`, `{a}` |
| `#sum{word:a}.` | `{}`, `{a}` | `{}` |
| `2#sum+{-2:a;2:b}2.` | `{b}`, `{a,b}` | `{b}` |
| `0#sum+{-2:a;2:b}0.` | `{}`, `{a}` | `{}`, `{a}`, `{a,b}` |
| `#min{:a}=#sup.` | `{}`, `{a}` | `{}` |
| `#max{:a}=#inf.` | `{}`, `{a}` | `{}` |
| `#min{:a;0:b}=0.` | `{b}`, `{a,b}` | `{b}` |
| `#max{:a;0:b}=0.` | `{b}`, `{a,b}` | `{b}` |
| `#min{:a}=0.` / `#max{:a}=0.` | No answers | No answers |
| `#min{:a}.` | `{}`, `{a}` | `{}`, `{a}` |

The mixed `sum+` reference also becomes inconsistent after adding facts `a. b.`.
Adding facts already satisfied by an answer set preserves it under a fixed
Ferraris theory. These contextual reference results consequently cannot all
implement one fixed translation. That observation identifies a correspondence
problem without establishing its upstream cause.

The external comparison test checks complete records and exact versioned
families for every known difference, failing if any changes. All other admitted
matrix entries must agree with clingo. Native semantic checks apply to every
entry, including missing-extremum heads. Agreement on the two `=0` examples
alone does not determine the missing-value convention.
