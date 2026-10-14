# CPU corpus comparison, 14 October 2026

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

This study compares released **0.5.0** with a frozen **pre-release candidate** on the complete 94-program correctness corpus. The candidate has no assigned release version or tag; its executable and source-manifest hashes below identify the measured snapshot. It is a cumulative comparison, not an experiment isolating one optimization.

The candidate’s summed process-time estimate is 35.48% lower at one worker and 35.78% lower at fourteen. All 94 case estimates are more than 3% lower than the release at each worker count. The [portable observations](observations/cpu-corpus-20261014.json) retain individual timed values, common internal stages, memory observations, counts, costs and input hashes; the [TSV](observations/cpu-corpus-20261014.tsv) contains the derived per-case tables.

## Results and method

| Native workers | Release total | Candidate total | Direct clingo total | Release W/C/L | Candidate W/C/L |
|---:|---:|---:|---:|---:|---:|
| 1 | 808.670 ms | 521.714 ms | 805.594 ms | 4/0/90 | 58/26/10 |
| 14 | 777.447 ms | 499.265 ms | 811.996 ms | 4/0/90 | 48/32/14 |

W/C/L means wins/close/losses against clingo using a descriptive 3% band: a native/clingo ratio below 0.97 wins, 0.97 through 1.03 is close, and above 1.03 loses. Counts use unrounded values. This band does not establish statistical significance. Clingo uses one worker and `--opt-mode=optN` in every comparison.

At fourteen workers, 31 of the 32 close cases have a lower candidate estimate:
zetesis has the lower measured time on 79 of 94 programs. At one worker, one of
the 26 close cases favors zetesis, for 59 of 94 lower estimates. These directional
counts retain the small differences that the close category groups together.

The host was an Apple M4 Pro with 48 GiB RAM running macOS 26.6.2, on AC power with no competing intensive work reported. Each worker population ran release, candidate, candidate, release (ABBA). Every campaign had one qualification and one warmup, then three timed repetitions and one separate RSS observation per solver/case. The fourteen-worker sequence ran first, followed by the one-worker sequence. All eight campaigns passed all 1,128 planned positions each. No sample was omitted.

Native execution used CPU, automatic grounding and oracle selection, indexed formula joins and region search. Both solvers enumerated every ordinary answer or final optimum tie. The matrix checked complete selected displays, multiplicities and final costs against clingo; it also checked full native atom/shown-value identities across profiles and repetitions within each report. This study did not add a full-native comparison between release and candidate reports. Clingo’s hidden interpretations are unavailable.

For each case, the native estimate is the mean of the two campaign medians; clingo uses the mean of all four campaign medians. Totals sum these case estimates. They are neither one combined timed run nor medians pooled over all repetitions. Repeated campaign totals varied as follows:

| Native workers | Release range | Candidate range | Direct clingo range |
|---:|---:|---:|---:|
| 1 | 791.224–826.116 ms | 520.621–522.807 ms | 790.662–819.440 ms |
| 14 | 735.710–819.184 ms | 491.259–507.271 ms | 788.413–866.525 ms |

These ranges describe repeatability, not confidence intervals. Whole-process time includes startup, input, parsing, grounding, solving, captured JSON/statistics output and process handling. Release 0.5.0 used one GPU-capable executable; the candidate used its CPU front. The process improvement therefore includes packaging/startup changes as well as solver work. A process-minus-driver difference is not an isolated measurement of loader cost.

The direct clingo executable was measured, without a package-manager launcher. Earlier launcher-inclusive observations are a different protocol and are not pooled here. This study covers the correctness corpus; it does not characterize GPU performance, the scalability population, or original USA, wide Mastermind and Family Crossing workloads.

## Grounding, solving and memory

The internal fields below use the common reported phase schema 5 and are present in every timed native sample. These are sums of mean campaign medians, in milliseconds. Independently reduced stages need not add exactly to driver time. A changed execution route can move work between stages.

| Workers | Build | Driver | Source preparation | Grounding | Solving | Median case RSS | Largest case RSS |
|---:|---|---:|---:|---:|---:|---:|---:|
| 1 | Release | 339.928 | 65.845 | 100.779 | 152.497 | 15.262 MiB | 17.914 MiB |
| 1 | Candidate | 265.421 | 62.467 | 77.528 | 105.586 | 11.547 MiB | 14.391 MiB |
| 14 | Release | 275.328 | 70.150 | 105.046 | 75.764 | 15.352 MiB | 19.328 MiB |
| 14 | Candidate | 220.626 | 58.802 | 73.874 | 65.415 | 11.621 MiB | 16.625 MiB |

A case’s RSS value averages the two separate native peak observations. The table then reports the median and maximum across cases. RSS is not live heap, simultaneous process-tree memory or device memory; never sum these peaks as one allocation. Per-case native and clingo RSS remains in the TSV.

## Every corpus family

These groups cover every case. Totals use the same estimator as the complete corpus; W/C/L compares each candidate case with direct clingo using the same descriptive 3% band.

| Workers | Family | Cases | Release  ms | Candidate  ms | Direct clingo  ms | Candidate W/C/L |
|---:|---|---:|---:|---:|---:|---:|
| 1 | equality-generalized-tsp | 3 | 19.028 | 11.976 | 13.983 | 2/1/0 |
| 1 | shortest-path | 35 | 249.497 | 155.677 | 178.037 | 26/7/2 |
| 1 | task-allocation | 20 | 173.295 | 120.950 | 304.602 | 9/10/1 |
| 1 | traveling-salesman | 29 | 202.576 | 130.768 | 145.795 | 19/7/3 |
| 1 | n-queens | 6 | 150.122 | 90.817 | 151.051 | 1/1/4 |
| 1 | send-money | 1 | 14.152 | 11.525 | 12.127 | 1/0/0 |
| 14 | equality-generalized-tsp | 3 | 22.151 | 13.476 | 14.457 | 1/2/0 |
| 14 | shortest-path | 35 | 266.410 | 165.544 | 177.086 | 14/18/3 |
| 14 | task-allocation | 20 | 166.323 | 107.628 | 313.123 | 15/2/3 |
| 14 | traveling-salesman | 29 | 222.912 | 138.396 | 143.752 | 15/9/5 |
| 14 | n-queens | 6 | 86.183 | 64.887 | 151.205 | 2/1/3 |
| 14 | send-money | 1 | 13.468 | 9.334 | 12.373 | 1/0/0 |

## All 94 programs

Each table includes every corpus entry. Times are milliseconds. Change is candidate/release minus one; ratio is candidate/direct clingo. The source files are unchanged from the release.

### 1 native worker

| Program | Release | Candidate | Clingo | Change | Ratio |
|---|---:|---:|---:|---:|---:|
| [equality-generalized-tsp/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/equality-generalized-tsp/01-basic.lp) | 6.627 | 3.999 | 4.879 | -39.65% | 0.820 |
| [equality-generalized-tsp/02-larger](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/equality-generalized-tsp/02-larger.lp) | 6.534 | 3.978 | 5.185 | -39.12% | 0.767 |
| [equality-generalized-tsp/03-unreachable-subset-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp) | 5.867 | 3.999 | 3.919 | -31.83% | 1.020 |
| [shortest-path/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/01-basic.lp) | 5.778 | 3.963 | 4.219 | -31.41% | 0.939 |
| [shortest-path/v01/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/02-start-equals-end.lp) | 5.877 | 3.957 | 3.935 | -32.67% | 1.006 |
| [shortest-path/v01/03-zero-cost-detour](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/03-zero-cost-detour.lp) | 5.842 | 3.947 | 4.227 | -32.44% | 0.934 |
| [shortest-path/v01/04-no-path](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/04-no-path.lp) | 5.256 | 3.962 | 3.919 | -24.62% | 1.011 |
| [shortest-path/v01/05-multi-path](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/05-multi-path.lp) | 6.499 | 3.966 | 5.178 | -38.98% | 0.766 |
| [shortest-path/v01/06-layered-dag](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/06-layered-dag.lp) | 8.489 | 5.257 | 5.213 | -38.07% | 1.009 |
| [shortest-path/v01/07-cycles](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/07-cycles.lp) | 6.522 | 4.520 | 5.198 | -30.69% | 0.870 |
| [shortest-path/v01/08-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/08-negative-weights.lp) | 7.121 | 3.986 | 4.232 | -44.02% | 0.942 |
| [shortest-path/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/01-basic.lp) | 5.905 | 3.967 | 4.557 | -32.82% | 0.871 |
| [shortest-path/v02/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/02-start-equals-end.lp) | 6.535 | 3.951 | 4.225 | -39.53% | 0.935 |
| [shortest-path/v02/03-before-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/03-before-forces-detour.lp) | 6.477 | 3.968 | 4.855 | -38.74% | 0.817 |
| [shortest-path/v02/04-after-forces-extension](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/04-after-forces-extension.lp) | 6.551 | 3.949 | 4.867 | -39.72% | 0.811 |
| [shortest-path/v02/05-ordering-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/05-ordering-unsat.lp) | 5.841 | 3.957 | 4.869 | -32.25% | 0.813 |
| [shortest-path/v02/06-layered-dag-before](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/06-layered-dag-before.lp) | 9.042 | 5.940 | 5.508 | -34.30% | 1.078 |
| [shortest-path/v02/07-layered-dag-before-after](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp) | 9.134 | 5.965 | 5.782 | -34.70% | 1.032 |
| [shortest-path/v02/08-tie-break-under-ordering](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp) | 6.567 | 4.011 | 5.167 | -38.93% | 0.776 |
| [shortest-path/v02/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/09-negative-weights.lp) | 6.496 | 3.965 | 5.170 | -38.96% | 0.767 |
| [shortest-path/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/01-basic.lp) | 6.556 | 4.009 | 5.189 | -38.85% | 0.772 |
| [shortest-path/v03/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/02-start-equals-end.lp) | 5.859 | 3.944 | 4.865 | -32.69% | 0.811 |
| [shortest-path/v03/03-budget-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/03-budget-forces-detour.lp) | 6.518 | 3.952 | 4.871 | -39.36% | 0.811 |
| [shortest-path/v03/04-cost-at-cap-allowed](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp) | 5.853 | 3.961 | 4.250 | -32.33% | 0.932 |
| [shortest-path/v03/05-budget-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/05-budget-unsat.lp) | 6.556 | 3.976 | 5.187 | -39.35% | 0.767 |
| [shortest-path/v03/06-layered-dag-cap](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/06-layered-dag-cap.lp) | 10.294 | 6.494 | 6.434 | -36.91% | 1.009 |
| [shortest-path/v03/07-layered-dag-tight-cap](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp) | 11.643 | 6.595 | 6.469 | -43.36% | 1.019 |
| [shortest-path/v03/08-tie-break-under-cap](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp) | 6.586 | 3.986 | 5.205 | -39.49% | 0.766 |
| [shortest-path/v03/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/09-negative-weights.lp) | 7.245 | 4.023 | 5.203 | -44.47% | 0.773 |
| [shortest-path/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/01-basic.lp) | 6.585 | 3.960 | 5.200 | -39.87% | 0.761 |
| [shortest-path/v04/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/02-start-equals-end.lp) | 6.562 | 4.031 | 5.197 | -38.56% | 0.776 |
| [shortest-path/v04/03-before-forces-detour-within-budget](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp) | 6.525 | 3.956 | 5.196 | -39.37% | 0.761 |
| [shortest-path/v04/04-ordering-violates-budget](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp) | 6.500 | 4.005 | 5.198 | -38.39% | 0.770 |
| [shortest-path/v04/05-after-and-budget-interact](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp) | 6.539 | 3.937 | 5.172 | -39.80% | 0.761 |
| [shortest-path/v04/06-layered-dag-ordering-cap](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp) | 10.978 | 6.521 | 6.445 | -40.60% | 1.012 |
| [shortest-path/v04/07-layered-dag-combined](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/07-layered-dag-combined.lp) | 11.036 | 6.483 | 6.448 | -41.25% | 1.005 |
| [shortest-path/v04/08-tie-break-under-ordering-and-cap](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp) | 7.200 | 4.668 | 5.205 | -35.17% | 0.897 |
| [shortest-path/v04/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/09-negative-weights.lp) | 6.531 | 3.946 | 5.179 | -39.59% | 0.762 |
| [task-allocation/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-01/01-basic.lp) | 5.907 | 3.999 | 3.939 | -32.31% | 1.015 |
| [task-allocation/v01/02-agent-reuse](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-01/02-agent-reuse.lp) | 6.526 | 3.968 | 4.217 | -39.20% | 0.941 |
| [task-allocation/v01/03-selective-compatibility](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-01/03-selective-compatibility.lp) | 6.553 | 4.013 | 3.942 | -38.77% | 1.018 |
| [task-allocation/v01/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp) | 5.246 | 3.983 | 3.919 | -24.07% | 1.016 |
| [task-allocation/v01/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-01/05-larger-mix.lp) | 6.530 | 3.978 | 3.921 | -39.09% | 1.015 |
| [task-allocation/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-02/01-basic.lp) | 6.525 | 3.951 | 4.557 | -39.45% | 0.867 |
| [task-allocation/v02/02-makespan-tiebreak](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp) | 6.581 | 3.947 | 4.453 | -40.03% | 0.886 |
| [task-allocation/v02/03-cost-dominates](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-02/03-cost-dominates.lp) | 6.550 | 3.957 | 4.833 | -39.59% | 0.819 |
| [task-allocation/v02/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp) | 6.543 | 3.988 | 3.914 | -39.05% | 1.019 |
| [task-allocation/v02/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-02/05-larger-mix.lp) | 8.388 | 5.212 | 5.215 | -37.87% | 0.999 |
| [task-allocation/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-03/01-basic.lp) | 5.932 | 4.008 | 3.921 | -32.45% | 1.022 |
| [task-allocation/v03/02-multiple-groups](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-03/02-multiple-groups.lp) | 5.886 | 3.955 | 4.217 | -32.80% | 0.938 |
| [task-allocation/v03/03-mixed-grouped-ungrouped](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp) | 5.867 | 4.041 | 3.893 | -31.11% | 1.038 |
| [task-allocation/v03/04-incompatible-group-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp) | 5.233 | 3.947 | 3.914 | -24.59% | 1.008 |
| [task-allocation/v03/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-03/05-larger-mix.lp) | 6.529 | 3.977 | 3.917 | -39.09% | 1.015 |
| [task-allocation/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-04/01-basic.lp) | 8.985 | 6.494 | 22.790 | -27.72% | 0.285 |
| [task-allocation/v04/02-precedence](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-04/02-precedence.lp) | 7.888 | 5.298 | 6.469 | -32.84% | 0.819 |
| [task-allocation/v04/03-agent-serialization](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-04/03-agent-serialization.lp) | 8.509 | 6.513 | 24.662 | -23.45% | 0.264 |
| [task-allocation/v04/04-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp) | 6.623 | 5.344 | 5.274 | -19.31% | 1.013 |
| [task-allocation/v04/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-04/05-larger-mix.lp) | 46.494 | 36.378 | 182.635 | -21.76% | 0.199 |
| [traveling-salesman/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-01/01-basic.lp) | 6.661 | 4.118 | 5.273 | -38.17% | 0.781 |
| [traveling-salesman/v01/02-multiple-tours](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-01/02-multiple-tours.lp) | 7.131 | 3.966 | 5.186 | -44.38% | 0.765 |
| [traveling-salesman/v01/03-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-01/03-asymmetric.lp) | 7.838 | 5.269 | 5.212 | -32.77% | 1.011 |
| [traveling-salesman/v01/04-subtour-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp) | 5.890 | 3.980 | 4.248 | -32.42% | 0.937 |
| [traveling-salesman/v01/05-ring](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-01/05-ring.lp) | 7.820 | 5.264 | 5.178 | -32.69% | 1.017 |
| [traveling-salesman/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-02/01-basic.lp) | 6.486 | 3.961 | 5.162 | -38.92% | 0.767 |
| [traveling-salesman/v02/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-02/02-single-salesman.lp) | 6.501 | 3.982 | 3.918 | -38.75% | 1.016 |
| [traveling-salesman/v02/03-too-many-salesmen-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp) | 5.748 | 3.965 | 4.520 | -31.02% | 0.877 |
| [traveling-salesman/v02/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp) | 6.537 | 4.005 | 4.876 | -38.73% | 0.821 |
| [traveling-salesman/v02/05-larger-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp) | 6.527 | 4.001 | 5.184 | -38.69% | 0.772 |
| [traveling-salesman/v02/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp) | 6.542 | 3.984 | 5.136 | -39.10% | 0.776 |
| [traveling-salesman/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-03/01-basic.lp) | 6.556 | 4.000 | 4.860 | -38.99% | 0.823 |
| [traveling-salesman/v03/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-03/02-single-salesman.lp) | 6.521 | 3.960 | 4.563 | -39.27% | 0.868 |
| [traveling-salesman/v03/03-depot-crossing-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp) | 5.859 | 3.968 | 4.561 | -32.29% | 0.870 |
| [traveling-salesman/v03/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp) | 6.543 | 3.970 | 5.167 | -39.33% | 0.768 |
| [traveling-salesman/v03/05-larger-three-depots](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp) | 6.485 | 4.631 | 5.191 | -28.59% | 0.892 |
| [traveling-salesman/v03/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp) | 6.553 | 3.995 | 5.203 | -39.03% | 0.768 |
| [traveling-salesman/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-04/01-basic.lp) | 7.088 | 5.239 | 5.183 | -26.09% | 1.011 |
| [traveling-salesman/v04/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-04/02-single-salesman.lp) | 7.081 | 5.293 | 5.201 | -25.26% | 1.018 |
| [traveling-salesman/v04/03-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp) | 6.513 | 4.601 | 5.210 | -29.35% | 0.883 |
| [traveling-salesman/v04/04-depot-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp) | 6.584 | 4.018 | 5.223 | -38.97% | 0.769 |
| [traveling-salesman/v04/05-three-depots-asymmetric-times](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp) | 9.059 | 5.830 | 5.182 | -35.65% | 1.125 |
| [traveling-salesman/v04/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp) | 9.122 | 5.240 | 5.189 | -42.56% | 1.010 |
| [traveling-salesman/v05/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-05/01-basic.lp) | 7.166 | 5.209 | 5.184 | -27.31% | 1.005 |
| [traveling-salesman/v05/02-tight-bound-exact](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp) | 7.128 | 4.648 | 5.200 | -34.79% | 0.894 |
| [traveling-salesman/v05/03-revisit-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp) | 6.565 | 3.947 | 5.181 | -39.89% | 0.762 |
| [traveling-salesman/v05/04-depot-and-vertex-revisits](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp) | 6.546 | 3.999 | 5.208 | -38.91% | 0.768 |
| [traveling-salesman/v05/05-three-depots-mixed](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp) | 9.057 | 5.868 | 5.200 | -35.22% | 1.128 |
| [traveling-salesman/v05/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp) | 8.469 | 5.856 | 5.198 | -30.85% | 1.127 |
| [n-queens/v01](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/standalone/n-queens/variant-01.lp) | 11.547 | 6.499 | 5.518 | -43.72% | 1.178 |
| [n-queens/v02](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/standalone/n-queens/variant-02.lp) | 93.936 | 54.947 | 120.026 | -41.51% | 0.458 |
| [n-queens/v03](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/standalone/n-queens/variant-03.lp) | 14.221 | 7.228 | 6.506 | -49.18% | 1.111 |
| [n-queens/v04](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/standalone/n-queens/variant-04.lp) | 9.097 | 6.572 | 6.403 | -27.75% | 1.027 |
| [n-queens/v05](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/standalone/n-queens/variant-05.lp) | 10.314 | 7.799 | 6.476 | -24.38% | 1.204 |
| [n-queens/v06](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/standalone/n-queens/variant-06.lp) | 11.007 | 7.772 | 6.122 | -29.39% | 1.269 |
| [send-money/send-money](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/standalone/send-money/send-money.lp) | 14.152 | 11.525 | 12.127 | -18.56% | 0.950 |

### 14 native workers

| Program | Release | Candidate | Clingo | Change | Ratio |
|---|---:|---:|---:|---:|---:|
| [equality-generalized-tsp/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/equality-generalized-tsp/01-basic.lp) | 7.873 | 4.769 | 4.840 | -39.42% | 0.986 |
| [equality-generalized-tsp/02-larger](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/equality-generalized-tsp/02-larger.lp) | 7.814 | 4.714 | 4.797 | -39.67% | 0.983 |
| [equality-generalized-tsp/03-unreachable-subset-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp) | 6.465 | 3.993 | 4.821 | -38.23% | 0.828 |
| [shortest-path/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/01-basic.lp) | 6.414 | 3.920 | 4.802 | -38.88% | 0.816 |
| [shortest-path/v01/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/02-start-equals-end.lp) | 6.404 | 3.966 | 4.490 | -38.07% | 0.883 |
| [shortest-path/v01/03-zero-cost-detour](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/03-zero-cost-detour.lp) | 7.161 | 3.161 | 4.801 | -55.86% | 0.658 |
| [shortest-path/v01/04-no-path](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/04-no-path.lp) | 7.197 | 3.955 | 4.814 | -45.05% | 0.822 |
| [shortest-path/v01/05-multi-path](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/05-multi-path.lp) | 7.178 | 4.678 | 4.798 | -34.83% | 0.975 |
| [shortest-path/v01/06-layered-dag](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/06-layered-dag.lp) | 9.238 | 4.677 | 5.182 | -49.37% | 0.903 |
| [shortest-path/v01/07-cycles](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/07-cycles.lp) | 7.193 | 4.691 | 4.799 | -34.79% | 0.978 |
| [shortest-path/v01/08-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-01/08-negative-weights.lp) | 7.179 | 4.712 | 4.506 | -34.36% | 1.046 |
| [shortest-path/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/01-basic.lp) | 6.394 | 4.702 | 4.799 | -26.46% | 0.980 |
| [shortest-path/v02/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/02-start-equals-end.lp) | 6.458 | 3.954 | 4.807 | -38.78% | 0.823 |
| [shortest-path/v02/03-before-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/03-before-forces-detour.lp) | 7.209 | 4.720 | 4.806 | -34.53% | 0.982 |
| [shortest-path/v02/04-after-forces-extension](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/04-after-forces-extension.lp) | 7.180 | 4.721 | 4.812 | -34.25% | 0.981 |
| [shortest-path/v02/05-ordering-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/05-ordering-unsat.lp) | 6.910 | 3.929 | 4.792 | -43.15% | 0.820 |
| [shortest-path/v02/06-layered-dag-before](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/06-layered-dag-before.lp) | 9.228 | 6.256 | 5.974 | -32.20% | 1.047 |
| [shortest-path/v02/07-layered-dag-before-after](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp) | 9.221 | 5.445 | 5.945 | -40.95% | 0.916 |
| [shortest-path/v02/08-tie-break-under-ordering](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp) | 7.185 | 4.738 | 4.806 | -34.07% | 0.986 |
| [shortest-path/v02/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-02/09-negative-weights.lp) | 7.148 | 4.708 | 4.803 | -34.13% | 0.980 |
| [shortest-path/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/01-basic.lp) | 7.154 | 4.705 | 4.775 | -34.23% | 0.985 |
| [shortest-path/v03/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/02-start-equals-end.lp) | 6.405 | 3.166 | 4.801 | -50.56% | 0.659 |
| [shortest-path/v03/03-budget-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/03-budget-forces-detour.lp) | 7.225 | 4.728 | 4.812 | -34.56% | 0.983 |
| [shortest-path/v03/04-cost-at-cap-allowed](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp) | 6.953 | 3.943 | 4.794 | -43.29% | 0.822 |
| [shortest-path/v03/05-budget-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/05-budget-unsat.lp) | 7.172 | 4.723 | 5.184 | -34.15% | 0.911 |
| [shortest-path/v03/06-layered-dag-cap](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/06-layered-dag-cap.lp) | 10.575 | 6.232 | 6.627 | -41.07% | 0.940 |
| [shortest-path/v03/07-layered-dag-tight-cap](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp) | 10.613 | 6.264 | 6.631 | -40.97% | 0.945 |
| [shortest-path/v03/08-tie-break-under-cap](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp) | 7.168 | 4.714 | 4.820 | -34.24% | 0.978 |
| [shortest-path/v03/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-03/09-negative-weights.lp) | 7.183 | 4.715 | 4.815 | -34.36% | 0.979 |
| [shortest-path/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/01-basic.lp) | 6.395 | 4.697 | 4.801 | -26.56% | 0.978 |
| [shortest-path/v04/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/02-start-equals-end.lp) | 7.175 | 4.712 | 4.490 | -34.33% | 1.049 |
| [shortest-path/v04/03-before-forces-detour-within-budget](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp) | 7.128 | 4.716 | 4.817 | -33.84% | 0.979 |
| [shortest-path/v04/04-ordering-violates-budget](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp) | 7.183 | 4.733 | 4.814 | -34.11% | 0.983 |
| [shortest-path/v04/05-after-and-budget-interact](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp) | 7.124 | 4.702 | 4.797 | -33.99% | 0.980 |
| [shortest-path/v04/06-layered-dag-ordering-cap](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp) | 10.552 | 6.227 | 6.229 | -40.98% | 1.000 |
| [shortest-path/v04/07-layered-dag-combined](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/07-layered-dag-combined.lp) | 10.564 | 6.205 | 6.521 | -41.26% | 0.952 |
| [shortest-path/v04/08-tie-break-under-ordering-and-cap](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp) | 7.785 | 4.728 | 4.818 | -39.27% | 0.981 |
| [shortest-path/v04/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/shortest-path/variant-04/09-negative-weights.lp) | 7.159 | 4.701 | 4.804 | -34.34% | 0.979 |
| [task-allocation/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-01/01-basic.lp) | 6.419 | 3.217 | 4.489 | -49.89% | 0.716 |
| [task-allocation/v01/02-agent-reuse](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-01/02-agent-reuse.lp) | 6.367 | 3.168 | 4.487 | -50.24% | 0.706 |
| [task-allocation/v01/03-selective-compatibility](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-01/03-selective-compatibility.lp) | 6.411 | 3.184 | 4.489 | -50.34% | 0.709 |
| [task-allocation/v01/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp) | 6.338 | 3.166 | 4.485 | -50.05% | 0.706 |
| [task-allocation/v01/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-01/05-larger-mix.lp) | 7.184 | 3.199 | 4.492 | -55.47% | 0.712 |
| [task-allocation/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-02/01-basic.lp) | 6.392 | 4.687 | 4.789 | -26.68% | 0.979 |
| [task-allocation/v02/02-makespan-tiebreak](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp) | 7.172 | 4.709 | 4.489 | -34.34% | 1.049 |
| [task-allocation/v02/03-cost-dominates](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-02/03-cost-dominates.lp) | 7.158 | 4.702 | 4.785 | -34.32% | 0.983 |
| [task-allocation/v02/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp) | 7.079 | 3.947 | 4.485 | -44.24% | 0.880 |
| [task-allocation/v02/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-02/05-larger-mix.lp) | 9.177 | 5.438 | 5.158 | -40.74% | 1.054 |
| [task-allocation/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-03/01-basic.lp) | 7.155 | 3.209 | 4.483 | -55.16% | 0.716 |
| [task-allocation/v03/02-multiple-groups](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-03/02-multiple-groups.lp) | 7.149 | 3.926 | 4.487 | -45.08% | 0.875 |
| [task-allocation/v03/03-mixed-grouped-ungrouped](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp) | 6.456 | 3.204 | 4.477 | -50.37% | 0.716 |
| [task-allocation/v03/04-incompatible-group-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp) | 5.746 | 3.180 | 4.481 | -44.65% | 0.710 |
| [task-allocation/v03/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-03/05-larger-mix.lp) | 7.174 | 4.710 | 4.477 | -34.35% | 1.052 |
| [task-allocation/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-04/01-basic.lp) | 9.249 | 6.265 | 23.157 | -32.27% | 0.271 |
| [task-allocation/v04/02-precedence](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-04/02-precedence.lp) | 7.868 | 4.729 | 6.264 | -39.90% | 0.755 |
| [task-allocation/v04/03-agent-serialization](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-04/03-agent-serialization.lp) | 8.548 | 6.235 | 24.945 | -27.06% | 0.250 |
| [task-allocation/v04/04-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp) | 7.254 | 4.761 | 5.213 | -34.36% | 0.913 |
| [task-allocation/v04/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/task-allocation/variant-04/05-larger-mix.lp) | 30.027 | 27.993 | 184.992 | -6.77% | 0.151 |
| [traveling-salesman/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-01/01-basic.lp) | 7.888 | 4.877 | 5.264 | -38.17% | 0.927 |
| [traveling-salesman/v01/02-multiple-tours](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-01/02-multiple-tours.lp) | 7.164 | 4.724 | 4.801 | -34.06% | 0.984 |
| [traveling-salesman/v01/03-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-01/03-asymmetric.lp) | 7.826 | 4.730 | 5.191 | -39.56% | 0.911 |
| [traveling-salesman/v01/04-subtour-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp) | 6.421 | 3.193 | 4.484 | -50.28% | 0.712 |
| [traveling-salesman/v01/05-ring](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-01/05-ring.lp) | 9.213 | 6.230 | 5.176 | -32.38% | 1.204 |
| [traveling-salesman/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-02/01-basic.lp) | 7.143 | 4.749 | 4.810 | -33.52% | 0.987 |
| [traveling-salesman/v02/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-02/02-single-salesman.lp) | 7.202 | 4.688 | 4.824 | -34.90% | 0.972 |
| [traveling-salesman/v02/03-too-many-salesmen-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp) | 6.409 | 3.923 | 4.801 | -38.79% | 0.817 |
| [traveling-salesman/v02/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp) | 7.159 | 4.718 | 4.498 | -34.10% | 1.049 |
| [traveling-salesman/v02/05-larger-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp) | 7.157 | 4.713 | 4.797 | -34.15% | 0.982 |
| [traveling-salesman/v02/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp) | 7.201 | 4.734 | 4.808 | -34.26% | 0.984 |
| [traveling-salesman/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-03/01-basic.lp) | 7.133 | 4.696 | 4.785 | -34.16% | 0.981 |
| [traveling-salesman/v03/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-03/02-single-salesman.lp) | 7.151 | 3.203 | 4.495 | -55.21% | 0.712 |
| [traveling-salesman/v03/03-depot-crossing-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp) | 7.095 | 3.917 | 4.482 | -44.80% | 0.874 |
| [traveling-salesman/v03/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp) | 7.165 | 4.746 | 4.807 | -33.77% | 0.987 |
| [traveling-salesman/v03/05-larger-three-depots](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp) | 8.307 | 4.704 | 4.798 | -43.37% | 0.981 |
| [traveling-salesman/v03/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp) | 7.162 | 4.744 | 4.803 | -33.77% | 0.988 |
| [traveling-salesman/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-04/01-basic.lp) | 7.727 | 4.704 | 5.169 | -39.12% | 0.910 |
| [traveling-salesman/v04/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-04/02-single-salesman.lp) | 7.844 | 4.733 | 5.173 | -39.66% | 0.915 |
| [traveling-salesman/v04/03-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp) | 7.146 | 4.714 | 5.173 | -34.04% | 0.911 |
| [traveling-salesman/v04/04-depot-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp) | 7.230 | 4.725 | 5.196 | -34.65% | 0.909 |
| [traveling-salesman/v04/05-three-depots-asymmetric-times](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp) | 9.762 | 6.210 | 5.174 | -36.39% | 1.200 |
| [traveling-salesman/v04/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp) | 9.200 | 4.709 | 5.179 | -48.82% | 0.909 |
| [traveling-salesman/v05/01-basic](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-05/01-basic.lp) | 8.535 | 4.729 | 5.184 | -44.58% | 0.912 |
| [traveling-salesman/v05/02-tight-bound-exact](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp) | 7.182 | 4.744 | 5.200 | -33.96% | 0.912 |
| [traveling-salesman/v05/03-revisit-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp) | 7.148 | 4.689 | 5.129 | -34.40% | 0.914 |
| [traveling-salesman/v05/04-depot-and-vertex-revisits](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp) | 7.191 | 4.718 | 5.188 | -34.40% | 0.909 |
| [traveling-salesman/v05/05-three-depots-mixed](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp) | 9.943 | 6.221 | 5.180 | -37.44% | 1.201 |
| [traveling-salesman/v05/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp) | 9.208 | 6.214 | 5.182 | -32.51% | 1.199 |
| [n-queens/v01](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/standalone/n-queens/variant-01.lp) | 11.405 | 4.681 | 5.936 | -58.95% | 0.789 |
| [n-queens/v02](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/standalone/n-queens/variant-02.lp) | 33.470 | 32.069 | 120.441 | -4.19% | 0.266 |
| [n-queens/v03](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/standalone/n-queens/variant-03.lp) | 13.432 | 6.311 | 6.298 | -53.01% | 1.002 |
| [n-queens/v04](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/standalone/n-queens/variant-04.lp) | 8.591 | 6.264 | 5.963 | -27.09% | 1.050 |
| [n-queens/v05](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/standalone/n-queens/variant-05.lp) | 9.275 | 7.808 | 6.291 | -15.82% | 1.241 |
| [n-queens/v06](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/standalone/n-queens/variant-06.lp) | 10.010 | 7.754 | 6.277 | -22.54% | 1.235 |
| [send-money/send-money](https://github.com/GregoryGelfond/zetesis/blob/bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835/examples/correctness/standalone/send-money/send-money.lp) | 13.468 | 9.334 | 12.373 | -30.70% | 0.754 |

## Recorded identities

Release source: tag `v0.5.0`, commit `bfb8beea70f7c1765c2d3ac8d77b4e16d3bb0835`. Candidate source is an unreleased snapshot identified by source-manifest SHA-256 `718870c9720452e17b535cadf5df8e9a0f850d5916d17f1e508cd30bc827a71c`. No release identifier is assigned by this measurement.

| Executable | SHA-256 |
|---|---|
| Release | `fce4b1825bf0cfe5bcdc381fc450faf5f9c03479b46d6f621ccf853e97e5ece7` |
| Candidate CPU front | `2e826825e81e654b3211bc526e1bcf6d7a09b9795a7bed4ed16b13853da0ef90` |
| Maintained benchmark runner | `e47d59cde913931d4f602870c2052bfc7a8f4bcf77c655569946a92fcb2952dd` |
| Direct clingo 5.8.2 | `31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015` |

The source corpus manifest SHA-256 is `b43df1adf17ae0c035f1e310a5c15345c26cbcad8b59596932627c46fd1c6958`. Portable observations retain each source digest and raw report digest. Digests identify the retained evidence; they do not imply public raw-capture downloads.

## Reproduce the campaign

Use the identified frozen binaries, or record new identities for new builds. The candidate snapshot is pre-release; matching a version string alone does not reproduce it. From a checkout containing the unchanged corpus and the maintained benchmark tool, set `BASELINE`, `CANDIDATE`, `BENCH` and `CLINGO` to those executable paths. `CLINGO` must name the direct solver, not a package-manager launcher. These commands write new checkout-relative reports and preserve ordinary solver defaults:

```sh
: "${BASELINE:?Set the release executable path}"
: "${CANDIDATE:?Set the candidate executable path}"
: "${BENCH:?Set the maintained zetesis-bench path}"
: "${CLINGO:?Set the direct clingo 5.8.2 executable path}"
mkdir -p measurements/cpu-corpus-20261014
for workers in 14 1; do
  for block in A B B2 A2; do
    case "$block" in
      A|A2) solver=$BASELINE ;;
      B|B2) solver=$CANDIDATE ;;
    esac
    "$BENCH" run examples/correctness --suite corpus \
      --zetesis "$solver" --native-interface solve --clingo "$CLINGO" \
      --backend cpu --grounder auto --oracle auto --threads "$workers" \
      --formula-joins indexed --search regions --clingo-threads 1 \
      --warmups 1 --repetitions 3 --memory-runs 1 \
      --timeout-seconds 30 --campaign-seconds 1800 \
      --report "measurements/cpu-corpus-20261014/w${workers}-${block}.json"
  done
done
```

Stop builds and competing measurements first. Report files are never silently replaced; use a new directory for another campaign and retain refusals and timeouts. The recorded child/campaign deadlines are measurement controls; no solver memory or work budget override was used. A different machine or binary produces a new observation. CPU measurements do not qualify a physical GPU backend.
