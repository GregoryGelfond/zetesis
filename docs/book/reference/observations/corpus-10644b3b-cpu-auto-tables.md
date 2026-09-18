Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | corpus | corpus/reference |
|---|---:|---:|
| equality-generalized-tsp/01-basic | 6.195 [5.829, 7.064] | 1.138 |
| equality-generalized-tsp/02-larger | 6.869 [5.521, 6.932] | 1.540 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.907 [4.684, 6.441] | 1.313 |
| variant-01/01-basic | 6.018 [5.184, 6.247] | 1.090 |
| variant-01/02-start-equals-end | 5.880 [5.684, 6.745] | 1.068 |
| variant-01/03-zero-cost-detour | 5.939 [4.995, 5.960] | 1.091 |
| variant-01/04-no-path | 5.094 [4.560, 5.516] | 0.927 |
| variant-01/05-multi-path | 5.260 [5.111, 6.630] | 0.952 |
| variant-01/06-layered-dag | 8.712 [8.130, 9.900] | 1.550 |
| variant-01/07-cycles | 6.236 [5.591, 6.842] | 1.411 |
| variant-01/08-negative-weights | 5.537 [5.489, 5.543] | 1.241 |
| variant-02/01-basic | 5.100 [4.733, 5.526] | 0.932 |
| variant-02/02-start-equals-end | 5.592 [4.478, 5.691] | 1.248 |
| variant-02/03-before-forces-detour | 5.483 [4.675, 6.378] | 1.241 |
| variant-02/04-after-forces-extension | 5.096 [4.801, 5.838] | 1.525 |
| variant-02/05-ordering-unsat | 5.850 [4.878, 6.203] | 1.328 |
| variant-02/06-layered-dag-before | 7.853 [7.601, 9.130] | 1.427 |
| variant-02/07-layered-dag-before-after | 7.309 [7.273, 7.823] | 1.327 |
| variant-02/08-tie-break-under-ordering | 5.519 [5.502, 5.990] | 1.237 |
| variant-02/09-negative-weights | 5.214 [4.857, 6.928] | 1.177 |
| variant-03/01-basic | 5.096 [4.739, 5.905] | 1.146 |
| variant-03/02-start-equals-end | 4.764 [4.487, 5.528] | 1.080 |
| variant-03/03-budget-forces-detour | 5.723 [5.560, 6.841] | 1.290 |
| variant-03/04-cost-at-cap-allowed | 5.524 [5.023, 5.533] | 1.012 |
| variant-03/05-budget-unsat | 5.727 [5.526, 5.748] | 1.305 |
| variant-03/06-layered-dag-cap | 12.908 [11.051, 12.986] | 1.967 |
| variant-03/07-layered-dag-tight-cap | 9.829 [9.748, 9.999] | 1.493 |
| variant-03/08-tie-break-under-cap | 5.520 [4.740, 6.198] | 1.209 |
| variant-03/09-negative-weights | 5.531 [5.247, 6.615] | 1.222 |
| variant-04/01-basic | 4.957 [4.647, 5.652] | 1.120 |
| variant-04/02-start-equals-end | 6.922 [6.158, 7.245] | 1.263 |
| variant-04/03-before-forces-detour-within-budget | 5.560 [5.496, 5.936] | 1.252 |
| variant-04/04-ordering-violates-budget | 6.582 [4.868, 6.633] | 1.484 |
| variant-04/05-after-and-budget-interact | 5.812 [5.534, 6.564] | 1.069 |
| variant-04/06-layered-dag-ordering-cap | 10.483 [10.249, 10.836] | 1.601 |
| variant-04/07-layered-dag-combined | 9.775 [8.714, 10.341] | 1.493 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.602 [6.255, 7.185] | 1.197 |
| variant-04/09-negative-weights | 6.749 [6.601, 6.999] | 1.562 |
| variant-01/01-basic | 4.850 [4.665, 5.257] | 1.092 |
| variant-01/02-agent-reuse | 4.883 [4.329, 5.201] | 1.097 |
| variant-01/03-selective-compatibility | 5.484 [3.735, 5.776] | 1.235 |
| variant-01/04-no-compatible-agent-unsat | 4.828 [4.823, 4.861] | 1.093 |
| variant-01/05-larger-mix | 5.741 [5.678, 5.744] | 1.048 |
| variant-02/01-basic | 5.830 [5.489, 6.708] | 1.315 |
| variant-02/02-makespan-tiebreak | 5.653 [5.600, 6.600] | 1.272 |
| variant-02/03-cost-dominates | 6.593 [5.522, 6.951] | 1.203 |
| variant-02/04-no-compatible-agent-unsat | 5.545 [5.027, 5.636] | 1.247 |
| variant-02/05-larger-mix | 8.973 [8.402, 10.874] | 1.634 |
| variant-03/01-basic | 5.663 [4.477, 6.114] | 1.276 |
| variant-03/02-multiple-groups | 5.545 [5.514, 6.059] | 1.247 |
| variant-03/03-mixed-grouped-ungrouped | 5.606 [5.154, 5.965] | 1.024 |
| variant-03/04-incompatible-group-unsat | 5.146 [4.770, 6.838] | 1.547 |
| variant-03/05-larger-mix | 6.271 [6.194, 6.736] | 1.416 |
| variant-04/01-basic | 9.851 [9.029, 10.864] | 0.370 |
| variant-04/02-precedence | 8.233 [7.760, 9.099] | 1.487 |
| variant-04/03-agent-serialization | 8.224 [7.666, 9.944] | 0.286 |
| variant-04/04-window-too-tight-unsat | 7.683 [7.015, 7.859] | 1.377 |
| variant-04/05-larger-mix | 32.843 [32.722, 35.245] | 0.218 |
| variant-01/01-basic | 5.963 [5.773, 6.172] | 1.282 |
| variant-01/02-multiple-tours | 6.052 [5.877, 6.685] | 1.327 |
| variant-01/03-asymmetric | 7.639 [6.340, 7.793] | 1.649 |
| variant-01/04-subtour-unsat | 4.957 [4.450, 5.647] | 0.903 |
| variant-01/05-ring | 8.687 [8.588, 8.862] | 1.582 |
| variant-02/01-basic | 6.371 [6.030, 6.633] | 1.160 |
| variant-02/02-single-salesman | 5.947 [5.655, 6.626] | 1.347 |
| variant-02/03-too-many-salesmen-unsat | 5.585 [4.466, 5.624] | 1.273 |
| variant-02/04-equal-cost-split | 5.518 [5.283, 6.590] | 1.252 |
| variant-02/05-larger-asymmetric | 5.768 [5.759, 5.802] | 1.308 |
| variant-02/06-unreachable-edge | 6.109 [5.803, 6.300] | 1.109 |
| variant-03/01-basic | 4.957 [4.728, 5.530] | 1.116 |
| variant-03/02-single-salesman | 4.647 [4.490, 5.562] | 1.043 |
| variant-03/03-depot-crossing-unsat | 5.579 [4.536, 5.982] | 1.248 |
| variant-03/04-equal-cost-split | 6.320 [5.930, 6.600] | 1.422 |
| variant-03/05-larger-three-depots | 6.617 [6.581, 6.813] | 1.205 |
| variant-03/06-unreachable-edge | 6.004 [4.937, 6.301] | 1.352 |
| variant-04/01-basic | 6.592 [5.429, 7.044] | 1.479 |
| variant-04/02-single-salesman | 6.138 [6.009, 6.772] | 1.115 |
| variant-04/03-window-too-tight-unsat | 6.885 [6.774, 6.925] | 1.250 |
| variant-04/04-depot-window-too-tight-unsat | 6.197 [6.094, 6.596] | 1.395 |
| variant-04/05-three-depots-asymmetric-times | 7.919 [7.636, 8.757] | 1.444 |
| variant-04/06-unreachable-edge | 7.641 [7.615, 8.796] | 1.396 |
| variant-05/01-basic | 7.277 [6.840, 7.499] | 1.630 |
| variant-05/02-tight-bound-exact | 6.666 [5.587, 7.289] | 1.219 |
| variant-05/03-revisit-too-tight-unsat | 6.584 [5.505, 6.655] | 1.199 |
| variant-05/04-depot-and-vertex-revisits | 6.575 [5.789, 6.629] | 1.197 |
| variant-05/05-three-depots-mixed | 8.681 [7.644, 9.492] | 1.581 |
| variant-05/06-unreachable-edge | 8.698 [8.372, 9.022] | 1.589 |
| n-queens/variant-01 | 9.938 [9.387, 10.105] | 1.528 |
| n-queens/variant-02 | 62.621 [62.488, 67.125] | 0.512 |
| n-queens/variant-03 | 8.840 [8.465, 9.877] | 1.153 |
| n-queens/variant-04 | 7.841 [7.088, 8.295] | 1.188 |
| n-queens/variant-05 | 8.765 [8.421, 9.567] | 1.320 |
| n-queens/variant-06 | 8.972 [8.733, 9.132] | 1.610 |
| send-money/send-money | 13.031 [12.894, 13.799] | 0.868 |

Reference wall time, ms, same notation.

| Cell | corpus |
|---|---:|
| equality-generalized-tsp/01-basic | 5.445 [4.477, 6.718] |
| equality-generalized-tsp/02-larger | 4.460 [4.456, 6.617] |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.497 [4.360, 5.543] |
| variant-01/01-basic | 5.521 [4.392, 5.526] |
| variant-01/02-start-equals-end | 5.504 [4.481, 5.526] |
| variant-01/03-zero-cost-detour | 5.441 [4.474, 5.524] |
| variant-01/04-no-path | 5.497 [4.405, 5.504] |
| variant-01/05-multi-path | 5.525 [5.493, 5.528] |
| variant-01/06-layered-dag | 5.619 [5.515, 6.624] |
| variant-01/07-cycles | 4.421 [4.421, 5.533] |
| variant-01/08-negative-weights | 4.460 [4.437, 5.494] |
| variant-02/01-basic | 5.473 [4.455, 5.493] |
| variant-02/02-start-equals-end | 4.481 [4.449, 4.485] |
| variant-02/03-before-forces-detour | 4.419 [4.369, 4.441] |
| variant-02/04-after-forces-extension | 3.341 [3.306, 4.449] |
| variant-02/05-ordering-unsat | 4.407 [4.356, 4.409] |
| variant-02/06-layered-dag-before | 5.505 [4.348, 6.587] |
| variant-02/07-layered-dag-before-after | 5.507 [5.374, 6.689] |
| variant-02/08-tie-break-under-ordering | 4.462 [4.385, 5.523] |
| variant-02/09-negative-weights | 4.432 [4.409, 4.442] |
| variant-03/01-basic | 4.447 [4.393, 4.461] |
| variant-03/02-start-equals-end | 4.411 [3.277, 4.461] |
| variant-03/03-budget-forces-detour | 4.437 [4.425, 5.499] |
| variant-03/04-cost-at-cap-allowed | 5.456 [4.386, 5.518] |
| variant-03/05-budget-unsat | 4.389 [4.351, 5.523] |
| variant-03/06-layered-dag-cap | 6.561 [5.443, 6.598] |
| variant-03/07-layered-dag-tight-cap | 6.584 [6.555, 7.598] |
| variant-03/08-tie-break-under-cap | 4.567 [4.472, 5.465] |
| variant-03/09-negative-weights | 4.526 [4.407, 5.486] |
| variant-04/01-basic | 4.427 [4.382, 5.427] |
| variant-04/02-start-equals-end | 5.479 [4.420, 5.505] |
| variant-04/03-before-forces-detour-within-budget | 4.440 [4.430, 4.455] |
| variant-04/04-ordering-violates-budget | 4.436 [4.409, 5.511] |
| variant-04/05-after-and-budget-interact | 5.439 [4.443, 5.496] |
| variant-04/06-layered-dag-ordering-cap | 6.549 [6.523, 7.636] |
| variant-04/07-layered-dag-combined | 6.547 [5.446, 7.627] |
| variant-04/08-tie-break-under-ordering-and-cap | 5.514 [5.504, 5.523] |
| variant-04/09-negative-weights | 4.321 [3.343, 4.464] |
| variant-01/01-basic | 4.442 [3.284, 4.448] |
| variant-01/02-agent-reuse | 4.450 [4.404, 4.455] |
| variant-01/03-selective-compatibility | 4.439 [3.321, 5.507] |
| variant-01/04-no-compatible-agent-unsat | 4.418 [4.407, 4.468] |
| variant-01/05-larger-mix | 5.476 [4.440, 5.523] |
| variant-02/01-basic | 4.435 [4.356, 4.463] |
| variant-02/02-makespan-tiebreak | 4.445 [4.438, 6.602] |
| variant-02/03-cost-dominates | 5.482 [4.419, 5.491] |
| variant-02/04-no-compatible-agent-unsat | 4.445 [4.435, 4.448] |
| variant-02/05-larger-mix | 5.493 [4.534, 5.503] |
| variant-03/01-basic | 4.439 [4.435, 4.442] |
| variant-03/02-multiple-groups | 4.447 [4.421, 4.456] |
| variant-03/03-mixed-grouped-ungrouped | 5.476 [4.433, 5.500] |
| variant-03/04-incompatible-group-unsat | 3.326 [3.306, 4.390] |
| variant-03/05-larger-mix | 4.427 [4.411, 5.497] |
| variant-04/01-basic | 26.601 [25.661, 26.658] |
| variant-04/02-precedence | 5.538 [5.476, 7.613] |
| variant-04/03-agent-serialization | 28.740 [27.817, 29.839] |
| variant-04/04-window-too-tight-unsat | 5.579 [5.546, 6.580] |
| variant-04/05-larger-mix | 150.554 [148.594, 150.678] |
| variant-01/01-basic | 4.653 [4.522, 5.483] |
| variant-01/02-multiple-tours | 4.561 [4.497, 5.478] |
| variant-01/03-asymmetric | 4.633 [4.448, 5.473] |
| variant-01/04-subtour-unsat | 5.488 [5.488, 5.542] |
| variant-01/05-ring | 5.490 [4.364, 5.493] |
| variant-02/01-basic | 5.490 [4.435, 5.621] |
| variant-02/02-single-salesman | 4.415 [3.337, 5.535] |
| variant-02/03-too-many-salesmen-unsat | 4.389 [3.297, 4.403] |
| variant-02/04-equal-cost-split | 4.408 [4.373, 5.443] |
| variant-02/05-larger-asymmetric | 4.409 [4.333, 5.529] |
| variant-02/06-unreachable-edge | 5.507 [4.434, 5.732] |
| variant-03/01-basic | 4.442 [4.389, 5.443] |
| variant-03/02-single-salesman | 4.453 [4.445, 4.462] |
| variant-03/03-depot-crossing-unsat | 4.471 [4.391, 5.509] |
| variant-03/04-equal-cost-split | 4.446 [4.400, 4.480] |
| variant-03/05-larger-three-depots | 5.491 [4.442, 5.536] |
| variant-03/06-unreachable-edge | 4.441 [4.424, 5.444] |
| variant-04/01-basic | 4.456 [4.388, 5.485] |
| variant-04/02-single-salesman | 5.506 [5.497, 5.521] |
| variant-04/03-window-too-tight-unsat | 5.506 [4.366, 5.520] |
| variant-04/04-depot-window-too-tight-unsat | 4.442 [4.416, 5.484] |
| variant-04/05-three-depots-asymmetric-times | 5.486 [5.473, 5.499] |
| variant-04/06-unreachable-edge | 5.473 [4.411, 5.553] |
| variant-05/01-basic | 4.463 [4.370, 5.480] |
| variant-05/02-tight-bound-exact | 5.470 [4.447, 7.591] |
| variant-05/03-revisit-too-tight-unsat | 5.490 [5.465, 5.494] |
| variant-05/04-depot-and-vertex-revisits | 5.494 [4.434, 5.538] |
| variant-05/05-three-depots-mixed | 5.492 [4.449, 6.521] |
| variant-05/06-unreachable-edge | 5.475 [5.457, 6.593] |
| n-queens/variant-01 | 6.505 [5.466, 6.594] |
| n-queens/variant-02 | 122.362 [122.169, 126.616] |
| n-queens/variant-03 | 7.666 [6.549, 7.701] |
| n-queens/variant-04 | 6.600 [5.498, 7.680] |
| n-queens/variant-05 | 6.640 [6.575, 6.779] |
| n-queens/variant-06 | 5.572 [5.543, 6.499] |
| send-money/send-money | 15.010 [13.931, 15.017] |

Counters of report corpus: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 0 | 1 | 4 | 8982 | 2.211 |
| equality-generalized-tsp/02-larger | 0 | 1 | 7 | 22348 | 2.867 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 0 | 0 | 0 | 2258 | 1.696 |
| variant-01/01-basic | 0 | 1 | 2 | 1559 | 1.523 |
| variant-01/02-start-equals-end | 0 | 1 | 1 | 1208 | 1.505 |
| variant-01/03-zero-cost-detour | 0 | 1 | 2 | 1291 | 1.556 |
| variant-01/04-no-path | 0 | 0 | 0 | 1085 | 1.465 |
| variant-01/05-multi-path | 0 | 1 | 3 | 3958 | 1.671 |
| variant-01/06-layered-dag | 0 | 1 | 17 | 129531 | 4.953 |
| variant-01/07-cycles | 0 | 1 | 5 | 13480 | 2.522 |
| variant-01/08-negative-weights | 0 | 1 | 1 | 2442 | 1.688 |
| variant-02/01-basic | 0 | 1 | 2 | 1700 | 1.557 |
| variant-02/02-start-equals-end | 0 | 1 | 1 | 1557 | 1.872 |
| variant-02/03-before-forces-detour | 0 | 1 | 1 | 2383 | 1.975 |
| variant-02/04-after-forces-extension | 0 | 1 | 1 | 2245 | 1.683 |
| variant-02/05-ordering-unsat | 0 | 0 | 0 | 1322 | 1.933 |
| variant-02/06-layered-dag-before | 0 | 2 | 9 | 48810 | 4.423 |
| variant-02/07-layered-dag-before-after | 0 | 1 | 7 | 42762 | 3.873 |
| variant-02/08-tie-break-under-ordering | 0 | 1 | 2 | 2777 | 1.820 |
| variant-02/09-negative-weights | 0 | 1 | 1 | 2816 | 2.045 |
| variant-03/01-basic | 0 | 1 | 2 | 1667 | 2.016 |
| variant-03/02-start-equals-end | 0 | 1 | 1 | 1467 | 1.484 |
| variant-03/03-budget-forces-detour | 0 | 1 | 1 | 2141 | 1.643 |
| variant-03/04-cost-at-cap-allowed | 0 | 1 | 1 | 1272 | 1.524 |
| variant-03/05-budget-unsat | 0 | 0 | 0 | 4970 | 1.793 |
| variant-03/06-layered-dag-cap | 0 | 2 | 11 | 275611 | 8.759 |
| variant-03/07-layered-dag-tight-cap | 0 | 4 | 6 | 107944 | 6.108 |
| variant-03/08-tie-break-under-cap | 0 | 1 | 2 | 3301 | 1.770 |
| variant-03/09-negative-weights | 0 | 1 | 1 | 2872 | 1.977 |
| variant-04/01-basic | 0 | 1 | 2 | 1808 | 1.812 |
| variant-04/02-start-equals-end | 0 | 1 | 1 | 1816 | 2.152 |
| variant-04/03-before-forces-detour-within-budget | 0 | 1 | 1 | 2774 | 2.115 |
| variant-04/04-ordering-violates-budget | 0 | 0 | 0 | 2816 | 1.909 |
| variant-04/05-after-and-budget-interact | 0 | 1 | 1 | 4169 | 1.943 |
| variant-04/06-layered-dag-ordering-cap | 0 | 2 | 8 | 69872 | 6.448 |
| variant-04/07-layered-dag-combined | 0 | 1 | 4 | 63334 | 6.048 |
| variant-04/08-tie-break-under-ordering-and-cap | 0 | 1 | 2 | 6554 | 2.352 |
| variant-04/09-negative-weights | 0 | 1 | 1 | 3246 | 3.152 |
| variant-01/01-basic | 0 | 1 | 4 | 1462 | 1.122 |
| variant-01/02-agent-reuse | 0 | 1 | 8 | 2796 | 1.263 |
| variant-01/03-selective-compatibility | 0 | 1 | 4 | 1873 | 1.382 |
| variant-01/04-no-compatible-agent-unsat | 0 | 0 | 0 | 1217 | 1.023 |
| variant-01/05-larger-mix | 0 | 1 | 43 | 36198 | 2.054 |
| variant-02/01-basic | 0 | 1 | 4 | 3047 | 2.046 |
| variant-02/02-makespan-tiebreak | 0 | 3 | 8 | 4031 | 1.814 |
| variant-02/03-cost-dominates | 0 | 1 | 8 | 4006 | 2.547 |
| variant-02/04-no-compatible-agent-unsat | 0 | 0 | 0 | 2395 | 1.604 |
| variant-02/05-larger-mix | 0 | 1 | 13 | 174500 | 5.823 |
| variant-03/01-basic | 0 | 2 | 2 | 1970 | 1.802 |
| variant-03/02-multiple-groups | 0 | 4 | 4 | 3816 | 2.219 |
| variant-03/03-mixed-grouped-ungrouped | 0 | 2 | 4 | 2639 | 2.060 |
| variant-03/04-incompatible-group-unsat | 0 | 0 | 0 | 1224 | 1.378 |
| variant-03/05-larger-mix | 0 | 1 | 27 | 5137 | 2.315 |
| variant-04/01-basic | 0 | 81 | 90 | 83986 | 6.128 |
| variant-04/02-precedence | 0 | 3 | 7 | 13443 | 4.095 |
| variant-04/03-agent-serialization | 0 | 30 | 30 | 32214 | 5.007 |
| variant-04/04-window-too-tight-unsat | 0 | 0 | 0 | 2865 | 3.468 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 2200969 | 29.201 |
| variant-01/01-basic | 0 | 1 | 2 | 5138 | 2.019 |
| variant-01/02-multiple-tours | 0 | 2 | 9 | 35544 | 2.484 |
| variant-01/03-asymmetric | 0 | 1 | 9 | 54815 | 3.738 |
| variant-01/04-subtour-unsat | 0 | 0 | 0 | 2473 | 1.383 |
| variant-01/05-ring | 0 | 2 | 12 | 189266 | 5.205 |
| variant-02/01-basic | 0 | 2 | 2 | 5689 | 2.410 |
| variant-02/02-single-salesman | 0 | 1 | 2 | 3644 | 2.086 |
| variant-02/03-too-many-salesmen-unsat | 0 | 0 | 0 | 3466 | 1.675 |
| variant-02/04-equal-cost-split | 0 | 2 | 2 | 4647 | 1.783 |
| variant-02/05-larger-asymmetric | 0 | 2 | 2 | 6731 | 2.047 |
| variant-02/06-unreachable-edge | 0 | 2 | 2 | 7627 | 2.388 |
| variant-03/01-basic | 0 | 1 | 1 | 3939 | 1.712 |
| variant-03/02-single-salesman | 0 | 1 | 2 | 3498 | 1.815 |
| variant-03/03-depot-crossing-unsat | 0 | 0 | 0 | 3422 | 1.601 |
| variant-03/04-equal-cost-split | 0 | 2 | 2 | 7949 | 2.309 |
| variant-03/05-larger-three-depots | 0 | 1 | 1 | 10821 | 3.149 |
| variant-03/06-unreachable-edge | 0 | 1 | 1 | 6365 | 2.181 |
| variant-04/01-basic | 0 | 1 | 1 | 6955 | 2.663 |
| variant-04/02-single-salesman | 0 | 1 | 1 | 4700 | 2.598 |
| variant-04/03-window-too-tight-unsat | 0 | 0 | 0 | 3400 | 2.630 |
| variant-04/04-depot-window-too-tight-unsat | 0 | 0 | 0 | 2325 | 2.109 |
| variant-04/05-three-depots-asymmetric-times | 0 | 1 | 1 | 18328 | 4.364 |
| variant-04/06-unreachable-edge | 0 | 1 | 1 | 14062 | 4.388 |
| variant-05/01-basic | 0 | 1 | 1 | 6957 | 3.006 |
| variant-05/02-tight-bound-exact | 0 | 1 | 1 | 3439 | 2.421 |
| variant-05/03-revisit-too-tight-unsat | 0 | 0 | 0 | 3413 | 2.304 |
| variant-05/04-depot-and-vertex-revisits | 0 | 1 | 1 | 3504 | 2.392 |
| variant-05/05-three-depots-mixed | 0 | 1 | 1 | 18378 | 4.565 |
| variant-05/06-unreachable-edge | 0 | 1 | 1 | 14062 | 4.726 |
| n-queens/variant-01 | 0 | 92 | 92 | 199032 | 6.374 |
| n-queens/variant-02 | 0 | 92 | 92 | 14461401 | 58.480 |
| n-queens/variant-03 | 0 | 92 | 92 | 256834 | 5.283 |
| n-queens/variant-04 | 0 | 92 | 92 | 195309 | 3.808 |
| n-queens/variant-05 | 0 | 92 | 92 | 223549 | 5.322 |
| n-queens/variant-06 | 0 | 92 | 92 | 251111 | 5.352 |
| send-money/send-money | 0 | 1 | 1 | 409024 | 9.329 |

Against the reference: report corpus, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 9 of 94 cells where both passed (9.5%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 32.843 | 150.554 | 0.218 | 3.855 | 34.131 | 11.724 | 86.000 | 58.000 |
| variant-04/03-agent-serialization | 8.224 | 28.740 | 0.286 | 0.928 | 1.529 | 0.346 | 24.000 | 0.000 |
| variant-04/01-basic | 9.851 | 26.601 | 0.370 | 1.022 | 3.569 | 0.686 | 18.000 | 5.000 |
| n-queens/variant-02 | 62.621 | 122.362 | 0.512 | 2.443 | 177.569 | 1.170 | 1.000 | 117.000 |
| send-money/send-money | 13.031 | 15.010 | 0.868 | 3.198 | 6.799 | 0.282 | 9.000 | 1.000 |
| variant-01/04-subtour-unsat | 4.957 | 5.488 | 0.903 | 0.195 | 0.037 | 0.010 | 1.000 | 0.000 |
| variant-01/04-no-path | 5.094 | 5.497 | 0.927 | 0.176 | 0.032 | 0.006 | 1.000 | 0.000 |
| variant-02/01-basic | 5.100 | 5.473 | 0.932 | 0.183 | 0.073 | 0.018 | 1.000 | 0.000 |
| variant-01/05-multi-path | 5.260 | 5.525 | 0.952 | 0.300 | 0.141 | 0.027 | 1.000 | 0.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-03/04-cost-at-cap-allowed | 5.524 | 5.456 | 1.012 | 0.181 | 0.049 | 0.008 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 5.606 | 5.476 | 1.024 | 0.197 | 0.100 | 0.025 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 4.647 | 4.453 | 1.043 | 0.273 | 0.118 | 0.092 | 1.000 | 0.000 |
| variant-01/05-larger-mix | 5.741 | 5.476 | 1.048 | 0.264 | 1.622 | 0.155 | 1.000 | 0.000 |
| variant-01/02-start-equals-end | 5.880 | 5.504 | 1.068 | 0.155 | 0.045 | 0.028 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 5.812 | 5.439 | 1.069 | 0.308 | 0.094 | 0.020 | 1.000 | 0.000 |
| variant-03/02-start-equals-end | 4.764 | 4.411 | 1.080 | 0.171 | 0.046 | 0.027 | 1.000 | 0.000 |
| variant-01/01-basic | 6.018 | 5.521 | 1.090 | 0.191 | 0.071 | 0.014 | 1.000 | 0.000 |
| variant-01/03-zero-cost-detour | 5.939 | 5.441 | 1.091 | 0.168 | 0.036 | 0.009 | 1.000 | 0.000 |
| variant-01/01-basic | 4.850 | 4.442 | 1.092 | 0.142 | 0.079 | 0.021 | 0.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 4.828 | 4.418 | 1.093 | 0.116 | 0.030 | 0.006 | 0.000 | 0.000 |
| variant-01/02-agent-reuse | 4.883 | 4.450 | 1.097 | 0.155 | 0.117 | 0.033 | 0.000 | 0.000 |
| variant-02/06-unreachable-edge | 6.109 | 5.507 | 1.109 | 0.485 | 0.170 | 0.149 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 6.138 | 5.506 | 1.115 | 0.475 | 0.091 | 0.060 | 1.000 | 0.000 |
| variant-03/01-basic | 4.957 | 4.442 | 1.116 | 0.312 | 0.076 | 0.052 | 1.000 | 0.000 |
| variant-04/01-basic | 4.957 | 4.427 | 1.120 | 0.205 | 0.072 | 0.018 | 1.000 | 0.000 |
| equality-generalized-tsp/01-basic | 6.195 | 5.445 | 1.138 | 0.392 | 0.198 | 0.236 | 1.000 | 0.000 |
| variant-03/01-basic | 5.096 | 4.447 | 1.146 | 0.207 | 0.067 | 0.015 | 1.000 | 0.000 |
| n-queens/variant-03 | 8.840 | 7.666 | 1.153 | 1.858 | 4.932 | 0.767 | 1.000 | 1.000 |
| variant-02/01-basic | 6.371 | 5.490 | 1.160 | 0.376 | 0.142 | 0.126 | 1.000 | 0.000 |
| variant-02/09-negative-weights | 5.214 | 4.432 | 1.177 | 0.272 | 0.086 | 0.047 | 1.000 | 0.000 |
| n-queens/variant-04 | 7.841 | 6.600 | 1.188 | 0.712 | 4.961 | 0.637 | 1.000 | 1.000 |
| variant-05/04-depot-and-vertex-revisits | 6.575 | 5.494 | 1.197 | 0.408 | 0.073 | 0.047 | 1.000 | 0.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.602 | 5.514 | 1.197 | 0.439 | 0.187 | 0.043 | 1.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 6.584 | 5.490 | 1.199 | 0.367 | 0.066 | 0.011 | 1.000 | 0.000 |
| variant-02/03-cost-dominates | 6.593 | 5.482 | 1.203 | 0.329 | 0.174 | 0.044 | 1.000 | 0.000 |
| variant-03/05-larger-three-depots | 6.617 | 5.491 | 1.205 | 0.821 | 0.174 | 0.127 | 1.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 5.520 | 4.567 | 1.209 | 0.270 | 0.101 | 0.021 | 1.000 | 0.000 |
| variant-05/02-tight-bound-exact | 6.666 | 5.470 | 1.219 | 0.390 | 0.072 | 0.048 | 1.000 | 0.000 |
| variant-03/09-negative-weights | 5.531 | 4.526 | 1.222 | 0.302 | 0.084 | 0.050 | 1.000 | 0.000 |
| variant-01/03-selective-compatibility | 5.484 | 4.439 | 1.235 | 0.169 | 0.062 | 0.019 | 0.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 5.519 | 4.462 | 1.237 | 0.253 | 0.095 | 0.020 | 1.000 | 0.000 |
| variant-02/03-before-forces-detour | 5.483 | 4.419 | 1.241 | 0.246 | 0.069 | 0.014 | 1.000 | 0.000 |
| variant-01/08-negative-weights | 5.537 | 4.460 | 1.241 | 0.268 | 0.071 | 0.043 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 5.545 | 4.447 | 1.247 | 0.245 | 0.094 | 0.028 | 1.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 5.545 | 4.445 | 1.247 | 0.281 | 0.046 | 0.009 | 0.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 5.579 | 4.471 | 1.248 | 0.306 | 0.083 | 0.010 | 1.000 | 0.000 |
| variant-02/02-start-equals-end | 5.592 | 4.481 | 1.248 | 0.201 | 0.052 | 0.030 | 1.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 6.885 | 5.506 | 1.250 | 0.364 | 0.068 | 0.011 | 1.000 | 0.000 |
| variant-02/04-equal-cost-split | 5.518 | 4.408 | 1.252 | 0.322 | 0.100 | 0.088 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 5.560 | 4.440 | 1.252 | 0.307 | 0.073 | 0.015 | 1.000 | 0.000 |
| variant-04/02-start-equals-end | 6.922 | 5.479 | 1.263 | 0.201 | 0.051 | 0.032 | 1.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 5.653 | 4.445 | 1.272 | 0.356 | 0.252 | 0.063 | 1.000 | 0.000 |
| variant-02/03-too-many-salesmen-unsat | 5.585 | 4.389 | 1.273 | 0.294 | 0.071 | 0.012 | 1.000 | 0.000 |
| variant-03/01-basic | 5.663 | 4.439 | 1.276 | 0.167 | 0.071 | 0.015 | 0.000 | 0.000 |
| variant-01/01-basic | 5.963 | 4.653 | 1.282 | 0.280 | 0.134 | 0.087 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 5.723 | 4.437 | 1.290 | 0.217 | 0.061 | 0.012 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 5.727 | 4.389 | 1.305 | 0.358 | 0.122 | 0.014 | 1.000 | 0.000 |
| variant-02/05-larger-asymmetric | 5.768 | 4.409 | 1.308 | 0.444 | 0.162 | 0.145 | 1.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.907 | 4.497 | 1.313 | 0.193 | 0.039 | 0.010 | 0.000 | 0.000 |
| variant-02/01-basic | 5.830 | 4.435 | 1.315 | 0.449 | 0.163 | 0.033 | 1.000 | 0.000 |
| n-queens/variant-05 | 8.765 | 6.640 | 1.320 | 1.043 | 4.781 | 0.786 | 1.000 | 1.000 |
| variant-01/02-multiple-tours | 6.052 | 4.561 | 1.327 | 0.390 | 0.532 | 0.987 | 1.000 | 0.000 |
| variant-02/07-layered-dag-before-after | 7.309 | 5.507 | 1.327 | 1.146 | 1.623 | 0.267 | 2.000 | 0.000 |
| variant-02/05-ordering-unsat | 5.850 | 4.407 | 1.328 | 0.182 | 0.042 | 0.006 | 1.000 | 0.000 |
| variant-02/02-single-salesman | 5.947 | 4.415 | 1.347 | 0.296 | 0.127 | 0.087 | 1.000 | 0.000 |
| variant-03/06-unreachable-edge | 6.004 | 4.441 | 1.352 | 0.524 | 0.128 | 0.075 | 1.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 7.683 | 5.579 | 1.377 | 0.321 | 0.085 | 0.010 | 1.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 6.197 | 4.442 | 1.395 | 0.302 | 0.052 | 0.009 | 1.000 | 0.000 |
| variant-04/06-unreachable-edge | 7.641 | 5.473 | 1.396 | 1.231 | 0.215 | 0.180 | 1.000 | 0.000 |
| variant-01/07-cycles | 6.236 | 4.421 | 1.411 | 0.412 | 0.306 | 0.358 | 1.000 | 0.000 |
| variant-03/05-larger-mix | 6.271 | 4.427 | 1.416 | 0.280 | 0.624 | 0.096 | 1.000 | 0.000 |
| variant-03/04-equal-cost-split | 6.320 | 4.446 | 1.422 | 0.461 | 0.170 | 0.166 | 1.000 | 0.000 |
| variant-02/06-layered-dag-before | 7.853 | 5.505 | 1.427 | 1.133 | 1.825 | 0.289 | 2.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 7.919 | 5.486 | 1.444 | 1.584 | 0.255 | 0.202 | 1.000 | 0.000 |
| variant-04/01-basic | 6.592 | 4.456 | 1.479 | 0.607 | 0.116 | 0.081 | 1.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 6.582 | 4.436 | 1.484 | 0.296 | 0.075 | 0.010 | 1.000 | 0.000 |
| variant-04/02-precedence | 8.233 | 5.538 | 1.487 | 0.631 | 0.756 | 0.094 | 2.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 9.829 | 6.584 | 1.493 | 1.733 | 3.070 | 0.301 | 2.000 | 0.000 |
| variant-04/07-layered-dag-combined | 9.775 | 6.547 | 1.493 | 1.739 | 2.176 | 0.344 | 2.000 | 0.000 |
| variant-02/04-after-forces-extension | 5.096 | 3.341 | 1.525 | 0.234 | 0.062 | 0.013 | 1.000 | 0.000 |
| n-queens/variant-01 | 9.938 | 6.505 | 1.528 | 2.871 | 4.555 | 0.893 | 1.000 | 1.000 |
| equality-generalized-tsp/02-larger | 6.869 | 4.460 | 1.540 | 0.693 | 0.429 | 0.504 | 1.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 5.146 | 3.326 | 1.547 | 0.135 | 0.041 | 0.005 | 0.000 | 0.000 |
| variant-01/06-layered-dag | 8.712 | 5.619 | 1.550 | 1.049 | 3.581 | 0.314 | 2.000 | 0.000 |
| variant-04/09-negative-weights | 6.749 | 4.321 | 1.562 | 0.373 | 0.135 | 0.053 | 1.000 | 0.000 |
| variant-05/05-three-depots-mixed | 8.681 | 5.492 | 1.581 | 1.562 | 0.263 | 0.208 | 1.000 | 0.000 |
| variant-01/05-ring | 8.687 | 5.490 | 1.582 | 0.687 | 2.380 | 1.400 | 1.000 | 0.000 |
| variant-05/06-unreachable-edge | 8.698 | 5.475 | 1.589 | 1.210 | 0.211 | 0.174 | 1.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 10.483 | 6.549 | 1.601 | 1.798 | 2.321 | 0.424 | 2.000 | 0.000 |
| n-queens/variant-06 | 8.972 | 5.572 | 1.610 | 1.072 | 5.678 | 1.053 | 1.000 | 1.000 |
| variant-05/01-basic | 7.277 | 4.463 | 1.630 | 0.629 | 0.133 | 0.086 | 1.000 | 0.000 |
| variant-02/05-larger-mix | 8.973 | 5.493 | 1.634 | 0.720 | 5.501 | 0.163 | 1.000 | 0.000 |
| variant-01/03-asymmetric | 7.639 | 4.633 | 1.649 | 0.550 | 0.633 | 0.964 | 1.000 | 0.000 |
| variant-03/06-layered-dag-cap | 12.908 | 6.561 | 1.967 | 1.686 | 6.376 | 0.453 | 3.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 15.9 | 23.6 | n/a |
| variant-04/03-agent-serialization | 11.5 | 12.2 | n/a |
| variant-04/01-basic | 12.2 | 13.4 | n/a |
| n-queens/variant-02 | 16.5 | 12.8 | n/a |
| send-money/send-money | 14.3 | 12.9 | n/a |
| variant-01/04-subtour-unsat | 10.4 | 10.4 | n/a |
| variant-01/04-no-path | 10.5 | 10.4 | n/a |
| variant-02/01-basic | 10.6 | 10.6 | n/a |
| variant-01/05-multi-path | 10.8 | 10.5 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.5 | 10.5 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.5 | 10.2 | n/a |
| variant-03/02-single-salesman | 10.6 | 10.5 | n/a |
| variant-01/05-larger-mix | 10.9 | 10.2 | n/a |
| variant-01/02-start-equals-end | 10.4 | 10.4 | n/a |
| variant-04/05-after-and-budget-interact | 10.6 | 10.3 | n/a |
| variant-03/02-start-equals-end | 10.5 | 10.5 | n/a |
| variant-01/01-basic | 10.7 | 10.5 | n/a |
| variant-01/03-zero-cost-detour | 10.3 | 10.4 | n/a |
| variant-01/01-basic | 10.3 | 10.3 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.3 | 10.4 | n/a |
| variant-01/02-agent-reuse | 10.3 | 10.4 | n/a |
| variant-02/06-unreachable-edge | 10.5 | 10.5 | n/a |
| variant-04/02-single-salesman | 11.3 | 10.5 | n/a |
| variant-03/01-basic | 10.3 | 10.2 | n/a |
| variant-04/01-basic | 10.6 | 10.6 | n/a |
| equality-generalized-tsp/01-basic | 10.5 | 10.6 | n/a |
| variant-03/01-basic | 10.6 | 10.6 | n/a |
| n-queens/variant-03 | 11.5 | 10.4 | n/a |
| variant-02/01-basic | 10.7 | 10.4 | n/a |
| variant-02/09-negative-weights | 10.8 | 10.5 | n/a |
| n-queens/variant-04 | 11.3 | 10.4 | n/a |
| variant-05/04-depot-and-vertex-revisits | 11.0 | 10.5 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 10.6 | 10.8 | n/a |
| variant-05/03-revisit-too-tight-unsat | 10.8 | 10.4 | n/a |
| variant-02/03-cost-dominates | 10.7 | 10.3 | n/a |
| variant-03/05-larger-three-depots | 10.8 | 10.5 | n/a |
| variant-03/08-tie-break-under-cap | 10.9 | 10.5 | n/a |
| variant-05/02-tight-bound-exact | 10.7 | 10.6 | n/a |
| variant-03/09-negative-weights | 10.8 | 10.6 | n/a |
| variant-01/03-selective-compatibility | 10.4 | 10.1 | n/a |
| variant-02/08-tie-break-under-ordering | 10.9 | 10.6 | n/a |
| variant-02/03-before-forces-detour | 10.8 | 10.6 | n/a |
| variant-01/08-negative-weights | 10.7 | 10.4 | n/a |
| variant-03/02-multiple-groups | 10.9 | 10.5 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.5 | 10.4 | n/a |
| variant-03/03-depot-crossing-unsat | 10.5 | 10.3 | n/a |
| variant-02/02-start-equals-end | 10.8 | 10.5 | n/a |
| variant-04/03-window-too-tight-unsat | 11.0 | 10.6 | n/a |
| variant-02/04-equal-cost-split | 10.7 | 10.6 | n/a |
| variant-04/03-before-forces-detour-within-budget | 10.5 | 10.6 | n/a |
| variant-04/02-start-equals-end | 10.4 | 10.6 | n/a |
| variant-02/02-makespan-tiebreak | 10.4 | 10.4 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.8 | 10.3 | n/a |
| variant-03/01-basic | 10.5 | 10.3 | n/a |
| variant-01/01-basic | 10.4 | 10.4 | n/a |
| variant-03/03-budget-forces-detour | 10.6 | 10.6 | n/a |
| variant-03/05-budget-unsat | 10.6 | 10.5 | n/a |
| variant-02/05-larger-asymmetric | 10.6 | 10.6 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.6 | 10.4 | n/a |
| variant-02/01-basic | 10.4 | 10.4 | n/a |
| n-queens/variant-05 | 11.7 | 10.6 | n/a |
| variant-01/02-multiple-tours | 11.6 | 10.5 | n/a |
| variant-02/07-layered-dag-before-after | 12.0 | 10.7 | n/a |
| variant-02/05-ordering-unsat | 10.6 | 10.4 | n/a |
| variant-02/02-single-salesman | 10.4 | 10.4 | n/a |
| variant-03/06-unreachable-edge | 10.7 | 10.5 | n/a |
| variant-04/04-window-too-tight-unsat | 11.1 | 10.5 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 10.9 | 10.5 | n/a |
| variant-04/06-unreachable-edge | 11.3 | 10.4 | n/a |
| variant-01/07-cycles | 10.9 | 10.6 | n/a |
| variant-03/05-larger-mix | 10.7 | 10.3 | n/a |
| variant-03/04-equal-cost-split | 10.7 | 10.3 | n/a |
| variant-02/06-layered-dag-before | 12.0 | 10.7 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.3 | 10.7 | n/a |
| variant-04/01-basic | 10.9 | 10.5 | n/a |
| variant-04/04-ordering-violates-budget | 10.9 | 10.5 | n/a |
| variant-04/02-precedence | 11.2 | 10.8 | n/a |
| variant-03/07-layered-dag-tight-cap | 13.8 | 10.8 | n/a |
| variant-04/07-layered-dag-combined | 12.9 | 11.0 | n/a |
| variant-02/04-after-forces-extension | 10.5 | 10.6 | n/a |
| n-queens/variant-01 | 11.4 | 10.5 | n/a |
| equality-generalized-tsp/02-larger | 11.2 | 10.6 | n/a |
| variant-03/04-incompatible-group-unsat | 10.5 | 10.4 | n/a |
| variant-01/06-layered-dag | 12.3 | 10.7 | n/a |
| variant-04/09-negative-weights | 11.0 | 10.5 | n/a |
| variant-05/05-three-depots-mixed | 11.1 | 10.3 | n/a |
| variant-01/05-ring | 13.8 | 10.7 | n/a |
| variant-05/06-unreachable-edge | 11.0 | 10.5 | n/a |
| variant-04/06-layered-dag-ordering-cap | 12.5 | 11.0 | n/a |
| n-queens/variant-06 | 11.7 | 10.3 | n/a |
| variant-05/01-basic | 10.6 | 10.4 | n/a |
| variant-02/05-larger-mix | 11.5 | 10.6 | n/a |
| variant-01/03-asymmetric | 11.7 | 10.6 | n/a |
| variant-03/06-layered-dag-cap | 16.7 | 11.0 | n/a |
