# Language coverage obligations

The [admitted language](language.md) defines the current executable contract.
This checklist records known language boundaries and their current status. Its entries
have unequal size: a single entry can include several related scopes or
aggregate functions. The count is not a percentage of language parity.

There are **7 open classified ordinary-language obligations**, **6 separate
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

L01, L03, L06, L07, L09–L13, L15 and L16 are closed within their documented finite
profiles. L02, L04, L05, L08, L14, L17 and L18 remain open. Stable identifiers
connect these boundaries to their tests and the admitted-language reference.

| ID | Contract | Representative tests |
| --- | --- | --- |
| L01 | **Closed:** `not` and `not not` over atomic and Boolean operands in ordinary choices and all five admitted function heads, preserving signed contribution identity and positive producer support | [Signed elements](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/signed_element_contracts.rs) |
| L02 | Nontrivial conditional disjuncts with atomic, false and comparison conditions, including local scope and reduct behavior | [Boolean heads](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/boolean_heads.rs) |
| L03 | **Closed:** `not` and `not not` after complete finite anonymous witness projection in universal body conditionals, preserving source alternatives and condition scope | [Consequent alternatives](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/consequent_alternatives.rs) |
| L04 | Finite flat pools in currently refused body and local contexts | [Finite pools](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/finite_pools.rs) |
| L05 | Nested pools and intervals with bounded construction and preserved scope | [Finite values](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/finite_values.rs) |
| L06 | **Closed:** directed finite integer-affine comparison chains involving several unresolved variables, preserving whole-guard correlation, local scope and checked source arithmetic; nonlinear inversion and systems without directed finite bounds remain outside this profile | [Finite chains](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/finite_chains.rs) |
| L07 | **Closed:** complete logical bounds on ordinary choices and admitted numeric count/sum comparisons, preserving logical ordering, signed activity and frozen-reduct truth within existing finite binding and measure profiles | [Logical bounds](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/logical_bounds.rs) |
| L08 | Aggregate-assignment results consumed in additional local scopes | [Conditional consumers](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/conditional_consumers.rs) |
| L09 | **Closed:** dependency-ordered finite assignment consumers and multiple assignments preserve complete predecessor bindings, original equalities and objective costs; recursive producer dependencies use completed possible support | [Multiple assignments](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/aggregate_assignments_multiple.rs) |
| L10 | **Closed:** missing/nonnumeric sum-head measures are neutral; missing extremum-head measures use the declared present-first-value projection, retaining independent head permission in guarded and unbounded contexts | [Head contributions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/head_contributions.rs) |
| L11 | **Closed:** negative numeric `#sum+` head measures contribute zero while retaining independent positive permission in guarded and unbounded admitted contexts; fixed-formula semantics and explicit clingo differences are documented | [Head contributions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/head_contributions.rs) |
| L12 | **Closed:** complete first tuple values in extremum heads preserve logical ordering and empty-extremum semantics within the documented numeric endpoint, producer and eligibility boundaries; neutral missing values follow L10 | [Extremum heads](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/extrema_heads.rs) |
| L13 | **Closed:** finite safely bound scalar priorities preserve same-binding weight/tuple identity and objective ordering, including recursive aggregate and conditional producers after support completion | [Source measure carriers](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_measure_carriers.rs), [rich cycles](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_rich_cycles.rs) |
| L14 | **Open:** scoped weak aggregates, conditionals and finite scalar/structural bindings are admitted, including pooled conditional consequents; pools in ordinary body atoms, local conditions, aggregate elements, scalar binders and objective fields retain exact refusals | [Scoped objectives](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_scopes.rs), [pool boundaries](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_pools.rs) |
| L15 | **Closed:** signed and disjunctive dependencies, aggregate heads and recursive aggregate/conditional producer cones use completed finite support and original-model objective queries within the admitted source profiles | [Rich producers](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_rich_producers.rs), [rich cycles](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_rich_cycles.rs) |
| L16 | **Closed:** filtered and multiple observers, mixed extrema and recursive observer dependencies preserve complete finite source rows, independently applicable carrier refinements and original-model costs | [Aggregate objective observers](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/aggregate_objective_observers.rs), [carrier composition](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_carrier_composition.rs) |
| L17 | **Open:** scoped arithmetic, aggregates, conditionals, structural pools, positive equality chains and finite constructor/tuple capture preserve full answer identity; arithmetic inversion, pooled equality-capture patterns and the other exact observation boundaries remain restricted | [Observation bindings](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/observation_bindings.rs), [expressions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/observation_expressions.rs) |
| L18 | `#project` with explicit projected enumeration and answer-identity contracts | [Metadata admission](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/metadata.rs) |

The [objective boundary witnesses](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_language_boundaries.rs)
retain all seven original sources, their declared complete native results and
exact versioned reference captures for L10 and L13–16. Four cyclic objective
cases also retain the complete native families of their objective-free programs.
The [neutral-head rationale](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/head-contributions.md)
identifies the declared extension and answer-family differences. Objective
zero-slot reporting differences remain separate from answer identity and ranking.

Closure uses the existing finite source profiles: complete typed bindings,
dependency-ordered local assignments and successful bounded support completion.
It does not admit every syntactically valid local generator or establish
termination of arbitrary recursive value generation. L14's
[pool witnesses](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_pools.rs)
and L17's [exact observation witnesses](language.md#objectives-and-observations)
remain ordinary implementation boundaries, including valid finite forms. Their
continued restrictions are not counted as resource failures or undefined input.

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
the finite L04–L06 profiles, and self-dependent or mutually dependent bindings
without an established local evaluation order still need precise classification. Concrete cases can become subcases of an
existing entry or explicitly identified additions. Unsafe scopes and
ungroundable cycles require a sound finite binding contract before admission.
The [whole-value/open-tuple comparison refusal](language.md#rules-terms-and-bindings)
is one concrete unclassified logical-value boundary. It is separate from L05's
nested pools and intervals and does not change the classified counts above.

ASPIF import/export and any libclingo interoperability require their own
interchange contracts; they are not included in the ordinary-language count.
The [numeric endpoint boundary](language.md#numeric-boundaries-and-refusal-meaning)
and themelios's minimum-integer literal representation limit remain separate
correspondence/dependency reviews. Reached undefined arithmetic, resource
exhaustion and deliberate theory/scripting/`#heuristic`/`#edge` exclusions are
not ordinary implementation-gap entries.
