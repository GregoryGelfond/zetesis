Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | corpus | corpus/reference |
|---|---:|---:|
| equality-generalized-tsp/01-basic | 6.104 [5.725, 6.751] | 1.381 |
| equality-generalized-tsp/02-larger | 6.664 [6.641, 7.304] | 1.206 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.967 [4.789, 5.278] | 0.904 |
| variant-01/01-basic | 5.228 [4.436, 5.496] | 1.203 |
| variant-01/02-start-equals-end | 5.579 [5.252, 6.142] | 1.256 |
| variant-01/03-zero-cost-detour | 5.957 [5.532, 6.040] | 1.341 |
| variant-01/04-no-path | 4.924 [4.627, 5.050] | 1.108 |
| variant-01/05-multi-path | 5.494 [4.789, 5.907] | 1.242 |
| variant-01/06-layered-dag | 7.830 [7.642, 8.727] | 1.426 |
| variant-01/07-cycles | 5.858 [5.483, 6.797] | 1.075 |
| variant-01/08-negative-weights | 6.248 [5.529, 6.491] | 1.408 |
| variant-02/01-basic | 4.766 [4.607, 6.312] | 1.074 |
| variant-02/02-start-equals-end | 5.535 [4.445, 6.185] | 1.254 |
| variant-02/03-before-forces-detour | 5.664 [5.520, 6.596] | 1.036 |
| variant-02/04-after-forces-extension | 6.286 [5.573, 7.528] | 1.346 |
| variant-02/05-ordering-unsat | 5.515 [5.029, 5.572] | 1.006 |
| variant-02/06-layered-dag-before | 7.846 [7.011, 7.866] | 1.425 |
| variant-02/07-layered-dag-before-after | 7.752 [7.718, 7.783] | 1.416 |
| variant-02/08-tie-break-under-ordering | 5.809 [5.560, 7.284] | 1.317 |
| variant-02/09-negative-weights | 5.499 [5.269, 5.897] | 1.248 |
| variant-03/01-basic | 5.520 [4.991, 6.161] | 1.248 |
| variant-03/02-start-equals-end | 5.700 [5.699, 5.926] | 1.285 |
| variant-03/03-budget-forces-detour | 5.540 [5.053, 7.407] | 1.246 |
| variant-03/04-cost-at-cap-allowed | 5.717 [4.862, 5.907] | 1.293 |
| variant-03/05-budget-unsat | 6.654 [4.789, 6.654] | 1.474 |
| variant-03/06-layered-dag-cap | 11.422 [9.955, 11.552] | 1.720 |
| variant-03/07-layered-dag-tight-cap | 12.969 [12.969, 14.165] | 1.972 |
| variant-03/08-tie-break-under-cap | 6.376 [5.931, 6.671] | 1.167 |
| variant-03/09-negative-weights | 5.805 [5.721, 7.556] | 1.063 |
| variant-04/01-basic | 5.497 [5.346, 5.745] | 1.246 |
| variant-04/02-start-equals-end | 5.598 [5.525, 5.725] | 1.277 |
| variant-04/03-before-forces-detour-within-budget | 5.804 [5.783, 6.173] | 1.302 |
| variant-04/04-ordering-violates-budget | 6.182 [5.002, 8.731] | 1.396 |
| variant-04/05-after-and-budget-interact | 5.424 [4.889, 6.278] | 1.222 |
| variant-04/06-layered-dag-ordering-cap | 10.278 [9.800, 10.909] | 1.563 |
| variant-04/07-layered-dag-combined | 10.449 [9.181, 10.822] | 1.585 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.261 [5.192, 6.587] | 1.424 |
| variant-04/09-negative-weights | 6.107 [5.836, 6.615] | 1.378 |
| variant-01/01-basic | 5.634 [4.577, 5.960] | 1.279 |
| variant-01/02-agent-reuse | 5.130 [4.943, 5.170] | 1.160 |
| variant-01/03-selective-compatibility | 5.128 [5.006, 5.259] | 1.160 |
| variant-01/04-no-compatible-agent-unsat | 5.694 [4.942, 6.801] | 1.241 |
| variant-01/05-larger-mix | 6.013 [4.942, 6.266] | 1.359 |
| variant-02/01-basic | 5.192 [5.018, 6.373] | 0.962 |
| variant-02/02-makespan-tiebreak | 5.867 [5.659, 6.600] | 1.325 |
| variant-02/03-cost-dominates | 5.986 [5.538, 6.457] | 1.090 |
| variant-02/04-no-compatible-agent-unsat | 5.780 [5.048, 5.890] | 1.305 |
| variant-02/05-larger-mix | 9.321 [7.613, 9.452] | 1.693 |
| variant-03/01-basic | 6.087 [5.149, 6.365] | 1.374 |
| variant-03/02-multiple-groups | 5.541 [5.487, 5.781] | 1.246 |
| variant-03/03-mixed-grouped-ungrouped | 5.569 [5.102, 8.088] | 1.254 |
| variant-03/04-incompatible-group-unsat | 5.674 [5.115, 5.900] | 1.276 |
| variant-03/05-larger-mix | 6.588 [5.494, 6.623] | 1.480 |
| variant-04/01-basic | 10.249 [9.360, 11.675] | 0.366 |
| variant-04/02-precedence | 7.738 [7.214, 8.094] | 1.159 |
| variant-04/03-agent-serialization | 9.207 [7.803, 9.745] | 0.319 |
| variant-04/04-window-too-tight-unsat | 7.022 [6.872, 7.320] | 1.259 |
| variant-04/05-larger-mix | 34.180 [33.332, 34.940] | 0.216 |
| variant-01/01-basic | 5.873 [5.705, 6.297] | 0.896 |
| variant-01/02-multiple-tours | 6.689 [6.533, 7.317] | 1.218 |
| variant-01/03-asymmetric | 7.624 [7.612, 7.801] | 1.387 |
| variant-01/04-subtour-unsat | 5.580 [4.414, 6.156] | 1.269 |
| variant-01/05-ring | 8.780 [8.730, 11.089] | 1.340 |
| variant-02/01-basic | 6.011 [5.940, 6.275] | 1.104 |
| variant-02/02-single-salesman | 6.802 [6.401, 7.733] | 1.522 |
| variant-02/03-too-many-salesmen-unsat | 5.276 [4.957, 6.004] | 0.959 |
| variant-02/04-equal-cost-split | 6.123 [5.568, 6.707] | 1.115 |
| variant-02/05-larger-asymmetric | 6.569 [6.215, 6.571] | 1.485 |
| variant-02/06-unreachable-edge | 6.249 [6.132, 6.349] | 1.146 |
| variant-03/01-basic | 5.794 [5.176, 7.756] | 1.059 |
| variant-03/02-single-salesman | 6.390 [5.701, 8.228] | 1.160 |
| variant-03/03-depot-crossing-unsat | 5.928 [5.530, 6.197] | 1.332 |
| variant-03/04-equal-cost-split | 6.354 [6.284, 6.555] | 1.422 |
| variant-03/05-larger-three-depots | 7.209 [5.604, 7.649] | 1.314 |
| variant-03/06-unreachable-edge | 6.270 [5.727, 6.668] | 1.413 |
| variant-04/01-basic | 6.534 [5.517, 6.631] | 1.182 |
| variant-04/02-single-salesman | 6.610 [6.581, 6.790] | 1.200 |
| variant-04/03-window-too-tight-unsat | 5.987 [5.938, 6.627] | 1.097 |
| variant-04/04-depot-window-too-tight-unsat | 7.073 [6.574, 7.682] | 1.285 |
| variant-04/05-three-depots-asymmetric-times | 8.382 [7.767, 9.136] | 1.274 |
| variant-04/06-unreachable-edge | 9.288 [8.439, 9.830] | 1.696 |
| variant-05/01-basic | 6.733 [6.714, 6.941] | 1.231 |
| variant-05/02-tight-bound-exact | 6.091 [5.500, 7.152] | 1.375 |
| variant-05/03-revisit-too-tight-unsat | 6.532 [6.151, 6.864] | 1.457 |
| variant-05/04-depot-and-vertex-revisits | 6.595 [6.033, 7.860] | 1.175 |
| variant-05/05-three-depots-mixed | 8.869 [8.156, 8.998] | 1.353 |
| variant-05/06-unreachable-edge | 9.778 [8.836, 10.132] | 1.479 |
| n-queens/variant-01 | 9.418 [8.824, 10.536] | 1.424 |
| n-queens/variant-02 | 66.267 [65.713, 70.625] | 0.545 |
| n-queens/variant-03 | 10.547 [10.190, 11.091] | 1.612 |
| n-queens/variant-04 | 7.163 [7.098, 7.960] | 1.090 |
| n-queens/variant-05 | 8.479 [8.319, 8.665] | 1.281 |
| n-queens/variant-06 | 9.861 [9.036, 10.874] | 1.495 |
| send-money/send-money | 14.048 [13.006, 14.072] | 0.933 |

Reference wall time, ms, same notation.

| Cell | corpus |
|---|---:|
| equality-generalized-tsp/01-basic | 4.420 [4.404, 7.763] |
| equality-generalized-tsp/02-larger | 5.525 [5.514, 5.548] |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.492 [4.395, 5.524] |
| variant-01/01-basic | 4.347 [3.320, 5.494] |
| variant-01/02-start-equals-end | 4.443 [4.403, 5.479] |
| variant-01/03-zero-cost-detour | 4.441 [4.403, 6.623] |
| variant-01/04-no-path | 4.445 [4.385, 4.451] |
| variant-01/05-multi-path | 4.422 [4.393, 4.451] |
| variant-01/06-layered-dag | 5.489 [5.451, 6.575] |
| variant-01/07-cycles | 5.450 [4.451, 5.554] |
| variant-01/08-negative-weights | 4.437 [4.386, 6.644] |
| variant-02/01-basic | 4.437 [4.424, 5.507] |
| variant-02/02-start-equals-end | 4.414 [3.299, 5.512] |
| variant-02/03-before-forces-detour | 5.468 [4.455, 5.482] |
| variant-02/04-after-forces-extension | 4.669 [4.443, 5.503] |
| variant-02/05-ordering-unsat | 5.481 [3.287, 6.667] |
| variant-02/06-layered-dag-before | 5.505 [5.427, 5.512] |
| variant-02/07-layered-dag-before-after | 5.476 [5.464, 6.564] |
| variant-02/08-tie-break-under-ordering | 4.412 [4.391, 4.428] |
| variant-02/09-negative-weights | 4.405 [3.326, 4.411] |
| variant-03/01-basic | 4.423 [4.349, 5.463] |
| variant-03/02-start-equals-end | 4.436 [4.403, 4.446] |
| variant-03/03-budget-forces-detour | 4.445 [4.341, 5.516] |
| variant-03/04-cost-at-cap-allowed | 4.422 [4.397, 5.494] |
| variant-03/05-budget-unsat | 4.515 [4.457, 5.530] |
| variant-03/06-layered-dag-cap | 6.642 [6.558, 7.645] |
| variant-03/07-layered-dag-tight-cap | 6.577 [6.572, 6.585] |
| variant-03/08-tie-break-under-cap | 5.466 [4.463, 5.511] |
| variant-03/09-negative-weights | 5.461 [4.444, 5.591] |
| variant-04/01-basic | 4.412 [4.383, 5.558] |
| variant-04/02-start-equals-end | 4.383 [3.339, 4.390] |
| variant-04/03-before-forces-detour-within-budget | 4.458 [4.402, 5.456] |
| variant-04/04-ordering-violates-budget | 4.429 [4.371, 5.561] |
| variant-04/05-after-and-budget-interact | 4.437 [4.325, 5.511] |
| variant-04/06-layered-dag-ordering-cap | 6.577 [6.567, 7.637] |
| variant-04/07-layered-dag-combined | 6.592 [5.488, 7.624] |
| variant-04/08-tie-break-under-ordering-and-cap | 4.397 [4.346, 5.510] |
| variant-04/09-negative-weights | 4.431 [4.390, 4.457] |
| variant-01/01-basic | 4.404 [3.326, 4.449] |
| variant-01/02-agent-reuse | 4.423 [4.406, 5.479] |
| variant-01/03-selective-compatibility | 4.419 [4.406, 4.433] |
| variant-01/04-no-compatible-agent-unsat | 4.590 [4.546, 5.505] |
| variant-01/05-larger-mix | 4.423 [4.421, 5.482] |
| variant-02/01-basic | 5.397 [4.484, 5.521] |
| variant-02/02-makespan-tiebreak | 4.430 [4.404, 4.433] |
| variant-02/03-cost-dominates | 5.491 [4.391, 6.695] |
| variant-02/04-no-compatible-agent-unsat | 4.430 [4.391, 5.531] |
| variant-02/05-larger-mix | 5.506 [5.505, 5.546] |
| variant-03/01-basic | 4.430 [3.291, 5.487] |
| variant-03/02-multiple-groups | 4.448 [4.448, 5.477] |
| variant-03/03-mixed-grouped-ungrouped | 4.440 [3.297, 4.457] |
| variant-03/04-incompatible-group-unsat | 4.446 [4.403, 5.597] |
| variant-03/05-larger-mix | 4.453 [3.440, 5.521] |
| variant-04/01-basic | 27.976 [25.641, 29.945] |
| variant-04/02-precedence | 6.676 [6.632, 7.598] |
| variant-04/03-agent-serialization | 28.834 [26.664, 28.855] |
| variant-04/04-window-too-tight-unsat | 5.575 [5.475, 5.579] |
| variant-04/05-larger-mix | 158.552 [152.807, 159.531] |
| variant-01/01-basic | 6.556 [5.752, 6.665] |
| variant-01/02-multiple-tours | 5.494 [5.466, 5.508] |
| variant-01/03-asymmetric | 5.496 [4.392, 6.570] |
| variant-01/04-subtour-unsat | 4.396 [3.320, 4.452] |
| variant-01/05-ring | 6.554 [5.476, 6.559] |
| variant-02/01-basic | 5.446 [4.360, 5.505] |
| variant-02/02-single-salesman | 4.470 [4.401, 5.511] |
| variant-02/03-too-many-salesmen-unsat | 5.502 [5.492, 5.548] |
| variant-02/04-equal-cost-split | 5.493 [4.385, 5.507] |
| variant-02/05-larger-asymmetric | 4.423 [4.403, 5.455] |
| variant-02/06-unreachable-edge | 5.452 [4.427, 5.498] |
| variant-03/01-basic | 5.473 [4.495, 5.656] |
| variant-03/02-single-salesman | 5.508 [5.480, 7.769] |
| variant-03/03-depot-crossing-unsat | 4.452 [4.422, 6.563] |
| variant-03/04-equal-cost-split | 4.468 [4.459, 5.503] |
| variant-03/05-larger-three-depots | 5.488 [5.482, 5.498] |
| variant-03/06-unreachable-edge | 4.438 [4.354, 5.418] |
| variant-04/01-basic | 5.528 [4.454, 6.582] |
| variant-04/02-single-salesman | 5.510 [4.415, 5.511] |
| variant-04/03-window-too-tight-unsat | 5.457 [4.393, 5.495] |
| variant-04/04-depot-window-too-tight-unsat | 5.503 [4.483, 6.564] |
| variant-04/05-three-depots-asymmetric-times | 6.579 [4.433, 6.590] |
| variant-04/06-unreachable-edge | 5.475 [4.479, 6.543] |
| variant-05/01-basic | 5.470 [4.417, 5.536] |
| variant-05/02-tight-bound-exact | 4.429 [4.402, 4.446] |
| variant-05/03-revisit-too-tight-unsat | 4.484 [4.409, 5.576] |
| variant-05/04-depot-and-vertex-revisits | 5.612 [4.442, 6.702] |
| variant-05/05-three-depots-mixed | 6.553 [4.471, 6.608] |
| variant-05/06-unreachable-edge | 6.612 [5.497, 7.856] |
| n-queens/variant-01 | 6.614 [5.551, 6.631] |
| n-queens/variant-02 | 121.697 [120.238, 122.676] |
| n-queens/variant-03 | 6.543 [5.748, 7.702] |
| n-queens/variant-04 | 6.573 [6.538, 7.647] |
| n-queens/variant-05 | 6.618 [5.463, 7.662] |
| n-queens/variant-06 | 6.596 [6.564, 6.628] |
| send-money/send-money | 15.063 [12.892, 15.193] |

Counters of report corpus: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 0 | 1 | 4 | 8982 | 2.148 |
| equality-generalized-tsp/02-larger | 0 | 1 | 8 | 20870 | 2.661 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 0 | 0 | 0 | 2258 | 1.505 |
| variant-01/01-basic | 0 | 1 | 2 | 1559 | 1.392 |
| variant-01/02-start-equals-end | 0 | 1 | 1 | 1208 | 1.698 |
| variant-01/03-zero-cost-detour | 0 | 1 | 2 | 1291 | 1.884 |
| variant-01/04-no-path | 0 | 0 | 0 | 1085 | 1.442 |
| variant-01/05-multi-path | 0 | 1 | 3 | 3958 | 1.582 |
| variant-01/06-layered-dag | 0 | 1 | 17 | 135486 | 4.940 |
| variant-01/07-cycles | 0 | 1 | 5 | 13987 | 2.245 |
| variant-01/08-negative-weights | 0 | 1 | 1 | 2442 | 2.324 |
| variant-02/01-basic | 0 | 1 | 2 | 1700 | 1.566 |
| variant-02/02-start-equals-end | 0 | 1 | 1 | 1557 | 1.628 |
| variant-02/03-before-forces-detour | 0 | 1 | 1 | 2383 | 1.760 |
| variant-02/04-after-forces-extension | 0 | 1 | 1 | 2245 | 1.884 |
| variant-02/05-ordering-unsat | 0 | 0 | 0 | 1322 | 1.718 |
| variant-02/06-layered-dag-before | 0 | 2 | 9 | 48810 | 3.892 |
| variant-02/07-layered-dag-before-after | 0 | 1 | 7 | 42762 | 4.020 |
| variant-02/08-tie-break-under-ordering | 0 | 1 | 2 | 2777 | 2.232 |
| variant-02/09-negative-weights | 0 | 1 | 1 | 2816 | 1.833 |
| variant-03/01-basic | 0 | 1 | 2 | 1667 | 1.754 |
| variant-03/02-start-equals-end | 0 | 1 | 1 | 1467 | 1.650 |
| variant-03/03-budget-forces-detour | 0 | 1 | 1 | 2141 | 1.635 |
| variant-03/04-cost-at-cap-allowed | 0 | 1 | 1 | 1272 | 1.734 |
| variant-03/05-budget-unsat | 0 | 0 | 0 | 4970 | 2.579 |
| variant-03/06-layered-dag-cap | 0 | 2 | 10 | 273988 | 6.918 |
| variant-03/07-layered-dag-tight-cap | 0 | 4 | 6 | 464436 | 9.104 |
| variant-03/08-tie-break-under-cap | 0 | 1 | 2 | 3301 | 2.373 |
| variant-03/09-negative-weights | 0 | 1 | 1 | 2872 | 2.238 |
| variant-04/01-basic | 0 | 1 | 2 | 1808 | 1.868 |
| variant-04/02-start-equals-end | 0 | 1 | 1 | 1816 | 1.915 |
| variant-04/03-before-forces-detour-within-budget | 0 | 1 | 1 | 2774 | 1.846 |
| variant-04/04-ordering-violates-budget | 0 | 0 | 0 | 2816 | 2.983 |
| variant-04/05-after-and-budget-interact | 0 | 1 | 1 | 4169 | 2.121 |
| variant-04/06-layered-dag-ordering-cap | 0 | 2 | 7 | 73132 | 6.548 |
| variant-04/07-layered-dag-combined | 0 | 1 | 5 | 60073 | 6.245 |
| variant-04/08-tie-break-under-ordering-and-cap | 0 | 1 | 2 | 6554 | 2.364 |
| variant-04/09-negative-weights | 0 | 1 | 1 | 3246 | 2.220 |
| variant-01/01-basic | 0 | 1 | 4 | 1462 | 1.186 |
| variant-01/02-agent-reuse | 0 | 1 | 8 | 2131 | 1.239 |
| variant-01/03-selective-compatibility | 0 | 1 | 4 | 1873 | 1.554 |
| variant-01/04-no-compatible-agent-unsat | 0 | 0 | 0 | 1217 | 1.678 |
| variant-01/05-larger-mix | 0 | 1 | 54 | 38407 | 2.014 |
| variant-02/01-basic | 0 | 1 | 4 | 3047 | 1.542 |
| variant-02/02-makespan-tiebreak | 0 | 3 | 8 | 4031 | 1.839 |
| variant-02/03-cost-dominates | 0 | 1 | 8 | 4006 | 2.003 |
| variant-02/04-no-compatible-agent-unsat | 0 | 0 | 0 | 2395 | 1.670 |
| variant-02/05-larger-mix | 0 | 1 | 14 | 207638 | 5.068 |
| variant-03/01-basic | 0 | 2 | 2 | 1970 | 1.747 |
| variant-03/02-multiple-groups | 0 | 4 | 4 | 3816 | 1.696 |
| variant-03/03-mixed-grouped-ungrouped | 0 | 2 | 4 | 2639 | 1.738 |
| variant-03/04-incompatible-group-unsat | 0 | 0 | 0 | 1224 | 1.865 |
| variant-03/05-larger-mix | 0 | 1 | 22 | 7304 | 2.227 |
| variant-04/01-basic | 0 | 81 | 105 | 111825 | 5.760 |
| variant-04/02-precedence | 0 | 3 | 10 | 12342 | 3.823 |
| variant-04/03-agent-serialization | 0 | 30 | 30 | 32344 | 5.080 |
| variant-04/04-window-too-tight-unsat | 0 | 0 | 0 | 2865 | 3.058 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 2095178 | 30.205 |
| variant-01/01-basic | 0 | 1 | 2 | 5138 | 1.969 |
| variant-01/02-multiple-tours | 0 | 2 | 9 | 35544 | 2.793 |
| variant-01/03-asymmetric | 0 | 1 | 9 | 54815 | 3.675 |
| variant-01/04-subtour-unsat | 0 | 0 | 0 | 2473 | 1.325 |
| variant-01/05-ring | 0 | 2 | 26 | 345587 | 4.775 |
| variant-02/01-basic | 0 | 2 | 2 | 5689 | 2.080 |
| variant-02/02-single-salesman | 0 | 1 | 2 | 3644 | 2.259 |
| variant-02/03-too-many-salesmen-unsat | 0 | 0 | 0 | 3466 | 1.597 |
| variant-02/04-equal-cost-split | 0 | 2 | 2 | 4647 | 2.272 |
| variant-02/05-larger-asymmetric | 0 | 2 | 2 | 6731 | 2.468 |
| variant-02/06-unreachable-edge | 0 | 2 | 2 | 7627 | 2.315 |
| variant-03/01-basic | 0 | 1 | 1 | 3939 | 1.697 |
| variant-03/02-single-salesman | 0 | 1 | 2 | 3498 | 2.682 |
| variant-03/03-depot-crossing-unsat | 0 | 0 | 0 | 3422 | 1.845 |
| variant-03/04-equal-cost-split | 0 | 2 | 2 | 7949 | 2.589 |
| variant-03/05-larger-three-depots | 0 | 1 | 1 | 10821 | 3.264 |
| variant-03/06-unreachable-edge | 0 | 1 | 1 | 6365 | 2.223 |
| variant-04/01-basic | 0 | 1 | 1 | 6955 | 2.817 |
| variant-04/02-single-salesman | 0 | 1 | 1 | 4700 | 2.781 |
| variant-04/03-window-too-tight-unsat | 0 | 0 | 0 | 3400 | 2.106 |
| variant-04/04-depot-window-too-tight-unsat | 0 | 0 | 0 | 2325 | 2.673 |
| variant-04/05-three-depots-asymmetric-times | 0 | 1 | 1 | 18328 | 4.383 |
| variant-04/06-unreachable-edge | 0 | 1 | 1 | 14062 | 4.061 |
| variant-05/01-basic | 0 | 1 | 1 | 6957 | 2.824 |
| variant-05/02-tight-bound-exact | 0 | 1 | 1 | 3439 | 2.442 |
| variant-05/03-revisit-too-tight-unsat | 0 | 0 | 0 | 3413 | 2.755 |
| variant-05/04-depot-and-vertex-revisits | 0 | 1 | 1 | 3504 | 2.558 |
| variant-05/05-three-depots-mixed | 0 | 1 | 1 | 18378 | 4.567 |
| variant-05/06-unreachable-edge | 0 | 1 | 1 | 14062 | 4.978 |
| n-queens/variant-01 | 0 | 92 | 92 | 199032 | 5.965 |
| n-queens/variant-02 | 0 | 92 | 92 | 14461401 | 61.371 |
| n-queens/variant-03 | 0 | 92 | 92 | 256834 | 6.135 |
| n-queens/variant-04 | 0 | 92 | 92 | 195309 | 3.738 |
| n-queens/variant-05 | 0 | 92 | 92 | 223549 | 4.995 |
| n-queens/variant-06 | 0 | 92 | 92 | 251111 | 5.771 |
| send-money/send-money | 0 | 1 | 1 | 409024 | 10.278 |

Against the reference: report corpus, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 9 of 94 cells where both passed (9.5%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 34.180 | 158.552 | 0.216 | 3.284 | 32.581 | 12.915 | 88.000 | 62.000 |
| variant-04/03-agent-serialization | 9.207 | 28.834 | 0.319 | 0.933 | 1.411 | 0.320 | 24.000 | 0.000 |
| variant-04/01-basic | 10.249 | 27.976 | 0.366 | 1.474 | 2.692 | 0.617 | 18.000 | 4.000 |
| n-queens/variant-02 | 66.267 | 121.697 | 0.545 | 3.816 | 180.814 | 1.296 | 1.000 | 116.000 |
| variant-01/01-basic | 5.873 | 6.556 | 0.896 | 0.292 | 0.138 | 0.119 | 1.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.967 | 5.492 | 0.904 | 0.208 | 0.047 | 0.013 | 1.000 | 0.000 |
| send-money/send-money | 14.048 | 15.063 | 0.933 | 2.581 | 6.478 | 0.282 | 9.000 | 1.000 |
| variant-02/03-too-many-salesmen-unsat | 5.276 | 5.502 | 0.959 | 0.297 | 0.062 | 0.011 | 1.000 | 0.000 |
| variant-02/01-basic | 5.192 | 5.397 | 0.962 | 0.298 | 0.139 | 0.032 | 1.000 | 0.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-02/05-ordering-unsat | 5.515 | 5.481 | 1.006 | 0.174 | 0.043 | 0.005 | 1.000 | 0.000 |
| variant-02/03-before-forces-detour | 5.664 | 5.468 | 1.036 | 0.221 | 0.062 | 0.012 | 1.000 | 0.000 |
| variant-03/01-basic | 5.794 | 5.473 | 1.059 | 0.309 | 0.078 | 0.051 | 1.000 | 0.000 |
| variant-03/09-negative-weights | 5.805 | 5.461 | 1.063 | 0.313 | 0.076 | 0.048 | 1.000 | 0.000 |
| variant-02/01-basic | 4.766 | 4.437 | 1.074 | 0.195 | 0.066 | 0.018 | 1.000 | 0.000 |
| variant-01/07-cycles | 5.858 | 5.450 | 1.075 | 0.394 | 0.288 | 0.386 | 1.000 | 0.000 |
| n-queens/variant-04 | 7.163 | 6.573 | 1.090 | 0.737 | 4.506 | 0.648 | 1.000 | 1.000 |
| variant-02/03-cost-dominates | 5.986 | 5.491 | 1.090 | 0.330 | 0.253 | 0.060 | 1.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 5.987 | 5.457 | 1.097 | 0.389 | 0.063 | 0.011 | 1.000 | 0.000 |
| variant-02/01-basic | 6.011 | 5.446 | 1.104 | 0.384 | 0.141 | 0.126 | 1.000 | 0.000 |
| variant-01/04-no-path | 4.924 | 4.445 | 1.108 | 0.179 | 0.030 | 0.005 | 1.000 | 0.000 |
| variant-02/04-equal-cost-split | 6.123 | 5.493 | 1.115 | 0.324 | 0.120 | 0.105 | 1.000 | 0.000 |
| variant-02/06-unreachable-edge | 6.249 | 5.452 | 1.146 | 0.519 | 0.170 | 0.157 | 1.000 | 0.000 |
| variant-04/02-precedence | 7.738 | 6.676 | 1.159 | 0.625 | 0.688 | 0.117 | 2.000 | 0.000 |
| variant-01/02-agent-reuse | 5.130 | 4.423 | 1.160 | 0.164 | 0.116 | 0.032 | 0.000 | 0.000 |
| variant-03/02-single-salesman | 6.390 | 5.508 | 1.160 | 0.270 | 0.093 | 0.088 | 1.000 | 0.000 |
| variant-01/03-selective-compatibility | 5.128 | 4.419 | 1.160 | 0.183 | 0.080 | 0.019 | 0.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 6.376 | 5.466 | 1.167 | 0.289 | 0.103 | 0.024 | 1.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 6.595 | 5.612 | 1.175 | 0.419 | 0.074 | 0.051 | 1.000 | 0.000 |
| variant-04/01-basic | 6.534 | 5.528 | 1.182 | 0.621 | 0.112 | 0.080 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 6.610 | 5.510 | 1.200 | 0.485 | 0.085 | 0.059 | 1.000 | 0.000 |
| variant-01/01-basic | 5.228 | 4.347 | 1.203 | 0.166 | 0.071 | 0.013 | 1.000 | 0.000 |
| equality-generalized-tsp/02-larger | 6.664 | 5.525 | 1.206 | 0.613 | 0.398 | 0.573 | 1.000 | 0.000 |
| variant-01/02-multiple-tours | 6.689 | 5.494 | 1.218 | 0.441 | 0.583 | 1.046 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 5.424 | 4.437 | 1.222 | 0.316 | 0.091 | 0.022 | 1.000 | 0.000 |
| variant-05/01-basic | 6.733 | 5.470 | 1.231 | 0.614 | 0.116 | 0.087 | 1.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 5.694 | 4.590 | 1.241 | 0.233 | 0.027 | 0.005 | 0.000 | 0.000 |
| variant-01/05-multi-path | 5.494 | 4.422 | 1.242 | 0.289 | 0.159 | 0.029 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 5.541 | 4.448 | 1.246 | 0.219 | 0.122 | 0.032 | 1.000 | 0.000 |
| variant-04/01-basic | 5.497 | 4.412 | 1.246 | 0.207 | 0.079 | 0.018 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 5.540 | 4.445 | 1.246 | 0.218 | 0.060 | 0.013 | 1.000 | 0.000 |
| variant-03/01-basic | 5.520 | 4.423 | 1.248 | 0.232 | 0.070 | 0.014 | 1.000 | 0.000 |
| variant-02/09-negative-weights | 5.499 | 4.405 | 1.248 | 0.257 | 0.086 | 0.056 | 1.000 | 0.000 |
| variant-02/02-start-equals-end | 5.535 | 4.414 | 1.254 | 0.182 | 0.042 | 0.029 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 5.569 | 4.440 | 1.254 | 0.216 | 0.108 | 0.024 | 0.000 | 0.000 |
| variant-01/02-start-equals-end | 5.579 | 4.443 | 1.256 | 0.168 | 0.042 | 0.025 | 1.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 7.022 | 5.575 | 1.259 | 0.328 | 0.087 | 0.011 | 1.000 | 0.000 |
| variant-01/04-subtour-unsat | 5.580 | 4.396 | 1.269 | 0.181 | 0.034 | 0.010 | 0.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 8.382 | 6.579 | 1.274 | 1.515 | 0.230 | 0.197 | 1.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 5.674 | 4.446 | 1.276 | 0.147 | 0.035 | 0.005 | 0.000 | 0.000 |
| variant-04/02-start-equals-end | 5.598 | 4.383 | 1.277 | 0.203 | 0.049 | 0.032 | 1.000 | 0.000 |
| variant-01/01-basic | 5.634 | 4.404 | 1.279 | 0.145 | 0.083 | 0.020 | 0.000 | 0.000 |
| n-queens/variant-05 | 8.479 | 6.618 | 1.281 | 1.019 | 5.029 | 0.908 | 1.000 | 1.000 |
| variant-03/02-start-equals-end | 5.700 | 4.436 | 1.285 | 0.162 | 0.044 | 0.027 | 1.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 7.073 | 5.503 | 1.285 | 0.320 | 0.055 | 0.010 | 1.000 | 0.000 |
| variant-03/04-cost-at-cap-allowed | 5.717 | 4.422 | 1.293 | 0.216 | 0.048 | 0.009 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 5.804 | 4.458 | 1.302 | 0.266 | 0.076 | 0.014 | 1.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 5.780 | 4.430 | 1.305 | 0.291 | 0.046 | 0.009 | 0.000 | 0.000 |
| variant-03/05-larger-three-depots | 7.209 | 5.488 | 1.314 | 0.838 | 0.166 | 0.114 | 1.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 5.809 | 4.412 | 1.317 | 0.562 | 0.075 | 0.015 | 1.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 5.867 | 4.430 | 1.325 | 0.354 | 0.233 | 0.058 | 1.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 5.928 | 4.452 | 1.332 | 0.333 | 0.064 | 0.012 | 1.000 | 0.000 |
| variant-01/05-ring | 8.780 | 6.554 | 1.340 | 0.706 | 3.178 | 1.707 | 1.000 | 0.000 |
| variant-01/03-zero-cost-detour | 5.957 | 4.441 | 1.341 | 0.202 | 0.057 | 0.012 | 1.000 | 0.000 |
| variant-02/04-after-forces-extension | 6.286 | 4.669 | 1.346 | 0.245 | 0.063 | 0.013 | 1.000 | 0.000 |
| variant-05/05-three-depots-mixed | 8.869 | 6.553 | 1.353 | 1.537 | 0.251 | 0.207 | 1.000 | 0.000 |
| variant-01/05-larger-mix | 6.013 | 4.423 | 1.359 | 0.255 | 1.587 | 0.166 | 0.000 | 0.000 |
| variant-03/01-basic | 6.087 | 4.430 | 1.374 | 0.170 | 0.064 | 0.014 | 0.000 | 0.000 |
| variant-05/02-tight-bound-exact | 6.091 | 4.429 | 1.375 | 0.379 | 0.069 | 0.050 | 1.000 | 0.000 |
| variant-04/09-negative-weights | 6.107 | 4.431 | 1.378 | 0.305 | 0.095 | 0.058 | 1.000 | 0.000 |
| equality-generalized-tsp/01-basic | 6.104 | 4.420 | 1.381 | 0.393 | 0.194 | 0.235 | 1.000 | 0.000 |
| variant-01/03-asymmetric | 7.624 | 5.496 | 1.387 | 0.599 | 0.724 | 1.404 | 1.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 6.182 | 4.429 | 1.396 | 0.294 | 0.078 | 0.010 | 1.000 | 0.000 |
| variant-01/08-negative-weights | 6.248 | 4.437 | 1.408 | 0.259 | 0.068 | 0.041 | 1.000 | 0.000 |
| variant-03/06-unreachable-edge | 6.270 | 4.438 | 1.413 | 0.528 | 0.127 | 0.076 | 1.000 | 0.000 |
| variant-02/07-layered-dag-before-after | 7.752 | 5.476 | 1.416 | 1.122 | 1.651 | 0.261 | 2.000 | 0.000 |
| variant-03/04-equal-cost-split | 6.354 | 4.468 | 1.422 | 0.470 | 0.164 | 0.155 | 1.000 | 0.000 |
| n-queens/variant-01 | 9.418 | 6.614 | 1.424 | 2.785 | 4.301 | 0.826 | 1.000 | 1.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.261 | 4.397 | 1.424 | 0.435 | 0.195 | 0.039 | 1.000 | 0.000 |
| variant-02/06-layered-dag-before | 7.846 | 5.505 | 1.425 | 1.112 | 1.754 | 0.293 | 2.000 | 0.000 |
| variant-01/06-layered-dag | 7.830 | 5.489 | 1.426 | 1.019 | 4.008 | 0.300 | 2.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 6.532 | 4.484 | 1.457 | 0.460 | 0.075 | 0.012 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 6.654 | 4.515 | 1.474 | 0.355 | 0.120 | 0.014 | 1.000 | 0.000 |
| variant-05/06-unreachable-edge | 9.778 | 6.612 | 1.479 | 1.245 | 0.207 | 0.166 | 1.000 | 0.000 |
| variant-03/05-larger-mix | 6.588 | 4.453 | 1.480 | 0.290 | 0.522 | 0.109 | 1.000 | 0.000 |
| variant-02/05-larger-asymmetric | 6.569 | 4.423 | 1.485 | 0.457 | 0.113 | 0.106 | 1.000 | 0.000 |
| n-queens/variant-06 | 9.861 | 6.596 | 1.495 | 1.638 | 5.333 | 1.018 | 1.000 | 1.000 |
| variant-02/02-single-salesman | 6.802 | 4.470 | 1.522 | 0.262 | 0.095 | 0.089 | 1.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 10.278 | 6.577 | 1.563 | 1.831 | 2.758 | 0.467 | 2.000 | 0.000 |
| variant-04/07-layered-dag-combined | 10.449 | 6.592 | 1.585 | 1.771 | 2.355 | 0.357 | 3.000 | 0.000 |
| n-queens/variant-03 | 10.547 | 6.543 | 1.612 | 1.925 | 5.268 | 0.935 | 1.000 | 1.000 |
| variant-02/05-larger-mix | 9.321 | 5.506 | 1.693 | 0.741 | 5.248 | 0.182 | 1.000 | 0.000 |
| variant-04/06-unreachable-edge | 9.288 | 5.475 | 1.696 | 1.195 | 0.212 | 0.172 | 1.000 | 0.000 |
| variant-03/06-layered-dag-cap | 11.422 | 6.642 | 1.720 | 1.690 | 5.456 | 0.428 | 3.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 12.969 | 6.577 | 1.972 | 1.682 | 6.989 | 0.285 | 2.000 | 1.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 16.0 | 23.6 | n/a |
| variant-04/03-agent-serialization | 11.5 | 12.3 | n/a |
| variant-04/01-basic | 12.1 | 13.3 | n/a |
| n-queens/variant-02 | 16.3 | 12.8 | n/a |
| variant-01/01-basic | 10.6 | 10.5 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.5 | 10.4 | n/a |
| send-money/send-money | 14.1 | 12.8 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.4 | 10.4 | n/a |
| variant-02/01-basic | 10.4 | 10.4 | n/a |
| variant-02/05-ordering-unsat | 10.5 | 10.4 | n/a |
| variant-02/03-before-forces-detour | 10.6 | 10.5 | n/a |
| variant-03/01-basic | 10.4 | 10.4 | n/a |
| variant-03/09-negative-weights | 10.2 | 10.4 | n/a |
| variant-02/01-basic | 10.6 | 10.5 | n/a |
| variant-01/07-cycles | 10.6 | 10.4 | n/a |
| n-queens/variant-04 | 10.8 | 10.6 | n/a |
| variant-02/03-cost-dominates | 10.5 | 10.3 | n/a |
| variant-04/03-window-too-tight-unsat | 10.7 | 10.5 | n/a |
| variant-02/01-basic | 10.8 | 10.5 | n/a |
| variant-01/04-no-path | 10.3 | 10.4 | n/a |
| variant-02/04-equal-cost-split | 10.5 | 10.2 | n/a |
| variant-02/06-unreachable-edge | 10.6 | 10.4 | n/a |
| variant-04/02-precedence | 11.6 | 10.7 | n/a |
| variant-01/02-agent-reuse | 10.4 | 10.3 | n/a |
| variant-03/02-single-salesman | 10.5 | 10.5 | n/a |
| variant-01/03-selective-compatibility | 10.2 | 10.2 | n/a |
| variant-03/08-tie-break-under-cap | 10.6 | 10.3 | n/a |
| variant-05/04-depot-and-vertex-revisits | 10.8 | 10.5 | n/a |
| variant-04/01-basic | 10.9 | 10.5 | n/a |
| variant-04/02-single-salesman | 10.9 | 10.4 | n/a |
| variant-01/01-basic | 10.4 | 10.4 | n/a |
| equality-generalized-tsp/02-larger | 10.6 | 10.3 | n/a |
| variant-01/02-multiple-tours | 11.2 | 10.6 | n/a |
| variant-04/05-after-and-budget-interact | 10.9 | 10.6 | n/a |
| variant-05/01-basic | 10.8 | 10.4 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.0 | 10.4 | n/a |
| variant-01/05-multi-path | 9.9 | 10.4 | n/a |
| variant-03/02-multiple-groups | 10.6 | 10.4 | n/a |
| variant-04/01-basic | 10.7 | 10.6 | n/a |
| variant-03/03-budget-forces-detour | 10.4 | 10.6 | n/a |
| variant-03/01-basic | 10.4 | 10.4 | n/a |
| variant-02/09-negative-weights | 10.8 | 10.5 | n/a |
| variant-02/02-start-equals-end | 10.5 | 10.5 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.2 | 10.4 | n/a |
| variant-01/02-start-equals-end | 10.2 | 10.5 | n/a |
| variant-04/04-window-too-tight-unsat | 11.3 | 10.5 | n/a |
| variant-01/04-subtour-unsat | 10.1 | 10.2 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.2 | 10.6 | n/a |
| variant-03/04-incompatible-group-unsat | 10.4 | 10.2 | n/a |
| variant-04/02-start-equals-end | 10.8 | 10.3 | n/a |
| variant-01/01-basic | 10.0 | 10.2 | n/a |
| n-queens/variant-05 | 11.5 | 10.6 | n/a |
| variant-03/02-start-equals-end | 10.5 | 10.4 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 11.0 | 10.4 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.5 | 10.5 | n/a |
| variant-04/03-before-forces-detour-within-budget | 10.8 | 10.5 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.3 | 10.4 | n/a |
| variant-03/05-larger-three-depots | 10.8 | 10.5 | n/a |
| variant-02/08-tie-break-under-ordering | 10.4 | 10.5 | n/a |
| variant-02/02-makespan-tiebreak | 10.4 | 10.5 | n/a |
| variant-03/03-depot-crossing-unsat | 10.4 | 10.2 | n/a |
| variant-01/05-ring | 15.4 | 10.6 | n/a |
| variant-01/03-zero-cost-detour | 10.5 | 10.4 | n/a |
| variant-02/04-after-forces-extension | 10.5 | 10.6 | n/a |
| variant-05/05-three-depots-mixed | 11.5 | 10.6 | n/a |
| variant-01/05-larger-mix | 10.3 | 10.2 | n/a |
| variant-03/01-basic | 10.5 | 10.3 | n/a |
| variant-05/02-tight-bound-exact | 10.7 | 10.6 | n/a |
| variant-04/09-negative-weights | 10.8 | 10.6 | n/a |
| equality-generalized-tsp/01-basic | 10.7 | 10.4 | n/a |
| variant-01/03-asymmetric | 12.5 | 10.7 | n/a |
| variant-04/04-ordering-violates-budget | 10.7 | 10.4 | n/a |
| variant-01/08-negative-weights | 10.4 | 10.3 | n/a |
| variant-03/06-unreachable-edge | 10.7 | 10.6 | n/a |
| variant-02/07-layered-dag-before-after | 11.7 | 10.6 | n/a |
| variant-03/04-equal-cost-split | 10.8 | 10.6 | n/a |
| n-queens/variant-01 | 11.2 | 10.5 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 10.8 | 10.5 | n/a |
| variant-02/06-layered-dag-before | 11.3 | 10.8 | n/a |
| variant-01/06-layered-dag | 11.2 | 10.6 | n/a |
| variant-05/03-revisit-too-tight-unsat | 10.9 | 10.4 | n/a |
| variant-03/05-budget-unsat | 10.4 | 10.6 | n/a |
| variant-05/06-unreachable-edge | 11.3 | 10.7 | n/a |
| variant-03/05-larger-mix | 10.5 | 10.4 | n/a |
| variant-02/05-larger-asymmetric | 10.4 | 10.4 | n/a |
| n-queens/variant-06 | 11.9 | 10.6 | n/a |
| variant-02/02-single-salesman | 10.6 | 10.4 | n/a |
| variant-04/06-layered-dag-ordering-cap | 13.3 | 11.0 | n/a |
| variant-04/07-layered-dag-combined | 12.9 | 10.9 | n/a |
| n-queens/variant-03 | 11.2 | 10.4 | n/a |
| variant-02/05-larger-mix | 11.7 | 10.4 | n/a |
| variant-04/06-unreachable-edge | 11.1 | 10.7 | n/a |
| variant-03/06-layered-dag-cap | 17.6 | 11.0 | n/a |
| variant-03/07-layered-dag-tight-cap | 14.0 | 10.9 | n/a |
