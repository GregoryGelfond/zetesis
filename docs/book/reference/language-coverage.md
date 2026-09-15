# Language coverage obligations

The [admitted language](language.md) defines the current executable contract.
This checklist records known language boundaries and their current status. Its entries
have unequal size: a single entry can include several related scopes or
aggregate functions. The count is not a percentage of language parity.

There are **0 open classified ordinary-language obligations within the finite
profiles below**, **6 separate state/extension obligations**, and **2 input-boundary
obligations**. The
unclassified scopes below are additional; these totals are not an exhaustive
inventory of every possible program or a definition of the v1.0 release scope.

An ordinary-language entry closes when its stated finite profile is admitted,
its semantic contract is explicit, and its interaction cases pass the relevant
independent semantic, source-comparison, execution and resource checks. The
associated Lean results and documented refinement limits must remain current.
The linked tests are representative evidence for these contracts, not a claim
that every syntactically valid program belongs to an admitted finite profile.

## Ordinary language

L01–L18 are closed within their documented finite profiles. Stable identifiers
connect these contracts to their tests and the admitted-language reference.
The separate state/input obligations and unclassified scopes below remain open.

| ID | Contract | Representative tests |
| --- | --- | --- |
| L01 | **Closed:** `not` and `not not` over atomic and Boolean operands in ordinary choices and all five admitted function heads, preserving signed contribution identity and positive producer support | [Signed elements](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/signed_element_contracts.rs) |
| L02 | **Closed:** finite conditional disjuncts preserve same-binding condition/head pairs, independent local scope, signed and Boolean heads, empty families and the original implication in the frozen reduct | [Conditional heads](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/conditional_heads.rs) |
| L03 | **Closed:** `not` and `not not` after complete finite anonymous witness projection in universal body conditionals, preserving source alternatives and condition scope | [Consequent alternatives](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/consequent_alternatives.rs) |
| L04 | **Closed:** finite pools in ordinary rules and local choice, aggregate and conditional scopes preserve independent source occurrences, whole-rule products and each local collection's connective and key identity | [Finite pools](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/finite_pools.rs), [Consequent alternatives](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/consequent_alternatives.rs) |
| L05 | **Closed:** nested finite pools and intervals in admitted constructors, expressions and scoped fields preserve independent occurrences, complete value construction, checked binding order and required arithmetic errors | [Finite values](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/finite_values.rs), [Evaluated heads](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/evaluated_heads.rs) |
| L06 | **Closed:** directed finite integer-affine comparison chains involving several unresolved variables, preserving whole-guard correlation, local scope and checked source arithmetic; nonlinear inversion and systems without directed finite bounds remain outside this profile | [Finite chains](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/finite_chains.rs) |
| L07 | **Closed:** complete logical bounds on ordinary choices and admitted numeric count/sum comparisons, preserving logical ordering, signed activity and frozen-reduct truth within existing finite binding and measure profiles | [Logical bounds](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/logical_bounds.rs) |
| L08 | **Closed:** complete outer aggregate-assignment proposals feed finite local head, choice, aggregate-element and universal-conditional consumers without losing their original equality or leaking local bindings | [Conditional consumers](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/conditional_consumers.rs), [Conditional heads](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/conditional_heads.rs) |
| L09 | **Closed:** dependency-ordered finite assignment consumers and multiple assignments preserve complete predecessor bindings, original equalities and objective costs; recursive producer dependencies use completed possible support | [Multiple assignments](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/aggregate_assignments_multiple.rs) |
| L10 | **Closed:** missing/nonnumeric sum-head measures are neutral; missing extremum-head measures use the declared present-first-value projection, retaining independent head permission in guarded and unbounded contexts | [Head contributions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/head_contributions.rs) |
| L11 | **Closed:** negative numeric `#sum+` head measures contribute zero while retaining independent positive permission in guarded and unbounded admitted contexts; fixed-formula semantics and explicit clingo differences are documented | [Head contributions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/head_contributions.rs) |
| L12 | **Closed:** complete first tuple values in extremum heads preserve logical ordering and empty-extremum semantics within the documented numeric endpoint, producer and eligibility boundaries; neutral missing values follow L10 | [Extremum heads](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/extrema_heads.rs) |
| L13 | **Closed:** finite safely bound scalar priorities preserve same-binding weight/tuple identity and objective ordering, including recursive aggregate and conditional producers after support completion | [Source measure carriers](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_measure_carriers.rs), [rich cycles](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_rich_cycles.rs) |
| L14 | **Closed:** scoped weak aggregates, conditionals and finite pooled/ranged objective fields preserve complete weight/priority/tuple identity, polarity, local scope and original-model costs without adding logical producers | [Scoped objectives](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_scopes.rs), [Objective pools](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_pools.rs) |
| L15 | **Closed:** signed and disjunctive dependencies, aggregate heads and recursive aggregate/conditional producer cones use completed finite support and original-model objective queries within the admitted source profiles | [Rich producers](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_rich_producers.rs), [rich cycles](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_rich_cycles.rs) |
| L16 | **Closed:** filtered and multiple observers, mixed extrema and recursive observer dependencies preserve complete finite source rows, independently applicable carrier refinements and original-model costs | [Aggregate objective observers](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/aggregate_objective_observers.rs), [carrier composition](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_carrier_composition.rs) |
| L17 | **Closed:** finite observation bindings preserve whole-chain values, pooled structural capture and single-occurrence checked arithmetic inversion; anonymous negative cardinality keys preserve existential pattern identity, while full answer identity remains unchanged | [Observation bindings](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/observation_bindings.rs), [Inverse contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/observation_inverse.rs), [Expressions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/observation_expressions.rs) |
| L18 | **Closed:** explicit source projection prepares a fixed typed atom domain; projected sessions return unique full answer-set representatives after objective selection with bounded history and separate completion, while ordinary enumeration and WorldView retain full identity | [Projection metadata](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/project_metadata.rs), [Projected sessions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/tests/projected_sessions.rs), [Projected references](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/tests/projected_reference.rs) |

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
termination of arbitrary recursive value generation. The
[pool witnesses](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/objective_pools.rs)
and [observation contracts](language.md#objectives-and-observations) distinguish
admitted finite results from unresolved bindings, unsupported inverse forms and
required arithmetic failures. The remaining unclassified scopes are listed
below; their existence is not a claim of undefined input or resource exhaustion.

Projection closure concerns the declared fixed source-activity domain and its
selected key image. It does not claim that every optional source condition is
jointly realizable or reproduce every simplification of another grounder.
[Projected-answer laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/projected-answers.md)
keep full membership, objective selection and quotient coverage separate.
Independent source comparisons check their declared compatible profiles;
semantic extensions and reporting differences remain explicit.

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
