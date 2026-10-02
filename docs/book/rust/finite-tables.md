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

`Table::prepare_with` and `select_with` use the same algorithms with a caller's
fallible work-admission callback. A positive amount must be admitted before the
charged operation; zero asks the caller to poll control without charging work,
including operation entry and allocation boundaries. `MeteredFailure` preserves
the caller's original typed refusal separately from a table failure, together
with the accepted work prefix and observed capacity peak. Callers must not charge
that receipt a second time. The ordinary preparation and selection methods adapt
`Cancellation` through the same accounting boundary.

The source adapter uses these operations for both eager table joins and certified
computed-domain selection. Independent hybrid checkers atomically admit each
step against their shared `ConstraintAllowance`; a completed batch is never
charged afterward. Failure preserves both local and combined accepted receipts,
and table work and capacity observations remain available on refusal.

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
the table strategy. `JoinStrategy::Indexed` is the default for ordinary probes.
A separately certified computed equality can supply finite domains to the same
selector under either strategy, as described below. These execution choices
preserve complete source meaning and the formula/reduct solver; their different
work and storage costs can change bounded admission.

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
still use indexed probes. `zetesis-bench run` accepts the optional
`--formula-joins indexed|table` setting for compatible native binaries;
omission preserves their default. See the
[validation commands](https://github.com/GregoryGelfond/zetesis/tree/main/crates/zetesis-validation)
for their complete capture and comparison contracts.

The table strategy applies to flat positive atom patterns over the immutable
relation supplied by completed possible support. Its index distinguishes signed
predicates and canonical column scopes. Constants and already-bound source slots
supply singleton domains; ordinary probes leave unbound slots unrestricted. Repeated variables
share a table label, while distinct anonymous slots remain distinct. Each probe
derives fresh domains from the current binding.

Selected row positions feed the existing whole-row matcher. Source-occurrence
identity, binding extension and the original positive atom remain intact.
Authored comparisons, negative conditions and aggregates retain their residual
validation after positive binding. A support row is a possible atom, not an
assertion of its truth in an answer set.

For a completed flat constraint, a separate finite totality check can cover all
scalar inputs using positive source columns. Covered unary equality results can
then provide finite domains for unbound variables of the next occurrence. The
selector intersects those domains with ordinary bound values, constants and
repeated-variable coherence. It shares this same cached table owner under either
join strategy; it introduces no second tuple store. Missing known-side inputs
and all-allowed input groups retain the ordinary probe. Eager materialization,
hybrid capture and frozen hybrid model/region checks share this selector; hybrid
capture prepares speculative computed values before freezing the source owner.
The explicit hybrid table-strategy restriction still applies to ordinary probes.
Source-family evidence continues to traverse complete rows, and all residual
comparisons remain in place. See [grounding](../architecture/grounding.md) for the
exact coverage and failure obligations.

This is a formula-grounding selector. The CLI's `--grounder lazy` uses hybrid
checking for formula input; successfully admitted relational input instead uses
source-lazy reduct closure and its separate joins. That relational closure path
does not acquire this computed-domain certificate or selector.

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

The first restrictive domain initializes the result directly from its coherent
supports: a singleton copies its support, while a finite domain unions matching
supports into the result. Later restrictions intersect the result; later finite
domains reuse one union buffer. An entirely unrestricted or nullary query copies
the original coherent mask. Every supplied domain is still checked, even when
an earlier restriction leaves no rows. This avoids a redundant base-mask copy
and, for a first finite domain, one intersection and potentially a scratch mask.
Retaining a table still leaves per-query mask initialization and traversal.
Result and union masks belong to that query; they cannot be shared as mutable
truth across bindings or workers.

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

## Optional argument-domain guards

Prepared formula and bundle values also expose
`with_domain_analysis(Some(DomainLimits { .. }))`; `None` is the library
default and turns the attempt off, and the ordinary command requests the
attempt with its default limits. This option adds a final-instantiation
consumer for the exact normalized whole program. It is independent of
`Indexed` or `Table` row selection and has no CLI flag.

The initial profile admits ordinary positive flat rules and constraints over
whole named variables, atomic logical values and body comparisons. It excludes
arithmetic in atoms, generators, negative body literals, structured terms,
local scopes and richer heads from the complete attempt. A dependency projection cannot qualify.
Unsupported profiles use the existing complete path; they receive no new
language refusal. A global Unknown or Stopped analysis supplies no narrowing.
An individually Unknown argument supplies no restriction.

For each eligible final rule, the consumer intersects the finite domains of
all positive argument occurrences naming one variable and removes every value a
comparison over that variable alone is defined and false at; a value the
comparison cannot evaluate stays, so the join reaches it. A guard is prepared
only where the candidates are fewer than the argument's domain. Borrowed source
values are resolved through the completed support owner's existing equality
dictionary; no second dictionary assigns IDs. A row failing a necessary domain is rejected
before binding copies and deeper probes, in every support-completion round
and in final instantiation. Surviving rows keep their original positions,
matcher and emission order. Objective joins and existing factorized rule
plans retain their existing paths. The analyzer is an
upper-bound producer, not a candidate-truth or answer-set certificate.

`DomainObservation` exposes Disabled, Inapplicable or the actual borrowed
`Analysis`, including its owner, status, context and logical statistics. Detailed
observation emits a sequential `domain_analysis` phase only when requested.
`domain_prepare_work` includes charged applicability, analysis and guard
preparation, including a stopped or failed prefix. `domain_guard_rows`,
`domain_guard_checks` and `domain_rejected_rows` count actual row visits,
dictionary-ID comparisons and rejections; `domain_excluded_values` counts
the values comparisons excluded before any row was read, charged to the
analysis phase that prepares the candidates. Visited rows remain in `join_rows`
and `table_rows`; avoided deeper work appears in the existing probe counts.

Analyzer populations and inspected source bytes have separate finite
`DomainLimits`. Its admitted work is capped by and charged to the remaining
cumulative formula work budget, with no refund on Stopped. It uses bounded
standard collections and has no fallible-allocation or caller-control API.
This is an uninterruptible operation under the existing eager contract, not a
new cancellation/deadline guarantee. Its heap is outside `max_support_bytes`.
Rule guards separately charge their named headers, preparation scratch,
dictionary IDs and actual vector capacities beside retained support, table
indices and live masks. Candidate terms remain borrowed canonical references.
Query-attempt receipts retain original
failed work and actual capacity peaks. These capacities exclude allocator
metadata, analyzer heap and a complete stack/RSS measure. Additional analysis,
guard and mask costs may outweigh avoided bindings or probes.

The [domain-binding guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/domain-bindings.md)
states the necessary argument-coverage and complete-continuation premises.
Source controls compare enabled/disabled complete atoms, theories and provenance,
plus actual row/probe counts and failure receipts. They do not prove a general
source-to-Rust refinement or a performance improvement.

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
