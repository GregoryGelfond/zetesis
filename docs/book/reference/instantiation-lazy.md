# Grounding and prepared CPU closure

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

Source links use the [revision map](source-revisions.md); labels retain the
recorded revision identifiers.

The arithmetic chain took about **15% less whole-process time** and independent
negation about **7% less** in this comparison. Queens grounding also improved.
The 2,000-step ordinary chain regressed by about **4%**, and several workloads
used more memory. The corpus's sum of process medians was nearly unchanged;
the execution-series and scalability sums decreased.

This record compares zetesis 0.1.6 at
[`7c717c05`](https://github.com/GregoryGelfond/zetesis/commit/461b332b56328466c8ef868b716d61a4b6602411)
with
[`159c4563`](https://github.com/GregoryGelfond/zetesis/commit/c2a4abff748f6b244eb7826c6cf0d549e5fe6ecd).
The candidate combines canonical grounding-leaf reuse, support-publication
directory reuse, narrower dense delta scans and prepared ground-head coordinates.
It also includes the intervening solve-output and timing-presentation changes.
This is a combined comparison; it does not isolate the effect of any one change.
The [grounding explanation](../architecture/grounding.md) describes the ownership,
work, storage and reduct-preservation contracts.

## Complete selections

These are **sums of individual case medians**, in milliseconds. They are neither
campaign durations nor medians of pooled samples. The selections overlap, so
they must not be combined into an overall population speedup. Each native time
uses seven timed fresh processes; clingo uses one worker in both native profiles.

| Selection | Native workers | Cells | Baseline sum, ms | Candidate sum, ms | Change | Faster than clingo, baseline → candidate |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Corpus | 4 | 94 | 791.782 | 791.078 | −0.09% | 4 → 4 |
| Corpus | 14 | 94 | 797.644 | 792.062 | −0.70% | 4 → 4 |
| Execution series | 4 | 22 | 714.932 | 688.944 | −3.64% | 6 → 7 |
| Execution series | 14 | 22 | 657.094 | 624.913 | −4.90% | 6 → 7 |
| Scalability, including Einstein | 4 | 10 | 234.846 | 220.018 | −6.31% | 4 → 4 |
| Scalability, including Einstein | 14 | 10 | 205.507 | 193.171 | −6.00% | 4 → 5 |

Every cell, including regressions, appears in the complete tables:

| Selection | Four workers | Fourteen workers |
| --- | --- | --- |
| Corpus | [94 cells](#corpus-four-workers) | [94 cells](#corpus-fourteen-workers) |
| Execution series | [22 cells](#series-four-workers) | [22 cells](#series-fourteen-workers) |
| Scalability | [10 cells](#scalability-four-workers) | [10 cells](#scalability-fourteen-workers) |

The tables retain full entry paths, amended or generated workload parameters,
process ranges, clingo results and separate peak RSS. Driver and phase medians,
counts, full workload identities and qualification results are in the
[evidence projection](observations/instantiation-lazy-159c4563-evidence.json)
and [family comparison](observations/instantiation-lazy-159c4563-families.json).
An amended queens
entry at ten queens is a different cell from the same entry at eleven queens.

## Gains and regressions

Selected execution-series process medians, in milliseconds:

| Full entry and cell | Four workers, baseline → candidate | Fourteen workers, baseline → candidate |
| --- | ---: | ---: |
| `generated/chain-arithmetic-1000.lp` | 19.952 → 16.966 | 19.982 → 16.988 |
| `generated/independent-negation-10.lp` | 43.901 → 40.881 | 45.420 → 42.214 |
| `standalone/n-queens/variant-01.lp`, n=10 | 40.761 → 35.921 | 34.383 → 29.271 |
| `standalone/n-queens/variant-01.lp`, n=11 | 97.831 → 91.378 | 66.733 → 56.758 |
| `generated/chain-2000.lp` | 33.573 → 35.054 | 35.099 → 36.546 |
| `generated/producer-chain-700.lp` | 21.360 → 21.341 | 21.371 → 22.840 |
| `generated/transitive-path-200.lp` | 50.574 → 50.725 | 50.868 → 51.987 |

The phase measurements help locate the changes. Arithmetic-chain grounding fell
from 11.558 to 8.094 ms at four workers and from 11.794 to 8.196 ms at fourteen.
For eleven queens, grounding fell from 26.890 to 20.057 ms and from 27.445 to
20.428 ms. Independent-negation-10 proposal time fell from 36.958 to 33.883 ms
and from 37.360 to 34.077 ms. These gains appear in the driver interval as well
as the process interval; they are not solely changes to process startup time.

The regressions also reach the driver interval. Chain-2000 driver time increased
from 27.141 to 28.722 ms at four workers and from 28.726 to 30.091 ms at fourteen.
Its closure-membership interval increased slightly at both counts. At fourteen
workers, producer-chain-700 membership increased from 8.521 to 9.293 ms and its
driver time from 15.774 to 17.190 ms. Queens variant 02 also regressed at fourteen
workers in both the corpus and scalability selections despite lower grounding
time; its proposal time increased.

Less reported search work does not by itself mean less total work. For a
one-candidate chain, preparing a ground head's dense position moves its ranking
from the closure check into query preparation. The initial focused comparison
showed that movement, with a small increase in preparation-plus-closure work.
The series search-work field omits query-preparation work on this route. Its
decrease therefore cannot establish a total-work reduction. The timed
closure-membership interval includes that preparation.

Memory results are mixed. In the fourteen-worker series, eleven-queen variant 01
peak RSS rose from 23.891 to 31.688 MiB. In the fourteen-worker corpus, variant
02 rose from 32.797 to 37.469 MiB. Each value is the median of three separate
fresh-process memory observations. Peak RSS measures resident host memory, not
the solver's logical storage allowance, and is not a timed process's memory peak.
For eleven-queen variant 01, all three candidate observations, 30.078–32.422 MiB,
exceeded all three baseline observations, 22.969–26.109 MiB. This is a consistent
increase within these batches. The four-worker counterpart was nearly flat.
The measured source and model-construction storage counters did not explain the
increase; its cause remains unestablished.

## Conditions and limits

All twelve campaigns passed and were fully accounted for: all 13,104 scheduled
samples passed, with no refusals, process faults or unresolved children. The 252 native
suite/worker cells in each build qualified against clingo, and the independent
before/after comparison matched all 252 complete native answer families.
Qualification includes selected displays, multiplicity, costs and all final
optimum ties; clingo's hidden interpretations are not available from its displays.
These checks establish this measured population, not a general implementation
proof or physical GPU qualification.

| Setting | Value |
| --- | --- |
| Native backend / requested grounder / oracle | CPU / auto / auto |
| Native workers | Separate campaigns at 4 and 14 |
| Completion workers / batch size | 1 / 64 |
| Completion scratch allowance | 268,435,456 bytes |
| Native work, expansion and storage budgets | Solver defaults; no raised allowance |
| Qualification / warmup / timed / separate RSS runs | 1 / 2 / 7 / 3 per solver and cell |
| Clingo | 5.8.2, one worker in both native configurations |
| External process / campaign deadline | 60 s / 1,800 s |
| Native output | Complete enumeration with JSON and statistics |

Scalability uses indexed joins and region search and includes Einstein. Each
build ran corpus, series and scalability at four workers, then the same suites
at fourteen workers. All baseline campaigns preceded the candidate campaigns.
This fixed order does not control host drift. The complete acquisition spans
1 October 2026, 02:50–03:09 UTC. Seven repetitions and their ranges do not
establish statistical significance, cold caches or independent replications.

The session's measurement host was an Apple M4 Pro with 48 GB RAM. A read-only
observation after acquisition recorded macOS 26.6.2 and Rust 1.97.1; the processor
and memory description comes from the earlier host record. This is host context,
not an at-run hardware seal. Individual clingo medians also changed between builds;
for example, its fourteen-worker-profile producer-chain-700 time fell 14.2%.
The win counts compare each native build with its own clingo runs.

Process wall time includes launch, source loading, grounding, solving, JSON,
statistics and captured output. Driver and phase measurements have narrower
scopes; parallel phase durations may overlap and must not be added to reconstruct
wall time. These results do not describe statistics-free solving, Metal or Vulkan.

## Executables and reproduction

Both solvers identify themselves as version 0.1.6. The executable seals, rather
than the version string, identify the measured builds:

| Executable | SHA-256 |
| --- | --- |
| Baseline, source `7c717c05` | `16d80325183ca71fb98b95ecd8151a319e1d26a749f702fd2c4db4c10ec37f98` |
| Candidate, source `159c4563` | `bcd3028e30523459215c73e06eebf95f439c11959adbf3a3b6c3aab56b0bf60e` |
| Common zetesis-bench runner and RSS helper | `3db4ea709f9635203068db8a0ca94f283de34b1887a41e9bcca1ff47524dde97` |
| Common clingo 5.8.2 | `31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015` |

The common runner's source is
[`ae389373`](https://github.com/GregoryGelfond/zetesis/commit/59f686b3e288c04368fad380b56afc0bf062f6e0).
The correctness manifest SHA-256 is
`b43df1adf17ae0c035f1e310a5c15345c26cbcad8b59596932627c46fd1c6958`.
The appendices are portable projections, not raw model captures or inputs to
`zetesis-bench compare`. Rechecking the complete populations requires the retained
captures or a new run; a report hash does not imply a public download.
The [projection receipt](observations/instantiation-lazy-159c4563-receipt.json)
records hashes of the tables and their evidence files.

Prepare clean checkouts at the two solver commits and the runner commit above,
each with its own Cargo target directory. Use the Rust 1.97.1 pin and locked
release builds. Build `zetesis` in each solver checkout and the common runner in
its checkout:

```sh
cargo build --release --locked --target aarch64-apple-darwin -p zetesis-cli --bin zetesis
cargo build --release --locked --target aarch64-apple-darwin -p zetesis-bench --bin zetesis-bench
```

Default features compile GPU support; every measured solve selects CPU. Record
the revision, toolchain, lockfile and executable hashes. A rebuild is a new
executable and measurement even when its source revision matches.

Run the following from the candidate checkout. Set `BENCH` to the common runner,
`BASELINE` and `CANDIDATE` to the two solver executables, `CLINGO` to clingo 5.8.2,
and `OUT` to a new output directory; use absolute paths. Keep builds, tests and
profilers stopped during timing. The loop preserves the recorded campaign order
and times clingo separately beside each native profile.

```sh
set -eu
mkdir "$OUT"
for build in baseline candidate; do
    if [ "$build" = baseline ]; then
        native=$BASELINE
    else
        native=$CANDIDATE
    fi
    for workers in 4 14; do
        for suite in corpus series scalability; do
            if [ "$suite" = scalability ]; then
                set -- --include-einstein
            else
                set --
            fi
            "$BENCH" run examples/correctness --suite "$suite" "$@" \
                --zetesis "$native" --clingo "$CLINGO" --backend cpu \
                --threads "$workers" --warmups 2 --repetitions 7 --memory-runs 3 \
                --timeout-seconds 60 --campaign-seconds 1800 \
                --report "$OUT/$build-$suite-$workers.json" \
                > "$OUT/$build-$suite-$workers.stdout" \
                2> "$OUT/$build-$suite-$workers.stderr"
        done
    done
done
for workers in 4 14; do
    for suite in corpus series scalability; do
        "$BENCH" compare "baseline=$OUT/baseline-$suite-$workers.json" \
            "candidate=$OUT/candidate-$suite-$workers.json" \
            --output "$OUT/comparison-$suite-$workers.json" --markdown \
            > "$OUT/comparison-$suite-$workers.md"
    done
done
```

Retain exit statuses and any refusal; do not replace a failed sample or raise a
solver budget to obtain a timing. The [benchmark reference](benchmarking.md)
documents protocol, default limits and the larger capture ceilings for series
records. A reverse-order replication would add evidence about host drift.

## Complete corpus tables

### Corpus: four workers

{{#include observations/instantiation-lazy-159c4563-corpus-4.md:3:}}

### Corpus: fourteen workers

{{#include observations/instantiation-lazy-159c4563-corpus-14.md:3:}}

## Complete series tables

### Series: four workers

{{#include observations/instantiation-lazy-159c4563-series-4.md:3:}}

### Series: fourteen workers

{{#include observations/instantiation-lazy-159c4563-series-14.md:3:}}

## Complete scalability tables

### Scalability: four workers

{{#include observations/instantiation-lazy-159c4563-scalability-4.md:3:}}

### Scalability: fourteen workers

{{#include observations/instantiation-lazy-159c4563-scalability-14.md:3:}}
