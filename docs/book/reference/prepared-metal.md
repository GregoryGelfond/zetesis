# Prepared grounding: CPU and Metal

The prepared-grounding changes reduce repeated work in independent CPU closure.
The measurements here show no general Metal speedup or reduction in GPU
transport storage. Eager timings include some regressions and substantial block
variation. These results complement the
[ordinary CPU timing and RSS comparison](performance.md#prepared-grounding-cpu-comparison);
they use different output and execution profiles.

## Subjects and measurement scope

Measurements ran on Apple M4 Pro / Metal, macOS 26.6.2, on 14 September 2026.
The three measured implementations all report **0.1.0**:

| Role | Implementation | Source |
| --- | --- | --- |
| Q | Atom catalog | [`ca10a5e7`](https://github.com/GregoryGelfond/zetesis/tree/ca10a5e7ec84e13fbcc4a23bd0de8b0232c53fe1) |
| A | Reusable query workspaces | [`f56a5a24`](https://github.com/GregoryGelfond/zetesis/tree/f56a5a2496f519d7b71b7c4c8fdc166c355874ff) |
| B | Prepared queries and independent delta closure | [`679ca856`](https://github.com/GregoryGelfond/zetesis/tree/679ca8568a6fd8577d9b944fbd99d7c54f666601) |

Each uses Rust 1.97.1 and ordinary release optimization. Q/B used the explicit
Apple-target recipe; A used the native-target recipe. The retained records do
not establish identical transitive build recipes. These are integrated revision
comparisons, not isolated algorithm measurements. Native executable identities
are listed with the [CPU comparison](performance.md#prepared-grounding-cpu-comparison).

The acquisition order was Q/A/B/B/A/Q. Eager matrices ran first, followed by
six lazy-library commands. The complete interval was 18:30:46–18:39:45 UTC;
the lazy commands occupied its final five seconds. Host load averages changed
from 2.94/3.78/3.38 to 4.74/4.97/4.55. Small differences and block drift must be
retained. No kernel timestamps, process RSS or cold-cache guarantee are supplied
by this acquisition.

## Complete eager solves

Each revision block ran the baseline suite (SEND, queens 02 and task allocation)
and all six N=8 queens encodings. Queens 02 therefore appears in two separately
timed suite entries: nine entries represent eight distinct programs. Every entry
has CPU, Metal and direct clingo observations: qualification, one warmup and
three timed runs.

All **810 positions passed**, with no capture faults or unresolved children.
The 540 native observations retain 109,260 full model records. Their typed atom
families, displayed selections, costs and multiplicities agree across revisions,
CPU/Metal routes and repetitions. All 270 reference observations are exhaustive;
the maintained comparison reconciles clingo's optimum-discovery replay before
comparing the final optimum ties. Hidden clingo interpretations are unavailable.
All 270 Metal captures record actual device work.

CPU `Auto` selects tight-support checking; Metal `Auto` selects the countermodel
procedure with exact CPU completion of residuals. Thus CPU/Metal wall times
compare available product routes, not the same isolated kernel. Each time
includes fresh-process spawn, JSON/statistics output, capture and reap. Comparison
and hashing follow that interval. The ordinary human-output CPU/RSS measurements
are a separate population.

The [eager timing observations](observations/prepared-metal-eager-20260914.tsv)
retain six exact timed intervals per implementation/suite/case/producer, split
into the two actual blocks, with their report hashes and median/minimum/maximum.
On clingo rows, `native_sha256` identifies the accompanying native implementation;
clingo itself is the same executable throughout. The fixed B performance runner
hashes to `c83264ce4f459287d79042061a0320884117385ff876185373ae5ba168594d89`;
direct clingo 5.8.2 hashes to
`31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015`.

Values below are milliseconds: the midpoint of the two block medians, each
computed from three timed samples. They are not pooled six-sample medians.

| Case | CPU Q | CPU A | CPU B | Metal Q | Metal A | Metal B |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| SEND | 36.811 | 38.694 | 39.252 | 64.432 | 65.755 | 66.967 |
| Queens 2, baseline suite | 93.357 | 95.908 | 96.509 | 107.927 | 111.058 | 110.988 |
| Task allocation | 319.970 | 323.689 | 322.570 | 563.166 | 568.298 | 562.423 |
| Queens 1 | 19.557 | 19.311 | 19.259 | 34.030 | 40.022 | 34.203 |
| Queens 2 | 94.431 | 95.886 | 96.409 | 110.089 | 110.283 | 111.483 |
| Queens 3 | 19.453 | 19.185 | 19.824 | 35.424 | 34.886 | 34.823 |
| Queens 4 | 12.048 | 11.661 | 11.620 | 26.366 | 26.090 | 26.043 |
| Queens 5 | 51.826 | 48.290 | 49.054 | 69.374 | 65.130 | 65.786 |
| Queens 6 | 53.299 | 49.388 | 49.287 | 69.954 | 66.407 | 67.138 |

The reference executable is unchanged; its three columns retain the matched
acquisition intervals rather than pooling them:

| Case | clingo with Q | clingo with A | clingo with B |
| --- | ---: | ---: | ---: |
| SEND | 12.159 | 11.579 | 11.525 |
| Queens 2, baseline suite | 118.869 | 122.788 | 123.307 |
| Task allocation | 175.938 | 181.124 | 181.800 |
| Queens 1 | 6.409 | 6.540 | 6.534 |
| Queens 2 | 121.474 | 123.214 | 122.675 |
| Queens 3 | 6.395 | 6.500 | 6.522 |
| Queens 4 | 6.360 | 6.528 | 6.511 |
| Queens 5 | 6.430 | 6.607 | 6.573 |
| Queens 6 | 6.453 | 6.595 | 6.588 |

B/Q SEND increases 6.63% on CPU and 3.93% on Metal; task allocation changes
+0.81% and −0.13%. Every matched Metal block/case median exceeds its CPU median.
Several apparent improvements depend on drift: Q queens 05 CPU rises
47.801→55.851 ms between blocks, and A queens 01 Metal rises 33.034→47.011 ms.
The latter dominates the apparent 14.54% B/A improvement. These observations
establish neither a broad speedup nor precise small-effect estimates.

SEND's single candidate and all 92 queens candidates are decided on the GPU.
Task allocation submits 1,208 candidates in 19 batches; 1,194–1,196 candidates
require exact CPU completion. That partition varies slightly, but every
candidate settles and the final 1,176 optimum ties at cost 5 agree. Its roughly
3.024 billion charged GPU work units are algorithmic accounting, not hardware
instructions. No reduction in peak process memory follows from these records.

## Lazy library batches

The [exact timed observations](observations/prepared-metal-lazy-20260914.tsv)
retain all 3,456 integer nanosecond intervals. Their
[provenance](observations/prepared-metal-lazy-provenance-20260914.tsv) binds the six
blocks to source revisions, executable hashes and original stream hashes.
Iteration, position and timed-order fields are zero-based; timed order follows
the producer's rotated route schedule within each block.

Each block has eight fixtures: sparse/dense families, widths 4/8 and candidate
batches 1/32. Six routes run on each fixture: scalar, Rayon, portable Union,
portable Worlds, Metal Union and Metal Worlds. All 4,320 initial, warmup and
timed slots completed, including 1,440 Metal slots and 1,152 timed Metal slots.
Before publishing each sample, the maintained producer compares complete
closures, constraint violations and seed mismatches against its scalar reference.
The streams retain the successful checks, not the closure contents themselves;
this is neither an independently reconstructed cross-version closure comparison
nor complete outer answer-set enumeration.

The following percentages compare medians of each revision's 24 timed samples
per fixture and route. Negative means B took less time. Unrelated fixtures are
not pooled.

| Metal route | B faster than Q | B/Q range | B faster than A | B/A range |
| --- | ---: | ---: | ---: | ---: |
| Union | 4 of 8 | −6.60% to +2.39% | 4 of 8 | −9.36% to +1.93% |
| Worlds | 3 of 8 | −0.71% to +3.11% | 2 of 8 | −15.66% to +2.52% |

Every B/Q Metal sample range overlaps. Sparse width-4/batch-1 has both B block
medians above both Q medians: pooled Union is 0.632042→0.647146 ms and Worlds
0.627271→0.646750 ms. These are small absolute regressions, not omitted samples.
Dense width-8/batch-32 Union is 4.040084/4.162833/3.773334 ms for Q/A/B, but B's
two block medians differ: 3.622041 and 4.144916 ms. This short acquisition does
not establish a stable small-effect estimate.

Metal Union takes 1.97–25.69 times as long, and Metal Worlds 1.83–30.77 times
as long, as their matched portable shared-source routes, comparing B medians.
Metal does not take longer than every CPU strategy. For dense
width-8/batch-32, B medians are:

| Scalar | Rayon | Portable Union | Portable Worlds | Metal Union | Metal Worlds |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 13.506167 ms | 3.747646 ms | 1.918437 ms | 1.979938 ms | 3.773334 ms | 3.615667 ms |

Device setup is outside these sample timers. Its six observations are
48.441/42.354/47.270/9.554/8.793/8.479 ms in acquisition order. No particular
cache or driver cause is inferred. Timed Metal work includes host source
evaluation, buffer work, dispatch and readback. The reported host wait also
includes decoding; it is not GPU kernel time.

### Work and storage

All non-time device fields match across Q/A/B and repetitions. A/B source
receipts match; they count 33 more source-work units per batch than Q. That
accounting difference is not a count of extra GPU instructions. The GPU paths
retain their shared source schedule; they do not yet consume the independent
CPU delta schedule.

Each batch submits three or seven chunks. Uploads range from 1,304 to 53,896
bytes, decoded outputs from 48 to 22,272 bytes, and named device transport peaks
from 496 to 25,360 bytes. These populations do not decrease in B. The existing
Worlds route does reduce sparse width-8/batch-32 work relative to Union:
1,121→113 instances, seven→three dispatches, 53,896→3,912 uploaded bytes and
25,360→2,856 transport bytes. Those reductions already occur in Q and A.

Independent CPU closure is different. For dense width-8/batch-32, A→B reduces
counted work from 2,981,376 to 2,310,016 and bindings from 35,872 to 17,440,
preserving 96 summed rounds and 17,408 derived-atom occurrences. Maximum
individual closure capacity increases by 320 bytes, to 138,142 bytes. These
named capacities exclude other host owners and driver allocations; they neither
measure RSS nor form additive whole-process memory totals.

## Reproduction

Use separate checkouts and frozen binaries for Q/A/B, and one fixed performance
runner from B. Run the two suites for each block in Q/A/B/B/A/Q order, retaining
each report under a distinct name. For example:

```sh
zetesis-perf examples/kr-domains \
  --zetesis /path/to/frozen/zetesis --clingo /path/to/direct/clingo \
  --report target/metal-baseline.json --suite baseline \
  --profile cpu-eager --profile metal-eager --formula-joins indexed \
  --workers 4 --completion-workers 4 --clingo-workers 1 --batch-size 64 \
  --warmups 1 --repetitions 3 --timeout-seconds 10 --campaign-seconds 90 \
  --sample-bytes 33554432 --native-report-bytes 33554432 \
  --capture-bytes 134217728 --report-bytes 268435456
```

Repeat with `--suite queens` and another report path. These are finite limits;
a refusal or partial run remains an outcome rather than a discarded sample.
Then run each revision's own frozen bench, in the same block order:

```sh
zetesis-bench lazy --backend metal --widths 4,8 --batches 1,32 \
  --families sparse,dense --workers 4 --warmups 2 --repetitions 12 \
  --chunk-rules 256 --max-work 100000000
```

The lazy producer also applies finite candidate, atom, round, host/device byte
and GPU wait limits. They do not establish a whole-process wall deadline. Keep
competing builds and measurements stopped and retain actual adapter and work
observations. Rebuilding the same revision need not reproduce its binary bytes.

## Version 0.1.1 qualification

The later release build from
[`9b8cf74c`](https://github.com/GregoryGelfond/zetesis/tree/9b8cf74c818b884b2a7510ec6d98b0ee0873d6cb)
was checked independently. Version and statistics report 0.1.1; the baseline
CPU/Metal/clingo matrix passed all 18 qualification and single-timed positions.
Complete native families agree for SEND, queens 02 and task allocation, with
real Apple M4 Pro work. This single-sample characterization supplies no stable
before/after performance estimate. It does not relabel the Q/A/B measurements.
The [coverage snapshot](validation.md#coverage) separately records 56 maintained
physical tests and their exact compiled source.
