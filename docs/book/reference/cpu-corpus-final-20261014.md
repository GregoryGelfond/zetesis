# Final 0.6.0 CPU corpus check, 14 October 2026

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

This final check measures the 0.6.0 candidate after the lookup correction, on all 94 corpus programs. It is separate from the [earlier repeated comparison with 0.5.0](cpu-corpus-20261014.md); its results supersede that candidate’s figures in the README.

Each time is the median of three fresh-process runs. Totals sum those per-program medians; they are not one timed combined run. The [portable observations](observations/cpu-corpus-final-20261014.json) retain all timed values, phases, memory, counts, costs and source hashes. The [TSV](observations/cpu-corpus-final-20261014.tsv) contains the complete derived table.

## Results

| zetesis threads | zetesis sum | clingo sum | Wins / close / losses | Lower zetesis time |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 561.044 ms | 808.198 ms | 13 / 68 / 13 | 16 / 94 |
| 14 | 520.523 ms | 803.673 ms | 16 / 65 / 13 | 19 / 94 |

Wins and losses use a descriptive 3% band: zetesis/clingo below 0.97 wins, 0.97–1.03 is close, and above 1.03 loses. This is not a statistical significance test. Clingo 5.8.2 uses one thread throughout. At fourteen zetesis threads, three of the 65 close cases favor zetesis; small-case direction counts vary considerably between campaigns.

All cases completed and agreed on displayed answers, multiplicities and costs, including every optimal tie. Full native answer families also match the preceding candidate across all six positions per case. The comparison cannot inspect clingo’s hidden interpretations.

Compared with the earlier candidate estimates, summed process time is 7.54% higher at one thread and 4.26% higher at fourteen; reported solver-driver time is 0.87% and 2.46% higher. These measurements do not isolate the cause. The earlier 79-of-94 directional count describes that earlier snapshot, not this final check.

The recorded 0.5.0 totals were 808.670 ms and 777.447 ms at one and fourteen threads. The final totals are approximately 31% and 33% lower, respectively. Those baselines came from the earlier alternating campaign; 0.5.0 was not rerun alongside this final check. The change includes the separate CPU executable as well as grounding and solving improvements.

## Method and phases

The host was an Apple M4 Pro with 48 GiB RAM and macOS 26.6.2. AC power and no competing intensive work were reported. One campaign ran at fourteen zetesis threads, followed by one at a single thread. Each solver/case had one qualification, one warmup, three timed runs and one separate peak-RSS observation. Both campaigns passed all 1,128 planned positions; none were omitted.

zetesis used CPU execution, automatic grounding and oracle selection, indexed formula joins and region search. Automatic grounding was eager for every case. Both solvers enumerated all ordinary answers or all final optimal ties; clingo used `--opt-mode=optN`. Whole-process time includes startup, input, grounding, solving and captured output. zetesis statistics were enabled. GPU execution, explicit lazy execution and clingo’s parallel modes were not measured.

The phase figures below sum per-case medians independently and need not add exactly to driver time. RSS is a separate peak host-memory observation, not live heap or device memory.

| Threads | Driver | Source preparation | Grounding | Solving | Median case RSS | Largest case RSS |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 267.721 ms | 62.686 ms | 77.802 ms | 106.114 ms | 11.578 MiB | 14.453 MiB |
| 14 | 226.060 ms | 63.675 ms | 78.578 ms | 60.688 ms | 11.680 MiB | 16.641 MiB |

## All 94 programs

Times are milliseconds. Each program is unchanged from the earlier comparison. The ratio is zetesis/clingo; below one favors zetesis.

### 1 zetesis thread

| Program | zetesis | clingo | Ratio |
| --- | ---: | ---: | ---: |
| [equality-generalized-tsp/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/equality-generalized-tsp/01-basic.lp) | 4.811 | 4.701 | 1.023 |
| [equality-generalized-tsp/02-larger](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/equality-generalized-tsp/02-larger.lp) | 4.718 | 4.666 | 1.011 |
| [equality-generalized-tsp/03-unreachable-subset-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp) | 3.268 | 4.680 | 0.698 |
| [shortest-path/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/01-basic.lp) | 3.204 | 4.691 | 0.683 |
| [shortest-path/v01/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/02-start-equals-end.lp) | 3.217 | 4.708 | 0.683 |
| [shortest-path/v01/03-zero-cost-detour](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/03-zero-cost-detour.lp) | 4.685 | 4.646 | 1.008 |
| [shortest-path/v01/04-no-path](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/04-no-path.lp) | 3.226 | 4.684 | 0.689 |
| [shortest-path/v01/05-multi-path](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/05-multi-path.lp) | 4.709 | 4.657 | 1.011 |
| [shortest-path/v01/06-layered-dag](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/06-layered-dag.lp) | 4.690 | 4.681 | 1.002 |
| [shortest-path/v01/07-cycles](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/07-cycles.lp) | 4.729 | 4.647 | 1.018 |
| [shortest-path/v01/08-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/08-negative-weights.lp) | 4.771 | 4.694 | 1.016 |
| [shortest-path/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/01-basic.lp) | 4.691 | 4.676 | 1.003 |
| [shortest-path/v02/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/02-start-equals-end.lp) | 3.215 | 4.645 | 0.692 |
| [shortest-path/v02/03-before-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/03-before-forces-detour.lp) | 4.742 | 4.658 | 1.018 |
| [shortest-path/v02/04-after-forces-extension](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/04-after-forces-extension.lp) | 4.740 | 4.638 | 1.022 |
| [shortest-path/v02/05-ordering-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/05-ordering-unsat.lp) | 4.657 | 4.669 | 0.997 |
| [shortest-path/v02/06-layered-dag-before](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/06-layered-dag-before.lp) | 6.272 | 6.250 | 1.004 |
| [shortest-path/v02/07-layered-dag-before-after](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp) | 6.238 | 6.201 | 1.006 |
| [shortest-path/v02/08-tie-break-under-ordering](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp) | 4.745 | 4.704 | 1.009 |
| [shortest-path/v02/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/09-negative-weights.lp) | 4.737 | 4.678 | 1.013 |
| [shortest-path/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/01-basic.lp) | 4.768 | 4.691 | 1.016 |
| [shortest-path/v03/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/02-start-equals-end.lp) | 3.184 | 4.652 | 0.684 |
| [shortest-path/v03/03-budget-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/03-budget-forces-detour.lp) | 4.752 | 4.695 | 1.012 |
| [shortest-path/v03/04-cost-at-cap-allowed](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp) | 4.675 | 4.689 | 0.997 |
| [shortest-path/v03/05-budget-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/05-budget-unsat.lp) | 4.748 | 4.680 | 1.014 |
| [shortest-path/v03/06-layered-dag-cap](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/06-layered-dag-cap.lp) | 6.259 | 6.187 | 1.012 |
| [shortest-path/v03/07-layered-dag-tight-cap](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp) | 6.329 | 6.191 | 1.022 |
| [shortest-path/v03/08-tie-break-under-cap](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp) | 4.699 | 4.645 | 1.012 |
| [shortest-path/v03/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/09-negative-weights.lp) | 4.759 | 4.643 | 1.025 |
| [shortest-path/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/01-basic.lp) | 4.779 | 4.686 | 1.020 |
| [shortest-path/v04/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/02-start-equals-end.lp) | 4.733 | 4.690 | 1.009 |
| [shortest-path/v04/03-before-forces-detour-within-budget](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp) | 4.702 | 4.685 | 1.004 |
| [shortest-path/v04/04-ordering-violates-budget](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp) | 4.818 | 4.692 | 1.027 |
| [shortest-path/v04/05-after-and-budget-interact](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp) | 4.759 | 4.664 | 1.020 |
| [shortest-path/v04/06-layered-dag-ordering-cap](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp) | 6.267 | 6.174 | 1.015 |
| [shortest-path/v04/07-layered-dag-combined](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/07-layered-dag-combined.lp) | 6.271 | 6.167 | 1.017 |
| [shortest-path/v04/08-tie-break-under-ordering-and-cap](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp) | 4.849 | 4.729 | 1.025 |
| [shortest-path/v04/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/09-negative-weights.lp) | 4.797 | 4.701 | 1.020 |
| [task-allocation/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-01/01-basic.lp) | 4.865 | 4.669 | 1.042 |
| [task-allocation/v01/02-agent-reuse](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-01/02-agent-reuse.lp) | 4.717 | 4.682 | 1.007 |
| [task-allocation/v01/03-selective-compatibility](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-01/03-selective-compatibility.lp) | 4.870 | 4.716 | 1.033 |
| [task-allocation/v01/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp) | 3.241 | 4.722 | 0.686 |
| [task-allocation/v01/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-01/05-larger-mix.lp) | 4.797 | 4.713 | 1.018 |
| [task-allocation/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-02/01-basic.lp) | 4.757 | 4.684 | 1.016 |
| [task-allocation/v02/02-makespan-tiebreak](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp) | 4.780 | 4.694 | 1.018 |
| [task-allocation/v02/03-cost-dominates](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-02/03-cost-dominates.lp) | 4.745 | 4.657 | 1.019 |
| [task-allocation/v02/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp) | 4.871 | 4.672 | 1.043 |
| [task-allocation/v02/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-02/05-larger-mix.lp) | 4.704 | 4.695 | 1.002 |
| [task-allocation/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-03/01-basic.lp) | 4.795 | 4.668 | 1.027 |
| [task-allocation/v03/02-multiple-groups](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-03/02-multiple-groups.lp) | 4.687 | 4.656 | 1.007 |
| [task-allocation/v03/03-mixed-grouped-ungrouped](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp) | 4.744 | 4.672 | 1.015 |
| [task-allocation/v03/04-incompatible-group-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp) | 3.398 | 4.675 | 0.727 |
| [task-allocation/v03/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-03/05-larger-mix.lp) | 4.772 | 4.706 | 1.014 |
| [task-allocation/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-04/01-basic.lp) | 6.354 | 22.801 | 0.279 |
| [task-allocation/v04/02-precedence](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-04/02-precedence.lp) | 6.406 | 6.221 | 1.030 |
| [task-allocation/v04/03-agent-serialization](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-04/03-agent-serialization.lp) | 6.301 | 25.817 | 0.244 |
| [task-allocation/v04/04-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp) | 4.853 | 4.692 | 1.034 |
| [task-allocation/v04/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-04/05-larger-mix.lp) | 39.508 | 190.330 | 0.208 |
| [traveling-salesman/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-01/01-basic.lp) | 4.927 | 4.809 | 1.024 |
| [traveling-salesman/v01/02-multiple-tours](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-01/02-multiple-tours.lp) | 4.780 | 4.706 | 1.016 |
| [traveling-salesman/v01/03-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-01/03-asymmetric.lp) | 4.778 | 4.718 | 1.013 |
| [traveling-salesman/v01/04-subtour-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp) | 4.703 | 4.658 | 1.010 |
| [traveling-salesman/v01/05-ring](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-01/05-ring.lp) | 6.349 | 6.185 | 1.026 |
| [traveling-salesman/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-02/01-basic.lp) | 4.768 | 4.697 | 1.015 |
| [traveling-salesman/v02/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-02/02-single-salesman.lp) | 4.759 | 4.694 | 1.014 |
| [traveling-salesman/v02/03-too-many-salesmen-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp) | 4.754 | 4.685 | 1.015 |
| [traveling-salesman/v02/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp) | 4.731 | 4.733 | 1.000 |
| [traveling-salesman/v02/05-larger-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp) | 4.748 | 4.667 | 1.017 |
| [traveling-salesman/v02/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp) | 4.920 | 4.709 | 1.045 |
| [traveling-salesman/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-03/01-basic.lp) | 4.719 | 4.677 | 1.009 |
| [traveling-salesman/v03/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-03/02-single-salesman.lp) | 3.266 | 4.609 | 0.709 |
| [traveling-salesman/v03/03-depot-crossing-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp) | 4.716 | 4.647 | 1.015 |
| [traveling-salesman/v03/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp) | 4.801 | 4.689 | 1.024 |
| [traveling-salesman/v03/05-larger-three-depots](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp) | 4.724 | 4.651 | 1.016 |
| [traveling-salesman/v03/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp) | 4.762 | 4.684 | 1.017 |
| [traveling-salesman/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-04/01-basic.lp) | 4.695 | 4.671 | 1.005 |
| [traveling-salesman/v04/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-04/02-single-salesman.lp) | 4.816 | 4.651 | 1.036 |
| [traveling-salesman/v04/03-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp) | 4.706 | 4.651 | 1.012 |
| [traveling-salesman/v04/04-depot-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp) | 4.720 | 4.670 | 1.011 |
| [traveling-salesman/v04/05-three-depots-asymmetric-times](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp) | 6.203 | 4.641 | 1.337 |
| [traveling-salesman/v04/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp) | 6.265 | 4.677 | 1.340 |
| [traveling-salesman/v05/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-05/01-basic.lp) | 4.724 | 4.664 | 1.013 |
| [traveling-salesman/v05/02-tight-bound-exact](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp) | 4.698 | 4.677 | 1.005 |
| [traveling-salesman/v05/03-revisit-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp) | 4.688 | 4.665 | 1.005 |
| [traveling-salesman/v05/04-depot-and-vertex-revisits](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp) | 4.733 | 4.692 | 1.009 |
| [traveling-salesman/v05/05-three-depots-mixed](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp) | 6.225 | 4.647 | 1.340 |
| [traveling-salesman/v05/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp) | 6.270 | 4.653 | 1.348 |
| [n-queens/v01](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/standalone/n-queens/variant-01.lp) | 6.235 | 6.187 | 1.008 |
| [n-queens/v02](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/standalone/n-queens/variant-02.lp) | 54.624 | 120.789 | 0.452 |
| [n-queens/v03](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/standalone/n-queens/variant-03.lp) | 7.926 | 6.220 | 1.274 |
| [n-queens/v04](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/standalone/n-queens/variant-04.lp) | 6.311 | 6.193 | 1.019 |
| [n-queens/v05](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/standalone/n-queens/variant-05.lp) | 7.787 | 6.235 | 1.249 |
| [n-queens/v06](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/standalone/n-queens/variant-06.lp) | 9.361 | 6.199 | 1.510 |
| [send-money/send-money](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/standalone/send-money/send-money.lp) | 12.476 | 12.222 | 1.021 |

### 14 zetesis threads

| Program | zetesis | clingo | Ratio |
| --- | ---: | ---: | ---: |
| [equality-generalized-tsp/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/equality-generalized-tsp/01-basic.lp) | 4.829 | 4.726 | 1.022 |
| [equality-generalized-tsp/02-larger](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/equality-generalized-tsp/02-larger.lp) | 4.739 | 4.677 | 1.013 |
| [equality-generalized-tsp/03-unreachable-subset-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp) | 4.791 | 4.684 | 1.023 |
| [shortest-path/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/01-basic.lp) | 4.779 | 4.675 | 1.022 |
| [shortest-path/v01/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/02-start-equals-end.lp) | 3.243 | 4.681 | 0.693 |
| [shortest-path/v01/03-zero-cost-detour](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/03-zero-cost-detour.lp) | 4.745 | 4.683 | 1.013 |
| [shortest-path/v01/04-no-path](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/04-no-path.lp) | 3.273 | 4.715 | 0.694 |
| [shortest-path/v01/05-multi-path](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/05-multi-path.lp) | 4.692 | 4.659 | 1.007 |
| [shortest-path/v01/06-layered-dag](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/06-layered-dag.lp) | 6.203 | 6.197 | 1.001 |
| [shortest-path/v01/07-cycles](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/07-cycles.lp) | 4.739 | 4.673 | 1.014 |
| [shortest-path/v01/08-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-01/08-negative-weights.lp) | 4.769 | 4.698 | 1.015 |
| [shortest-path/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/01-basic.lp) | 4.724 | 4.673 | 1.011 |
| [shortest-path/v02/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/02-start-equals-end.lp) | 3.199 | 4.675 | 0.684 |
| [shortest-path/v02/03-before-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/03-before-forces-detour.lp) | 4.724 | 4.669 | 1.012 |
| [shortest-path/v02/04-after-forces-extension](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/04-after-forces-extension.lp) | 4.774 | 4.645 | 1.028 |
| [shortest-path/v02/05-ordering-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/05-ordering-unsat.lp) | 4.693 | 4.702 | 0.998 |
| [shortest-path/v02/06-layered-dag-before](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/06-layered-dag-before.lp) | 6.260 | 6.209 | 1.008 |
| [shortest-path/v02/07-layered-dag-before-after](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp) | 6.211 | 6.175 | 1.006 |
| [shortest-path/v02/08-tie-break-under-ordering](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp) | 4.776 | 4.704 | 1.015 |
| [shortest-path/v02/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-02/09-negative-weights.lp) | 4.718 | 4.658 | 1.013 |
| [shortest-path/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/01-basic.lp) | 4.770 | 4.686 | 1.018 |
| [shortest-path/v03/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/02-start-equals-end.lp) | 4.699 | 4.687 | 1.002 |
| [shortest-path/v03/03-budget-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/03-budget-forces-detour.lp) | 4.747 | 4.724 | 1.005 |
| [shortest-path/v03/04-cost-at-cap-allowed](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp) | 3.173 | 4.656 | 0.682 |
| [shortest-path/v03/05-budget-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/05-budget-unsat.lp) | 4.754 | 4.665 | 1.019 |
| [shortest-path/v03/06-layered-dag-cap](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/06-layered-dag-cap.lp) | 6.224 | 6.172 | 1.008 |
| [shortest-path/v03/07-layered-dag-tight-cap](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp) | 6.317 | 6.171 | 1.024 |
| [shortest-path/v03/08-tie-break-under-cap](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp) | 4.759 | 4.670 | 1.019 |
| [shortest-path/v03/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-03/09-negative-weights.lp) | 4.741 | 4.693 | 1.010 |
| [shortest-path/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/01-basic.lp) | 4.710 | 4.653 | 1.012 |
| [shortest-path/v04/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/02-start-equals-end.lp) | 4.740 | 4.693 | 1.010 |
| [shortest-path/v04/03-before-forces-detour-within-budget](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp) | 4.727 | 4.671 | 1.012 |
| [shortest-path/v04/04-ordering-violates-budget](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp) | 4.751 | 4.674 | 1.016 |
| [shortest-path/v04/05-after-and-budget-interact](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp) | 4.736 | 4.666 | 1.015 |
| [shortest-path/v04/06-layered-dag-ordering-cap](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp) | 6.331 | 6.217 | 1.018 |
| [shortest-path/v04/07-layered-dag-combined](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/07-layered-dag-combined.lp) | 6.315 | 6.210 | 1.017 |
| [shortest-path/v04/08-tie-break-under-ordering-and-cap](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp) | 4.777 | 4.650 | 1.027 |
| [shortest-path/v04/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/shortest-path/variant-04/09-negative-weights.lp) | 4.734 | 4.651 | 1.018 |
| [task-allocation/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-01/01-basic.lp) | 3.230 | 4.662 | 0.693 |
| [task-allocation/v01/02-agent-reuse](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-01/02-agent-reuse.lp) | 3.242 | 4.668 | 0.695 |
| [task-allocation/v01/03-selective-compatibility](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-01/03-selective-compatibility.lp) | 3.252 | 4.705 | 0.691 |
| [task-allocation/v01/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp) | 3.194 | 4.697 | 0.680 |
| [task-allocation/v01/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-01/05-larger-mix.lp) | 4.699 | 4.720 | 0.996 |
| [task-allocation/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-02/01-basic.lp) | 4.764 | 4.689 | 1.016 |
| [task-allocation/v02/02-makespan-tiebreak](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp) | 4.735 | 4.698 | 1.008 |
| [task-allocation/v02/03-cost-dominates](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-02/03-cost-dominates.lp) | 4.760 | 4.678 | 1.018 |
| [task-allocation/v02/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp) | 4.844 | 4.689 | 1.033 |
| [task-allocation/v02/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-02/05-larger-mix.lp) | 6.246 | 4.695 | 1.331 |
| [task-allocation/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-03/01-basic.lp) | 4.730 | 4.654 | 1.016 |
| [task-allocation/v03/02-multiple-groups](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-03/02-multiple-groups.lp) | 4.803 | 4.675 | 1.027 |
| [task-allocation/v03/03-mixed-grouped-ungrouped](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp) | 4.815 | 4.688 | 1.027 |
| [task-allocation/v03/04-incompatible-group-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp) | 3.217 | 4.679 | 0.688 |
| [task-allocation/v03/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-03/05-larger-mix.lp) | 4.773 | 4.712 | 1.013 |
| [task-allocation/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-04/01-basic.lp) | 6.229 | 23.887 | 0.261 |
| [task-allocation/v04/02-precedence](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-04/02-precedence.lp) | 6.374 | 6.239 | 1.022 |
| [task-allocation/v04/03-agent-serialization](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-04/03-agent-serialization.lp) | 6.315 | 24.308 | 0.260 |
| [task-allocation/v04/04-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp) | 4.875 | 4.709 | 1.035 |
| [task-allocation/v04/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/task-allocation/variant-04/05-larger-mix.lp) | 28.263 | 176.552 | 0.160 |
| [traveling-salesman/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-01/01-basic.lp) | 4.868 | 4.786 | 1.017 |
| [traveling-salesman/v01/02-multiple-tours](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-01/02-multiple-tours.lp) | 4.753 | 4.679 | 1.016 |
| [traveling-salesman/v01/03-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-01/03-asymmetric.lp) | 4.788 | 4.674 | 1.024 |
| [traveling-salesman/v01/04-subtour-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp) | 4.745 | 4.692 | 1.011 |
| [traveling-salesman/v01/05-ring](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-01/05-ring.lp) | 6.238 | 6.242 | 0.999 |
| [traveling-salesman/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-02/01-basic.lp) | 4.766 | 4.396 | 1.084 |
| [traveling-salesman/v02/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-02/02-single-salesman.lp) | 4.768 | 4.679 | 1.019 |
| [traveling-salesman/v02/03-too-many-salesmen-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp) | 4.711 | 4.676 | 1.008 |
| [traveling-salesman/v02/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp) | 4.853 | 4.717 | 1.029 |
| [traveling-salesman/v02/05-larger-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp) | 4.763 | 4.658 | 1.022 |
| [traveling-salesman/v02/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp) | 4.841 | 4.677 | 1.035 |
| [traveling-salesman/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-03/01-basic.lp) | 4.720 | 4.654 | 1.014 |
| [traveling-salesman/v03/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-03/02-single-salesman.lp) | 4.741 | 4.677 | 1.014 |
| [traveling-salesman/v03/03-depot-crossing-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp) | 4.698 | 4.689 | 1.002 |
| [traveling-salesman/v03/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp) | 4.770 | 4.691 | 1.017 |
| [traveling-salesman/v03/05-larger-three-depots](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp) | 4.724 | 4.682 | 1.009 |
| [traveling-salesman/v03/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp) | 4.870 | 4.638 | 1.050 |
| [traveling-salesman/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-04/01-basic.lp) | 4.772 | 6.187 | 0.771 |
| [traveling-salesman/v04/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-04/02-single-salesman.lp) | 4.927 | 4.746 | 1.038 |
| [traveling-salesman/v04/03-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp) | 4.796 | 4.683 | 1.024 |
| [traveling-salesman/v04/04-depot-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp) | 4.939 | 4.711 | 1.048 |
| [traveling-salesman/v04/05-three-depots-asymmetric-times](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp) | 6.301 | 6.180 | 1.020 |
| [traveling-salesman/v04/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp) | 6.347 | 4.684 | 1.355 |
| [traveling-salesman/v05/01-basic](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-05/01-basic.lp) | 4.787 | 6.212 | 0.771 |
| [traveling-salesman/v05/02-tight-bound-exact](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp) | 4.920 | 4.751 | 1.035 |
| [traveling-salesman/v05/03-revisit-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp) | 4.786 | 4.667 | 1.025 |
| [traveling-salesman/v05/04-depot-and-vertex-revisits](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp) | 4.802 | 4.726 | 1.016 |
| [traveling-salesman/v05/05-three-depots-mixed](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp) | 6.293 | 6.179 | 1.019 |
| [traveling-salesman/v05/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp) | 6.306 | 4.698 | 1.342 |
| [n-queens/v01](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/standalone/n-queens/variant-01.lp) | 6.247 | 6.209 | 1.006 |
| [n-queens/v02](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/standalone/n-queens/variant-02.lp) | 25.926 | 122.510 | 0.212 |
| [n-queens/v03](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/standalone/n-queens/variant-03.lp) | 6.415 | 6.255 | 1.026 |
| [n-queens/v04](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/standalone/n-queens/variant-04.lp) | 6.377 | 6.219 | 1.025 |
| [n-queens/v05](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/standalone/n-queens/variant-05.lp) | 7.870 | 6.237 | 1.262 |
| [n-queens/v06](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/standalone/n-queens/variant-06.lp) | 7.897 | 6.263 | 1.261 |
| [send-money/send-money](https://github.com/GregoryGelfond/zetesis/blob/v0.6.0/examples/correctness/standalone/send-money/send-money.lp) | 10.949 | 12.257 | 0.893 |

## Identities and reproduction

The measured CPU executable reports 0.6.0. Its source-manifest SHA-256 is `229398f36b7292327b96b5f81ebcbff6fca28a43b100ce2687ef2b6f475bc211`; publication-only documentation follows this measured snapshot. The retained benchmark runner is the same executable used for the earlier study. Rebuilt binaries must record their own identities.

| Executable | SHA-256 |
| --- | --- |
| zetesis CPU | `ae6c213806eec6ec31145a8a616d593d7e533701a26f8e21771b4218530e3ae9` |
| Benchmark runner | `e47d59cde913931d4f602870c2052bfc7a8f4bcf77c655569946a92fcb2952dd` |
| Direct clingo 5.8.2 | `31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015` |

The corpus manifest SHA-256 is `b43df1adf17ae0c035f1e310a5c15345c26cbcad8b59596932627c46fd1c6958`. The portable observations retain individual input and raw-report hashes; a raw-report hash identifies retained evidence, not a public capture download.

From the release checkout, set `ZETESIS`, `BENCH` and `CLINGO` to the solver, maintained benchmark runner and direct clingo executable paths. Stop other builds and measurements first. Use a new output directory for each campaign.

```sh
: "${ZETESIS:?Set the zetesis executable path}"
: "${BENCH:?Set the maintained zetesis-bench path}"
: "${CLINGO:?Set the direct clingo 5.8.2 executable path}"
mkdir -p measurements/cpu-corpus-final-20261014
for workers in 14 1; do
  "$BENCH" run examples/correctness --suite corpus \
    --zetesis "$ZETESIS" --native-interface solve --clingo "$CLINGO" \
    --backend cpu --grounder auto --oracle auto --threads "$workers" \
    --formula-joins indexed --search regions --clingo-threads 1 \
    --warmups 1 --repetitions 3 --memory-runs 1 \
    --timeout-seconds 120 --campaign-seconds 1800 \
    --report "measurements/cpu-corpus-final-20261014/w${workers}.json" --json
done
```

The process deadline was 120 seconds, compared with 30 in the earlier study; every case completed in both. These are benchmark deadlines, not solver work or memory overrides. Ordinary solver defaults were retained. A different machine or binary produces a new observation; preserve refusals and timeouts.
