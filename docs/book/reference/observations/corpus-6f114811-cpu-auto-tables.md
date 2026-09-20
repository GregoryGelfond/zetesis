Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | corpus | corpus/reference |
|---|---:|---:|
| equality-generalized-tsp/01-basic | 6.669 [5.570, 6.856] | 1.505 |
| equality-generalized-tsp/02-larger | 6.621 [6.115, 7.453] | 1.470 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 6.205 [5.005, 6.259] | 1.385 |
| variant-01/01-basic | 5.076 [5.049, 5.284] | 1.147 |
| variant-01/02-start-equals-end | 5.888 [5.282, 5.894] | 1.327 |
| variant-01/03-zero-cost-detour | 5.502 [4.383, 6.157] | 1.235 |
| variant-01/04-no-path | 5.044 [4.985, 5.542] | 0.922 |
| variant-01/05-multi-path | 5.200 [5.169, 5.623] | 0.946 |
| variant-01/06-layered-dag | 7.964 [7.850, 8.263] | 1.450 |
| variant-01/07-cycles | 6.558 [5.920, 7.316] | 1.210 |
| variant-01/08-negative-weights | 5.550 [5.116, 6.322] | 1.258 |
| variant-02/01-basic | 4.837 [4.498, 5.120] | 1.088 |
| variant-02/02-start-equals-end | 6.006 [4.972, 6.158] | 1.363 |
| variant-02/03-before-forces-detour | 5.487 [4.895, 5.535] | 1.230 |
| variant-02/04-after-forces-extension | 5.909 [5.902, 6.112] | 1.330 |
| variant-02/05-ordering-unsat | 4.937 [4.896, 5.706] | 1.114 |
| variant-02/06-layered-dag-before | 8.703 [7.622, 9.771] | 1.581 |
| variant-02/07-layered-dag-before-after | 7.665 [7.053, 8.723] | 1.405 |
| variant-02/08-tie-break-under-ordering | 6.628 [6.428, 6.696] | 1.492 |
| variant-02/09-negative-weights | 5.947 [5.682, 6.330] | 1.338 |
| variant-03/01-basic | 5.196 [5.028, 5.688] | 1.173 |
| variant-03/02-start-equals-end | 5.597 [4.407, 6.091] | 1.269 |
| variant-03/03-budget-forces-detour | 5.550 [4.594, 6.538] | 1.241 |
| variant-03/04-cost-at-cap-allowed | 5.196 [5.066, 5.247] | 0.950 |
| variant-03/05-budget-unsat | 5.810 [5.411, 5.822] | 1.057 |
| variant-03/06-layered-dag-cap | 11.385 [10.272, 11.716] | 1.738 |
| variant-03/07-layered-dag-tight-cap | 11.996 [10.816, 12.201] | 1.824 |
| variant-03/08-tie-break-under-cap | 5.785 [5.417, 5.874] | 1.051 |
| variant-03/09-negative-weights | 5.828 [5.445, 5.960] | 1.292 |
| variant-04/01-basic | 5.714 [5.524, 5.927] | 1.044 |
| variant-04/02-start-equals-end | 5.514 [4.796, 5.538] | 1.251 |
| variant-04/03-before-forces-detour-within-budget | 5.144 [4.611, 5.590] | 1.159 |
| variant-04/04-ordering-violates-budget | 6.322 [5.927, 6.493] | 1.423 |
| variant-04/05-after-and-budget-interact | 5.686 [5.353, 6.778] | 1.277 |
| variant-04/06-layered-dag-ordering-cap | 10.530 [10.053, 12.380] | 1.382 |
| variant-04/07-layered-dag-combined | 9.779 [9.396, 10.297] | 1.482 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.605 [5.954, 6.625] | 1.203 |
| variant-04/09-negative-weights | 6.873 [6.763, 7.086] | 1.268 |
| variant-01/01-basic | 5.535 [4.466, 5.785] | 1.250 |
| variant-01/02-agent-reuse | 5.053 [4.893, 5.459] | 1.138 |
| variant-01/03-selective-compatibility | 5.809 [3.969, 5.864] | 1.307 |
| variant-01/04-no-compatible-agent-unsat | 4.748 [4.429, 4.751] | 1.069 |
| variant-01/05-larger-mix | 5.970 [5.432, 6.219] | 1.346 |
| variant-02/01-basic | 5.507 [5.317, 6.027] | 1.257 |
| variant-02/02-makespan-tiebreak | 5.616 [5.542, 6.197] | 1.255 |
| variant-02/03-cost-dominates | 6.134 [5.792, 6.317] | 1.387 |
| variant-02/04-no-compatible-agent-unsat | 5.041 [4.824, 5.485] | 1.132 |
| variant-02/05-larger-mix | 8.727 [8.702, 9.429] | 1.582 |
| variant-03/01-basic | 5.681 [5.523, 6.613] | 1.283 |
| variant-03/02-multiple-groups | 5.567 [5.080, 5.834] | 1.266 |
| variant-03/03-mixed-grouped-ungrouped | 5.610 [4.721, 5.900] | 1.253 |
| variant-03/04-incompatible-group-unsat | 5.698 [5.221, 6.549] | 1.283 |
| variant-03/05-larger-mix | 6.755 [5.709, 6.972] | 1.513 |
| variant-04/01-basic | 9.504 [8.732, 10.245] | 0.370 |
| variant-04/02-precedence | 7.533 [7.119, 7.702] | 1.135 |
| variant-04/03-agent-serialization | 8.389 [7.725, 8.871] | 0.302 |
| variant-04/04-window-too-tight-unsat | 6.655 [6.639, 6.988] | 1.204 |
| variant-04/05-larger-mix | 33.145 [32.039, 33.913] | 0.223 |
| variant-01/01-basic | 5.457 [5.039, 5.769] | 1.169 |
| variant-01/02-multiple-tours | 6.218 [5.468, 7.575] | 1.408 |
| variant-01/03-asymmetric | 6.838 [6.688, 7.690] | 1.244 |
| variant-01/04-subtour-unsat | 4.902 [4.405, 5.056] | 1.124 |
| variant-01/05-ring | 8.759 [7.640, 9.344] | 1.582 |
| variant-02/01-basic | 5.792 [5.781, 6.591] | 1.304 |
| variant-02/02-single-salesman | 5.803 [5.543, 5.863] | 1.305 |
| variant-02/03-too-many-salesmen-unsat | 5.496 [5.457, 5.646] | 1.238 |
| variant-02/04-equal-cost-split | 5.758 [5.527, 5.787] | 1.290 |
| variant-02/05-larger-asymmetric | 5.828 [4.920, 6.337] | 1.313 |
| variant-02/06-unreachable-edge | 6.469 [5.993, 6.665] | 1.444 |
| variant-03/01-basic | 5.771 [5.579, 5.909] | 1.305 |
| variant-03/02-single-salesman | 4.895 [4.479, 6.191] | 1.106 |
| variant-03/03-depot-crossing-unsat | 5.594 [4.374, 6.009] | 1.261 |
| variant-03/04-equal-cost-split | 6.683 [5.886, 6.848] | 1.517 |
| variant-03/05-larger-three-depots | 6.263 [6.210, 6.968] | 1.133 |
| variant-03/06-unreachable-edge | 6.030 [5.701, 6.101] | 1.353 |
| variant-04/01-basic | 6.227 [6.211, 6.742] | 1.140 |
| variant-04/02-single-salesman | 7.003 [6.821, 7.677] | 1.577 |
| variant-04/03-window-too-tight-unsat | 6.016 [5.570, 7.605] | 1.093 |
| variant-04/04-depot-window-too-tight-unsat | 6.545 [5.008, 6.621] | 1.455 |
| variant-04/05-three-depots-asymmetric-times | 7.881 [7.766, 8.269] | 1.788 |
| variant-04/06-unreachable-edge | 8.030 [7.792, 8.360] | 1.824 |
| variant-05/01-basic | 7.048 [5.946, 7.622] | 1.289 |
| variant-05/02-tight-bound-exact | 6.360 [5.646, 7.210] | 1.161 |
| variant-05/03-revisit-too-tight-unsat | 6.132 [5.675, 6.795] | 1.121 |
| variant-05/04-depot-and-vertex-revisits | 6.158 [6.130, 7.027] | 1.129 |
| variant-05/05-three-depots-mixed | 7.804 [7.277, 8.731] | 1.420 |
| variant-05/06-unreachable-edge | 7.630 [6.876, 7.856] | 1.387 |
| n-queens/variant-01 | 8.080 [7.614, 9.341] | 1.415 |
| n-queens/variant-02 | 62.547 [61.568, 62.570] | 0.529 |
| n-queens/variant-03 | 8.970 [8.708, 9.383] | 1.370 |
| n-queens/variant-04 | 7.394 [6.719, 8.464] | 1.329 |
| n-queens/variant-05 | 8.710 [8.316, 8.808] | 1.296 |
| n-queens/variant-06 | 9.525 [8.869, 9.836] | 1.232 |
| send-money/send-money | 13.214 [13.140, 13.888] | 0.878 |

Reference wall time, ms, same notation.

| Cell | corpus |
|---|---:|
| equality-generalized-tsp/01-basic | 4.432 [4.402, 4.539] |
| equality-generalized-tsp/02-larger | 4.504 [4.417, 5.753] |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.481 [4.447, 5.507] |
| variant-01/01-basic | 4.426 [4.407, 4.437] |
| variant-01/02-start-equals-end | 4.438 [3.285, 4.452] |
| variant-01/03-zero-cost-detour | 4.455 [4.448, 5.535] |
| variant-01/04-no-path | 5.471 [4.440, 5.477] |
| variant-01/05-multi-path | 5.498 [4.379, 6.677] |
| variant-01/06-layered-dag | 5.491 [5.450, 5.560] |
| variant-01/07-cycles | 5.421 [4.441, 6.478] |
| variant-01/08-negative-weights | 4.411 [3.338, 5.462] |
| variant-02/01-basic | 4.448 [4.351, 4.489] |
| variant-02/02-start-equals-end | 4.406 [3.323, 4.482] |
| variant-02/03-before-forces-detour | 4.463 [3.289, 4.482] |
| variant-02/04-after-forces-extension | 4.443 [4.377, 4.460] |
| variant-02/05-ordering-unsat | 4.433 [3.294, 5.477] |
| variant-02/06-layered-dag-before | 5.503 [4.387, 5.513] |
| variant-02/07-layered-dag-before-after | 5.456 [5.418, 6.536] |
| variant-02/08-tie-break-under-ordering | 4.441 [3.322, 5.531] |
| variant-02/09-negative-weights | 4.444 [4.435, 4.564] |
| variant-03/01-basic | 4.430 [3.363, 5.460] |
| variant-03/02-start-equals-end | 4.410 [3.321, 4.441] |
| variant-03/03-budget-forces-detour | 4.473 [4.388, 5.506] |
| variant-03/04-cost-at-cap-allowed | 5.469 [4.428, 5.516] |
| variant-03/05-budget-unsat | 5.497 [4.523, 5.586] |
| variant-03/06-layered-dag-cap | 6.551 [5.400, 6.662] |
| variant-03/07-layered-dag-tight-cap | 6.576 [5.455, 6.586] |
| variant-03/08-tie-break-under-cap | 5.504 [4.467, 6.543] |
| variant-03/09-negative-weights | 4.511 [4.409, 5.482] |
| variant-04/01-basic | 5.475 [4.443, 6.535] |
| variant-04/02-start-equals-end | 4.406 [4.403, 4.450] |
| variant-04/03-before-forces-detour-within-budget | 4.438 [4.397, 5.488] |
| variant-04/04-ordering-violates-budget | 4.443 [4.438, 4.489] |
| variant-04/05-after-and-budget-interact | 4.454 [4.413, 5.510] |
| variant-04/06-layered-dag-ordering-cap | 7.622 [6.500, 7.684] |
| variant-04/07-layered-dag-combined | 6.598 [6.552, 7.626] |
| variant-04/08-tie-break-under-ordering-and-cap | 5.492 [5.486, 5.496] |
| variant-04/09-negative-weights | 5.420 [4.446, 5.481] |
| variant-01/01-basic | 4.427 [4.387, 4.483] |
| variant-01/02-agent-reuse | 4.442 [4.361, 4.481] |
| variant-01/03-selective-compatibility | 4.443 [4.367, 4.449] |
| variant-01/04-no-compatible-agent-unsat | 4.440 [3.336, 5.460] |
| variant-01/05-larger-mix | 4.437 [3.324, 4.447] |
| variant-02/01-basic | 4.383 [4.350, 4.500] |
| variant-02/02-makespan-tiebreak | 4.476 [4.422, 4.542] |
| variant-02/03-cost-dominates | 4.423 [4.353, 4.471] |
| variant-02/04-no-compatible-agent-unsat | 4.453 [4.364, 5.491] |
| variant-02/05-larger-mix | 5.517 [5.501, 5.579] |
| variant-03/01-basic | 4.427 [4.410, 4.433] |
| variant-03/02-multiple-groups | 4.397 [4.394, 4.403] |
| variant-03/03-mixed-grouped-ungrouped | 4.478 [4.406, 4.707] |
| variant-03/04-incompatible-group-unsat | 4.441 [4.437, 4.456] |
| variant-03/05-larger-mix | 4.464 [4.391, 5.460] |
| variant-04/01-basic | 25.712 [25.710, 26.690] |
| variant-04/02-precedence | 6.638 [6.580, 7.784] |
| variant-04/03-agent-serialization | 27.790 [27.739, 27.913] |
| variant-04/04-window-too-tight-unsat | 5.525 [4.458, 5.554] |
| variant-04/05-larger-mix | 148.321 [145.922, 148.518] |
| variant-01/01-basic | 4.667 [4.643, 5.689] |
| variant-01/02-multiple-tours | 4.417 [3.554, 5.518] |
| variant-01/03-asymmetric | 5.496 [5.469, 5.551] |
| variant-01/04-subtour-unsat | 4.362 [3.327, 4.447] |
| variant-01/05-ring | 5.536 [5.432, 5.547] |
| variant-02/01-basic | 4.441 [4.410, 5.469] |
| variant-02/02-single-salesman | 4.445 [4.352, 4.479] |
| variant-02/03-too-many-salesmen-unsat | 4.441 [4.439, 5.536] |
| variant-02/04-equal-cost-split | 4.464 [4.417, 5.527] |
| variant-02/05-larger-asymmetric | 4.437 [4.404, 5.442] |
| variant-02/06-unreachable-edge | 4.481 [4.444, 5.480] |
| variant-03/01-basic | 4.423 [4.372, 4.490] |
| variant-03/02-single-salesman | 4.424 [3.330, 5.467] |
| variant-03/03-depot-crossing-unsat | 4.436 [4.384, 5.499] |
| variant-03/04-equal-cost-split | 4.405 [3.319, 4.439] |
| variant-03/05-larger-three-depots | 5.526 [5.474, 5.717] |
| variant-03/06-unreachable-edge | 4.456 [4.447, 4.467] |
| variant-04/01-basic | 5.464 [5.442, 5.503] |
| variant-04/02-single-salesman | 4.441 [4.421, 4.517] |
| variant-04/03-window-too-tight-unsat | 5.503 [4.491, 5.529] |
| variant-04/04-depot-window-too-tight-unsat | 4.498 [4.439, 5.494] |
| variant-04/05-three-depots-asymmetric-times | 4.407 [4.380, 5.521] |
| variant-04/06-unreachable-edge | 4.402 [4.367, 5.497] |
| variant-05/01-basic | 5.467 [4.397, 5.484] |
| variant-05/02-tight-bound-exact | 5.479 [4.362, 5.576] |
| variant-05/03-revisit-too-tight-unsat | 5.471 [4.383, 5.481] |
| variant-05/04-depot-and-vertex-revisits | 5.457 [4.439, 5.495] |
| variant-05/05-three-depots-mixed | 5.495 [5.464, 6.585] |
| variant-05/06-unreachable-edge | 5.501 [4.447, 5.513] |
| n-queens/variant-01 | 5.709 [5.543, 6.626] |
| n-queens/variant-02 | 118.294 [116.802, 119.205] |
| n-queens/variant-03 | 6.546 [5.632, 7.699] |
| n-queens/variant-04 | 5.564 [5.507, 6.603] |
| n-queens/variant-05 | 6.721 [6.589, 7.627] |
| n-queens/variant-06 | 7.734 [5.550, 8.738] |
| send-money/send-money | 15.050 [15.041, 15.136] |

Counters of report corpus: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 0 | 1 | 4 | 8982 | 2.906 |
| equality-generalized-tsp/02-larger | 0 | 1 | 7 | 23383 | 2.792 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 0 | 0 | 0 | 2258 | 1.560 |
| variant-01/01-basic | 0 | 1 | 2 | 1559 | 1.485 |
| variant-01/02-start-equals-end | 0 | 1 | 1 | 1208 | 1.767 |
| variant-01/03-zero-cost-detour | 0 | 1 | 2 | 1291 | 1.547 |
| variant-01/04-no-path | 0 | 0 | 0 | 1085 | 1.398 |
| variant-01/05-multi-path | 0 | 1 | 3 | 3958 | 1.704 |
| variant-01/06-layered-dag | 0 | 1 | 16 | 140606 | 4.914 |
| variant-01/07-cycles | 0 | 1 | 5 | 13987 | 2.403 |
| variant-01/08-negative-weights | 0 | 1 | 1 | 2442 | 1.789 |
| variant-02/01-basic | 0 | 1 | 2 | 1700 | 1.460 |
| variant-02/02-start-equals-end | 0 | 1 | 1 | 1557 | 1.606 |
| variant-02/03-before-forces-detour | 0 | 1 | 1 | 2383 | 1.612 |
| variant-02/04-after-forces-extension | 0 | 1 | 1 | 2245 | 1.882 |
| variant-02/05-ordering-unsat | 0 | 0 | 0 | 1322 | 1.461 |
| variant-02/06-layered-dag-before | 0 | 2 | 9 | 48810 | 4.396 |
| variant-02/07-layered-dag-before-after | 0 | 1 | 7 | 42762 | 3.656 |
| variant-02/08-tie-break-under-ordering | 0 | 1 | 2 | 2777 | 2.593 |
| variant-02/09-negative-weights | 0 | 1 | 1 | 2816 | 2.057 |
| variant-03/01-basic | 0 | 1 | 2 | 1667 | 1.891 |
| variant-03/02-start-equals-end | 0 | 1 | 1 | 1467 | 1.785 |
| variant-03/03-budget-forces-detour | 0 | 1 | 1 | 2141 | 1.611 |
| variant-03/04-cost-at-cap-allowed | 0 | 1 | 1 | 1272 | 1.556 |
| variant-03/05-budget-unsat | 0 | 0 | 0 | 4970 | 1.901 |
| variant-03/06-layered-dag-cap | 0 | 2 | 14 | 276160 | 7.369 |
| variant-03/07-layered-dag-tight-cap | 0 | 4 | 6 | 269165 | 7.913 |
| variant-03/08-tie-break-under-cap | 0 | 1 | 2 | 3301 | 1.835 |
| variant-03/09-negative-weights | 0 | 1 | 1 | 2872 | 2.016 |
| variant-04/01-basic | 0 | 1 | 2 | 1808 | 1.817 |
| variant-04/02-start-equals-end | 0 | 1 | 1 | 1816 | 1.741 |
| variant-04/03-before-forces-detour-within-budget | 0 | 1 | 1 | 2774 | 1.806 |
| variant-04/04-ordering-violates-budget | 0 | 0 | 0 | 2816 | 2.218 |
| variant-04/05-after-and-budget-interact | 0 | 1 | 1 | 4169 | 2.192 |
| variant-04/06-layered-dag-ordering-cap | 0 | 2 | 8 | 69872 | 6.168 |
| variant-04/07-layered-dag-combined | 0 | 1 | 5 | 60073 | 5.833 |
| variant-04/08-tie-break-under-ordering-and-cap | 0 | 1 | 2 | 6554 | 2.447 |
| variant-04/09-negative-weights | 0 | 1 | 1 | 3246 | 3.116 |
| variant-01/01-basic | 0 | 1 | 4 | 1462 | 1.203 |
| variant-01/02-agent-reuse | 0 | 1 | 8 | 2131 | 1.236 |
| variant-01/03-selective-compatibility | 0 | 1 | 4 | 1873 | 1.149 |
| variant-01/04-no-compatible-agent-unsat | 0 | 0 | 0 | 1217 | 1.069 |
| variant-01/05-larger-mix | 0 | 1 | 38 | 33000 | 2.063 |
| variant-02/01-basic | 0 | 1 | 4 | 3047 | 1.626 |
| variant-02/02-makespan-tiebreak | 0 | 3 | 8 | 4031 | 1.887 |
| variant-02/03-cost-dominates | 0 | 1 | 8 | 4006 | 2.146 |
| variant-02/04-no-compatible-agent-unsat | 0 | 0 | 0 | 2395 | 1.535 |
| variant-02/05-larger-mix | 0 | 1 | 11 | 217145 | 5.182 |
| variant-03/01-basic | 0 | 2 | 2 | 1970 | 1.883 |
| variant-03/02-multiple-groups | 0 | 4 | 4 | 3816 | 1.666 |
| variant-03/03-mixed-grouped-ungrouped | 0 | 2 | 4 | 2639 | 1.692 |
| variant-03/04-incompatible-group-unsat | 0 | 0 | 0 | 1224 | 1.763 |
| variant-03/05-larger-mix | 0 | 1 | 27 | 5137 | 2.454 |
| variant-04/01-basic | 0 | 81 | 101 | 111703 | 5.561 |
| variant-04/02-precedence | 0 | 3 | 11 | 13804 | 3.733 |
| variant-04/03-agent-serialization | 0 | 30 | 30 | 32215 | 4.639 |
| variant-04/04-window-too-tight-unsat | 0 | 0 | 0 | 2865 | 2.899 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 2115171 | 29.229 |
| variant-01/01-basic | 0 | 1 | 2 | 5138 | 1.827 |
| variant-01/02-multiple-tours | 0 | 2 | 9 | 35544 | 2.692 |
| variant-01/03-asymmetric | 0 | 1 | 8 | 52685 | 3.122 |
| variant-01/04-subtour-unsat | 0 | 0 | 0 | 2473 | 1.266 |
| variant-01/05-ring | 0 | 2 | 9 | 179205 | 4.698 |
| variant-02/01-basic | 0 | 2 | 2 | 5689 | 1.949 |
| variant-02/02-single-salesman | 0 | 1 | 2 | 3644 | 1.780 |
| variant-02/03-too-many-salesmen-unsat | 0 | 0 | 0 | 3466 | 1.490 |
| variant-02/04-equal-cost-split | 0 | 2 | 2 | 4647 | 1.922 |
| variant-02/05-larger-asymmetric | 0 | 2 | 2 | 6731 | 2.036 |
| variant-02/06-unreachable-edge | 0 | 2 | 2 | 7627 | 2.280 |
| variant-03/01-basic | 0 | 1 | 1 | 3939 | 1.778 |
| variant-03/02-single-salesman | 0 | 1 | 2 | 3498 | 1.751 |
| variant-03/03-depot-crossing-unsat | 0 | 0 | 0 | 3422 | 1.871 |
| variant-03/04-equal-cost-split | 0 | 2 | 2 | 7949 | 2.199 |
| variant-03/05-larger-three-depots | 0 | 1 | 1 | 10821 | 2.741 |
| variant-03/06-unreachable-edge | 0 | 1 | 1 | 6365 | 2.231 |
| variant-04/01-basic | 0 | 1 | 1 | 6955 | 2.796 |
| variant-04/02-single-salesman | 0 | 1 | 1 | 4700 | 3.200 |
| variant-04/03-window-too-tight-unsat | 0 | 0 | 0 | 3400 | 2.087 |
| variant-04/04-depot-window-too-tight-unsat | 0 | 0 | 0 | 2325 | 2.243 |
| variant-04/05-three-depots-asymmetric-times | 0 | 1 | 1 | 18328 | 4.297 |
| variant-04/06-unreachable-edge | 0 | 1 | 1 | 14062 | 4.053 |
| variant-05/01-basic | 0 | 1 | 1 | 6957 | 3.173 |
| variant-05/02-tight-bound-exact | 0 | 1 | 1 | 3439 | 2.548 |
| variant-05/03-revisit-too-tight-unsat | 0 | 0 | 0 | 3413 | 2.241 |
| variant-05/04-depot-and-vertex-revisits | 0 | 1 | 1 | 3504 | 2.390 |
| variant-05/05-three-depots-mixed | 0 | 1 | 1 | 18378 | 4.187 |
| variant-05/06-unreachable-edge | 0 | 1 | 1 | 14062 | 3.816 |
| n-queens/variant-01 | 0 | 92 | 92 | 199032 | 5.123 |
| n-queens/variant-02 | 0 | 92 | 92 | 14461401 | 58.305 |
| n-queens/variant-03 | 0 | 92 | 92 | 256834 | 5.450 |
| n-queens/variant-04 | 0 | 92 | 92 | 195309 | 3.852 |
| n-queens/variant-05 | 0 | 92 | 92 | 223549 | 4.869 |
| n-queens/variant-06 | 0 | 92 | 92 | 251111 | 5.820 |
| send-money/send-money | 0 | 1 | 1 | 409024 | 9.482 |

Against the reference: report corpus, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 8 of 94 cells where both passed (8.5%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 33.145 | 148.321 | 0.223 | 4.246 | 31.597 | 11.550 | 84.000 | 57.000 |
| variant-04/03-agent-serialization | 8.389 | 27.790 | 0.302 | 0.918 | 1.415 | 0.285 | 24.000 | 0.000 |
| variant-04/01-basic | 9.504 | 25.712 | 0.370 | 0.992 | 3.129 | 0.624 | 18.000 | 4.000 |
| n-queens/variant-02 | 62.547 | 118.294 | 0.529 | 2.455 | 172.646 | 1.184 | 1.000 | 113.000 |
| send-money/send-money | 13.214 | 15.050 | 0.878 | 2.561 | 6.819 | 0.497 | 9.000 | 1.000 |
| variant-01/04-no-path | 5.044 | 5.471 | 0.922 | 0.169 | 0.036 | 0.005 | 1.000 | 0.000 |
| variant-01/05-multi-path | 5.200 | 5.498 | 0.946 | 0.300 | 0.139 | 0.027 | 1.000 | 0.000 |
| variant-03/04-cost-at-cap-allowed | 5.196 | 5.469 | 0.950 | 0.194 | 0.049 | 0.008 | 1.000 | 0.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/01-basic | 5.714 | 5.475 | 1.044 | 0.195 | 0.077 | 0.017 | 1.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 5.785 | 5.504 | 1.051 | 0.277 | 0.105 | 0.021 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 5.810 | 5.497 | 1.057 | 0.359 | 0.134 | 0.014 | 1.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 4.748 | 4.440 | 1.069 | 0.125 | 0.031 | 0.006 | 0.000 | 0.000 |
| variant-02/01-basic | 4.837 | 4.448 | 1.088 | 0.180 | 0.072 | 0.017 | 1.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 6.016 | 5.503 | 1.093 | 0.369 | 0.062 | 0.011 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 4.895 | 4.424 | 1.106 | 0.258 | 0.097 | 0.083 | 1.000 | 0.000 |
| variant-02/05-ordering-unsat | 4.937 | 4.433 | 1.114 | 0.172 | 0.045 | 0.005 | 1.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 6.132 | 5.471 | 1.121 | 0.358 | 0.070 | 0.011 | 1.000 | 0.000 |
| variant-01/04-subtour-unsat | 4.902 | 4.362 | 1.124 | 0.211 | 0.039 | 0.010 | 0.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 6.158 | 5.457 | 1.129 | 0.392 | 0.076 | 0.048 | 1.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 5.041 | 4.453 | 1.132 | 0.283 | 0.051 | 0.008 | 0.000 | 0.000 |
| variant-03/05-larger-three-depots | 6.263 | 5.526 | 1.133 | 0.822 | 0.165 | 0.119 | 1.000 | 0.000 |
| variant-04/02-precedence | 7.533 | 6.638 | 1.135 | 0.614 | 0.796 | 0.125 | 2.000 | 0.000 |
| variant-01/02-agent-reuse | 5.053 | 4.442 | 1.138 | 0.166 | 0.110 | 0.031 | 0.000 | 0.000 |
| variant-04/01-basic | 6.227 | 5.464 | 1.140 | 0.614 | 0.164 | 0.096 | 1.000 | 0.000 |
| variant-01/01-basic | 5.076 | 4.426 | 1.147 | 0.185 | 0.076 | 0.015 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 5.144 | 4.438 | 1.159 | 0.265 | 0.072 | 0.014 | 1.000 | 0.000 |
| variant-05/02-tight-bound-exact | 6.360 | 5.479 | 1.161 | 0.380 | 0.075 | 0.048 | 1.000 | 0.000 |
| variant-01/01-basic | 5.457 | 4.667 | 1.169 | 0.270 | 0.148 | 0.117 | 1.000 | 0.000 |
| variant-03/01-basic | 5.196 | 4.430 | 1.173 | 0.289 | 0.090 | 0.021 | 1.000 | 0.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.605 | 5.492 | 1.203 | 0.460 | 0.196 | 0.040 | 1.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 6.655 | 5.525 | 1.204 | 0.319 | 0.094 | 0.011 | 1.000 | 0.000 |
| variant-01/07-cycles | 6.558 | 5.421 | 1.210 | 0.365 | 0.264 | 0.376 | 1.000 | 0.000 |
| variant-02/03-before-forces-detour | 5.487 | 4.463 | 1.230 | 0.224 | 0.065 | 0.013 | 1.000 | 0.000 |
| n-queens/variant-06 | 9.525 | 7.734 | 1.232 | 1.074 | 5.459 | 0.915 | 1.000 | 1.000 |
| variant-01/03-zero-cost-detour | 5.502 | 4.455 | 1.235 | 0.189 | 0.054 | 0.010 | 1.000 | 0.000 |
| variant-02/03-too-many-salesmen-unsat | 5.496 | 4.441 | 1.238 | 0.283 | 0.067 | 0.011 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 5.550 | 4.473 | 1.241 | 0.223 | 0.063 | 0.013 | 1.000 | 0.000 |
| variant-01/03-asymmetric | 6.838 | 5.496 | 1.244 | 0.541 | 0.856 | 1.272 | 1.000 | 0.000 |
| variant-01/01-basic | 5.535 | 4.427 | 1.250 | 0.144 | 0.079 | 0.019 | 0.000 | 0.000 |
| variant-04/02-start-equals-end | 5.514 | 4.406 | 1.251 | 0.207 | 0.052 | 0.031 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 5.610 | 4.478 | 1.253 | 0.190 | 0.096 | 0.024 | 0.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 5.616 | 4.476 | 1.255 | 0.353 | 0.269 | 0.063 | 1.000 | 0.000 |
| variant-02/01-basic | 5.507 | 4.383 | 1.257 | 0.280 | 0.148 | 0.029 | 1.000 | 0.000 |
| variant-01/08-negative-weights | 5.550 | 4.411 | 1.258 | 0.255 | 0.076 | 0.043 | 1.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 5.594 | 4.436 | 1.261 | 0.300 | 0.060 | 0.010 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 5.567 | 4.397 | 1.266 | 0.223 | 0.112 | 0.031 | 1.000 | 0.000 |
| variant-04/09-negative-weights | 6.873 | 5.420 | 1.268 | 0.543 | 0.096 | 0.058 | 1.000 | 0.000 |
| variant-03/02-start-equals-end | 5.597 | 4.410 | 1.269 | 0.167 | 0.042 | 0.027 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 5.686 | 4.454 | 1.277 | 0.335 | 0.100 | 0.020 | 1.000 | 0.000 |
| variant-03/01-basic | 5.681 | 4.427 | 1.283 | 0.195 | 0.073 | 0.015 | 0.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 5.698 | 4.441 | 1.283 | 0.117 | 0.048 | 0.005 | 0.000 | 0.000 |
| variant-05/01-basic | 7.048 | 5.467 | 1.289 | 0.591 | 0.120 | 0.083 | 1.000 | 0.000 |
| variant-02/04-equal-cost-split | 5.758 | 4.464 | 1.290 | 0.315 | 0.129 | 0.100 | 1.000 | 0.000 |
| variant-03/09-negative-weights | 5.828 | 4.511 | 1.292 | 0.302 | 0.090 | 0.054 | 1.000 | 0.000 |
| n-queens/variant-05 | 8.710 | 6.721 | 1.296 | 1.005 | 4.952 | 0.825 | 1.000 | 1.000 |
| variant-02/01-basic | 5.792 | 4.441 | 1.304 | 0.355 | 0.139 | 0.127 | 1.000 | 0.000 |
| variant-03/01-basic | 5.771 | 4.423 | 1.305 | 0.311 | 0.088 | 0.057 | 1.000 | 0.000 |
| variant-02/02-single-salesman | 5.803 | 4.445 | 1.305 | 0.246 | 0.102 | 0.090 | 1.000 | 0.000 |
| variant-01/03-selective-compatibility | 5.809 | 4.443 | 1.307 | 0.172 | 0.071 | 0.018 | 0.000 | 0.000 |
| variant-02/05-larger-asymmetric | 5.828 | 4.437 | 1.313 | 0.446 | 0.159 | 0.146 | 1.000 | 0.000 |
| variant-01/02-start-equals-end | 5.888 | 4.438 | 1.327 | 0.159 | 0.039 | 0.026 | 1.000 | 0.000 |
| n-queens/variant-04 | 7.394 | 5.564 | 1.329 | 0.724 | 4.939 | 0.669 | 1.000 | 1.000 |
| variant-02/04-after-forces-extension | 5.909 | 4.443 | 1.330 | 0.234 | 0.066 | 0.012 | 1.000 | 0.000 |
| variant-02/09-negative-weights | 5.947 | 4.444 | 1.338 | 0.269 | 0.083 | 0.048 | 1.000 | 0.000 |
| variant-01/05-larger-mix | 5.970 | 4.437 | 1.346 | 0.254 | 1.435 | 0.143 | 0.000 | 0.000 |
| variant-03/06-unreachable-edge | 6.030 | 4.456 | 1.353 | 0.528 | 0.123 | 0.079 | 1.000 | 0.000 |
| variant-02/02-start-equals-end | 6.006 | 4.406 | 1.363 | 0.181 | 0.051 | 0.028 | 1.000 | 0.000 |
| n-queens/variant-03 | 8.970 | 6.546 | 1.370 | 1.836 | 5.360 | 0.844 | 1.000 | 1.000 |
| variant-04/06-layered-dag-ordering-cap | 10.530 | 7.622 | 1.382 | 1.779 | 2.170 | 0.412 | 3.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 6.205 | 4.481 | 1.385 | 0.189 | 0.040 | 0.009 | 1.000 | 0.000 |
| variant-05/06-unreachable-edge | 7.630 | 5.501 | 1.387 | 1.179 | 0.208 | 0.178 | 1.000 | 0.000 |
| variant-02/03-cost-dominates | 6.134 | 4.423 | 1.387 | 0.332 | 0.233 | 0.060 | 1.000 | 0.000 |
| variant-02/07-layered-dag-before-after | 7.665 | 5.456 | 1.405 | 1.108 | 1.522 | 0.258 | 2.000 | 0.000 |
| variant-01/02-multiple-tours | 6.218 | 4.417 | 1.408 | 0.397 | 0.575 | 1.044 | 1.000 | 0.000 |
| n-queens/variant-01 | 8.080 | 5.709 | 1.415 | 1.848 | 4.129 | 0.672 | 1.000 | 1.000 |
| variant-05/05-three-depots-mixed | 7.804 | 5.495 | 1.420 | 1.515 | 0.239 | 0.196 | 1.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 6.322 | 4.443 | 1.423 | 0.378 | 0.088 | 0.009 | 1.000 | 0.000 |
| variant-02/06-unreachable-edge | 6.469 | 4.481 | 1.444 | 0.492 | 0.179 | 0.163 | 1.000 | 0.000 |
| variant-01/06-layered-dag | 7.964 | 5.491 | 1.450 | 0.998 | 3.026 | 0.327 | 2.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 6.545 | 4.498 | 1.455 | 0.295 | 0.055 | 0.009 | 1.000 | 0.000 |
| equality-generalized-tsp/02-larger | 6.621 | 4.504 | 1.470 | 0.623 | 0.334 | 0.461 | 1.000 | 0.000 |
| variant-04/07-layered-dag-combined | 9.779 | 6.598 | 1.482 | 1.787 | 2.280 | 0.357 | 2.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 6.628 | 4.441 | 1.492 | 0.332 | 0.099 | 0.020 | 1.000 | 0.000 |
| equality-generalized-tsp/01-basic | 6.669 | 4.432 | 1.505 | 0.374 | 0.209 | 0.240 | 1.000 | 0.000 |
| variant-03/05-larger-mix | 6.755 | 4.464 | 1.513 | 0.279 | 0.605 | 0.088 | 0.000 | 0.000 |
| variant-03/04-equal-cost-split | 6.683 | 4.405 | 1.517 | 0.455 | 0.168 | 0.151 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 7.003 | 4.441 | 1.577 | 0.603 | 0.100 | 0.073 | 1.000 | 0.000 |
| variant-02/06-layered-dag-before | 8.703 | 5.503 | 1.581 | 1.118 | 1.827 | 0.292 | 2.000 | 0.000 |
| variant-02/05-larger-mix | 8.727 | 5.517 | 1.582 | 0.723 | 5.451 | 0.151 | 1.000 | 0.000 |
| variant-01/05-ring | 8.759 | 5.536 | 1.582 | 0.674 | 2.778 | 1.744 | 1.000 | 0.000 |
| variant-03/06-layered-dag-cap | 11.385 | 6.551 | 1.738 | 1.642 | 6.150 | 0.496 | 2.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 7.881 | 4.407 | 1.788 | 1.517 | 0.251 | 0.196 | 1.000 | 0.000 |
| variant-04/06-unreachable-edge | 8.030 | 4.402 | 1.824 | 1.203 | 0.231 | 0.172 | 1.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 11.996 | 6.576 | 1.824 | 1.657 | 6.456 | 0.349 | 2.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 16.1 | 23.2 | n/a |
| variant-04/03-agent-serialization | 11.3 | 12.2 | n/a |
| variant-04/01-basic | 12.1 | 13.3 | n/a |
| n-queens/variant-02 | 16.9 | 13.0 | n/a |
| send-money/send-money | 14.0 | 12.8 | n/a |
| variant-01/04-no-path | 10.4 | 10.4 | n/a |
| variant-01/05-multi-path | 10.6 | 10.4 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.6 | 10.6 | n/a |
| variant-04/01-basic | 10.4 | 10.5 | n/a |
| variant-03/08-tie-break-under-cap | 10.5 | 10.5 | n/a |
| variant-03/05-budget-unsat | 10.6 | 10.3 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.4 | 10.1 | n/a |
| variant-02/01-basic | 10.6 | 10.5 | n/a |
| variant-04/03-window-too-tight-unsat | 10.9 | 10.5 | n/a |
| variant-03/02-single-salesman | 10.4 | 10.5 | n/a |
| variant-02/05-ordering-unsat | 10.5 | 10.4 | n/a |
| variant-05/03-revisit-too-tight-unsat | 10.6 | 10.5 | n/a |
| variant-01/04-subtour-unsat | 10.6 | 10.4 | n/a |
| variant-05/04-depot-and-vertex-revisits | 10.8 | 10.5 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.4 | 10.4 | n/a |
| variant-03/05-larger-three-depots | 10.9 | 10.4 | n/a |
| variant-04/02-precedence | 11.5 | 10.8 | n/a |
| variant-01/02-agent-reuse | 10.3 | 10.4 | n/a |
| variant-04/01-basic | 10.8 | 10.5 | n/a |
| variant-01/01-basic | 10.8 | 10.5 | n/a |
| variant-04/03-before-forces-detour-within-budget | 10.7 | 10.5 | n/a |
| variant-05/02-tight-bound-exact | 11.0 | 10.5 | n/a |
| variant-01/01-basic | 10.5 | 10.6 | n/a |
| variant-03/01-basic | 10.6 | 10.5 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 11.0 | 10.7 | n/a |
| variant-04/04-window-too-tight-unsat | 11.2 | 10.3 | n/a |
| variant-01/07-cycles | 10.8 | 10.5 | n/a |
| variant-02/03-before-forces-detour | 10.8 | 10.4 | n/a |
| n-queens/variant-06 | 11.6 | 10.5 | n/a |
| variant-01/03-zero-cost-detour | 10.5 | 10.4 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.5 | 10.3 | n/a |
| variant-03/03-budget-forces-detour | 10.4 | 10.5 | n/a |
| variant-01/03-asymmetric | 12.3 | 10.7 | n/a |
| variant-01/01-basic | 10.2 | 10.1 | n/a |
| variant-04/02-start-equals-end | 10.9 | 10.5 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.6 | 10.4 | n/a |
| variant-02/02-makespan-tiebreak | 10.3 | 10.3 | n/a |
| variant-02/01-basic | 10.2 | 10.5 | n/a |
| variant-01/08-negative-weights | 10.3 | 10.3 | n/a |
| variant-03/03-depot-crossing-unsat | 10.6 | 10.3 | n/a |
| variant-03/02-multiple-groups | 10.4 | 10.4 | n/a |
| variant-04/09-negative-weights | 11.1 | 10.3 | n/a |
| variant-03/02-start-equals-end | 10.8 | 10.5 | n/a |
| variant-04/05-after-and-budget-interact | 10.7 | 10.5 | n/a |
| variant-03/01-basic | 10.6 | 10.3 | n/a |
| variant-03/04-incompatible-group-unsat | 10.6 | 10.4 | n/a |
| variant-05/01-basic | 11.1 | 10.5 | n/a |
| variant-02/04-equal-cost-split | 10.5 | 10.6 | n/a |
| variant-03/09-negative-weights | 10.7 | 10.5 | n/a |
| n-queens/variant-05 | 11.5 | 10.4 | n/a |
| variant-02/01-basic | 10.7 | 10.6 | n/a |
| variant-03/01-basic | 10.7 | 10.3 | n/a |
| variant-02/02-single-salesman | 10.5 | 10.4 | n/a |
| variant-01/03-selective-compatibility | 10.5 | 10.4 | n/a |
| variant-02/05-larger-asymmetric | 10.8 | 10.5 | n/a |
| variant-01/02-start-equals-end | 10.6 | 10.3 | n/a |
| n-queens/variant-04 | 11.2 | 10.8 | n/a |
| variant-02/04-after-forces-extension | 10.7 | 10.5 | n/a |
| variant-02/09-negative-weights | 10.8 | 10.5 | n/a |
| variant-01/05-larger-mix | 10.5 | 10.2 | n/a |
| variant-03/06-unreachable-edge | 10.8 | 10.6 | n/a |
| variant-02/02-start-equals-end | 10.7 | 10.2 | n/a |
| n-queens/variant-03 | 11.4 | 10.5 | n/a |
| variant-04/06-layered-dag-ordering-cap | 13.1 | 11.0 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.3 | 10.3 | n/a |
| variant-05/06-unreachable-edge | 11.2 | 10.5 | n/a |
| variant-02/03-cost-dominates | 10.4 | 10.3 | n/a |
| variant-02/07-layered-dag-before-after | 11.7 | 10.6 | n/a |
| variant-01/02-multiple-tours | 11.4 | 10.3 | n/a |
| n-queens/variant-01 | 11.0 | 10.6 | n/a |
| variant-05/05-three-depots-mixed | 11.2 | 10.7 | n/a |
| variant-04/04-ordering-violates-budget | 10.9 | 10.5 | n/a |
| variant-02/06-unreachable-edge | 10.8 | 10.5 | n/a |
| variant-01/06-layered-dag | 12.4 | 10.8 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 10.9 | 10.6 | n/a |
| equality-generalized-tsp/02-larger | 11.0 | 10.5 | n/a |
| variant-04/07-layered-dag-combined | 12.8 | 11.1 | n/a |
| variant-02/08-tie-break-under-ordering | 10.7 | 10.5 | n/a |
| equality-generalized-tsp/01-basic | 10.3 | 10.6 | n/a |
| variant-03/05-larger-mix | 10.3 | 10.5 | n/a |
| variant-03/04-equal-cost-split | 10.9 | 10.3 | n/a |
| variant-04/02-single-salesman | 11.1 | 10.6 | n/a |
| variant-02/06-layered-dag-before | 11.8 | 10.9 | n/a |
| variant-02/05-larger-mix | 11.9 | 10.5 | n/a |
| variant-01/05-ring | 14.3 | 10.7 | n/a |
| variant-03/06-layered-dag-cap | 13.0 | 11.0 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.3 | 10.6 | n/a |
| variant-04/06-unreachable-edge | 11.3 | 10.6 | n/a |
| variant-03/07-layered-dag-tight-cap | 17.8 | 11.0 | n/a |
