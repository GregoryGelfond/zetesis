# Recorded release observations

Four fixed datasets reproduce the ordinary release comparisons in the
[performance reference](../performance.md). Each retains its own sources,
executable identities, observations and completion limits. None qualifies a
later implementation.

The [prepared-grounding CPU/Metal comparison](../prepared-metal.md) also retains
plain timing data: [eager intervals](prepared-metal-eager-20260914.tsv),
[lazy intervals](prepared-metal-lazy-20260914.tsv) and
[lazy provenance](prepared-metal-lazy-provenance-20260914.tsv). These contain
486 eager and 3,456 lazy timed observations, preserving block/source identities
and raw report or stream hashes. Eager rows retain the six original intervals
and each block's median/range; lazy rows retain one original interval each.
Qualification and warmup results belong to the acquisition review described in
the chapter, rather than being inferred from these timed-only files. These
TSV views are separate from the four fixed JSON datasets and their renderer.

## Table-grounding comparison

The [observation data](release-6bebb980-1e5b78ce.json),
[provenance](release-6bebb980-1e5b78ce-provenance.json) and
[table view](release-6bebb980-1e5b78ce-tables.md) reproduce the canonical release
comparison in the [performance reference](../performance.md#canonical-release-comparison).
They describe source `6bebb980f9c102dbb7f943076d7cde92374841ce` versus
`1e5b78ce913ab3aeece6ed496f69ca8176f0644d`, measured on 12 September 2026.
They do not qualify a later implementation.

## Atom-catalog comparison

The [observation data](release-1e5b78ce-ca10a5e7.json),
[provenance](release-1e5b78ce-ca10a5e7-provenance.json) and
[table view](release-1e5b78ce-ca10a5e7-tables.md) describe source
`1e5b78ce913ab3aeece6ed496f69ca8176f0644d` versus
`ca10a5e7ec84e13fbcc4a23bd0de8b0232c53fe1`, observed on 14 September 2026
from 03:42:07 to 03:43:27 UTC. Both native versions explicitly use Indexed joins.
Task allocation uses less sampled child RSS, SEND takes longer, and the other
workloads have mixed results. The [earlier atom-catalog comparison](../performance.md#earlier-atom-catalog-cpu-comparison)
keeps all four block medians and ranges visible.

## Prepared-grounding comparisons

Two views select from one six-block acquisition on 14 September 2026:

| Comparison | Observations | Provenance | Tables |
|---|---|---|---|
| `ca10a5e7` → `679ca856` | [data](release-ca10a5e7-679ca856.json) | [receipts](release-ca10a5e7-679ca856-provenance.json) | [table view](release-ca10a5e7-679ca856-tables.md) |
| `f56a5a24` → `679ca856` | [data](release-f56a5a24-679ca856.json) | [receipts](release-f56a5a24-679ca856-provenance.json) | [table view](release-f56a5a24-679ca856-tables.md) |

The full source identities are `ca10a5e7ec84e13fbcc4a23bd0de8b0232c53fe1`,
`f56a5a2496f519d7b71b7c4c8fdc166c355874ff` and
`679ca8568a6fd8577d9b944fbd99d7c54f666601`. The actual block order was
`ca10a5e7-1`, `f56a5a24-1`, `679ca856-1`, `679ca856-2`, `f56a5a24-2`,
`ca10a5e7-2`. Each view retains its four blocks in that order, with 468
observations. Both contain the same two `679ca856` reports: the acquisition has
**702 unique observations**, not 936 independent observations. Each provenance
records all six report identities and timestamps, its selected order and the
shared reports. The report timestamps do not measure outer publication duration.

All three versions explicitly request Indexed joins. The first comparison covers
the integrated changes from the atom-catalog release; the second compares the
prepared-owner implementation with the subsequent algorithm changes. These are
application comparisons. The provenance preserves the native versus explicit
Apple target/package recipe distinction; matching top-level GPU features does
not establish full transitive build equivalence or isolate one change's effect.

## Series baseline

[series-896a5f73-cpu-auto.json](series-896a5f73-cpu-auto.json) and its
[table view](series-896a5f73-cpu-auto-tables.md) are the derived comparison
of one `zetesis-perf --suite series --profile cpu-auto` campaign on the
executable built from `896a5f73fc9d3def3b0b4dd3ce3a1904dc7555ed`
(SHA-256 `a7e3c81305a6bbaef8757f1a83d1cc5159e6bdf7664a9e1c7534993e2075b5d2`)
against clingo 5.8.2, observed on 16 September 2026 from 18:40:01 to
18:41:08 UTC on an AMD Ryzen 7 7840U under Linux. One warmup and three timed
rounds per cell and producer; the raw report of 481 MB, which retains every
native record, is not published; its SHA-256 is
`aac1254e8123a7f756d47c3ca7a11e96a0724a72b4ceb86bc40ca3efcbf77319`. Nineteen
cells pass complete-family parity; the stratified cell reaches its 30-second
process deadline in qualification, which disables its later positions, and
the derived view reports that decision rather than a time. This is the
first column of the series that measures the changes following the audit of
15 September; later comparisons add their own columns from their own
campaigns, always beside a fresh baseline column.

The derived view (`zetesis-series --json`) keeps exact integer medians,
minimum and maximum of the timed intervals per cell and producer, the
counters the native records carry, the decisions of cells that did not pass,
and each report's native, reference and manifest seals. It is a derived
observation view, not an archive of the report.

## Item 10: counters and receipts

[series-6a43c71b-cpu-auto.json](series-6a43c71b-cpu-auto.json) and its
[table view](series-6a43c71b-cpu-auto-tables.md) compare two campaigns run
in one session on 16 September 2026 from 19:57:26 to 20:04:35 UTC on the
same machine: `main`, the 896a5f73 executable above rerun as the control, and
`after`, the executable built from
`6a43c71be799b6c5d5dae054cdbaf625be6afeb9`
(SHA-256 `55866ff58f55517ef6303cb3a25e56ebbea4fb7c50d10b2d66a5f12ca448559d`),
the head of the counters-and-receipts change. Raw reports of 481 MB each are
not published; their SHA-256 are
`47426d207c6001d42e2b4fdf3befee18dd501deecea92f5cdd8fb2a439b52f79` (main) and
`0d568421f1a697c4690630e005885657d14482c776c87fee1baeb3997e13b8c3` (after).
The change sums counters the oracle already computed, so no speedup was
expected and none is claimed: the after/main ratios of the nineteen passing
cells lie between 0.93 and 1.07, inside the spread of the three timed
rounds. What the observation establishes is the receipt itself: the `work`
column, which the baseline view left empty for every closure-route cell, now
carries the charged closure work summed over completed checks, from 88,672
units for the producer chain to 470,272,422 for the independent-negation
cell at size 10.

## Item 4: the deadline timer

Four campaigns on 16 September 2026 between 20:36:00 and 20:55:40 UTC,
same machine and clingo as above, each binary once without a deadline and
once with `--time-limit 3600`, a deadline no cell reaches:
[series-7d7413ae-cpu-auto.json](series-7d7413ae-cpu-auto.json) with its
[table view](series-7d7413ae-cpu-auto-tables.md) compares the runs without
a deadline, and
[series-7d7413ae-cpu-auto-deadline.json](series-7d7413ae-cpu-auto-deadline.json)
with its [table view](series-7d7413ae-cpu-auto-deadline-tables.md) the runs
with one. `main` is the 896a5f73 executable rerun as the control; `after`
is built from `7d7413ae1b1ee220d534be7b0df37854e4ab0966` (SHA-256
`0e8814954b5eec03e1b82b3ec6abb9f7447abb3d4d15a0eae4588e70cba946c5`), where
a timer thread marks the deadline and a poll reads flags only. The four raw
reports are not published; their SHA-256 are
`5e0417247174ce6d30ac82ea8d858cbf0038d3d3a90043babe7409758e327b33` (main),
`3a3cdee586ff966098241f143270b0bd850e0f1610e636b843552576cfd4e2ed` (main
with deadline),
`269e16be5df8480e48b20bd091c9ed373c8e80a260771d32e9f00a5d527e6157` (after)
and `cb73be7a754b940edbef5d46fb3b70c3232a89e7f5d8fb4fe19875bb1449aae9`
(after with deadline).

The quantity this step changes is the cost of an unreached deadline, the
ratio of a cell's native median with `--time-limit 3600` to the same cell's
median without it, read across the two views for each binary:

| Cell | main | main+deadline | ratio | after | after+deadline | ratio |
|---|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 23.0 | 30.1 | 1.306 | 22.3 | 23.1 | 1.036 |
| independent-choice-16 | 197.4 | 241.5 | 1.224 | 203.5 | 193.8 | 0.953 |
| independent-negation-8 | 110.5 | 199.9 | 1.809 | 110.5 | 107.4 | 0.972 |
| independent-negation-10 | 1423.3 | 2557.3 | 1.797 | 1381.8 | 1367.2 | 0.989 |
| independent-negation-aggregate-16 | 350.4 | 407.3 | 1.162 | 310.8 | 299.9 | 0.965 |
| disjunction-12 | 218.1 | 357.5 | 1.639 | 213.2 | 211.4 | 0.991 |
| ties-50 | 182.7 | 332.5 | 1.820 | 185.1 | 186.9 | 1.010 |
| transitive-path-100 | 32.6 | 59.6 | 1.830 | 31.4 | 30.3 | 0.965 |
| transitive-path-200 | 186.4 | 364.8 | 1.957 | 178.0 | 182.1 | 1.023 |
| transitive-dense-40 | 39.7 | 79.8 | 2.010 | 38.8 | 39.0 | 1.004 |
| chain-1000 | 101.0 | 207.3 | 2.053 | 95.9 | 98.2 | 1.024 |
| chain-2000 | 356.5 | 775.1 | 2.174 | 345.2 | 347.5 | 1.007 |
| chain-arithmetic-1000 | 10.0 | 14.1 | 1.412 | 9.2 | 9.9 | 1.073 |
| stratified-16 | n/a | n/a | n/a | n/a | n/a | n/a |
| producer-chain-700 | 26.9 | 30.3 | 1.126 | 25.9 | 23.6 | 0.913 |
| n-queens/variant-01 8→10 | 125.0 | 438.4 | 3.506 | 122.4 | 123.9 | 1.012 |
| n-queens/variant-01 8→11 | 455.6 | 2002.2 | 4.395 | 460.9 | 467.7 | 1.015 |
| n-queens/variant-04 8→11 | 310.8 | 1303.6 | 4.194 | 314.4 | 329.1 | 1.047 |
| send-money/send-money | 49.7 | 108.7 | 2.186 | 49.9 | 49.2 | 0.986 |
| variant-04/05-larger-mix | 342.2 | 1138.0 | 3.325 | 328.3 | 341.1 | 1.039 |

On the control an unreached deadline costs between 13 percent and a factor
of 4.4, most on the formula-route cells (the queens boards and the task
allocation mix) where a charged unit is shortest and the clock read that
each poll made dominated it. On the changed executable the same ratio lies
between 0.91 and 1.07, inside the spread of three timed rounds. Without a
deadline the two executables agree: after/main lies between 0.89 and 1.03
across the nineteen passing cells, so the timer thread costs nothing
measurable when it is idle. The audit's description of a "strided" poll in
the shared completion budget was inaccurate: that budget polls on each
refill of a 64-permit grant to amortise its lock, and it is unchanged.

## Item 5: the worker product

Five campaigns on 16 September 2026 between 21:22:40 and 21:46:33 UTC, same
machine (eight cores, sixteen hardware threads) and clingo as above. `main`
is the 896a5f73 executable; `after` is built from
`c6ebce2ed80e8fb5a21bfa19a924b36557fa82ed` (SHA-256
`ec3b59e3709dde7d0dd36d8a8292e36ea730237b3d128663c6509f680c6260a1`), where
`SolveConfig::validate` refuses `workers × max_closure_bytes` above the
collective ceiling before a session starts and the command derives the
per-closure allowance as each worker's share of that ceiling.
[series-c6ebce2e-cpu-auto.json](series-c6ebce2e-cpu-auto.json)
([tables](series-c6ebce2e-cpu-auto-tables.md)) compares both at four
workers;
[series-c6ebce2e-cpu-auto-8-workers.json](series-c6ebce2e-cpu-auto-8-workers.json)
([tables](series-c6ebce2e-cpu-auto-8-workers-tables.md)) at eight; and
[series-c6ebce2e-cpu-auto-16-workers.json](series-c6ebce2e-cpu-auto-16-workers.json)
([tables](series-c6ebce2e-cpu-auto-16-workers-tables.md)) records the new
executable alone at sixteen. Raw report SHA-256:
`1c3d42e4422e3b7c8a2a6d5397934b6999c3bb56e6fcdcbd1d60d72c036ff63f` (main,
4), `2df3fc12ede37548673e0a9989e8b34d23e4d69638dce367ffa45dab0edf3107`
(after, 4), `58b21ac998153b27b49627430c942b65d7b255f0b3ec125956da44f1f6ec87a7`
(main, 8), `c399a3f0b1af3b896c39a2ad568a0de644f558e5df63d794e317be0ca6e5ba91`
(after, 8) and
`5f2ded556a29ce2f1e3b0ada39c9171efe0476798dd6edc585f2b179739d7f20` (after,
16).

At four workers the two executables agree (after/main 0.94 to 1.11 on
eighteen cells; the 10 ms arithmetic chain reads 1.25 from a 2 ms
difference). At eight workers the control refuses every closure-route cell
that submits a batch of five or more candidates, recorded as
`invocation_failure` after the first batch; its formula-route cells, which
do not use the closure pool, pass unchanged. The new executable passes every
cell at eight and at sixteen workers. Native medians, ms, of the new
executable on the closure-route enumeration cells:

| Cell | 4 workers | 8 | 16 | 8/4 | 16/8 |
|---|---:|---:|---:|---:|---:|
| independent-choice-12 | 26.5 | 21.0 | 21.0 | 0.79 | 1.00 |
| independent-choice-16 | 194.3 | 180.0 | 177.0 | 0.93 | 0.98 |
| independent-negation-8 | 106.7 | 73.3 | 74.0 | 0.69 | 1.01 |
| independent-negation-10 | 1385.6 | 955.2 | 889.5 | 0.69 | 0.93 |

Every other cell is a single candidate or runs on the formula route, whose
completion pool the option does not size, and is unchanged across the three
counts. The phase receipts attribute the remainder: on
independent-negation-10 the closure-membership phase takes 1,262, 835 and
763 ms at four, eight and sixteen workers while candidate generation stays
serial at about 109 ms; on independent-choice-16 the output phase, 116 to
126 ms of the 177 to 194, dominates at every count. The step from eight to
sixteen workers coincides with the step from eight cores to their
hyperthreads on this host, so these campaigns do not separate the batch
barrier from the hardware; a per-worker busy-time receipt would.

## Item 3: views as runs, new rows first, and the rule index

Item 3 has four commits and two retained campaigns. The first three commits
were measured at `cf051cf7e1ba7423b2a3f9e39fc695848086ae5d` (SHA-256
`f91b48f2007e372ea64d177c2c77c530b139eea475559f54ea086197441184c0`), on 16
September 2026 between 23:05:14 and 23:12:23 UTC:
[series-cf051cf7-cpu-auto.json](series-cf051cf7-cpu-auto.json)
([tables](series-cf051cf7-cpu-auto-tables.md)), raw report SHA-256
`e38488e9599dcaefd8217c59201ff2c10387a1ed4ed24eda6e596cb2d9c5bc1b` (main) and
`e12c3bbc5285910dd2a057834fdc8e039a1634401169e0c3e99146031a6d4bde` (after).
The whole item was measured at `58709af3bae0a8eee5e6488b96a16a0ba5978f4f`
(SHA-256 `d4d7b01b9c55b53676bcd0e2056ab39e2c1bd5869d330b8db2824a4d9990da89`)
between 23:39:46 and 23:46:54 UTC:
[series-58709af3-cpu-auto.json](series-58709af3-cpu-auto.json)
([tables](series-58709af3-cpu-auto-tables.md)), raw report SHA-256
`475b419895298b4044a9637854acd5086b98d7e04d1637fe34aae5d36a9e8f76` (main) and
`77f3b90d2449ee4cbe891aefa33414fbb34b602b23af2276053c27d092907f70` (after).
Same machine, profile, four workers and clingo as above; `main` is the
896a5f73 executable rerun as the control each time.

In the problem's words: on a program that builds a long chain of
consequences, the solver used to make two complete passes over every grown
relation each time it derived one more fact, then scan an unchanged relation
from end to end, then visit every rule of the program, facts included, and
after the first three commits it still copied the whole ordered view once per
round. It now keeps each relation's order as a stack of sorted runs that it
merges only when two are within a factor of two, starts each step from the
new facts, and visits only the rules that could use them. Native medians,
ms, with clingo on the same cell:

| Cell | before | after | after/before | clingo | after/clingo |
|---|---:|---:|---:|---:|---:|
| chain-1000 | 99.9 | 19.5 | 0.20 | 7.7 | 2.5 |
| chain-2000 | 357.2 | 34.4 | 0.10 | 10.9 | 3.2 |
| transitive-path-100 | 32.5 | 16.0 | 0.49 | 8.9 | 1.8 |
| transitive-path-200 | 183.5 | 46.8 | 0.26 | 22.5 | 2.1 |
| transitive-dense-40 | 39.8 | 39.0 | 0.98 | 6.7 | 5.8 |

The other fourteen passing cells lie between 0.96 and 1.09 of the control:
they derive few consequences per candidate, or run on the formula route,
which this item does not touch. The stratified cell still reaches its
deadline on both executables. Memory did not change: the chain of depth
4,000 holds 33.6 MB resident after against 33.1 MB before.

The growth law of the chain family on the closure route, charged work in
units and wall time in ms, at depths 250, 500, 1,000, 2,000, 8,000 and
16,000:

| Executable | 250 | 500 | 1,000 | 2,000 | 8,000 | 16,000 |
|---|---:|---:|---:|---:|---:|---:|
| control, work | 625,819 | 2,330,526 | 8,945,042 | ceiling at 4,000 | | |
| after three commits, work | 129,417 | 338,654 | 962,114 | 2,994,036 | 36,535,576 | ceiling |
| after four commits, work | 99,136 | 215,091 | 467,474 | 1,002,175 | 4,574,620 | 9,698,552 |
| after four commits, ms | 5.3 | 8.8 | 13.8 | 29.2 | 125.6 | 260.2 |

The control's work grew 3.7 times per doubling and stopped at the default
ceiling of 100 M units at depth 4,000; after the first three commits it grew
3.4 times per doubling, the linear copy per round, and stopped at depth
16,000 after 266 ms of work that would have completed; after the fourth it
grows 2.1 to 2.3 times per doubling, the n log n of the run merges, and depth
16,000 completes in 260 ms with 9.7 M units. Depth 64,000 is refused at
source admission by the syntax-node ceiling of 262,144 nodes, not by the
solver.

## Item 8: one lookup per depth, one order per closure, one merge for the agreement

Two campaigns on 17 September 2026 between 00:46:41 and 00:53:50 UTC, same
machine, profile, four workers and clingo as above. `main` is the 896a5f73
executable rerun as the control; `after` is built from
`87a80cd0584cbeb8fb094d26187fc2994c436176` (SHA-256
`11921136c06e73177d2e1ce021418a25e7d44c594f8f9b8b20b0b0ccedab803b`), which
resolves a body pattern's relation once per join depth and indexes it on
every probe, assembles the model from the relations in predicate order
without sorting, checks the gate agreement by one merge over the gate
predicates' ranges, and borrows the prepared runs without measuring
capacity on every probe.
[series-87a80cd0-cpu-auto.json](series-87a80cd0-cpu-auto.json) and its
[table view](series-87a80cd0-cpu-auto-tables.md) are the derived comparison;
raw report SHA-256
`00b34c428d8f3f4be5a39577bc87a8599600d81148187fec680b5c10a6514d68` (main) and
`9d409190ac877165b8a6514dd67bc0417138dcd7fc5915deb8ed0b03748be44c` (after).

In the problem's words: for every row it examined, the solver looked up the
relation by the predicate's name again; after every candidate it sorted the
whole answer although its relations already held the order; and to compare
the answer with the candidate it searched the gate predicates by name for
every atom of the answer. It now looks a relation up once per join, adopts
the relations' order, and walks the gate atoms and the candidate together.
Native medians, ms, with clingo on the same cell, and the ratio the previous
item's campaign gave for the same cell:

| Cell | before | after | after/before | after item 3 | clingo | after/clingo |
|---|---:|---:|---:|---:|---:|---:|
| transitive-path-100 | 33.2 | 14.4 | 0.44 | 0.49 | 8.1 | 1.8 |
| transitive-path-200 | 189.8 | 42.7 | 0.23 | 0.26 | 22.7 | 1.9 |
| transitive-dense-40 | 43.6 | 38.7 | 0.89 | 0.98 | 6.7 | 5.8 |
| independent-choice-12 | 23.8 | 20.9 | 0.88 | 1.01 | 6.8 | 3.0 |
| independent-negation-10 | 1397.5 | 1319.1 | 0.94 | 1.03 | 5.7 | 231.5 |
| chain-2000 | 360.9 | 35.6 | 0.10 | 0.10 | 11.0 | 3.2 |

This item's own effect is the difference between the two ratio columns:
five to ten percent on the join-heavy and enumeration cells, nothing on the
chains, whose joins are one row wide, and nothing on the formula-route cells.
A profile of the independent-negation cell on this executable attributes
the per-candidate cost that remains: about a fifth to the catalog's
insertion machinery (dictionary planning, index insertion, membership
lookups), a sixth to allocation and freeing, five percent to predicate name
comparison. The last two are the representation question deferred to the
cube search; the first is the cost of building a closure at all, which
fewer candidates, not cheaper ones, address. Memory did not change.

The enumeration cells also show where their series time goes: the
independent-choice cell at size 16 solves and prints its 2,584 models in
30 ms as plain text and in 191 ms under the instrumented JSON record the
matrix asks for, of which the output phase is 120 ms; clingo's `--outf=2`
JSON run takes 28 ms in all. Both solvers emit JSON here; the cost is in
zetesis's writer and is a separate item.

## Item 7: the formula route's witness rescan and the support law

Two campaigns on 17 September 2026 between 01:27:57 and 01:35:06 UTC, same
machine, profile, four workers and clingo as above. `main` is the 896a5f73
executable rerun as the control; `after` is built from
`8b7239fd8e90befdd3a96392356d8f7e8be7682e` (SHA-256
`60e306c71b10bd941cf2bad99e194114a4dc9e7e992e57a1058996d89708d0de`), which
no longer rescans every base clause after each satisfying assignment (the
watch scheme's invariant, asserted in debug builds and stated by the
truth-table tests) and rejects a candidate with an unsupported present atom
under the complete tight certificate by `TightPlans.stable_supported`
instead of sending it to a reduct query.
[series-8b7239fd-cpu-auto.json](series-8b7239fd-cpu-auto.json) and its
[table view](series-8b7239fd-cpu-auto-tables.md) are the derived comparison;
raw report SHA-256
`e81d77d18bf075d1acf1ae978c7c97aa58645d62fdc9dac6201e1be46dcc40e4` (main) and
`f8d8dfc887adfc3578a6eefea20247301748cf3e518d3b2607bfc17c014abc44` (after).

In the problem's words: after every candidate the search found, the solver
read every clause of the problem again to confirm the candidate satisfied
them, which the way it finds candidates already guarantees; and a candidate
that the tight certificate showed to hold an atom nothing produces was sent
to a full search for a counterexample that the support law names directly.
Native medians, ms, with clingo on the same cell, and the charged search
work before and after:

| Cell | before | after | after/before | clingo | after/clingo | work before | work after |
|---|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-01 8→10 | 126.2 | 116.2 | 0.92 | 33.9 | 3.4 | 17.3 M | 11.4 M |
| n-queens/variant-01 8→11 | 457.0 | 432.1 | 0.95 | 187.2 | 2.3 | 85.1 M | 57.2 M |
| n-queens/variant-04 8→11 | 311.5 | 305.0 | 0.98 | 179.3 | 1.7 | 55.5 M | 40.8 M |
| variant-04/05-larger-mix | 347.8 | 297.1 | 0.85 | 163.4 | 1.8 | 44.2 M | 23.2 M |
| ties-50 | 183.6 | 172.5 | 0.94 | 18.4 | 9.4 | 8.1 M | 4.7 M |
| disjunction-12 | 216.2 | 206.8 | 0.96 | 22.1 | 9.4 | 12.3 M | 9.0 M |
| send-money/send-money | 50.0 | 50.9 | 1.02 | 15.3 | 3.3 | 3.6 M | 3.6 M |

The closure-route cells are unchanged, as they must be. The charged work of
the formula cells fell by a third to a half while their time fell by five to
fifteen percent: the rescan was a third of the charged units but a cheap
third, a sequential pass the processor predicts well, so the work receipts
overstated its share of the time. Send-money runs one candidate and has no
rescan to lose. The reduct-query phase is absent on every formula cell
after the change; the certificate's own phase is unchanged, since it did
the same evaluation before and is now believed.

### The branching order under assumptions

The audit's remaining formula-route point asked whether the branching
order, which the search recomputes for every seeded call, could be computed
once per base closure and shared by its assumptions. Two campaigns on 17
September 2026 between 01:43:14 and 01:50:23 UTC compared the 8b7239fd
executable (`after`) with an experimental build (`static`, SHA-256
`8d15f5829ec4a3724001ddb453b9cf1125203daacec8715c13cfc31f1371bf36`) that
computes the order before the assumptions are assigned, so the same order
serves every seed:
[series-8b7239fd-cpu-auto-ordering.json](series-8b7239fd-cpu-auto-ordering.json)
with its [table view](series-8b7239fd-cpu-auto-ordering-tables.md); raw
report SHA-256
`51bb1a80dd6a60bf587efa65264a668d63b194070903afd8e9758ee728ac3cc8` (after)
and `a991848fc00110f7642bdf4eee5ab88a5dea47fd26ed4244815e23923038065e`
(static). The experimental build is not retained.

The order depends on the assumptions by construction: the ordering heuristic
reads the current values, so a seed that fixes a literal changes which
clauses still need a decision. Sharing the order removed that dependence,
and the charged search work was identical on eighteen of the nineteen
measured cells and 4.1 times larger on disjunction-12 (37.0 M against
9.0 M units; 282 against 206 ms). The seeds are the reduct parameters of
each candidate: the order computed after they are assigned ranks the
clauses they leave open, while the shared order ranks the unfrozen problem
and decides in an order the frozen one no longer rewards. The order stays
per seed. The decision record: the once-per-closure order
is cheaper to compute but not cheaper to search with; it would be
reconsidered only with an ordering heuristic that does not read the current
values.

## Item 2: admission charged at its cost

Two campaigns on 17 September 2026 between 02:08:09 and 02:15:24 UTC, same
machine, profile, four workers and clingo as above. `main` is the 896a5f73
executable rerun as the control; `after` is built from
`2e9abd7a` (SHA-256
`a5b030e44d841e945c6e549094cd83cb91772365a35e37bd4ef566d160f6c892`), where
the producer plan charges each dependency validation one logarithmic lookup
instead of a comparison against every predicate, the head allowance is
charged once per new atom after the membership test, a transient binding
frame is not charged to the cumulative byte allowance, that allowance is the
`--max-expansion-bytes` option, and the four small formula ceilings follow
the atom ceiling's scale.
[series-2e9abd7a-cpu-auto.json](series-2e9abd7a-cpu-auto.json) and its
[table view](series-2e9abd7a-cpu-auto-tables.md) are the derived comparison;
raw report SHA-256
`75d1c1b86228f8c436810bbebb944a7fbaead0428cda2c7f1cd667efeb536acd` (main) and
`93902d9519912cca182023343cbd2d6952cb0a7b7eb3929b6ae45af4c3650666` (after).

The quantity this step changes is reach, not speed: which programs the
formula route admits at its defaults. The series cells were all admitted
before, and their native medians moved within the run-to-run band of the
earlier steps (the formula cells between 0.89 and 0.97 of the control, the
closure cells carrying the cumulative gains of items 3 and 8). The audit's
four reproductions, each run once per executable with default options
except where a flag is named, and clingo on the same file:

| Program | main | after | clingo | answer |
|---|---|---:|---:|---|
| 700 facts and 699 two-literal rules under `#project` | refused: work 10,004,588 of 10,000,000 | 47 ms | 11 ms | 1,399 atoms, equal |
| 400-node arithmetic path closure, `--max-expansion-work 100000000` | refused: scalar bytes 16,777,220 of 16,777,216 | 609 ms | 66 ms | 80,199 atoms, equal |
| `p(0). p(X+1) :- p(X), X < 1500.` | refused: support rounds 1,025 of 1,024 | 13 ms | 6 ms | 1,501 atoms, equal |
| `d(1..1100). p(X) :- d(X), X < 3.` | refused: domain values 1,025 of 1,024 | 15 ms | 5 ms | 1,102 atoms, equal |

In the problem's words: before, a program could be refused for the shape of
an allowance rather than for its size. Preparing the producer plan for a
rule cost a comparison against every predicate in the program, so seven
hundred small rules exhausted the work ceiling before a single instance was
grounded; every proposal of a head atom was charged three times its bytes
whether or not the atom was new, and every binding frame was charged to a
budget that never released it, so the transitive closure of a four-hundred
node path ran out of a sixteen-mebibyte allowance while occupying a fraction
of it; and four ceilings of 1,024 refused a chain of 1,500 numbers or a
domain of 1,100 values. Each charge now counts the operation or the retained
payload it names, and the allowance that was hidden is an option and is
reported with the others. The programs answer, and their answers agree with
clingo; the closure-route cells are unaffected because none of these charges
is on that route.

## Item 6, first step: the join criterion

Three campaigns on 17 September 2026 between 02:40:06 and 02:53:20 UTC,
same machine, profile, four workers and clingo as above. `main` is the
896a5f73 executable rerun as the control, `before` the item 2 executable
(`2e9abd7a`, SHA-256
`a5b030e44d841e945c6e549094cd83cb91772365a35e37bd4ef566d160f6c892`) and
`after` is built from `25f6b6e6` (SHA-256
`37486be255e47a444e0a4761918a5817fd4e44733da3b63d668dc5250790eea4`), where
a positive body is joined in an order chosen by a named criterion: tests
before generators, fewer offered rows first (a semi-naive pivot's new rows
count as its rows), then the occurrence that decides the most waiting
comparisons, then the one that binds the most variables they wait on, then
the canonical order.
[series-25f6b6e6-cpu-auto.json](series-25f6b6e6-cpu-auto.json) and its
[table view](series-25f6b6e6-cpu-auto-tables.md) are the derived comparison;
raw report SHA-256
`d2f474774f5d3c6bd0f915f1c9cb89fbb8ac104118903845fef5c92aff57eb93` (main),
`62098526db0d98a799aa3348c546256a83f6ecf2a0caa2f527266a13b4fdea3f` (before)
and `0d4787f75be96aad00a6ec909ff0ebda21670c8070314fb8544c938149b15bce`
(after).

No series cell has a body whose order the criterion changes with effect, and
the after/before column is within 0.98 and 1.10 on every cell, the widest
being the ten-millisecond arithmetic chain. The audit's reproduction pair,
`p(X,Y,Z) :- d(X), d(Y), X < Y, d(Z).` and the same rule with `W` for `Z`
over `d(1..40)`, run three times per executable with
`--max-expansion-work 100000000`, median driver wall time and the rows the
support-completion join reads (the count the
[join-order tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/join_order.rs)
pin):

| Program | main | before | after | clingo | support rows before | support rows after |
|---|---:|---:|---:|---:|---:|---:|
| third variable `Z` | 237.2 ms | 235.0 ms | 240.8 ms | 25 ms | 32,840 | 32,840 |
| third variable `W` | 234.6 ms | 236.4 ms | 234.7 ms | 26 ms | 65,640 | 32,840 |

In the problem's words: before, the body was joined in the order the
canonical program lists its literals, which sorts them by predicate and then
by variable name, so calling the third variable `W` instead of `Z` moved it
in front of the comparison and doubled the rows the possible-support join
read. The order is now chosen by what the body says, and both spellings
read the same rows. The time did not move, on either spelling: the
possible-support join is a small part of grounding this rule, and the rule
instantiation that follows reads the full product of 64,000 rows on every
build, because a complete row is validated before it is rejected so that a
reached undefined operation is a refusal rather than a dropped row. Where
that contract stands is the language's decision, recorded in the reference;
the criterion decides how early a comparison is bound, not what a false one
may skip. The answers agree with clingo on both programs.

## The carrier bounds: the seed counter between the program's two closures

Three campaigns on 17 September 2026 between 04:17:00 and 04:29:50 UTC,
same machine, profile, four workers and clingo as above. `main` is the
896a5f73 executable rerun as the control, `before` the item 6 executable
(`25f6b6e6`, SHA-256
`37486be255e47a444e0a4761918a5817fd4e44733da3b63d668dc5250790eea4`) and
`after` is built from `14000c8b` (SHA-256
`56b07e3014eb6fc7e6c1777b3fc7388200a59d3d38857d6d398459afd3614011`), where
the closure route computes the program's closure twice before its first
seed, once with every gate treated as possible and once with none, and the
counter runs over the gate atoms between the two: an atom outside the first
is never offered, an atom inside the second is held in every seed
(`Bounds.undecided_bounds_accepted`).
[series-14000c8b-cpu-auto.json](series-14000c8b-cpu-auto.json) and its
[table view](series-14000c8b-cpu-auto-tables.md) are the derived comparison;
raw report SHA-256
`75d17ea86e441dd9d71ebb9f61e80c7d838838afb43ea560f2627a195c67be82` (main),
`e0d808ef1c0feed9dd6413473f17b1f0978612cf4324d090b4dc395f959a48f8` (before)
and `ca180dc4a91b748c460091bb071eba1cfcf82c24d3adddae16a31b12ceb2c94b`
(after).

The cell this step changes is stratified-16, which timed out at thirty
seconds on every earlier build:

| Cell | before | after | clingo | seeds before | seeds after |
|---|---:|---:|---:|---:|---:|
| stratified-16 | timeout (30 s) | 750 ms | 4.5 ms | 2³² | 2¹⁵ = 32,768 |

In the problem's words: the program has two gate predicates, `blocked`
and `reach`, and the counter proposed every combination of their sixteen
atoms each, four thousand million seeds. Only two `blocked` atoms can be
derived at all, and the fact `reach(1)` and those two are derived by
rules with no gate, so every answer set holds them: fourteen atoms leave
the carrier and three are held, and the remaining fifteen give the
32,768 seeds the run checks. Every other cell is within the run-to-run
band, after a first build of this step had cost the gate-free chain and
transitive cells two closures for nothing; a program without gate
predicates now computes none. The counter still enumerates 2¹⁵ seeds for
a program clingo grounds without negation: the fifteen `reach` atoms
wait on gates that the first narrowing has decided, and deciding them
needs the closures recomputed with those gates read as decided, which is
the second narrowing pass and the subject of the next step.

## The iterated narrowing: deciding the region before the first seed

Three campaigns on 17 September 2026 between 04:51:39 and 05:03:56 UTC,
same machine, profile, four workers and clingo as above. `main` is the
896a5f73 executable rerun as the control, `before` the carrier-bounds
executable (`14000c8b`, SHA-256
`56b07e3014eb6fc7e6c1777b3fc7388200a59d3d38857d6d398459afd3614011`) and
`after` is built from `ccdd5ac9` (SHA-256
`80f014302c1e55949c203e7389a311e4a63c3413c29d47dc221ab749387ffbb6`), where
the counter narrows its region pass by pass, each pass reading the
decisions of the last, until one changes nothing, and offers no seed when
a constraint fires in a lower closure (`Bounds.narrowed_contains_accepted`,
`Bounds.lower_constraint_refutes`).
[series-ccdd5ac9-cpu-auto.json](series-ccdd5ac9-cpu-auto.json) and its
[table view](series-ccdd5ac9-cpu-auto-tables.md) are the derived comparison;
raw report SHA-256
`da4d2140a13e53d57ab10a6b4962377ec01502d94780540cba556085d7f58f65` (main),
`46d1a2c4b8542527c46032aa0bc39ae3f33e00cf8f1a52c0f2779cc7424f71d5` (before)
and `abd9758e49541a3b3dce846e548b9bf93398552ebc6c8589d703e7b17e1db52e`
(after).

The cell this step changes is again stratified-16; every other cell is
within the run-to-run band. The stratified family was then run once per
size and executable outside the harness, with a sixty-second limit, four
workers and all answers requested, and clingo on the same files; the
answer at every size has the same atoms in both solvers (1,312 at 256):

| size | main | carrier bounds | narrowing | clingo | seeds after |
|---:|---:|---:|---:|---:|---:|
| 16 | > 60 s | 765 ms (32,768 seeds) | 7 ms | 6 ms | 1 |
| 32 | > 60 s | > 60 s | 7 ms | 7 ms | 1 |
| 64 | > 60 s | > 60 s | 9 ms | 6 ms | 1 |
| 128 | > 60 s | > 60 s | 16 ms | 6 ms | 1 |
| 256 | > 60 s | > 60 s | 26 ms | 8 ms | 1 |

In the series, stratified-16 went from 749 ms with 32,768 seeds to
4.5 ms with one, against clingo's 4.4 ms.

In the problem's words: the first pass decided which `blocked` atoms can
exist at all, but the `reach` rules wait on those very atoms, and a single
pass read them as still open. Reading the decisions of one pass in the
next, the second pass derives every reachable node as necessary and every
unreachable one as impossible, and the third confirms that nothing moves.
Three passes, each two closures, decide the program's whole gate carrier,
which is what the well-founded model does for a stratified program; the
counter then checks one seed. A program in which a constraint can never be
satisfied, such as one demanding a vertex no edge reaches, is refuted in
the first pass and no seed is checked at all. The `in/out` families are
unchanged, as they must be: nothing in their rules is decided before a
choice is made, and that is the case for the rest of the cube search.

## The grounding core: the node index, decided comparisons, integer evaluation

Three campaigns on 17 September 2026 between 11:24:11 and 11:36:27 UTC,
same machine, profile, four workers and clingo as above. `main` is the
896a5f73 executable rerun as the control, `before` the iterated-narrowing
executable (`ccdd5ac9`, SHA-256
`80f014302c1e55949c203e7389a311e4a63c3413c29d47dc221ab749387ffbb6`) and
`after` is built from `e5828b84` (SHA-256
`629b682de860f6e1cb19a17ea58444c351ec25c9ffc8b52f1cf04105e4d7a625`), three
commits from a profile of the `before` executable on send-money: the formula
node index is placed by a fixed multiplicative hash instead of the standard
library's randomized one (`77e7d11a`); each comparison is evaluated once, at
the depth whose occurrence binds its last variable, and deeper prefixes
inherit the verdict (`7d98f5f5`); and a plan over numbers runs in integer
cells without constructing a value per node (`e5828b84`).
[series-e5828b84-cpu-auto.json](series-e5828b84-cpu-auto.json) and its
[table view](series-e5828b84-cpu-auto-tables.md) are the derived comparison,
the first to carry each report's median over the reference solver's; raw
report SHA-256
`a8452092e4b6478537b976b1996d6e51d6beebe46006a89973b6ef58499238fa` (main),
`6dedbae2e98fb0e7eb953e5b8035892496d8eb93640e0907e338f16ae158a1eb` (before)
and `9cb5f2db4c152241822c12d55093f3fce453370f9972d182b396de92559d421d`
(after).

The profile that chose these three, taken on `before` with `perf` at 4,999
Hz over the whole send-money run with all answers requested: the search's
unit propagation took 21 percent of the samples; the formula node index,
its randomized hashing, rehashing and interning, 16 percent; the atom
catalog's ordered search with its predicate-name comparison, 21 percent;
expression evaluation 7 percent; binding-frame and value copies 6 percent.
Native medians, ms, with clingo on the same cell:

| Cell | before | after | after/before | clingo | after/clingo |
|---|---:|---:|---:|---:|---:|
| send-money/send-money | 52.7 | 42.9 | 0.81 | 14.4 | 3.0 |
| n-queens/variant-01 8→10 | 116.1 | 109.1 | 0.94 | 31.3 | 3.5 |
| n-queens/variant-01 8→11 | 429.1 | 426.6 | 0.99 | 183.9 | 2.3 |
| ties-50 | 182.5 | 170.8 | 0.94 | 18.3 | 9.3 |
| transitive-dense-40 | 41.3 | 34.5 | 0.84 | 7.0 | 5.0 |
| independent-choice-16 | 202.2 | 190.6 | 0.94 | 27.7 | 6.9 |
| variant-04/05-larger-mix | 310.8 | 307.0 | 0.99 | 150.2 | 2.0 |

In the problem's words: the grounder built each column test of send-money
by hashing the identity of every formula node it created with a hash meant
for keys an author could choose, walked every comparison's expression at
every join depth to ask whether its variables were bound yet, evaluated
every bound comparison again at every deeper depth, and built a value for
every number it added or compared. The node identities are numbers the
builder itself assigns, the depth at which a comparison is decided is fixed
by the join order, and a number needs no value. On send-money, whose
grounding is most of its time, the run is a fifth shorter; on queens the
gain is a few percent, since its comparisons are decided only at the last
depth and its time is in the search. The closure-route cells are unchanged
within their band. After these three, the same profile on `after` puts the
search at a quarter of the samples and the atom catalog's identity
comparison, which compares predicate names byte by byte at every visited
node, at a fifth: that is the next grounding cost, and it is a
representation decision recorded in the audit as R4.

## What the views preserve

These are derived observation views, not byte-identical archives of the original
reports. Each retains all 468 ordered observation receipts across nine workloads
and four blocks, including qualification, warmup, timing, diagnostics and memory
positions. The provenance records original report hashes and byte lengths,
source and executable identities, corpus files and the actual observation
limits. The Table-grounding provenance also records its build recipe and
platform; the atom-catalog provenance states the external source/binary binding
and its observation interval without reconstructing compiler lineage.
The prepared-grounding provenance also retains the shared acquisition and build
recipe limits described above. The original reports remain retained separately.
Machine-local paths, process IDs, output streams and diagnostic payloads are
omitted. These files do not publish the other Metal, LTO, shared-lazy or matrix
comparisons.

`decision: "pass"` records the original comparison's conclusion: complete
selected displays, including symbol and model multiplicities, final costs and
every optimum tie, matched the pinned corpus contract and clingo. The retained
selected counts and costs do not independently prove that conclusion. Native
human output and clingo JSON have different encoding costs; hidden full
interpretations were unavailable to this ordinary comparison. Rendering the
tables does not rerun the solvers or re-establish output parity.

Each producer/case/block has exactly three timed nanosecond observations and
one separate child-RSS observation in bytes. Other elapsed values remain in
their own populations. The table uses the middle sorted time, minimum and
maximum; it never pools blocks. RSS comes from a fresh resource helper, excludes
that helper, and can include usage propagated by waited descendants. It is not
simultaneous process-tree RSS or device memory. For memory positions the capture
exit belongs to the helper, while `memory.exit_code` belongs to the solver;
both are retained. The small populations provide neither confidence intervals
nor a general speedup claim.

Run the maintained Rust view from the repository root:

```sh
cargo run --locked -p zetesis-validation --example release_observations
cargo run --locked -p zetesis-validation --example release_observations -- --check
cargo run --locked -p zetesis-validation --example release_observations -- --dataset atom-catalog
cargo run --locked -p zetesis-validation --example release_observations -- --dataset atom-catalog --check
cargo run --locked -p zetesis-validation --example release_observations -- --dataset release-ca10a5e7-679ca856
cargo run --locked -p zetesis-validation --example release_observations -- --dataset release-ca10a5e7-679ca856 --check
cargo run --locked -p zetesis-validation --example release_observations -- --dataset release-f56a5a24-679ca856
cargo run --locked -p zetesis-validation --example release_observations -- --dataset release-f56a5a24-679ca856 --check
```

The default remains the historical Table-grounding comparison; explicit
`--dataset table-grounding` selects the same data. `--dataset atom-catalog`
selects the atom-catalog dataset. The two source-pair names select the corresponding
prepared-grounding views. Each prints its three published Markdown
tables; `--check` validates without writing them. Unknown names, repeated
options and arbitrary file paths are refused. All use only embedded data and
one shared validator/renderer: they check the linked provenance digest, exact
selected source/binary bindings and join policies, all positions
against `zetesis_validation::performance::Schedule`, recorded completion and
cost consistency, and the separate RSS receipts. Missing, extra, duplicate or
reordered positions refuse table generation. Integer formatting rounds to
three decimal places with ties to even, without converting raw units through
floating point. The example also requires equality with the retained published
table view; its regression checks that view against the manual.

For the historical Table-grounding comparison, direct integer rounding corrects
eight final displayed digits from its earlier presentation. For example, 32,871,500 ns is exactly
32.8715 ms and rounds to 32.872; 6,094,848 bytes is exactly 5.8125 MiB and rounds
to 5.812. The raw observation and provenance JSON remain unchanged.
