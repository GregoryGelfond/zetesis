# Language coverage obligations

The [admitted language](language.md) defines the current executable contract.
This checklist records known language boundaries and their current status. Its entries
have unequal size: a single entry can include several related scopes or
aggregate functions. The count is not a percentage of language parity.

There are **17 open classified ordinary-language obligations**, **6 separate
state/extension obligations**, and **2 input-boundary obligations**. The
unclassified scopes below are additional; these totals are not an exhaustive
inventory of every possible program or a definition of the v1.0 release scope.

An ordinary-language entry closes when its stated finite profile is admitted,
its semantic contract is explicit, and its interaction cases pass the relevant
independent semantic, source-comparison, execution and resource checks. The
associated Lean results and documented refinement limits must remain current.
The linked tests are representative evidence of current boundaries, not claims
that an open item is already qualified in its entirety.

## Ordinary language

L01 is closed within the admitted finite profiles. L02–L18 remain open. Stable
identifiers connect these boundaries to their tests and the admitted-language
reference.

| ID | Contract | Representative tests |
| --- | --- | --- |
| L01 | **Closed:** `not` and `not not` over atomic and Boolean operands in ordinary choices and all five admitted function heads, preserving signed contribution identity and positive producer support | [Signed elements](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/signed_element_contracts.rs) |
| L02 | Nontrivial conditional disjuncts with atomic, false and comparison conditions, including local scope and reduct behavior | [Boolean heads](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/boolean_heads.rs) |
| L03 | Negative anonymous consequent witnesses in universal body conditionals with a sound finite witness contract | [Consequent alternatives](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/consequent_alternatives.rs) |
| L04 | Finite flat pools in currently refused body and local contexts | [Finite pools](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/finite_pools.rs) |
| L05 | Nested pools and intervals with bounded construction and preserved scope | [Finite values](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/finite_values.rs) |
| L06 | Finite generating comparison chains involving several unresolved variables | [Comparison generators](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/comparison_generators.rs) |
| L07 | Symbolic choice bounds with defined ordering and activity semantics | [Choice consumers](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/choice_consumers.rs) |
| L08 | Aggregate-assignment results consumed in additional local scopes | [Conditional consumers](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/conditional_consumers.rs) |
| L09 | Objective-relevant assignment consumers and multiple assignments with complete dependency handling | [Multiple assignments](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/aggregate_assignments_multiple.rs) |
| L10 | An explicit contribution contract for missing head measures and nonnumeric sum-head weights, implemented across their admitted contexts | [Weighted heads](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/weighted_heads.rs) |
| L11 | Negative `#sum+` head weights under an explicit contribution contract | [Weighted heads](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/weighted_heads.rs) |
| L12 | Complete logical values in extremum heads, preserving ordering and empty-extremum semantics | [Extremum heads](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/extrema_heads.rs) |
| L13 | Dynamic objective priorities with complete priority identity and ordering | [Objective literals](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_literal_weights.rs) |
| L14 | Additional objective conditions with their declared scope and complete eligibility | [Weak objectives](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/weak_objectives.rs) |
| L15 | Negative, disjunctive and conditional objective-producer dependencies | [Objective extrema presence](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_extrema_presence.rs) |
| L16 | Filtered or multiple aggregate observers and broader mixed-extrema observer profiles | [Aggregate objective observers](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/aggregate_objective_observers.rs) |
| L17 | Broader observation expressions and conditions, preserving full answer identity | [Observation contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/observations_adversarial.rs) |
| L18 | `#project` with explicit projected enumeration and answer-identity contracts | [Metadata admission](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/metadata.rs) |

## State, extensions and input

These are separate contracts because they involve state ownership, host calls or
source identity in addition to finite one-shot semantics. All remain open.

| ID | Contract |
| --- | --- |
| E01 | Named program parts |
| E02 | Parameterized program parts |
| E03 | `#external` |
| E04 | Assumptions |
| E05 | Incremental solving |
| E06 | Pure Rust ground-time `@` functions over supplied logical interface types |
| I01 | Mixed standard-input and file inputs |
| I02 | Broader include alias and redirection behavior |

The current boundaries are exercised by [program-part
tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/program_parts.rs),
[metadata tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/metadata.rs),
[bundle admission](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/bundle_admission.rs)
and [multiple-input
tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cli/tests/multiple_inputs.rs).
These refusal and source-handling contracts do not establish an incremental API.

## Unclassified scopes and reviews

Broader expression contexts, aggregate eligibility/local generators beyond
L04–L06, and potentially admissible finite cyclic or self-dependent assignments
still need precise classification. Concrete cases can become subcases of an
existing entry or explicitly identified additions. Unsafe scopes and
ungroundable cycles require a sound finite binding contract before admission.

ASPIF import/export and any libclingo interoperability require their own
interchange contracts; they are not included in the ordinary-language count.
The [numeric endpoint boundary](language.md#numeric-boundaries-and-refusal-meaning)
and themelios's minimum-integer literal representation limit remain separate
correspondence/dependency reviews. Reached undefined arithmetic, resource
exhaustion and deliberate theory/scripting/`#heuristic`/`#edge` exclusions are
not ordinary implementation-gap entries.
