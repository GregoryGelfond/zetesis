# Selecting and projecting finite table rows

`zetesis_cpu::table` selects complete rows from an immutable typed relation.
Callers can retain the selected row positions directly or project the values
witnessed by those rows. Eager formula grounding can explicitly choose this
primitive for flat positive joins over completed possible support; the default
grounder retains shortest-posting joins.

Suppose a table lists allowed pairs of departure and arrival values. A domain
restriction on either variable can remove rows; each remaining row witnesses
one value for each variable. The projected domains contain exactly those
witnessed values. This operation preserves the table's solutions. Applying it
to an ASP program additionally requires proving that the supplied table is a
complete description of the relevant constraint.

## Ownership and exact projection

`Table::prepare` borrows one authoritative `Relation`. Its column scope names
variables in first-occurrence order: `[0, 1, 0]` assigns the first and third
columns to the same variable. Rows violating that equality are excluded.
Logical values retain their types; duplicate row occurrences retain their
original positions.

`Table::select` accepts one `Domain` per variable: `Unrestricted`, a borrowed
`Singleton`, or a borrowed `Finite` slice. An empty finite domain permits no
value; it is distinct from an unrestricted domain. Every call starts from the
original coherent rows, so domains may narrow, widen or return to an earlier
state. Equal domain sizes do not imply equal contents.

The resulting `Selection` owns a packed row mask and borrows only the original
relation. It can outlive the prepared index and input domain slices. `rows()`
visits increasing original positions without allocating a position vector;
`contains` and `next_row` support direct inspection. Selection does not compute
projected domains or copy row values.

`next_row_with` lets a consuming operation charge work or check interruption
before each inspected mask word, including zero words. A failed check publishes
no row and leaves the selection and supplied starting position unchanged. The
ordinary scanning methods use the same decoder without a fallible callback.

`Table::project` retains its finite-domain interface; `project_domains` accepts
the same `Domain` variants as selection. Both reuse the row restriction operation
and then compute the witnessed domains. A projection borrows the table; its
returned values borrow the original source.

```rust
# extern crate zetesis_core;
# extern crate zetesis_cpu;
{{#include ../examples/finite-tables.rs:example}}
```

Preparation stores a support bitset for each distinct variable/value pair.
Projection composes unions, intersections and witness tests:

```text
rows := coherent_original_rows
for each variable v:
    allowed := union(support[v, x] for x in domain[v])
    rows := rows intersect allowed
for each variable v:
    projected[v] := {x in domain[v] | rows intersect support[v, x] is nonempty}
```

Selection stops after restricting `rows`; projection additionally collects
the witnessed values. The explicit initial row mask handles a table with no
columns. The same whole row must witness all its columns; independently
occurring values cannot create a new tuple.

## Positive joins in eager formula grounding

On a `PreparedFormula` or `PreparedFormulaBundle`,
`with_grounding_options(GroundingOptions { joins: JoinStrategy::Table })` selects
the table strategy. `JoinStrategy::Indexed` is the default. This is an execution
choice made before materialization; it changes neither source admission nor the
formula/reduct solver.

This example materializes a program using each join strategy and compares the
complete admitted atoms, formulas and source locations. Repeated `X` requires
equal route endpoints; the `rail` constant restricts the final column. A work
observer checks that the table index was prepared, queried and reused.

```rust
# extern crate themelios_base;
# extern crate zetesis_themelios;
{{#include ../examples/table-grounding.rs:example}}
```

The [source preparation API](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula/preparation.rs)
owns materialization; the
[observer API](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/grounding_observer.rs)
reports work without reading a clock. Both strategies return the same
`AdmittedFormula` boundary for downstream consumers.

The CLI exposes the same choice. For example, use the formula countermodel route
and retain its grounding statistics:

```sh
zetesis --grounder eager --oracle countermodel --formula-joins table --stats input.lp
```

The requested strategy alone is not route evidence. Inspect `table_preparations`,
`table_probes` and `table_rows` in the grounding work report; inapplicable patterns
still use indexed probes. `zetesis-bench grounding --joins table` profiles this
materialization against an indexed reference. `zetesis-perf` accepts the optional
`--formula-joins indexed|table` setting for compatible native binaries; omission
preserves their default. See the
[grounding experiment](https://github.com/GregoryGelfond/zetesis/tree/main/crates/zetesis-experiments#original-source-grounding)
and [validation commands](https://github.com/GregoryGelfond/zetesis/tree/main/crates/zetesis-validation)
for their complete capture and comparison contracts.

The table strategy applies to flat positive atom patterns over the immutable
relation supplied by completed possible support. Its index distinguishes signed
predicates and canonical column scopes. Constants and already-bound source slots
supply singleton domains; unbound slots are unrestricted. Repeated variables
share a table label, while distinct anonymous slots remain distinct. Each probe
derives fresh domains from the current binding.

Selected row positions feed the existing whole-row matcher. Source-occurrence
identity, binding extension and the original positive atom remain intact.
Authored comparisons, negative conditions and aggregates retain their validation
after positive binding; they do not provide extra table domains. A support row
is a possible atom, not an assertion of its truth in an answer set.

Structural patterns and support-growth rounds retain indexed joins. This is a
declared applicability boundary, not recovery from a failed table operation.
Table allocation, capacity and work refusals remain grounding failures. The
enclosing grounding operation owns cumulative index, live-query and work bounds;
each probe does not receive a fresh independent source budget.

## Cost and applicability

For R rows and E indexed variable/value entries, support storage uses
`E * ceil(R / 32)` words. Many distinct values can make this representation
larger than the original columns. Repeated queries may amortize preparation;
that is a measurement question, not a guarantee of the API.

`Limits` bounds support entries, live operation capacity and charged work.
Cancellation, exhausted limits and allocation failures return no partial table,
selection or projection. `Statistics` distinguishes retained capacity from the
conservative operation peak. Neither measure is process RSS.

The implementation is scalar. Independent callers may share the immutable
table across CPU workers; each query owns its result mask and scratch. That does
not parallelize the grounder's mutable bindings, formula builder or source-error
order. No GPU table kernel or automatic crossover policy is selected.
The [GPU Compact-Table paper](https://arxiv.org/abs/2507.18413)
motivates examining this family of support operations; its performance results
do not establish a speedup for zetesis.

## Compare complete projections

The maintained experiment compares a prepared row scan, scalar support bitsets
and independent projections sharing one table through Rayon:

```sh
zetesis-bench table --case independent --rows 1024 --queries 32 \
  --workers 4 --warmups 1 --repetitions 3 > table.jsonl
```

The other cases are `correlated` and `aliased`. Every route receives the same
typed rows and sequence of narrowing, replacement and restored domains. An
independent whole-row reference checks row positions and projected domains.
JSON-lines output retains the subject, preparation costs, every batch and
explicit limit refusals. A completed schedule with such refusals exits 1;
configuration, execution, parity or output failures exit 2.

Compare Rayon batch wall times, since individual worker intervals overlap.
Preparation, projection and common-output conversion have separate intervals.
The fixed scan/table/Rayon route order does not control cache or thermal effects.
The capacity receipts describe the named objects, not RSS or total concurrent
memory. See the [experiment contract](https://github.com/GregoryGelfond/zetesis/tree/main/crates/zetesis-experiments#finite-table-domain-projection)
for finite populations and report limits. This experiment establishes neither
source-grounding completeness nor ordinary solver acceleration.

The [measured comparison](../reference/performance.md#optional-finite-table-experiment)
reports the current fixture timings, preparation costs and retained-capacity
limitations.

## Preservation argument

A selected row belongs to every variable's union of permitted supports, hence
its complete values satisfy the supplied domains. Conversely, a permitted
coherent row supplies its own value in each union. Projection cannot remove
any value of a surviving row or introduce a row that lacked a witness.

`FiniteTables.indexed_survival_exact`,
`narrowing_preserves_rows`, `narrowing_idempotent` and
`widening_preserves_rows` formalize these statements in
[the Lean module](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FiniteTables.lean).
`TableBindings.flat_match_survives` derives the necessary domains and aliases
from successful flat matching. `indexed_matches_preserved` and
`join_family_preserved` retain the ordered source/row witnesses and resulting
bindings through selection and a fixed finite positive-join schedule. The
[binding-family guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/table-bindings.md)
explains their premises and relation to original source atoms.

These laws do not verify Rust bitsets, resource accounting, fallible execution
or the complete source-grounding translation. A surviving table row is not by
itself an answer set; reduct membership remains a separate obligation.
