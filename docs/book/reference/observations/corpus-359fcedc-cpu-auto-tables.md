Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | corpus | corpus/reference |
|---|---:|---:|
| equality-generalized-tsp/01-basic | 6.386 [6.069, 7.432] | 1.154 |
| equality-generalized-tsp/02-larger | 6.293 [6.130, 7.291] | 1.391 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.099 [4.937, 7.056] | 0.923 |
| variant-01/01-basic | 5.865 [5.487, 6.513] | 1.329 |
| variant-01/02-start-equals-end | 5.573 [5.480, 5.788] | 1.245 |
| variant-01/03-zero-cost-detour | 6.010 [5.097, 6.229] | 1.362 |
| variant-01/04-no-path | 4.837 [4.418, 5.878] | 1.098 |
| variant-01/05-multi-path | 5.317 [4.788, 5.831] | 0.973 |
| variant-01/06-layered-dag | 8.812 [8.695, 9.792] | 1.596 |
| variant-01/07-cycles | 6.091 [6.009, 6.598] | 1.108 |
| variant-01/08-negative-weights | 5.106 [4.734, 6.157] | 1.123 |
| variant-02/01-basic | 6.141 [4.659, 6.372] | 1.379 |
| variant-02/02-start-equals-end | 6.649 [4.587, 7.035] | 1.446 |
| variant-02/03-before-forces-detour | 6.113 [5.098, 7.159] | 1.108 |
| variant-02/04-after-forces-extension | 5.550 [5.525, 7.236] | 1.238 |
| variant-02/05-ordering-unsat | 5.551 [5.177, 6.249] | 1.016 |
| variant-02/06-layered-dag-before | 8.693 [8.687, 9.108] | 1.577 |
| variant-02/07-layered-dag-before-after | 7.740 [7.683, 7.763] | 1.406 |
| variant-02/08-tie-break-under-ordering | 6.253 [5.858, 6.332] | 1.032 |
| variant-02/09-negative-weights | 5.687 [4.615, 5.695] | 1.306 |
| variant-03/01-basic | 5.515 [5.174, 6.108] | 1.254 |
| variant-03/02-start-equals-end | 5.938 [4.589, 7.059] | 1.351 |
| variant-03/03-budget-forces-detour | 5.523 [5.292, 6.323] | 1.000 |
| variant-03/04-cost-at-cap-allowed | 5.721 [5.594, 5.808] | 1.277 |
| variant-03/05-budget-unsat | 4.805 [4.730, 6.583] | 1.089 |
| variant-03/06-layered-dag-cap | 12.418 [12.123, 12.753] | 1.895 |
| variant-03/07-layered-dag-tight-cap | 11.917 [11.038, 11.963] | 1.810 |
| variant-03/08-tie-break-under-cap | 5.703 [5.550, 6.634] | 1.033 |
| variant-03/09-negative-weights | 5.885 [5.324, 6.743] | 1.330 |
| variant-04/01-basic | 5.497 [5.283, 5.568] | 0.986 |
| variant-04/02-start-equals-end | 5.863 [5.543, 5.961] | 1.316 |
| variant-04/03-before-forces-detour-within-budget | 5.719 [5.568, 7.301] | 1.285 |
| variant-04/04-ordering-violates-budget | 6.609 [4.847, 6.711] | 1.485 |
| variant-04/05-after-and-budget-interact | 5.885 [5.292, 6.693] | 1.075 |
| variant-04/06-layered-dag-ordering-cap | 10.531 [9.498, 11.216] | 1.593 |
| variant-04/07-layered-dag-combined | 10.925 [9.318, 12.948] | 1.429 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.190 [5.443, 6.787] | 1.124 |
| variant-04/09-negative-weights | 6.970 [6.227, 7.210] | 1.274 |
| variant-01/01-basic | 5.551 [4.552, 6.135] | 1.251 |
| variant-01/02-agent-reuse | 5.521 [4.666, 5.607] | 1.657 |
| variant-01/03-selective-compatibility | 4.975 [4.850, 6.125] | 1.120 |
| variant-01/04-no-compatible-agent-unsat | 4.675 [4.430, 5.248] | 1.058 |
| variant-01/05-larger-mix | 6.121 [5.648, 6.765] | 1.370 |
| variant-02/01-basic | 5.624 [5.516, 5.799] | 1.020 |
| variant-02/02-makespan-tiebreak | 5.899 [5.843, 5.920] | 1.325 |
| variant-02/03-cost-dominates | 5.957 [4.508, 6.003] | 1.340 |
| variant-02/04-no-compatible-agent-unsat | 5.748 [4.680, 6.611] | 1.286 |
| variant-02/05-larger-mix | 8.264 [7.928, 10.191] | 1.500 |
| variant-03/01-basic | 5.119 [5.036, 5.790] | 1.162 |
| variant-03/02-multiple-groups | 5.503 [4.688, 5.681] | 1.215 |
| variant-03/03-mixed-grouped-ungrouped | 5.312 [5.172, 5.471] | 1.199 |
| variant-03/04-incompatible-group-unsat | 5.352 [5.231, 6.088] | 1.202 |
| variant-03/05-larger-mix | 5.791 [5.560, 6.784] | 1.307 |
| variant-04/01-basic | 9.399 [9.307, 9.655] | 0.337 |
| variant-04/02-precedence | 7.688 [7.059, 8.167] | 1.167 |
| variant-04/03-agent-serialization | 10.146 [8.149, 11.896] | 0.339 |
| variant-04/04-window-too-tight-unsat | 6.709 [6.652, 7.944] | 1.199 |
| variant-04/05-larger-mix | 43.543 [43.085, 46.409] | 0.289 |
| variant-01/01-basic | 5.694 [5.589, 6.189] | 1.232 |
| variant-01/02-multiple-tours | 6.009 [5.202, 7.318] | 1.282 |
| variant-01/03-asymmetric | 6.697 [6.621, 8.226] | 1.163 |
| variant-01/04-subtour-unsat | 5.043 [4.692, 5.065] | 1.147 |
| variant-01/05-ring | 8.246 [8.062, 8.707] | 1.500 |
| variant-02/01-basic | 5.856 [5.311, 7.736] | 1.072 |
| variant-02/02-single-salesman | 5.527 [5.020, 5.574] | 1.007 |
| variant-02/03-too-many-salesmen-unsat | 5.097 [5.024, 5.748] | 1.154 |
| variant-02/04-equal-cost-split | 5.777 [5.499, 5.811] | 1.050 |
| variant-02/05-larger-asymmetric | 5.986 [5.782, 6.299] | 1.086 |
| variant-02/06-unreachable-edge | 5.653 [5.563, 6.703] | 1.033 |
| variant-03/01-basic | 5.943 [5.581, 6.042] | 1.088 |
| variant-03/02-single-salesman | 5.951 [5.553, 7.323] | 1.074 |
| variant-03/03-depot-crossing-unsat | 5.477 [4.819, 6.367] | 1.243 |
| variant-03/04-equal-cost-split | 5.769 [5.683, 6.599] | 1.310 |
| variant-03/05-larger-three-depots | 6.622 [6.111, 6.908] | 1.501 |
| variant-03/06-unreachable-edge | 6.598 [5.807, 6.734] | 1.194 |
| variant-04/01-basic | 6.067 [5.981, 6.990] | 1.103 |
| variant-04/02-single-salesman | 6.310 [5.472, 6.796] | 1.422 |
| variant-04/03-window-too-tight-unsat | 6.223 [5.453, 6.557] | 1.407 |
| variant-04/04-depot-window-too-tight-unsat | 6.413 [6.110, 6.429] | 1.173 |
| variant-04/05-three-depots-asymmetric-times | 8.055 [7.673, 9.000] | 1.477 |
| variant-04/06-unreachable-edge | 8.085 [8.032, 8.217] | 1.470 |
| variant-05/01-basic | 6.651 [6.089, 6.924] | 1.209 |
| variant-05/02-tight-bound-exact | 6.109 [5.938, 6.307] | 1.342 |
| variant-05/03-revisit-too-tight-unsat | 6.553 [5.722, 6.621] | 1.470 |
| variant-05/04-depot-and-vertex-revisits | 6.099 [5.702, 7.101] | 1.369 |
| variant-05/05-three-depots-mixed | 9.052 [7.872, 9.139] | 1.665 |
| variant-05/06-unreachable-edge | 7.806 [7.010, 8.038] | 1.420 |
| n-queens/variant-01 | 9.074 [8.686, 10.965] | 1.390 |
| n-queens/variant-02 | 88.044 [88.032, 90.332] | 0.707 |
| n-queens/variant-03 | 9.385 [9.041, 10.402] | 1.416 |
| n-queens/variant-04 | 7.449 [6.908, 7.871] | 1.345 |
| n-queens/variant-05 | 8.774 [7.896, 9.346] | 1.333 |
| n-queens/variant-06 | 8.938 [7.997, 9.870] | 1.611 |
| send-money/send-money | 14.225 [12.404, 14.855] | 0.883 |

Reference wall time, ms, same notation.

| Cell | corpus |
|---|---:|
| equality-generalized-tsp/01-basic | 5.536 [5.466, 5.618] |
| equality-generalized-tsp/02-larger | 4.523 [4.360, 5.431] |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.523 [4.427, 5.552] |
| variant-01/01-basic | 4.414 [4.400, 5.521] |
| variant-01/02-start-equals-end | 4.476 [4.426, 5.504] |
| variant-01/03-zero-cost-detour | 4.411 [3.313, 5.508] |
| variant-01/04-no-path | 4.406 [4.403, 4.419] |
| variant-01/05-multi-path | 5.466 [4.413, 5.471] |
| variant-01/06-layered-dag | 5.521 [5.465, 5.530] |
| variant-01/07-cycles | 5.499 [5.486, 5.605] |
| variant-01/08-negative-weights | 4.548 [4.437, 5.496] |
| variant-02/01-basic | 4.455 [4.416, 5.526] |
| variant-02/02-start-equals-end | 4.597 [4.463, 5.553] |
| variant-02/03-before-forces-detour | 5.518 [5.450, 5.613] |
| variant-02/04-after-forces-extension | 4.485 [4.402, 5.454] |
| variant-02/05-ordering-unsat | 5.466 [4.454, 5.479] |
| variant-02/06-layered-dag-before | 5.513 [5.499, 6.553] |
| variant-02/07-layered-dag-before-after | 5.507 [5.502, 6.660] |
| variant-02/08-tie-break-under-ordering | 6.060 [5.467, 6.673] |
| variant-02/09-negative-weights | 4.354 [4.347, 5.491] |
| variant-03/01-basic | 4.400 [3.394, 4.427] |
| variant-03/02-start-equals-end | 4.394 [4.375, 4.445] |
| variant-03/03-budget-forces-detour | 5.525 [5.487, 5.586] |
| variant-03/04-cost-at-cap-allowed | 4.480 [4.397, 5.514] |
| variant-03/05-budget-unsat | 4.413 [4.342, 4.435] |
| variant-03/06-layered-dag-cap | 6.552 [5.460, 6.580] |
| variant-03/07-layered-dag-tight-cap | 6.583 [6.554, 7.631] |
| variant-03/08-tie-break-under-cap | 5.518 [5.481, 5.528] |
| variant-03/09-negative-weights | 4.426 [4.415, 6.967] |
| variant-04/01-basic | 5.576 [5.440, 6.572] |
| variant-04/02-start-equals-end | 4.457 [4.369, 5.507] |
| variant-04/03-before-forces-detour-within-budget | 4.450 [4.438, 5.475] |
| variant-04/04-ordering-violates-budget | 4.449 [4.372, 4.451] |
| variant-04/05-after-and-budget-interact | 5.472 [5.424, 5.560] |
| variant-04/06-layered-dag-ordering-cap | 6.612 [6.564, 7.627] |
| variant-04/07-layered-dag-combined | 7.647 [6.590, 8.748] |
| variant-04/08-tie-break-under-ordering-and-cap | 5.506 [4.421, 6.546] |
| variant-04/09-negative-weights | 5.469 [4.375, 5.584] |
| variant-01/01-basic | 4.438 [3.379, 4.440] |
| variant-01/02-agent-reuse | 3.331 [3.285, 5.532] |
| variant-01/03-selective-compatibility | 4.443 [4.425, 5.471] |
| variant-01/04-no-compatible-agent-unsat | 4.417 [4.403, 4.430] |
| variant-01/05-larger-mix | 4.466 [4.438, 4.481] |
| variant-02/01-basic | 5.513 [4.469, 6.673] |
| variant-02/02-makespan-tiebreak | 4.453 [4.419, 4.483] |
| variant-02/03-cost-dominates | 4.444 [4.403, 5.463] |
| variant-02/04-no-compatible-agent-unsat | 4.470 [3.299, 5.521] |
| variant-02/05-larger-mix | 5.508 [5.473, 5.554] |
| variant-03/01-basic | 4.405 [4.362, 4.437] |
| variant-03/02-multiple-groups | 4.528 [4.439, 5.428] |
| variant-03/03-mixed-grouped-ungrouped | 4.431 [4.404, 5.506] |
| variant-03/04-incompatible-group-unsat | 4.451 [4.392, 4.458] |
| variant-03/05-larger-mix | 4.431 [4.427, 5.529] |
| variant-04/01-basic | 27.887 [26.888, 29.252] |
| variant-04/02-precedence | 6.591 [5.503, 7.796] |
| variant-04/03-agent-serialization | 29.888 [27.167, 29.918] |
| variant-04/04-window-too-tight-unsat | 5.597 [4.430, 6.555] |
| variant-04/05-larger-mix | 150.645 [150.257, 151.908] |
| variant-01/01-basic | 4.621 [4.582, 5.873] |
| variant-01/02-multiple-tours | 4.689 [4.373, 5.481] |
| variant-01/03-asymmetric | 5.758 [5.508, 6.573] |
| variant-01/04-subtour-unsat | 4.396 [4.376, 4.537] |
| variant-01/05-ring | 5.497 [5.482, 5.509] |
| variant-02/01-basic | 5.464 [4.357, 5.479] |
| variant-02/02-single-salesman | 5.490 [4.435, 5.505] |
| variant-02/03-too-many-salesmen-unsat | 4.417 [4.358, 4.472] |
| variant-02/04-equal-cost-split | 5.504 [4.435, 5.579] |
| variant-02/05-larger-asymmetric | 5.514 [5.477, 5.541] |
| variant-02/06-unreachable-edge | 5.472 [4.463, 5.503] |
| variant-03/01-basic | 5.460 [4.463, 5.508] |
| variant-03/02-single-salesman | 5.539 [4.333, 7.751] |
| variant-03/03-depot-crossing-unsat | 4.404 [4.387, 5.490] |
| variant-03/04-equal-cost-split | 4.404 [3.331, 4.415] |
| variant-03/05-larger-three-depots | 4.412 [4.410, 5.643] |
| variant-03/06-unreachable-edge | 5.526 [4.460, 5.662] |
| variant-04/01-basic | 5.501 [5.456, 6.546] |
| variant-04/02-single-salesman | 4.437 [4.434, 5.439] |
| variant-04/03-window-too-tight-unsat | 4.424 [4.414, 4.432] |
| variant-04/04-depot-window-too-tight-unsat | 5.469 [4.416, 5.582] |
| variant-04/05-three-depots-asymmetric-times | 5.454 [4.393, 6.552] |
| variant-04/06-unreachable-edge | 5.501 [5.471, 5.536] |
| variant-05/01-basic | 5.499 [4.365, 5.553] |
| variant-05/02-tight-bound-exact | 4.552 [4.460, 6.626] |
| variant-05/03-revisit-too-tight-unsat | 4.458 [4.355, 4.480] |
| variant-05/04-depot-and-vertex-revisits | 4.455 [4.447, 5.528] |
| variant-05/05-three-depots-mixed | 5.437 [4.447, 5.507] |
| variant-05/06-unreachable-edge | 5.499 [4.501, 5.508] |
| n-queens/variant-01 | 6.528 [5.462, 6.594] |
| n-queens/variant-02 | 124.488 [123.615, 125.089] |
| n-queens/variant-03 | 6.626 [5.761, 6.654] |
| n-queens/variant-04 | 5.540 [5.475, 9.834] |
| n-queens/variant-05 | 6.579 [5.610, 6.621] |
| n-queens/variant-06 | 5.549 [5.438, 6.686] |
| send-money/send-money | 16.105 [15.051, 16.175] |

Counters of report corpus: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 0 | 1 | 4 | 11999 | 2.228 |
| equality-generalized-tsp/02-larger | 0 | 1 | 8 | 25183 | 2.577 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 0 | 0 | 0 | 2258 | 1.431 |
| variant-01/01-basic | 0 | 1 | 2 | 1559 | 1.570 |
| variant-01/02-start-equals-end | 0 | 1 | 1 | 1208 | 1.950 |
| variant-01/03-zero-cost-detour | 0 | 1 | 2 | 1291 | 1.682 |
| variant-01/04-no-path | 0 | 0 | 0 | 1085 | 1.374 |
| variant-01/05-multi-path | 0 | 1 | 3 | 3958 | 1.879 |
| variant-01/06-layered-dag | 0 | 1 | 8 | 263618 | 4.856 |
| variant-01/07-cycles | 0 | 1 | 5 | 15684 | 2.032 |
| variant-01/08-negative-weights | 0 | 1 | 1 | 2738 | 1.717 |
| variant-02/01-basic | 0 | 1 | 2 | 1700 | 1.606 |
| variant-02/02-start-equals-end | 0 | 1 | 1 | 1557 | 2.211 |
| variant-02/03-before-forces-detour | 0 | 1 | 1 | 2383 | 1.778 |
| variant-02/04-after-forces-extension | 0 | 1 | 1 | 2245 | 1.714 |
| variant-02/05-ordering-unsat | 0 | 0 | 0 | 1322 | 1.613 |
| variant-02/06-layered-dag-before | 0 | 2 | 6 | 82315 | 4.749 |
| variant-02/07-layered-dag-before-after | 0 | 1 | 7 | 64181 | 4.076 |
| variant-02/08-tie-break-under-ordering | 0 | 1 | 2 | 2777 | 2.130 |
| variant-02/09-negative-weights | 0 | 1 | 1 | 3096 | 1.637 |
| variant-03/01-basic | 0 | 1 | 2 | 1667 | 1.616 |
| variant-03/02-start-equals-end | 0 | 1 | 1 | 1467 | 1.466 |
| variant-03/03-budget-forces-detour | 0 | 1 | 1 | 2141 | 1.757 |
| variant-03/04-cost-at-cap-allowed | 0 | 1 | 1 | 1272 | 1.666 |
| variant-03/05-budget-unsat | 0 | 0 | 0 | 4970 | 1.791 |
| variant-03/06-layered-dag-cap | 0 | 2 | 8 | 799304 | 8.467 |
| variant-03/07-layered-dag-tight-cap | 0 | 4 | 5 | 672274 | 7.375 |
| variant-03/08-tie-break-under-cap | 0 | 1 | 2 | 3301 | 1.843 |
| variant-03/09-negative-weights | 0 | 1 | 1 | 3168 | 1.989 |
| variant-04/01-basic | 0 | 1 | 2 | 1808 | 1.712 |
| variant-04/02-start-equals-end | 0 | 1 | 1 | 1816 | 2.194 |
| variant-04/03-before-forces-detour-within-budget | 0 | 1 | 1 | 2774 | 2.198 |
| variant-04/04-ordering-violates-budget | 0 | 0 | 0 | 2816 | 2.334 |
| variant-04/05-after-and-budget-interact | 0 | 1 | 1 | 4169 | 1.855 |
| variant-04/06-layered-dag-ordering-cap | 0 | 2 | 7 | 113055 | 6.617 |
| variant-04/07-layered-dag-combined | 0 | 1 | 3 | 352340 | 6.614 |
| variant-04/08-tie-break-under-ordering-and-cap | 0 | 1 | 2 | 6554 | 2.453 |
| variant-04/09-negative-weights | 0 | 1 | 1 | 3526 | 2.581 |
| variant-01/01-basic | 0 | 1 | 4 | 1462 | 1.074 |
| variant-01/02-agent-reuse | 0 | 1 | 8 | 2131 | 1.476 |
| variant-01/03-selective-compatibility | 0 | 1 | 4 | 1873 | 1.263 |
| variant-01/04-no-compatible-agent-unsat | 0 | 0 | 0 | 1217 | 0.981 |
| variant-01/05-larger-mix | 0 | 1 | 19 | 66910 | 2.202 |
| variant-02/01-basic | 0 | 1 | 4 | 3047 | 1.710 |
| variant-02/02-makespan-tiebreak | 0 | 3 | 8 | 5121 | 1.916 |
| variant-02/03-cost-dominates | 0 | 1 | 8 | 4914 | 1.882 |
| variant-02/04-no-compatible-agent-unsat | 0 | 0 | 0 | 2395 | 1.657 |
| variant-02/05-larger-mix | 0 | 1 | 11 | 341227 | 4.545 |
| variant-03/01-basic | 0 | 2 | 2 | 1970 | 1.690 |
| variant-03/02-multiple-groups | 0 | 4 | 4 | 3816 | 1.749 |
| variant-03/03-mixed-grouped-ungrouped | 0 | 2 | 4 | 2639 | 1.714 |
| variant-03/04-incompatible-group-unsat | 0 | 0 | 0 | 1224 | 2.205 |
| variant-03/05-larger-mix | 0 | 1 | 11 | 12890 | 2.058 |
| variant-04/01-basic | 0 | 81 | 98 | 205476 | 5.618 |
| variant-04/02-precedence | 0 | 3 | 10 | 23701 | 3.625 |
| variant-04/03-agent-serialization | 0 | 30 | 30 | 76722 | 4.867 |
| variant-04/04-window-too-tight-unsat | 0 | 0 | 0 | 2865 | 2.884 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 5791371 | 39.392 |
| variant-01/01-basic | 0 | 1 | 2 | 5348 | 1.734 |
| variant-01/02-multiple-tours | 0 | 2 | 6 | 45796 | 2.339 |
| variant-01/03-asymmetric | 0 | 1 | 8 | 70491 | 2.754 |
| variant-01/04-subtour-unsat | 0 | 0 | 0 | 2473 | 1.251 |
| variant-01/05-ring | 0 | 2 | 8 | 272334 | 4.173 |
| variant-02/01-basic | 0 | 2 | 2 | 6048 | 1.910 |
| variant-02/02-single-salesman | 0 | 1 | 2 | 3805 | 1.678 |
| variant-02/03-too-many-salesmen-unsat | 0 | 0 | 0 | 3466 | 1.511 |
| variant-02/04-equal-cost-split | 0 | 2 | 2 | 4938 | 1.958 |
| variant-02/05-larger-asymmetric | 0 | 2 | 2 | 7158 | 2.102 |
| variant-02/06-unreachable-edge | 0 | 2 | 2 | 8124 | 2.179 |
| variant-03/01-basic | 0 | 1 | 1 | 3939 | 1.876 |
| variant-03/02-single-salesman | 0 | 1 | 2 | 3659 | 2.478 |
| variant-03/03-depot-crossing-unsat | 0 | 0 | 0 | 3422 | 1.537 |
| variant-03/04-equal-cost-split | 0 | 2 | 2 | 7812 | 2.051 |
| variant-03/05-larger-three-depots | 0 | 1 | 1 | 10821 | 2.505 |
| variant-03/06-unreachable-edge | 0 | 1 | 1 | 6365 | 2.203 |
| variant-04/01-basic | 0 | 1 | 1 | 6955 | 2.643 |
| variant-04/02-single-salesman | 0 | 1 | 1 | 4700 | 2.370 |
| variant-04/03-window-too-tight-unsat | 0 | 0 | 0 | 3400 | 2.476 |
| variant-04/04-depot-window-too-tight-unsat | 0 | 0 | 0 | 2325 | 2.125 |
| variant-04/05-three-depots-asymmetric-times | 0 | 1 | 1 | 18328 | 4.080 |
| variant-04/06-unreachable-edge | 0 | 1 | 1 | 14062 | 3.900 |
| variant-05/01-basic | 0 | 1 | 1 | 6957 | 2.677 |
| variant-05/02-tight-bound-exact | 0 | 1 | 1 | 3439 | 2.460 |
| variant-05/03-revisit-too-tight-unsat | 0 | 0 | 0 | 3413 | 2.374 |
| variant-05/04-depot-and-vertex-revisits | 0 | 1 | 1 | 3504 | 2.243 |
| variant-05/05-three-depots-mixed | 0 | 1 | 1 | 18378 | 5.248 |
| variant-05/06-unreachable-edge | 0 | 1 | 1 | 14062 | 3.688 |
| n-queens/variant-01 | 0 | 92 | 92 | 344466 | 5.112 |
| n-queens/variant-02 | 0 | 92 | 92 | 25717860 | 84.647 |
| n-queens/variant-03 | 0 | 92 | 92 | 504289 | 5.746 |
| n-queens/variant-04 | 0 | 92 | 92 | 296224 | 3.990 |
| n-queens/variant-05 | 0 | 92 | 92 | 297671 | 4.800 |
| n-queens/variant-06 | 0 | 92 | 92 | 305512 | 5.329 |
| send-money/send-money | 0 | 1 | 1 | 844145 | 9.992 |

Against the reference: report corpus, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 9 of 94 cells where both passed (9.5%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 43.543 | 150.645 | 0.289 | 3.251 | 80.713 | 13.195 | 87.000 | 58.000 |
| variant-04/01-basic | 9.399 | 27.887 | 0.337 | 0.963 | 5.437 | 0.571 | 18.000 | 4.000 |
| variant-04/03-agent-serialization | 10.146 | 29.888 | 0.339 | 0.926 | 2.694 | 0.296 | 24.000 | 0.000 |
| n-queens/variant-02 | 88.044 | 124.488 | 0.707 | 2.551 | 286.768 | 1.233 | 2.000 | 118.000 |
| send-money/send-money | 14.225 | 16.105 | 0.883 | 2.541 | 11.756 | 0.303 | 10.000 | 1.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.099 | 5.523 | 0.923 | 0.193 | 0.042 | 0.009 | 1.000 | 0.000 |
| variant-01/05-multi-path | 5.317 | 5.466 | 0.973 | 0.269 | 0.168 | 0.029 | 1.000 | 0.000 |
| variant-04/01-basic | 5.497 | 5.576 | 0.986 | 0.204 | 0.083 | 0.014 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 5.523 | 5.525 | 1.000 | 0.224 | 0.059 | 0.013 | 1.000 | 0.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-02/02-single-salesman | 5.527 | 5.490 | 1.007 | 0.250 | 0.100 | 0.084 | 1.000 | 0.000 |
| variant-02/05-ordering-unsat | 5.551 | 5.466 | 1.016 | 0.171 | 0.041 | 0.006 | 1.000 | 0.000 |
| variant-02/01-basic | 5.624 | 5.513 | 1.020 | 0.346 | 0.192 | 0.032 | 1.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 6.253 | 6.060 | 1.032 | 0.256 | 0.107 | 0.020 | 1.000 | 0.000 |
| variant-02/06-unreachable-edge | 5.653 | 5.472 | 1.033 | 0.492 | 0.193 | 0.150 | 1.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 5.703 | 5.518 | 1.033 | 0.292 | 0.116 | 0.022 | 1.000 | 0.000 |
| variant-02/04-equal-cost-split | 5.777 | 5.504 | 1.050 | 0.315 | 0.141 | 0.109 | 1.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 4.675 | 4.417 | 1.058 | 0.119 | 0.031 | 0.005 | 0.000 | 0.000 |
| variant-02/01-basic | 5.856 | 5.464 | 1.072 | 0.366 | 0.179 | 0.117 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 5.951 | 5.539 | 1.074 | 0.268 | 0.105 | 0.084 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 5.885 | 5.472 | 1.075 | 0.310 | 0.063 | 0.017 | 1.000 | 0.000 |
| variant-02/05-larger-asymmetric | 5.986 | 5.514 | 1.086 | 0.468 | 0.167 | 0.138 | 1.000 | 0.000 |
| variant-03/01-basic | 5.943 | 5.460 | 1.088 | 0.302 | 0.083 | 0.047 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 4.805 | 4.413 | 1.089 | 0.355 | 0.116 | 0.014 | 1.000 | 0.000 |
| variant-01/04-no-path | 4.837 | 4.406 | 1.098 | 0.163 | 0.037 | 0.005 | 0.000 | 0.000 |
| variant-04/01-basic | 6.067 | 5.501 | 1.103 | 0.600 | 0.111 | 0.078 | 1.000 | 0.000 |
| variant-01/07-cycles | 6.091 | 5.499 | 1.108 | 0.371 | 0.342 | 0.334 | 1.000 | 0.000 |
| variant-02/03-before-forces-detour | 6.113 | 5.518 | 1.108 | 0.248 | 0.061 | 0.013 | 1.000 | 0.000 |
| variant-01/03-selective-compatibility | 4.975 | 4.443 | 1.120 | 0.175 | 0.084 | 0.021 | 0.000 | 0.000 |
| variant-01/08-negative-weights | 5.106 | 4.548 | 1.123 | 0.263 | 0.066 | 0.041 | 1.000 | 0.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.190 | 5.506 | 1.124 | 0.454 | 0.154 | 0.042 | 1.000 | 0.000 |
| variant-01/04-subtour-unsat | 5.043 | 4.396 | 1.147 | 0.182 | 0.033 | 0.008 | 0.000 | 0.000 |
| equality-generalized-tsp/01-basic | 6.386 | 5.536 | 1.154 | 0.381 | 0.233 | 0.198 | 1.000 | 0.000 |
| variant-02/03-too-many-salesmen-unsat | 5.097 | 4.417 | 1.154 | 0.288 | 0.056 | 0.010 | 1.000 | 0.000 |
| variant-03/01-basic | 5.119 | 4.405 | 1.162 | 0.154 | 0.073 | 0.014 | 0.000 | 0.000 |
| variant-01/03-asymmetric | 6.697 | 5.758 | 1.163 | 0.536 | 1.006 | 0.965 | 2.000 | 0.000 |
| variant-04/02-precedence | 7.688 | 6.591 | 1.167 | 0.614 | 1.011 | 0.094 | 2.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 6.413 | 5.469 | 1.173 | 0.326 | 0.051 | 0.010 | 1.000 | 0.000 |
| variant-03/06-unreachable-edge | 6.598 | 5.526 | 1.194 | 0.532 | 0.114 | 0.074 | 1.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 6.709 | 5.597 | 1.199 | 0.322 | 0.083 | 0.011 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 5.312 | 4.431 | 1.199 | 0.197 | 0.114 | 0.025 | 1.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 5.352 | 4.451 | 1.202 | 0.142 | 0.038 | 0.006 | 0.000 | 0.000 |
| variant-05/01-basic | 6.651 | 5.499 | 1.209 | 0.608 | 0.114 | 0.078 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 5.503 | 4.528 | 1.215 | 0.227 | 0.183 | 0.030 | 1.000 | 0.000 |
| variant-01/01-basic | 5.694 | 4.621 | 1.232 | 0.266 | 0.149 | 0.104 | 1.000 | 0.000 |
| variant-02/04-after-forces-extension | 5.550 | 4.485 | 1.238 | 0.242 | 0.059 | 0.012 | 1.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 5.477 | 4.404 | 1.243 | 0.294 | 0.055 | 0.010 | 1.000 | 0.000 |
| variant-01/02-start-equals-end | 5.573 | 4.476 | 1.245 | 0.154 | 0.040 | 0.023 | 1.000 | 0.000 |
| variant-01/01-basic | 5.551 | 4.438 | 1.251 | 0.146 | 0.101 | 0.018 | 0.000 | 0.000 |
| variant-03/01-basic | 5.515 | 4.400 | 1.254 | 0.201 | 0.070 | 0.013 | 1.000 | 0.000 |
| variant-04/09-negative-weights | 6.970 | 5.469 | 1.274 | 0.310 | 0.076 | 0.052 | 1.000 | 0.000 |
| variant-03/04-cost-at-cap-allowed | 5.721 | 4.480 | 1.277 | 0.173 | 0.027 | 0.008 | 1.000 | 0.000 |
| variant-01/02-multiple-tours | 6.009 | 4.689 | 1.282 | 0.385 | 0.785 | 0.566 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 5.719 | 4.450 | 1.285 | 0.301 | 0.097 | 0.024 | 1.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 5.748 | 4.470 | 1.286 | 0.279 | 0.031 | 0.008 | 0.000 | 0.000 |
| variant-02/09-negative-weights | 5.687 | 4.354 | 1.306 | 0.245 | 0.070 | 0.045 | 1.000 | 0.000 |
| variant-03/05-larger-mix | 5.791 | 4.431 | 1.307 | 0.272 | 0.639 | 0.068 | 1.000 | 0.000 |
| variant-03/04-equal-cost-split | 5.769 | 4.404 | 1.310 | 0.452 | 0.187 | 0.149 | 1.000 | 0.000 |
| variant-04/02-start-equals-end | 5.863 | 4.457 | 1.316 | 0.211 | 0.047 | 0.029 | 1.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 5.899 | 4.453 | 1.325 | 0.368 | 0.303 | 0.060 | 1.000 | 0.000 |
| variant-01/01-basic | 5.865 | 4.414 | 1.329 | 0.195 | 0.086 | 0.015 | 1.000 | 0.000 |
| variant-03/09-negative-weights | 5.885 | 4.426 | 1.330 | 0.304 | 0.077 | 0.049 | 1.000 | 0.000 |
| n-queens/variant-05 | 8.774 | 6.579 | 1.333 | 1.008 | 7.165 | 0.893 | 1.000 | 1.000 |
| variant-02/03-cost-dominates | 5.957 | 4.444 | 1.340 | 0.353 | 0.294 | 0.059 | 1.000 | 0.000 |
| variant-05/02-tight-bound-exact | 6.109 | 4.552 | 1.342 | 0.384 | 0.069 | 0.046 | 1.000 | 0.000 |
| n-queens/variant-04 | 7.449 | 5.540 | 1.345 | 0.707 | 6.764 | 0.621 | 1.000 | 1.000 |
| variant-03/02-start-equals-end | 5.938 | 4.394 | 1.351 | 0.167 | 0.038 | 0.022 | 1.000 | 0.000 |
| variant-01/03-zero-cost-detour | 6.010 | 4.411 | 1.362 | 0.184 | 0.060 | 0.012 | 1.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 6.099 | 4.455 | 1.369 | 0.381 | 0.069 | 0.046 | 1.000 | 0.000 |
| variant-01/05-larger-mix | 6.121 | 4.466 | 1.370 | 0.260 | 2.292 | 0.112 | 0.000 | 0.000 |
| variant-02/01-basic | 6.141 | 4.455 | 1.379 | 0.198 | 0.084 | 0.016 | 1.000 | 0.000 |
| n-queens/variant-01 | 9.074 | 6.528 | 1.390 | 1.828 | 7.484 | 0.768 | 1.000 | 1.000 |
| equality-generalized-tsp/02-larger | 6.293 | 4.523 | 1.391 | 0.599 | 0.500 | 0.536 | 1.000 | 0.000 |
| variant-02/07-layered-dag-before-after | 7.740 | 5.507 | 1.406 | 1.158 | 2.036 | 0.207 | 2.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 6.223 | 4.424 | 1.407 | 0.346 | 0.048 | 0.010 | 1.000 | 0.000 |
| n-queens/variant-03 | 9.385 | 6.626 | 1.416 | 1.858 | 8.826 | 0.730 | 1.000 | 1.000 |
| variant-05/06-unreachable-edge | 7.806 | 5.499 | 1.420 | 1.186 | 0.195 | 0.150 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 6.310 | 4.437 | 1.422 | 0.470 | 0.075 | 0.057 | 1.000 | 0.000 |
| variant-04/07-layered-dag-combined | 10.925 | 7.647 | 1.429 | 1.960 | 5.539 | 0.260 | 3.000 | 0.000 |
| variant-02/02-start-equals-end | 6.649 | 4.597 | 1.446 | 0.205 | 0.049 | 0.026 | 1.000 | 0.000 |
| variant-04/06-unreachable-edge | 8.085 | 5.501 | 1.470 | 1.213 | 0.208 | 0.196 | 1.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 6.553 | 4.458 | 1.470 | 0.361 | 0.054 | 0.011 | 1.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 8.055 | 5.454 | 1.477 | 1.532 | 0.234 | 0.171 | 1.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 6.609 | 4.449 | 1.485 | 0.286 | 0.056 | 0.009 | 1.000 | 0.000 |
| variant-02/05-larger-mix | 8.264 | 5.508 | 1.500 | 0.727 | 7.031 | 0.126 | 1.000 | 0.000 |
| variant-01/05-ring | 8.246 | 5.497 | 1.500 | 0.675 | 4.704 | 1.416 | 1.000 | 0.000 |
| variant-03/05-larger-three-depots | 6.622 | 4.412 | 1.501 | 0.813 | 0.151 | 0.099 | 1.000 | 0.000 |
| variant-02/06-layered-dag-before | 8.693 | 5.513 | 1.577 | 1.101 | 3.427 | 0.193 | 2.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 10.531 | 6.612 | 1.593 | 1.827 | 5.292 | 0.316 | 3.000 | 0.000 |
| variant-01/06-layered-dag | 8.812 | 5.521 | 1.596 | 1.066 | 4.973 | 0.150 | 1.000 | 0.000 |
| n-queens/variant-06 | 8.938 | 5.549 | 1.611 | 1.083 | 6.441 | 0.832 | 1.000 | 1.000 |
| variant-01/02-agent-reuse | 5.521 | 3.331 | 1.657 | 0.151 | 0.120 | 0.029 | 0.000 | 0.000 |
| variant-05/05-three-depots-mixed | 9.052 | 5.437 | 1.665 | 2.243 | 0.241 | 0.169 | 1.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 11.917 | 6.583 | 1.810 | 1.691 | 9.601 | 0.241 | 2.000 | 1.000 |
| variant-03/06-layered-dag-cap | 12.418 | 6.552 | 1.895 | 1.727 | 12.060 | 0.298 | 2.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 13.4 | 23.7 | n/a |
| variant-04/01-basic | 11.3 | 13.2 | n/a |
| variant-04/03-agent-serialization | 11.4 | 12.6 | n/a |
| n-queens/variant-02 | 12.4 | 13.0 | n/a |
| send-money/send-money | 12.5 | 12.8 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.4 | 10.4 | n/a |
| variant-01/05-multi-path | 10.3 | 10.4 | n/a |
| variant-04/01-basic | 10.7 | 10.5 | n/a |
| variant-03/03-budget-forces-detour | 10.3 | 10.5 | n/a |
| variant-02/02-single-salesman | 10.6 | 10.5 | n/a |
| variant-02/05-ordering-unsat | 10.6 | 10.4 | n/a |
| variant-02/01-basic | 10.2 | 10.4 | n/a |
| variant-02/08-tie-break-under-ordering | 10.8 | 10.3 | n/a |
| variant-02/06-unreachable-edge | 10.6 | 10.3 | n/a |
| variant-03/08-tie-break-under-cap | 10.6 | 10.5 | n/a |
| variant-02/04-equal-cost-split | 10.5 | 10.4 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.3 | 10.4 | n/a |
| variant-02/01-basic | 10.4 | 10.5 | n/a |
| variant-03/02-single-salesman | 10.6 | 10.5 | n/a |
| variant-04/05-after-and-budget-interact | 10.6 | 10.7 | n/a |
| variant-02/05-larger-asymmetric | 10.5 | 10.3 | n/a |
| variant-03/01-basic | 10.3 | 10.5 | n/a |
| variant-03/05-budget-unsat | 10.8 | 10.5 | n/a |
| variant-01/04-no-path | 10.4 | 10.5 | n/a |
| variant-04/01-basic | 10.9 | 10.5 | n/a |
| variant-01/07-cycles | 10.7 | 10.4 | n/a |
| variant-02/03-before-forces-detour | 10.5 | 10.5 | n/a |
| variant-01/03-selective-compatibility | 10.5 | 10.3 | n/a |
| variant-01/08-negative-weights | 10.7 | 10.4 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 10.7 | 10.7 | n/a |
| variant-01/04-subtour-unsat | 10.4 | 10.4 | n/a |
| equality-generalized-tsp/01-basic | 10.2 | 10.3 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.6 | 10.3 | n/a |
| variant-03/01-basic | 10.4 | 10.5 | n/a |
| variant-01/03-asymmetric | 11.3 | 10.5 | n/a |
| variant-04/02-precedence | 11.6 | 10.8 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 10.6 | 10.6 | n/a |
| variant-03/06-unreachable-edge | 10.4 | 10.5 | n/a |
| variant-04/04-window-too-tight-unsat | 11.5 | 10.5 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.4 | 10.5 | n/a |
| variant-03/04-incompatible-group-unsat | 10.6 | 10.4 | n/a |
| variant-05/01-basic | 11.1 | 10.6 | n/a |
| variant-03/02-multiple-groups | 10.5 | 10.4 | n/a |
| variant-01/01-basic | 10.4 | 10.5 | n/a |
| variant-02/04-after-forces-extension | 10.5 | 10.4 | n/a |
| variant-03/03-depot-crossing-unsat | 10.5 | 10.5 | n/a |
| variant-01/02-start-equals-end | 10.4 | 10.5 | n/a |
| variant-01/01-basic | 10.0 | 10.4 | n/a |
| variant-03/01-basic | 10.4 | 10.5 | n/a |
| variant-04/09-negative-weights | 10.5 | 10.5 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.2 | 10.4 | n/a |
| variant-01/02-multiple-tours | 10.5 | 10.4 | n/a |
| variant-04/03-before-forces-detour-within-budget | 10.8 | 10.6 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.4 | 10.4 | n/a |
| variant-02/09-negative-weights | 10.6 | 10.5 | n/a |
| variant-03/05-larger-mix | 10.8 | 10.5 | n/a |
| variant-03/04-equal-cost-split | 10.5 | 10.5 | n/a |
| variant-04/02-start-equals-end | 10.5 | 10.5 | n/a |
| variant-02/02-makespan-tiebreak | 10.3 | 10.5 | n/a |
| variant-01/01-basic | 10.7 | 10.4 | n/a |
| variant-03/09-negative-weights | 10.6 | 10.6 | n/a |
| n-queens/variant-05 | 10.8 | 10.8 | n/a |
| variant-02/03-cost-dominates | 10.6 | 10.3 | n/a |
| variant-05/02-tight-bound-exact | 10.7 | 10.6 | n/a |
| n-queens/variant-04 | 10.2 | 10.5 | n/a |
| variant-03/02-start-equals-end | 10.5 | 10.5 | n/a |
| variant-01/03-zero-cost-detour | 10.5 | 10.4 | n/a |
| variant-05/04-depot-and-vertex-revisits | 11.0 | 10.5 | n/a |
| variant-01/05-larger-mix | 10.5 | 10.5 | n/a |
| variant-02/01-basic | 10.6 | 10.6 | n/a |
| n-queens/variant-01 | 10.5 | 10.5 | n/a |
| equality-generalized-tsp/02-larger | 10.4 | 10.6 | n/a |
| variant-02/07-layered-dag-before-after | 11.6 | 10.7 | n/a |
| variant-04/03-window-too-tight-unsat | 10.8 | 10.5 | n/a |
| n-queens/variant-03 | 10.5 | 10.5 | n/a |
| variant-05/06-unreachable-edge | 11.2 | 10.6 | n/a |
| variant-04/02-single-salesman | 11.1 | 10.5 | n/a |
| variant-04/07-layered-dag-combined | 11.9 | 11.1 | n/a |
| variant-02/02-start-equals-end | 10.5 | 10.5 | n/a |
| variant-04/06-unreachable-edge | 11.0 | 10.7 | n/a |
| variant-05/03-revisit-too-tight-unsat | 11.0 | 10.4 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.1 | 10.5 | n/a |
| variant-04/04-ordering-violates-budget | 10.7 | 10.5 | n/a |
| variant-02/05-larger-mix | 11.1 | 10.5 | n/a |
| variant-01/05-ring | 11.5 | 10.7 | n/a |
| variant-03/05-larger-three-depots | 10.7 | 10.5 | n/a |
| variant-02/06-layered-dag-before | 11.0 | 10.7 | n/a |
| variant-04/06-layered-dag-ordering-cap | 12.5 | 11.1 | n/a |
| variant-01/06-layered-dag | 11.1 | 10.7 | n/a |
| n-queens/variant-06 | 10.8 | 10.2 | n/a |
| variant-01/02-agent-reuse | 10.5 | 10.3 | n/a |
| variant-05/05-three-depots-mixed | 11.1 | 10.5 | n/a |
| variant-03/07-layered-dag-tight-cap | 12.4 | 11.0 | n/a |
| variant-03/06-layered-dag-cap | 12.7 | 10.9 | n/a |
