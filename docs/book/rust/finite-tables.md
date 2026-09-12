# Projecting finite domains through a table

`zetesis_cpu::table` is an optional library primitive. It does not participate
in ordinary answer-set solving. It selects complete rows from an immutable
typed relation and returns the values witnessed by those rows.

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

`Table::project` accepts one finite domain per variable. Every call starts from
the original coherent rows, so domains may narrow, widen or return to an earlier
state. Equal domain sizes do not imply equal contents. A projection borrows
the table; its returned values borrow the original source.

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

The explicit initial row mask handles a table with no columns. The same whole
row must witness all its columns; independently occurring values cannot create
a new tuple.

## Cost and applicability

For R rows and E indexed variable/value entries, support storage uses
`E * ceil(R / 32)` words. Many distinct values can make this representation
larger than the original columns. Repeated queries may amortize preparation;
that is a measurement question, not a guarantee of the API.

`Limits` bounds support entries, live operation capacity and charged work.
Cancellation, exhausted limits and allocation failures return no partial table
or projection. `Statistics` distinguishes retained capacity from the
conservative operation peak. Neither measure is process RSS.

The implementation is scalar. Independent callers may share the immutable
table across CPU workers. No GPU table kernel or automatic source transformation
is selected. The [GPU Compact-Table paper](https://arxiv.org/abs/2507.18413)
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

## Preservation argument

A selected row belongs to every variable's union of permitted supports, hence
its complete values satisfy the supplied domains. Conversely, a permitted
coherent row supplies its own value in each union. Projection cannot remove
any value of a surviving row or introduce a row that lacked a witness.

`FiniteTables.indexed_survival_exact`,
`narrowing_preserves_rows`, `narrowing_idempotent` and
`widening_preserves_rows` formalize these statements in
[the Lean module](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FiniteTables.lean).
They do not verify the Rust bitsets, resource accounting or a source-grounding
translation. A surviving table row is not by itself an answer set; reduct
membership remains a separate obligation.
