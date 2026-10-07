# CPU corpus comparison, 7 October 2026

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

This complete 94-program CPU comparison measures zetesis 0.4.0 against clingo 5.8.2. The [portable observations](observations/cpu-corpus-20261007.json) identify the exact measured executable and source snapshot.

On the Apple M4 Pro measurement host, zetesis used 14 threads, CPU execution and automatic grounding/oracle selection; clingo used one thread and stock search heuristics. Both requested every answer or every tied optimum. One qualification and one warmup preceded three timed repetitions and one separate RSS run for each solver and case. All **94 cases and 1128 positions passed**, with no changed inputs, faults or unsettled children.

The native profile uses ordinary resource policy, the executable's default memory allowance and no cooperative deadline. The harness retains a 10-second child timeout, a 600-second campaign deadline and capture/normalization bounds. Memory allowance is not a process RSS ceiling. The current [benchmark protocol](benchmarking.md) distinguishes solver resources from measurement bounds.

## Results and limits

Summed per-case wall medians are **729.179 ms for zetesis** and **804.916 ms for clingo**. Zetesis is faster on **4 of 94 cases**; clingo is faster on 90. A few substantial wins reduce the total; most individual programs favor clingo. These totals are sums of separate medians, not one timed combined run, and three repetitions do not establish a general speedup or statistical significance.

Whole-process time includes startup, input loading, parsing, grounding, solving and captured JSON/statistics output. Driver stages exclude source loading, statistics output and the JSON envelope. RSS uses separate fresh-child observations. These CPU results do not qualify GPU performance, other thread counts, or time to the first answer.

Qualification compares complete displayed families and final optimum ties/costs with clingo, including multiplicity. Full native atoms and shown values must also agree across native profiles/repeats; clingo's hidden full interpretations are unavailable. Removal of an execution ceiling is not an algorithmic speedup. Earlier [foundation-reuse measurements](foundation-reuse.md) remain historical observations of their own sources and schedules.

## All 94 programs

Times are milliseconds; ratio is zetesis/clingo, so smaller favors zetesis. Ratios and win counts use unrounded values. Program links navigate the current repository; the observations retain the measured corpus file hashes, per-case ranges and stage medians.

| Program | zetesis | clingo | Ratio |
| --- | ---: | ---: | ---: |
| [equality-generalized-tsp/01-basic](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/equality-generalized-tsp/01-basic.lp) | 6.63 | 5.21 | 1.273 |
| [equality-generalized-tsp/02-larger](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/equality-generalized-tsp/02-larger.lp) | 6.57 | 5.22 | 1.260 |
| [equality-generalized-tsp/03-unreachable-subset-unsat](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp) | 6.54 | 3.95 | 1.657 |
| [shortest-path/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-01/01-basic.lp) | 6.53 | 5.16 | 1.266 |
| [shortest-path/v01/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-01/02-start-equals-end.lp) | 5.26 | 3.93 | 1.338 |
| [shortest-path/v01/03-zero-cost-detour](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-01/03-zero-cost-detour.lp) | 6.52 | 5.20 | 1.255 |
| [shortest-path/v01/04-no-path](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-01/04-no-path.lp) | 6.38 | 3.95 | 1.618 |
| [shortest-path/v01/05-multi-path](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-01/05-multi-path.lp) | 6.51 | 5.18 | 1.255 |
| [shortest-path/v01/06-layered-dag](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-01/06-layered-dag.lp) | 9.11 | 5.21 | 1.748 |
| [shortest-path/v01/07-cycles](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-01/07-cycles.lp) | 6.63 | 5.21 | 1.273 |
| [shortest-path/v01/08-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-01/08-negative-weights.lp) | 6.52 | 3.97 | 1.641 |
| [shortest-path/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-02/01-basic.lp) | 6.59 | 5.22 | 1.260 |
| [shortest-path/v02/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-02/02-start-equals-end.lp) | 6.52 | 3.94 | 1.656 |
| [shortest-path/v02/03-before-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-02/03-before-forces-detour.lp) | 6.49 | 5.16 | 1.258 |
| [shortest-path/v02/04-after-forces-extension](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-02/04-after-forces-extension.lp) | 6.61 | 5.20 | 1.273 |
| [shortest-path/v02/05-ordering-unsat](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-02/05-ordering-unsat.lp) | 6.54 | 5.21 | 1.256 |
| [shortest-path/v02/06-layered-dag-before](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-02/06-layered-dag-before.lp) | 9.08 | 5.22 | 1.738 |
| [shortest-path/v02/07-layered-dag-before-after](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp) | 9.07 | 5.20 | 1.746 |
| [shortest-path/v02/08-tie-break-under-ordering](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp) | 6.55 | 5.19 | 1.263 |
| [shortest-path/v02/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-02/09-negative-weights.lp) | 6.51 | 5.20 | 1.252 |
| [shortest-path/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-03/01-basic.lp) | 6.54 | 5.18 | 1.263 |
| [shortest-path/v03/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-03/02-start-equals-end.lp) | 6.50 | 5.18 | 1.255 |
| [shortest-path/v03/03-budget-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-03/03-budget-forces-detour.lp) | 6.54 | 5.20 | 1.257 |
| [shortest-path/v03/04-cost-at-cap-allowed](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp) | 6.49 | 3.93 | 1.652 |
| [shortest-path/v03/05-budget-unsat](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-03/05-budget-unsat.lp) | 6.56 | 5.16 | 1.271 |
| [shortest-path/v03/06-layered-dag-cap](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-03/06-layered-dag-cap.lp) | 10.38 | 6.51 | 1.595 |
| [shortest-path/v03/07-layered-dag-tight-cap](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp) | 10.30 | 6.45 | 1.598 |
| [shortest-path/v03/08-tie-break-under-cap](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp) | 6.56 | 5.22 | 1.258 |
| [shortest-path/v03/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-03/09-negative-weights.lp) | 6.52 | 5.19 | 1.257 |
| [shortest-path/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-04/01-basic.lp) | 6.49 | 5.18 | 1.252 |
| [shortest-path/v04/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-04/02-start-equals-end.lp) | 6.59 | 5.12 | 1.286 |
| [shortest-path/v04/03-before-forces-detour-within-budget](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp) | 6.54 | 5.19 | 1.261 |
| [shortest-path/v04/04-ordering-violates-budget](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp) | 6.51 | 5.00 | 1.303 |
| [shortest-path/v04/05-after-and-budget-interact](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp) | 6.53 | 5.21 | 1.253 |
| [shortest-path/v04/06-layered-dag-ordering-cap](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp) | 10.33 | 6.45 | 1.600 |
| [shortest-path/v04/07-layered-dag-combined](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-04/07-layered-dag-combined.lp) | 10.32 | 6.48 | 1.591 |
| [shortest-path/v04/08-tie-break-under-ordering-and-cap](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp) | 6.56 | 5.21 | 1.258 |
| [shortest-path/v04/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/shortest-path/variant-04/09-negative-weights.lp) | 6.56 | 5.21 | 1.259 |
| [task-allocation/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-01/01-basic.lp) | 6.53 | 3.92 | 1.668 |
| [task-allocation/v01/02-agent-reuse](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-01/02-agent-reuse.lp) | 5.23 | 3.90 | 1.341 |
| [task-allocation/v01/03-selective-compatibility](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-01/03-selective-compatibility.lp) | 6.53 | 3.90 | 1.675 |
| [task-allocation/v01/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp) | 5.26 | 3.92 | 1.343 |
| [task-allocation/v01/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-01/05-larger-mix.lp) | 6.52 | 3.90 | 1.673 |
| [task-allocation/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-02/01-basic.lp) | 6.50 | 3.91 | 1.661 |
| [task-allocation/v02/02-makespan-tiebreak](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp) | 6.60 | 3.91 | 1.688 |
| [task-allocation/v02/03-cost-dominates](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-02/03-cost-dominates.lp) | 6.50 | 3.92 | 1.657 |
| [task-allocation/v02/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp) | 6.48 | 3.92 | 1.653 |
| [task-allocation/v02/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-02/05-larger-mix.lp) | 7.78 | 5.20 | 1.496 |
| [task-allocation/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-03/01-basic.lp) | 6.53 | 3.92 | 1.665 |
| [task-allocation/v03/02-multiple-groups](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-03/02-multiple-groups.lp) | 6.53 | 5.20 | 1.255 |
| [task-allocation/v03/03-mixed-grouped-ungrouped](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp) | 6.58 | 3.92 | 1.676 |
| [task-allocation/v03/04-incompatible-group-unsat](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp) | 5.23 | 3.90 | 1.341 |
| [task-allocation/v03/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-03/05-larger-mix.lp) | 6.50 | 3.90 | 1.668 |
| [task-allocation/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-04/01-basic.lp) | 9.00 | 22.74 | 0.396 |
| [task-allocation/v04/02-precedence](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-04/02-precedence.lp) | 7.81 | 6.47 | 1.207 |
| [task-allocation/v04/03-agent-serialization](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-04/03-agent-serialization.lp) | 7.78 | 24.08 | 0.323 |
| [task-allocation/v04/04-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp) | 6.62 | 5.21 | 1.270 |
| [task-allocation/v04/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/task-allocation/variant-04/05-larger-mix.lp) | 28.74 | 181.34 | 0.159 |
| [traveling-salesman/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-01/01-basic.lp) | 7.92 | 5.30 | 1.495 |
| [traveling-salesman/v01/02-multiple-tours](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-01/02-multiple-tours.lp) | 6.57 | 5.21 | 1.260 |
| [traveling-salesman/v01/03-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-01/03-asymmetric.lp) | 7.89 | 5.17 | 1.525 |
| [traveling-salesman/v01/04-subtour-unsat](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp) | 6.57 | 3.94 | 1.668 |
| [traveling-salesman/v01/05-ring](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-01/05-ring.lp) | 8.18 | 5.19 | 1.575 |
| [traveling-salesman/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-02/01-basic.lp) | 6.54 | 5.20 | 1.256 |
| [traveling-salesman/v02/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-02/02-single-salesman.lp) | 6.64 | 5.18 | 1.283 |
| [traveling-salesman/v02/03-too-many-salesmen-unsat](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp) | 6.58 | 5.20 | 1.266 |
| [traveling-salesman/v02/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp) | 6.49 | 5.19 | 1.251 |
| [traveling-salesman/v02/05-larger-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp) | 6.55 | 5.21 | 1.256 |
| [traveling-salesman/v02/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp) | 6.58 | 5.19 | 1.268 |
| [traveling-salesman/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-03/01-basic.lp) | 6.50 | 5.16 | 1.259 |
| [traveling-salesman/v03/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-03/02-single-salesman.lp) | 6.57 | 5.20 | 1.263 |
| [traveling-salesman/v03/03-depot-crossing-unsat](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp) | 6.56 | 5.19 | 1.263 |
| [traveling-salesman/v03/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp) | 6.53 | 5.19 | 1.257 |
| [traveling-salesman/v03/05-larger-three-depots](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp) | 7.78 | 5.20 | 1.495 |
| [traveling-salesman/v03/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp) | 6.50 | 3.93 | 1.654 |
| [traveling-salesman/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-04/01-basic.lp) | 7.82 | 5.19 | 1.506 |
| [traveling-salesman/v04/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-04/02-single-salesman.lp) | 7.64 | 5.19 | 1.473 |
| [traveling-salesman/v04/03-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp) | 6.51 | 5.19 | 1.254 |
| [traveling-salesman/v04/04-depot-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp) | 6.54 | 5.20 | 1.259 |
| [traveling-salesman/v04/05-three-depots-asymmetric-times](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp) | 8.99 | 5.18 | 1.736 |
| [traveling-salesman/v04/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp) | 7.79 | 5.19 | 1.501 |
| [traveling-salesman/v05/01-basic](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-05/01-basic.lp) | 7.63 | 5.18 | 1.474 |
| [traveling-salesman/v05/02-tight-bound-exact](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp) | 6.55 | 5.20 | 1.258 |
| [traveling-salesman/v05/03-revisit-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp) | 6.48 | 5.19 | 1.248 |
| [traveling-salesman/v05/04-depot-and-vertex-revisits](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp) | 6.54 | 5.21 | 1.257 |
| [traveling-salesman/v05/05-three-depots-mixed](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp) | 9.10 | 5.21 | 1.746 |
| [traveling-salesman/v05/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp) | 9.11 | 5.19 | 1.754 |
| [n-queens/v01](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/n-queens/variant-01.lp) | 12.81 | 5.19 | 2.468 |
| [n-queens/v02](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/n-queens/variant-02.lp) | 31.58 | 118.36 | 0.267 |
| [n-queens/v03](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/n-queens/variant-03.lp) | 12.93 | 6.48 | 1.996 |
| [n-queens/v04](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/n-queens/variant-04.lp) | 7.82 | 6.46 | 1.210 |
| [n-queens/v05](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/n-queens/variant-05.lp) | 9.21 | 6.45 | 1.427 |
| [n-queens/v06](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/n-queens/variant-06.lp) | 9.14 | 6.43 | 1.420 |
| [send-money/send-money](https://github.com/GregoryGelfond/zetesis/blob/main/examples/correctness/standalone/send-money/send-money.lp) | 12.84 | 11.48 | 1.118 |
| **All 94: sum of per-case medians** | **729.179** | **804.916** | **0.906** |

## Identity and reproduction

The measured zetesis binary SHA-256 is `d75a5060bb76194d954c022b5a58ff9aa2106e019863b0a75e2113ac71938647`; its workspace source-manifest digest is `8c4c4499a2f8269b1905ad5357a9b7c9950edca3762ed8903c8b678e87b73d5d`. The observations retain individual workspace source, shader, manifest and configuration hashes, and the Cargo.lock identity for pinned dependencies. These hashes identify the measurement independently of a branch or release tag; a rebuilt executable has its own identity.

From the checkout whose source manifest is being measured, build the tools and use a fresh report destination:

```sh
cargo build --locked --release -p zetesis-cli -p zetesis-bench --all-features --bins
target/release/zetesis --version
clingo --version
target/release/zetesis-bench run examples/correctness \
  --suite corpus --zetesis "$PWD/target/release/zetesis" \
  --clingo "$(command -v clingo)" --backend cpu --grounder auto --oracle auto \
  --threads 14 --clingo-threads 1 --warmups 1 --repetitions 3 --memory-runs 1 \
  --timeout-seconds 10 --campaign-seconds 600 --report corpus-0.4-candidate.json
target/release/zetesis-bench compare measured=corpus-0.4-candidate.json \
  --markdown > corpus-0.4-candidate.md
```

Use clingo 5.8.2 and the pinned Rust toolchain. Retain the checkout/source hashes, executable hashes and complete generated report with each new run. Stop competing builds and measurements first. A different machine or source produces a new observation; source identity alone does not predict identical timings. The public JSON is a path-free projection of retained evidence, not another measurement or a runner input. Its raw-report digest identifies retained evidence rather than promising a public raw-capture download.
