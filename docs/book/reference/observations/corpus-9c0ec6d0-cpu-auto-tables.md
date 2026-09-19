Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | corpus | corpus/reference |
|---|---:|---:|
| equality-generalized-tsp/01-basic | 5.733 [5.672, 6.097] | 1.287 |
| equality-generalized-tsp/02-larger | 6.970 [6.086, 7.147] | 1.573 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.176 [4.941, 5.556] | 1.166 |
| variant-01/01-basic | 5.679 [5.212, 6.046] | 1.287 |
| variant-01/02-start-equals-end | 5.022 [4.860, 6.067] | 1.511 |
| variant-01/03-zero-cost-detour | 4.926 [4.707, 5.487] | 1.119 |
| variant-01/04-no-path | 5.543 [4.640, 5.626] | 1.242 |
| variant-01/05-multi-path | 5.109 [4.634, 5.506] | 0.920 |
| variant-01/06-layered-dag | 9.941 [7.771, 10.120] | 1.823 |
| variant-01/07-cycles | 6.071 [6.000, 6.238] | 1.108 |
| variant-01/08-negative-weights | 5.612 [5.045, 5.952] | 1.022 |
| variant-02/01-basic | 5.542 [4.694, 6.051] | 1.247 |
| variant-02/02-start-equals-end | 5.704 [5.546, 6.857] | 1.282 |
| variant-02/03-before-forces-detour | 5.584 [4.725, 5.678] | 1.252 |
| variant-02/04-after-forces-extension | 5.652 [4.680, 6.586] | 1.288 |
| variant-02/05-ordering-unsat | 5.867 [5.557, 6.003] | 1.336 |
| variant-02/06-layered-dag-before | 7.728 [7.644, 8.424] | 1.182 |
| variant-02/07-layered-dag-before-after | 7.761 [7.612, 8.805] | 1.404 |
| variant-02/08-tie-break-under-ordering | 5.747 [5.546, 5.771] | 1.291 |
| variant-02/09-negative-weights | 5.564 [4.856, 5.967] | 1.261 |
| variant-03/01-basic | 5.813 [5.647, 6.259] | 1.300 |
| variant-03/02-start-equals-end | 5.807 [5.532, 6.901] | 1.317 |
| variant-03/03-budget-forces-detour | 5.936 [5.542, 6.634] | 1.334 |
| variant-03/04-cost-at-cap-allowed | 5.465 [5.004, 5.623] | 1.234 |
| variant-03/05-budget-unsat | 5.871 [5.731, 6.583] | 1.070 |
| variant-03/06-layered-dag-cap | 10.392 [10.006, 13.046] | 1.591 |
| variant-03/07-layered-dag-tight-cap | 14.033 [10.943, 14.107] | 2.135 |
| variant-03/08-tie-break-under-cap | 5.891 [5.575, 6.624] | 0.898 |
| variant-03/09-negative-weights | 5.613 [5.526, 6.771] | 1.261 |
| variant-04/01-basic | 6.205 [5.498, 6.600] | 1.404 |
| variant-04/02-start-equals-end | 5.804 [5.746, 6.425] | 1.301 |
| variant-04/03-before-forces-detour-within-budget | 6.616 [5.732, 6.838] | 1.210 |
| variant-04/04-ordering-violates-budget | 5.882 [4.698, 6.580] | 1.338 |
| variant-04/05-after-and-budget-interact | 5.921 [4.870, 6.195] | 1.356 |
| variant-04/06-layered-dag-ordering-cap | 10.832 [9.858, 11.899] | 1.646 |
| variant-04/07-layered-dag-combined | 11.648 [10.088, 13.231] | 1.523 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.812 [6.194, 6.907] | 1.536 |
| variant-04/09-negative-weights | 5.914 [5.901, 6.322] | 1.345 |
| variant-01/01-basic | 5.003 [3.932, 5.094] | 1.137 |
| variant-01/02-agent-reuse | 4.769 [4.360, 4.839] | 0.874 |
| variant-01/03-selective-compatibility | 5.177 [5.140, 5.727] | 1.164 |
| variant-01/04-no-compatible-agent-unsat | 4.552 [3.988, 4.609] | 1.035 |
| variant-01/05-larger-mix | 5.512 [4.644, 6.884] | 1.246 |
| variant-02/01-basic | 5.138 [4.810, 5.738] | 1.174 |
| variant-02/02-makespan-tiebreak | 6.221 [5.269, 6.514] | 1.407 |
| variant-02/03-cost-dominates | 6.093 [4.930, 6.192] | 0.922 |
| variant-02/04-no-compatible-agent-unsat | 4.676 [4.529, 5.208] | 1.043 |
| variant-02/05-larger-mix | 10.004 [9.783, 11.915] | 1.820 |
| variant-03/01-basic | 5.770 [4.597, 5.785] | 1.305 |
| variant-03/02-multiple-groups | 5.186 [4.813, 5.665] | 1.180 |
| variant-03/03-mixed-grouped-ungrouped | 5.317 [5.021, 5.707] | 1.157 |
| variant-03/04-incompatible-group-unsat | 5.540 [5.122, 5.955] | 1.257 |
| variant-03/05-larger-mix | 5.518 [4.931, 7.337] | 1.644 |
| variant-04/01-basic | 9.006 [8.805, 9.471] | 0.350 |
| variant-04/02-precedence | 8.177 [7.965, 9.516] | 1.246 |
| variant-04/03-agent-serialization | 8.452 [8.000, 8.734] | 0.304 |
| variant-04/04-window-too-tight-unsat | 6.574 [6.354, 7.252] | 1.479 |
| variant-04/05-larger-mix | 33.441 [33.353, 33.939] | 0.227 |
| variant-01/01-basic | 5.856 [5.471, 6.427] | 1.271 |
| variant-01/02-multiple-tours | 7.243 [5.663, 7.803] | 1.306 |
| variant-01/03-asymmetric | 6.544 [5.728, 8.541] | 1.456 |
| variant-01/04-subtour-unsat | 4.973 [4.454, 5.429] | 1.119 |
| variant-01/05-ring | 10.091 [8.759, 10.634] | 1.552 |
| variant-02/01-basic | 5.919 [5.724, 6.305] | 1.091 |
| variant-02/02-single-salesman | 5.367 [4.824, 5.532] | 1.211 |
| variant-02/03-too-many-salesmen-unsat | 5.126 [4.514, 5.145] | 1.156 |
| variant-02/04-equal-cost-split | 5.845 [5.573, 6.178] | 1.329 |
| variant-02/05-larger-asymmetric | 5.866 [5.683, 6.125] | 1.071 |
| variant-02/06-unreachable-edge | 6.460 [4.654, 7.279] | 1.424 |
| variant-03/01-basic | 6.698 [5.734, 7.171] | 1.226 |
| variant-03/02-single-salesman | 5.751 [5.551, 6.447] | 1.046 |
| variant-03/03-depot-crossing-unsat | 4.820 [4.767, 5.510] | 1.086 |
| variant-03/04-equal-cost-split | 7.171 [6.624, 7.208] | 1.613 |
| variant-03/05-larger-three-depots | 7.041 [5.827, 9.177] | 1.571 |
| variant-03/06-unreachable-edge | 5.924 [5.737, 7.145] | 1.328 |
| variant-04/01-basic | 6.722 [6.612, 7.212] | 1.222 |
| variant-04/02-single-salesman | 6.897 [6.261, 7.192] | 1.559 |
| variant-04/03-window-too-tight-unsat | 6.568 [5.568, 6.613] | 1.195 |
| variant-04/04-depot-window-too-tight-unsat | 5.086 [4.906, 6.567] | 0.931 |
| variant-04/05-three-depots-asymmetric-times | 8.069 [7.906, 8.189] | 1.436 |
| variant-04/06-unreachable-edge | 7.898 [7.660, 8.211] | 1.437 |
| variant-05/01-basic | 6.798 [5.804, 6.804] | 1.235 |
| variant-05/02-tight-bound-exact | 6.588 [5.791, 7.138] | 1.215 |
| variant-05/03-revisit-too-tight-unsat | 5.938 [5.678, 5.986] | 1.088 |
| variant-05/04-depot-and-vertex-revisits | 6.285 [4.687, 7.016] | 1.417 |
| variant-05/05-three-depots-mixed | 8.690 [8.340, 9.207] | 1.955 |
| variant-05/06-unreachable-edge | 7.718 [7.645, 9.171] | 1.403 |
| n-queens/variant-01 | 8.715 [8.210, 8.988] | 1.552 |
| n-queens/variant-02 | 61.878 [60.174, 64.560] | 0.514 |
| n-queens/variant-03 | 9.236 [9.078, 9.393] | 1.406 |
| n-queens/variant-04 | 7.336 [7.213, 7.961] | 1.123 |
| n-queens/variant-05 | 9.086 [8.398, 9.221] | 1.638 |
| n-queens/variant-06 | 8.822 [8.763, 10.801] | 1.341 |
| send-money/send-money | 14.169 [13.340, 14.224] | 1.011 |

Reference wall time, ms, same notation.

| Cell | corpus |
|---|---:|
| equality-generalized-tsp/01-basic | 4.455 [4.417, 5.584] |
| equality-generalized-tsp/02-larger | 4.430 [4.399, 4.489] |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.439 [4.394, 5.474] |
| variant-01/01-basic | 4.411 [4.343, 5.457] |
| variant-01/02-start-equals-end | 3.323 [3.289, 4.424] |
| variant-01/03-zero-cost-detour | 4.403 [4.397, 5.560] |
| variant-01/04-no-path | 4.462 [4.402, 5.488] |
| variant-01/05-multi-path | 5.554 [4.434, 6.636] |
| variant-01/06-layered-dag | 5.454 [4.530, 5.465] |
| variant-01/07-cycles | 5.479 [4.477, 5.509] |
| variant-01/08-negative-weights | 5.493 [4.460, 5.520] |
| variant-02/01-basic | 4.446 [4.371, 5.460] |
| variant-02/02-start-equals-end | 4.449 [3.326, 4.491] |
| variant-02/03-before-forces-detour | 4.461 [4.406, 4.489] |
| variant-02/04-after-forces-extension | 4.388 [3.318, 4.445] |
| variant-02/05-ordering-unsat | 4.392 [3.300, 4.437] |
| variant-02/06-layered-dag-before | 6.539 [5.480, 6.579] |
| variant-02/07-layered-dag-before-after | 5.529 [5.507, 6.561] |
| variant-02/08-tie-break-under-ordering | 4.453 [4.424, 6.641] |
| variant-02/09-negative-weights | 4.411 [4.405, 4.439] |
| variant-03/01-basic | 4.471 [4.401, 4.502] |
| variant-03/02-start-equals-end | 4.408 [4.392, 4.626] |
| variant-03/03-budget-forces-detour | 4.450 [4.394, 5.456] |
| variant-03/04-cost-at-cap-allowed | 4.429 [4.429, 5.461] |
| variant-03/05-budget-unsat | 5.487 [4.435, 5.499] |
| variant-03/06-layered-dag-cap | 6.533 [6.518, 6.581] |
| variant-03/07-layered-dag-tight-cap | 6.574 [6.553, 7.667] |
| variant-03/08-tie-break-under-cap | 6.564 [4.365, 6.650] |
| variant-03/09-negative-weights | 4.450 [4.393, 4.452] |
| variant-04/01-basic | 4.421 [4.396, 5.471] |
| variant-04/02-start-equals-end | 4.461 [4.387, 4.473] |
| variant-04/03-before-forces-detour-within-budget | 5.467 [4.439, 5.505] |
| variant-04/04-ordering-violates-budget | 4.396 [4.390, 4.404] |
| variant-04/05-after-and-budget-interact | 4.367 [4.350, 4.455] |
| variant-04/06-layered-dag-ordering-cap | 6.579 [6.571, 6.583] |
| variant-04/07-layered-dag-combined | 7.647 [6.489, 7.708] |
| variant-04/08-tie-break-under-ordering-and-cap | 4.435 [4.359, 5.488] |
| variant-04/09-negative-weights | 4.396 [4.342, 5.452] |
| variant-01/01-basic | 4.400 [3.285, 4.432] |
| variant-01/02-agent-reuse | 5.459 [4.474, 5.502] |
| variant-01/03-selective-compatibility | 4.450 [4.413, 4.497] |
| variant-01/04-no-compatible-agent-unsat | 4.400 [4.387, 4.438] |
| variant-01/05-larger-mix | 4.422 [3.288, 5.519] |
| variant-02/01-basic | 4.378 [4.373, 4.425] |
| variant-02/02-makespan-tiebreak | 4.421 [4.410, 4.518] |
| variant-02/03-cost-dominates | 6.607 [4.391, 7.705] |
| variant-02/04-no-compatible-agent-unsat | 4.483 [3.320, 5.526] |
| variant-02/05-larger-mix | 5.496 [5.461, 5.517] |
| variant-03/01-basic | 4.423 [4.383, 4.443] |
| variant-03/02-multiple-groups | 4.396 [3.295, 4.402] |
| variant-03/03-mixed-grouped-ungrouped | 4.595 [4.441, 5.458] |
| variant-03/04-incompatible-group-unsat | 4.409 [3.338, 4.432] |
| variant-03/05-larger-mix | 3.356 [3.286, 4.442] |
| variant-04/01-basic | 25.705 [25.625, 27.780] |
| variant-04/02-precedence | 6.560 [5.536, 6.595] |
| variant-04/03-agent-serialization | 27.802 [27.685, 28.029] |
| variant-04/04-window-too-tight-unsat | 4.445 [4.398, 5.551] |
| variant-04/05-larger-mix | 147.602 [147.320, 149.661] |
| variant-01/01-basic | 4.607 [4.566, 5.558] |
| variant-01/02-multiple-tours | 5.546 [4.657, 5.736] |
| variant-01/03-asymmetric | 4.493 [4.485, 5.495] |
| variant-01/04-subtour-unsat | 4.444 [4.428, 5.489] |
| variant-01/05-ring | 6.500 [4.479, 6.823] |
| variant-02/01-basic | 5.425 [4.395, 5.513] |
| variant-02/02-single-salesman | 4.433 [4.426, 5.450] |
| variant-02/03-too-many-salesmen-unsat | 4.433 [4.370, 5.476] |
| variant-02/04-equal-cost-split | 4.399 [4.399, 4.419] |
| variant-02/05-larger-asymmetric | 5.477 [4.440, 5.488] |
| variant-02/06-unreachable-edge | 4.535 [4.354, 5.503] |
| variant-03/01-basic | 5.461 [4.480, 5.463] |
| variant-03/02-single-salesman | 5.499 [4.430, 5.532] |
| variant-03/03-depot-crossing-unsat | 4.440 [4.314, 4.457] |
| variant-03/04-equal-cost-split | 4.447 [4.369, 5.476] |
| variant-03/05-larger-three-depots | 4.483 [4.391, 5.504] |
| variant-03/06-unreachable-edge | 4.461 [4.437, 5.520] |
| variant-04/01-basic | 5.500 [5.468, 5.576] |
| variant-04/02-single-salesman | 4.425 [4.406, 4.426] |
| variant-04/03-window-too-tight-unsat | 5.498 [4.436, 5.514] |
| variant-04/04-depot-window-too-tight-unsat | 5.464 [3.380, 5.486] |
| variant-04/05-three-depots-asymmetric-times | 5.621 [4.426, 6.521] |
| variant-04/06-unreachable-edge | 5.498 [5.457, 5.565] |
| variant-05/01-basic | 5.505 [5.478, 5.536] |
| variant-05/02-tight-bound-exact | 5.424 [4.435, 5.493] |
| variant-05/03-revisit-too-tight-unsat | 5.459 [4.459, 5.532] |
| variant-05/04-depot-and-vertex-revisits | 4.436 [4.349, 5.480] |
| variant-05/05-three-depots-mixed | 4.444 [4.384, 5.485] |
| variant-05/06-unreachable-edge | 5.499 [4.421, 5.515] |
| n-queens/variant-01 | 5.617 [5.534, 6.552] |
| n-queens/variant-02 | 120.317 [120.285, 121.212] |
| n-queens/variant-03 | 6.568 [6.540, 6.746] |
| n-queens/variant-04 | 6.535 [5.491, 6.694] |
| n-queens/variant-05 | 5.547 [5.521, 6.563] |
| n-queens/variant-06 | 6.579 [6.529, 7.665] |
| send-money/send-money | 14.013 [13.918, 16.138] |

Counters of report corpus: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 0 | 1 | 4 | 8982 | 2.177 |
| equality-generalized-tsp/02-larger | 0 | 1 | 8 | 22756 | 2.571 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 0 | 0 | 0 | 2258 | 1.439 |
| variant-01/01-basic | 0 | 1 | 2 | 1559 | 1.515 |
| variant-01/02-start-equals-end | 0 | 1 | 1 | 1208 | 1.788 |
| variant-01/03-zero-cost-detour | 0 | 1 | 2 | 1291 | 1.363 |
| variant-01/04-no-path | 0 | 0 | 0 | 1085 | 1.506 |
| variant-01/05-multi-path | 0 | 1 | 3 | 3958 | 1.636 |
| variant-01/06-layered-dag | 0 | 1 | 18 | 128999 | 4.973 |
| variant-01/07-cycles | 0 | 1 | 5 | 13987 | 2.313 |
| variant-01/08-negative-weights | 0 | 1 | 1 | 2442 | 1.815 |
| variant-02/01-basic | 0 | 1 | 2 | 1700 | 1.723 |
| variant-02/02-start-equals-end | 0 | 1 | 1 | 1557 | 2.303 |
| variant-02/03-before-forces-detour | 0 | 1 | 1 | 2383 | 1.627 |
| variant-02/04-after-forces-extension | 0 | 1 | 1 | 2245 | 2.058 |
| variant-02/05-ordering-unsat | 0 | 0 | 0 | 1322 | 1.661 |
| variant-02/06-layered-dag-before | 0 | 2 | 9 | 48810 | 4.166 |
| variant-02/07-layered-dag-before-after | 0 | 1 | 7 | 42762 | 3.957 |
| variant-02/08-tie-break-under-ordering | 0 | 1 | 2 | 2777 | 1.786 |
| variant-02/09-negative-weights | 0 | 1 | 1 | 2816 | 1.883 |
| variant-03/01-basic | 0 | 1 | 2 | 1667 | 2.068 |
| variant-03/02-start-equals-end | 0 | 1 | 1 | 1467 | 1.975 |
| variant-03/03-budget-forces-detour | 0 | 1 | 1 | 2141 | 1.745 |
| variant-03/04-cost-at-cap-allowed | 0 | 1 | 1 | 1272 | 1.592 |
| variant-03/05-budget-unsat | 0 | 0 | 0 | 4970 | 1.950 |
| variant-03/06-layered-dag-cap | 0 | 2 | 15 | 227235 | 6.662 |
| variant-03/07-layered-dag-tight-cap | 0 | 4 | 6 | 120823 | 10.136 |
| variant-03/08-tie-break-under-cap | 0 | 1 | 2 | 3301 | 2.071 |
| variant-03/09-negative-weights | 0 | 1 | 1 | 2872 | 1.945 |
| variant-04/01-basic | 0 | 1 | 2 | 1808 | 1.892 |
| variant-04/02-start-equals-end | 0 | 1 | 1 | 1816 | 1.833 |
| variant-04/03-before-forces-detour-within-budget | 0 | 1 | 1 | 2774 | 2.448 |
| variant-04/04-ordering-violates-budget | 0 | 0 | 0 | 2816 | 1.961 |
| variant-04/05-after-and-budget-interact | 0 | 1 | 1 | 4169 | 2.120 |
| variant-04/06-layered-dag-ordering-cap | 0 | 2 | 8 | 69872 | 7.088 |
| variant-04/07-layered-dag-combined | 0 | 1 | 5 | 60073 | 7.675 |
| variant-04/08-tie-break-under-ordering-and-cap | 0 | 1 | 2 | 6554 | 2.373 |
| variant-04/09-negative-weights | 0 | 1 | 1 | 3246 | 2.143 |
| variant-01/01-basic | 0 | 1 | 4 | 1462 | 1.118 |
| variant-01/02-agent-reuse | 0 | 1 | 8 | 2191 | 1.325 |
| variant-01/03-selective-compatibility | 0 | 1 | 4 | 1873 | 1.624 |
| variant-01/04-no-compatible-agent-unsat | 0 | 0 | 0 | 1217 | 1.038 |
| variant-01/05-larger-mix | 0 | 1 | 34 | 42877 | 2.063 |
| variant-02/01-basic | 0 | 1 | 4 | 3047 | 1.523 |
| variant-02/02-makespan-tiebreak | 0 | 3 | 8 | 4031 | 1.930 |
| variant-02/03-cost-dominates | 0 | 1 | 8 | 5703 | 1.916 |
| variant-02/04-no-compatible-agent-unsat | 0 | 0 | 0 | 2395 | 1.526 |
| variant-02/05-larger-mix | 0 | 1 | 15 | 278383 | 5.797 |
| variant-03/01-basic | 0 | 2 | 2 | 1970 | 1.762 |
| variant-03/02-multiple-groups | 0 | 4 | 4 | 3816 | 1.748 |
| variant-03/03-mixed-grouped-ungrouped | 0 | 2 | 4 | 2639 | 1.695 |
| variant-03/04-incompatible-group-unsat | 0 | 0 | 0 | 1224 | 1.556 |
| variant-03/05-larger-mix | 0 | 1 | 27 | 5137 | 2.161 |
| variant-04/01-basic | 0 | 81 | 96 | 110850 | 5.392 |
| variant-04/02-precedence | 0 | 3 | 11 | 12702 | 4.698 |
| variant-04/03-agent-serialization | 0 | 30 | 30 | 32473 | 4.621 |
| variant-04/04-window-too-tight-unsat | 0 | 0 | 0 | 2865 | 2.891 |
| variant-04/05-larger-mix | 0 | 1176 | 1179 | 2119386 | 29.727 |
| variant-01/01-basic | 0 | 1 | 2 | 5138 | 1.881 |
| variant-01/02-multiple-tours | 0 | 2 | 9 | 35544 | 2.971 |
| variant-01/03-asymmetric | 0 | 1 | 8 | 52685 | 2.957 |
| variant-01/04-subtour-unsat | 0 | 0 | 0 | 2473 | 1.342 |
| variant-01/05-ring | 0 | 2 | 13 | 192804 | 5.223 |
| variant-02/01-basic | 0 | 2 | 2 | 5689 | 1.931 |
| variant-02/02-single-salesman | 0 | 1 | 2 | 3644 | 1.882 |
| variant-02/03-too-many-salesmen-unsat | 0 | 0 | 0 | 3466 | 1.527 |
| variant-02/04-equal-cost-split | 0 | 2 | 2 | 4647 | 1.956 |
| variant-02/05-larger-asymmetric | 0 | 2 | 2 | 6731 | 2.098 |
| variant-02/06-unreachable-edge | 0 | 2 | 2 | 7627 | 2.323 |
| variant-03/01-basic | 0 | 1 | 1 | 3939 | 2.038 |
| variant-03/02-single-salesman | 0 | 1 | 2 | 3498 | 1.928 |
| variant-03/03-depot-crossing-unsat | 0 | 0 | 0 | 3422 | 1.649 |
| variant-03/04-equal-cost-split | 0 | 2 | 2 | 7949 | 2.948 |
| variant-03/05-larger-three-depots | 0 | 1 | 1 | 10821 | 3.357 |
| variant-03/06-unreachable-edge | 0 | 1 | 1 | 6365 | 2.436 |
| variant-04/01-basic | 0 | 1 | 1 | 6955 | 3.019 |
| variant-04/02-single-salesman | 0 | 1 | 1 | 4700 | 2.707 |
| variant-04/03-window-too-tight-unsat | 0 | 0 | 0 | 3400 | 2.417 |
| variant-04/04-depot-window-too-tight-unsat | 0 | 0 | 0 | 2325 | 2.007 |
| variant-04/05-three-depots-asymmetric-times | 0 | 1 | 1 | 18328 | 4.219 |
| variant-04/06-unreachable-edge | 0 | 1 | 1 | 14062 | 3.792 |
| variant-05/01-basic | 0 | 1 | 1 | 6957 | 2.700 |
| variant-05/02-tight-bound-exact | 0 | 1 | 1 | 3439 | 2.388 |
| variant-05/03-revisit-too-tight-unsat | 0 | 0 | 0 | 3413 | 2.220 |
| variant-05/04-depot-and-vertex-revisits | 0 | 1 | 1 | 3504 | 2.500 |
| variant-05/05-three-depots-mixed | 0 | 1 | 1 | 18378 | 5.211 |
| variant-05/06-unreachable-edge | 0 | 1 | 1 | 14062 | 3.784 |
| n-queens/variant-01 | 0 | 92 | 92 | 199032 | 5.143 |
| n-queens/variant-02 | 0 | 92 | 92 | 14461401 | 57.857 |
| n-queens/variant-03 | 0 | 92 | 92 | 256834 | 5.422 |
| n-queens/variant-04 | 0 | 92 | 92 | 195309 | 4.003 |
| n-queens/variant-05 | 0 | 92 | 92 | 223549 | 5.299 |
| n-queens/variant-06 | 0 | 92 | 92 | 251111 | 5.200 |
| send-money/send-money | 0 | 1 | 1 | 409024 | 9.637 |

Against the reference: report corpus, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 9 of 94 cells where both passed (9.5%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 33.441 | 147.602 | 0.227 | 4.127 | 32.768 | 12.562 | 84.000 | 57.000 |
| variant-04/03-agent-serialization | 8.452 | 27.802 | 0.304 | 0.930 | 1.456 | 0.323 | 24.000 | 0.000 |
| variant-04/01-basic | 9.006 | 25.705 | 0.350 | 0.956 | 3.980 | 0.762 | 17.000 | 4.000 |
| n-queens/variant-02 | 61.878 | 120.317 | 0.514 | 2.450 | 173.279 | 1.236 | 2.000 | 114.000 |
| variant-01/02-agent-reuse | 4.769 | 5.459 | 0.874 | 0.160 | 0.142 | 0.032 | 0.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 5.891 | 6.564 | 0.898 | 0.296 | 0.107 | 0.022 | 1.000 | 0.000 |
| variant-01/05-multi-path | 5.109 | 5.554 | 0.920 | 0.277 | 0.159 | 0.028 | 1.000 | 0.000 |
| variant-02/03-cost-dominates | 6.093 | 6.607 | 0.922 | 0.320 | 0.227 | 0.038 | 1.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 5.086 | 5.464 | 0.931 | 0.292 | 0.039 | 0.009 | 1.000 | 0.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| send-money/send-money | 14.169 | 14.013 | 1.011 | 2.597 | 7.002 | 0.282 | 9.000 | 1.000 |
| variant-01/08-negative-weights | 5.612 | 5.493 | 1.022 | 0.267 | 0.077 | 0.047 | 1.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 4.552 | 4.400 | 1.035 | 0.115 | 0.030 | 0.005 | 0.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 4.676 | 4.483 | 1.043 | 0.274 | 0.043 | 0.008 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 5.751 | 5.499 | 1.046 | 0.271 | 0.096 | 0.081 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 5.871 | 5.487 | 1.070 | 0.361 | 0.132 | 0.014 | 1.000 | 0.000 |
| variant-02/05-larger-asymmetric | 5.866 | 5.477 | 1.071 | 0.463 | 0.151 | 0.146 | 1.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 4.820 | 4.440 | 1.086 | 0.329 | 0.066 | 0.011 | 1.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 5.938 | 5.459 | 1.088 | 0.382 | 0.076 | 0.012 | 1.000 | 0.000 |
| variant-02/01-basic | 5.919 | 5.425 | 1.091 | 0.361 | 0.148 | 0.125 | 1.000 | 0.000 |
| variant-01/07-cycles | 6.071 | 5.479 | 1.108 | 0.387 | 0.282 | 0.378 | 1.000 | 0.000 |
| variant-01/03-zero-cost-detour | 4.926 | 4.403 | 1.119 | 0.155 | 0.062 | 0.012 | 1.000 | 0.000 |
| variant-01/04-subtour-unsat | 4.973 | 4.444 | 1.119 | 0.183 | 0.038 | 0.011 | 0.000 | 0.000 |
| n-queens/variant-04 | 7.336 | 6.535 | 1.123 | 0.714 | 5.079 | 0.656 | 1.000 | 1.000 |
| variant-01/01-basic | 5.003 | 4.400 | 1.137 | 0.138 | 0.077 | 0.020 | 0.000 | 0.000 |
| variant-02/03-too-many-salesmen-unsat | 5.126 | 4.433 | 1.156 | 0.299 | 0.065 | 0.011 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 5.317 | 4.595 | 1.157 | 0.200 | 0.091 | 0.022 | 0.000 | 0.000 |
| variant-01/03-selective-compatibility | 5.177 | 4.450 | 1.164 | 0.174 | 0.077 | 0.023 | 0.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.176 | 4.439 | 1.166 | 0.183 | 0.044 | 0.010 | 0.000 | 0.000 |
| variant-02/01-basic | 5.138 | 4.378 | 1.174 | 0.293 | 0.131 | 0.028 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 5.186 | 4.396 | 1.180 | 0.223 | 0.094 | 0.027 | 1.000 | 0.000 |
| variant-02/06-layered-dag-before | 7.728 | 6.539 | 1.182 | 1.088 | 1.781 | 0.292 | 2.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 6.568 | 5.498 | 1.195 | 0.407 | 0.052 | 0.011 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 6.616 | 5.467 | 1.210 | 0.290 | 0.071 | 0.014 | 1.000 | 0.000 |
| variant-02/02-single-salesman | 5.367 | 4.433 | 1.211 | 0.273 | 0.094 | 0.088 | 1.000 | 0.000 |
| variant-05/02-tight-bound-exact | 6.588 | 5.424 | 1.215 | 0.382 | 0.074 | 0.049 | 1.000 | 0.000 |
| variant-04/01-basic | 6.722 | 5.500 | 1.222 | 0.625 | 0.116 | 0.080 | 1.000 | 0.000 |
| variant-03/01-basic | 6.698 | 5.461 | 1.226 | 0.365 | 0.095 | 0.059 | 1.000 | 0.000 |
| variant-03/04-cost-at-cap-allowed | 5.465 | 4.429 | 1.234 | 0.178 | 0.047 | 0.008 | 1.000 | 0.000 |
| variant-05/01-basic | 6.798 | 5.505 | 1.235 | 0.603 | 0.127 | 0.088 | 1.000 | 0.000 |
| variant-01/04-no-path | 5.543 | 4.462 | 1.242 | 0.169 | 0.037 | 0.005 | 1.000 | 0.000 |
| variant-04/02-precedence | 8.177 | 6.560 | 1.246 | 0.661 | 0.743 | 0.112 | 2.000 | 0.000 |
| variant-01/05-larger-mix | 5.512 | 4.422 | 1.246 | 0.257 | 1.770 | 0.152 | 0.000 | 0.000 |
| variant-02/01-basic | 5.542 | 4.446 | 1.247 | 0.195 | 0.081 | 0.019 | 1.000 | 0.000 |
| variant-02/03-before-forces-detour | 5.584 | 4.461 | 1.252 | 0.217 | 0.068 | 0.012 | 1.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 5.540 | 4.409 | 1.257 | 0.137 | 0.042 | 0.006 | 0.000 | 0.000 |
| variant-02/09-negative-weights | 5.564 | 4.411 | 1.261 | 0.256 | 0.083 | 0.049 | 1.000 | 0.000 |
| variant-03/09-negative-weights | 5.613 | 4.450 | 1.261 | 0.298 | 0.084 | 0.049 | 1.000 | 0.000 |
| variant-01/01-basic | 5.856 | 4.607 | 1.271 | 0.282 | 0.147 | 0.116 | 1.000 | 0.000 |
| variant-02/02-start-equals-end | 5.704 | 4.449 | 1.282 | 0.183 | 0.049 | 0.028 | 1.000 | 0.000 |
| equality-generalized-tsp/01-basic | 5.733 | 4.455 | 1.287 | 0.393 | 0.189 | 0.233 | 1.000 | 0.000 |
| variant-01/01-basic | 5.679 | 4.411 | 1.287 | 0.170 | 0.067 | 0.013 | 1.000 | 0.000 |
| variant-02/04-after-forces-extension | 5.652 | 4.388 | 1.288 | 0.228 | 0.061 | 0.012 | 1.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 5.747 | 4.453 | 1.291 | 0.250 | 0.100 | 0.020 | 1.000 | 0.000 |
| variant-03/01-basic | 5.813 | 4.471 | 1.300 | 0.303 | 0.074 | 0.015 | 1.000 | 0.000 |
| variant-04/02-start-equals-end | 5.804 | 4.461 | 1.301 | 0.212 | 0.055 | 0.034 | 1.000 | 0.000 |
| variant-03/01-basic | 5.770 | 4.423 | 1.305 | 0.176 | 0.071 | 0.015 | 0.000 | 0.000 |
| variant-01/02-multiple-tours | 7.243 | 5.546 | 1.306 | 0.410 | 0.516 | 1.020 | 1.000 | 0.000 |
| variant-03/02-start-equals-end | 5.807 | 4.408 | 1.317 | 0.204 | 0.045 | 0.028 | 1.000 | 0.000 |
| variant-03/06-unreachable-edge | 5.924 | 4.461 | 1.328 | 0.551 | 0.131 | 0.094 | 1.000 | 0.000 |
| variant-02/04-equal-cost-split | 5.845 | 4.399 | 1.329 | 0.311 | 0.128 | 0.111 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 5.936 | 4.450 | 1.334 | 0.240 | 0.065 | 0.013 | 1.000 | 0.000 |
| variant-02/05-ordering-unsat | 5.867 | 4.392 | 1.336 | 0.170 | 0.044 | 0.005 | 1.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 5.882 | 4.396 | 1.338 | 0.272 | 0.072 | 0.009 | 1.000 | 0.000 |
| n-queens/variant-06 | 8.822 | 6.579 | 1.341 | 1.047 | 5.732 | 1.108 | 1.000 | 1.000 |
| variant-04/09-negative-weights | 5.914 | 4.396 | 1.345 | 0.315 | 0.100 | 0.055 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 5.921 | 4.367 | 1.356 | 0.303 | 0.094 | 0.019 | 1.000 | 0.000 |
| variant-05/06-unreachable-edge | 7.718 | 5.499 | 1.403 | 1.186 | 0.209 | 0.169 | 1.000 | 0.000 |
| variant-04/01-basic | 6.205 | 4.421 | 1.404 | 0.210 | 0.081 | 0.018 | 1.000 | 0.000 |
| variant-02/07-layered-dag-before-after | 7.761 | 5.529 | 1.404 | 1.079 | 1.592 | 0.243 | 2.000 | 0.000 |
| n-queens/variant-03 | 9.236 | 6.568 | 1.406 | 1.836 | 5.400 | 0.962 | 1.000 | 1.000 |
| variant-02/02-makespan-tiebreak | 6.221 | 4.421 | 1.407 | 0.354 | 0.206 | 0.046 | 1.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 6.285 | 4.436 | 1.417 | 0.393 | 0.074 | 0.047 | 1.000 | 0.000 |
| variant-02/06-unreachable-edge | 6.460 | 4.535 | 1.424 | 0.493 | 0.176 | 0.164 | 1.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 8.069 | 5.621 | 1.436 | 1.519 | 0.243 | 0.197 | 1.000 | 0.000 |
| variant-04/06-unreachable-edge | 7.898 | 5.498 | 1.437 | 1.187 | 0.211 | 0.164 | 2.000 | 0.000 |
| variant-01/03-asymmetric | 6.544 | 4.493 | 1.456 | 0.529 | 0.737 | 1.232 | 1.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 6.574 | 4.445 | 1.479 | 0.326 | 0.097 | 0.011 | 1.000 | 0.000 |
| variant-01/02-start-equals-end | 5.022 | 3.323 | 1.511 | 0.148 | 0.053 | 0.025 | 1.000 | 0.000 |
| variant-04/07-layered-dag-combined | 11.648 | 7.647 | 1.523 | 1.849 | 2.108 | 0.326 | 2.000 | 0.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.812 | 4.435 | 1.536 | 0.440 | 0.156 | 0.038 | 1.000 | 0.000 |
| n-queens/variant-01 | 8.715 | 5.617 | 1.552 | 1.789 | 4.156 | 0.780 | 1.000 | 1.000 |
| variant-01/05-ring | 10.091 | 6.500 | 1.552 | 0.678 | 3.237 | 2.719 | 2.000 | 0.000 |
| variant-04/02-single-salesman | 6.897 | 4.425 | 1.559 | 0.482 | 0.091 | 0.060 | 1.000 | 0.000 |
| variant-03/05-larger-three-depots | 7.041 | 4.483 | 1.571 | 0.993 | 0.163 | 0.120 | 1.000 | 0.000 |
| equality-generalized-tsp/02-larger | 6.970 | 4.430 | 1.573 | 0.592 | 0.409 | 0.593 | 1.000 | 0.000 |
| variant-03/06-layered-dag-cap | 10.392 | 6.533 | 1.591 | 1.651 | 4.959 | 0.519 | 2.000 | 0.000 |
| variant-03/04-equal-cost-split | 7.171 | 4.447 | 1.613 | 0.706 | 0.169 | 0.152 | 1.000 | 0.000 |
| n-queens/variant-05 | 9.086 | 5.547 | 1.638 | 1.496 | 4.967 | 0.860 | 1.000 | 1.000 |
| variant-03/05-larger-mix | 5.518 | 3.356 | 1.644 | 0.294 | 0.492 | 0.095 | 0.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 10.832 | 6.579 | 1.646 | 1.807 | 2.461 | 0.434 | 3.000 | 0.000 |
| variant-02/05-larger-mix | 10.004 | 5.496 | 1.820 | 0.723 | 6.069 | 0.178 | 1.000 | 0.000 |
| variant-01/06-layered-dag | 9.941 | 5.454 | 1.823 | 1.042 | 3.499 | 0.335 | 2.000 | 0.000 |
| variant-05/05-three-depots-mixed | 8.690 | 4.444 | 1.955 | 1.666 | 0.284 | 0.201 | 1.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 14.033 | 6.574 | 2.135 | 1.738 | 5.525 | 0.322 | 2.000 | 1.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 16.3 | 23.5 | n/a |
| variant-04/03-agent-serialization | 11.7 | 12.4 | n/a |
| variant-04/01-basic | 11.9 | 13.2 | n/a |
| n-queens/variant-02 | 16.4 | 12.7 | n/a |
| variant-01/02-agent-reuse | 10.3 | 10.4 | n/a |
| variant-03/08-tie-break-under-cap | 10.7 | 10.5 | n/a |
| variant-01/05-multi-path | 10.5 | 10.4 | n/a |
| variant-02/03-cost-dominates | 10.6 | 10.4 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 11.3 | 10.5 | n/a |
| send-money/send-money | 13.9 | 12.7 | n/a |
| variant-01/08-negative-weights | 10.4 | 10.5 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.1 | 10.4 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.6 | 10.3 | n/a |
| variant-03/02-single-salesman | 10.7 | 10.3 | n/a |
| variant-03/05-budget-unsat | 10.8 | 10.5 | n/a |
| variant-02/05-larger-asymmetric | 10.9 | 10.3 | n/a |
| variant-03/03-depot-crossing-unsat | 10.5 | 10.2 | n/a |
| variant-05/03-revisit-too-tight-unsat | 11.1 | 10.5 | n/a |
| variant-02/01-basic | 10.4 | 10.6 | n/a |
| variant-01/07-cycles | 11.0 | 10.5 | n/a |
| variant-01/03-zero-cost-detour | 10.6 | 10.5 | n/a |
| variant-01/04-subtour-unsat | 10.4 | 10.3 | n/a |
| n-queens/variant-04 | 11.1 | 10.5 | n/a |
| variant-01/01-basic | 10.2 | 10.3 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.6 | 10.5 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.7 | 10.4 | n/a |
| variant-01/03-selective-compatibility | 10.1 | 10.2 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.4 | 10.4 | n/a |
| variant-02/01-basic | 10.5 | 10.5 | n/a |
| variant-03/02-multiple-groups | 10.7 | 10.3 | n/a |
| variant-02/06-layered-dag-before | 11.6 | 10.8 | n/a |
| variant-04/03-window-too-tight-unsat | 11.2 | 10.4 | n/a |
| variant-04/03-before-forces-detour-within-budget | 10.7 | 10.3 | n/a |
| variant-02/02-single-salesman | 10.6 | 10.3 | n/a |
| variant-05/02-tight-bound-exact | 11.1 | 10.6 | n/a |
| variant-04/01-basic | 11.2 | 10.5 | n/a |
| variant-03/01-basic | 10.4 | 10.5 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.4 | 10.5 | n/a |
| variant-05/01-basic | 10.7 | 10.6 | n/a |
| variant-01/04-no-path | 10.5 | 10.3 | n/a |
| variant-04/02-precedence | 11.3 | 10.8 | n/a |
| variant-01/05-larger-mix | 11.1 | 10.2 | n/a |
| variant-02/01-basic | 10.5 | 10.5 | n/a |
| variant-02/03-before-forces-detour | 10.6 | 10.3 | n/a |
| variant-03/04-incompatible-group-unsat | 10.6 | 10.1 | n/a |
| variant-02/09-negative-weights | 11.0 | 10.3 | n/a |
| variant-03/09-negative-weights | 10.8 | 10.5 | n/a |
| variant-01/01-basic | 10.5 | 10.5 | n/a |
| variant-02/02-start-equals-end | 10.5 | 10.5 | n/a |
| equality-generalized-tsp/01-basic | 10.7 | 10.4 | n/a |
| variant-01/01-basic | 10.4 | 10.5 | n/a |
| variant-02/04-after-forces-extension | 10.7 | 10.5 | n/a |
| variant-02/08-tie-break-under-ordering | 11.1 | 10.5 | n/a |
| variant-03/01-basic | 11.0 | 10.2 | n/a |
| variant-04/02-start-equals-end | 10.9 | 10.4 | n/a |
| variant-03/01-basic | 10.6 | 10.5 | n/a |
| variant-01/02-multiple-tours | 11.3 | 10.5 | n/a |
| variant-03/02-start-equals-end | 10.9 | 10.2 | n/a |
| variant-03/06-unreachable-edge | 10.6 | 10.5 | n/a |
| variant-02/04-equal-cost-split | 10.6 | 10.5 | n/a |
| variant-03/03-budget-forces-detour | 10.6 | 10.3 | n/a |
| variant-02/05-ordering-unsat | 10.5 | 10.2 | n/a |
| variant-04/04-ordering-violates-budget | 10.8 | 10.5 | n/a |
| n-queens/variant-06 | 11.8 | 10.4 | n/a |
| variant-04/09-negative-weights | 10.8 | 10.4 | n/a |
| variant-04/05-after-and-budget-interact | 11.0 | 10.5 | n/a |
| variant-05/06-unreachable-edge | 11.4 | 10.7 | n/a |
| variant-04/01-basic | 10.9 | 10.5 | n/a |
| variant-02/07-layered-dag-before-after | 11.9 | 10.9 | n/a |
| n-queens/variant-03 | 11.5 | 10.4 | n/a |
| variant-02/02-makespan-tiebreak | 10.8 | 10.4 | n/a |
| variant-05/04-depot-and-vertex-revisits | 10.9 | 10.6 | n/a |
| variant-02/06-unreachable-edge | 10.8 | 10.5 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.3 | 10.7 | n/a |
| variant-04/06-unreachable-edge | 11.1 | 10.8 | n/a |
| variant-01/03-asymmetric | 13.2 | 10.6 | n/a |
| variant-04/04-window-too-tight-unsat | 11.3 | 10.6 | n/a |
| variant-01/02-start-equals-end | 10.5 | 10.4 | n/a |
| variant-04/07-layered-dag-combined | 12.6 | 11.0 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 10.8 | 10.6 | n/a |
| n-queens/variant-01 | 11.4 | 10.5 | n/a |
| variant-01/05-ring | 13.9 | 10.6 | n/a |
| variant-04/02-single-salesman | 11.1 | 10.5 | n/a |
| variant-03/05-larger-three-depots | 10.9 | 10.5 | n/a |
| equality-generalized-tsp/02-larger | 11.3 | 10.3 | n/a |
| variant-03/06-layered-dag-cap | 13.8 | 11.0 | n/a |
| variant-03/04-equal-cost-split | 10.8 | 10.5 | n/a |
| n-queens/variant-05 | 11.5 | 10.7 | n/a |
| variant-03/05-larger-mix | 10.6 | 10.4 | n/a |
| variant-04/06-layered-dag-ordering-cap | 12.8 | 10.8 | n/a |
| variant-02/05-larger-mix | 12.1 | 10.5 | n/a |
| variant-01/06-layered-dag | 11.9 | 10.8 | n/a |
| variant-05/05-three-depots-mixed | 11.3 | 10.5 | n/a |
| variant-03/07-layered-dag-tight-cap | 12.6 | 11.0 | n/a |
