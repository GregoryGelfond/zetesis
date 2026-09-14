# Delta rounds with completed history

A scalar reduct computation fixes the candidate and grows positive truth from
empty. Repeating every old binding is unnecessary if its consequences were
already published and its constraint effects were retained. The mathematical
question is what information a delta round must preserve before it can stand
in for the complete consequence step.

[`DeltaRounds`](../Zetesis/DeltaRounds.lean) imports the occurrence partition in
[`DeltaJoins`](../Zetesis/DeltaJoins.lean) and the binding, filter, gate and
projection definitions in [`Lifted`](../Zetesis/Lifted.lean). It proves three
schedule laws. It does not change the reduct or supply a new answer-set definition.

## First-new binding coverage

A complete binding has one row choice for each positive source occurrence.
The same predicate can occur more than once; those occurrences keep separate
positions in the row-choice witness. For each occurrence, old row IDs are a
prefix of current row IDs. IDs denote stable insertion positions, rather than
canonical ranks that may move when smaller tuples are appended.

`coverage_of_first_delta` has three substantive premises. `currentRows` maps
current positive bindings to current row IDs. `oldRows` says that such a binding
was positive in the old snapshot exactly when every chosen ID was old.
`partitions` supplies every enabled, filter-valid current binding in each
first-new partition. These are correspondence and coverage assumptions, not
consequences of knowing relation sizes.

If a current binding was not old, at least one chosen row must be new.
`DeltaJoins.partition_complete` supplies its first new occurrence. The corresponding
partition selects Old rows before that occurrence, New rows at it and Current
rows afterward. The supplied selection therefore includes the binding.
`DeltaJoins.partition_disjoint` separately proves that the pivot is unique.
Neither result says that different row combinations produce different heads;
several bindings can have the same consequence.

## Old heads and the inflationary step

`delta_step_exact` assumes the coverage just described and that every old
enabled consequence is already in current truth. It concludes that retaining
current truth and adding selected delta consequences has exactly the same effect
as retaining current truth and adding the complete current consequence set.

The proof separates a complete consequence into two cases. If its positive
binding was old, the history premise already places the head in current truth.
Otherwise delta coverage selects the binding. In the other direction, the
selected transform checks the complete current body, filter and fixed candidate
gates, so `Lifted.materialized_sound` supplies soundness. The old snapshot need
not itself be closed. The theorem preserves the resulting atom set, not the
list or order of evaluated bindings.

Facts and other zero-positive rules have no new positive occurrence. They
therefore need a separate complete bootstrap scan. Their enabled heads must
already satisfy the old-consequence premise before the delta schedule omits
them. An empty delta alone is not evidence that bootstrap or earlier scans
were complete.

## Constraints need their own history

A constraint has no head to retain in current truth. Its effect is a trigger
that must persist separately. `latched_constraints_exact` considers an entire
constraint family and requires the old latch to mean exactly that some member
of this family triggered in the old snapshot. The law also assumes growing
positive truth and new-binding coverage for every member.

An old trigger persists because positive truth grows while filters and candidate
gates stay fixed. A current trigger either was old, and is already latched, or
has a new positive combination covered by the selection. A selected trigger
still checks the complete current constraint. This proves equality between the
old latch combined with new triggers and the complete current trigger test.
Zero-positive constraints must enter the latch during bootstrap, including a
bootstrap that derives no heads. A true latch is not permission to skip other
required scans or return a partial closure.

For example, consider the fact `a`, the rules `b :- a` and `c :- b`, and the
constraint `:- c`. Bootstrap derives `a`; the next rounds derive `b` and `c`.
The final round first encounters the constraint's new `c` row and derives no
new head. Its rejected result still contains the complete closure `{a,b,c}`.
The fact need not be reevaluated in that final round because its earlier
completed consequence is retained.

## The scalar implementation boundary

The independent scalar evaluator in
[`zetesis_cpu::oracle`](../../crates/zetesis-cpu/src/oracle.rs) keeps one
[`Catalog`](../../crates/zetesis-core/src/relation/catalog.rs) per signed
predicate. `OrderedRows::row_id` connects canonical access positions to stable
insertion IDs. The private
[partition owner](../../crates/zetesis-cpu/src/oracle/relations/partition.rs)
constructs ordered Old/New ID slices over that same tuple payload, with a cache
key containing both cutoff and current extent. No second tuple store is created.

`visit_round` checks complete selected bindings under the same frozen `SeedView`.
Bootstrap uses full traversal, including zero-positive rules and constraints.
After a growing round completes every selected scan, `least_closure_with`
retains the constraint trigger, advances old cutoffs, and appends the bounded
pending heads. It never
appends while a borrowed round view is live. The final no-change round combines
with completed history; it does not rescan every old binding. The existing gate
comparison then checks the final closure's projection against the seed in both
directions.

The reusable workspace retires dirty state on failure, and successful atom
extraction clears cutoffs, cache keys and logical ID lengths before another
candidate. Typed rows, actual view capacity, preparation and probe work are
covered by Rust controls. The private full-round reference compares literal
closures, constraints and seed agreement, alongside inclusive work and named
capacity receipts. Exact resource cutoffs can differ between schedules. The
reference is not a runtime mode or a second public implementation.

Public `source::scan` and shared-world source traversal retain their full
ordered schedules. Eager possible-support discovery has a separate eligible
producer grammar and a distinct correspondence obligation. None of these
scalar laws establishes shared-world or device delta execution.

The Lean module does not prove that the concrete catalog, row matcher, source
registration or publication code satisfies its premises. It also does not prove
machine bounds, cancellation, finite termination or a speedup. Leastness and
answer-set membership require the existing semantic bridges plus sound,
completed execution. A smaller binding or tuple-probe counter cannot substitute
for those obligations, and does not establish lower elapsed time.
