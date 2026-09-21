# Eager and hybrid formula grounding

Streaming source constraints reduces the stored formula, but can increase the
number of core answer sets that must be checked. This comparison measures both
effects using the same zetesis executable. It does not establish a general
speedup or qualify GPU execution of the hybrid profile.

The measured source is
[`4a281c96`](https://github.com/GregoryGelfond/zetesis/commit/4a281c9612dc4ff22557341354b1ad610cfd9672).
Explicit lazy formula execution retains producers and ineligible constraints,
then checks eligible source constraints against each core answer set. Complete
support and arithmetic admission still precede solving. See the
[grounding contract](../architecture/grounding.md#eager-and-lazy-execution).

## Matched eager and lazy requests

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
[grounding comparison example](commands.md#measure-a-corpus), once with
`--workers 1` and once with `--workers 4`. It uses unchanged corpus files with
recorded constant substitutions and the library's generated workloads.

## Preservation of eager execution

A separate comparison uses the prior eager executable
[`3d7454d8`](https://github.com/GregoryGelfond/zetesis/commit/3d7454d810acec99121ee5b5615505b6ddc6eea7)
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
