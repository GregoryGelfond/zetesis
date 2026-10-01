# Reusing support-publication directories

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

The 1,000-step arithmetic chain took **22.6% less grounding time and 12.6% less
whole-process time** in this CPU comparison. Other workloads were mixed. The
sums of case medians increased in the corpus and execution series and decreased
slightly in scalability; these observations do not establish an overall speedup.

This record compares baseline
[`7c717c05`](https://github.com/GregoryGelfond/zetesis/commit/7c717c05df4f10830008d297b9611e89410d9813)
with its direct child
[`d08a1bd1`](https://github.com/GregoryGelfond/zetesis/commit/d08a1bd1a380afc7b51c2269275174f1da877544).
Both identify themselves as zetesis 0.1.6. The change reuses exclusively owned
canonical publication directories; the later human solve-output changes are
absent from both measured executables. Measurements include JSON and statistics
output and do not describe statistics-free solving or Metal execution.

## Results

Each process-time value is the median of five timed fresh processes. The table
below sums those medians within each suite, in milliseconds. A sum is neither
campaign duration nor a median of pooled samples. The suites overlap: these are
126 suite-qualified workload cells, not 126 independent programs.

| Suite | Complete cells in each build | Baseline sum, ms | Candidate sum, ms | Change |
| --- | ---: | ---: | ---: | ---: |
| Corpus | 94 | 857.075 | 863.451 | +0.74% |
| Execution series | 22 | 714.218 | 721.093 | +0.96% |
| Scalability, including Einstein | 10 | 239.267 | 238.242 | −0.43% |

Every case remains in the [corpus tables](#complete-corpus-tables),
[series tables](#complete-series-tables) and
[scalability tables](#complete-scalability-tables), including improvements,
regressions, clingo comparisons, ranges and separate memory observations.
Their short cell labels follow the maintained comparison command. The
[compact evidence](observations/support-publication-d08a1bd1-evidence.json)
retains full ordered entry paths and generated or amended identities to
disambiguate repeated labels.

The arithmetic chain selected eager CPU positive consequences and completed
1,002 support rounds, discovering 1,001 support atoms. Its measurements were:

| Arithmetic-chain-1000 measurement | Baseline | Candidate | Change |
| --- | ---: | ---: | ---: |
| Whole process, median ms | 20.551375 | 17.956916 | −12.62% |
| Grounding stage, median ms | 12.324958 | 9.537916 | −22.61% |
| Support completion, median ms | 10.157791 | 7.306917 | −28.07% |
| Charged support-publication work | 1,257,622 | 259,660 | −79.35% |
| Separate peak RSS, MiB | 16.063 | 15.563 | one observation per build |

Charged work is the implementation's accounting unit, not allocator events or
bytes copied. Support completion is inside grounding, which is inside the
process interval; these durations must not be added. The single RSS observations
cannot establish a repeatable memory reduction. The five wall samples ranged
from 20.252–21.637 ms before and 17.855–19.365 ms after; ranges are descriptive,
not confidence intervals.

## What changed

The support catalog records atoms discovered during grounding. A snapshot gives
its consumers an immutable view of those atoms.

Support growth already shared immutable segment payload. Each publication still
copied the directories of earlier segment references into a new snapshot.
Repeated short publications could therefore spend growing work on the directory
history. The changed
[`Store` publication path](https://github.com/GregoryGelfond/zetesis/blob/d08a1bd1a380afc7b51c2269275174f1da877544/crates/zetesis-core/src/catalog/storage/publication/renewal.rs)
extends the current directories when the snapshot and its growing vocabulary
are exclusively owned. Directory relocation then occurs at capacity-growth
boundaries. An externally retained snapshot continues to require fresh
publication; frozen vocabulary remains shared.

Canonical IDs, discovery order, immutable retained prefixes and the existing
work and storage limits remain the contract. Reservations precede visible
changes. The [ownership explanation](../architecture/ownership.md) and
[implementation correspondence](../lean/correspondence.md) describe that
boundary; this timing record adds no Rust or shader proof claim.

## Acquisition and qualification

Each suite ran **baseline, then candidate**, in corpus, series, scalability
order. There is one campaign per build and suite, without reverse-order blocks.
The fixed order does not control host drift, and the repetitions do not establish
statistical significance or cold caches. The reports were acquired on
30 September 2026 in America/Chicago, 1 October UTC. Exact acquisition times are
retained in the evidence.

The session's measurement host was an Apple M4 Pro with 48 GB RAM. A separate
read-only observation after acquisition found macOS 26.6.2 (25G83), fourteen
cores (ten performance and four efficiency), Rust 1.97.1 and LLVM 22.1.6.
That observation supplies host context, not an at-run hardware seal. The reports'
executable seals, tool versions, arguments and source checks are the retained
measurement identities.

| Setting | Recorded value |
| --- | --- |
| Native backend / requested grounding / oracle | CPU / auto / auto |
| Native threads / completion workers / batch size | 4 / 1 / 64 |
| Completion scratch allowance | 268,435,456 bytes |
| Qualification per solver and cell | 1 |
| Warmup / timed / separate RSS runs per solver and cell | 1 / 5 / 1 |
| Clingo | 5.8.2, one worker, complete enumeration or all optimum ties |
| Native work, expansion and storage budgets | Solver defaults; no raised allowance |
| External process / campaign deadline | 30 s / 600 s |
| Native format and instrumentation | JSON and statistics |

Scalability explicitly selects indexed joins and region search and includes
Einstein. The series contains 17 generated workloads, three amended queens
cells and two original corpus cases. Amended queens variant 01 occurs at both
ten and eleven queens; a filename alone does not identify its cell. SEND and
task allocation occur in all three selections.

All **4,032 scheduled positions passed**: 1,504 per corpus report, 352 per series
report and 160 per scalability report. All 2,016 native outcomes were exhausted,
with no interruption, publication stop or error. The six reports have no faults
or unresolved children. Every input and executable retained its admitted seal
through its campaign.
Every native profile uses CPU execution; compiling GPU support is not device
qualification.

The runner qualifies complete selected displays against clingo, including
repeated symbols, duplicate displays, objective costs and all final optimum
ties. It also compares full native models across each report's repetitions.
An independent comparison of the two qualification populations found identical
full native families in every cell, resolving the cumulative atom catalogs and
preserving full atoms, shown occurrences, per-model costs and multiplicity.
Clingo's hidden interpretations remain unavailable from its displayed results.

Wall time includes process launch, source loading, grounding, solving, JSON,
statistics and captured output. Detailed phase intervals may overlap and do not
reconstruct wall time. RSS comes from a separate fresh helper's solver child;
the projection retains distinct helper and child process IDs and the original
byte-valued receipt. It is host peak RSS, not GPU memory or a timed sample's
memory peak.

## Executables and evidence

The preserved executables match these report seals:

| Role | SHA-256 |
| --- | --- |
| Baseline zetesis, source `7c717c05` | `16d80325183ca71fb98b95ecd8151a319e1d26a749f702fd2c4db4c10ec37f98` |
| Candidate zetesis, source `d08a1bd1` | `aeb0bc80d400565059ebceb48f4f7010a265d8b2d3177b9383d08edc5cfa02eb` |
| Common zetesis-bench runner and RSS helper | `14e4f21542ede3b5b683ca5cd4bbe992b1f911b9c48630f0b8ca87cec73a8daf` |
| Common clingo 5.8.2 | `31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015` |

The correctness manifest SHA-256 is
`b43df1adf17ae0c035f1e310a5c15345c26cbcad8b59596932627c46fd1c6958`.
The evidence retains original-report hashes, ordered workloads, source-file
hashes, plans and limits, every timed wall and stage sample, phase medians,
separate RSS receipts and qualification-family fingerprints. Timing arrays keep
round order; missing measurements are unavailable, never zero. Native verified
counts and search work can vary with scheduling even when the selected family
is identical, so their per-round values remain explicit.

This is a projection of the six reports, not their raw captures or an input to
`zetesis-bench compare`. Raw model and diagnostic captures remain outside the
repository. Their hashes identify the retained reports without claiming a
public download. Rechecking the complete raw populations requires those
captures or a new run; the projection alone is not an independent solver proof.

## Reproduce

Prepare clean checkouts at the two full commits above with separate Cargo target
directories. Their locked release builds use the repository's Rust 1.97.1 pin;
GPU support was compiled, while every measured solve selected CPU. The retained
build logs show successful optimized release compilation but do not embed the
complete shell environment. Build each checkout with the maintained packages:

```sh
cargo build --release --locked -p zetesis-cli -p zetesis-bench --bins
```

Record source revision, toolchain, lockfile and executable hashes. A rebuild is
a new executable and measurement, even from the same source. Run the following
from the candidate checkout, using its unchanged workload roots. Set `BENCH`
to one common `zetesis-bench` executable, `BASELINE` and `CANDIDATE` to the two
absolute solver paths, `CLINGO` to clingo 5.8.2, and `OUT` to a new absolute
output directory. Keep other builds, tests and profilers stopped during timing.

```sh
set -eu
mkdir "$OUT"
for suite in corpus series scalability; do
    if [ "$suite" = scalability ]; then
        set -- --include-einstein
    else
        set --
    fi
    for build in baseline candidate; do
        if [ "$build" = baseline ]; then
            native=$BASELINE
        else
            native=$CANDIDATE
        fi
        "$BENCH" run examples/correctness --suite "$suite" "$@" \
            --zetesis "$native" --clingo "$CLINGO" \
            --backend cpu --threads 4 --warmups 1 --repetitions 5 \
            --memory-runs 1 --timeout-seconds 30 --campaign-seconds 600 \
            --report "$OUT/$suite-$build.json" \
            > "$OUT/$suite-$build.stdout" 2> "$OUT/$suite-$build.stderr"
    done
    "$BENCH" compare "baseline=$OUT/$suite-baseline.json" \
        "candidate=$OUT/$suite-candidate.json" --markdown \
        > "$OUT/$suite-comparison.md"
done
```

Retain each command's exit status and inspect any failure before continuing.
Do not raise a solver budget or replace a refused sample to obtain a timing.
`compare` reads the new reports without launching solvers; it checks matching
ordered workloads, identities and profiles. The
[benchmark command reference](benchmarking.md) explains the complete protocol
and capture ceilings, including the larger defaults for series records.
Repeating this exact order reproduces the recorded design; a reverse-order
replication would provide additional evidence, not another sample from it.

## Complete corpus tables

All 94 cells, generated by the maintained comparison command:

{{#include observations/support-publication-d08a1bd1-corpus.md}}

## Complete series tables

All 22 cells:

{{#include observations/support-publication-d08a1bd1-series.md}}

## Complete scalability tables

All ten cells, including Einstein:

{{#include observations/support-publication-d08a1bd1-scalability.md}}
