# Eager and hybrid formula grounding

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

Source links use the [revision map](source-revisions.md); labels retain the
recorded revision identifiers.

Hybrid formula execution retains an eager core and checks eligible source
constraints while searching and before publishing each answer set. Complete
support and arithmetic admission still precede solving. The measurements below
record successive changes to this schedule and retain the earlier full-candidate
experiment separately. See the
[execution contract](../architecture/grounding.md#eager-and-lazy-execution)
for eligibility and completion boundaries.

## Reusing completed-source join plans

The implementation at
[`469d4d87`](https://github.com/GregoryGelfond/zetesis/commit/9007f29f751545eecfa681c26d2446c4aef613ee)
retains immutable join ordering and comparison readiness for repeated hybrid
constraint scans. Each scan has fresh bindings, probes, arithmetic state and
traversal. Eager and hybrid execution share the planner; ordinary joins own
their plans and hybrid scans borrow them. Zero-amount resource charges also
avoid a shared atomic write while preserving the cancellation and positive-charge
checks. The comparison combines both changes.

The compiled baseline is
[`df99d4bb`](https://github.com/GregoryGelfond/zetesis/commit/2d6fbc5b85ad398472c5b520f1ee881c6957470e).
Measurements ran on September 21, 2026 in America/Chicago, on macOS 26.6.2 with
Rust 1.97.1, using the same 20-workload population as the
[original-region comparison](#original-region-checks). Each worker setting uses
one old/new/new/old block. Every leg includes eager and hybrid profiles, one
warmup, four timed processes and two separate RSS processes per cell.

All eight campaigns pass 340 positions each. The 2,560 complete native captures
agree on full typed answer-set families, shown channels and multiplicity across
versions, grounders and worker settings. Clingo supplies 160 untimed selected-output
qualifications and complete model counts. These programs have no objectives.
The [complete observations](observations/hybrid-preparation-469d4d87-evidence.json)
retain every position, integer wall/RSS sample, family-audit receipt, workload
identity, executable hash and protocol. Original capture streams are retained
separately; the public projection supports recalculating timing and memory
summaries, rather than repeating the semantic audit from the original streams.

The table shows separate leg medians in milliseconds. Compare B1 with A1 and
B2 with A2; samples are not pooled. Wall time includes the complete process,
JSON and statistics capture.

| Hybrid program | Threads | Baseline A1 / A2 | Prepared B1 / B2 |
| --- | ---: | ---: | ---: |
| Queens 02, n=5 | 1 | 31.779 / 31.769 | 30.251 / 30.263 |
| Queens 02, n=5 | 4 | 137.341 / 136.477 | 131.427 / 130.813 |
| Queens 03, n=6 | 4 | 33.388 / 30.388 | 29.460 / 28.790 |
| Queens 02, n=4 | 4 | 18.386 / 16.421 | 17.627 / 17.824 |

Queens 02 at n=5 is about 4–5% faster in both pairings. Other cases show drift
or reversals, including Queens 02 at n=4. No eager cell has a wall increase
above 5% in both pairings; that observation does not establish unchanged eager
cost. Four-thread hybrid execution still trails one thread substantially on
the representative queens workloads. These results do not isolate either
change's contribution or establish a general speedup or Metal performance gain.

Each RSS cell is the median of two separately measured process peaks. Typical
paired increases are 0.1–0.2 MiB. RSS excludes the fixed helper, may include
waited descendants, and is not simultaneous process-tree or device memory.

Caching preparation changes charged work and retained capacity. Queens 02 at
n=5 uses 3,394,271 baseline source-work units and 3,395,219 afterward with one
thread; substitutions remain 14,212. Successful ample-budget runs do not
establish identical admission at resource ceilings. Plan storage is charged,
and refused preparation publishes no plan. Reduct membership and final
original-constraint checks remain mandatory.

Reproduce with the maintained `grounding_comparison` example's `--study refutation`
option. Use its compiled baseline revision `df99d4bb` as the fixed runner and
use the baseline solver as `--helper` for every leg. Supply each measured solver
as `--zetesis` in A1/B1/B2/A2 order, first with `--workers 1` and then with
`--workers 4`, using a new `--report` path each time. The evidence retains both
profiles, all limits, source edits and generated workload hashes. These runs
use the 2 GiB solver allowance, 10-second process limit and 300-second campaign
limit described below; they make no cold-cache claim.

## Original-region checks

The refinement at
[`3108acfe`](https://github.com/GregoryGelfond/zetesis/commit/2685426214a16fa1431d3c21d64e1fc8d9798cdd)
checks original constraints after ordinary region narrowing, before splitting
or core membership. Necessary predicate tests and held positive-row selection
avoid constructing join bindings that cannot witness a constraint violation.
The final full-candidate check remains. The baseline is
[`9bb73da9`](https://github.com/GregoryGelfond/zetesis/commit/f99221cb849653551ebfa4a925b105b10fe04f47),
which checks these constraints after finding a core answer set.

Both executables were measured on September 21, 2026 UTC, in
old/new/new/old order, first at one CPU thread and then at four. Each leg compares
eager and explicit lazy requests on the same 20 workloads: all six queens
encodings at n=4 and n=5, encodings 01 and 03 also at n=6, and monotone-choice
and redundant-transitivity controls. Every request asks for all answer sets.
Clingo 5.8.2 supplies an untimed qualification census; these are not
zetesis-versus-clingo timing results.
The programs have no objectives, so this comparison does not exercise optimization.

Each cell has one warmup, four timed processes and two separate RSS processes.
The table spans the two leg medians for each executable, rather than pooling
samples. Times include the complete CLI request, JSON and statistics. The
[evidence for all 20 workloads](observations/hybrid-regions-3108acfe-evidence.json)
retains the individual observations, identities and dispositions.

| Program | Threads | Prior hybrid ms | Refined hybrid ms | Refined eager ms |
| --- | ---: | ---: | ---: | ---: |
| Queens 01, n=6 | 1 | 61.892–62.243 | 11.527–11.548 | 7.139–7.240 |
| Queens 01, n=6 | 4 | 62.953–63.016 | 36.373–37.060 | 7.866–7.969 |
| Queens 03, n=6 | 1 | 52.125–52.196 | 11.554–11.586 | 6.534–6.589 |
| Queens 03, n=6 | 4 | 53.104–53.551 | 36.459–36.730 | 7.274–7.961 |
| Queens 02, n=5 | 1 | work limit | 33.478–34.096 | 7.271–7.852 |
| Queens 02, n=5 | 4 | work limit | 142.782–146.976 | 7.274–7.882 |
| Queens 03, n=5 | 1 | 11.524–11.611 | 8.983–9.012 | 6.531–6.583 |
| Queens 03, n=5 | 4 | 11.541–11.561 | 12.865–12.926 | 6.513–6.562 |
| Monotone choices, n=10 | 1 | 13.353–13.356 | 6.397–6.505 | 5.893–6.494 |
| Monotone choices, n=10 | 4 | 11.611–11.659 | 6.589–7.826 | 5.887–6.508 |
| Redundant transitivity, n=16 | 1 | 11.638–11.665 | 12.299–12.592 | 10.297–10.335 |
| Redundant transitivity, n=16 | 4 | 11.558–11.574 | 12.162–12.256 | 10.340–10.403 |

The early checks reduce complete core candidates substantially on the rejection
workloads. These one-thread counters are identical in both repeated legs:

| Program | Prior → refined core answers | Accepted answers | Prior → refined source work |
| --- | ---: | ---: | ---: |
| Queens 01, n=6 | 720 → 4 | 4 | 4,625,076 → 616,106 |
| Queens 03, n=6 | 720 → 4 | 4 | 4,307,335 → 614,762 |
| Monotone choices, n=10 | 1,024 → 11 | 11 | 242,854 → 49,731 |
| Redundant transitivity, n=16 | 2 → 2 | 2 | 1,059,178 → 1,097,017 |

For Queens 01 at n=6, one-thread initial grounding remains
0.572–0.607 ms before and 0.565–0.566 ms after. Solving falls from
54.756–55.264 ms to 4.975–4.995 ms. The change is in searching and checking,
not a removal of initial source admission. `original_validation` includes
region preparation/checking and final checks; it overlaps candidate generation,
and worker-duration sums are not additive with solving wall time.

There are measured costs as well. Refined hybrid still trails eager on the
representative queens cases and is substantially slower with four threads than
with one. Queens 03 at n=5 regresses by about 11.5–11.8% against prior hybrid
at four threads. Redundant transitivity refutes no region: n=12 regresses
16.1–18.6% at four threads, and n=16 regresses about 5–8% at both settings.
Its n=16 source checks still visit 8,192 substitutions; extra preparation and
region checks add work. No eager cell has a wall increase above 5% in both
paired comparisons, which is a bounded observation rather than proof of
unchanged eager cost. The no-stream Queens 06 n=4 control also rises by
about 0.54–0.59 ms at four threads; most of that difference is outside the
recorded driver interval, so it is not evidence of a source-checking cause.

Separate peak-RSS results illustrate the retained storage tradeoff:

| Program | Threads | Prior hybrid MiB | Refined hybrid MiB | Refined eager MiB |
| --- | ---: | ---: | ---: | ---: |
| Queens 01, n=6 | 1 | 13.44–13.57 | 13.40–13.44 | 13.41–13.42 |
| Queens 01, n=6 | 4 | 13.84–13.89 | 13.73–13.73 | 13.59–13.70 |
| Redundant transitivity, n=16 | 1 | 13.58–13.65 | 13.51–13.54 | 16.39–16.41 |
| Redundant transitivity, n=16 | 4 | 13.64–13.68 | 13.58–13.70 | 17.16–17.18 |

The refinement does not establish a general memory reduction. Redundant
transitivity retains 547 hybrid roots versus 4,643 eager roots at n=16, with
lower hybrid RSS here, but the old hybrid schedule already had that smaller
core. Root counts are not byte counts.

All four refined campaigns pass **340 of 340 positions** each. Every baseline
campaign records one Queens 02 n=5 hybrid refusal at its 10,000,000-unit source
work ceiling, then leaves seven later positions in that cell unattempted. Across
the eight campaigns, **2,688 positions pass, four refuse and 28 are unattempted**.
The 2,528 complete native captures agree on full typed answer-set families,
displayed results, costs and multiplicity across revisions, profiles and worker
counts. The 160 clingo qualification captures agree on selected results; hidden
clingo interpretations are unavailable. Incomplete baseline prefixes provide
neither a completed timing baseline nor a speedup ratio.

The fixed protocol uses CPU execution, automatic membership selection, indexed
joins, region search, batch size 64 and one completion worker. Captures record
a 2 GiB solver allowance; source checking separately allows 10,000,000 work
units, 10,000,000 substitutions and 16 MiB cumulative copied scalar payload.
Each process has a 10-second limit and each campaign a 300-second limit;
output is bounded at 4 MiB per process and 128 MiB per campaign. The reports
also retain cleanup and normalization limits. The host ran macOS 26.6.2;
release builds used Rust 1.97.1 and LLVM 22.1.6. These small CPU workloads,
four timed samples per leg and two RSS samples do not establish a general
speedup, a fully lazy source pipeline or Metal performance.

Reproduce the population with the maintained `grounding_comparison` example's
`--study refutation` option, using one frozen comparison executable and the same
refined `--helper` for every leg. Supply the baseline or refined `--zetesis`
executable in old/new/new/old order, and repeat with `--workers 1` and
`--workers 4`; every `--report` path must be new. The retained comparison
runner was built from `5b50286a`, while the two measured native executables
have the revisions above. Their full source and binary identities, generated
workload hashes and queens constant edits are in the evidence file. See
[corpus measurements](benchmarking.md#a-smaller-fixed-population) for the library/example door.

## Historical full-candidate comparison

The following results retain the original full-candidate checking schedule at
`4a281c96`. They do not measure the later original-region checks above.

Streaming source constraints reduces the stored formula, but can increase the
number of core answer sets that must be checked. This comparison measures both
effects using the same zetesis executable. It does not establish a general
speedup or qualify GPU execution of the hybrid profile.

The measured source is
[`4a281c96`](https://github.com/GregoryGelfond/zetesis/commit/2678bda826a428e34082a62277ce0628471558db).
Explicit lazy formula execution retains producers and ineligible constraints,
then checks eligible source constraints against each core answer set. Complete
support and arithmetic admission still precede solving. See the
[grounding contract](../architecture/grounding.md#eager-and-lazy-execution).

### Matched eager and lazy requests

Both requests enumerate all answer sets with CPU execution, indexed joins,
region search, batch size 64 and one completion worker. Each worker setting
uses one warmup, four timed runs and two separate peak-RSS runs per profile.
Clingo supplies an untimed qualification census; it is not a timing competitor
in this experiment. Measurements ran on macOS 26.6.2 with Rust 1.97.1.

The table gives one-worker medians. Time includes the complete command-line
request, statistics and output; memory is separate peak resident host memory.
These are small workloads with short durations, not a scaling study.

| Program | Eager ms | Hybrid ms | Eager MiB | Hybrid MiB |
| --- | ---: | ---: | ---: | ---: |
| Queens 01, n=4 | 5.415 | 6.192 | 13.20 | 13.24 |
| Queens 01, n=5 | 6.197 | 12.217 | 13.41 | 13.41 |
| Queens 03, n=4 | 6.204 | 6.212 | 13.36 | 13.18 |
| Queens 03, n=5 | 6.194 | 10.721 | 13.48 | 13.38 |
| Queens 02, n=4 | 6.192 | 24.046 | 13.43 | 13.30 |
| Queens 06, n=4 | 6.198 | 6.190 | 13.59 | 13.55 |
| Monotone choices, n=6 | 4.656 | 5.433 | 12.63 | 12.70 |
| Redundant transitivity, n=8 | 6.197 | 6.190 | 13.26 | 12.88 |
| Redundant transitivity, n=12 | 6.171 | 7.702 | 14.38 | 13.23 |

The four-worker results show the same principal trade-off. Queens 02 took
6.196 ms eager and 24.961 ms hybrid. Redundant transitivity at n=12 took
6.182 ms and 7.730 ms, with peak RSS of 14.78 MiB and 13.41 MiB.

For Queens 02, hybrid execution checks **1,820 core answers to accept two**.
The eager constraints can prune earlier. Queens 06 is a control: its scoped
constraints remain eager, so it streams no constraint templates. Redundant
transitivity at n=12 keeps the same two core answers while reducing retained
formula roots from **2,043 to 315**. That saves about 8–9% peak RSS here, but
rechecking constraints costs time. Root counts are not byte counts.

Initial grounding time excludes streamed checks. Those checks occur during
solving and are recorded under `original_validation`. For redundant transitivity
at n=12 with one worker, initial grounding falls from 1.040 ms to 0.846 ms while
solving rises from 0.581 ms to 1.331 ms. Initial grounding alone would therefore
give an incomplete account of the work.

All **306 scheduled positions** passed. The **288 native captures** agree on
complete typed answer-set families, costs and displayed results across profiles,
repetitions and worker settings. The
[evidence](observations/hybrid-4a281c96-evidence.json) retains per-sample times,
separate RSS observations, source and executable hashes, work counts and family
checks. This is bounded experimental evidence, not a proof of implementation
correctness or a claim about larger instances.

Reproduce the workload and protocol through the maintained
[grounding comparison example](benchmarking.md#a-smaller-fixed-population), once with
`--workers 1` and once with `--workers 4`. It uses unchanged corpus files with
recorded constant substitutions and the library's generated workloads.

### Preservation of eager execution

A separate comparison uses the prior eager executable
[`3d7454d8`](https://github.com/GregoryGelfond/zetesis/commit/7161ec59b33a742f21bcc547d8139cbf7a1006fb)
and the measured source above in old/new/new/old order. Each leg has seven timed
runs and two separate RSS runs, at one and four workers. The four workloads are
dense transitive closure at sizes 20 and 40, a 1,000-edge chain and its arithmetic
variant. Auto grounding selects eager formula execution for the dense and
arithmetic cases; the ordinary chain is a lazy relational control.

Complete wall-time changes are near zero on the eager cases. This does not
establish unchanged cost at every stage: dense grounding is **1.7–4.4% slower**,
with unchanged work counts, and peak RSS is slightly higher. The chain control
has one faster four-worker leg that is not repeated. No speedup is attributed
to that observation.

All **576 positions** and **384 complete native families** passed their checks.
The [separate evidence](observations/eager-preservation-4a281c96.json) records the
leg medians and identities. These results preserve the distinction between
earlier [row-lending gains](grounding-row-lending.md), the old/new implementation
comparison, and the eager-versus-hybrid choice within one executable.
