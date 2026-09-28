# Canonical storage and answer construction

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

This CPU comparison measures the 0.1.5 implementation against the preceding
0.1.4 executable on unchanged maintained workloads. The current build completes
all three populations, but takes longer on most series cases and has higher
sums of comparable case times in each population. Correcting answer
construction does not establish an overall performance improvement.

The comparison covers the integrated changes between the two sources below.
It does not isolate the cost of canonical storage, ordering, grounding or
answer construction. The separate worker measurements show how the current
implementation behaves at 1, 2, 4, 8 and 14 workers; future scheduling choices
need profiling of the relevant workload.

## Results

Each number below is a **sum of per-case wall-time medians**, in milliseconds.
It is neither campaign duration nor a median of pooled samples. Each block
median uses two timed fresh processes per solver and case.

| Population | Cases with four passing native blocks | A1 | B1 | B2 | A2 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Corpus | 94 | 858.615 | 942.543 | 993.075 | 857.193 |
| Series | 22 | 520.350 | 879.108 | 891.607 | 520.920 |
| Scalability, excluding the refused A Einstein cell | 9 of 10 | 177.726 | 245.426 | 245.216 | 179.687 |

**A** is the earlier executable; **B** is the current executable. Native
four-worker medians are below their corresponding single-worker clingo medians
on 4 of 94 corpus cases in each block. In the series, A wins 9 of 22 cases in
each block and B wins 4 of 22. These counts describe these measured cells;
a sum or win count does not describe every program's performance.

The current series medians exceed A in 20 of 22 B1/A1 comparisons and all
22 B2/A2 comparisons. Independent-choice-16 takes about 2.96–3.03 times as
long; independent-negation-10 takes 3.77–3.96 times as long. The amended
variant-04 queens workload at n=11 is nearly unchanged, at 0.982 and 1.003
times A. Both improvements and regressions remain in the full tables.

The series worker curves vary by workload. The three amended queens cases
have their lowest observed S medians at four workers. Independent-choice-16
continues improving through fourteen workers, while several chain and closure
cases change little. Additional workers also change RSS; this is not evidence
for one universally best worker count.

Both current Einstein blocks completed all requested positions, with B1/B2
wall medians of **20.388 / 20.414 ms** and separate peak-RSS observations of
**20.969 / 20.719 MiB**. Their clingo medians were 36.063 / 35.389 ms. The
earlier executable refused during qualification in both A blocks: source
admission required at least 10,000,001 formula-work units against its
10,000,000-unit limit. Its later native positions were not attempted, so
there is no A Einstein timing or before/after speed ratio. All ten current
scalability workloads also passed at all five S worker counts.

The complete tables retain every workload, worker profile, reference bracket,
RSS observation and refusal:

| Population | Four-worker A/B observations | Current worker observations |
| --- | --- | --- |
| Corpus, 94 cells | [A/B data](observations/canonical-storage-8c847132-corpus-matched.json) | [Worker data](observations/canonical-storage-8c847132-corpus-scaling.json) |
| Series, 22 cells | [A/B data](observations/canonical-storage-8c847132-series-matched.json) | [Worker data](observations/canonical-storage-8c847132-series-scaling.json) |
| Scalability, 10 cells | [A/B data](observations/canonical-storage-8c847132-scalability-matched.json) | [Worker data](observations/canonical-storage-8c847132-scalability-scaling.json) |

The [provenance record](observations/canonical-storage-8c847132-provenance.json)
identifies acquisition intervals, source and executable identities, build
inputs, workloads, settings and original-report hashes. These normalized views
do not contain the complete raw model captures. The hashes identify retained
reports; they are not public download locations or a substitute for those
captures when independently rechecking complete families.

## Workloads and acquisition

The population is **126 suite-qualified workload cells**: 94 corpus cases,
22 series cases and 10 scalability cases with Einstein included. SEND and
task allocation recur in all three suites, and queens variant 02 occurs in
corpus and scalability. These are overlapping selections, not 126 independent
programs. The series includes 17 generated workloads, three amended queens
workloads and two unchanged cases. Scalability includes authored queens at
n=8/9/10, pigeonhole at h=5/6/7, three unchanged corpus cases and Einstein.

A table row identifies its suite and workload, including generated size or
constant amendment. A filename alone is insufficient: series queens variant
01 appears at both n=10 and n=11. Original input bytes, transitive include
closures, generated identities and amendments remain attached to the reports.

Each suite used the serial order **A1 → B1 → S → B2 → A2**. The suites ran
in series, corpus, scalability order. S measures only B at five worker counts;
it is an intervening scaling window, not part of the four-worker A/B sample
population. B1 and B2 provide the contemporaneous clingo bracket around S.
The measurements were acquired on 25 September 2026 in America/Chicago
(26 September UTC), on an Apple M4 Pro with 48 GB RAM under macOS 26.6.2.
The hardware report identifies fourteen cores, ten performance and four
efficiency cores. The provenance distinguishes the successful hardware report
from the unavailable `sysctl` probe.
Campaigns were scheduled without overlapping builds, tests or profiling.
This does not establish cold caches, an isolated host or statistical significance.

| Setting | Four-worker A/B block | Current S block |
| --- | --- | --- |
| Native device / grounding / oracle | CPU / auto / auto | CPU / auto / auto |
| Native workers | 4 | 1, 2, 4, 8, 14 |
| Completion workers / batch size | 1 / 64 | 1 / 64 |
| Completion scratch allowance | 268,435,456 bytes | 268,435,456 bytes |
| Qualification | Native and clingo | Every native profile and clingo |
| Warmups per native profile | 1 | 1 |
| Timed samples per native profile | 2 | 5 |
| Separate RSS observations per native profile | 1 | 1 |
| Clingo workers | 1 | 1, qualification only |
| Process timeout / campaign scheduling allowance | 30 s / 1,800 s | 30 s / 1,800 s |
| Per-capture / native-decoder ceiling | 16 MiB / 16 MiB | 16 MiB / 16 MiB |
| Aggregate capture / serialized report ceiling | 512 MiB / 1 GiB | 1 GiB / 2 GiB |

The A/B reports measure clingo in qualification, warmup, timing and RSS phases.
S uses the maintained `--compare-threads` policy: clingo qualifies complete
families once per case and supplies **no timed or RSS population**. Accordingly,
the worker tables show clingo B1 and B2 as separate measured bracket columns.
They are not S samples, interleaved worker/reference pairs or interpolated
reference values. S's four-worker result is kept separate from B1/B2.

The five timed rounds rotate the five native profiles through every launch
position once per case when all positions complete. This balances planned
timed positions, not all possible case orders. The single RSS round is a
separate population, with one observation per profile and case.

Native children request machine-readable results and statistics. Wall time
includes process launch, source loading, grounding, solving and captured
output. Internal phase intervals can overlap and must not be added as wall
time. A phase absent from an older report is unavailable, not zero.

RSS is measured by a fresh helper that waits for one solver child and reads
its `RUSAGE_CHILDREN.ru_maxrss`; the helper itself is excluded. It is host
memory, not GPU memory, allocator capacity, simultaneous process-tree RSS or
the peak of a timed sample. macOS reports bytes; Linux reports KiB, which the
runner converts to bytes. Memory-run durations do not enter the timing table.

The requested profile and workload are matched; each executable retains its
own ordinary implementation defaults. No native work, storage or expansion
limit was raised to make a row pass. Automatic capacities can depend on the
worker count. The scalability suite selects indexed joins and region search;
the reports retain effective execution settings and actual observations.

Qualification checks selected complete displayed families and costs, including
duplicates, empty displays and all optimal ties. The supported clingo `optN`
final-incumbent replay is normalized. Hidden clingo atoms cannot be recovered
from its displayed output; native full models are compared across native runs.
A first nonpass disables later launches for that cell. Refusals and unlaunched
positions remain visible, with no replacement samples or substituted timeout
values. Ratios require paired passing timed populations; RSS can separately be
unavailable. A range of block medians is not a confidence interval.

## Measured executables

Both native executables were ordinary release builds with all features. B also
served as the common benchmark runner and RSS helper for every A/B and S
campaign. A shared runner does not make the older executable a current build.
Later documentation or qualification commits do not change these identities.

| Role | Compiled source | Executable SHA-256 |
| --- | --- | --- |
| A, zetesis 0.1.4 | `013ae6d3fc0290533fe98d723647d59451c39df6` | `e644b180309907ad3b78a934ade1e46cc7e079f15b5b16d79d919aa4ff02c47a` |
| B, zetesis 0.1.5; runner and helper | `8c84713265cf2f97f5102548be52b431585a2cd6` | `0dfa108013a7f9e714d9c0bf5d3f13e5ebd019ad3f9d51b79357109710d020b7` |
| Stock clingo 5.8.2 | External reference executable | `31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015` |

The correctness catalog SHA-256 is
`b43df1adf17ae0c035f1e310a5c15345c26cbcad8b59596932627c46fd1c6958`.
Workload identities additionally cover the selected/generated/amended sources.
Clingo used its stock search configuration with one worker and the arguments
`--models=0 --outf=2 --opt-mode=optN --parallel-mode=1 --warn=none`, followed
by the unchanged workload path.

## Reproduce the comparison

Prepare separate clean checkouts at the two full source commits above. Keep
their Cargo target directories separate. Install Rust 1.97.1 and fetch each
checkout's locked dependencies first; then run this recorded build command
in each checkout:

```sh
cargo +1.97.1 build --release --locked --offline \
  -p zetesis-cli -p zetesis-maintenance --bins --all-features \
  --message-format=json-render-diagnostics > build.jsonl
```

Retain each build's source identity, status, toolchain, lockfile and executable
hash. A rebuild on another host is a new executable and a new measurement;
source identity alone does not reproduce the recorded binary hash. Use the
current B checkout for all benchmark commands and workload roots. Set `A`,
`B` and `CLINGO` to absolute paths of the pinned executables; set `OUT` to a
new absolute output directory. The B path is used both as the command runner
and as the current solver. Use `LC_ALL=C` for the acquisition and create a
fresh output directory before launching any block.

For each suite, run the four-worker command once per block, setting `NATIVE`
to A for A1/A2 and B for B1/B2, with the S command inserted after B1. Use
`SUITE=series`, then `corpus`, then `scalability`. For scalability, add
`--examples examples --include-einstein` to both measurement commands.

```sh
"$B" bench corpus examples/correctness --suite "$SUITE" \
  --zetesis "$NATIVE" --native-interface solve \
  --clingo "$CLINGO" --clingo-threads 1 \
  --device cpu --grounder auto --threads 4 \
  --completion-workers 1 --batch-size 64 \
  --warmups 1 --repetitions 2 --memory-runs 1 \
  --timeout-seconds 30 --campaign-seconds 1800 \
  --sample-bytes 16777216 --native-report-bytes 16777216 \
  --capture-bytes 536870912 --report-bytes 1073741824 \
  --json --report "$OUT/$SUITE-$BLOCK.json" \
  > "$OUT/$SUITE-$BLOCK-summary.json"
```

The S block uses the same B solver at all five worker counts:

```sh
"$B" bench corpus examples/correctness --suite "$SUITE" \
  --zetesis "$B" --native-interface solve \
  --clingo "$CLINGO" --clingo-threads 1 \
  --device cpu --grounder auto --compare-threads 1,2,4,8,14 \
  --completion-workers 1 --batch-size 64 \
  --warmups 1 --repetitions 5 --memory-runs 1 \
  --timeout-seconds 30 --campaign-seconds 1800 \
  --sample-bytes 16777216 --native-report-bytes 16777216 \
  --capture-bytes 1073741824 --report-bytes 2147483648 \
  --json --report "$OUT/$SUITE-S.json" \
  > "$OUT/$SUITE-S-summary.json"
```

Retain stderr, exit status, exact expanded arguments, source/binary/input seals
before and after, host observations and acquisition times for every block.
Inspect a nonzero exit and its reported cause before continuing; an accounted
native refusal is a result to preserve. Do not raise a bound or replace a
sample after seeing its outcome. The native `solve` interface must be explicit
when using `--zetesis`; the compatibility interface is a different protocol.

After all blocks for a suite finish, derive the two views independently:

```sh
"$B" bench compare \
  --report "A1=$OUT/$SUITE-A1.json" --report "B1=$OUT/$SUITE-B1.json" \
  --report "B2=$OUT/$SUITE-B2.json" --report "A2=$OUT/$SUITE-A2.json" \
  --report-bytes 1073741824 --json \
  --output "$OUT/$SUITE-abba.json"

"$B" bench compare --report "S=$OUT/$SUITE-S.json" \
  --report-bytes 2147483648 --json \
  --output "$OUT/$SUITE-scaling.json"
```

These commands read reports without launching solvers. The input-byte bound
does not bound decoded memory. Keep each suite's ordered workload/profile
identity intact; combine the public tables by suite and workload identity,
not by concatenating incompatible reports. Preserve the A1/B1/B2/A2 medians
and B1/B2 clingo bracket instead of pooling them into one apparent sample.

## Full corpus: four-worker comparison

All 94 current and earlier cases passed. Each wall column has two timed samples
per case; each RSS column has one separate observation.

{{#include observations/canonical-storage-8c847132-corpus-matched.md}}

## Full corpus: current worker scaling

Each native wall column has five timed samples per case; each RSS column has
one separate observation. Clingo B1/B2 are the measured bracket, not S samples.

{{#include observations/canonical-storage-8c847132-corpus-scaling.md}}

## Execution series: four-worker comparison

All 22 current and earlier cases passed. Amended queens labels identify their
board sizes; the two variant-01 rows are distinct workloads.

{{#include observations/canonical-storage-8c847132-series-matched.md}}

## Execution series: current worker scaling

{{#include observations/canonical-storage-8c847132-series-scaling.md}}

## Scalability: four-worker comparison

The current executable passed all ten cases. The earlier executable's Einstein
refusal is retained as R1; it contributes no native successful timing or RSS.

{{#include observations/canonical-storage-8c847132-scalability-matched.md}}

## Scalability: current worker scaling

{{#include observations/canonical-storage-8c847132-scalability-scaling.md}}
