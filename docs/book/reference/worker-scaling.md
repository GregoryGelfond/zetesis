# CPU and Metal worker scaling

This comparison measures all 94 corpus cases before and after two changes to
native CPU worker coordination. Four workers give the best observed times for
queens variant 2 and the larger task-allocation case. Fourteen workers show a
clear queens variant 2 improvement over the earlier implementation, with higher
process memory. The full suite shows smaller CPU changes and no established
Metal improvement. Stock clingo is faster on most individual cases.

## How to read this comparison

The question is whether reducing worker coordination makes a complete solve
faster, and how the result compares with clingo. Both zetesis versions run the
same programs with 1, 2, 4 or 14 host threads. Metal still uses host threads for
candidate generation; this number is not a count of GPU execution units.

**A** is the version before the change; **B** is the version after it. Runs were
ordered **A1, B1, B2, A2** to help reveal changes in machine conditions. Each
table keeps those groups separate. The reported time includes process startup,
input, eager grounding, solving, statistics and output. Clingo runs use one
thread and request equivalent completed answers and optimum ties.

Start with the totals below, then the examples and memory comparison. The
[method and data](#sources-execution-and-evidence) give the compiled sources,
complete per-program tables and measurement settings. These results describe
those builds on this machine, not every program or newer zetesis version.

## Full-suite process time

Each entry below is the **sum of 94 per-case wall medians**, in milliseconds.
It is neither campaign duration nor a median of pooled cases. Each leg's median
uses its two timed observations. Clingo columns are fresh one-worker observations
from the corresponding B legs; all four reference legs are in the linked tables.

| Backend | Workers | A1 | B1 | B2 | A2 | Clingo B1 | Clingo B2 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| CPU | 1 | 889.773 | 859.803 | 853.530 | 876.073 | 735.206 | 732.046 |
| CPU | 2 | 792.894 | 790.048 | 780.561 | 784.684 | 732.025 | 734.340 |
| CPU | 4 | 760.825 | 755.565 | 738.613 | 743.377 | 740.988 | 748.740 |
| CPU | 14 | 925.091 | 904.926 | 903.965 | 916.879 | 744.219 | 753.997 |
| Metal | 1 | 2247.295 | 2162.043 | 2200.097 | 2192.271 | 778.422 | 789.231 |
| Metal | 2 | 2152.012 | 2160.661 | 2249.466 | 2151.752 | 795.283 | 833.350 |
| Metal | 4 | 2110.945 | 2227.635 | 2218.886 | 2120.381 | 822.705 | 825.508 |
| Metal | 14 | 2316.607 | 2454.169 | 2470.070 | 2441.586 | 838.640 | 836.167 |

At four CPU workers, these sums fall 0.69% for B1/A1 and 0.64% for B2/A2.
Fourteen workers give larger reductions, 2.18% and 1.41%, but remain slower than
four workers. Scalar sums also fall 3.37% and 2.57%, although scalar execution
bypasses the changed worker pool. This control limits attribution of small
aggregate differences to the two synchronization changes.

Native wins against contemporaneous clingo are unchanged between revisions:
3, 4, 4 and 3 of 94 cases for CPU at 1, 2, 4 and 14 workers; Metal wins 2, 3, 3
and 2. A near-equal full-suite sum does not mean near-equal performance on each
case: a few expensive wins can offset many slower small cases.

At four workers, no candidate case is ten times slower than its matched clingo
median in either B leg. The largest CPU ratio is 1.988; none reaches 2.0.
Metal has 90 cases at least twice as slow in both B legs, with a maximum ratio
of 5.231. These thresholds describe this corpus and instrumentation, not
unmeasured larger instances. Ratio and absolute time loss answer different
questions: a large ratio on a short case can reflect a shared fixed cost.

## Domain totals at four workers

Pairs below are B1 / B2 sums of per-case medians in milliseconds. The two clingo
columns belong to their respective CPU and Metal acquisitions; they are not
interchangeable reference measurements.

| Domain | Cases | CPU | Clingo beside CPU | Metal | Clingo beside Metal |
| --- | ---: | ---: | ---: | ---: | ---: |
| Equality generalized TSP | 3 | 21.993 / 20.864 | 11.728 / 12.343 | 60.857 / 61.587 | 14.072 / 14.829 |
| Shortest path | 35 | 244.196 / 241.242 | 150.781 / 150.625 | 765.646 / 763.281 | 179.387 / 181.864 |
| Task allocation | 20 | 162.417 / 160.790 | 290.045 / 300.117 | 499.335 / 497.540 | 307.494 / 310.046 |
| Traveling salesman | 29 | 207.729 / 201.640 | 128.167 / 125.102 | 640.274 / 637.936 | 149.439 / 146.933 |
| N-queens | 6 | 107.787 / 102.517 | 149.459 / 149.116 | 229.000 / 226.204 | 160.076 / 159.610 |
| SEND + MORE = MONEY | 1 | 11.443 / 11.559 | 10.807 / 11.437 | 32.524 / 32.338 | 12.238 / 12.225 |

All 35 shortest-path cases are slower than clingo in these legs. Their CPU
medians span 6.321–9.640 ms, against 3.875–5.812 ms for clingo across the four CPU
legs. Small process-time differences need particular care at this duration.

## Selected cases at four workers

Queens use the unchanged N=8 inputs. Each encoding publishes 92 answers. SEND
publishes one answer. Task allocation means
`scenarios/task-allocation/variant-04/05-larger-mix.lp`, with 1,176 optimum ties
at priority 0 and cost 5. Before pairs are A1 / A2; after and clingo pairs are
B1 / B2. All times are process wall medians in milliseconds.

| Case | CPU before | CPU after | Clingo beside CPU after |
| --- | ---: | ---: | ---: |
| Queens 01 | 8.888 / 9.017 | 8.985 / 9.012 | 5.188 / 5.174 |
| Queens 02 | 67.162 / 58.924 | 62.702 / 57.982 | 122.577 / 123.220 |
| Queens 03 | 10.342 / 9.731 | 10.262 / 10.337 | 5.167 / 5.248 |
| Queens 04 | 7.763 / 7.741 | 7.664 / 7.749 | 5.670 / 5.167 |
| Queens 05 | 9.094 / 9.021 | 9.087 / 8.373 | 5.700 / 5.159 |
| Queens 06 | 9.060 / 9.038 | 9.086 / 9.063 | 5.157 / 5.147 |
| SEND | 11.534 / 11.473 | 11.443 / 11.559 | 10.807 / 11.437 |
| Task allocation | 31.392 / 31.945 | 31.259 / 31.223 | 174.755 / 185.315 |

| Case | Metal before | Metal after | Clingo beside Metal after |
| --- | ---: | ---: | ---: |
| Queens 01 | 25.441 / 26.497 | 27.431 / 27.436 | 6.197 / 5.979 |
| Queens 02 | 85.122 / 85.447 | 92.563 / 90.679 | 128.929 / 128.940 |
| Queens 03 | 26.736 / 25.931 | 27.460 / 27.432 | 6.277 / 6.262 |
| Queens 04 | 24.209 / 24.409 | 24.431 / 24.431 | 6.228 / 6.003 |
| Queens 05 | 26.708 / 27.306 | 28.167 / 27.456 | 6.221 / 6.206 |
| Queens 06 | 27.363 / 27.434 | 28.947 / 28.771 | 6.223 / 6.220 |
| SEND | 31.766 / 30.490 | 32.524 / 32.338 | 12.238 / 12.225 |
| Task allocation | 93.766 / 99.426 | 102.409 / 100.703 | 175.548 / 177.966 |

Increasing workers is not monotonically beneficial. After the change, queens 02
CPU medians are 134.562–136.395 ms at one worker, 90.885–90.910 at two,
57.982–62.702 at four and 143.229–147.742 at fourteen. Task allocation takes
51.332–51.896, 38.096–38.892, 31.223–31.259 and 51.928–53.393 ms respectively.
These are observed leg-median ranges, not recommendations for every workload.

## Memory and attribution

RSS below is the two separate B observations in MiB, not device memory or a
simultaneous process-tree sum. Each clingo range comes from the same B legs.

| Case, four workers | CPU RSS | Clingo beside CPU | Metal RSS | Clingo beside Metal |
| --- | ---: | ---: | ---: | ---: |
| Queens 02 | 22.13–22.17 | 8.72–8.91 | 33.89–34.25 | 8.80–9.14 |
| SEND | 18.05–18.66 | 8.48 | 29.23–29.45 | 8.48 |
| Task allocation | 23.00–23.05 | 19.73–19.75 | 33.47–33.75 | 19.88–21.38 |

The clearest CPU gain is queens 02 at fourteen workers: 155.997 / 157.353 ms
before and 143.229 / 147.742 after, reductions of 8.18% / 6.11%. All four after
timings, 141.309–150.015 ms, are below all four before timings,
154.542–158.832 ms. RSS rises from 28.72–30.64 to 32.81–34.67 MiB. The work
remains exactly 15,076,428 and the candidate count 92. This is a time/memory
tradeoff; it does not establish isolated mutex latency or a general memory saving.

Queens and SEND retain identical work within each backend. Parallel optimization
can change incumbent discovery and therefore work before reaching the same final
family: task allocation's four-worker CPU timed captures range from 8,308,878
to 8,463,836 work and 1,177–1,178 candidates. Timing differences there cannot be
interpreted as the cost of executing an identical operation sequence.

Metal's broad after slowdown coincides with slower clingo observations. At four
workers, native median-sums rise 5.53% / 4.65%, while clingo rises
5.41% / 3.80% against the corresponding before legs. Queens 02 retains exactly
two device batches, 92 candidates, 579,232 device work and 1,323,080 logical
device bytes in every timed capture. These observations constrain attribution;
they do not establish that the slowdown is harmless noise or a GPU kernel change.

Do not pool away the fourteen-worker Metal queens 02 outlier. A2 has timings
194.1025 and 435.602 ms, giving a 314.852 ms median; A1 is 198.778 ms. The B
medians are 218.857 and 222.193 ms. A pooled before figure can misleadingly imply
an improvement. CPU scalar queens 02 also drifts from 129.447 ms at A1 to
142.817 at A2. Four timed samples do not support a statistical neutrality claim.
Accumulated worker phase timings can exceed wall time and must not be added as
disjoint wall intervals. Kernel duration was not measured.

## Sources, execution and evidence

| Role | Compiled source | Solver SHA-256 |
| --- | --- | --- |
| A, before | [`f8146e50`](https://github.com/GregoryGelfond/zetesis/tree/f8146e50304aaff3186d827b84f1e4f91ab068f7) | `9a4b598b882730c79293b96d520f2269378504437bd80c6ce286227b47820965` |
| B, after | [`687f0d0b`](https://github.com/GregoryGelfond/zetesis/tree/687f0d0b473d015125d4e70040735625b4c042a3) | `d0e8a70d705aef6b09995724a5927536095454d9e02fff5b8b7b44b59bf47513` |

The changes move a child's theory-sized knowledge copy outside the shared pool
mutex and let a worker pop its own pending region without acquiring that mutex.
A single atomic closure flag serves local and shared takes; shared donation,
idle registration and exhaustion still synchronize through the pool mutex.
An adjacent correctness fix retains a typed cancellation or deadline stop when
an idle worker observes it, preventing channel disconnection from reporting
unfinished search as exhausted. These changes are in the native CPU parallel
region walk. They do not change Metal's separate parallel candidate producer.

Acquisition ran on 20 September 2026, on Apple M4 Pro and arm64 macOS 26.6.2,
with stock clingo 5.8.2. CPU and Metal ran sequentially in the same environment.
All native captures reported a 24 GiB memory allowance. Both zetesis executables
were ordinary release builds with GPU support compiled in, without coverage instrumentation.
The [provenance](observations/workers-687f0d0b-provenance.json) records executable,
tool, source and raw-report identities and the measurement settings; each
comparison retains its original timestamps.

| Backend | Workers | Maintained comparison | All 94 cases |
| --- | ---: | --- | --- |
| CPU | 1 | [JSON](observations/workers-687f0d0b-cpu-1.json) | [tables](observations/workers-687f0d0b-cpu-1-tables.md) |
| CPU | 2 | [JSON](observations/workers-687f0d0b-cpu-2.json) | [tables](observations/workers-687f0d0b-cpu-2-tables.md) |
| CPU | 4 | [JSON](observations/workers-687f0d0b-cpu-4.json) | [tables](observations/workers-687f0d0b-cpu-4-tables.md) |
| CPU | 14 | [JSON](observations/workers-687f0d0b-cpu-14.json) | [tables](observations/workers-687f0d0b-cpu-14-tables.md) |
| Metal | 1 | [JSON](observations/workers-687f0d0b-metal-1.json) | [tables](observations/workers-687f0d0b-metal-1-tables.md) |
| Metal | 2 | [JSON](observations/workers-687f0d0b-metal-2.json) | [tables](observations/workers-687f0d0b-metal-2-tables.md) |
| Metal | 4 | [JSON](observations/workers-687f0d0b-metal-4.json) | [tables](observations/workers-687f0d0b-metal-4-tables.md) |
| Metal | 14 | [JSON](observations/workers-687f0d0b-metal-14.json) | [tables](observations/workers-687f0d0b-metal-14-tables.md) |

Each backend uses A1, B1, B2, A2. A1/B1 visit workers 1, 2, 4, 14; B2/A2 reverse
that worker order. Fourteen was the observed host default. Each leg has one
qualification, one warmup, two timed and one separate RSS invocation per case
and producer. Each executable therefore has four timed and two RSS observations
per case, worker count and backend. There is no cold-cache or confidence claim.

The profile is eager grounding, indexed formula joins, region search and automatic
checker selection, with batch size 64, one completion worker and one clingo
worker. Native JSON and statistics are always enabled by this measurement
protocol. Wall time includes process launch, preparation, grounding, solving,
statistics and captured output. It does not isolate counter or lock overhead.
This comparison does not cover lazy grounding or uninstrumented CLI execution.

Native CPU workers both explore regions and check membership. Metal workers
produce classical candidates, join, then submit batches to the device. General
device queries may require exact CPU completion; complete tight checks do not
use a residual worker pool. Actual receipts retain 53 tight and 41 countermodel
cases in every leg. An UNSAT case can finish without submitting a GPU batch.

All 30,080 scheduled positions passed. An additional comparison checked 15,040
complete native captures across revisions, workers and backends, including hidden
atoms, model multiplicity and objective priorities. The
[native-family receipt](observations/workers-687f0d0b-native-families.json)
records those checks separately. Clingo comparison covers the
selected displayed families and costs, not its hidden interpretations. The
separate 59 ordinary physical Metal tests also passed. Those tests and these
timings are not coverage measurements or an update to a coverage percentage.

## Reproduction and semantic scope

Use retained release executables and the exact corpus/tool identities in the
provenance. Set absolute paths in `CORPUS`, `SOLVER`, `CLINGO` and `REPORT`;
choose `PROFILE=cpu-eager` or `metal-eager` and `WORKERS=1`, `2`, `4` or `14`.
For each leg, the maintained collector invocation is:

```sh
zetesis-perf "$CORPUS" --suite corpus --profile "$PROFILE" \
  --formula-joins indexed --search regions --oracle auto \
  --workers "$WORKERS" --completion-workers 1 --clingo-workers 1 --batch-size 64 \
  --warmups 1 --repetitions 2 --memory-runs 1 \
  --timeout-seconds 10 --campaign-seconds 180 \
  --sample-bytes 33554432 --native-report-bytes 33554432 \
  --capture-bytes 536870912 --report-bytes 1073741824 \
  --zetesis "$SOLVER" --clingo "$CLINGO" --report "$REPORT"
```

Repeat in the stated ABBA and worker order, keeping both backends in the same
environment and checking actual memory, worker and device receipts. The collector
retains refusals, incomplete outcomes and unattempted positions; never replace
them with successful durations. Reports and output destinations must be new.
For each fixed backend/worker configuration, derive the maintained comparison:

```sh
zetesis-series --report "A1=$A1" --report "B1=$B1" \
  --report "B2=$B2" --report "A2=$A2" --json "$COMPARISON" > "$TABLES"
```

The [parallel execution contract](../rust/parallel.md) keeps pending, local and
active regions in the unfinished frontier until they are refuted, split or
checked. `Pending.Step.perm`, `Pending.Walk.exhausted` and `WorkPermits` provide
abstract preservation, exhaustion and work-conservation laws. As the
[Lean correspondence](../lean/correspondence.md) states, atomic memory ordering,
mutex/condition-variable progress, ownership transfer and cancellation receipts
remain Rust refinement obligations. Regression tests and matching complete
families support this implementation change; neither they nor these laws prove
the complete Rust solver or its device execution correct.
