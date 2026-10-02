# Reusing grounding and formula preparation

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

This record compares all 94 unchanged curated programs on the CPU, with every answer set or tied optimum requested. It compares a combined set of preparation, domain and representation changes against the original source and an intermediate version. The three sources are:

| Label | Source |
| --- | --- |
| Original | [`ec6adadd`](https://github.com/GregoryGelfond/zetesis/commit/ec6adaddc250dae7ccc01e3efd3fb73220f9dbaf) |
| Prior | [`d6557d0a`](https://github.com/GregoryGelfond/zetesis/commit/d6557d0af5f46978196aa40998d27630dd94d674) |
| Candidate | [`120fadfb`](https://github.com/GregoryGelfond/zetesis/commit/120fadfb3744c00760bcefd575ce3641075341dc) |

The corpus campaigns ran on 1 October 2026 local time, from 00:36:22 to 00:38:38 UTC on 2 October, on an Apple M4 Pro with macOS 26.6.2 and AC power. Rust 1.97.1 built the release executables. No other builds or measurements were run concurrently. Their order was original A, prior A, candidate A, candidate B, prior B, original B. All used the same maintained benchmark runner, CPU backend, automatic grounding and oracle selection, 14 search workers, one completion worker and batches of 64 candidates. Every corpus case actually used eager grounding; hybrid and relational lazy execution need their separate measurements. Adjacent clingo 5.8.2 used one worker and its stock search heuristics. Solver budgets remained at their defaults; the measurement harness allowed 30 seconds per child and 1,200 seconds per campaign.

Each campaign had one qualification, one warmup, five timed runs and two separate RSS runs for each solver and case: **1,692 planned positions per report**, or **10,152 across six reports**. All positions passed and were accounted for, without changed inputs, campaign faults or unsettled children. The native qualification families matched across all six reports after resolving the incremental atom catalogs: full typed interpretations, shown atoms and terms, costs and model multiplicity. Selected-model counts and costs also agreed across every native sample. Clingo qualifies shown answers and costs; that comparison alone does not establish equality of hidden native atoms.

## What changed

The combined changes reuse completed joins, objective preparation and arithmetic
results; derive conservative domains from positive bodies; and reduce propagation
and candidate-transfer storage. Comparison heads and the curated Sudoku example
extend the exercised language and workload set. The last step, between the prior
and candidate sources, retains completed hybrid preparation, removes redundant
finite-table mask initialization and reuses aggregate-prefix validation.

These changes share checked operations and their resource accounting. They do
not replace original satisfaction or reduct membership. The
[grounding architecture](../architecture/grounding.md),
[finite-table API](../rust/finite-tables.md) and
[Lean correspondence](../lean/correspondence.md) describe the contracts and
remaining implementation-proof boundaries. This comparison measures integrated
revisions; it does not attribute each gain to one change.

## Timing and memory

For each program and revision, the value below is the arithmetic mean of its two campaign medians. Each timing median has five samples. Totals sum those per-program values; they are not a separately timed combined run. Whole-process wall time includes startup, input loading, parsing, grounding, solving and captured JSON/statistics output. Driver time omits source loading and the output envelope. Grounding is the recorded grounding stage; phases can overlap and must not be added to reconstruct wall time.

| Revision | Grounding sum ms | Driver sum ms | Wall sum ms | Mean case RSS MiB | Largest case RSS MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| Original | 106.218 | 291.896 | 845.723 | 15.524 | 36.730 |
| Prior | 95.586 | 272.782 | 824.571 | 15.244 | 25.098 |
| Candidate | 94.160 | 271.510 | 823.370 | 15.280 | 26.223 |

Against the original source, the candidate is **1.128× faster in grounding**, **1.075× faster in driver time** and **1.027× faster in wall time** on these totals. Against the prior candidate, the factors are 1.015×, 1.005× and 1.001×. The latter wall result is effectively flat at this repetition level: prior campaign totals were 814.503 and 834.639 ms, while candidate totals were 821.587 and 825.154 ms.

The candidate’s adjacent clingo total is **812.490 ms**, against **823.370 ms** for zetesis: **clingo 1.01× faster**. Zetesis is faster on **4 of 94 cases**, clingo on 90, with the same four native wins in every report. Those wins are queens variant 02 and task-allocation variant 04’s basic, agent-serialization and larger-mix cases. For the candidate, respectively, zetesis is 3.65×, 2.56×, 2.75× and 5.88× faster. On SEND, clingo is 1.26× faster.

RSS values are paired means of per-campaign medians from separate memory runs: four fresh-child observations per revision and program. They measure process peak resident memory, not live allocations, bytes copied, simultaneous aggregate memory or device storage. Candidate mean RSS is 15.280 MiB; adjacent clingo averages 5.774 MiB. Candidate mean RSS is 1.6% below original and 0.23% above prior.

## Where the changes appear

The family table separates driver time from process overhead. Triples are **original / prior / candidate**; memory values are means of the programs’ paired RSS medians.

| Family | Programs | Driver sum ms | Mean case RSS MiB |
| --- | ---: | --- | --- |
| Equality generalized TSP | 3 | 5.903 / 5.275 / 5.284 | 14.793 / 14.648 / 14.712 |
| Eight queens | 6 | 54.787 / 52.190 / 51.730 | 19.587 / 17.149 / 17.417 |
| SEND + MORE = MONEY | 1 | 9.245 / 8.873 / 8.866 | 20.641 / 18.223 / 18.617 |
| Shortest path | 35 | 82.019 / 79.100 / 78.237 | 15.206 / 15.148 / 15.145 |
| Task allocation | 20 | 68.943 / 60.387 / 61.409 | 15.363 / 15.116 / 15.095 |
| Traveling salesman | 29 | 70.997 / 66.956 / 65.983 | 15.077 / 15.014 / 15.071 |

The strongest repeatable prior-relative driver regression is task-allocation variant 04/larger-mix: **24.531→26.135 ms**, with both candidate campaign medians above both prior medians. Wall time rises **30.460→32.097 ms**, although zetesis still beats adjacent clingo by 5.88×. Queens variant 02’s RSS increases **25.098→26.223 MiB** versus prior, but remains below original’s **36.730 MiB**. SEND’s driver is nearly unchanged versus prior; wall increases **14.801→15.396 ms**, and RSS **18.223→18.617 MiB**.

Across all programs, candidate wall medians improve on 61 cases versus original and 50 versus prior; driver medians improve on 90 and 69. Checked-candidate ranges differ across revisions on seven cases, and search-work ranges on 27. Optimized searches can visit different candidates while establishing the same optimal family, so these observations do not isolate the cost of one primitive. No correctness or resource-incompleteness regression appears in this corpus; the timing and memory regressions above remain part of the result. These CPU measurements do not qualify device execution or predict a general speedup.

## All 94 programs

All values are wall milliseconds under the paired-median method above. Clingo is the reference measured beside the candidate. Ratios and wins use unrounded values. Program links identify the unchanged source at the candidate revision; full driver, grounding, RSS, counters and failure receipts are retained in the [portable evidence](observations/foundation-reuse-120fadfb-corpus.json).

| Program | Original | Prior | Candidate | Clingo |
| --- | ---: | ---: | ---: | ---: |
| [equality-generalized-tsp/01-basic](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/equality-generalized-tsp/01-basic.lp) | 7.91 | 7.91 | 7.83 | 4.69 |
| [equality-generalized-tsp/02-larger](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/equality-generalized-tsp/02-larger.lp) | 7.87 | 7.80 | 7.82 | 4.67 |
| [equality-generalized-tsp/03-unreachable-subset-unsat](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp) | 7.26 | 7.15 | 7.06 | 4.67 |
| [shortest-path/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-01/01-basic.lp) | 7.15 | 7.58 | 7.01 | 4.67 |
| [shortest-path/v01/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-01/02-start-equals-end.lp) | 7.21 | 7.19 | 7.04 | 4.69 |
| [shortest-path/v01/03-zero-cost-detour](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-01/03-zero-cost-detour.lp) | 7.16 | 6.46 | 6.25 | 4.69 |
| [shortest-path/v01/04-no-path](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-01/04-no-path.lp) | 7.77 | 7.20 | 7.07 | 4.69 |
| [shortest-path/v01/05-multi-path](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-01/05-multi-path.lp) | 7.80 | 7.81 | 7.81 | 4.67 |
| [shortest-path/v01/06-layered-dag](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-01/06-layered-dag.lp) | 9.28 | 9.21 | 9.35 | 6.19 |
| [shortest-path/v01/07-cycles](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-01/07-cycles.lp) | 7.85 | 7.86 | 7.79 | 4.68 |
| [shortest-path/v01/08-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-01/08-negative-weights.lp) | 7.89 | 7.82 | 7.83 | 4.67 |
| [shortest-path/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-02/01-basic.lp) | 7.68 | 7.85 | 7.83 | 4.66 |
| [shortest-path/v02/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-02/02-start-equals-end.lp) | 7.22 | 7.84 | 7.88 | 4.67 |
| [shortest-path/v02/03-before-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-02/03-before-forces-detour.lp) | 7.84 | 7.18 | 7.78 | 4.68 |
| [shortest-path/v02/04-after-forces-extension](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-02/04-after-forces-extension.lp) | 7.89 | 7.83 | 7.79 | 4.67 |
| [shortest-path/v02/05-ordering-unsat](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-02/05-ordering-unsat.lp) | 7.16 | 7.04 | 7.82 | 4.66 |
| [shortest-path/v02/06-layered-dag-before](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-02/06-layered-dag-before.lp) | 9.23 | 9.20 | 9.36 | 6.20 |
| [shortest-path/v02/07-layered-dag-before-after](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp) | 9.71 | 9.21 | 9.33 | 6.20 |
| [shortest-path/v02/08-tie-break-under-ordering](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp) | 7.87 | 7.89 | 7.85 | 4.69 |
| [shortest-path/v02/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-02/09-negative-weights.lp) | 7.79 | 7.82 | 7.81 | 4.67 |
| [shortest-path/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-03/01-basic.lp) | 7.85 | 7.88 | 7.84 | 4.67 |
| [shortest-path/v03/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-03/02-start-equals-end.lp) | 7.15 | 7.86 | 7.02 | 4.68 |
| [shortest-path/v03/03-budget-forces-detour](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-03/03-budget-forces-detour.lp) | 7.85 | 7.84 | 7.82 | 4.67 |
| [shortest-path/v03/04-cost-at-cap-allowed](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp) | 7.14 | 7.15 | 7.77 | 4.66 |
| [shortest-path/v03/05-budget-unsat](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-03/05-budget-unsat.lp) | 7.82 | 7.86 | 7.85 | 4.68 |
| [shortest-path/v03/06-layered-dag-cap](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-03/06-layered-dag-cap.lp) | 11.10 | 10.60 | 10.81 | 6.17 |
| [shortest-path/v03/07-layered-dag-tight-cap](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp) | 10.62 | 10.67 | 10.89 | 6.21 |
| [shortest-path/v03/08-tie-break-under-cap](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp) | 7.88 | 7.84 | 7.77 | 4.68 |
| [shortest-path/v03/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-03/09-negative-weights.lp) | 7.84 | 7.86 | 7.89 | 4.67 |
| [shortest-path/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-04/01-basic.lp) | 7.16 | 7.75 | 7.78 | 4.68 |
| [shortest-path/v04/02-start-equals-end](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-04/02-start-equals-end.lp) | 7.72 | 7.83 | 7.84 | 4.69 |
| [shortest-path/v04/03-before-forces-detour-within-budget](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp) | 7.79 | 7.86 | 7.80 | 4.66 |
| [shortest-path/v04/04-ordering-violates-budget](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp) | 7.88 | 7.86 | 7.81 | 4.67 |
| [shortest-path/v04/05-after-and-budget-interact](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp) | 7.81 | 7.78 | 7.83 | 4.67 |
| [shortest-path/v04/06-layered-dag-ordering-cap](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp) | 11.98 | 11.22 | 10.81 | 6.16 |
| [shortest-path/v04/07-layered-dag-combined](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-04/07-layered-dag-combined.lp) | 11.88 | 11.24 | 10.80 | 6.18 |
| [shortest-path/v04/08-tie-break-under-ordering-and-cap](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp) | 8.68 | 8.63 | 7.87 | 4.65 |
| [shortest-path/v04/09-negative-weights](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/shortest-path/variant-04/09-negative-weights.lp) | 7.83 | 7.82 | 7.84 | 4.66 |
| [task-allocation/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-01/01-basic.lp) | 6.43 | 7.19 | 6.31 | 4.67 |
| [task-allocation/v01/02-agent-reuse](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-01/02-agent-reuse.lp) | 7.17 | 7.15 | 6.26 | 4.68 |
| [task-allocation/v01/03-selective-compatibility](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-01/03-selective-compatibility.lp) | 7.15 | 6.46 | 6.28 | 4.67 |
| [task-allocation/v01/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp) | 6.42 | 6.43 | 6.29 | 4.68 |
| [task-allocation/v01/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-01/05-larger-mix.lp) | 7.84 | 7.24 | 7.06 | 4.69 |
| [task-allocation/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-02/01-basic.lp) | 7.69 | 7.16 | 7.77 | 4.69 |
| [task-allocation/v02/02-makespan-tiebreak](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp) | 7.85 | 7.88 | 7.81 | 4.68 |
| [task-allocation/v02/03-cost-dominates](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-02/03-cost-dominates.lp) | 7.79 | 7.81 | 7.79 | 4.69 |
| [task-allocation/v02/04-no-compatible-agent-unsat](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp) | 7.79 | 7.20 | 7.81 | 4.68 |
| [task-allocation/v02/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-02/05-larger-mix.lp) | 9.21 | 9.22 | 9.30 | 4.70 |
| [task-allocation/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-03/01-basic.lp) | 7.15 | 7.18 | 7.08 | 4.67 |
| [task-allocation/v03/02-multiple-groups](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-03/02-multiple-groups.lp) | 7.79 | 7.79 | 7.03 | 4.68 |
| [task-allocation/v03/03-mixed-grouped-ungrouped](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp) | 7.21 | 7.56 | 7.10 | 4.67 |
| [task-allocation/v03/04-incompatible-group-unsat](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp) | 6.43 | 6.41 | 6.83 | 4.69 |
| [task-allocation/v03/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-03/05-larger-mix.lp) | 7.82 | 7.80 | 7.85 | 4.70 |
| [task-allocation/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-04/01-basic.lp) | 10.71 | 9.89 | 9.46 | 24.26 |
| [task-allocation/v04/02-precedence](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-04/02-precedence.lp) | 9.33 | 9.31 | 9.40 | 6.22 |
| [task-allocation/v04/03-agent-serialization](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-04/03-agent-serialization.lp) | 9.23 | 9.24 | 9.36 | 25.72 |
| [task-allocation/v04/04-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp) | 7.86 | 7.90 | 7.90 | 4.70 |
| [task-allocation/v04/05-larger-mix](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/task-allocation/variant-04/05-larger-mix.lp) | 37.11 | 30.46 | 32.10 | 188.57 |
| [traveling-salesman/v01/01-basic](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-01/01-basic.lp) | 7.96 | 7.98 | 7.91 | 4.76 |
| [traveling-salesman/v01/02-multiple-tours](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-01/02-multiple-tours.lp) | 7.81 | 7.77 | 7.75 | 4.67 |
| [traveling-salesman/v01/03-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-01/03-asymmetric.lp) | 7.87 | 7.85 | 7.86 | 5.19 |
| [traveling-salesman/v01/04-subtour-unsat](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp) | 7.18 | 7.07 | 7.14 | 4.69 |
| [traveling-salesman/v01/05-ring](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-01/05-ring.lp) | 8.17 | 9.23 | 8.57 | 4.69 |
| [traveling-salesman/v02/01-basic](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-02/01-basic.lp) | 7.89 | 7.87 | 7.81 | 4.70 |
| [traveling-salesman/v02/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-02/02-single-salesman.lp) | 7.87 | 7.19 | 7.82 | 4.69 |
| [traveling-salesman/v02/03-too-many-salesmen-unsat](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp) | 7.76 | 7.20 | 7.81 | 4.69 |
| [traveling-salesman/v02/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp) | 7.78 | 7.80 | 7.86 | 4.69 |
| [traveling-salesman/v02/05-larger-asymmetric](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp) | 7.86 | 7.86 | 7.77 | 4.69 |
| [traveling-salesman/v02/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp) | 7.86 | 7.82 | 7.85 | 4.66 |
| [traveling-salesman/v03/01-basic](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-03/01-basic.lp) | 7.77 | 7.80 | 7.83 | 4.67 |
| [traveling-salesman/v03/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-03/02-single-salesman.lp) | 7.86 | 7.85 | 7.84 | 4.67 |
| [traveling-salesman/v03/03-depot-crossing-unsat](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp) | 7.78 | 7.18 | 7.78 | 4.69 |
| [traveling-salesman/v03/04-equal-cost-split](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp) | 7.82 | 7.82 | 7.77 | 4.67 |
| [traveling-salesman/v03/05-larger-three-depots](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp) | 9.18 | 7.80 | 7.83 | 4.67 |
| [traveling-salesman/v03/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp) | 7.86 | 7.85 | 7.80 | 4.67 |
| [traveling-salesman/v04/01-basic](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-04/01-basic.lp) | 7.79 | 7.78 | 8.50 | 4.68 |
| [traveling-salesman/v04/02-single-salesman](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-04/02-single-salesman.lp) | 7.87 | 7.81 | 7.85 | 4.65 |
| [traveling-salesman/v04/03-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp) | 7.77 | 7.81 | 7.78 | 4.66 |
| [traveling-salesman/v04/04-depot-window-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp) | 7.86 | 7.88 | 7.82 | 4.66 |
| [traveling-salesman/v04/05-three-depots-asymmetric-times](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp) | 10.56 | 10.57 | 9.30 | 5.41 |
| [traveling-salesman/v04/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp) | 10.60 | 9.24 | 9.29 | 4.66 |
| [traveling-salesman/v05/01-basic](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-05/01-basic.lp) | 9.21 | 8.45 | 7.84 | 4.69 |
| [traveling-salesman/v05/02-tight-bound-exact](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp) | 7.76 | 7.84 | 7.85 | 4.67 |
| [traveling-salesman/v05/03-revisit-too-tight-unsat](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp) | 7.84 | 7.83 | 7.78 | 4.67 |
| [traveling-salesman/v05/04-depot-and-vertex-revisits](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp) | 7.82 | 7.86 | 7.85 | 4.65 |
| [traveling-salesman/v05/05-three-depots-mixed](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp) | 10.56 | 10.60 | 10.04 | 5.43 |
| [traveling-salesman/v05/06-unreachable-edge](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp) | 10.55 | 9.25 | 9.32 | 4.68 |
| [n-queens/v01](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/standalone/n-queens/variant-01.lp) | 13.25 | 13.20 | 12.59 | 6.20 |
| [n-queens/v02](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/standalone/n-queens/variant-02.lp) | 35.00 | 33.82 | 33.88 | 123.64 |
| [n-queens/v03](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/standalone/n-queens/variant-03.lp) | 14.12 | 13.39 | 13.98 | 6.26 |
| [n-queens/v04](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/standalone/n-queens/variant-04.lp) | 9.29 | 9.29 | 9.41 | 6.22 |
| [n-queens/v05](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/standalone/n-queens/variant-05.lp) | 9.89 | 9.25 | 9.38 | 6.22 |
| [n-queens/v06](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/standalone/n-queens/variant-06.lp) | 10.71 | 9.29 | 9.35 | 6.21 |
| [send-money/send-money](https://github.com/GregoryGelfond/zetesis/blob/120fadfb3744c00760bcefd575ce3641075341dc/examples/correctness/standalone/send-money/send-money.lp) | 15.48 | 14.80 | 15.40 | 12.22 |

The portable evidence records executable and report hashes, source mappings, actual profiles, every per-report cell summary, qualified-family hashes and all timed candidate/work ranges. It is a projection of maintained benchmark reports, not another solver run or a report input. Machine-local paths and raw captures are omitted; the raw report hashes identify retained source evidence rather than public downloads. The [benchmark protocol](benchmarking.md) describes reproduction and the limits of each measurement.

## Worker scaling

The `d6557d0a` → `120fadfb` comparison completed all **55 cells**: eleven
workloads at five worker counts. CPU execution used automatic oracle/grounding
selection, indexed joins, region search, and default budgets. Admission was unchanged.

Every cell passed clingo qualification. Complete native families, shown values,
costs, and multiplicities matched across all 220 qualification documents, including
1,176 task-allocation optimal ties at cost 5. Clingo checks displayed families;
its hidden interpretations are unavailable.

Baseline–candidate–candidate–baseline reports each used one warmup, five timed
repetitions, and two memory runs per cell. Table entries average the two candidate
report medians, in milliseconds, including startup and JSON/statistics output.
[The observations](observations/foundation-reuse-120fadfb-workers.json) retain
both versions’ cell measurements, work receipts, and qualification identities.

| Workload | 1 | 2 | 4 | 8 | 14 |
|---|---:|---:|---:|---:|---:|
| Authored queens, n=8 | 15.25 | 13.85 | 13.68 | 13.66 | 12.99 |
| Authored queens, n=9 | 26.00 | 22.83 | 20.40 | 19.66 | 18.61 |
| Authored queens, n=10 | 59.22 | 44.67 | 35.28 | 30.07 | 27.65 |
| Pigeonhole, h=5 | 6.34 | 6.27 | 6.25 | 6.26 | 7.00 |
| Pigeonhole, h=6 | 7.78 | 7.77 | 7.77 | 7.82 | 7.80 |
| Pigeonhole, h=7 | 16.84 | 15.36 | 12.44 | 10.92 | 11.25 |
| Queens variant 02 | 137.84 | 88.77 | 55.88 | 38.82 | 33.94 |
| SEND+MORE | 16.93 | 15.36 | 15.37 | 15.40 | 15.45 |
| Task allocation, larger mix | 50.56 | 41.40 | 35.89 | 32.44 | 31.62 |
| Curated Sudoku | 33.61 | 33.54 | 33.81 | 34.15 | 34.63 |
| Einstein | 18.45 | 17.49 | 18.07 | 18.26 | 18.04 |

Across the same 55 cell means, summed wall time changed **−0.14%**, driver time
−0.45%, and grounding −0.74%. Mean cell RSS changed +0.04%, from 17.312 to
17.320 MiB. RSS uses each report’s median child peak, not simultaneous memory.
The measurements are essentially flat; no broad speedup is established.
Aggregate one-to-fourteen-worker wall speedup was 1.793× before and 1.775× after.

Sudoku’s table-query work fell 516,456 → 449,388 (−12.99%), while wall time
changed −0.05%. Einstein’s reconstruction work fell 1,230,166 → 885,850
(−27.99%); wall time changed −1.89% and RSS +0.93%. Search work is unchanged. Sudoku uses eager tight checking; Einstein uses
eager base checking with terminal reconstruction. Neither exercises hybrid
grounding here. Curated Sudoku is an easy-search fixture.

Pigeonhole h=5 at fourteen workers increased
wall time 11.22% (+0.706 ms), driven by one report, while driver time rose 0.26%.
Pigeonhole h=7 at two workers rose 6.27% (+0.906 ms), with driver +0.72%.
Task allocation at fourteen workers had driver +2.09%; its parallel search work
varied with scheduling despite identical optimal families. These descriptive observations establish neither statistical significance nor
isolated causation.

## Eager and requested-lazy grounders

This comparison covers only `d6557d0a` → `120fadfb`, the shared-preparation and
table-primitive follow-up. It does not include the earlier foundation gains.
The [grounder observations](observations/foundation-reuse-120fadfb-grounders.json)
retain populations, source identities, execution routes, and per-cell results.

Each version ran twice, in baseline/candidate/candidate/baseline order, with four
CPU workers, automatic oracle selection, one warmup, four timed repetitions, and
two separate memory runs per report. Values are arithmetic means of the two
per-report medians. The following totals sum only cells completed in all four
reports; they are not campaign elapsed times. Small changes have no statistical
significance claim.

| Suite / requested grounder | Completed cells | Wall baseline → candidate, ms | Change | Driver baseline → candidate, ms | Change |
|---|---:|---:|---:|---:|---:|
| Scalability / eager | 10/11 | 239.866 → 237.202 | −1.11% | 177.512 → 176.564 | −0.53% |
| Scalability / lazy | 2/11 | 109.120 → 110.139 | +0.93% | 96.041 → 96.990 | +0.99% |
| Series / eager | 18/22 | 553.049 → 552.581 | −0.08% | 440.456 → 439.609 | −0.19% |
| Series / lazy | 15/22 | 535.436 → 536.562 | +0.21% | 441.360 → 443.273 | +0.43% |

No cell gained or lost completion. Successful qualification records preserve
the complete native families, including typed atoms, displayed values,
multiplicities, costs, and optimal ties. Clingo qualification passed for every
workload's selected display; hidden reference interpretations were unavailable.
Refused cells establish no complete family.

The unchanged non-completions are:

- **Scalability:** requested lazy exceeds the 10,000,000 constraint-check Work
  ceiling for authored queens 8/9/10, pigeonhole 6/7, queens variant 02, and
  SEND+MORE=MONEY. Lazy task allocation rejects objectives. Einstein exceeds
  10,000,000 formula-admission Work under both explicit eager and lazy.
- **Series:** eager transitive paths 100/200 exceed 1,000,000 static rules; eager
  chains 1000/2000 exceed 1,000,000 static atoms. Lazy
  independent-negation-aggregate-16, Latin square 5, queens variant 01 at 10/11,
  and SEND+MORE=MONEY exceed 10,000,000 constraint-check Work. Lazy ties-50 and
  task allocation reject objectives. Their later timed slots were not attempted.

Einstein's successful automatic-grounder measurements use a different admitted
plan: `auto` selects an eager base with terminal definitions and reconstructs
the deferred definition before publishing the original answer. Explicit eager
and lazy select complete or hybrid materialization without that adaptive
partition. The recorded automatic route completes one reconstructed answer;
its success does not establish completion of either explicit profile above.

### Sudoku and execution routes

| Requested grounder | Wall baseline → candidate, ms | Driver baseline → candidate, ms | Peak RSS baseline → candidate, MiB |
|---|---:|---:|---:|
| Eager | 35.114 → 33.592 (−4.33%) | 28.151 → 27.345 (−2.86%) | 23.863 → 23.965 |
| Lazy, effective hybrid | 50.842 → 52.680 (+3.62%) | 44.058 → 45.643 (+3.60%) | 19.492 → 19.672 |

Sudoku table-query work falls 516,456 → 449,388 (−12.99%). Hybrid constraint
work falls 3,097,823 → 3,023,303 (−2.41%), preserving 8,748 substitutions and
one answer. However, hybrid original validation rises 17.593 → 20.629 ms while
admission improves 24.062 → 22.620 ms. Both candidate reports have slower hybrid
driver medians. These combined measurements do not isolate the cost of retained
maps, table selection, or per-operation shared resource admission.

Requested lazy does not always mean hybrid. Independent-choice 12/16 and
independent-negation 8/10 use lazy-interleaved CPU closure, with unchanged work.
Ten completed lazy series cells use closure; five use hybrid. Only planning-14
among those five has nonzero streamed constraint work, unchanged at 3,099,523.
Thus the series provides mostly controls, not evidence of retained-map speedups.

Larger queens cases change little. Adverse driver changes include eager
pigeonhole 7 (+3.50%) and lazy producer-chain-700 (+2.66%). Separate-process peak
RSS changes remain small and do not demonstrate reduced retained-map memory.
The measured result is modest eager Sudoku improvement, a hybrid Sudoku
regression, and largely unchanged broader series timing.

## Reproducing the measurements

Build the three named revisions with `scripts/install.sh`, using a separate
checkout and Cargo target for each. Retain the installed executables outside the
build caches. Use the benchmark runner built from `120fadfb` for every revision,
and clingo 5.8.2. Set `NATIVE` and `CLINGO` below to the absolute executable paths;
choose a fresh report name for each invocation.

The corpus order is original A, prior A, candidate A, candidate B, prior B,
original B. All other selections use prior A, candidate A, candidate B, prior B.
Run each selection sequentially on an otherwise idle host. This is the corpus
invocation for one revision/repeat:

```sh
zetesis-bench run --suite corpus --threads 14 --repetitions 5 \
  --zetesis "$NATIVE" --clingo "$CLINGO" --backend cpu \
  --warmups 1 --memory-runs 2 --timeout-seconds 30 --campaign-seconds 1200 \
  --report corpus-candidate-a.json
```

For the other selections, replace `--suite corpus --threads 14 --repetitions 5`
and the report name with the corresponding arguments:

| Selection | Arguments |
| --- | --- |
| Worker scaling | `--suite scalability --include-einstein --compare-threads 1,2,4,8,14 --repetitions 5` |
| Eager/lazy scalability | `--suite scalability --include-einstein --threads 4 --compare-grounders --repetitions 4` |
| Eager/lazy series | `--suite series --threads 4 --compare-grounders --repetitions 4` |

The corpus has adjacent clingo timing and RSS observations. In the matrices,
clingo supplies qualification only. Their populations overlap: do not add them
together as a count of distinct programs. Exit status 1 retains explicit
non-completions; inspect their typed reasons rather than treating missing samples
as elapsed times. Preserve every report and compare compatible selections with
the maintained reader:

```sh
zetesis-bench compare original-a=corpus-original-a.json \
  baseline-a=corpus-baseline-a.json candidate-a=corpus-candidate-a.json \
  candidate-b=corpus-candidate-b.json baseline-b=corpus-baseline-b.json \
  original-b=corpus-original-b.json --output corpus-comparison.json --markdown
```

Each displayed value is the mean of two report medians. It is not the median of
pooled samples. Paired report values, executable and workload identities, report
hashes and qualified-family hashes are retained in the linked evidence. The
[measurement protocol](measurement-protocols.md) defines the capture and
resource boundaries. These measurements are descriptive; small changes need
not persist on another host or run. They measure CPU execution, not Metal.
