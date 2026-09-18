Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | corpus | corpus/reference |
|---|---:|---:|
| equality-generalized-tsp/01-basic | 5.956 [5.616, 5.976] | 1.339 |
| equality-generalized-tsp/02-larger | 5.675 [5.490, 6.852] | 1.040 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.037 [4.889, 5.723] | 1.146 |
| variant-01/01-basic | 4.753 [4.656, 5.779] | 1.062 |
| variant-01/02-start-equals-end | 5.285 [5.133, 5.885] | 1.189 |
| variant-01/03-zero-cost-detour | 4.649 [4.616, 5.569] | 1.056 |
| variant-01/04-no-path | 4.762 [4.760, 5.191] | 1.075 |
| variant-01/05-multi-path | 4.587 [4.457, 4.794] | 1.037 |
| variant-01/06-layered-dag | 8.703 [8.313, 8.714] | 1.578 |
| variant-01/07-cycles | 5.763 [5.740, 6.374] | 1.287 |
| variant-01/08-negative-weights | 5.860 [5.618, 5.896] | 1.297 |
| variant-02/01-basic | 5.557 [5.481, 5.725] | 1.268 |
| variant-02/02-start-equals-end | 5.828 [5.646, 6.212] | 1.317 |
| variant-02/03-before-forces-detour | 5.062 [5.026, 5.243] | 1.147 |
| variant-02/04-after-forces-extension | 5.218 [4.735, 6.228] | 1.175 |
| variant-02/05-ordering-unsat | 4.882 [4.802, 5.805] | 0.893 |
| variant-02/06-layered-dag-before | 8.839 [7.702, 9.855] | 1.610 |
| variant-02/07-layered-dag-before-after | 8.208 [7.445, 8.238] | 1.480 |
| variant-02/08-tie-break-under-ordering | 5.146 [5.094, 6.295] | 1.157 |
| variant-02/09-negative-weights | 5.580 [5.483, 6.167] | 1.017 |
| variant-03/01-basic | 5.277 [5.190, 5.498] | 1.175 |
| variant-03/02-start-equals-end | 4.799 [4.437, 5.769] | 1.090 |
| variant-03/03-budget-forces-detour | 5.757 [5.532, 6.783] | 1.294 |
| variant-03/04-cost-at-cap-allowed | 5.013 [4.816, 5.797] | 1.139 |
| variant-03/05-budget-unsat | 5.259 [4.966, 6.571] | 1.187 |
| variant-03/06-layered-dag-cap | 12.082 [11.059, 13.983] | 1.838 |
| variant-03/07-layered-dag-tight-cap | 10.239 [9.806, 10.578] | 1.554 |
| variant-03/08-tie-break-under-cap | 5.777 [5.702, 5.980] | 1.038 |
| variant-03/09-negative-weights | 5.584 [4.980, 6.696] | 1.009 |
| variant-04/01-basic | 6.608 [5.258, 7.052] | 1.481 |
| variant-04/02-start-equals-end | 6.084 [5.521, 6.384] | 1.116 |
| variant-04/03-before-forces-detour-within-budget | 5.821 [5.815, 5.930] | 1.316 |
| variant-04/04-ordering-violates-budget | 5.620 [4.741, 6.709] | 1.268 |
| variant-04/05-after-and-budget-interact | 5.932 [5.737, 6.270] | 1.322 |
| variant-04/06-layered-dag-ordering-cap | 10.995 [9.918, 11.041] | 1.678 |
| variant-04/07-layered-dag-combined | 9.917 [9.843, 10.419] | 1.798 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.169 [5.523, 6.690] | 1.392 |
| variant-04/09-negative-weights | 6.364 [5.886, 6.634] | 1.426 |
| variant-01/01-basic | 4.717 [4.623, 5.207] | 1.058 |
| variant-01/02-agent-reuse | 4.579 [3.790, 4.645] | 1.039 |
| variant-01/03-selective-compatibility | 5.543 [4.987, 5.743] | 1.261 |
| variant-01/04-no-compatible-agent-unsat | 4.445 [3.358, 4.475] | 0.991 |
| variant-01/05-larger-mix | 6.026 [5.825, 6.173] | 1.331 |
| variant-02/01-basic | 5.510 [4.874, 5.544] | 1.235 |
| variant-02/02-makespan-tiebreak | 4.677 [4.481, 5.842] | 1.037 |
| variant-02/03-cost-dominates | 5.670 [4.679, 5.983] | 1.290 |
| variant-02/04-no-compatible-agent-unsat | 5.558 [4.416, 5.708] | 1.253 |
| variant-02/05-larger-mix | 9.291 [8.917, 10.801] | 2.090 |
| variant-03/01-basic | 4.945 [4.692, 5.871] | 1.108 |
| variant-03/02-multiple-groups | 5.515 [5.481, 6.673] | 1.245 |
| variant-03/03-mixed-grouped-ungrouped | 5.706 [5.439, 5.948] | 1.286 |
| variant-03/04-incompatible-group-unsat | 4.705 [4.634, 5.629] | 1.069 |
| variant-03/05-larger-mix | 5.779 [5.709, 5.839] | 1.301 |
| variant-04/01-basic | 9.509 [8.140, 10.884] | 0.356 |
| variant-04/02-precedence | 7.800 [7.645, 7.822] | 1.397 |
| variant-04/03-agent-serialization | 8.077 [7.946, 8.822] | 0.280 |
| variant-04/04-window-too-tight-unsat | 6.828 [6.582, 7.757] | 1.503 |
| variant-04/05-larger-mix | 33.735 [33.423, 34.885] | 0.225 |
| variant-01/01-basic | 6.206 [5.582, 6.386] | 1.124 |
| variant-01/02-multiple-tours | 5.720 [5.681, 7.403] | 1.037 |
| variant-01/03-asymmetric | 7.567 [7.335, 7.789] | 1.385 |
| variant-01/04-subtour-unsat | 4.936 [4.737, 5.073] | 1.116 |
| variant-01/05-ring | 8.221 [8.201, 9.730] | 1.492 |
| variant-02/01-basic | 5.349 [4.928, 6.779] | 1.229 |
| variant-02/02-single-salesman | 5.765 [5.531, 6.594] | 1.302 |
| variant-02/03-too-many-salesmen-unsat | 5.367 [4.868, 5.908] | 0.987 |
| variant-02/04-equal-cost-split | 4.981 [4.506, 5.931] | 1.124 |
| variant-02/05-larger-asymmetric | 5.489 [4.851, 5.616] | 1.232 |
| variant-02/06-unreachable-edge | 6.567 [5.683, 6.618] | 1.470 |
| variant-03/01-basic | 5.629 [5.232, 5.971] | 1.280 |
| variant-03/02-single-salesman | 5.533 [5.460, 5.969] | 1.011 |
| variant-03/03-depot-crossing-unsat | 5.111 [4.855, 5.942] | 1.152 |
| variant-03/04-equal-cost-split | 6.198 [5.843, 7.280] | 1.399 |
| variant-03/05-larger-three-depots | 7.223 [6.927, 7.356] | 1.320 |
| variant-03/06-unreachable-edge | 6.320 [5.707, 6.668] | 1.420 |
| variant-04/01-basic | 6.589 [6.186, 6.791] | 1.452 |
| variant-04/02-single-salesman | 6.572 [5.441, 6.728] | 1.183 |
| variant-04/03-window-too-tight-unsat | 6.213 [5.617, 6.562] | 1.144 |
| variant-04/04-depot-window-too-tight-unsat | 6.037 [4.764, 6.271] | 1.357 |
| variant-04/05-three-depots-asymmetric-times | 8.715 [7.798, 9.836] | 1.985 |
| variant-04/06-unreachable-edge | 8.086 [7.142, 8.272] | 1.471 |
| variant-05/01-basic | 6.803 [5.584, 7.205] | 1.532 |
| variant-05/02-tight-bound-exact | 6.878 [5.892, 6.937] | 1.252 |
| variant-05/03-revisit-too-tight-unsat | 5.898 [5.809, 6.290] | 1.080 |
| variant-05/04-depot-and-vertex-revisits | 6.739 [5.833, 7.710] | 1.518 |
| variant-05/05-three-depots-mixed | 8.821 [8.510, 8.903] | 1.936 |
| variant-05/06-unreachable-edge | 7.986 [7.685, 8.279] | 1.443 |
| n-queens/variant-01 | 8.903 [8.178, 10.412] | 1.365 |
| n-queens/variant-02 | 61.485 [61.425, 62.998] | 0.508 |
| n-queens/variant-03 | 9.396 [9.220, 9.503] | 1.422 |
| n-queens/variant-04 | 7.654 [6.991, 7.928] | 1.169 |
| n-queens/variant-05 | 9.140 [8.059, 9.379] | 1.387 |
| n-queens/variant-06 | 9.925 [8.794, 10.196] | 1.794 |
| send-money/send-money | 12.518 [12.268, 14.308] | 0.895 |

Reference wall time, ms, same notation.

| Cell | corpus |
|---|---:|
| equality-generalized-tsp/01-basic | 4.449 [4.406, 5.489] |
| equality-generalized-tsp/02-larger | 5.457 [4.429, 5.544] |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.394 [3.401, 4.445] |
| variant-01/01-basic | 4.476 [4.410, 5.466] |
| variant-01/02-start-equals-end | 4.446 [4.432, 5.500] |
| variant-01/03-zero-cost-detour | 4.404 [3.282, 4.447] |
| variant-01/04-no-path | 4.431 [4.409, 4.441] |
| variant-01/05-multi-path | 4.423 [4.381, 5.444] |
| variant-01/06-layered-dag | 5.516 [5.427, 5.519] |
| variant-01/07-cycles | 4.476 [4.445, 5.658] |
| variant-01/08-negative-weights | 4.518 [4.391, 4.545] |
| variant-02/01-basic | 4.383 [4.373, 5.506] |
| variant-02/02-start-equals-end | 4.424 [4.402, 4.436] |
| variant-02/03-before-forces-detour | 4.414 [4.364, 4.487] |
| variant-02/04-after-forces-extension | 4.442 [4.405, 4.464] |
| variant-02/05-ordering-unsat | 5.467 [3.367, 5.474] |
| variant-02/06-layered-dag-before | 5.489 [5.441, 5.507] |
| variant-02/07-layered-dag-before-after | 5.548 [5.472, 6.575] |
| variant-02/08-tie-break-under-ordering | 4.448 [4.358, 4.493] |
| variant-02/09-negative-weights | 5.488 [4.459, 5.493] |
| variant-03/01-basic | 4.489 [4.450, 5.511] |
| variant-03/02-start-equals-end | 4.404 [4.357, 5.498] |
| variant-03/03-budget-forces-detour | 4.450 [4.429, 4.467] |
| variant-03/04-cost-at-cap-allowed | 4.400 [4.400, 4.440] |
| variant-03/05-budget-unsat | 4.432 [4.402, 5.491] |
| variant-03/06-layered-dag-cap | 6.573 [6.557, 7.649] |
| variant-03/07-layered-dag-tight-cap | 6.589 [6.580, 7.650] |
| variant-03/08-tie-break-under-cap | 5.563 [4.467, 5.639] |
| variant-03/09-negative-weights | 5.535 [5.503, 5.554] |
| variant-04/01-basic | 4.461 [4.367, 5.515] |
| variant-04/02-start-equals-end | 5.451 [4.431, 5.644] |
| variant-04/03-before-forces-detour-within-budget | 4.423 [4.350, 5.444] |
| variant-04/04-ordering-violates-budget | 4.430 [4.423, 4.582] |
| variant-04/05-after-and-budget-interact | 4.488 [4.374, 5.467] |
| variant-04/06-layered-dag-ordering-cap | 6.554 [5.475, 6.592] |
| variant-04/07-layered-dag-combined | 5.517 [5.429, 7.639] |
| variant-04/08-tie-break-under-ordering-and-cap | 4.431 [4.402, 4.438] |
| variant-04/09-negative-weights | 4.462 [4.434, 5.474] |
| variant-01/01-basic | 4.460 [4.382, 4.492] |
| variant-01/02-agent-reuse | 4.407 [3.284, 4.433] |
| variant-01/03-selective-compatibility | 4.394 [4.365, 4.406] |
| variant-01/04-no-compatible-agent-unsat | 4.485 [3.325, 5.499] |
| variant-01/05-larger-mix | 4.529 [4.427, 5.502] |
| variant-02/01-basic | 4.462 [4.395, 4.467] |
| variant-02/02-makespan-tiebreak | 4.511 [4.441, 5.476] |
| variant-02/03-cost-dominates | 4.397 [4.349, 5.498] |
| variant-02/04-no-compatible-agent-unsat | 4.436 [4.390, 4.446] |
| variant-02/05-larger-mix | 4.445 [4.444, 4.451] |
| variant-03/01-basic | 4.463 [4.456, 5.465] |
| variant-03/02-multiple-groups | 4.429 [4.383, 4.445] |
| variant-03/03-mixed-grouped-ungrouped | 4.438 [4.363, 4.446] |
| variant-03/04-incompatible-group-unsat | 4.401 [4.401, 5.601] |
| variant-03/05-larger-mix | 4.442 [4.399, 4.450] |
| variant-04/01-basic | 26.708 [25.715, 26.717] |
| variant-04/02-precedence | 5.583 [5.518, 6.637] |
| variant-04/03-agent-serialization | 28.797 [27.763, 28.840] |
| variant-04/04-window-too-tight-unsat | 4.542 [4.537, 5.557] |
| variant-04/05-larger-mix | 149.983 [149.211, 150.108] |
| variant-01/01-basic | 5.520 [4.417, 5.559] |
| variant-01/02-multiple-tours | 5.517 [4.506, 5.808] |
| variant-01/03-asymmetric | 5.462 [4.700, 5.512] |
| variant-01/04-subtour-unsat | 4.422 [3.246, 5.560] |
| variant-01/05-ring | 5.510 [5.490, 5.515] |
| variant-02/01-basic | 4.351 [3.332, 4.355] |
| variant-02/02-single-salesman | 4.428 [3.320, 5.518] |
| variant-02/03-too-many-salesmen-unsat | 5.438 [4.405, 5.583] |
| variant-02/04-equal-cost-split | 4.430 [4.424, 5.459] |
| variant-02/05-larger-asymmetric | 4.456 [4.414, 5.499] |
| variant-02/06-unreachable-edge | 4.467 [4.445, 4.488] |
| variant-03/01-basic | 4.396 [4.373, 4.401] |
| variant-03/02-single-salesman | 5.472 [4.434, 5.501] |
| variant-03/03-depot-crossing-unsat | 4.438 [4.400, 5.483] |
| variant-03/04-equal-cost-split | 4.432 [3.319, 5.465] |
| variant-03/05-larger-three-depots | 5.471 [4.434, 5.480] |
| variant-03/06-unreachable-edge | 4.451 [4.434, 5.496] |
| variant-04/01-basic | 4.538 [4.406, 5.494] |
| variant-04/02-single-salesman | 5.555 [4.401, 6.549] |
| variant-04/03-window-too-tight-unsat | 5.432 [4.369, 5.540] |
| variant-04/04-depot-window-too-tight-unsat | 4.450 [4.449, 5.459] |
| variant-04/05-three-depots-asymmetric-times | 4.392 [4.349, 5.510] |
| variant-04/06-unreachable-edge | 5.499 [5.464, 5.507] |
| variant-05/01-basic | 4.439 [4.436, 5.505] |
| variant-05/02-tight-bound-exact | 5.493 [5.458, 5.523] |
| variant-05/03-revisit-too-tight-unsat | 5.462 [4.417, 5.473] |
| variant-05/04-depot-and-vertex-revisits | 4.439 [4.355, 5.478] |
| variant-05/05-three-depots-mixed | 4.557 [4.468, 5.521] |
| variant-05/06-unreachable-edge | 5.535 [4.439, 6.585] |
| n-queens/variant-01 | 6.523 [5.481, 6.563] |
| n-queens/variant-02 | 121.127 [119.313, 122.256] |
| n-queens/variant-03 | 6.607 [5.450, 6.683] |
| n-queens/variant-04 | 6.549 [5.484, 6.551] |
| n-queens/variant-05 | 6.592 [5.522, 6.597] |
| n-queens/variant-06 | 5.532 [5.492, 6.742] |
| send-money/send-money | 13.992 [13.985, 15.039] |

Counters of report corpus: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 0 | 1 | 4 | 8982 | 2.009 |
| equality-generalized-tsp/02-larger | 0 | 1 | 8 | 24023 | 2.478 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 0 | 0 | 0 | 2258 | 1.366 |
| variant-01/01-basic | 0 | 1 | 2 | 1559 | 1.443 |
| variant-01/02-start-equals-end | 0 | 1 | 1 | 1208 | 1.404 |
| variant-01/03-zero-cost-detour | 0 | 1 | 2 | 1291 | 1.295 |
| variant-01/04-no-path | 0 | 0 | 0 | 1085 | 1.404 |
| variant-01/05-multi-path | 0 | 1 | 3 | 3958 | 1.522 |
| variant-01/06-layered-dag | 0 | 1 | 17 | 132328 | 5.057 |
| variant-01/07-cycles | 0 | 1 | 5 | 13987 | 2.125 |
| variant-01/08-negative-weights | 0 | 1 | 1 | 2442 | 1.755 |
| variant-02/01-basic | 0 | 1 | 2 | 1700 | 1.922 |
| variant-02/02-start-equals-end | 0 | 1 | 1 | 1557 | 2.117 |
| variant-02/03-before-forces-detour | 0 | 1 | 1 | 2383 | 1.561 |
| variant-02/04-after-forces-extension | 0 | 1 | 1 | 2245 | 1.682 |
| variant-02/05-ordering-unsat | 0 | 0 | 0 | 1322 | 1.413 |
| variant-02/06-layered-dag-before | 0 | 2 | 9 | 48810 | 4.860 |
| variant-02/07-layered-dag-before-after | 0 | 1 | 7 | 42762 | 4.022 |
| variant-02/08-tie-break-under-ordering | 0 | 1 | 2 | 2777 | 1.746 |
| variant-02/09-negative-weights | 0 | 1 | 1 | 2816 | 1.765 |
| variant-03/01-basic | 0 | 1 | 2 | 1667 | 1.614 |
| variant-03/02-start-equals-end | 0 | 1 | 1 | 1467 | 1.446 |
| variant-03/03-budget-forces-detour | 0 | 1 | 1 | 2141 | 1.761 |
| variant-03/04-cost-at-cap-allowed | 0 | 1 | 1 | 1272 | 1.565 |
| variant-03/05-budget-unsat | 0 | 0 | 0 | 4970 | 2.133 |
| variant-03/06-layered-dag-cap | 0 | 2 | 13 | 251813 | 8.134 |
| variant-03/07-layered-dag-tight-cap | 0 | 4 | 6 | 107944 | 6.533 |
| variant-03/08-tie-break-under-cap | 0 | 1 | 2 | 3301 | 1.822 |
| variant-03/09-negative-weights | 0 | 1 | 1 | 2872 | 1.983 |
| variant-04/01-basic | 0 | 1 | 2 | 1808 | 2.124 |
| variant-04/02-start-equals-end | 0 | 1 | 1 | 1816 | 1.894 |
| variant-04/03-before-forces-detour-within-budget | 0 | 1 | 1 | 2774 | 2.177 |
| variant-04/04-ordering-violates-budget | 0 | 0 | 0 | 2816 | 1.855 |
| variant-04/05-after-and-budget-interact | 0 | 1 | 1 | 4169 | 2.222 |
| variant-04/06-layered-dag-ordering-cap | 0 | 2 | 8 | 69872 | 6.256 |
| variant-04/07-layered-dag-combined | 0 | 1 | 5 | 60073 | 5.888 |
| variant-04/08-tie-break-under-ordering-and-cap | 0 | 1 | 2 | 6554 | 2.340 |
| variant-04/09-negative-weights | 0 | 1 | 1 | 3246 | 2.364 |
| variant-01/01-basic | 0 | 1 | 4 | 1462 | 1.147 |
| variant-01/02-agent-reuse | 0 | 1 | 8 | 2131 | 1.218 |
| variant-01/03-selective-compatibility | 0 | 1 | 4 | 1873 | 1.201 |
| variant-01/04-no-compatible-agent-unsat | 0 | 0 | 0 | 1217 | 0.952 |
| variant-01/05-larger-mix | 0 | 1 | 37 | 35597 | 2.093 |
| variant-02/01-basic | 0 | 1 | 4 | 3047 | 1.478 |
| variant-02/02-makespan-tiebreak | 0 | 3 | 8 | 4031 | 1.739 |
| variant-02/03-cost-dominates | 0 | 1 | 8 | 4006 | 1.874 |
| variant-02/04-no-compatible-agent-unsat | 0 | 0 | 0 | 2395 | 1.827 |
| variant-02/05-larger-mix | 0 | 1 | 11 | 180180 | 5.618 |
| variant-03/01-basic | 0 | 2 | 2 | 1970 | 1.564 |
| variant-03/02-multiple-groups | 0 | 4 | 4 | 3816 | 1.666 |
| variant-03/03-mixed-grouped-ungrouped | 0 | 2 | 4 | 2639 | 1.946 |
| variant-03/04-incompatible-group-unsat | 0 | 0 | 0 | 1224 | 1.390 |
| variant-03/05-larger-mix | 0 | 1 | 13 | 10526 | 2.077 |
| variant-04/01-basic | 0 | 81 | 89 | 106796 | 5.968 |
| variant-04/02-precedence | 0 | 3 | 6 | 11981 | 3.844 |
| variant-04/03-agent-serialization | 0 | 30 | 30 | 32994 | 4.399 |
| variant-04/04-window-too-tight-unsat | 0 | 0 | 0 | 2865 | 2.782 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 2135119 | 29.439 |
| variant-01/01-basic | 0 | 1 | 2 | 5138 | 1.889 |
| variant-01/02-multiple-tours | 0 | 2 | 9 | 35544 | 2.437 |
| variant-01/03-asymmetric | 0 | 1 | 9 | 54815 | 3.084 |
| variant-01/04-subtour-unsat | 0 | 0 | 0 | 2473 | 1.241 |
| variant-01/05-ring | 0 | 2 | 12 | 192397 | 4.725 |
| variant-02/01-basic | 0 | 2 | 2 | 5689 | 1.871 |
| variant-02/02-single-salesman | 0 | 1 | 2 | 3644 | 1.811 |
| variant-02/03-too-many-salesmen-unsat | 0 | 0 | 0 | 3466 | 1.595 |
| variant-02/04-equal-cost-split | 0 | 2 | 2 | 4647 | 1.778 |
| variant-02/05-larger-asymmetric | 0 | 2 | 2 | 6731 | 1.929 |
| variant-02/06-unreachable-edge | 0 | 2 | 2 | 7627 | 2.378 |
| variant-03/01-basic | 0 | 1 | 1 | 3939 | 1.804 |
| variant-03/02-single-salesman | 0 | 1 | 2 | 3498 | 1.753 |
| variant-03/03-depot-crossing-unsat | 0 | 0 | 0 | 3422 | 1.563 |
| variant-03/04-equal-cost-split | 0 | 2 | 2 | 7949 | 2.128 |
| variant-03/05-larger-three-depots | 0 | 1 | 1 | 10821 | 3.461 |
| variant-03/06-unreachable-edge | 0 | 1 | 1 | 6365 | 2.264 |
| variant-04/01-basic | 0 | 1 | 1 | 6955 | 2.614 |
| variant-04/02-single-salesman | 0 | 1 | 1 | 4700 | 2.464 |
| variant-04/03-window-too-tight-unsat | 0 | 0 | 0 | 3400 | 2.155 |
| variant-04/04-depot-window-too-tight-unsat | 0 | 0 | 0 | 2325 | 2.452 |
| variant-04/05-three-depots-asymmetric-times | 0 | 1 | 1 | 18328 | 4.795 |
| variant-04/06-unreachable-edge | 0 | 1 | 1 | 14062 | 3.773 |
| variant-05/01-basic | 0 | 1 | 1 | 6957 | 2.835 |
| variant-05/02-tight-bound-exact | 0 | 1 | 1 | 3439 | 2.602 |
| variant-05/03-revisit-too-tight-unsat | 0 | 0 | 0 | 3413 | 2.159 |
| variant-05/04-depot-and-vertex-revisits | 0 | 1 | 1 | 3504 | 2.654 |
| variant-05/05-three-depots-mixed | 0 | 1 | 1 | 18378 | 5.025 |
| variant-05/06-unreachable-edge | 0 | 1 | 1 | 14062 | 3.998 |
| n-queens/variant-01 | 0 | 92 | 92 | 199032 | 5.392 |
| n-queens/variant-02 | 0 | 92 | 92 | 14461401 | 57.346 |
| n-queens/variant-03 | 0 | 92 | 92 | 256834 | 5.299 |
| n-queens/variant-04 | 0 | 92 | 92 | 195309 | 3.957 |
| n-queens/variant-05 | 0 | 92 | 92 | 223549 | 5.220 |
| n-queens/variant-06 | 0 | 92 | 92 | 251111 | 6.012 |
| send-money/send-money | 0 | 1 | 1 | 409024 | 8.756 |

Against the reference: report corpus, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 8 of 94 cells where both passed (8.5%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 33.735 | 149.983 | 0.225 | 3.244 | 32.805 | 12.796 | 85.000 | 58.000 |
| variant-04/03-agent-serialization | 8.077 | 28.797 | 0.280 | 0.935 | 1.492 | 0.314 | 24.000 | 0.000 |
| variant-04/01-basic | 9.509 | 26.708 | 0.356 | 0.993 | 3.687 | 0.689 | 17.000 | 4.000 |
| n-queens/variant-02 | 61.485 | 121.127 | 0.508 | 2.437 | 175.201 | 1.288 | 1.000 | 115.000 |
| variant-02/05-ordering-unsat | 4.882 | 5.467 | 0.893 | 0.159 | 0.039 | 0.005 | 1.000 | 0.000 |
| send-money/send-money | 12.518 | 13.992 | 0.895 | 2.637 | 6.333 | 0.285 | 9.000 | 1.000 |
| variant-02/03-too-many-salesmen-unsat | 5.367 | 5.438 | 0.987 | 0.281 | 0.062 | 0.012 | 1.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 4.445 | 4.485 | 0.991 | 0.115 | 0.030 | 0.006 | 0.000 | 0.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-03/09-negative-weights | 5.584 | 5.535 | 1.009 | 0.311 | 0.092 | 0.055 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 5.533 | 5.472 | 1.011 | 0.253 | 0.091 | 0.083 | 1.000 | 0.000 |
| variant-02/09-negative-weights | 5.580 | 5.488 | 1.017 | 0.245 | 0.086 | 0.049 | 1.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 4.677 | 4.511 | 1.037 | 0.341 | 0.239 | 0.061 | 1.000 | 0.000 |
| variant-01/02-multiple-tours | 5.720 | 5.517 | 1.037 | 0.377 | 0.519 | 1.013 | 1.000 | 0.000 |
| variant-01/05-multi-path | 4.587 | 4.423 | 1.037 | 0.272 | 0.154 | 0.028 | 1.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 5.777 | 5.563 | 1.038 | 0.266 | 0.102 | 0.022 | 1.000 | 0.000 |
| variant-01/02-agent-reuse | 4.579 | 4.407 | 1.039 | 0.149 | 0.117 | 0.032 | 0.000 | 0.000 |
| equality-generalized-tsp/02-larger | 5.675 | 5.457 | 1.040 | 0.576 | 0.490 | 0.484 | 1.000 | 0.000 |
| variant-01/03-zero-cost-detour | 4.649 | 4.404 | 1.056 | 0.151 | 0.061 | 0.012 | 1.000 | 0.000 |
| variant-01/01-basic | 4.717 | 4.460 | 1.058 | 0.134 | 0.080 | 0.019 | 0.000 | 0.000 |
| variant-01/01-basic | 4.753 | 4.476 | 1.062 | 0.166 | 0.057 | 0.012 | 1.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 4.705 | 4.401 | 1.069 | 0.114 | 0.042 | 0.005 | 0.000 | 0.000 |
| variant-01/04-no-path | 4.762 | 4.431 | 1.075 | 0.172 | 0.033 | 0.006 | 1.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 5.898 | 5.462 | 1.080 | 0.350 | 0.073 | 0.012 | 1.000 | 0.000 |
| variant-03/02-start-equals-end | 4.799 | 4.404 | 1.090 | 0.162 | 0.045 | 0.028 | 1.000 | 0.000 |
| variant-03/01-basic | 4.945 | 4.463 | 1.108 | 0.161 | 0.068 | 0.015 | 0.000 | 0.000 |
| variant-01/04-subtour-unsat | 4.936 | 4.422 | 1.116 | 0.178 | 0.038 | 0.012 | 1.000 | 0.000 |
| variant-04/02-start-equals-end | 6.084 | 5.451 | 1.116 | 0.221 | 0.050 | 0.034 | 1.000 | 0.000 |
| variant-02/04-equal-cost-split | 4.981 | 4.430 | 1.124 | 0.311 | 0.123 | 0.106 | 1.000 | 0.000 |
| variant-01/01-basic | 6.206 | 5.520 | 1.124 | 0.266 | 0.156 | 0.123 | 1.000 | 0.000 |
| variant-03/04-cost-at-cap-allowed | 5.013 | 4.400 | 1.139 | 0.179 | 0.044 | 0.008 | 1.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 6.213 | 5.432 | 1.144 | 0.348 | 0.069 | 0.012 | 1.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.037 | 4.394 | 1.146 | 0.179 | 0.047 | 0.010 | 0.000 | 0.000 |
| variant-02/03-before-forces-detour | 5.062 | 4.414 | 1.147 | 0.215 | 0.064 | 0.012 | 1.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 5.111 | 4.438 | 1.152 | 0.288 | 0.063 | 0.012 | 1.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 5.146 | 4.448 | 1.157 | 0.253 | 0.099 | 0.021 | 1.000 | 0.000 |
| n-queens/variant-04 | 7.654 | 6.549 | 1.169 | 0.719 | 5.146 | 0.730 | 1.000 | 1.000 |
| variant-02/04-after-forces-extension | 5.218 | 4.442 | 1.175 | 0.231 | 0.062 | 0.013 | 1.000 | 0.000 |
| variant-03/01-basic | 5.277 | 4.489 | 1.175 | 0.208 | 0.068 | 0.013 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 6.572 | 5.555 | 1.183 | 0.461 | 0.086 | 0.062 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 5.259 | 4.432 | 1.187 | 0.369 | 0.132 | 0.014 | 1.000 | 0.000 |
| variant-01/02-start-equals-end | 5.285 | 4.446 | 1.189 | 0.156 | 0.043 | 0.027 | 1.000 | 0.000 |
| variant-02/01-basic | 5.349 | 4.351 | 1.229 | 0.353 | 0.144 | 0.121 | 1.000 | 0.000 |
| variant-02/05-larger-asymmetric | 5.489 | 4.456 | 1.232 | 0.440 | 0.151 | 0.141 | 1.000 | 0.000 |
| variant-02/01-basic | 5.510 | 4.462 | 1.235 | 0.279 | 0.159 | 0.032 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 5.515 | 4.429 | 1.245 | 0.213 | 0.121 | 0.032 | 1.000 | 0.000 |
| variant-05/02-tight-bound-exact | 6.878 | 5.493 | 1.252 | 0.390 | 0.074 | 0.051 | 1.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 5.558 | 4.436 | 1.253 | 0.279 | 0.039 | 0.008 | 0.000 | 0.000 |
| variant-01/03-selective-compatibility | 5.543 | 4.394 | 1.261 | 0.176 | 0.073 | 0.019 | 0.000 | 0.000 |
| variant-02/01-basic | 5.557 | 4.383 | 1.268 | 0.166 | 0.072 | 0.018 | 1.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 5.620 | 4.430 | 1.268 | 0.298 | 0.070 | 0.010 | 1.000 | 0.000 |
| variant-03/01-basic | 5.629 | 4.396 | 1.280 | 0.309 | 0.073 | 0.050 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 5.706 | 4.438 | 1.286 | 0.196 | 0.103 | 0.025 | 1.000 | 0.000 |
| variant-01/07-cycles | 5.763 | 4.476 | 1.287 | 0.368 | 0.283 | 0.377 | 1.000 | 0.000 |
| variant-02/03-cost-dominates | 5.670 | 4.397 | 1.290 | 0.321 | 0.232 | 0.058 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 5.757 | 4.450 | 1.294 | 0.226 | 0.063 | 0.013 | 1.000 | 0.000 |
| variant-01/08-negative-weights | 5.860 | 4.518 | 1.297 | 0.245 | 0.076 | 0.043 | 1.000 | 0.000 |
| variant-03/05-larger-mix | 5.779 | 4.442 | 1.301 | 0.285 | 0.467 | 0.093 | 1.000 | 0.000 |
| variant-02/02-single-salesman | 5.765 | 4.428 | 1.302 | 0.252 | 0.095 | 0.092 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 5.821 | 4.423 | 1.316 | 0.256 | 0.069 | 0.014 | 1.000 | 0.000 |
| variant-02/02-start-equals-end | 5.828 | 4.424 | 1.317 | 0.371 | 0.053 | 0.030 | 1.000 | 0.000 |
| variant-03/05-larger-three-depots | 7.223 | 5.471 | 1.320 | 0.817 | 0.171 | 0.114 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 5.932 | 4.488 | 1.322 | 0.307 | 0.094 | 0.019 | 1.000 | 0.000 |
| variant-01/05-larger-mix | 6.026 | 4.529 | 1.331 | 0.254 | 1.589 | 0.149 | 0.000 | 0.000 |
| equality-generalized-tsp/01-basic | 5.956 | 4.449 | 1.339 | 0.375 | 0.198 | 0.238 | 1.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 6.037 | 4.450 | 1.357 | 0.294 | 0.057 | 0.011 | 1.000 | 0.000 |
| n-queens/variant-01 | 8.903 | 6.523 | 1.365 | 1.845 | 5.242 | 1.000 | 1.000 | 1.000 |
| variant-01/03-asymmetric | 7.567 | 5.462 | 1.385 | 0.543 | 0.776 | 0.796 | 1.000 | 0.000 |
| n-queens/variant-05 | 9.140 | 6.592 | 1.387 | 1.009 | 5.563 | 0.945 | 1.000 | 1.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.169 | 4.431 | 1.392 | 0.437 | 0.185 | 0.039 | 1.000 | 0.000 |
| variant-04/02-precedence | 7.800 | 5.583 | 1.397 | 0.617 | 0.759 | 0.096 | 2.000 | 0.000 |
| variant-03/04-equal-cost-split | 6.198 | 4.432 | 1.399 | 0.460 | 0.168 | 0.157 | 1.000 | 0.000 |
| variant-03/06-unreachable-edge | 6.320 | 4.451 | 1.420 | 0.524 | 0.121 | 0.084 | 1.000 | 0.000 |
| n-queens/variant-03 | 9.396 | 6.607 | 1.422 | 1.837 | 5.264 | 0.948 | 1.000 | 1.000 |
| variant-04/09-negative-weights | 6.364 | 4.462 | 1.426 | 0.321 | 0.089 | 0.055 | 1.000 | 0.000 |
| variant-05/06-unreachable-edge | 7.986 | 5.535 | 1.443 | 1.195 | 0.210 | 0.175 | 1.000 | 0.000 |
| variant-04/01-basic | 6.589 | 4.538 | 1.452 | 0.601 | 0.128 | 0.082 | 1.000 | 0.000 |
| variant-02/06-unreachable-edge | 6.567 | 4.467 | 1.470 | 0.489 | 0.171 | 0.168 | 1.000 | 0.000 |
| variant-04/06-unreachable-edge | 8.086 | 5.499 | 1.471 | 1.173 | 0.206 | 0.176 | 1.000 | 0.000 |
| variant-02/07-layered-dag-before-after | 8.208 | 5.548 | 1.480 | 1.110 | 1.603 | 0.242 | 2.000 | 0.000 |
| variant-04/01-basic | 6.608 | 4.461 | 1.481 | 0.204 | 0.075 | 0.016 | 1.000 | 0.000 |
| variant-01/05-ring | 8.221 | 5.510 | 1.492 | 0.665 | 2.521 | 1.620 | 1.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 6.828 | 4.542 | 1.503 | 0.312 | 0.085 | 0.010 | 1.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 6.739 | 4.439 | 1.518 | 0.385 | 0.079 | 0.062 | 1.000 | 0.000 |
| variant-05/01-basic | 6.803 | 4.439 | 1.532 | 0.629 | 0.085 | 0.051 | 1.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 10.239 | 6.589 | 1.554 | 1.704 | 2.971 | 0.333 | 2.000 | 1.000 |
| variant-01/06-layered-dag | 8.703 | 5.516 | 1.578 | 1.009 | 3.306 | 0.341 | 2.000 | 0.000 |
| variant-02/06-layered-dag-before | 8.839 | 5.489 | 1.610 | 1.098 | 1.495 | 0.261 | 2.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 10.995 | 6.554 | 1.678 | 1.795 | 2.434 | 0.409 | 2.000 | 0.000 |
| n-queens/variant-06 | 9.925 | 5.532 | 1.794 | 1.104 | 6.002 | 1.094 | 1.000 | 1.000 |
| variant-04/07-layered-dag-combined | 9.917 | 5.517 | 1.798 | 1.832 | 2.259 | 0.373 | 2.000 | 0.000 |
| variant-03/06-layered-dag-cap | 12.082 | 6.573 | 1.838 | 1.659 | 6.129 | 0.494 | 2.000 | 0.000 |
| variant-05/05-three-depots-mixed | 8.821 | 4.557 | 1.936 | 1.537 | 0.253 | 0.202 | 1.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 8.715 | 4.392 | 1.985 | 1.541 | 0.249 | 0.193 | 1.000 | 0.000 |
| variant-02/05-larger-mix | 9.291 | 4.445 | 2.090 | 0.982 | 4.968 | 0.162 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 16.0 | 23.7 | n/a |
| variant-04/03-agent-serialization | 11.6 | 12.3 | n/a |
| variant-04/01-basic | 12.2 | 13.1 | n/a |
| n-queens/variant-02 | 16.6 | 12.8 | n/a |
| variant-02/05-ordering-unsat | 10.6 | 10.2 | n/a |
| send-money/send-money | 14.2 | 12.8 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.5 | 10.5 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.1 | 10.2 | n/a |
| variant-03/09-negative-weights | 10.5 | 10.5 | n/a |
| variant-03/02-single-salesman | 10.8 | 10.4 | n/a |
| variant-02/09-negative-weights | 10.9 | 10.3 | n/a |
| variant-02/02-makespan-tiebreak | 10.8 | 10.4 | n/a |
| variant-01/02-multiple-tours | 11.6 | 10.5 | n/a |
| variant-01/05-multi-path | 10.7 | 10.6 | n/a |
| variant-03/08-tie-break-under-cap | 10.9 | 10.5 | n/a |
| variant-01/02-agent-reuse | 10.6 | 10.4 | n/a |
| equality-generalized-tsp/02-larger | 11.2 | 10.5 | n/a |
| variant-01/03-zero-cost-detour | 10.4 | 10.4 | n/a |
| variant-01/01-basic | 10.3 | 10.5 | n/a |
| variant-01/01-basic | 10.6 | 10.5 | n/a |
| variant-03/04-incompatible-group-unsat | 10.7 | 10.4 | n/a |
| variant-01/04-no-path | 10.4 | 10.5 | n/a |
| variant-05/03-revisit-too-tight-unsat | 11.0 | 10.5 | n/a |
| variant-03/02-start-equals-end | 10.8 | 10.4 | n/a |
| variant-03/01-basic | 10.6 | 10.5 | n/a |
| variant-01/04-subtour-unsat | 10.5 | 10.4 | n/a |
| variant-04/02-start-equals-end | 11.0 | 10.6 | n/a |
| variant-02/04-equal-cost-split | 10.8 | 10.5 | n/a |
| variant-01/01-basic | 10.7 | 10.6 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.7 | 10.4 | n/a |
| variant-04/03-window-too-tight-unsat | 10.9 | 10.6 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.6 | 10.2 | n/a |
| variant-02/03-before-forces-detour | 10.8 | 10.5 | n/a |
| variant-03/03-depot-crossing-unsat | 10.6 | 10.5 | n/a |
| variant-02/08-tie-break-under-ordering | 10.9 | 10.4 | n/a |
| n-queens/variant-04 | 11.2 | 10.5 | n/a |
| variant-02/04-after-forces-extension | 10.9 | 10.6 | n/a |
| variant-03/01-basic | 10.8 | 10.4 | n/a |
| variant-04/02-single-salesman | 11.1 | 10.5 | n/a |
| variant-03/05-budget-unsat | 10.4 | 10.5 | n/a |
| variant-01/02-start-equals-end | 10.7 | 10.4 | n/a |
| variant-02/01-basic | 10.9 | 10.1 | n/a |
| variant-02/05-larger-asymmetric | 10.6 | 10.5 | n/a |
| variant-02/01-basic | 10.7 | 10.3 | n/a |
| variant-03/02-multiple-groups | 10.7 | 10.4 | n/a |
| variant-05/02-tight-bound-exact | 11.2 | 10.5 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.6 | 10.4 | n/a |
| variant-01/03-selective-compatibility | 10.5 | 10.3 | n/a |
| variant-02/01-basic | 10.6 | 10.6 | n/a |
| variant-04/04-ordering-violates-budget | 10.9 | 10.4 | n/a |
| variant-03/01-basic | 10.9 | 10.3 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.8 | 10.4 | n/a |
| variant-01/07-cycles | 10.7 | 10.6 | n/a |
| variant-02/03-cost-dominates | 10.8 | 10.5 | n/a |
| variant-03/03-budget-forces-detour | 10.6 | 10.6 | n/a |
| variant-01/08-negative-weights | 10.9 | 10.3 | n/a |
| variant-03/05-larger-mix | 10.7 | 10.5 | n/a |
| variant-02/02-single-salesman | 10.9 | 10.3 | n/a |
| variant-04/03-before-forces-detour-within-budget | 10.8 | 10.4 | n/a |
| variant-02/02-start-equals-end | 10.8 | 10.4 | n/a |
| variant-03/05-larger-three-depots | 11.1 | 10.6 | n/a |
| variant-04/05-after-and-budget-interact | 10.6 | 10.4 | n/a |
| variant-01/05-larger-mix | 10.6 | 10.5 | n/a |
| equality-generalized-tsp/01-basic | 10.7 | 10.6 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 11.1 | 10.4 | n/a |
| n-queens/variant-01 | 11.5 | 10.4 | n/a |
| variant-01/03-asymmetric | 11.7 | 10.6 | n/a |
| n-queens/variant-05 | 11.7 | 10.5 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 10.7 | 10.7 | n/a |
| variant-04/02-precedence | 11.5 | 10.6 | n/a |
| variant-03/04-equal-cost-split | 10.8 | 10.5 | n/a |
| variant-03/06-unreachable-edge | 10.8 | 10.6 | n/a |
| n-queens/variant-03 | 11.6 | 10.4 | n/a |
| variant-04/09-negative-weights | 11.0 | 10.6 | n/a |
| variant-05/06-unreachable-edge | 11.5 | 10.6 | n/a |
| variant-04/01-basic | 11.0 | 10.5 | n/a |
| variant-02/06-unreachable-edge | 10.7 | 10.5 | n/a |
| variant-04/06-unreachable-edge | 11.2 | 10.4 | n/a |
| variant-02/07-layered-dag-before-after | 11.5 | 10.6 | n/a |
| variant-04/01-basic | 10.9 | 10.6 | n/a |
| variant-01/05-ring | 13.6 | 10.7 | n/a |
| variant-04/04-window-too-tight-unsat | 11.3 | 10.4 | n/a |
| variant-05/04-depot-and-vertex-revisits | 11.1 | 10.5 | n/a |
| variant-05/01-basic | 11.1 | 10.5 | n/a |
| variant-03/07-layered-dag-tight-cap | 12.8 | 11.0 | n/a |
| variant-01/06-layered-dag | 11.3 | 10.9 | n/a |
| variant-02/06-layered-dag-before | 11.8 | 10.7 | n/a |
| variant-04/06-layered-dag-ordering-cap | 13.3 | 10.9 | n/a |
| n-queens/variant-06 | 11.7 | 10.4 | n/a |
| variant-04/07-layered-dag-combined | 12.6 | 10.7 | n/a |
| variant-03/06-layered-dag-cap | 12.9 | 10.8 | n/a |
| variant-05/05-three-depots-mixed | 11.5 | 10.5 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.5 | 10.7 | n/a |
| variant-02/05-larger-mix | 11.7 | 10.5 | n/a |
