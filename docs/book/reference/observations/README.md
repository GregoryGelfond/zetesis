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
