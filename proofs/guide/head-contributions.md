# Aggregate-head contributions

Head permission is independent of a tuple's numeric contribution. This follows
from the corrected [Abstract Gringo](https://arxiv.org/pdf/1507.06576v2), §2.1 and
§3, equations (14)–(18): missing or nonnumeric sum tuples have weight zero,
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
| Bounded `#min`, `#max` | Missing first value | Named profile refusal |
| Any unbounded head | Admitted tuple terms | Retain choices; no aggregate result is needed |

The paper's finite nonempty extrema definition takes the first components of
tuples but does not establish a result when those components are missing.
zetesis therefore retains `ProfileFeature::HeadAggregateMissingValue` for
bounded extrema containing empty tuples. This remains an open source-language
boundary. It differs from a complete first value such as `#sup` or `#inf`.
The older, broader `ProfileFeature::HeadAggregateWeight` refusal is removed.

Source syntax, safety and reached binding instructions remain checked. Neutral
weights do not bypass complete-key accounting, copied-value limits, substitution
limits or arithmetic errors. An unbounded head avoids evaluating an aggregate
result; it still validates and accounts for its source terms and bindings.

## Mathematical and executable evidence

[HeadContributions](../Zetesis/HeadContributions.lean) proves that absent numeric
contributions equal zero weights, and that positive-only selection computes the
positive part of the finite sum. The input family already coalesces complete
active tuple keys; repeated numeric weights are allowed.

`neutral_permission_is_not_truth` gives a frozen-reduct witness showing why zero
contribution does not justify deleting permission. `unbounded_equivalent` and
`unbounded_in_context` preserve an arbitrary permission formula when its absent
bound is dropped, in original and frozen interpretations and surrounding theory.
The existing signed-head laws still govern activity and eligibility. Source
recognition, complete binding coverage, checked arithmetic, exact provenance and
Rust refinement remain separate obligations.

The [source matrix](../../crates/zetesis-themelios/tests/support/head_contributions.rs)
pairs every admitted example with explicit choices and constraints. Tests compare
complete native answer-set families, subset enumeration, original interpretations
and arbitrary frozen interpretation pairs. Further tests cover named refusals,
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
| `#min{:a}=#sup.` | Named refusal | `{}` |
| `#min{:a}.` | `{}`, `{a}` | `{}`, `{a}` |

The mixed `sum+` reference also becomes inconsistent after adding facts `a. b.`.
Adding facts already satisfied by an answer set preserves it under a fixed
Ferraris theory. These contextual reference results consequently cannot all
implement one fixed translation. That observation identifies a correspondence
problem without establishing its upstream cause.

The external comparison test checks complete records and exact versioned
families for every known difference, failing if any changes. All other admitted
matrix entries must agree with clingo. Native semantic checks apply to every
entry; bounded missing-extremum refusals have their own typed contract tests.
