# Recorded release observations

The current [0.1.4 coverage receipt](coverage-9bb73da9.json) identifies the
qualified source, independent CPU and workspace populations, and 59 physical
Metal tests. Three separate ordinary CLI backend checks also passed; they do
not contribute instrumented coverage. See the [validation reference](../validation.md#coverage)
for the scope. The performance records below retain their original identities.

Four fixed datasets reproduce the ordinary release comparisons in the
[performance reference](../performance.md). Each retains its own sources,
executable identities, observations and completion limits. None qualifies a
later implementation.

The latest [shared-plan execution comparison](../plan-execution.md) compares
`eca5a1a7` with `2e80d065` on 20 September 2026. It retains unchanged maintained
views for [CPU automatic grounding](series-2e80d065-cpu-auto.json),
[CPU eager grounding](series-2e80d065-cpu-eager.json) and
[Metal eager grounding](series-2e80d065-metal-eager.json), with their rendered
tables linked in the chapter. Its [provenance](series-2e80d065-provenance.json)
identifies all twelve campaigns; the [device receipt](series-2e80d065-metal-device.json)
separates actual execution routes, phase measurements, process RSS and accounted
device storage. The independent [coverage receipt](coverage-6754a4ff.json)
identifies the later qualification source, which changes one test and two manual
pages without changing the measured implementation.

The preceding [CPU and Metal execution series](../execution-series.md) compares
`994fbb79` with `eca5a1a7` on 20 September 2026. Its unchanged maintained views
are [CPU data](series-eca5a1a7-cpu-auto.json) / [tables](series-eca5a1a7-cpu-auto-tables.md)
and [Metal data](series-eca5a1a7-metal-eager.json) / [tables](series-eca5a1a7-metal-eager-tables.md).
The [provenance](series-eca5a1a7-provenance.json) retains ordered acquisitions,
workload identities, limits, source and executable identities, and all eight raw
report hashes. These derived series views are separate from the four fixed
release datasets and their renderer. The independent
[coverage receipt](coverage-eca5a1a7.json) identifies the qualified population.

Descriptions of arithmetic refusals below apply to those measured revisions.
The current [source-family policy](../language.md#numeric-boundaries-and-refusal-meaning)
admits mixed defined/zero-divisor families with warnings while retaining
all-undefined and fatal-error refusals; the historical timings do not measure
that policy's implementation.

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
comparison in the [performance reference](../grounding-measurements.md#canonical-release-comparison).
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
workloads have mixed results. The [earlier atom-catalog comparison](../grounding-measurements.md#earlier-atom-catalog-cpu-comparison)
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

## The atom catalog indexed by predicate

Three campaigns on 17 September 2026 between 11:57:13 and 12:09:25 UTC,
same machine, profile, four workers and clingo as above. `main` is the
896a5f73 executable rerun as the control, `before` the grounding-core
executable (`e5828b84`, SHA-256
`629b682de860f6e1cb19a17ea58444c351ec25c9ffc8b52f1cf04105e4d7a625`) and
`after` is built from `20b4f778` (SHA-256
`a6974b914cffffb7dff572279b2d2e7900ebb1e20ec27c5f873e2d0fe3833ba4`), where
the formula grounder's atom catalog keeps one ordered tree per predicate,
finds the predicate's tree by a checked binary search over the program's
relations and compares arguments only along it.
[series-20b4f778-cpu-auto.json](series-20b4f778-cpu-auto.json) and its
[table view](series-20b4f778-cpu-auto-tables.md) are the derived comparison;
raw report SHA-256
`4bf01d06b601c53acee02c7e6183a248f8e43780f2f73e536bdd1396e18176ab` (main),
`6ce06b982a4a0e8ad5b5cde1998ad64a7ffb1186e067790e9859a7640453d9f1` (before)
and `4f34782eea9a26b9639f7b2bb4f528620705413f69c358d2bbcecae57493403b`
(after).

The catalog is the formula route's, so the closure-route cells are
unchanged by construction and their movement in this campaign (chain-1000
1.17, transitive-path-100 1.06, independent-choice-12 1.09, all under thirty
milliseconds) is the width of the run-to-run band for small cells. On the
formula cells:

| Cell | before | after | after/before | clingo |
|---|---:|---:|---:|---:|
| ties-50 | 178.4 | 162.9 | 0.91 | 18.7 |
| independent-negation-aggregate-16 | 346.8 | 317.3 | 0.92 | 30.6 |
| disjunction-12 | 211.7 | 202.1 | 0.95 | 22.2 |
| variant-04/05-larger-mix | 308.4 | 298.7 | 0.97 | 150.4 |
| send-money/send-money | 43.5 | 42.7 | 0.98 | 14.1 |
| n-queens/variant-01 8→10 | 111.2 | 113.4 | 1.02 | 31.5 |
| n-queens/variant-01 8→11 | 418.1 | 436.8 | 1.05 | 183.7 |

In the problem's words: every lookup of a ground atom walked a tree over
all atoms, and at each of its levels compared the predicate's name byte by
byte before looking at the arguments. Now it finds the predicate's own tree
once, by a search over the handful of predicates the program names, and
compares only arguments on the way down. The profile of send-money shows
the predicate comparison falling from eight percent of the samples to
three and a half; the wall time moved by two to nine percent on the
formula cells except queens, whose two readings differ in the other
direction by two to four percent, within the band of a cell whose time is
in the search rather than the grounder. Atoms are still self-contained
values with their own predicate; the dense predicate identity the audit's
R4 also describes remains a separate representation decision.

## The exclusion rule: a false comparison excludes the substitution

Three campaigns on 17 September 2026, the reports written between 17:16:41
and 17:27:40 UTC, same machine, profile, four workers and clingo as above.
`main` is the 896a5f73 executable rerun as the control, `before` the
per-predicate catalog executable (`20b4f778`, SHA-256
`a6974b914cffffb7dff572279b2d2e7900ebb1e20ec27c5f873e2d0fe3833ba4`) and
`after` is built from `852c9598` (SHA-256
`182838f21d893684421981f80088530b842472c325e113a29c8df3464e47a2d2`), where
a comparison between terms over relationally bound variables that is defined
and false excludes the substitution, so nothing in it is reached and the
join prunes the prefix in every grounding pass.
[series-852c9598-cpu-auto.json](series-852c9598-cpu-auto.json) and its
[table view](series-852c9598-cpu-auto-tables.md) are the derived comparison;
raw report SHA-256
`d566c8ad1ebfe9f48fef6198894977a5f14c03660b1ec1534047d698720f7792` (main),
`bd250e2703dab42d0457dbbd59871d365251d80e87e8b9f521163215a806c258` (before)
and `b990497dd77a4ffe525a956db50ba96dded8589b43605fb9e0041f96683bb523`
(after).

The rule changes what rule instantiation reads: the substitutions the
comparisons leave, rather than the full product validated row by row. On
the formula cells:

| Cell | before | after | after/before | clingo |
|---|---:|---:|---:|---:|
| n-queens/variant-01 8→10 | 110.9 | 91.6 | 0.83 | 30.0 |
| n-queens/variant-01 8→11 | 428.2 | 397.0 | 0.93 | 178.8 |
| send-money/send-money | 42.2 | 39.5 | 0.94 | 14.4 |
| n-queens/variant-04 8→11 | 303.8 | 299.5 | 0.99 | 175.6 |
| variant-04/05-larger-mix | 306.1 | 310.8 | 1.02 | 146.3 |
| disjunction-12 | 207.1 | 212.6 | 1.03 | 22.3 |
| independent-negation-aggregate-16 | 342.8 | 351.7 | 1.03 | 31.3 |
| ties-50 | 169.2 | 181.2 | 1.07 | 18.5 |

The closure-route cells share no code with the change and move within
their bands (chain-1000 0.87, transitive-path-200 0.93, both under fifty
milliseconds). The audit's pair of forty-element rules with `X < Y`, timed
three times each outside the harness, answers in 0.11 s on both executables
at about 35 MB: the instantiation rows fell from 64,000 to 32,840 while the
support rows stayed at 32,840, and at this size the process is the
startup and the support completion.

In the problem's words: a rule is grounded over candidate substitutions for
its variables, and `X < Y` false at a pair is now the end of that pair.
Before, the grounder kept reading the rest of the rule for every pair, to
validate its arithmetic on a substitution it would then discard; now the
pair is excluded where the comparison is decided and nothing beneath it is
reached, in every grounding pass. The reference example `d(0..2). p(X) :-
d(X), X != 0, 1/X = 1.` answers with `p(1)`, where every earlier build
refused it because `1/0` was reached on the excluded substitution. The
departure from clingo stays where it was: a reached undefined or
overflowing operation on a substitution nothing excludes is still a
refusal, where clingo drops the instance. Queens, whose ordering
comparisons decide most pairs, gains eight to seventeen percent; the cells
with few comparisons move within their bands.

## Candidate narrowing: the comparisons over one variable, decided before any row

Three campaigns on 17 September 2026, the reports written between 17:49:46
and 18:00:45 UTC, same machine, profile, four workers and clingo as above.
`main` is the 896a5f73 executable rerun as the control, `before` the
exclusion-rule executable (`852c9598`, SHA-256
`182838f21d893684421981f80088530b842472c325e113a29c8df3464e47a2d2`) and
`after` is built from `8710ad31` (SHA-256
`b104d7c9fd90a47f2ae4acf7f3dba20379f32b584497f17b1cf3b248be8dbf44`), where
the domain guards narrow a variable's candidates by every comparison over
it alone, the analysis's positive profile admits body comparisons, and the
ordinary command runs the analysis by default.
[series-8710ad31-cpu-auto.json](series-8710ad31-cpu-auto.json) and its
[table view](series-8710ad31-cpu-auto-tables.md) are the derived comparison;
raw report SHA-256
`b10117abf9b91a9ef315f398f66075282befd42b1899abaa7926c68b9cb74bc2` (main),
`9fedf1d4f84d9083f5c72331e2fc94e03a76e48b2735c229d64c54f54acefbe1` (before)
and `dba2176baa84890b63fd4bfa5f4a99e236f3d02b082ef812bba6257749357830`
(after).

The series is flat, as expected: every comparison in these cells reads two
variables, which this step does not narrow, so the change measured here
is the analysis running by default on every formula cell. No cell refused
and none left its band: n-queens/variant-01 8→10 0.91, n-queens/variant-04
8→11 0.96, send-money 0.99, larger-mix 0.99, ties-50 1.02, disjunction-12
1.03; the closure-route cells, which the analysis does not touch, moved
0.82 to 1.01 under fifty milliseconds. The audit pair is unchanged at
0.11 s and about 35 MB.

In the problem's words: the exclusion rule decides a comparison over one
variable on that variable's value alone, so the decision can be taken once
per value instead of once per row. The guards take it against the inferred
domain of the variable's argument before the relation is read: on `d(0..2).
p(X) :- d(X), X != 0, 1/X = 1.` the analysis excludes the candidate `0`,
the guard rejects its row before binding, and the receipt reads two
narrowed candidates and two rejected rows for the one answer `p(1)`. On
`d(1..200). p(X) :- d(X), X < 3.` it rejects 198 of the 200 rows before
copying a binding. A value the comparison cannot evaluate is kept, so a
reached undefined operation refuses exactly as before. What this buys in
time depends on how many rows a comparison over one variable excludes and
how often that relation is offered; the series has no such cell, and the
step is recorded for its counted effect and for the analysis now being on.

## Completion narrowing: the same candidates in every round

Three campaigns on 17 September 2026, the reports written between 20:51:48
and 21:02:40 UTC, same machine, profile, four workers and clingo as above.
The machine suspended after the control's matrix had completed and resumed
at 20:46 UTC while its report was being written; the timed intervals are
taken inside each run and none spans the suspension. `main` is the
896a5f73 executable rerun as the control, `before` the candidate-narrowing
executable (`8710ad31`, SHA-256
`b104d7c9fd90a47f2ae4acf7f3dba20379f32b584497f17b1cf3b248be8dbf44`) and
`after` is built from `329b12cf` (SHA-256
`efd92b5329af30944c4f500a3010cec63772d8b6110d18fce72240a8358969c4`), where
the narrowed candidates are prepared once per rule with the analysis and
resolved into every support-completion snapshot as into the final one.
[series-329b12cf-cpu-auto.json](series-329b12cf-cpu-auto.json) and its
[table view](series-329b12cf-cpu-auto-tables.md) are the derived comparison;
raw report SHA-256
`493b6021d8c2b916b61fc0ae33b95d5a119e1b681fdbfb03f7bfee632e3fcbf0` (main),
`745bed5af2830f4bf0b2b4c5e9114d761d65a65fdaad7c804c1312c10759faa2` (before)
and `109d8616a8f1e1ba86ff84fc8d1759bf37a967604904f8990163392f0089ba1c`
(after).

The series is flat, as expected, since no cell carries a comparison over
one variable: the formula cells read 0.96 to 1.07 with ties-50 at 1.12
against a low `before` reading (160.7 ms; its readings across the last
four campaigns are 163 to 181 ms), and the closure-route cells, untouched
by the change, 0.88 to 1.02. Two shapes built for the step were timed
three times each outside the harness, on the `before` and `after`
executables:

| Shape | before | after | completion evaluations before | after |
|---|---:|---:|---:|---:|
| `d(1..2000). p(X) :- d(X), X < 3.` | 0.02 s | 0.02 s | 4,000 | 4,000 |
| `d(1..2000). e(1..250). p(X,Y) :- d(X), e(Y), Y < 3.` | 0.04 s | 0.04 s | 500 | 4 |

In the problem's words: the first shape's domain has 2,000 values, past
the analysis's 256-value widening, so nothing is narrowed and nothing
changes. On the second the join criterion already places `e(Y)` first,
because it decides `Y < 3`, so completion read 4,250 rows before this step
and reads 4,250 now; what the step removes is the evaluation of the
comparison on the 250 rows of `e` (500 evaluations, now 4) and the 248
binding copies the guard rejects first, a cost too small to time at this
size. The step's value is structural: the candidates are a property of
the rule and the analysis, computed once, and every completion round now
reads the rows the final instantiation reads, so a rule's comparison over
one variable is decided once per candidate value in the whole grounding
rather than once per offered row in every round.

## Keyed constraints: the one atom the key admits

Three campaigns on 17 September 2026, the reports written between 21:45:21
and 21:56:14 UTC, same machine, profile, four workers and clingo as above.
`main` is the 896a5f73 executable rerun as the control, `before` the
completion-narrowing executable (`329b12cf`, SHA-256
`efd92b5329af30944c4f500a3010cec63772d8b6110d18fce72240a8358969c4`) and
`after` is built from `808530bf` (SHA-256
`30849015c6feb2f8b8df4565aa99c0e601cdc1ecd9bfa9347c8e11a999b58c2d`), where
a constraint over a keyed value is asked as the one atom its key admits.
[series-808530bf-cpu-auto.json](series-808530bf-cpu-auto.json) and its
[table view](series-808530bf-cpu-auto-tables.md) are the derived comparison;
raw report SHA-256
`4fc2bd2f13512d3c03a491fcdd8410cd1f51bb45693a214d3c52c0acd827655b` (main),
`9e3534f1997504d5f8fe35c04fd9e37f78588930fd6a58f1feb42cbd87fed1a6` (before)
and `e081330c3fd1139374a674e3c94f1e35ea9f63c31d5ffc1871bb6e9c4053bf1b`
(after).

One cell has the form the rewrite reads, and it is the cell it was built
for:

| Cell | before | after | after/before | clingo |
|---|---:|---:|---:|---:|
| send-money/send-money | 40.4 | 10.0 | 0.25 | 14.1 |
| variant-04/05-larger-mix | 307.4 | 277.7 | 0.90 | 146.4 |
| n-queens/variant-01 8→10 | 99.6 | 91.3 | 0.92 | 30.7 |

Every other cell moved within its band (0.96 to 1.06; the closure-route
cells 0.87 to 1.03 under fifty milliseconds). Timed three times each
outside the harness, the send-money process falls from 0.03–0.04 s at
about 19 MB to under 0.01 s at about 12 MB; its formula summary reads
`120 atoms, 6963 nodes, 1927 roots; 5 constraints asked by key` where it
read `112 atoms, 38656 nodes, 13836 roots`.

In the problem's words: each column of SEND + MORE = MONEY was written as
"the digits and the carry must not fail the sum", a test over every
combination of a letter's ten digits and a carry's two values, of which
the grounder formed eighteen hundred forbidden combinations per column.
Each letter has exactly one digit and each column exactly one carry, so
the same constraint says "the digit of this letter must be the sum's last
digit, and the carry must be its tens": one hundred instances per
column, each asking for one atom. The program reads the same, the answer
is the same, and the cell is now faster than the reference, which grounds
the written form. The rewrite is per rule, needs the exactly-one reading
of the choice rules, the digit and carry domains from the analysis, and
the one-solution argument written in the module, and touches no other
cell in the corpus.

## The region split: the narrowed root visited region by region

Three campaigns on 17 September 2026, the reports written between 22:25:21
and 22:36:11 UTC, same machine, profile, four workers and clingo as above.
`main` is the 896a5f73 executable rerun as the control, `before` the
keyed-constraint executable (`808530bf`, SHA-256
`30849015c6feb2f8b8df4565aa99c0e601cdc1ecd9bfa9347c8e11a999b58c2d`) and
`after` is built from `fcf8cad0` (SHA-256
`004e153e8db2ca8366a316aa882e018db2c9e9cff330dd684b1267ba90de0265`), where
the closure route visits the narrowed root's regions instead of counting
its seeds. [series-fcf8cad0-cpu-auto.json](series-fcf8cad0-cpu-auto.json)
and its [table view](series-fcf8cad0-cpu-auto-tables.md) are the derived
comparison; raw report SHA-256
`bb02bc67934191f93e5fc321435d4a50d9d646e3f756e9d21182648a5ab962c2` (main),
`f06c5d41dbfa4892bea795d412ee3a16c3deba9a4ff2a0e2643eb17e3fce48bf` (before)
and `1871d6730f9d7252800a5ca04b730982a4209fb4a8e2d273071d5ed0b808f338`
(after).

The two cells whose answers the counter had to find among every subset of
their gate atoms:

| Cell | before | after | after/before | clingo |
|---|---:|---:|---:|---:|
| independent-negation-8 | 107.1 | 19.4 | 0.18 | 4.7 |
| independent-negation-10 | 1334.4 | 58.6 | 0.044 | 4.7 |

Every other cell moved within its band (0.92 to 1.04). The family was
also timed three times each outside the harness, on the `before` and
`after` executables and on clingo, with the candidates each executable
examined:

| Nodes | before | candidates | after | candidates | regions (refuted) | clingo |
|---:|---:|---:|---:|---:|---:|---:|
| 8 | 0.03 s | 14,080 | 0.01 s | 55 | 175 (33) | 0.00 s |
| 10 | 0.93 s | 147,456 | 0.05 s | 144 | 463 (88) | 0.00 s |
| 12 | 11.9 s | 1,544,192 | 0.15 s | 377 | 1,217 (232) | 0.00 s |

In the problem's words: the program chooses, for each node of a path,
whether it is in or out, and forbids two adjacent nodes in; its answers
are the independent sets of the path. Before, the counter offered every
combination of the twenty gate atoms that a restriction did not rule out
and let the reduct reject all but the answers. Now a region decides one
node at a time: holding a node out makes it in by its own rule, holding
it in makes it out impossible, and holding two adjacent nodes in fires
the constraint in the region's lower closure, which refutes the region
before any seed. The leaves are exactly the answers, each still checked
in full, and no region was counted. The remaining factor over clingo, ten
on these cells, is the two closures per region and the reduct check per
leaf, both computed from scratch; the parallel visit of regions and the
sharing of a parent's closure with its children are the next steps on
this route.

## The corpus at the region split, against clingo

Two campaigns over the whole kr-domains corpus on 17 September 2026, the
reports written at 22:39:35 and 22:40:13 UTC, same machine and profile,
four workers, one warmup and three repetitions, a 120-second timeout and
clingo timed beside every cell. `main` is the 896a5f73 executable and
`after` is built from `fcf8cad0` (SHA-256
`004e153e8db2ca8366a316aa882e018db2c9e9cff330dd684b1267ba90de0265`).
[corpus-fcf8cad0-cpu-auto.json](corpus-fcf8cad0-cpu-auto.json) and its
[table view](corpus-fcf8cad0-cpu-auto-tables.md) are the derived
comparison; raw report SHA-256
`f73a3e393e318fdd63e92a47e6f8c49ea371f60c5488bfd9aa4e879568a6240e` (main)
and `368eb6cd7aad2adb166c56bddb7d06118436ed6afaf3bb221d35f6c02dac21f3`
(after). Of the 86 cells, 85 were timed; task-allocation
variant-04/05-larger-mix exceeded the corpus profile's output capture
limit on both executables and is not compared here (the series suite
times it with a bounded model count).

Per family, the ratio of the current executable's median to clingo's in
the same run (count, least, median, greatest):

| Family | cells | least | median | greatest |
|---|---:|---:|---:|---:|
| send-money | 1 | 0.75 | 0.75 | 0.75 |
| task-allocation variant-01 | 16 | 0.99 | 1.24 | 2.27 |
| task-allocation variant-02 | 18 | 1.02 | 1.27 | 2.00 |
| task-allocation variant-03 | 18 | 1.02 | 1.30 | 1.64 |
| task-allocation variant-04 | 17 | 0.43 | 1.31 | 1.81 |
| equality-generalized-tsp | 3 | 1.15 | 1.43 | 1.50 |
| task-allocation variant-05 | 6 | 1.06 | 1.46 | 1.66 |
| n-queens | 6 | 0.76 | 2.07 | 7.74 |

Over the 85 cells the median ratio is 1.31 (it was 1.28 for the
pre-branch executable in its own run, and the totals are 742 ms against
clingo's 572 ms, from 816 ms). Five cells are faster than clingo
(agent-serialization 0.43, send-money 0.75, queens variant-02 0.76,
precedence 0.97, subtour-unsat 0.99); 75 lie between one and two times;
three between two and five (ring, queens variants 01 and 03); two above
five (queens variants 05 and 06, at 6.4 and 7.7).

In the problem's words: most of the corpus is scenario cells that both
solvers answer in five to twelve milliseconds, where the time is process
start and admission and the ratio is within a third of one; the branch's
work has not moved those and was not aimed at them. The cells the branch
moved are the ones with real grounding or search: send-money from 3.7
times clingo to 0.75, queens variants 01 and 03 halved to about twice
clingo, queens variant-02 from 0.85 to 0.76. The two queens variants
still at six to eight times spend, by the profile of this executable, a
quarter of their time in the reduct search's propagation and a sixth in
byte comparison of predicate names, which the dense predicate identity
is to remove; their per-model output cost is the writer's.

## Shared predicate names

Three campaigns on 17 September 2026, the reports written between 23:02:25
and 23:13:19 UTC, same machine, profile, four workers and clingo as above.
`main` is the 896a5f73 executable rerun as the control, `before` the
region-split executable (`fcf8cad0`, SHA-256
`004e153e8db2ca8366a316aa882e018db2c9e9cff330dd684b1267ba90de0265`) and
`after` is built from `df256d7d` (SHA-256
`fc865a4bda26205538d67e8b37b695afd0a41434ca45e345082581f996e2f21b`), where
a predicate's name is one shared allocation across the atoms that carry it.
[series-df256d7d-cpu-auto.json](series-df256d7d-cpu-auto.json) and its
[table view](series-df256d7d-cpu-auto-tables.md) are the derived comparison;
raw report SHA-256
`ea9e7f9d2cc2b770f0d9005c54a1b02177944e11443985d8a84a9db326624856` (main),
`7eb85fcd179f6e234c1ee75c5323a064d2cff948841f29afef48064f6b3c31b0` (before)
and `9d251ebb21023078406c5a462ee268967eea5b78019a531bf0079e6d6f08d833`
(after).

The series is flat. The formula cells read 0.94 to 1.06 (send-money 0.96,
independent-negation-aggregate-16 0.94, independent-choice-16 0.96,
n-queens/variant-01 8→10 1.06), the closure-route cells 1.00 to 1.11, with
chain-1000 at 1.11 and chain-2000 at 1.09. Those two were remeasured
outside the harness with `perf stat` over fifteen runs each: chain-2000
takes 34.58 ± 0.36 ms on `before` and 34.59 ± 0.30 ms on `after` with four
workers, 34.81 ± 0.43 and 35.29 ± 0.53 with one, so the campaign's
readings were the width of the band (the `before` run's maxima on the
transitive cells reached 78 ms) and not a change.

In the problem's words: the profile that motivated the step was taken
with the plain writer, where a seventh of the samples compared predicate
names byte by byte. Under the harness's JSON output the same cell's
profile is a different picture: the JSON encoder's buffer and quoting
take a third of the samples and the reduct search's propagation a sixth,
and the byte comparison is under three percent after this change, where
it was fourteen before. The comparison is gone, and its share of the
harness's cells was too small to show in wall time. What the profile now
says plainly is that the writer is the next cost on every cell with many
answers, and that the search's propagation is the one after it.

## The JSON document that spells each atom once

Three campaigns on 18 September 2026, the reports written between 00:33:14
and 00:39:27 UTC, same machine, profile, four workers and clingo as above.
`main` is the 896a5f73 executable rerun as the control, `before` the
shared-names executable (`df256d7d`, SHA-256
`fc865a4bda26205538d67e8b37b695afd0a41434ca45e345082581f996e2f21b`) and
`after` is built from `40d197a9` (SHA-256
`b34b5f8831f1d7b2ed046509776dc46e8c317a5418a0aaa911a2da9633f0b59e`), where
the JSON document spells each atom once and a record refers to its atoms
by index (schema 2). The harness reads both document forms, so the
control and the previous head are compared as before.
[series-40d197a9-cpu-auto.json](series-40d197a9-cpu-auto.json) and its
[table view](series-40d197a9-cpu-auto-tables.md) are the derived comparison;
raw report SHA-256
`28d202947e1aee2dc430c32b543a9e5e48da67edc6dae36ee33d76f957027f47` (main),
`40cbbc315ef4bc99bd09e50532ebe981954167ef3d003fbac0a5e0db4c67db7b` (before)
and `a4619f9f95c27b1d547d459c0d32cf1d2cfe5b2b6f484073e834757f74e7f411`
(after).

The harness times the JSON output, so every cell with many answers moved:

| Cell | before | after | after/before | clingo |
|---|---:|---:|---:|---:|
| independent-negation-aggregate-16 | 349.8 | 25.9 | 0.07 | 31.4 |
| ties-50 | 173.4 | 26.9 | 0.16 | 18.5 |
| independent-choice-16 | 204.2 | 41.2 | 0.20 | 28.9 |
| disjunction-12 | 209.1 | 47.1 | 0.23 | 20.7 |
| variant-04/05-larger-mix | 302.7 | 92.8 | 0.31 | 146.0 |
| independent-choice-12 | 24.7 | 10.0 | 0.41 | 6.0 |
| n-queens/variant-04 8→11 | 294.2 | 159.8 | 0.54 | 174.9 |
| n-queens/variant-01 8→10 | 94.4 | 59.8 | 0.63 | 31.4 |
| n-queens/variant-01 8→11 | 391.5 | 264.6 | 0.68 | 179.0 |
| independent-negation-10 | 58.0 | 50.0 | 0.86 | 4.7 |

The cells with one answer are flat: send-money 1.09 and transitive-path-100
1.22 lie inside their bands (transitive-path-100 measured 13.8 ± 0.5 ms on
`before` and 14.1 ± 0.3 ms on `after` over nine runs each outside the
harness), and chain-2000 0.94, transitive-path-200 1.02, stratified 1.01.
Outside the harness, queens-11 with 2,680 answers writes 639,741 bytes
where it wrote 8,730,633, and takes 234 ms against 246; with the plain
writer both take 232 ms.

In the problem's words: every answer used to be written out in full, each
atom with its predicate, sign and typed arguments, so a document of
thousands of answers over the same hundred atoms spelled those atoms
thousands of times, and the cells that print many answers spent most of
their time in that spelling. The document now spells an atom the first
time an answer holds it and refers to it by number afterwards, and what
is shown is still decided by the program's `#show` directives. Three
cells now run faster than the reference on the harness's terms
(aggregate-16 at 0.82, the larger mix at 0.64, queens variant-04 at
0.91), and the formula cells that had not moved in the whole tranche
turn out to have been waiting on the writer. A document of one answer
pays nothing for the numbering: its one record is deferred whole and
indexed only if a second record asks.

## The shared traversal and the regions method beside the clauses

Four campaigns on 18 September 2026, the reports written between 03:00:05
and 03:09:05 UTC, same machine, profile, four workers and clingo as above.
`main` is the 896a5f73 executable rerun as the control, `before` the
schema-2 executable (`40d197a9`, SHA-256
`b34b5f8831f1d7b2ed046509776dc46e8c317a5418a0aaa911a2da9633f0b59e`),
`after` is built from `906d89b6` (SHA-256
`96680774689e7a63ea2f7ac0f5d1faf629a286579f9a8272e670641e3a86b133`), where
the region traversal is one operation shared by both routes and the
formula route can propose candidates by regions, and `regions` is the same
executable run with `--candidates regions` (the flag was renamed `--search`
in the following commit), which is the only profile field the four reports
differ in. [series-906d89b6-cpu-auto.json](series-906d89b6-cpu-auto.json)
and its [table view](series-906d89b6-cpu-auto-tables.md) are the derived
comparison; raw report SHA-256
`fbe12748c647d32036ef51e6eadeaa5024e9ea8951be9e931de120b57cfedd9c` (main),
`2fe548d01221b8eb75386c3daf847d4a85eb0a022c94cc3583196e9b899c9691` (before),
`12fafec1c5ca1e6ad897f495f9ce3bced4945ef33ae6942cc3ddca4e73ae49e7` (after)
and `dc0116e85651128569127800e92f3a5c517a70f8736d0b2cd878e9decd239f80`
(regions).

The default is unchanged and so is the series: `after` against `before`
reads 0.91 to 1.06 on every cell, the closure route having moved onto the
shared traversal without a change in what it does. The regions method is
not at parity on the formula cells:

| Cell | after | regions | regions/after | clingo |
|---|---:|---:|---:|---:|
| n-queens/variant-01 8→11 | 270.1 | 5075.4 | 18.8 | 180.8 |
| n-queens/variant-04 8→11 | 164.5 | 3254.0 | 19.8 | 180.5 |
| n-queens/variant-01 8→10 | 61.3 | 927.6 | 15.1 | 32.3 |
| send-money/send-money | 11.5 | 158.9 | 13.8 | 15.2 |
| variant-04/05-larger-mix | 95.3 | 653.5 | 6.9 | 150.5 |
| chain-arithmetic-1000 | 10.0 | 63.3 | 6.3 | 5.6 |
| independent-negation-aggregate-16 | 25.7 | 84.4 | 3.3 | 33.4 |
| ties-50 | 27.6 | 72.6 | 2.6 | 18.4 |
| disjunction-12 | 45.8 | 92.8 | 2.0 | 21.7 |

The closure-route cells are flat under either method, as they must be,
since the method governs the formula route alone. The receipts say where
the regions method's time goes. On n-queens 11 it makes 26,195 decisions
where the clause search makes 33,712, visits 52,391 regions and refutes
23,516 of them, and reaches its 2,680 answers with no candidate rejected
by the reduct; but it charges 1.16 billion node visits against the clause
search's 57 million, about 22 thousand per region and 8 thousand
propagation events per narrowing on a DAG of 3,973 nodes. Every narrowing
starts from the region's decisions alone and re-derives the whole
knowledge of the parent, so the cost of a region is the size of the theory
rather than the size of the split. The search shape is right and the
primitive is not yet incremental across the tree: a child must inherit its
parent's knowledge and propagate the one atom that changed. That is the
next step, and the default follows its measurement.

In the problem's words: the formula route can now grow its answers the way
the closure route does, by splitting a region of candidates on one atom
and letting the theory's own rules decide what follows, and it reaches the
same answers with fewer guesses than the clause search. What it does not
yet do is remember, when it steps from a region to its child, what it
already knew about the parent; it works the whole theory out again at
every step, and on queens that is twenty times the work. The clause search
keeps its default until that memory is in place.

## The knowledge a region hands to its children

Four campaigns on 18 September 2026, the reports written between 03:57:05
and 04:05:17 UTC, same machine, profile, four workers and clingo as above.
`main` is the 896a5f73 executable rerun as the control, `before` the
traversal executable (`906d89b6`, SHA-256
`96680774689e7a63ea2f7ac0f5d1faf629a286579f9a8272e670641e3a86b133`),
`after` is built from `f5801c49` (SHA-256
`efd4027ab547f8e069c63fc00fbc4bfa49acd28b13a05bf92b20822539620865`), where
the reduct's proper-subset query is a region tree under the regions
method and a region hands its knowledge to its children, and `regions` is
the same executable run with `--search regions`.
[series-f5801c49-cpu-auto.json](series-f5801c49-cpu-auto.json) and its
[table view](series-f5801c49-cpu-auto-tables.md) are the derived
comparison; raw report SHA-256
`3b735cbbecab57b835e0c7e63e08e236e0ed7933695bf0918da0390f5de3ceb8` (main),
`1c1f5177eef7597fa58cc6fdcdc2536aeb1708b4455ce645f282a485905165e8` (before),
`d7002fb087bdc5ae5b5ab5e44d19be6deb395fb44998df9638ab39c0cb293e7f` (after)
and `f8a74b2cd398eb8ded35305e4748d2da8707be84225fb2ea17497635a71e9299`
(regions).

The default is unchanged and `after` against `before` is flat, 0.91 to
1.08 on every cell. The regions method, which now proposes candidates and
queries the reduct on the same trees with no clause form anywhere, stands
against the clause search as follows on the formula cells:

| Cell | after | regions | regions/after | clingo |
|---|---:|---:|---:|---:|
| disjunction-12 | 44.6 | 32.9 | 0.74 | 22.9 |
| independent-negation-aggregate-16 | 25.6 | 27.4 | 1.07 | 31.3 |
| n-queens/variant-01 8→11 | 262.9 | 287.7 | 1.09 | 183.3 |
| ties-50 | 26.6 | 29.3 | 1.10 | 19.1 |
| n-queens/variant-01 8→10 | 59.4 | 67.5 | 1.14 | 33.2 |
| variant-04/05-larger-mix | 90.8 | 104.3 | 1.15 | 153.1 |
| send-money/send-money | 10.9 | 15.5 | 1.42 | 15.1 |
| n-queens/variant-04 8→11 | 159.7 | 239.3 | 1.50 | 179.0 |

Against the previous observation the regions method moved from fifteen to
twenty times the clause search on queens to 1.09 and 1.50, and from 13.8
times on send-money to 1.42, while the answers and the decision counts
are what they were: on n-queens 11 it still makes 26,194 decisions where
the clause search makes 33,712, and its charged work fell from 1.18
billion to 46 million, below the clause search's 57 million. The
propagation events per region fell from eight thousand to about fifty,
which is the size of a split rather than of the theory. Disjunction-12,
where the support cut decides the second head of every rule as soon as the
first is known, is faster by regions than by clauses, and by a wider
margin than any formula cell is slower. The clause search keeps the
default: two cells still read above 1.4, and the rule is that the default
follows the measurement, not the thesis.

In the problem's words: a region now remembers what its parent worked
out, so stepping from a region to its child costs what the one decision
changed and nothing else. The formula route reaches its answers with the
same primitives that decide membership, generation and reduct query
alike, at roughly the clause search's cost on every cell and below it on
one; the remaining gap is on the cells where the clause search's root
probing finds forced atoms the readings only find by splitting.

## Several workers on the region tree

Five campaigns on 18 September 2026, the reports written between 04:24:15
and 04:33:09 UTC, same machine, profile, four workers and clingo as above.
`main` is the 896a5f73 executable rerun as the control, `before` the
carried-knowledge executable (`f5801c49`, SHA-256
`efd4027ab547f8e069c63fc00fbc4bfa49acd28b13a05bf92b20822539620865`),
`after` is built from `553acae3` (SHA-256
`0c9551639c03546fef8c06c6f0b480aff2e158469a6b2ce414ded95b82ada59b`), where
several workers can walk the region tree at once, `regions` is the same
executable run with `--search regions`, and `parallel` with
`--search regions --region-workers 4`, the profile's worker count.
[series-553acae3-cpu-auto.json](series-553acae3-cpu-auto.json) and its
[table view](series-553acae3-cpu-auto-tables.md) are the derived
comparison; raw report SHA-256
`51776535b833ed52b7b4ac0cea9335b48d8f945a27d12af3c74ade771baf40a4` (main),
`a69869bb0998554986a567cde1ccea52d9653b8fd3c65c626a54a4b6563dd2d7` (before),
`5f1c701679958317fe2a8e331306bea174530e29e81c2ffcd79016c49dc82cba` (after),
`d9824416f78b459f19db4b641868fc0c9c522af1adee34e3e9f178ccde359a1f`
(regions) and
`342c2b365a54c33b890397d3b976d4a3df7d8a74a16459dc0cf6cad6227f4368`
(parallel).

The default is unchanged and `after` against `before` is flat, 0.95 to
1.12 on every cell, the widest reading on a cell whose band is that wide.
Four workers on the region tree, against the clause search that is the
default and against the reference, on the formula cells:

| Cell | after | regions | parallel | parallel/after | clingo | parallel/clingo |
|---|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 88.3 | 103.7 | 42.8 | 0.48 | 149.0 | 0.29 |
| n-queens/variant-01 8→11 | 254.4 | 281.0 | 119.0 | 0.47 | 184.6 | 0.64 |
| n-queens/variant-04 8→11 | 156.2 | 235.8 | 97.0 | 0.62 | 179.6 | 0.54 |
| n-queens/variant-01 8→10 | 56.8 | 66.7 | 32.7 | 0.58 | 31.2 | 1.05 |
| disjunction-12 | 43.1 | 31.2 | 22.5 | 0.52 | 21.6 | 1.04 |
| independent-negation-aggregate-16 | 24.9 | 28.1 | 20.6 | 0.83 | 30.3 | 0.68 |
| ties-50 | 25.5 | 28.5 | 21.5 | 0.84 | 18.6 | 1.16 |
| send-money/send-money | 10.5 | 15.0 | 13.3 | 1.26 | 14.2 | 0.94 |

The clingo column is the reference measured in the parallel run itself.

The closure-route cells are flat under every arm, as they must be. The
answers are the same in every arm, each once, checked by the harness's
parity and by the corpus test that walks all 94 cases with four workers
and compares the answer sets atom for atom; the order they arrive in
differs between runs, which the contract now says. On the cells with many
answers the four workers take between 0.47 and 0.62 of the clause
search's time, which is the share-nothing partition doing what it was
built to do: the tree is cut into regions that need nothing from one
another, and each worker walks its own. On send-money, one answer found
after a long chain of refutations, the workers gain nothing and the
per-region cost of the regions method still shows. Against the reference
the parallel arm is below it on five of the eight formula cells and within
1.16 on the rest; it is the first arm in this record to be faster than
clingo on n-queens.

The default stays the clause search on one worker, the configuration the
harness runs, since the regions method on one worker is still above it on
six formula cells. Whether the session's default becomes regions with the
host's parallelism is a decision on the record, not a measurement.

In the problem's words: with the knowledge carried and the tree cut into
regions, four workers finish queens and the larger mix in under half the
time of the clause search and ahead of clingo, and the method that does it
is the one the thesis named, candidate generation and membership by the
same reading of the program, split into regions that share nothing.

## The regions default and one worker count

Four campaigns on 18 September 2026, the reports written between 07:48:22
and 07:56:37 UTC, same machine, profile, four workers and clingo as above.
`main` is the 896a5f73 executable rerun as the control, `before` the
parallel-regions executable (`553acae3`, SHA-256
`0c9551639c03546fef8c06c6f0b480aff2e158469a6b2ce414ded95b82ada59b`),
whose default was still the clause search, `after` is built from
`c2e939a6` (SHA-256
`46a51100c58cd8984aa494c6dead31edf21710c0ed89b9b7aa34d9f6f0b9c7fa`),
where the regions method is the default and `--workers` is the one worker
count, so the profile's four workers walk the region tree, and `clauses`
is the same executable run with `--search clauses`.
[series-c2e939a6-cpu-auto.json](series-c2e939a6-cpu-auto.json) and its
[table view](series-c2e939a6-cpu-auto-tables.md) are the derived
comparison, the first derived by the view that keeps a scoreboard against
the reference; raw report SHA-256
`f12e86eb7a568d47b69a960dd5df32f8a99bb3745eb1cca134b7ea91962d6448` (main),
`b512dc5641a85e2bb60b719e75b89db72264a1b411a0b522d0e12fd1ab96f9d5` (before),
`e20192b41dc32b49cac094b2dc11c16fce96a67428671fcdbb28f02b4d444853` (after)
and
`cfd6e4c3eff611fb78b84a4dca2bdc8be11f1249404508c287cbd83d3c84af26`
(clauses).

The formula cells, the new default against the previous one and against
the clause search on the same executable:

| Cell | before | after | clauses | after/before | clingo | after/clingo |
|---|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 90.0 | 44.3 | 95.7 | 0.49 | 151.2 | 0.29 |
| n-queens/variant-01 8→11 | 256.2 | 121.3 | 264.5 | 0.47 | 183.4 | 0.66 |
| n-queens/variant-04 8→11 | 158.0 | 98.5 | 162.7 | 0.62 | 181.9 | 0.54 |
| n-queens/variant-01 8→10 | 58.9 | 33.3 | 59.9 | 0.56 | 31.6 | 1.05 |
| disjunction-12 | 45.4 | 22.0 | 45.5 | 0.48 | 21.0 | 1.05 |
| independent-negation-aggregate-16 | 24.7 | 21.7 | 26.0 | 0.88 | 32.7 | 0.66 |
| ties-50 | 26.3 | 21.1 | 27.1 | 0.80 | 18.0 | 1.17 |
| send-money/send-money | 11.3 | 15.2 | 11.4 | 1.35 | 15.2 | 1.00 |
| stratified-16 | 4.5 | 4.5 | 4.5 | 1.01 | 4.4 | 1.03 |

The clingo column is the reference measured in the `after` run itself.
The closure-route cells are within 0.89 and 1.04 of `before`, the widest
readings on cells whose bands are that wide; the `clauses` arm is the
`before` arm again on every cell, as it must be, since it is the same
method. The answers are the same in every arm, each once, checked by the
harness's parity and by the corpus test in its three variants.

The default now does what the previous section measured for the parallel
arm: queens, the larger mix and the disjunction cells in about half the
clause search's time, and the send-money regression of the regions method
on one answer found after many refutations, 1.35, is now the default's.
The scoreboard says how the arms stand against the reference on the
twenty cells: `main` faster on none, `before` on four, `after` on four
and `clauses` on five, the difference being send-money, where `after` is
level with the reference at 1.00 and `clauses` below it at 0.79. The
losses of `after` are the closure-route cells the earlier sections
measured and three formula cells within 1.05 of the reference; the
scoreboard lists each with its parts. In this executable the parts of a
cell under several workers are misattributed: the coordinator times only
its wait for the workers' models, which the view reads as candidate
proposal, so membership reads as nearly nothing on the parallel cells.
Since this executable the workers time their own narrowing and leaf
decisions, summed over the workers, and the next observation reads
correctly.

In the problem's words: the default is now the method the thesis named,
on the host's workers, and it is faster than clingo on four of the twenty
cells and within a twentieth of it on three more; what remains above the
reference is the closure route's fixed cost on small programs and the
per-region cost of one long chain of refutations.

## The scoreboard, the parts of a cell and the memory rounds

Four campaigns on 18 September 2026, the reports written between 08:12:02
and 08:21:34 UTC, same machine, profile, four workers and clingo as above,
each with one memory round per solver and cell. `main` is the 896a5f73
executable rerun as the control, `before` the default-search executable
(`c2e939a6`, SHA-256
`46a51100c58cd8984aa494c6dead31edf21710c0ed89b9b7aa34d9f6f0b9c7fa`),
`after` is built from `4b6e2636` (SHA-256
`36a0876f8ef30382297b6accb50ca6acedaa1af0d16250f6c851e56726bec90c`), where
the region workers time their own narrowing and leaf decisions, and
`clauses` is the same executable run with `--search clauses`.
[series-4b6e2636-cpu-auto.json](series-4b6e2636-cpu-auto.json) and its
[table view](series-4b6e2636-cpu-auto-tables.md) are the derived
comparison; raw report SHA-256
`e035dd16192236c2c54a428455329e7a4a36e87b11d297e44c42b24e1b5e2741` (main),
`6a9fe5a5e6821cb1a07a2ed4fe786b8549b1299f659b69ceaead07f56efdf1b0` (before),
`3c41ef0f6c1cb78f928e7dbcb0dcab3bd6083b2f6d6d8392ed2fdb7c1b2cab42` (after)
and
`3d00eb912e592b58d51334a81790612aa6caa1cc995486ff6754a738efb71924`
(clauses).

`after` against `before` is flat, 0.96 to 1.04 on every cell: the
workers' timers cost nothing the measurement can see. The formula cells,
with the parts of `after` and the peak resident set of each solver:

| Cell | after | clingo | after/clingo | grounding | proposal | membership | clingo grounding | clingo solving | ours MiB | clingo MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 44.4 | 147.7 | 0.30 | 3.2 | 85.6 | 13.2 | 84.0 | 58.0 | 13.6 | 23.5 |
| n-queens/variant-04 8→11 | 98.7 | 179.8 | 0.55 | 1.6 | 324.5 | 18.6 | 1.0 | 173.0 | 11.3 | 11.9 |
| independent-negation-aggregate-16 | 20.4 | 31.1 | 0.65 | 0.2 | 8.5 | 4.0 | 1.0 | 27.0 | 10.5 | 10.2 |
| n-queens/variant-01 8→11 | 120.8 | 182.8 | 0.66 | 5.7 | 385.9 | 28.2 | 2.0 | 177.0 | 11.8 | 11.7 |
| send-money/send-money | 13.5 | 15.1 | 0.89 | 2.8 | 11.3 | 0.3 | 9.0 | 1.0 | 12.5 | 12.9 |
| stratified-16 | 4.5 | 4.5 | 1.01 | n/a | 0.8 | 0.3 | 0.0 | 0.0 | 8.9 | 10.4 |
| n-queens/variant-01 8→10 | 33.3 | 31.2 | 1.07 | 4.4 | 81.7 | 6.4 | 1.0 | 25.0 | 11.4 | 10.8 |
| disjunction-12 | 22.0 | 19.3 | 1.14 | 0.1 | 6.9 | 31.2 | 0.0 | 14.0 | 10.1 | 10.3 |
| ties-50 | 21.1 | 18.5 | 1.14 | 0.3 | 18.9 | 3.1 | 1.0 | 13.0 | 11.8 | 10.6 |

Milliseconds and mebibytes; the clingo columns are the reference
measured in the `after` run itself, its grounding its own total less its
solving time, both printed to the millisecond. The parts of `after` are
summed over the four workers, as the contract now says, so proposal and
membership together exceed the wall time on the parallel cells: on
queens 8→11 the workers spend 386 ms narrowing and 28 ms deciding leaves
to finish in 121 ms of wall time, and on the disjunction cell the leaf
decisions, 31 ms summed, are the larger part. The memory rounds put the
two solvers within a few mebibytes of each other on the formula cells,
ours below the reference on the larger mix, 13.6 against 23.5; on the
closure-route chain cells ours is above it, 20.7 against 11.1 on
chain-2000 and 24.6 against 12.5 on producer-chain-700, the retained
closure workspaces the earlier sections measured. No device ran, so the
device column is empty.

The scoreboard: `main` faster than the reference on none of the twenty
cells, `before` and `after` on five, `clauses` on four, and the closest
losses of `after` within 1.07 on stratified and queens 8→10 and at 1.14
on the disjunction and ties cells.

In the problem's words: the report now says, for each configuration,
where we beat clingo and by how much, and where the time goes when we do
not; on the cells we lose, it is the closure route's fixed cost on small
programs and, on the formula cells, the narrowing of many regions whose
leaves are few.

## The memory allowance from the host

Two campaigns on 18 September 2026, the reports written between 08:35:51
and 08:37:08 UTC, same machine, profile, four workers and clingo as above,
one memory round per solver and cell. `before` is the scoreboard
executable (`4b6e2636`, SHA-256
`36a0876f8ef30382297b6accb50ca6acedaa1af0d16250f6c851e56726bec90c`),
`after` is built from `e2b052b0` (SHA-256
`b9cb2b9c2c2ccb4ff7562d77ad24de55715f897a6c7c4ed7907df9f018410093`),
where the command sizes its byte ceilings by the host's memory: half of
the host's physical memory as the allowance, the library's ceilings
scaled by it, here 46 gibibytes on a host of 92.
[series-e2b052b0-cpu-auto.json](series-e2b052b0-cpu-auto.json) and its
[table view](series-e2b052b0-cpu-auto-tables.md) are the derived
comparison; raw report SHA-256
`6ba02660dd4633c9cbbff11fc88122c0bebec3f32ad03e9e5aa884d93e55fa60` (before)
and
`ca99dc51107c6855b3567f6aafc69a4a815fea18392625c188593a5618477274` (after).

`after` against `before` is 0.95 to 1.06 on eighteen cells and 0.80 and
1.19 on the two smallest, stratified-16 and transitive-path-100, whose
bands are that wide: the ceilings bound named storage and touch no timed
work, and no cell reached one under either allowance. The peak resident
set is the same to the mebibyte on every cell, since a larger ceiling
reserves nothing. The scoreboard reads six of twenty for `before` and
five for `after`, the sixth being send-money at 0.95 against 1.02, a cell
that sits at the reference either way. The campaign's first run asked
every executable for the reference allowance and the older one refused
the option, which is why the campaigns now leave the allowance to the
host and read it from each sample's statistics.

## The Latin-square and planning cells, and the corpus against clingo

Four campaigns on 18 September 2026, the reports written between 08:51:53
and 08:55:55 UTC, same machine, profile, four workers and clingo as above,
one memory round per solver and cell. `before` and `after` are the same
solver executable (SHA-256
`b9cb2b9c2c2ccb4ff7562d77ad24de55715f897a6c7c4ed7907df9f018410093`, the
tree at `359fcedc` changes only the measurement harness), run by the
harness that now names twenty-two series cells, the Latin square of order
five with its first row fixed (1,344 squares) and the line walked for
fourteen steps (3,432 plans) added; `clauses` is the same executable under
`--search clauses`; and `corpus` is the same executable on all 94 corpus
cases, the first corpus campaign under the scoreboard.
[series-359fcedc-cpu-auto.json](series-359fcedc-cpu-auto.json), its
[table view](series-359fcedc-cpu-auto-tables.md),
[corpus-359fcedc-cpu-auto.json](corpus-359fcedc-cpu-auto.json) and its
[table view](corpus-359fcedc-cpu-auto-tables.md) are the derived
comparisons; raw report SHA-256
`e32738797ff6760a31aa588edaa66bcc7c0ca282e4b3120a862a3b9a6bfe54b3` (before),
`ddacdee93b7ba071bb10dc9619c956f4258577d1ce6a3dca0587e41e450cbba3` (after),
`acfacca796cd96daa6ac26f07c6dff71b3fc56dc8fb6834ca89c589465c60139`
(clauses) and
`955ef83b94552892e263d18aff70b5f961aebf2db33bf46d3a901c776d0aee7e`
(corpus).

The two new cells, with the parts of `after` summed over the four workers
and the peak resident set:

| Cell | after | clauses | clingo | after/clingo | clauses/clingo | grounding | proposal | membership | ours MiB | clingo MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| latin-square-5 | 16.2 | 36.7 | 15.7 | 1.03 | 2.29 | 1.2 | 23.6 | 10.3 | 10.6 | 10.7 |
| planning-14 | 38.0 | 86.5 | 16.4 | 2.32 | 5.12 | 3.0 | 72.7 | 18.4 | 10.6 | 10.7 |

`after` against `before`, the same executable twice, is 0.89 to 1.06:
the band of the cells. The Latin square, a constraint problem in the shape
of Sudoku, is level with the reference under the default and at 2.29 under
the clause search; the plans, whose position is carried by frame rules
from step to step, are 2.32 above the reference under the default and
5.12 under the clause search, with the narrowing of the regions the
larger part. The scoreboard reads seven of twenty-two for the default and
four for the clause search.

On the corpus, all 94 cases passed on both solvers; the default is faster
than the reference on nine, at 0.29 to 0.34 on three task-allocation
cases and 0.71 on the second queens encoding, level on four, and the
median ratio over the 94 is 1.25 with the widest loss at 1.90. The losses
are the small cases the earlier sections measured, five to twelve
milliseconds against clingo's five to seven, where the fixed cost of
admission and grounding is the whole of the difference: on the
layered-DAG shortest-path cases the parts read one to two milliseconds of
grounding, five to twelve of narrowing summed over the workers, and under
a millisecond of membership.

In the problem's words: the report now covers the whole corpus, and on it
we are ahead of clingo where the search is large and behind by a fixed
few milliseconds where it is small; the plans cell names the next thing to
lift, the narrowing of many regions whose leaves are few.

## Each clause and body as one node

Four campaigns on 18 September 2026, the reports written between 12:36:58
and 12:40:49 UTC, same machine, profile, four workers and clingo as above,
one memory round per solver and cell. `before` is the families executable
(`359fcedc`, SHA-256
`b9cb2b9c2c2ccb4ff7562d77ad24de55715f897a6c7c4ed7907df9f018410093`),
`after` is built from `9b6ac698` (SHA-256
`0186dbbe214cd4e7e9d67430d36354472f11e908671b0d3c4fff327e7de47f1e`),
where the narrowing reads each clause and body as one node with two
counters, applies only the decisions made since the parent's closure and
keeps the split ranking as nodes become known; `clauses` is the same
executable under `--search clauses`; and `corpus` is the same executable
on all 94 corpus cases.
[series-9b6ac698-cpu-auto.json](series-9b6ac698-cpu-auto.json), its
[table view](series-9b6ac698-cpu-auto-tables.md),
[corpus-9b6ac698-cpu-auto.json](corpus-9b6ac698-cpu-auto.json) and its
[table view](corpus-9b6ac698-cpu-auto-tables.md) are the derived
comparisons; raw report SHA-256
`bb3d5b31927ae207715f8aa147b43089e0258c61b26cf018d9e5e28508096660` (before),
`ef299e8f164e915dfabe29efb17fdc567abdf40ef6462f22701f8eb1077f1d85` (after),
`b62df20c98d13f93bf9bde7206b1b53c1bc939eab414e284c202c3da32b54f83`
(clauses) and
`f055c4d3c441030e6e840dda8a62a9eec9b9ac8aafbfbf2f0537ad2d364b7685`
(corpus).

The formula cells, with the parts of `after` summed over the four workers:

| Cell | before | after | clauses | after/before | clingo | after/clingo | proposal | membership |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-01 8→11 | 123.3 | 85.2 | 268.8 | 0.69 | 182.5 | 0.47 | 215.4 | 28.0 |
| variant-04/05-larger-mix | 43.3 | 32.9 | 95.0 | 0.76 | 149.1 | 0.22 | 31.6 | 12.7 |
| n-queens/variant-01 8→10 | 33.0 | 26.2 | 60.5 | 0.79 | 31.2 | 0.84 | 46.9 | 6.2 |
| n-queens/variant-04 8→11 | 99.1 | 78.8 | 163.0 | 0.80 | 179.2 | 0.44 | 221.1 | 18.2 |
| latin-square-5 | 15.7 | 13.7 | 37.3 | 0.87 | 16.7 | 0.82 | 12.1 | 10.2 |
| planning-14 | 38.1 | 37.1 | 86.0 | 0.97 | 16.2 | 2.29 | 42.1 | 18.6 |
| independent-negation-aggregate-16 | 21.0 | 21.3 | 26.5 | 1.02 | 30.2 | 0.71 | 3.8 | 4.1 |
| disjunction-12 | 21.6 | 22.6 | 46.0 | 1.04 | 20.3 | 1.11 | 2.6 | 37.9 |
| ties-50 | 21.2 | 22.0 | 27.2 | 1.04 | 19.0 | 1.16 | 14.0 | 3.1 |
| send-money/send-money | 13.7 | 14.2 | 11.1 | 1.04 | 14.4 | 0.98 | 6.5 | 0.3 |
| stratified-16 | 4.5 | 4.5 | 4.6 | 1.00 | 4.5 | 1.01 | 0.5 | 0.3 |

The cells the narrowing dominates fall by a fifth to a third: queens 8→11
to 0.69, the larger mix to 0.76, the queens 8→10 and variant-04 boards to
about 0.8, the Latin square to 0.87. The cells whose time is elsewhere,
the disjunction cell in its leaf decisions, ties in its objective, send-money
in one long chain of refutations, are within 0.96 and 1.04, as are the
closure-route cells (0.88 to 1.09, the two widest on the smallest cells).
The plans cell moves little, 0.97: its narrowing is many regions of few
chains each, and the parts say the proposal is still its larger part. On
queens 11 with one worker, measured separately, the tree is the same
52,389 regions and the reading work halves, 31M against 46M.

The scoreboard reads seven of twenty-two for `after` against five for
`before` and four for the clause search; queens 8→11 stands at 0.47 of
the reference and the larger mix at 0.22. On the corpus the default is
faster on nine of 94 with a median ratio of 1.27 and a widest loss of
1.97: the small cases are the fixed cost of admission and grounding,
which this change does not touch.

In the problem's words: a decision now costs one step per clause it
touches instead of a walk of every clause it satisfies, and the cells
where the search is the work are a fifth to a third cheaper for it; what
remains on them is the propagation itself, the certificate's check of
each leaf, and the copy of the carried knowledge at each split.

## Two narrowing steps measured and not taken

Two further steps on the chain closure were built, measured on the same
cells with one worker, interleaved five runs each against the `9b6ac698`
executable, and reversed before a commit; they are recorded here so that
the reasoning is not repeated.

The first kept, per chain, the exclusive-or of the operands that could
still decide it, so the unit rule read its one open operand instead of
scanning the chain; the profile had put the scan at a tenth of the walk.
Measured: queens 11 194 ms against 189, the larger mix 50 against 48, the
tree identical. The scan runs over a few cache-hot operands and fires
rarely relative to the counter steps, and the extra word per chain costs
a step on every count and a copy at every split. What the experiment
found and kept is that a chain's operands must be distinct across the
subchains it absorbs (`fad7d459`).

The second replaced the copy of the carried knowledge at every split by a
trail with undo: the traversal kept one state for the path with a mark at
each split, restored for the sibling; the parallel walk copied only when
offering a region to the pool. The hypothesis was a tenth to an eighth
off the narrowing-dominated cells, the copies' share in the profile.
Measured, the tree identical on every cell: queens 11 227 ms against 193
with one worker and 142 against 83 with four; the plans 64 against 61;
the Latin square and the larger mix within their bands. Every learned bit
then costs a push and later a pop with a scattered write, and a region
learns a few hundred bits, which is dearer than one sequential copy of a
few kilobytes; under four workers the copies offered to the pool carried
the trail as well. The copy is the cheaper representation at this
theory size; the trail would be retired only where a region learns
little relative to the theory, which no cell of the series does. The
trail is kept as a patch outside the repository, not as code.

## Dense relations on the closure route

Four campaigns on 18 September 2026, the reports written between 09:59:50
and 10:03:00 UTC, same machine, profile, four workers and clingo as above,
one memory round per solver and cell. `before` is built from `a712f10d`
(SHA-256
`54069a1381bcc5fc86c39fa1e2f5afd1a7c4ae6b16809a8fafb0b29791acae25`),
the branch before this item; `after` is built from `6f114811` (SHA-256
`6930f3d13b893a151380aad0a3fab3d9147f48643f2f5cdf20fa9549bbb16fdf`),
where preparation infers a bound on each argument's values and a predicate
whose every argument is bounded is held as a bit array over the mixed-radix
index of its arguments' ranks, with membership a bit test, insertion a bit
set and a bound prefix one contiguous range; `clauses` is the same
executable under `--search clauses`; and `corpus` is the same executable on
all 94 corpus cases.
[series-6f114811-cpu-auto.json](series-6f114811-cpu-auto.json), its
[table view](series-6f114811-cpu-auto-tables.md),
[corpus-6f114811-cpu-auto.json](corpus-6f114811-cpu-auto.json) and its
[table view](corpus-6f114811-cpu-auto-tables.md) are the derived
comparisons; raw report SHA-256
`beca1ee081fcaef9fd770abee44fef9222d2b47f8f098277a60198b5fd18bb18` (before),
`e8a695b37de27fea07680928752319eeb1ad3c8bd4246bfd75c383caa0221149` (after),
`f77c803bc4ce128009381b8fc29cdf146b5d3d3cee94ca4b2042583ba35f71de`
(clauses) and
`38796bf2691d0bea061378918e8ec54145b9b4015377dcff532e3ba82576a7c8`
(corpus).

The closure-route cells, with the charged closure work of one run of each
executable on the cell's program and the peak resident set of the memory
round:

| Cell | before | after | after/before | clingo | after/clingo | work before | work after | MiB before | MiB after |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| transitive-path-200 | 46.7 | 31.0 | 0.66 | 22.4 | 1.39 | 5,378,096 | 955,649 | 17.4 | 14.5 |
| transitive-path-100 | 15.7 | 12.2 | 0.78 | 9.0 | 1.35 | 1,197,772 | 209,092 | 11.0 | 10.2 |
| independent-negation-10 | 53.4 | 49.9 | 0.93 | 4.5 | 11.06 | 425,724 | 136,012 | 9.0 | 8.5 |
| independent-negation-8 | 17.7 | 16.6 | 0.94 | 4.5 | 3.68 | 123,109 | 41,811 | 8.4 | 8.4 |
| chain-1000 | 19.6 | 18.5 | 0.94 | 6.9 | 2.67 | 475,452 | 179,678 | 14.5 | 14.6 |
| producer-chain-700 | 26.0 | 25.8 | 0.99 | 8.8 | 2.93 | 81,187 | 42,445 | 24.7 | 24.2 |
| chain-2000 | 34.7 | 39.8 | 1.15 | 10.1 | 3.94 | 1,025,311 | 578,537 | 20.5 | 21.0 |

The transitive cells fall by a fifth and a third, as the profile of the
item said they would: their time was the value comparisons of the
bound-prefix binary searches and the ordering of the catalogs each round,
and a dense window is a block of positions read word by word. The
enumeration cells move less than the hypothesis asked, 0.93 and 0.94
against a predicted 0.3 to 0.7: the charged work of a closure falls to a
third, but a derived atom of a dense predicate is still built as an atom,
kept in the round's pending set and inserted by ranking its values after
the round, so the allocation the hypothesis named is not yet gone; that is
the next step, the pending bits of the dense round. The chain cells are
where the hypothesis was refuted in part: the propositional producer chain
has nothing to lay out but one-cell relations, chain-1000 moves within
noise, and chain-2000 is slower by 0.15, because its edge relation over
2,001 values on each side has four million positions for two thousand
atoms, one megabyte of words per candidate against a hundred kilobytes of
catalog, and each probe of a bound row scans thirty-two words to find one
edge. Its memory rises by half a mebibyte. The condition that retires the
ceiling as it stands is a relation this sparse costing more than its
catalog did; a ceiling that reads the fact count as well as the product of
widths, or a row of the layout kept sparse, is the repair, and it is not
taken until the pending bits and the vector step have been measured on
the same cells, since both change what a probe costs.

Two corrections to the hypotheses as written. Transitive-dense-40 and
chain-arithmetic-1000 were named as closure-route cells; they are not.
The extended profile refuses variable arithmetic and the `<` comparison
in a body, so both take the formula route, where the dense store does not
reach, and both stand where they stood, 0.97 and 0.95. And chain-2000's
time before this item was mostly source preparation, twenty of thirty
milliseconds in parsing and fact expansion, which the closure cannot
touch. The formula cells are within 0.96 and 1.00 of `before` except
independent-choice-16 at 0.78, which this change does not reach and whose
statistics under the two executables are the same line for line; the
clause search of the same executable sits at 33.4 against `before`'s 40.5,
so the `before` run of that cell reads as the slow one.

The scoreboard reads seven of twenty-two for `before` and for `after`
alike; the transitive cells, at 1.35 and 1.39 of the reference, are now
among the closest losses. On the corpus all 94 cases passed
on both solvers; the default is faster on eight of 94, the median ratio
1.27 against the reference and the widest loss 1.82, against nine, 1.27
and 1.97 at `9b6ac698`; against that campaign's own times the median case
is 0.97 and the resident set 1.01, the largest gains 0.77 and 0.78 on two
salesman cases and the largest loss 1.25 on one unsatisfiable tour.

In the problem's words: a relation whose arguments the program bounds is
now a set of bits, and where the closure's time was comparing values and
ordering rows, the transitive cells, it is a fifth to a third cheaper;
where the time is building each derived atom, the enumeration cells, the
representation alone buys little until the round stops building them;
and where the relation is sparse in a wide box, the long chain, the bits
cost more than the tree.

## Pending bits for dense heads, and the size ladder

Six campaigns and one ladder on 18 September 2026, same machine, profile
and clingo as above. `before` is the dense-relation executable of
`6f114811` (SHA-256
`6930f3d13b893a151380aad0a3fab3d9147f48643f2f5cdf20fa9549bbb16fdf`);
`after` is built from `10644b3b` (SHA-256
`7934b92d743525db2ebd9ad8274e67078497566cac68eeff67b284a182a88c69`),
where a round records a derived head of a dense relation as a pending bit
in a row of words the closure workspace keeps for the layout, and joins the
marked words into the relation after the round, so that no atom of a dense
predicate is built before the model is assembled; `clauses` is the same
executable under `--search clauses`; and `corpus` is the same executable on
all 94 corpus cases, with four workers, and again with one worker under the
default search and under `--search clauses`. The series and four-worker
corpus reports were written between 15:43:01 and 15:44:09 UTC and the
one-worker corpus reports at 15:52:42 and 15:52:55, one memory round per
solver and cell.
[series-10644b3b-cpu-auto.json](series-10644b3b-cpu-auto.json), its
[table view](series-10644b3b-cpu-auto-tables.md),
[corpus-10644b3b-cpu-auto.json](corpus-10644b3b-cpu-auto.json), its
[table view](corpus-10644b3b-cpu-auto-tables.md),
[corpus-10644b3b-cpu-auto-one-worker.json](corpus-10644b3b-cpu-auto-one-worker.json)
and its [table view](corpus-10644b3b-cpu-auto-one-worker-tables.md) are the
derived comparisons; raw report SHA-256
`c329c0c553cdd94acda082d797f9f2407da7dbd8f2ee9df6eb3925434cbd3a35` (before),
`1e7fb64ecc29a60f5db1cdc669ec475df822913b119fcb755db60dfe45f0dac6` (after),
`4c84bfac46e076af55e67c3892cbe1e65d936effe26cb5d7412e538907c4f26b`
(clauses),
`5ff45f2ac557f0cb5beacf052d70e9a4d304d1420b4195ad92273a35cf993369`
(corpus, four workers),
`42ebb350630d9e78a6e75dbefb4f9d36cb59e53cbffc4cdd0a9aafc0934d387a`
(corpus, one worker) and
`57fdca9531ecd226f58f194b29c70af2f3e8d435e2af046cd4fe1ea1b897db3e`
(corpus, one worker, clauses).

The closure-route cells of the series, with the charged closure work of one
run and the peak resident set of the memory round:

| Cell | before | after | after/before | clingo | after/clingo | work before | work after | MiB before | MiB after |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| transitive-path-100 | 13.5 | 9.5 | 0.70 | 8.8 | 1.07 | 209,092 | 148,047 | 10.5 | 10.2 |
| transitive-path-200 | 32.2 | 23.1 | 0.72 | 21.8 | 1.06 | 955,649 | 804,204 | 14.5 | 14.7 |
| independent-choice-16 | 34.2 | 25.0 | 0.73 | 26.8 | 0.93 | 3,198,290 | 2,117,111 | 8.6 | 8.4 |
| stratified-16 | 4.5 | 3.4 | 0.77 | 4.4 | 0.78 | 2,439 | 1,681 | 8.8 | 8.7 |
| independent-negation-10 | 50.0 | 38.6 | 0.77 | 4.6 | 8.42 | 136,012 | 91,687 | 8.8 | 8.8 |
| independent-negation-8 | 16.9 | 14.0 | 0.83 | 4.6 | 3.05 | 41,811 | 28,784 | 8.4 | 8.4 |
| independent-choice-12 | 7.7 | 6.9 | 0.90 | 6.8 | 1.02 | 342,835 | 227,172 | 8.8 | 8.6 |
| chain-2000 | 35.5 | 33.4 | 0.94 | 11.0 | 3.05 | 578,537 | 736,073 | 21.0 | 21.5 |
| chain-1000 | 18.3 | 18.5 | 1.01 | 7.7 | 2.40 | 179,678 | 211,573 | 14.5 | 14.6 |
| producer-chain-700 | 24.7 | 25.9 | 1.05 | 9.8 | 2.65 | 42,445 | 48,256 | 24.3 | 24.1 |

The size ladder asks how a step scales, not only where it stands at one
size. Its programs are the series families' texts at the sizes named:
independent-negation at 8, 10, 12 and 14 nodes, transitive-path at 100,
200, 400 and 800 and chain at 1000, 2000 and 4000. Each executable runs
each program as `zetesis <program> --backend cpu --models 0 --workers 4
--stats` with the machine otherwise idle, once as a warmup and three times
timed; the figure is the median process wall time in milliseconds, and the
work is the charged closure work the statistics report, the same in every
run. The control is the executable of `a712f10d` (SHA-256
`54069a1381bcc5fc86c39fa1e2f5afd1a7c4ae6b16809a8fafb0b29791acae25`), the
branch before dense relations. [ladder-10644b3b.json](ladder-10644b3b.json)
holds every run. The ladder is rerun with each later step of the dense
round, all executables in one session, since times of different sessions
do not compare.

| Rung | control | dense | pending | dense/control | pending/dense | work control | work dense | work pending |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-negation-8 | 16.4 | 16.4 | 14.5 | 1.00 | 0.88 | 123,109 | 41,811 | 28,784 |
| independent-negation-10 | 50.4 | 48.5 | 37.9 | 0.96 | 0.78 | 425,724 | 136,012 | 91,687 |
| independent-negation-12 | 148.3 | 145.8 | 110.7 | 0.98 | 0.76 | 1,385,573 | 423,458 | 280,758 |
| independent-negation-14 | 445.6 | 450.4 | 340.8 | 1.01 | 0.76 | 4,354,196 | 1,298,863 | 857,156 |
| transitive-path-100 | 12.9 | 9.6 | 7.1 | 0.75 | 0.74 | 1,197,772 | 209,092 | 148,047 |
| transitive-path-200 | 37.3 | 27.3 | 16.0 | 0.73 | 0.59 | 5,378,096 | 955,649 | 804,204 |
| transitive-path-400 | 150.1 | 88.5 | 51.8 | 0.59 | 0.59 | 23,870,854 | 4,812,835 | 4,955,654 |
| transitive-path-800 | stopped | 358.1 | 202.9 | n/a | 0.57 | n/a | 27,231,857 | 33,800,382 |
| chain-1000 | 17.9 | 17.7 | 16.3 | 0.99 | 0.92 | 475,452 | 179,678 | 211,573 |
| chain-2000 | 33.0 | 31.0 | 32.4 | 0.94 | 1.05 | 1,025,311 | 578,537 | 736,073 |
| chain-4000 | 66.8 | 66.9 | 68.7 | 1.00 | 1.03 | 2,200,821 | 2,030,006 | 2,720,073 |

Every run exhausted its search and exited 0 except the control on
transitive-path-800, which stops at the default oracle work limit after
595 ms and has no ratio; both dense executables complete that rung. The
three timed runs of a cell agree within a few percent; on chain-2000 and
chain-4000 the ranges of `dense` and `pending` overlap, so those two
ratios do not differ from one.

The ladder answers the question it was set. The gain of the dense store
alone does not widen with size on the enumeration cells, 1.00, 0.96, 0.98
and 1.01 of the control, which is the reading that the cost per derived
atom dominated them; the pending bits remove that cost and the cells fall
to 0.88 at eight nodes and 0.76 to 0.78 from ten to fourteen, flat in
size because each closure stays small and only their number grows. On the
transitive cells the dense gain does widen, 0.75, 0.73 and 0.59, and the
pending bits take a further 0.74 to 0.57, so that the rung at 400 nodes
stands at 0.35 of the control. The chain shows no trend in either step.

Against the hypotheses. The enumeration cells were predicted at 0.80 to
0.90 and measure 0.76 to 0.88. The transitive cells were predicted at 0.85
to 0.95 and measure 0.57 to 0.74: building each derived atom, keeping it
in the round's tree set and ranking it a second time was a larger share
of that closure than the prediction allowed, and the step that remains
for those cells, the vector step, has less left to take than was thought;
they stand at 1.06 and 1.07 of the reference, from 1.44 and 1.53. The
chain was predicted near one and is. Two predictions were refuted. The
charged work was predicted to fall on every closure cell, and it rises on
the three chain rungs, on producer-chain-700 and on transitive-path-400
and -800 while their times fall or hold: sizing the pending rows charges a
unit a word, once for a workspace, and joining a round's marks charges the
span from the first marked word to the last, which on the transitive
cells is nearly the whole row every round, a cost of rounds times words,
eight hundred rounds of ten thousand words at 800 nodes. The vector step
writes whole rows and replaces that pass; until then the span is the
recorded cost of keeping one range, not a list of marked words. On
chain-4000 the two charges over the edge relation's 4,001 by 4,001
positions are a quarter of a million units each, and the pending row of
that relation is the half mebibyte by which chain-2000's resident set
rises, 21.0 to 21.5 MiB; the sparse-relation repair recorded with the
dense relations stands as it was, and the loss of 1.15 measured on
chain-2000 then does not recur, 0.94 of the control on the ladder. And
the scoreboard was predicted to stay at seven of twenty-two and reads ten:
independent-choice-16 at 0.93 of the reference is a gain of this step;
stratified-16 at 0.78 is a cell of three to four milliseconds whose
timings fall a millisecond apart, weak evidence; and disjunction-12 at
0.998 is a formula-route cell this step does not reach, a tie that fell
on the near side. The clause search reads six of twenty-two.

A correction to the section above. It names independent-choice-16 among
the formula cells, says the dense store does not reach it and reads its
0.78 as a slow `before` run. The cell takes the closure route under all
three executables, `oracle=closure` in their statistics, with its three
predicates laid out dense; its charged closure work is 10,427,471 at
`a712f10d`, 3,198,290 at `6f114811` and 2,117,111 here, over the same
2,584 checks and 91,926 derived atoms. The dense store reached it, and
the 0.78 was most probably its gain, as 0.73 is this step's. The earlier
timing itself is not re-established here, only the work. The remaining
cells that report no grounding time, independent-choice-12 and
stratified-16, are closure-route cells likewise. Of the formula-route
cells, all within 0.96 and 1.03 of `before`, chain-arithmetic-1000 stands
apart at 0.91, which this step cannot explain and which is recorded as
unexplained.

The corpus passed on both solvers in each of its three configurations,
all 94 cases: four workers under the default search, faster than the
reference on nine, median ratio 1.25, widest loss 1.97; one worker under
the default search, four, 1.29 and 2.06; one worker under `--search
clauses`, eight, 1.27 and 2.05, the clause search 1.02 of the default at
the median of the cases and between 0.75 and 1.29 on single ones. No
corpus case takes the closure route, every one reporting a grounding
time, so this step does not run on the corpus: the three runs establish
that it disturbed nothing there, and measure nothing of its gain. `before`
was not run on the corpus in this session, so the comparison with the
previous campaign is between sessions: the median case is 1.00 of its
earlier time and single cases lie between 0.82 and 1.26, which is the
spread of a case of five to seven milliseconds between sessions and the
reading of the widest loss, 1.74 then and 1.97 now on one shortest-path
case whose neighbour moved to 0.82.

In the problem's words: where a closure's time was building each atom it
derived, the enumeration and transitive cells, recording the atom as a bit
takes a quarter to two fifths of the time away, and takes the transitive
cells to within a tenth of the reference; where a closure derives one atom
a round, the chain, nothing moves; and the corpus, which does not use this
route at all, stands where it stood.

## The window's bound prefix, the row step, and the sparse-relation repair dropped

Seven campaigns and one ladder on 18 September 2026, the ladder written at
17:18:12 UTC and the reports between 17:18:39 and 17:20:38, same machine,
profile and clingo as above, one session. `before` is the pending-bit
executable of `10644b3b` (SHA-256
`7934b92d743525db2ebd9ad8274e67078497566cac68eeff67b284a182a88c69`);
`window` is built from `283180ae` (SHA-256
`fb58b495f447b9ff04e21e5a169cb1c2cf7c264f29496a8caceacb2908b078ac`), where
opening a window over a dense relation reads the bound prefix where it lies
instead of collecting it; `after` is built from `e98d9357` (SHA-256
`c1a2ed92849ccf0d8b0b37eb2d128a3e78b54caa2fb9bcfe5f244acd62d08aa7`), where
a rule whose innermost occurrence meets the row-step conditions joins the
block of matching rows into the head's pending row a word at a time, the step
the two sections above call the vector step;
`clauses` is that executable under `--search clauses`; and the corpus runs
are that executable on all 94 cases with four workers, with one worker, and
with one worker under `--search clauses`. Each executable was built from a
checkout of its own commit into its own target tree.
[series-e98d9357-cpu-auto.json](series-e98d9357-cpu-auto.json), its
[table view](series-e98d9357-cpu-auto-tables.md),
[corpus-e98d9357-cpu-auto.json](corpus-e98d9357-cpu-auto.json), its
[table view](corpus-e98d9357-cpu-auto-tables.md),
[corpus-e98d9357-cpu-auto-one-worker.json](corpus-e98d9357-cpu-auto-one-worker.json),
its [table view](corpus-e98d9357-cpu-auto-one-worker-tables.md) and
[ladder-e98d9357.json](ladder-e98d9357.json) are the derived comparisons and
the ladder's runs; raw report SHA-256
`6906f075b6bca8b7f89227d2afe8eb89b31d1f25d9f404ce8b81def66d71e5eb` (before),
`acb9d86c5b6139cf3c3bbd915cbbd715df6890ce6a019646adddf850b9f5b0f9` (window),
`ce5bdea4f5a5d9d64c9147dec0c03fd2dd858c990cee57a1c9ecf07f37fed173` (after),
`6a6eed12b065ac31d03697fec09481a1e8f94da1e6acd0847ecaae0500f95852`
(clauses),
`ead7eb9b859ffba650b677aaf739d1d18d09fcbbd41b21d790f9faf60e82bb0a`
(corpus, four workers),
`081754cc0c1ab1b8b7894c7855a1168c1fea68578efef8a1baded8e5c7ce5625`
(corpus, one worker) and
`64e90b5f70dea7c45e73b1d77d6bc7796c0d40cb90b5ed924090d6a53c4d010a`
(corpus, one worker, clauses).

The ladder is the one of the section above, all five executables in this
session, with three rungs added for the row step: transitive-complete at
40, 80 and 160 nodes, the facts `e(i,j)` for every `i < j` followed by the
two rules of transitive-path. It is given as facts, so it takes the closure
route, and a node has many edges, so a block of rows holds many bits; the
path graph of transitive-path holds one. These rungs are not series cells.
Median process wall time in milliseconds, and the blocks the last executable
joined:

| Rung | control | dense | pending | window | vector | window/pending | vector/window | vector/control | row steps |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-negation-8 | 17.1 | 16.5 | 13.9 | 13.0 | 14.1 | 0.94 | 1.08 | 0.82 | 0 |
| independent-negation-10 | 51.0 | 49.3 | 40.5 | 38.6 | 38.2 | 0.95 | 0.99 | 0.75 | 0 |
| independent-negation-12 | 152.0 | 150.6 | 114.3 | 112.9 | 113.3 | 0.99 | 1.00 | 0.75 | 0 |
| independent-negation-14 | 450.1 | 457.9 | 334.3 | 344.2 | 337.8 | 1.03 | 0.98 | 0.75 | 0 |
| transitive-path-100 | 12.6 | 9.4 | 7.0 | 7.1 | 6.0 | 1.02 | 0.84 | 0.47 | 4,851 |
| transitive-path-200 | 36.4 | 24.6 | 16.4 | 15.1 | 14.9 | 0.92 | 0.99 | 0.41 | 19,701 |
| transitive-path-400 | 151.4 | 88.4 | 53.5 | 49.9 | 46.4 | 0.93 | 0.93 | 0.31 | 79,401 |
| transitive-path-800 | stopped | 360.3 | 208.0 | 200.8 | 190.0 | 0.97 | 0.95 | n/a | 318,801 |
| chain-1000 | 18.0 | 15.8 | 18.0 | 16.7 | 16.6 | 0.92 | 1.00 | 0.92 | 0 |
| chain-2000 | 33.9 | 33.1 | 31.8 | 31.7 | 31.4 | 1.00 | 0.99 | 0.93 | 0 |
| chain-4000 | 68.5 | 67.9 | 67.0 | 65.7 | 66.9 | 0.98 | 1.02 | 0.98 | 0 |
| transitive-complete-40 | 16.5 | 15.7 | 15.2 | 15.4 | 13.1 | 1.02 | 0.85 | 0.79 | 741 |
| transitive-complete-80 | 67.0 | 62.3 | 57.6 | 58.1 | 49.1 | 1.01 | 0.85 | 0.73 | 3,081 |
| transitive-complete-160 | 358.4 | 309.9 | 296.9 | 287.3 | 215.0 | 0.97 | 0.75 | 0.60 | 12,561 |

Every run exhausted its search and exited 0 except the control on
transitive-path-800, as before. The charged closure work is the same under
`pending` and `window` on every rung, the window's change charging as it
did, and under `vector` it falls where blocks are joined: 148,047 to
111,564, 804,204 to 664,776, 4,955,654 to 4,382,983 and 33,800,382 to
31,419,753 on transitive-path, and 121,910 to 44,027, 1,066,060 to 415,182
and 10,807,373 to 5,474,591 on transitive-complete.

Against the hypotheses. The window's change was predicted at 0.95 to 1.00
and measures 0.92 to 1.03, within the few percent by which the runs of a
rung differ: it removes an allocation the function's contract already
denied, and its effect on time is not distinguishable from none. The row
step is taken exactly where it was predicted to be: nowhere on
independent-negation, whose gate reads the head's variable, nor on chain,
where `r` lists the values 0 to n and `e` lists 1 to n, and on every rung of
the two transitive families. On transitive-path it was predicted at 0.85 to
0.95 and measures 0.84, 0.99, 0.93 and 0.95: a block there holds one row, so
the step saves the binding of that row and no more. On transitive-complete
it was predicted at 0.3 to 0.6 of the run and measures 0.85, 0.85 and 0.75,
and the prediction is refuted for the run while it holds for the closure. At
160 nodes the solving stage falls from 98.1 to 22.2 ms, 0.23, over the same
695,360 bindings, with the rows offered to the matcher falling from 708,080
to 38,160; the run falls only to 0.75 because 166 of its 194 driver
milliseconds are source preparation, the reading and admission of 12,722
statements at thirteen microseconds each, which the closure does not touch.
The reference solves the whole of that program in 87 ms. A profile places
that time in ordering comparisons of whole statements, terms and symbols, in
the cloning and dropping of provenance and terms, and in the allocator under
them, in themelios's ingestion of the program as an ordered set of statements
and in this repository's normalization and fact expansion, which rebuild
every statement; the split between the two is not measured, the executables
carrying no frame pointers. For a program of many facts, reading it is now
the larger part of the run on this route. This is recorded as an
observation: no change to source preparation belongs to this tranche, and
what to do about it is left to the review that follows it.

The series is flat under both steps: `after` is between 0.95 and 1.08 of
`before` on every cell, the transitive cells at 0.99 and 0.98, and the one
excursion, `window` at 16.9 ms on transitive-path-100, is a median of runs
ranging from 9.1 to 19.3 ms that the ladder's 1.02 on the same rung does not
bear out. The scoreboard reads nine, twelve and eleven of twenty-two for
`before`, `window` and `after`, and seven for `clauses`; it does not resolve a
change here, several cells standing within two hundredths of the reference
and falling on either side from one run to the next, and `before` itself
read ten in the section above. The corpus passed on both solvers, all 94
cases in each of the three configurations: four workers, eight faster than
the reference, median ratio 1.26, widest loss 2.09; one worker, five, 1.26
and 1.99; one worker under `--search clauses`, six, 1.31 and 2.21. No corpus
case takes the closure route, so these runs establish that the two steps
disturbed nothing there. Against the campaign of the section above the median
case is 0.98 of its earlier time, single cases between 0.78 and 1.33, the
spread between sessions already recorded. The peak resident set is unchanged
under the three executables, chain-2000 at 21.3, 21.6 and 21.6 MiB.

A reversal. The section on dense relations named a repair for chain-2000,
then slower by 0.15 with its edge relation of two thousand atoms in a box of
four million positions: a ceiling that reads the fact count as well as the
product of the widths, or a row of the layout kept sparse, not to be taken
until the pending bits and the row step had been measured on the same cells.
They have been, and the repair is not taken. The loss has not recurred in
three sessions: the chain rungs stand at 0.92, 0.93 and 0.98 of the control.
What the dense store costs there is memory, half a mebibyte at 2,000 nodes
and two mebibytes at 4,000, and charged work for sizing and absorbing a wide
row, not time. Neither form of the repair is worth that. A ceiling that reads
a population makes the choice of store a heuristic with a constant to tune,
known at preparation only for facts and estimated for a derived relation, so
that a program's speed could change with the number of its facts and nothing
in it say why; and it would give back the gain it was meant to protect, since
the edge relation of transitive-path has the shape of chain's, n edges in a
box of n by n, and as a catalog it would lose its dense window and deny its
rule the row step, which asks that both relations be dense. A sparse row is
a third representation beside the catalog and the bit array, with its own
path through the join, the window, the absorbing of marks and the row step,
and its own argument, for a saving of mebibytes. What stands in its place is
the one rule there was: a predicate is dense when every argument is bounded
and the product of the widths fits `PreparationLimits::max_dense_atoms`. That
ceiling is explicit, accounted in the closure bytes and one number to lower.
The condition that would reopen this is a measured cost of the dense store,
in time or in memory, on a relation sparse in a wide box, that lowering the
ceiling does not remove.

In the problem's words: where a rule's last step reads a whole row of one
relation into a row of another, the round now does it a word at a time, and
the closure of a graph with many edges to a node takes a quarter of the time
it took; a graph with one edge to a node gains little, the series not at
all, and the corpus stands where it stood. What the ladder's new rungs
showed beside that is that reading a program of many facts now costs more
than closing it.

## A correction: a keyed constraint with an anonymous key

The review of the tranche as one change found that the keyed-constraint
rewrite of `808530bf` changed the answer sets of a program whose demanded
atom has an anonymous variable in a key position. For

```
letter(a;b). val(3;4).
1 { assign(K,V) : val(V) } 1 :- letter(K).
:- assign(_, Y), Y != 3.
```

clingo 5.8.2 and the executable of `896a5f73` give the one answer set
`assign(a,3) assign(b,3)`; the executable of `e98d9357` gave three, two of
them with a letter assigned 4, and reported one constraint asked by key. The
rewrite asked `:- letter(_), not assign(_, 3).`; under `not` an anonymous
argument reads "for no value at all", so the asked constraint forbade only
that no letter holds 3, where the written one forbids a value other than 3
at every letter. The recognition checked the value position of the demanded
atom and never its key positions. The digit-and-carry pattern had the same
hole. No test of the rewrite used an anonymous key, and no corpus case has
that shape, which is why the corpus passed atom for atom throughout; the
measurements of the keyed-constraints section stand, send-money naming every
key.

The rewrite now leaves such a constraint as written: a demand requires every
key position to be free of anonymous variables. Two contract tests state it,
one for each pattern, each against the same constraint with the key named,
which is asked and has the same answer sets. `KeyedConstraints.one_value` is
a law about one fixed key, and was never a law about this case; that the
recognition stays inside the law's premise is the Rust obligation the
correspondence names.

## The narrowing's closures prepared once

One ladder, two series campaigns and three corpus campaigns on 19 September
2026 UTC, the ladder written at 03:46:28, the series reports at 03:46:55 and
03:47:22 and the corpus reports between 03:47:35 and 03:48:01, same machine,
profile and clingo as above, one session. `before` is the executable of
`0b971adb` (SHA-256
`101a449b4d9b4bf3247394fd05718a7cf64bcb5a76f41e7e36cb0185f970787f`), the
head at the correction above; `after` is built from `9c0ec6d0` (SHA-256
`a9f308922432ae76f4506b68a7e0498fc887717ade3fe12f995bf34702272fbc`), where
the closure route's narrowing prepares its program once, when the bounds are
applied, and computes every closure of every region on that preparation in
one retained workspace, each closure charged as one candidate check with the
preparation apart, and where the one-shot doors of the first carrier-bound
step, which the narrowing had replaced and no session called, are gone. The
corpus runs are `after` on all 94 cases with four workers, with one worker,
and with one worker under `--search clauses`.
[series-9c0ec6d0-cpu-auto.json](series-9c0ec6d0-cpu-auto.json), its
[table view](series-9c0ec6d0-cpu-auto-tables.md),
[corpus-9c0ec6d0-cpu-auto.json](corpus-9c0ec6d0-cpu-auto.json), its
[table view](corpus-9c0ec6d0-cpu-auto-tables.md),
[corpus-9c0ec6d0-cpu-auto-one-worker.json](corpus-9c0ec6d0-cpu-auto-one-worker.json),
its [table view](corpus-9c0ec6d0-cpu-auto-one-worker-tables.md),
[ladder-9c0ec6d0.json](ladder-9c0ec6d0.json) and
[interleaved-9c0ec6d0.json](interleaved-9c0ec6d0.json) are the derived
comparisons, the ladder's runs and the re-measurement described below; raw
report SHA-256
`e97c414a05fa1dcc36990ebae9ec3eb96f69f730925a6a314cfb4c9496c1dc60` (before),
`9aca19f4e965dcdb06bb351c037fa6f57e14dd556115bea68e1825588dcb81e2` (after),
`e0fb34a1d672ad8d379c0b411df361acefaf0d2476310f916dcfac0c64b4249f`
(corpus, four workers),
`9765372e3c47392da0a67e64e530a6adaa3c331dd89f3ecfa775d7d58530f6c8`
(corpus, one worker) and
`f4c191ffb80d10a226f7b3bae463e6ba496b5e73a938048774c5eec204c9da15`
(corpus, one worker, clauses).

The review of the tranche as one change found that each narrowing pass ran
a region's two closures through a door that prepared the program from
nothing and allocated a fresh workspace, twice per pass, for the root and
for every region below it, while preparation had grown to infer the
argument bounds, lay out the dense relations and plan the row steps, and a
fresh workspace to allocate every dense layout's bit arrays and pending
rows. The hypotheses, written before the campaigns: the enumeration cells of
the closure route fall to between 0.75 and 0.90 of `before`, about the same
at every size, since the saved preparation and the closure it preceded grow
together with the program while only the number of regions grows with the
cell; transitive-path and chain stay within the noise of a rung, one region
and two or three passes; transitive-complete moves a few hundredths at most;
the formula-route cells and the corpus do not move; and every statistic the
runs print stays the same but the timings.

The ladder is the one of the sections above with the two executables of
this change alone, the dense round's executables having been retired when
its record closed, and its programs regenerated from the families' texts
and the description above. Median process wall time in milliseconds, and
the charged closure work, which is the same under both:

| Rung | before | after | after/before | closure work |
|---|---:|---:|---:|---:|
| independent-negation-8 | 13.3 | 9.6 | 0.72 | 28,784 |
| independent-negation-10 | 38.6 | 22.4 | 0.58 | 91,687 |
| independent-negation-12 | 111.9 | 71.1 | 0.64 | 280,758 |
| independent-negation-14 | 340.8 | 208.1 | 0.61 | 857,156 |
| transitive-path-100 | 7.4 | 6.5 | 0.88 | 111,564 |
| transitive-path-200 | 14.7 | 14.7 | 1.00 | 664,776 |
| transitive-path-400 | 44.8 | 46.5 | 1.04 | 4,382,983 |
| transitive-path-800 | 183.1 | 180.3 | 0.98 | 31,419,753 |
| chain-1000 | 17.7 | 16.7 | 0.94 | 211,573 |
| chain-2000 | 31.1 | 32.3 | 1.04 | 736,073 |
| chain-4000 | 66.5 | 67.7 | 1.02 | 2,720,073 |
| transitive-complete-40 | 12.8 | 14.3 | 1.12 | 44,027 |
| transitive-complete-80 | 50.5 | 49.8 | 0.99 | 415,182 |
| transitive-complete-160 | 219.7 | 220.9 | 1.01 | 5,474,591 |

Every run exhausted its search and exited 0.

Against the hypotheses. The enumeration cells fall further than predicted,
to 0.58, 0.64 and 0.61 from ten to fourteen nodes and 0.72 at eight, and
flat in size, as predicted: at fourteen nodes the narrowing visits 3,191
regions in 5,162 passes, 10,324 closures, each of which prepared the four
predicates' layouts and allocated their words before this change, a larger
share of a closure over a few rules than the prediction allowed. The
transitive-path and chain rungs stand within noise, 0.88 to 1.04, as
predicted: one region, two passes. Transitive-complete gains nothing where a
few hundredths were predicted; its rung at 40 nodes reads 1.12, which a
re-measurement alternating the two executables over five rounds of one
warmup and three timed runs each, retained beside the ladder, reads as 1.03
with the runs of the two overlapping, so it is not a change. Every closure
work count is the same under both executables, and so is every other
statistic the runs print.

The series is where the second refutation lies. Its closure-route cells,
with the charged closure work of one run and the peak resident set of the
memory round:

| Cell | before | after | after/before | clingo | after/clingo | work before | work after | MiB before | MiB after |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-negation-10 | 38.8 | 23.5 | 0.61 | 4.5 | 5.18 | 91,687 | 91,687 | 9.1 | 9.0 |
| independent-negation-8 | 14.2 | 9.4 | 0.66 | 4.6 | 2.05 | 28,784 | 28,784 | 9.2 | 8.5 |
| transitive-path-100 | 15.0 | 10.3 | 0.68 | 8.9 | 1.15 | 111,564 | 111,564 | 10.8 | 9.9 |
| stratified-16 | 3.6 | 3.4 | 0.96 | 4.4 | 0.78 | 1,669 | 1,669 | 9.6 | 8.4 |
| transitive-path-200 | 21.7 | 20.9 | 0.96 | 22.8 | 0.92 | 664,776 | 664,776 | 15.0 | 14.4 |
| chain-2000 | 32.3 | 32.5 | 1.01 | 9.9 | 3.29 | 736,073 | 736,073 | 22.2 | 21.4 |
| independent-choice-16 | 24.8 | 25.3 | 1.02 | 27.7 | 0.91 | 2,117,111 | 2,117,111 | 9.1 | 8.6 |
| chain-1000 | 17.3 | 18.5 | 1.07 | 6.6 | 2.79 | 211,573 | 211,573 | 15.2 | 14.6 |
| producer-chain-700 | 25.9 | 27.9 | 1.08 | 9.8 | 2.86 | 48,256 | 48,256 | 25.1 | 24.2 |
| independent-choice-12 | 5.7 | 6.8 | 1.20 | 6.7 | 1.01 | 227,172 | 227,172 | 8.9 | 8.4 |

The independent-negation cells fall as the ladder says, 0.66 and 0.61. The
independent-choice cells do not move, though they take the closure route
with regions: their narrowing decides nothing beyond each split, so the
tree is three regions and two passes below the root, counted, and the run
is the 377 or 2,584 candidate checks, which were prepared once already.
Their 1.20 and 1.02 here are medians of three runs of five and twenty-five
milliseconds, and the alternating re-measurement reads them at 1.04 and
0.97, the runs of the two executables overlapping. transitive-path-100 at
0.68 is the bimodal cell of the section above, a `before` median of 15.0
over runs from 8.5 to 16.0; on the ladder and alternated it reads 0.88 and
1.06. The formula-route cells, which this change does not reach, lie
between 0.98 and 1.10 with the `after` campaign run second, a band the
sections above also show, while the reference's own medians move between
the two campaigns by 0.80 to 1.20 of themselves on the smallest cells and
0.99 at the median. The scoreboard reads twelve of twenty-two for `before`
and ten for `after`; the two cells that moved, independent-choice-12 from
0.83 to 1.01 of the reference and disjunction-12 from 0.98 to 1.03, are
ties that fell on the other side. The peak resident set is unchanged,
chain-2000 at 22.2 and 21.4 MiB: the retained workspace is one closure's.

The corpus passed on both solvers, all 94 cases in each of the three
configurations: four workers, faster than the reference on nine, median
ratio 1.29, widest loss 2.13; one worker, eight, 1.29 and 1.88; one worker
under `--search clauses`, six, 1.30 and 2.05, the clause search 1.01 of the
default at the median of the cases and between 0.74 and 1.41 on single
ones. No corpus case takes the closure route, so these runs establish that
the change disturbed nothing there; against the campaign of the section
before the correction, the median case is 1.00 of its earlier time, single
cases between 0.84 and 1.37, the spread between sessions already recorded.

In the problem's words: where the seed counter narrows region by region and
the regions are many, each closure no longer prepares the program and
builds its workspace anew, and those cells run in three fifths of the time;
where the narrowing decides nothing beyond a split, or there is one region,
the closures were few and nothing moves; and the corpus, which does not use
this route, stands where it stood.

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

## The review's remedies at the head, and a hasher measured and reversed

Two campaigns, each of two series runs, two corpus runs and a second series
pair, on 19 September 2026 UTC, same machine, profile, four workers and
clingo as above, the machine otherwise idle. `before` is the executable of
`9c0ec6d0` (SHA-256
`a9f308922432ae76f4506b68a7e0498fc887717ade3fe12f995bf34702272fbc`), the
one the sections above record. The first `after` is built from `669b23cd`
(SHA-256
`90355121932eb515fe1b4a28dfef91a382b8bd26656962ed89677bf7dadcbca0`), the
head of the third audit's review remedies, 123 commits after `before`,
among them the source-preparation fast path on plain statements
(3c007684), the keyed rewrite prepared once (512c80db) and the renames of
the one-name rule; its reports were written between 23:11:57 and 23:13:15
and the second pair's, run after, before, after, before, between 23:14:19
and 23:15:52. The second `after` is built from `77fb091f` (SHA-256
`37a940dd51d41408152c91b6a267ebb1add93933c9654ad29fb707c346a761bd`), one
commit later, the reversal described below; its reports were written
between 23:46:40 and 23:47:57 and its second pair's between 23:48:00 and
23:49:31.
[series-669b23cd-cpu-auto.json](series-669b23cd-cpu-auto.json), its
[table view](series-669b23cd-cpu-auto-tables.md),
[corpus-669b23cd-cpu-auto.json](corpus-669b23cd-cpu-auto.json), its
[table view](corpus-669b23cd-cpu-auto-tables.md),
[series-669b23cd-cpu-auto-recheck.json](series-669b23cd-cpu-auto-recheck.json)
and its [table view](series-669b23cd-cpu-auto-recheck-tables.md) are the
first campaign's derived comparisons;
[series-77fb091f-cpu-auto.json](series-77fb091f-cpu-auto.json), its
[table view](series-77fb091f-cpu-auto-tables.md),
[corpus-77fb091f-cpu-auto.json](corpus-77fb091f-cpu-auto.json), its
[table view](corpus-77fb091f-cpu-auto-tables.md),
[series-77fb091f-cpu-auto-recheck.json](series-77fb091f-cpu-auto-recheck.json)
and its [table view](series-77fb091f-cpu-auto-recheck-tables.md) the
second's. Raw report SHA-256, first campaign:
`66c1534ae956174fff3ef7c78229ff6a21fb8edbd74e0150c59cbb35cb65f1d6`
(series, before),
`3cc105750528edcf0517bbe0caadfb9583bf7b6b5e47b21babb6990b578f9db2`
(series, after),
`33267d6e88558bfb7327c941cf961dd481723cde54c5f28bbf1046542a81347f`
(corpus, before),
`19fe2ad807eb4a40abde050a68d04f644faec3a9af7b2e865bb4ad7aade87986`
(corpus, after), and its pair
`ec711a38b3075182d33c3da586709194f5ec258ca3dbe2c69ae275a8a0a24907`,
`c62fc5ccbd0519b706690e9d746802eef07eac56a1f1eebaa013c7f16eee0de1`,
`cc6e091d78098339f5778dcaa31797defcf2a0b8a1cdb3a046529354980289cb` and
`1dc01dcb96ad93352675adbc9ed3e56c0bc2d7db81bfe4fe2fcd88845f31bb1a`
(after, before, after, before); second campaign:
`b745aabbd2f951f42282d1a1e780ee4a9b2e1026329dd85d6bc01ce464aa0b38`
(series, before),
`1a3d7139469fd23f583017a9c26f0e53969e6d4b563581e8c1d7cd5ce9df6ad5`
(series, after),
`63c1e31cb1b9db14eee43615fa119f30b78f7a7e35c9540c15b222281510727e`
(corpus, before),
`35964403d3b13b896b86a96dd4c500bf781c3ae855116642690463a013ccb7ba`
(corpus, after), and its pair
`2b99876514e6034631f874219c1d969392901a702ad26850810f5fdbca978cf9`,
`4e13b2c341e4a08ddc9696bce861d756747e8f4d8da82a197814f6aece6b22f3`,
`74b79182b0968335143e1906740906af26b105fb625333314ebb0b99407253c0` and
`601774897b1e03399f530c582460e79cc1e35358924681e4994fcccd4ed40d93`
(after, before, after, before).

Six hypotheses were written before the first campaign. That the
closure-route cells of plain facts and rules, whose source preparation was
three quarters of their time at `before`, would fall to 0.60 to 0.75 by the
fast path; that the cells whose time is the search would stay within 0.95
to 1.05; that send-money and the other keyed cells would keep their time
with the second preparation pass gone; that the corpus would pass 94 of 94
and every other cell stay within noise; that every statistic but the
timings, the receipts of the keyed programs and the renamed keys would
print the same; and, added before the campaign for the day's fifteen late
remedies, that none of them would move a cell, the record's atom table
hashing with the randomized hasher among them, "which touches the writing
of the records the harness captures and nothing the solver times".

The first campaign bore out the first and third and the corpus, and refuted
the sixth. The chain cells fell, less than predicted at the smallest size:

| Cell | before | 669b23cd | after/before | pair | 77fb091f | after/before | pair |
|---|---:|---:|---:|---:|---:|---:|---:|
| chain-1000 | 19.5 | 16.3 | 0.834 | 0.777 | 15.4 | 0.880 | 0.821 |
| chain-2000 | 38.1 | 28.1 | 0.739 | 0.901 | 28.7 | 0.844 | 0.839 |
| producer-chain-700 | 27.4 | 20.4 | 0.745 | 0.808 | 21.6 | 0.840 | 0.870 |
| send-money/send-money | 13.1 | 14.2 | 1.087 | 1.009 | 14.1 | 1.034 | 0.971 |

(medians in milliseconds; `pair` is after2 over before2 of the second
pair of that campaign). The corpus compared 94 cells of 94 in both
campaigns, the native executable ahead of clingo on 8 cells before and 12
after in the first, 9 and 9 in the second, its cells of five to fifteen
milliseconds moving both ways within the spread such cells show between
two runs of one executable. But the cells the regions method decides were
slower at `669b23cd`, and slower again in its pair:

| Cell | before | 669b23cd | after/before | pair | 77fb091f | after/before | pair |
|---|---:|---:|---:|---:|---:|---:|---:|
| planning-14 | 38.3 [37.6, 39.6] | 44.3 [43.2, 50.2] | 1.157 | 1.090 | 36.5 [36.0, 36.9] | 0.972 | 0.984 |
| n-queens/variant-01 8→11 | 89.0 [87.7, 93.6] | 97.4 [95.4, 97.4] | 1.094 | 1.083 | 89.9 [88.2, 92.5] | 0.995 | 1.033 |
| n-queens/variant-04 8→11 | 82.5 [82.4, 85.0] | 85.1 [84.8, 85.4] | 1.032 | 1.075 | 81.7 [80.6, 82.0] | 1.005 | 1.042 |
| disjunction-12 | 23.5 [22.5, 24.2] | 24.2 [23.5, 26.1] | 1.029 | 1.073 | 22.0 [21.8, 22.1] | 0.965 | 1.009 |
| transitive-dense-40 | 34.2 [33.8, 38.8] | 36.6 [36.6, 38.1] | 1.071 | 1.057 | 33.3 [33.0, 33.3] | 0.997 | 0.972 |

A bisection of the 123 commits between `before` and `669b23cd`, each step
a release build of the command and one series run, judged by planning-14's
median (at or under 41 ms good, above bad), took seven steps and named
`9b39079c`, the commit that had put the standard library's randomized
hasher on the JSON document's atom table for the atoms a program's author
spells:

| Commit | planning-14 |
|---|---:|
| bfc8443a | 38.8 |
| 9676f57e | 37.5 |
| 521770d2 | 38.2 |
| d991e5ec (the parent) | 37.7 |
| 9b39079c (the hasher) | 43.3 |
| 1f46103e | 42.5 |
| b31549c8 | 42.4 |

(raw report SHA-256, in that order,
`3eb501b74a3ff6dcf20a0ade7f839ad7522587c2dd341c2f6dab9c84e4eb8c22`,
`be6e299eba2c3b1421c5f9ba32443f984d94eeb7722f72233f1f54765f26c2ad`,
`48949a818c2a24d5ec887fadf11da5e9e6b7d459905043d09e3e26541c0afeba`,
`ff8d58cbbb458a0a0a95b70be4486b6be17b927035de83a797afe8b3dae1ce41`,
`366a01ceab041398282da05e69736345679ceb7d8f73efb554e3c146101e7a24`,
`6c14e745455b64e0e5cb75be3bcbb08dbc6b38fe15fecceb6807ab91ddf8a001` and
`7bb4d8e1b6bcabeae2cf60dcf9eca80238e6d580840eee600adac7af833c2d29`).

In the problem's words: the writer of the JSON document looks every atom
of every model up in the document's table once on the way out, to refer
to it by index rather than spell it again, and the cells that lost nine to
sixteen percent are the ones that write many models with many atoms;
hashing an atom's name and values with SipHash costs what the crate's word
hash, a few multiplies per word, does not, and the chain cells, which
write little, kept their gain. The hasher had been changed because the
word hash's module claimed its keys are not adversarial, which atoms an
author spells are not. The change is reversed at `77fb091f` with the
argument the first change lacked: a collision an author arranges costs a
lookup a scan of the table, bounded by its ceiling of distinct atoms, and
the solving his program commands already costs him more than any table
could. At `77fb091f` every series cell but one lies within 0.95 to 1.05 of
`before` or below it (independent-negation-10 at 1.054), the queens cells
at 1.00 to 1.04 across the campaign and its pair, within the spread the
cells show between two runs of one executable (chain-arithmetic-1000 at
0.904 and 1.105 in that pair). Every statistic the runs print is the same
line for line but the timings, the receipts of the eleven keyed programs
and the renamed keys, as the fifth hypothesis said.

## Keys renamed since these records

The records above keep the spellings of their day. A later record spells these keys as the second column says.

| In these records | Since |
| --- | --- |
| `necessary_gate_atoms`, `underivable_gate_atoms` under `carrier_bounds` | `held_gate_atoms`, `cut_gate_atoms` |
| `carrier_bounds` with `narrowing_passes` | `carrier_narrowing` with `passes` |
| `regions`, `reduct_regions` under `search` | `candidate_regions`, `reduct_query_regions` |
| `decided` under `carrier_regions` | `leaves` |
| `row_steps` under `closure joins` | `block_steps` |
