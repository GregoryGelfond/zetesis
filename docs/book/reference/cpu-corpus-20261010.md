# CPU corpus comparison, 10 October 2026

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

This study compares candidate `5be088ee` with the preceding runtime checkpoint
`f3c0e185`, using the complete 94-program corpus. It measures the final combined
propagation, objective-preparation and arithmetic-checking changes; it does not
measure the cumulative improvement since released 0.4.0. The
[portable observations](observations/cpu-corpus-20261010.json) retain individual
elapsed times, stage timings, memory observations, costs and source identities.

Both versions used CPU execution with **14 workers** on an Apple M4 Pro. Each
campaign had its own **one-worker clingo 5.8.2** comparison. Automatic grounding
was actually eager on all 94 cases; explicit lazy grounding used the hybrid
formula route on all 94. Both solvers enumerated every answer or every tied
optimum. A qualification and a warmup preceded three timed repetitions and one
separate RSS observation per solver and case.

All four campaigns passed every one of their 1,128 positions, with no changed
inputs, faults or unresolved children. All 282 additional cross-campaign checks
agreed on full native answer families and on clingo's displayed families and
final optimum costs, preserving multiplicities. Clingo's hidden interpretations
are outside that comparison.

## Results and limits

| Grounding | Before total | Candidate total | Change | Paired clingo before | Paired clingo after | Native wins before → after |
|---|---:|---:|---:|---:|---:|---:|
| Automatic (eager) | 741.749 ms | 755.324 ms | +1.83% | 823.342 ms | 830.639 ms | 4 → 4 |
| Lazy (hybrid) | 4780.577 ms | 2299.372 ms | -51.90% | 825.000 ms | 813.247 ms | 3 → 3 |

Totals sum per-case wall medians; they are not one combined timed run. The
eager total increased 1.83%, while the lazy total fell 51.90%. The win tally is
unchanged: most corpus cases still favor clingo. The largest lazy reductions are
Queens variants 1–3: 62.79%, 66.29% and 33.65%. Together they account for nearly
all of the decrease. Lazy Queens 2 still takes about 1.09 seconds, against
0.124 seconds for its paired clingo run.

Whole-process time includes startup, input, parsing, grounding, solving and
captured JSON/statistics output. Three repetitions do not establish statistical
significance or a general speedup. This is a combined-change observation, not
evidence isolating any one optimization.

The four campaigns ran in the order before-auto, candidate-auto, candidate-lazy,
before-lazy. The host was on AC power with no reported competing intensive work;
one-minute load observations ranged from 7.69 to 19.62. Paired clingo totals
rose 0.89% in the automatic comparison and fell 1.42% in the lazy comparison.
Many small-case changes are around 1.5 ms and cannot be attributed confidently
to solver work. Per-case ranges and both reference campaigns are retained.

## Grounding, solving and memory

These are sums of individual host-stage medians, in milliseconds. Driver stages
exclude input loading, statistics output and the JSON envelope. Lazy grounding
also performs work during the solving stage; these columns do not isolate every
grounding operation. Overlapping detailed phases must not be added to wall time.

| Campaign | Source preparation | Grounding stage | Solving stage | Output stage | Median per-case RSS | Largest per-case RSS |
|---|---:|---:|---:|---:|---:|---:|
| before-eager | 66.167 | 107.322 | 72.078 | 17.879 | 15.289 MiB | 19.703 MiB |
| after-eager | 66.230 | 107.943 | 71.677 | 17.639 | 15.328 MiB | 19.312 MiB |
| after-lazy | 64.275 | 101.967 | 1640.296 | 18.440 | 15.445 MiB | 20.188 MiB |
| before-lazy | 65.150 | 106.708 | 4115.769 | 18.303 | 15.422 MiB | 21.047 MiB |

The eager grounding and solving totals are nearly unchanged. The lazy solving
stage falls from 4.116 to 1.640 seconds. The median per-case RSS is nearly flat;
the largest eager observation falls from 19.703 to 19.313 MiB and the largest lazy
observation from 21.047 to 20.188 MiB. One memory sample per case does not establish
a stable reduction. RSS is a separate fresh-child measurement, excluding its
helper; it is not simultaneous process-tree RSS or device memory. Never sum these
peaks as if they represented one allocation.

## All 94 programs

Times below are milliseconds. “Ratio” is candidate zetesis / its paired clingo;
smaller favors zetesis. Change is candidate / before minus one. Ratios and win
counts use unrounded values. Each table retains both reference campaigns. Program
links point to the candidate source; the observations also retain file hashes.

### Automatic grounding (observed eager)

| Program | Before | Candidate | Change | Clingo before | Clingo after | Ratio |
|---|---:|---:|---:|---:|---:|---:|
| [equality-generalized-tsp/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/equality-generalized-tsp/01-basic.lp) | 6.356 | 6.340 | -0.26% | 4.668 | 4.715 | 1.345 |
| [equality-generalized-tsp/02-larger](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/equality-generalized-tsp/02-larger.lp) | 7.783 | 7.784 | +0.00% | 4.676 | 4.664 | 1.669 |
| [equality-generalized-tsp/03-unreachable-subset-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp) | 6.337 | 6.287 | -0.79% | 4.670 | 4.657 | 1.350 |
| [shortest-path/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/01-basic.lp) | 6.325 | 6.289 | -0.56% | 4.685 | 4.714 | 1.334 |
| [shortest-path/v01/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/02-start-equals-end.lp) | 6.320 | 6.347 | +0.42% | 4.693 | 4.706 | 1.349 |
| [shortest-path/v01/03-zero-cost-detour](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/03-zero-cost-detour.lp) | 6.288 | 6.304 | +0.26% | 4.663 | 4.693 | 1.343 |
| [shortest-path/v01/04-no-path](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/04-no-path.lp) | 6.339 | 6.342 | +0.06% | 4.677 | 4.707 | 1.347 |
| [shortest-path/v01/05-multi-path](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/05-multi-path.lp) | 7.788 | 6.242 | -19.85% | 4.704 | 4.688 | 1.331 |
| [shortest-path/v01/06-layered-dag](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/06-layered-dag.lp) | 9.395 | 9.335 | -0.63% | 6.178 | 6.171 | 1.513 |
| [shortest-path/v01/07-cycles](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/07-cycles.lp) | 7.829 | 7.846 | +0.22% | 4.685 | 4.663 | 1.683 |
| [shortest-path/v01/08-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/08-negative-weights.lp) | 6.310 | 6.310 | +0.01% | 4.668 | 4.656 | 1.355 |
| [shortest-path/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/01-basic.lp) | 6.302 | 6.310 | +0.12% | 4.671 | 4.668 | 1.352 |
| [shortest-path/v02/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/02-start-equals-end.lp) | 6.268 | 6.309 | +0.66% | 4.682 | 4.666 | 1.352 |
| [shortest-path/v02/03-before-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/03-before-forces-detour.lp) | 6.255 | 6.248 | -0.11% | 4.652 | 4.678 | 1.336 |
| [shortest-path/v02/04-after-forces-extension](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/04-after-forces-extension.lp) | 6.288 | 6.295 | +0.12% | 4.682 | 4.673 | 1.347 |
| [shortest-path/v02/05-ordering-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/05-ordering-unsat.lp) | 6.303 | 6.321 | +0.30% | 4.671 | 4.678 | 1.351 |
| [shortest-path/v02/06-layered-dag-before](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/06-layered-dag-before.lp) | 9.343 | 9.342 | -0.01% | 6.207 | 6.188 | 1.510 |
| [shortest-path/v02/07-layered-dag-before-after](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp) | 9.316 | 9.329 | +0.14% | 6.217 | 6.212 | 1.502 |
| [shortest-path/v02/08-tie-break-under-ordering](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp) | 6.309 | 6.275 | -0.53% | 4.693 | 4.683 | 1.340 |
| [shortest-path/v02/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/09-negative-weights.lp) | 6.310 | 6.253 | -0.91% | 4.678 | 4.691 | 1.333 |
| [shortest-path/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/01-basic.lp) | 6.289 | 6.327 | +0.60% | 4.656 | 4.688 | 1.350 |
| [shortest-path/v03/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/02-start-equals-end.lp) | 6.276 | 6.261 | -0.24% | 4.656 | 4.690 | 1.335 |
| [shortest-path/v03/03-budget-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/03-budget-forces-detour.lp) | 6.281 | 6.290 | +0.14% | 4.668 | 4.710 | 1.335 |
| [shortest-path/v03/04-cost-at-cap-allowed](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp) | 6.278 | 6.271 | -0.11% | 4.719 | 4.659 | 1.346 |
| [shortest-path/v03/05-budget-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/05-budget-unsat.lp) | 6.303 | 6.323 | +0.33% | 4.664 | 4.668 | 1.355 |
| [shortest-path/v03/06-layered-dag-cap](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/06-layered-dag-cap.lp) | 10.813 | 10.794 | -0.17% | 6.178 | 7.697 | 1.402 |
| [shortest-path/v03/07-layered-dag-tight-cap](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp) | 9.385 | 10.869 | +15.81% | 7.660 | 6.192 | 1.755 |
| [shortest-path/v03/08-tie-break-under-cap](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp) | 6.306 | 7.392 | +17.22% | 4.674 | 4.687 | 1.577 |
| [shortest-path/v03/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/09-negative-weights.lp) | 6.277 | 6.298 | +0.33% | 4.668 | 4.666 | 1.350 |
| [shortest-path/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/01-basic.lp) | 6.244 | 6.266 | +0.34% | 4.661 | 4.670 | 1.342 |
| [shortest-path/v04/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/02-start-equals-end.lp) | 6.306 | 6.284 | -0.35% | 4.677 | 4.662 | 1.348 |
| [shortest-path/v04/03-before-forces-detour-within-budget](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp) | 6.258 | 6.263 | +0.08% | 4.654 | 4.648 | 1.347 |
| [shortest-path/v04/04-ordering-violates-budget](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp) | 6.317 | 6.303 | -0.22% | 4.662 | 4.654 | 1.354 |
| [shortest-path/v04/05-after-and-budget-interact](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp) | 6.254 | 6.286 | +0.52% | 4.646 | 4.655 | 1.350 |
| [shortest-path/v04/06-layered-dag-ordering-cap](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp) | 10.816 | 10.843 | +0.25% | 6.173 | 6.166 | 1.759 |
| [shortest-path/v04/07-layered-dag-combined](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/07-layered-dag-combined.lp) | 10.262 | 10.824 | +5.48% | 6.172 | 6.181 | 1.751 |
| [shortest-path/v04/08-tie-break-under-ordering-and-cap](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp) | 7.814 | 7.833 | +0.25% | 4.642 | 4.703 | 1.665 |
| [shortest-path/v04/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/09-negative-weights.lp) | 6.537 | 7.767 | +18.81% | 4.660 | 4.672 | 1.662 |
| [task-allocation/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-01/01-basic.lp) | 6.307 | 6.326 | +0.29% | 4.678 | 4.676 | 1.353 |
| [task-allocation/v01/02-agent-reuse](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-01/02-agent-reuse.lp) | 6.325 | 6.286 | -0.61% | 4.675 | 4.681 | 1.343 |
| [task-allocation/v01/03-selective-compatibility](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-01/03-selective-compatibility.lp) | 6.333 | 6.378 | +0.71% | 4.689 | 4.721 | 1.351 |
| [task-allocation/v01/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp) | 6.261 | 6.249 | -0.20% | 4.686 | 4.697 | 1.330 |
| [task-allocation/v01/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-01/05-larger-mix.lp) | 6.266 | 7.755 | +23.76% | 4.696 | 4.701 | 1.650 |
| [task-allocation/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-02/01-basic.lp) | 6.259 | 6.283 | +0.39% | 4.669 | 4.701 | 1.337 |
| [task-allocation/v02/02-makespan-tiebreak](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp) | 6.304 | 6.297 | -0.11% | 4.693 | 4.677 | 1.346 |
| [task-allocation/v02/03-cost-dominates](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-02/03-cost-dominates.lp) | 6.286 | 6.265 | -0.34% | 4.671 | 4.681 | 1.338 |
| [task-allocation/v02/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp) | 6.302 | 6.325 | +0.38% | 4.683 | 4.687 | 1.350 |
| [task-allocation/v02/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-02/05-larger-mix.lp) | 9.273 | 9.281 | +0.08% | 4.658 | 4.668 | 1.988 |
| [task-allocation/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-03/01-basic.lp) | 6.305 | 6.314 | +0.15% | 4.672 | 4.663 | 1.354 |
| [task-allocation/v03/02-multiple-groups](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-03/02-multiple-groups.lp) | 6.271 | 6.274 | +0.04% | 4.700 | 4.684 | 1.339 |
| [task-allocation/v03/03-mixed-grouped-ungrouped](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp) | 6.309 | 6.335 | +0.41% | 4.681 | 4.688 | 1.351 |
| [task-allocation/v03/04-incompatible-group-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp) | 6.306 | 6.284 | -0.35% | 4.667 | 4.678 | 1.343 |
| [task-allocation/v03/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-03/05-larger-mix.lp) | 6.453 | 6.296 | -2.44% | 4.705 | 4.704 | 1.339 |
| [task-allocation/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-04/01-basic.lp) | 9.363 | 9.313 | -0.53% | 24.271 | 24.288 | 0.383 |
| [task-allocation/v04/02-precedence](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-04/02-precedence.lp) | 7.905 | 7.860 | -0.57% | 6.200 | 6.198 | 1.268 |
| [task-allocation/v04/03-agent-serialization](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-04/03-agent-serialization.lp) | 7.810 | 9.338 | +19.57% | 25.763 | 25.780 | 0.362 |
| [task-allocation/v04/04-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp) | 7.772 | 7.830 | +0.75% | 4.732 | 4.691 | 1.669 |
| [task-allocation/v04/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-04/05-larger-mix.lp) | 31.168 | 30.543 | -2.00% | 194.227 | 197.876 | 0.154 |
| [traveling-salesman/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-01/01-basic.lp) | 7.936 | 7.903 | -0.41% | 4.768 | 4.749 | 1.664 |
| [traveling-salesman/v01/02-multiple-tours](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-01/02-multiple-tours.lp) | 7.802 | 7.804 | +0.02% | 4.662 | 4.664 | 1.673 |
| [traveling-salesman/v01/03-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-01/03-asymmetric.lp) | 7.806 | 7.797 | -0.11% | 5.847 | 4.648 | 1.678 |
| [traveling-salesman/v01/04-subtour-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp) | 6.332 | 6.283 | -0.77% | 4.696 | 4.696 | 1.338 |
| [traveling-salesman/v01/05-ring](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-01/05-ring.lp) | 7.838 | 9.321 | +18.91% | 6.205 | 6.240 | 1.494 |
| [traveling-salesman/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-02/01-basic.lp) | 6.256 | 6.281 | +0.41% | 4.648 | 4.683 | 1.341 |
| [traveling-salesman/v02/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-02/02-single-salesman.lp) | 6.277 | 6.325 | +0.76% | 4.685 | 4.700 | 1.346 |
| [traveling-salesman/v02/03-too-many-salesmen-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp) | 6.250 | 6.281 | +0.49% | 4.683 | 4.664 | 1.347 |
| [traveling-salesman/v02/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp) | 6.310 | 6.311 | +0.01% | 4.670 | 4.695 | 1.344 |
| [traveling-salesman/v02/05-larger-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp) | 7.401 | 7.802 | +5.43% | 4.671 | 4.676 | 1.668 |
| [traveling-salesman/v02/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp) | 6.309 | 7.823 | +23.99% | 4.667 | 4.658 | 1.680 |
| [traveling-salesman/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-03/01-basic.lp) | 6.264 | 6.280 | +0.26% | 4.676 | 4.661 | 1.347 |
| [traveling-salesman/v03/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-03/02-single-salesman.lp) | 6.273 | 6.295 | +0.35% | 4.693 | 4.671 | 1.348 |
| [traveling-salesman/v03/03-depot-crossing-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp) | 6.269 | 6.302 | +0.53% | 4.671 | 4.683 | 1.346 |
| [traveling-salesman/v03/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp) | 7.788 | 7.788 | +0.01% | 4.667 | 4.672 | 1.667 |
| [traveling-salesman/v03/05-larger-three-depots](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp) | 7.755 | 7.873 | +1.52% | 4.679 | 4.661 | 1.689 |
| [traveling-salesman/v03/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp) | 7.132 | 7.810 | +9.50% | 4.687 | 4.669 | 1.673 |
| [traveling-salesman/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-04/01-basic.lp) | 7.500 | 7.802 | +4.04% | 4.665 | 4.685 | 1.665 |
| [traveling-salesman/v04/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-04/02-single-salesman.lp) | 7.766 | 7.817 | +0.65% | 4.652 | 4.651 | 1.681 |
| [traveling-salesman/v04/03-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp) | 7.783 | 7.790 | +0.09% | 4.661 | 4.651 | 1.675 |
| [traveling-salesman/v04/04-depot-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp) | 6.257 | 7.770 | +24.19% | 4.674 | 4.656 | 1.669 |
| [traveling-salesman/v04/05-three-depots-asymmetric-times](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp) | 9.293 | 9.297 | +0.04% | 4.676 | 4.659 | 1.995 |
| [traveling-salesman/v04/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp) | 9.340 | 9.362 | +0.24% | 4.640 | 4.694 | 1.995 |
| [traveling-salesman/v05/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-05/01-basic.lp) | 7.749 | 7.770 | +0.26% | 4.655 | 6.176 | 1.258 |
| [traveling-salesman/v05/02-tight-bound-exact](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp) | 7.753 | 7.798 | +0.58% | 4.667 | 4.687 | 1.664 |
| [traveling-salesman/v05/03-revisit-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp) | 6.255 | 7.757 | +24.01% | 4.647 | 4.652 | 1.667 |
| [traveling-salesman/v05/04-depot-and-vertex-revisits](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp) | 6.288 | 7.775 | +23.65% | 4.647 | 4.645 | 1.674 |
| [traveling-salesman/v05/05-three-depots-mixed](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp) | 9.152 | 9.303 | +1.65% | 4.675 | 6.185 | 1.504 |
| [traveling-salesman/v05/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp) | 9.296 | 9.312 | +0.17% | 4.689 | 4.688 | 1.986 |
| [n-queens/v01](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/standalone/n-queens/variant-01.lp) | 12.479 | 10.801 | -13.45% | 6.213 | 6.214 | 1.738 |
| [n-queens/v02](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/standalone/n-queens/variant-02.lp) | 29.926 | 29.423 | -1.68% | 126.853 | 128.307 | 0.229 |
| [n-queens/v03](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/standalone/n-queens/variant-03.lp) | 12.458 | 13.904 | +11.61% | 6.210 | 6.211 | 2.239 |
| [n-queens/v04](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/standalone/n-queens/variant-04.lp) | 9.378 | 9.359 | -0.20% | 6.204 | 6.188 | 1.512 |
| [n-queens/v05](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/standalone/n-queens/variant-05.lp) | 9.407 | 9.336 | -0.76% | 6.214 | 6.229 | 1.499 |
| [n-queens/v06](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/standalone/n-queens/variant-06.lp) | 9.420 | 9.369 | -0.55% | 6.189 | 6.180 | 1.516 |
| [send-money/send-money](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/standalone/send-money/send-money.lp) | 13.894 | 13.863 | -0.22% | 12.213 | 12.211 | 1.135 |
| **All 94: sum of medians** | **741.749** | **755.324** | **+1.83%** | **823.342** | **830.639** | **0.909** |

### Explicit lazy grounding (observed hybrid)

| Program | Before | Candidate | Change | Clingo before | Clingo after | Ratio |
|---|---:|---:|---:|---:|---:|---:|
| [equality-generalized-tsp/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/equality-generalized-tsp/01-basic.lp) | 7.984 | 7.848 | -1.71% | 4.718 | 4.708 | 1.667 |
| [equality-generalized-tsp/02-larger](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/equality-generalized-tsp/02-larger.lp) | 7.784 | 7.784 | +0.00% | 4.659 | 4.694 | 1.658 |
| [equality-generalized-tsp/03-unreachable-subset-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp) | 6.367 | 6.313 | -0.85% | 4.682 | 4.679 | 1.349 |
| [shortest-path/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/01-basic.lp) | 6.287 | 6.297 | +0.16% | 4.679 | 4.683 | 1.345 |
| [shortest-path/v01/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/02-start-equals-end.lp) | 6.329 | 6.390 | +0.96% | 4.694 | 4.696 | 1.361 |
| [shortest-path/v01/03-zero-cost-detour](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/03-zero-cost-detour.lp) | 6.279 | 6.349 | +1.11% | 4.697 | 4.680 | 1.357 |
| [shortest-path/v01/04-no-path](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/04-no-path.lp) | 6.318 | 6.373 | +0.87% | 4.717 | 4.699 | 1.356 |
| [shortest-path/v01/05-multi-path](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/05-multi-path.lp) | 6.281 | 6.300 | +0.30% | 4.682 | 4.691 | 1.343 |
| [shortest-path/v01/06-layered-dag](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/06-layered-dag.lp) | 9.332 | 9.287 | -0.48% | 6.173 | 6.212 | 1.495 |
| [shortest-path/v01/07-cycles](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/07-cycles.lp) | 7.775 | 7.857 | +1.05% | 4.669 | 4.676 | 1.680 |
| [shortest-path/v01/08-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-01/08-negative-weights.lp) | 6.315 | 6.360 | +0.71% | 4.666 | 4.671 | 1.361 |
| [shortest-path/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/01-basic.lp) | 6.284 | 6.313 | +0.46% | 4.673 | 4.712 | 1.340 |
| [shortest-path/v02/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/02-start-equals-end.lp) | 6.341 | 6.334 | -0.11% | 4.691 | 4.677 | 1.354 |
| [shortest-path/v02/03-before-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/03-before-forces-detour.lp) | 6.319 | 6.307 | -0.19% | 4.683 | 4.668 | 1.351 |
| [shortest-path/v02/04-after-forces-extension](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/04-after-forces-extension.lp) | 6.315 | 6.345 | +0.47% | 4.676 | 4.689 | 1.353 |
| [shortest-path/v02/05-ordering-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/05-ordering-unsat.lp) | 6.280 | 6.287 | +0.12% | 4.683 | 4.702 | 1.337 |
| [shortest-path/v02/06-layered-dag-before](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/06-layered-dag-before.lp) | 9.241 | 9.323 | +0.89% | 6.212 | 6.209 | 1.502 |
| [shortest-path/v02/07-layered-dag-before-after](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp) | 9.350 | 9.343 | -0.08% | 6.194 | 6.192 | 1.509 |
| [shortest-path/v02/08-tie-break-under-ordering](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp) | 6.294 | 6.334 | +0.64% | 4.704 | 4.672 | 1.356 |
| [shortest-path/v02/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-02/09-negative-weights.lp) | 7.808 | 6.253 | -19.90% | 4.677 | 4.677 | 1.337 |
| [shortest-path/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/01-basic.lp) | 6.283 | 6.301 | +0.28% | 4.688 | 4.659 | 1.352 |
| [shortest-path/v03/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/02-start-equals-end.lp) | 6.301 | 6.244 | -0.91% | 4.676 | 4.683 | 1.333 |
| [shortest-path/v03/03-budget-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/03-budget-forces-detour.lp) | 6.317 | 6.335 | +0.29% | 4.666 | 4.712 | 1.345 |
| [shortest-path/v03/04-cost-at-cap-allowed](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp) | 6.271 | 6.289 | +0.28% | 4.669 | 4.703 | 1.337 |
| [shortest-path/v03/05-budget-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/05-budget-unsat.lp) | 6.281 | 6.277 | -0.06% | 4.666 | 4.705 | 1.334 |
| [shortest-path/v03/06-layered-dag-cap](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/06-layered-dag-cap.lp) | 12.277 | 10.816 | -11.90% | 6.169 | 6.192 | 1.747 |
| [shortest-path/v03/07-layered-dag-tight-cap](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp) | 10.782 | 10.858 | +0.70% | 6.167 | 6.170 | 1.760 |
| [shortest-path/v03/08-tie-break-under-cap](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp) | 6.273 | 6.263 | -0.15% | 4.660 | 4.688 | 1.336 |
| [shortest-path/v03/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-03/09-negative-weights.lp) | 6.296 | 6.318 | +0.36% | 4.682 | 4.676 | 1.351 |
| [shortest-path/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/01-basic.lp) | 6.290 | 6.347 | +0.90% | 4.660 | 4.681 | 1.356 |
| [shortest-path/v04/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/02-start-equals-end.lp) | 6.266 | 6.277 | +0.18% | 4.660 | 4.682 | 1.341 |
| [shortest-path/v04/03-before-forces-detour-within-budget](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp) | 6.250 | 6.258 | +0.12% | 4.663 | 4.688 | 1.335 |
| [shortest-path/v04/04-ordering-violates-budget](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp) | 6.292 | 6.308 | +0.25% | 4.641 | 4.707 | 1.340 |
| [shortest-path/v04/05-after-and-budget-interact](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp) | 6.303 | 7.736 | +22.73% | 4.667 | 4.707 | 1.643 |
| [shortest-path/v04/06-layered-dag-ordering-cap](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp) | 10.845 | 10.806 | -0.36% | 6.186 | 6.180 | 1.749 |
| [shortest-path/v04/07-layered-dag-combined](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/07-layered-dag-combined.lp) | 10.669 | 10.915 | +2.30% | 6.200 | 6.182 | 1.766 |
| [shortest-path/v04/08-tie-break-under-ordering-and-cap](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp) | 7.794 | 7.809 | +0.18% | 4.654 | 4.677 | 1.670 |
| [shortest-path/v04/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/shortest-path/variant-04/09-negative-weights.lp) | 6.288 | 7.352 | +16.92% | 4.677 | 4.694 | 1.566 |
| [task-allocation/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-01/01-basic.lp) | 6.279 | 6.307 | +0.46% | 4.649 | 4.708 | 1.340 |
| [task-allocation/v01/02-agent-reuse](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-01/02-agent-reuse.lp) | 6.243 | 6.295 | +0.83% | 4.681 | 4.704 | 1.338 |
| [task-allocation/v01/03-selective-compatibility](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-01/03-selective-compatibility.lp) | 6.263 | 6.011 | -4.02% | 4.681 | 4.707 | 1.277 |
| [task-allocation/v01/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp) | 6.262 | 6.269 | +0.10% | 4.683 | 4.701 | 1.334 |
| [task-allocation/v01/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-01/05-larger-mix.lp) | 6.290 | 6.397 | +1.70% | 4.699 | 4.718 | 1.356 |
| [task-allocation/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-02/01-basic.lp) | 6.274 | 6.253 | -0.33% | 4.684 | 4.693 | 1.332 |
| [task-allocation/v02/02-makespan-tiebreak](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp) | 6.282 | 6.288 | +0.10% | 4.689 | 4.717 | 1.333 |
| [task-allocation/v02/03-cost-dominates](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-02/03-cost-dominates.lp) | 6.249 | 6.271 | +0.35% | 4.681 | 4.672 | 1.342 |
| [task-allocation/v02/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp) | 6.304 | 6.316 | +0.19% | 4.690 | 4.703 | 1.343 |
| [task-allocation/v02/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-02/05-larger-mix.lp) | 7.825 | 9.215 | +17.76% | 4.710 | 4.688 | 1.965 |
| [task-allocation/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-03/01-basic.lp) | 6.325 | 6.353 | +0.45% | 4.659 | 4.679 | 1.358 |
| [task-allocation/v03/02-multiple-groups](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-03/02-multiple-groups.lp) | 6.293 | 6.284 | -0.13% | 4.677 | 4.702 | 1.336 |
| [task-allocation/v03/03-mixed-grouped-ungrouped](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp) | 6.299 | 6.297 | -0.03% | 4.694 | 4.700 | 1.340 |
| [task-allocation/v03/04-incompatible-group-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp) | 6.228 | 6.329 | +1.61% | 4.695 | 4.705 | 1.345 |
| [task-allocation/v03/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-03/05-larger-mix.lp) | 7.808 | 7.790 | -0.23% | 4.692 | 4.712 | 1.653 |
| [task-allocation/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-04/01-basic.lp) | 14.080 | 14.132 | +0.37% | 24.282 | 23.956 | 0.590 |
| [task-allocation/v04/02-precedence](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-04/02-precedence.lp) | 9.413 | 7.823 | -16.89% | 6.210 | 6.192 | 1.263 |
| [task-allocation/v04/03-agent-serialization](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-04/03-agent-serialization.lp) | 9.322 | 9.352 | +0.31% | 25.834 | 25.832 | 0.362 |
| [task-allocation/v04/04-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp) | 7.835 | 7.901 | +0.84% | 4.666 | 4.698 | 1.682 |
| [task-allocation/v04/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/task-allocation/variant-04/05-larger-mix.lp) | 177.872 | 173.080 | -2.69% | 198.027 | 188.771 | 0.917 |
| [traveling-salesman/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-01/01-basic.lp) | 7.932 | 7.998 | +0.83% | 4.758 | 4.753 | 1.683 |
| [traveling-salesman/v01/02-multiple-tours](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-01/02-multiple-tours.lp) | 7.778 | 7.819 | +0.53% | 4.662 | 4.676 | 1.672 |
| [traveling-salesman/v01/03-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-01/03-asymmetric.lp) | 7.696 | 7.737 | +0.53% | 6.070 | 4.662 | 1.659 |
| [traveling-salesman/v01/04-subtour-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp) | 6.319 | 6.318 | -0.01% | 4.709 | 4.691 | 1.347 |
| [traveling-salesman/v01/05-ring](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-01/05-ring.lp) | 9.315 | 9.362 | +0.51% | 6.185 | 6.199 | 1.510 |
| [traveling-salesman/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-02/01-basic.lp) | 7.623 | 6.263 | -17.83% | 4.686 | 4.667 | 1.342 |
| [traveling-salesman/v02/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-02/02-single-salesman.lp) | 6.296 | 6.325 | +0.47% | 4.666 | 4.705 | 1.344 |
| [traveling-salesman/v02/03-too-many-salesmen-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp) | 6.298 | 6.274 | -0.38% | 4.660 | 4.686 | 1.339 |
| [traveling-salesman/v02/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp) | 6.285 | 6.313 | +0.45% | 4.699 | 4.699 | 1.344 |
| [traveling-salesman/v02/05-larger-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp) | 6.247 | 6.260 | +0.21% | 4.648 | 4.675 | 1.339 |
| [traveling-salesman/v02/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp) | 7.801 | 7.813 | +0.16% | 4.675 | 4.678 | 1.670 |
| [traveling-salesman/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-03/01-basic.lp) | 6.271 | 6.319 | +0.78% | 4.697 | 4.696 | 1.346 |
| [traveling-salesman/v03/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-03/02-single-salesman.lp) | 6.301 | 6.240 | -0.98% | 4.673 | 4.705 | 1.326 |
| [traveling-salesman/v03/03-depot-crossing-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp) | 6.301 | 6.286 | -0.23% | 4.683 | 4.705 | 1.336 |
| [traveling-salesman/v03/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp) | 6.292 | 6.256 | -0.58% | 4.677 | 4.707 | 1.329 |
| [traveling-salesman/v03/05-larger-three-depots](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp) | 7.823 | 7.551 | -3.47% | 4.654 | 4.701 | 1.606 |
| [traveling-salesman/v03/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp) | 7.796 | 7.612 | -2.36% | 4.690 | 4.710 | 1.616 |
| [traveling-salesman/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-04/01-basic.lp) | 7.760 | 7.752 | -0.10% | 4.646 | 4.704 | 1.648 |
| [traveling-salesman/v04/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-04/02-single-salesman.lp) | 7.790 | 7.821 | +0.39% | 4.657 | 4.676 | 1.673 |
| [traveling-salesman/v04/03-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp) | 6.225 | 7.824 | +25.70% | 4.695 | 4.665 | 1.677 |
| [traveling-salesman/v04/04-depot-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp) | 6.270 | 6.290 | +0.31% | 4.655 | 4.668 | 1.347 |
| [traveling-salesman/v04/05-three-depots-asymmetric-times](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp) | 9.231 | 9.220 | -0.12% | 4.681 | 4.668 | 1.975 |
| [traveling-salesman/v04/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp) | 9.317 | 9.372 | +0.58% | 4.682 | 4.650 | 2.015 |
| [traveling-salesman/v05/01-basic](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-05/01-basic.lp) | 7.778 | 7.744 | -0.43% | 4.684 | 4.689 | 1.652 |
| [traveling-salesman/v05/02-tight-bound-exact](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp) | 7.811 | 7.815 | +0.06% | 4.649 | 4.679 | 1.670 |
| [traveling-salesman/v05/03-revisit-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp) | 6.269 | 7.792 | +24.30% | 4.679 | 4.670 | 1.668 |
| [traveling-salesman/v05/04-depot-and-vertex-revisits](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp) | 7.747 | 7.781 | +0.45% | 4.704 | 4.658 | 1.671 |
| [traveling-salesman/v05/05-three-depots-mixed](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp) | 9.231 | 9.154 | -0.83% | 4.694 | 4.676 | 1.958 |
| [traveling-salesman/v05/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp) | 9.338 | 9.309 | -0.31% | 4.655 | 4.666 | 1.995 |
| [n-queens/v01](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/standalone/n-queens/variant-01.lp) | 379.140 | 141.081 | -62.79% | 6.298 | 6.231 | 22.643 |
| [n-queens/v02](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/standalone/n-queens/variant-02.lp) | 3223.158 | 1086.488 | -66.29% | 123.870 | 123.818 | 8.775 |
| [n-queens/v03](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/standalone/n-queens/variant-03.lp) | 291.661 | 193.504 | -33.65% | 6.325 | 6.280 | 30.812 |
| [n-queens/v04](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/standalone/n-queens/variant-04.lp) | 9.443 | 7.876 | -16.59% | 6.206 | 6.241 | 1.262 |
| [n-queens/v05](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/standalone/n-queens/variant-05.lp) | 9.447 | 9.437 | -0.10% | 6.204 | 6.223 | 1.517 |
| [n-queens/v06](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/standalone/n-queens/variant-06.lp) | 9.442 | 9.409 | -0.35% | 6.226 | 6.190 | 1.520 |
| [send-money/send-money](https://github.com/GregoryGelfond/zetesis/blob/5be088ee71d71404ab4b9e6f8ce1180b62fcb80d/examples/correctness/standalone/send-money/send-money.lp) | 52.038 | 48.489 | -6.82% | 13.730 | 12.252 | 3.958 |
| **All 94: sum of medians** | **4780.577** | **2299.372** | **-51.90%** | **825.000** | **813.247** | **2.827** |

## Identity and reproduction

| Artifact | Identity |
|---|---|
| Before runtime source | `f3c0e185fd61bb86c48b39d07b4ca654b86d66c8` |
| Candidate source | `5be088ee71d71404ab4b9e6f8ce1180b62fcb80d` |
| Before executable SHA-256 | `11082adcfe955aaeb40f81250a2557fe5c834559271a370fc8979331a05078b5` |
| Candidate executable SHA-256 | `0478d793a84252f3480817f8d4b1be3b69b788113a3ec5728e5c432c735ff115` |
| Benchmark executable SHA-256 | `ee98f853fca2e1f14f751f50873099d47710aaf2e50d0a8d98ece8593b44674e` |
| clingo executable SHA-256 | `31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015` |

The before executable was subsequently qualified at `f97f683c`; that checkpoint
changed proof, test and contract records without changing the measured runtime.
Both executables still report version 0.4.0. Commit and binary identities, rather
than that version string, distinguish these observations. Later documentation or
release changes do not retroactively qualify a different executable.

Every native sample recorded a **2 GiB** memory allowance, the fallback when
physical memory could not be queried. No cooperative solver deadline or ordinary
cumulative work ceiling was set. The harness separately bounded each child to
120 seconds, each campaign to 1,800 seconds, and captured output to 16 MiB per
process. The allowance controls named storage capacities, not process RSS.

Use separate source checkouts and target directories for the two revisions.
In each checkout, use the pinned Rust 1.97.1 toolchain and clingo 5.8.2. The
following maintained commands reproduce a candidate profile; explicitly setting
2 GiB keeps the memory policy comparable on hosts that can report their memory.

```sh
cargo build --locked --release -p zetesis-cli -p zetesis-bench --all-features --bins
target/release/zetesis-bench run examples/correctness \
  --suite corpus --zetesis "$PWD/target/release/zetesis" \
  --clingo "$(command -v clingo)" --backend cpu --grounder auto --oracle auto \
  --threads 14 --clingo-threads 1 --memory 2GiB \
  --warmups 1 --repetitions 3 --memory-runs 1 \
  --timeout-seconds 120 --campaign-seconds 1800 --color never \
  --report corpus-auto.json
target/release/zetesis-bench run examples/correctness \
  --suite corpus --zetesis "$PWD/target/release/zetesis" \
  --clingo "$(command -v clingo)" --backend cpu --grounder lazy --oracle auto \
  --threads 14 --clingo-threads 1 --memory 2GiB \
  --warmups 1 --repetitions 3 --memory-runs 1 \
  --timeout-seconds 120 --campaign-seconds 1800 --color never \
  --report corpus-lazy.json
target/release/zetesis-bench compare auto=corpus-auto.json lazy=corpus-lazy.json \
  --markdown > corpus-comparison.md
```

Retain new source and binary hashes with each report. Match the recorded campaign
order when comparing versions; stop builds and competing measurements first.
A different machine produces a new observation. The public JSON is a path-free
projection of retained reports, not another measurement or runner input. Raw
report digests identify evidence; they do not imply a public raw-capture download.
These measurements neither characterize GPU performance nor qualify a physical
backend. The [7 October comparison](cpu-corpus-20261007.md) remains a separate
historical observation of released 0.4.0 preparation.
