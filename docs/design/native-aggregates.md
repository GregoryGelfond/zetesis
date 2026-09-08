# Retained native aggregate operations

`zetesis_ferraris::native_aggregate` computes count, signed sum, positive sum,
minimum and maximum directly over finite tuple eligibility. It is a library
primitive with a differential test and benchmark consumer. It does not yet
replace Boolean aggregate lowering in ordinary source admission or solving.
There is no new formula node, candidate-search algorithm or device route in this
slice. The immutable original theory remains the subject of reduct checking.

## Semantic boundary

A [`Group`](../../crates/zetesis-ferraris/src/aggregate/native.rs) owns complete,
already-coalesced tuple keys, eligibility node IDs, one aggregate function and
an ordered list of guards. Each guard compares the measure with a wide integer
or an ASP term; their conjunction defines the aggregate predicate `P`.

For original interpretation `M` and tested interpretation `J`, let `A` contain
the tuple occurrences whose eligibility formulas hold in `M`, and let `B`
contain those whose eligibility formulas, frozen in `M`, hold in `J`. The
operation exposes both measures and both guard conjunctions:

```text
original aggregate truth = P(A)
aggregate reduct truth   = P(A) ∧ P(B)
```

The second conjunction is essential: the frozen eligibility mask alone cannot
establish aggregate-reduct truth. Aggregate inequality is also a comparison of
the measure, not default negation of equality. The regression suite contains a
counterexample with `#count { p } != 0`, `M = {p}` and `J = {}`.

This is aggregate-formula truth. Stable membership additionally requires the
original theory and the absence of a proper reduct model; this primitive does
not perform that search. `J` need not be a subset of `M` for formula evaluation.
The general original/frozen aggregate law belongs to the Lean semantic layer;
passing the Rust comparisons is not a proof that Rust or a future shader refines
that law.

Whole keys define tuple identity. Equal first components or equal eligibility
formulas do not merge distinct keys. Duplicate whole keys are refused, including
duplicate empty keys; a source producer must first OR-coalesce their conditions.
The constructor preserves input order and retains neutral contributions:

| Function | Selected contribution | Empty selection |
| --- | --- | --- |
| Count | One per complete key, including the empty tuple | Integer zero |
| Sum | Numeric first component; empty/nonnumeric keys are neutral | Integer zero |
| SumPlus | Strictly positive numeric first component; other keys are neutral | Integer zero |
| Min | First component in ASP term order; empty keys are neutral | `#sup` |
| Max | First component in ASP term order; empty keys are neutral | `#inf` |

Integers are accumulated with checked `i128` arithmetic and remain wide in the
result and guard comparison. They are never narrowed into a source `i32` term.
Ordered extrema reuse the core flat logical value representation and ASP term
comparison, including signed constants, strings, tuples, function terms and
genuine `#inf`/`#sup` endpoints. No floating-point values or encoded sentinels
stand in for exact arithmetic or absence.

## Three operations and their costs

`Group::new` transfers the tuple and guard vectors, shares the immutable `Theory`
instance, validates condition IDs, and checks whole-key uniqueness through a
bounded bottom-up index merge sort. It does not lower formulas or establish that
the retained theory contains, activates or entails this aggregate. Source and
candidate-planning bridges must retain that separate evidence.

Admission has independent tuple, key-value, value-node, guard, work and payload
limits. Transferred vector capacities and referenced term payload are charged;
already allocated inputs remain the caller's construction cost. Duplicate
checking uses two index arrays and temporary key-comparison costs. Sorting is
`O(n log n)` comparisons, with linear retained/scratch arrays in tuple count.
Comparison costs include both full admitted term carriers even when a
lexicographic comparison stops earlier. Referenced shared value payload is
conservatively charged per occurrence. Payload figures exclude allocator
metadata, stack and extra string capacity; they are not process RSS.

`Group::eligibility` borrows actual `M` and optional `J`, checks their original
theory instance, evaluates the existing topological formula prefix and projects
each tuple's condition into an ordered mask. Frozen evaluation reuses the
original node truth and reads atom truth from `J`. This path narrowly reuses the
existing oracle evaluator; its public semantics are unchanged. Repeated
condition IDs remain separate tuple occurrences. The returned `Eligibility`
record privately binds masks to that group and interpretation pair.

Eligibility acquisition has separate work and byte ceilings. For `v` formula
nodes, `n` tuples and `p` requested phases (one or two), it charges `p(v+n)` node
and projection visits and reserves at most `p(v+n)` logical Boolean cells. The
retained masks contain `pn` cells. Borrowed group, theory and interpretations
are excluded. It currently evaluates the complete retained formula prefix;
group-specific dependency slicing or shared acquisition is future work.

`Group::reduce` accepts original and optional frozen masks of exactly `n`
occurrences. These raw masks are observations supplied by the caller, not proof
receipts. `Eligibility::reduce` supplies the actual acquired masks without
repeating prefix evaluation. Reduction performs no heap allocation, visits each
mask cell once, and evaluates every guard, including guards following a false
one. Numeric work is linear in tuples plus guards; extrema and guards additionally
charge compared term carriers. Returned term measures borrow retained group
values or genuine empty endpoints. Reduction has its own work ceiling, separate
from acquisition and construction.

All three operations poll cancellation/deadlines and return typed refusals with
their local accounted prefix. Work and dimension arithmetic are checked.
Comparison-carrier charges are atomic: a refusal can leave unused budget smaller
than the next charge. No partial group, eligibility record or reduction is
published as a completed result. A refusal is not false aggregate truth or UNSAT.

## Qualification and measurement boundary

The [semantic controls](../../crates/zetesis-ferraris/tests/native_aggregates.rs)
compare direct guards with existing numeric/ordered-extremum lowering for every
`M,J` pair over two atoms, all six comparison polarities, signed/neutral values
and recursive/default-negated eligibility formulas. Other controls cover wide
sums, empty tuples and aggregate inequality. The
[admission controls](../../crates/zetesis-ferraris/tests/native_aggregate_limits.rs)
use an independent bounded quadratic key-equivalence reference, exact/one-short
budgets, identity, occurrence length and cancellation cases.

The [Criterion consumer](../../crates/zetesis-ferraris/benches/native_aggregates.rs)
has ten declared finite fixtures and three distinct populations each:

1. `masks`: preacquired masks, direct measure and complete guard evaluation.
2. `acquire_reduce`: actual original/frozen formula acquisition plus reduction.
3. `lowered_public_calls`: the existing `models` and `models_reduct` calls;
   the latter recomputes original truth under its public API.

Every invocation evaluates the same ordered sixteen `M,J` pairs, including true
and false aggregate/reduct outcomes. Construction, lowering, interpretation/mask
creation and complete parity checks precede measurement. Output is a fixed-size
truth-pair array. Count, positive sum and extrema use 8/64 tuples; signed sum uses
4/8 because its existing lowering enumerates subsets. Throughput denotes
semantic eligibility occurrences, not hardware instructions. These scopes are
not interchangeable, and an acquisition-free result is not an end-to-end solver
speedup.

Run benchmark correctness without accepting timings with:

```sh
cargo bench --locked -p zetesis-ferraris --bench native_aggregates -- --test
```

The [qualification record](../verification/native-aggregates-20260908/README.md)
retains commands, failures and source identities. No actual timing or physical
GPU result is claimed by this primitive's initial qualification.
