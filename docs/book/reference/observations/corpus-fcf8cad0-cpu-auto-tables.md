Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | main | after | after/main | main/reference | after/reference |
|---|---:|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 6.402 [6.276, 6.668] | 6.770 [5.882, 7.221] | 1.058 | 1.159 | 1.501 |
| equality-generalized-tsp/02-larger | 6.042 [5.612, 6.706] | 6.562 [6.379, 6.855] | 1.086 | 1.101 | 1.434 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.901 [4.382, 4.987] | 5.140 [4.647, 5.532] | 1.049 | 1.102 | 1.154 |
| variant-01/01-basic | 4.063 [4.021, 7.525] | 4.704 [4.370, 5.108] | 1.158 | 0.900 | 1.064 |
| variant-01/02-start-equals-end | 5.138 [4.442, 5.527] | 5.490 [5.287, 5.527] | 1.069 | 1.139 | 1.236 |
| variant-01/03-zero-cost-detour | 5.555 [4.537, 6.927] | 5.325 [4.674, 5.515] | 0.959 | 1.236 | 1.200 |
| variant-01/04-no-path | 5.539 [4.699, 6.179] | 5.452 [5.050, 6.044] | 0.984 | 1.237 | 1.235 |
| variant-01/05-multi-path | 4.893 [4.701, 5.572] | 5.129 [5.073, 6.264] | 1.048 | 0.894 | 1.170 |
| variant-01/06-layered-dag | 8.785 [7.736, 8.885] | 8.062 [7.628, 8.766] | 0.918 | 1.596 | 1.230 |
| variant-01/07-cycles | 5.664 [5.590, 6.614] | 6.211 [5.836, 6.318] | 1.097 | 1.028 | 1.399 |
| variant-01/08-negative-weights | 6.229 [5.801, 7.817] | 6.218 [6.199, 6.243] | 0.998 | 1.099 | 1.403 |
| variant-02/01-basic | 5.156 [5.012, 6.009] | 5.538 [4.829, 5.744] | 1.074 | 1.166 | 1.014 |
| variant-02/02-start-equals-end | 5.428 [5.051, 5.828] | 5.551 [5.053, 6.240] | 1.023 | 1.223 | 1.015 |
| variant-02/03-before-forces-detour | 5.171 [4.867, 5.770] | 5.517 [5.459, 5.538] | 1.067 | 1.179 | 1.237 |
| variant-02/04-after-forces-extension | 5.546 [4.933, 5.966] | 5.207 [4.525, 5.829] | 0.939 | 1.256 | 1.171 |
| variant-02/05-ordering-unsat | 5.520 [4.773, 5.686] | 5.285 [5.269, 5.571] | 0.957 | 1.240 | 1.187 |
| variant-02/06-layered-dag-before | 9.140 [8.524, 11.024] | 7.770 [7.765, 8.455] | 0.850 | 1.399 | 1.407 |
| variant-02/07-layered-dag-before-after | 8.752 [7.724, 8.929] | 7.727 [7.029, 7.973] | 0.883 | 1.563 | 1.399 |
| variant-02/08-tie-break-under-ordering | 5.212 [4.972, 5.482] | 5.526 [4.717, 6.556] | 1.060 | 1.173 | 1.249 |
| variant-02/09-negative-weights | 5.847 [5.695, 6.644] | 5.596 [4.894, 5.785] | 0.957 | 1.060 | 1.270 |
| variant-03/01-basic | 5.119 [4.588, 5.593] | 5.160 [4.958, 6.315] | 1.008 | 1.140 | 1.163 |
| variant-03/02-start-equals-end | 5.538 [5.208, 5.622] | 6.045 [4.534, 6.061] | 1.092 | 1.003 | 1.361 |
| variant-03/03-budget-forces-detour | 5.549 [5.000, 5.643] | 6.181 [5.844, 7.252] | 1.114 | 1.268 | 1.124 |
| variant-03/04-cost-at-cap-allowed | 4.605 [3.425, 5.045] | 5.142 [4.497, 5.532] | 1.117 | 1.038 | 1.145 |
| variant-03/05-budget-unsat | 5.527 [4.959, 5.586] | 5.668 [5.486, 6.572] | 1.025 | 1.011 | 1.274 |
| variant-03/06-layered-dag-cap | 10.993 [10.837, 11.023] | 10.794 [9.854, 10.956] | 0.982 | 1.445 | 1.642 |
| variant-03/07-layered-dag-tight-cap | 9.838 [9.088, 10.077] | 8.799 [8.687, 10.391] | 0.894 | 1.494 | 1.338 |
| variant-03/08-tie-break-under-cap | 5.534 [5.218, 7.740] | 5.529 [5.090, 6.512] | 0.999 | 1.244 | 1.022 |
| variant-03/09-negative-weights | 5.904 [5.857, 6.337] | 6.628 [5.947, 8.030] | 1.123 | 1.334 | 1.488 |
| variant-04/01-basic | 7.730 [6.266, 7.865] | 5.531 [5.507, 5.933] | 0.715 | 1.728 | 1.015 |
| variant-04/02-start-equals-end | 6.198 [5.274, 7.552] | 5.611 [5.543, 6.648] | 0.905 | 1.093 | 1.258 |
| variant-04/03-before-forces-detour-within-budget | 5.552 [4.508, 5.674] | 5.920 [5.898, 6.634] | 1.066 | 1.010 | 1.309 |
| variant-04/04-ordering-violates-budget | 5.693 [5.690, 5.856] | 5.795 [5.547, 6.545] | 1.018 | 1.037 | 1.310 |
| variant-04/05-after-and-budget-interact | 6.003 [5.684, 8.419] | 5.585 [5.476, 6.578] | 0.930 | 1.098 | 1.024 |
| variant-04/06-layered-dag-ordering-cap | 9.917 [8.692, 11.487] | 9.895 [9.104, 10.849] | 0.998 | 1.511 | 1.509 |
| variant-04/07-layered-dag-combined | 9.053 [8.737, 10.172] | 9.812 [9.150, 11.185] | 1.084 | 1.375 | 1.502 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.570 [5.887, 6.796] | 6.603 [5.947, 6.942] | 1.005 | 1.203 | 1.203 |
| variant-04/09-negative-weights | 5.661 [5.624, 6.045] | 5.956 [5.632, 6.606] | 1.052 | 1.026 | 1.339 |
| variant-01/01-basic | 4.714 [4.662, 4.928] | 5.568 [4.905, 6.682] | 1.181 | 1.059 | 1.252 |
| variant-01/02-agent-reuse | 4.712 [4.680, 5.927] | 4.910 [4.600, 5.709] | 1.042 | 1.056 | 1.105 |
| variant-01/03-selective-compatibility | 4.951 [4.193, 6.306] | 4.593 [4.484, 5.873] | 0.928 | 1.119 | 1.024 |
| variant-01/04-no-compatible-agent-unsat | 5.182 [4.786, 6.470] | 4.605 [4.429, 4.643] | 0.889 | 1.168 | 1.034 |
| variant-01/05-larger-mix | 6.712 [5.156, 6.799] | 6.558 [5.965, 7.198] | 0.977 | 1.522 | 1.474 |
| variant-02/01-basic | 5.604 [4.562, 6.046] | 4.923 [4.827, 7.111] | 0.878 | 1.017 | 1.118 |
| variant-02/02-makespan-tiebreak | 5.525 [4.619, 8.926] | 5.615 [5.535, 5.769] | 1.016 | 0.997 | 1.266 |
| variant-02/03-cost-dominates | 5.600 [5.572, 5.864] | 6.040 [5.545, 6.280] | 1.078 | 1.005 | 1.110 |
| variant-02/04-no-compatible-agent-unsat | 5.646 [4.652, 6.720] | 5.838 [5.652, 5.949] | 1.034 | 1.206 | 1.315 |
| variant-02/05-larger-mix | 11.212 [10.840, 11.423] | 10.970 [9.945, 11.228] | 0.978 | 2.008 | 1.997 |
| variant-03/01-basic | 4.926 [4.596, 5.867] | 5.163 [4.375, 5.511] | 1.048 | 0.867 | 1.167 |
| variant-03/02-multiple-groups | 5.101 [5.075, 5.842] | 6.048 [5.549, 6.556] | 1.186 | 1.150 | 1.362 |
| variant-03/03-mixed-grouped-ungrouped | 4.713 [4.595, 5.280] | 5.949 [5.566, 6.653] | 1.262 | 1.060 | 1.348 |
| variant-03/04-incompatible-group-unsat | 4.589 [4.563, 4.636] | 5.691 [4.988, 6.621] | 1.240 | 1.027 | 1.285 |
| variant-03/05-larger-mix | 6.285 [6.164, 7.097] | 5.701 [5.514, 6.293] | 0.907 | 1.148 | 1.287 |
| variant-04/01-basic | 20.893 [19.392, 23.256] | 20.749 [20.693, 21.145] | 0.993 | 0.709 | 0.776 |
| variant-04/02-precedence | 6.742 [6.180, 10.256] | 7.411 [7.126, 7.726] | 1.099 | 1.011 | 0.968 |
| variant-04/03-agent-serialization | 11.208 [10.055, 11.564] | 11.906 [9.899, 12.565] | 1.062 | 0.373 | 0.429 |
| variant-04/04-window-too-tight-unsat | 5.979 [5.778, 6.759] | 6.995 [6.091, 7.352] | 1.170 | 1.074 | 1.272 |
| variant-04/05-larger-mix | blocked by capture_limit ×3 | blocked by capture_limit ×3 | n/a | n/a | n/a |
| variant-01/01-basic | 6.327 [6.158, 6.821] | 6.478 [5.939, 6.588] | 1.024 | 1.110 | 1.419 |
| variant-01/02-multiple-tours | 7.462 [6.848, 8.003] | 6.915 [6.612, 7.953] | 0.927 | 1.661 | 1.254 |
| variant-01/03-asymmetric | 7.156 [6.964, 8.174] | 8.230 [7.132, 8.807] | 1.150 | 1.077 | 1.810 |
| variant-01/04-subtour-unsat | 4.644 [4.642, 5.563] | 5.501 [4.925, 5.523] | 1.184 | 1.053 | 0.991 |
| variant-01/05-ring | 13.503 [11.923, 14.775] | 12.427 [12.036, 13.191] | 0.920 | 2.471 | 2.270 |
| variant-02/01-basic | 6.551 [5.550, 6.742] | 5.820 [5.589, 6.017] | 0.888 | 1.198 | 1.320 |
| variant-02/02-single-salesman | 6.115 [4.873, 7.327] | 5.737 [5.530, 6.970] | 0.938 | 1.402 | 1.273 |
| variant-02/03-too-many-salesmen-unsat | 4.964 [4.418, 5.507] | 5.701 [4.906, 5.848] | 1.148 | 1.114 | 1.281 |
| variant-02/04-equal-cost-split | 6.592 [5.521, 6.607] | 5.537 [5.448, 6.623] | 0.840 | 1.470 | 1.244 |
| variant-02/05-larger-asymmetric | 6.250 [5.670, 6.698] | 6.185 [5.994, 6.588] | 0.990 | 1.130 | 1.403 |
| variant-02/06-unreachable-edge | 6.727 [6.040, 7.910] | 6.678 [6.159, 7.319] | 0.993 | 1.513 | 1.435 |
| variant-03/01-basic | 5.508 [4.992, 5.956] | 5.261 [4.791, 5.442] | 0.955 | 1.227 | 1.184 |
| variant-03/02-single-salesman | 5.523 [5.212, 5.536] | 5.648 [5.192, 5.998] | 1.023 | 1.240 | 1.270 |
| variant-03/03-depot-crossing-unsat | 5.304 [4.774, 6.139] | 5.722 [5.577, 5.796] | 1.079 | 0.969 | 1.287 |
| variant-03/04-equal-cost-split | 6.116 [6.067, 6.523] | 6.585 [5.965, 6.657] | 1.077 | 1.121 | 1.465 |
| variant-03/05-larger-three-depots | 7.605 [6.693, 8.706] | 7.681 [6.600, 7.719] | 1.010 | 1.380 | 1.393 |
| variant-03/06-unreachable-edge | 6.191 [5.997, 6.928] | 7.198 [6.194, 7.676] | 1.163 | 1.395 | 1.313 |
| variant-04/01-basic | 6.760 [6.229, 7.927] | 7.191 [5.748, 7.271] | 1.064 | 1.232 | 1.322 |
| variant-04/02-single-salesman | 6.610 [5.915, 6.616] | 7.156 [6.593, 7.643] | 1.083 | 1.461 | 1.612 |
| variant-04/03-window-too-tight-unsat | 6.499 [5.932, 7.846] | 6.612 [5.547, 6.773] | 1.017 | 1.176 | 1.487 |
| variant-04/04-depot-window-too-tight-unsat | 5.826 [5.797, 5.858] | 6.618 [6.264, 7.695] | 1.136 | 1.061 | 1.211 |
| variant-04/05-three-depots-asymmetric-times | 9.417 [8.724, 12.713] | 8.910 [8.078, 10.005] | 0.946 | 1.722 | 1.617 |
| variant-04/06-unreachable-edge | 8.730 [7.614, 9.422] | 8.050 [7.722, 8.347] | 0.922 | 1.952 | 1.813 |
| variant-05/01-basic | 6.851 [6.793, 6.891] | 7.224 [6.697, 7.813] | 1.054 | 1.254 | 1.622 |
| variant-05/02-tight-bound-exact | 5.843 [5.763, 6.958] | 5.906 [5.544, 6.204] | 1.011 | 1.068 | 1.335 |
| variant-05/03-revisit-too-tight-unsat | 5.465 [5.438, 5.474] | 5.778 [5.548, 6.452] | 1.057 | 0.995 | 1.058 |
| variant-05/04-depot-and-vertex-revisits | 6.010 [5.475, 6.237] | 6.897 [6.702, 7.190] | 1.147 | 1.344 | 1.261 |
| variant-05/05-three-depots-mixed | 10.830 [8.928, 11.082] | 9.153 [8.377, 10.093] | 0.845 | 2.447 | 1.661 |
| variant-05/06-unreachable-edge | 8.805 [8.418, 9.324] | 8.702 [7.989, 9.761] | 0.988 | 1.607 | 1.586 |
| n-queens/variant-01 | 21.658 [21.656, 22.669] | 12.177 [10.842, 13.086] | 0.562 | 3.259 | 2.134 |
| n-queens/variant-02 | 101.795 [100.488, 103.217] | 90.499 [90.488, 91.024] | 0.889 | 0.838 | 0.760 |
| n-queens/variant-03 | 23.744 [22.762, 23.776] | 11.529 [11.105, 13.428] | 0.486 | 3.553 | 2.008 |
| n-queens/variant-04 | 11.317 [9.994, 12.562] | 11.129 [10.043, 11.134] | 0.983 | 1.707 | 1.680 |
| n-queens/variant-05 | 46.637 [43.122, 46.706] | 41.712 [41.369, 42.537] | 0.894 | 6.699 | 6.381 |
| n-queens/variant-06 | 46.203 [43.797, 46.282] | 44.768 [43.579, 44.884] | 0.969 | 6.766 | 7.742 |
| send-money/send-money | 52.566 [49.470, 53.433] | 11.345 [11.049, 11.980] | 0.216 | 3.665 | 0.747 |

Reference wall time, ms, same notation.

| Cell | main | after |
|---|---:|---:|
| equality-generalized-tsp/01-basic | 5.525 [4.621, 6.995] | 4.510 [4.451, 5.599] |
| equality-generalized-tsp/02-larger | 5.488 [4.510, 5.622] | 4.576 [4.417, 5.472] |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.448 [4.387, 4.452] | 4.455 [4.431, 4.591] |
| variant-01/01-basic | 4.517 [4.361, 5.424] | 4.421 [4.371, 4.429] |
| variant-01/02-start-equals-end | 4.512 [4.455, 5.568] | 4.444 [4.430, 4.489] |
| variant-01/03-zero-cost-detour | 4.494 [3.342, 5.514] | 4.439 [4.405, 4.544] |
| variant-01/04-no-path | 4.477 [4.425, 5.489] | 4.413 [4.410, 4.559] |
| variant-01/05-multi-path | 5.476 [4.442, 5.476] | 4.383 [4.382, 5.511] |
| variant-01/06-layered-dag | 5.506 [4.401, 7.717] | 6.556 [5.510, 6.563] |
| variant-01/07-cycles | 5.512 [4.396, 5.526] | 4.439 [4.412, 5.536] |
| variant-01/08-negative-weights | 5.666 [4.445, 6.664] | 4.431 [4.374, 4.442] |
| variant-02/01-basic | 4.424 [4.415, 4.461] | 5.461 [4.431, 5.482] |
| variant-02/02-start-equals-end | 4.437 [4.436, 5.502] | 5.467 [4.472, 5.506] |
| variant-02/03-before-forces-detour | 4.388 [4.352, 4.448] | 4.458 [4.367, 4.489] |
| variant-02/04-after-forces-extension | 4.416 [4.405, 4.429] | 4.446 [4.437, 5.463] |
| variant-02/05-ordering-unsat | 4.450 [4.411, 5.519] | 4.452 [4.451, 4.544] |
| variant-02/06-layered-dag-before | 6.534 [5.507, 6.552] | 5.523 [5.494, 5.524] |
| variant-02/07-layered-dag-before-after | 5.599 [5.462, 6.579] | 5.524 [5.461, 6.481] |
| variant-02/08-tie-break-under-ordering | 4.445 [4.380, 4.454] | 4.425 [4.408, 5.511] |
| variant-02/09-negative-weights | 5.516 [4.418, 7.707] | 4.406 [4.342, 6.537] |
| variant-03/01-basic | 4.492 [4.484, 5.471] | 4.438 [4.352, 4.442] |
| variant-03/02-start-equals-end | 5.521 [4.448, 5.531] | 4.441 [4.419, 4.453] |
| variant-03/03-budget-forces-detour | 4.377 [3.335, 4.486] | 5.502 [4.376, 5.526] |
| variant-03/04-cost-at-cap-allowed | 4.436 [4.418, 5.483] | 4.493 [4.411, 5.454] |
| variant-03/05-budget-unsat | 5.468 [4.403, 5.502] | 4.448 [4.436, 5.494] |
| variant-03/06-layered-dag-cap | 7.608 [6.615, 7.637] | 6.574 [6.521, 6.598] |
| variant-03/07-layered-dag-tight-cap | 6.583 [6.580, 7.612] | 6.578 [6.573, 7.628] |
| variant-03/08-tie-break-under-cap | 4.450 [4.349, 5.521] | 5.412 [4.459, 5.512] |
| variant-03/09-negative-weights | 4.427 [4.391, 4.449] | 4.454 [4.406, 5.500] |
| variant-04/01-basic | 4.473 [4.443, 6.591] | 5.450 [4.444, 6.526] |
| variant-04/02-start-equals-end | 5.668 [4.591, 6.505] | 4.460 [4.430, 5.455] |
| variant-04/03-before-forces-detour-within-budget | 5.496 [4.377, 5.526] | 4.523 [4.413, 6.536] |
| variant-04/04-ordering-violates-budget | 5.492 [5.456, 5.498] | 4.423 [4.401, 4.442] |
| variant-04/05-after-and-budget-interact | 5.465 [5.431, 5.519] | 5.453 [4.498, 6.560] |
| variant-04/06-layered-dag-ordering-cap | 6.563 [5.421, 7.622] | 6.556 [6.537, 7.614] |
| variant-04/07-layered-dag-combined | 6.584 [5.423, 9.820] | 6.532 [6.514, 6.672] |
| variant-04/08-tie-break-under-ordering-and-cap | 5.462 [4.453, 5.523] | 5.490 [4.388, 5.491] |
| variant-04/09-negative-weights | 5.520 [4.387, 5.557] | 4.448 [4.445, 5.485] |
| variant-01/01-basic | 4.454 [4.398, 4.575] | 4.448 [4.443, 4.456] |
| variant-01/02-agent-reuse | 4.464 [3.645, 5.690] | 4.443 [4.351, 4.531] |
| variant-01/03-selective-compatibility | 4.425 [4.402, 5.707] | 4.487 [4.475, 4.583] |
| variant-01/04-no-compatible-agent-unsat | 4.437 [4.408, 5.558] | 4.453 [4.449, 5.460] |
| variant-01/05-larger-mix | 4.410 [4.371, 5.637] | 4.450 [4.428, 4.533] |
| variant-02/01-basic | 5.513 [4.386, 6.711] | 4.403 [3.363, 5.468] |
| variant-02/02-makespan-tiebreak | 5.541 [4.429, 6.730] | 4.437 [4.425, 5.477] |
| variant-02/03-cost-dominates | 5.573 [4.430, 5.798] | 5.442 [4.459, 5.458] |
| variant-02/04-no-compatible-agent-unsat | 4.683 [4.403, 5.672] | 4.439 [4.408, 4.450] |
| variant-02/05-larger-mix | 5.583 [5.500, 5.629] | 5.493 [5.461, 5.505] |
| variant-03/01-basic | 5.683 [3.328, 7.719] | 4.422 [3.321, 4.429] |
| variant-03/02-multiple-groups | 4.434 [4.405, 5.661] | 4.441 [3.380, 5.529] |
| variant-03/03-mixed-grouped-ungrouped | 4.444 [4.393, 6.711] | 4.412 [3.382, 5.517] |
| variant-03/04-incompatible-group-unsat | 4.469 [4.453, 5.616] | 4.427 [4.358, 5.441] |
| variant-03/05-larger-mix | 5.472 [4.419, 6.705] | 4.428 [4.367, 5.504] |
| variant-04/01-basic | 29.463 [26.681, 31.352] | 26.721 [25.697, 29.315] |
| variant-04/02-precedence | 6.667 [6.516, 8.033] | 7.656 [6.939, 7.875] |
| variant-04/03-agent-serialization | 30.013 [28.859, 31.092] | 27.762 [26.699, 28.781] |
| variant-04/04-window-too-tight-unsat | 5.567 [4.498, 6.650] | 5.500 [4.474, 5.534] |
| variant-04/05-larger-mix | 155.708 [152.717, 156.214] | 147.648 [145.832, 150.729] |
| variant-01/01-basic | 5.702 [5.482, 5.736] | 4.567 [4.547, 4.695] |
| variant-01/02-multiple-tours | 4.493 [4.469, 5.462] | 5.512 [5.509, 5.541] |
| variant-01/03-asymmetric | 6.647 [5.517, 7.714] | 4.546 [4.449, 6.576] |
| variant-01/04-subtour-unsat | 4.409 [3.328, 5.571] | 5.553 [4.435, 5.586] |
| variant-01/05-ring | 5.464 [4.403, 5.521] | 5.474 [5.436, 5.517] |
| variant-02/01-basic | 5.468 [3.352, 5.477] | 4.410 [4.395, 5.521] |
| variant-02/02-single-salesman | 4.360 [3.345, 5.498] | 4.507 [4.456, 5.472] |
| variant-02/03-too-many-salesmen-unsat | 4.455 [4.359, 5.500] | 4.450 [4.434, 5.531] |
| variant-02/04-equal-cost-split | 4.484 [4.449, 5.463] | 4.450 [4.439, 5.496] |
| variant-02/05-larger-asymmetric | 5.533 [5.496, 6.565] | 4.409 [4.373, 5.452] |
| variant-02/06-unreachable-edge | 4.447 [4.445, 4.474] | 4.653 [4.447, 5.514] |
| variant-03/01-basic | 4.487 [4.462, 5.403] | 4.443 [3.523, 5.519] |
| variant-03/02-single-salesman | 4.455 [4.442, 5.497] | 4.447 [4.441, 5.549] |
| variant-03/03-depot-crossing-unsat | 5.475 [4.386, 5.691] | 4.444 [4.353, 4.461] |
| variant-03/04-equal-cost-split | 5.456 [4.433, 5.486] | 4.493 [4.445, 5.575] |
| variant-03/05-larger-three-depots | 5.511 [4.438, 5.527] | 5.515 [5.496, 5.525] |
| variant-03/06-unreachable-edge | 4.438 [4.360, 5.470] | 5.480 [4.459, 5.493] |
| variant-04/01-basic | 5.488 [5.426, 7.615] | 5.441 [4.437, 5.482] |
| variant-04/02-single-salesman | 4.523 [4.420, 5.410] | 4.439 [4.418, 5.463] |
| variant-04/03-window-too-tight-unsat | 5.525 [4.423, 5.526] | 4.446 [4.440, 4.449] |
| variant-04/04-depot-window-too-tight-unsat | 5.490 [4.397, 6.740] | 5.464 [4.434, 5.487] |
| variant-04/05-three-depots-asymmetric-times | 5.469 [4.456, 5.492] | 5.509 [5.467, 5.514] |
| variant-04/06-unreachable-edge | 4.472 [4.436, 5.524] | 4.441 [4.416, 5.521] |
| variant-05/01-basic | 5.465 [4.588, 5.532] | 4.455 [4.421, 5.602] |
| variant-05/02-tight-bound-exact | 5.471 [4.444, 5.523] | 4.424 [4.403, 4.455] |
| variant-05/03-revisit-too-tight-unsat | 5.494 [4.418, 6.641] | 5.459 [4.604, 5.469] |
| variant-05/04-depot-and-vertex-revisits | 4.473 [4.438, 5.440] | 5.471 [4.436, 5.503] |
| variant-05/05-three-depots-mixed | 4.427 [4.355, 4.464] | 5.511 [5.468, 7.563] |
| variant-05/06-unreachable-edge | 5.479 [4.513, 5.736] | 5.486 [4.453, 5.507] |
| n-queens/variant-01 | 6.645 [5.491, 6.674] | 5.705 [5.488, 6.640] |
| n-queens/variant-02 | 121.444 [120.633, 125.629] | 119.120 [119.093, 119.696] |
| n-queens/variant-03 | 6.682 [6.650, 7.049] | 5.742 [5.528, 6.638] |
| n-queens/variant-04 | 6.631 [5.688, 6.647] | 6.624 [5.755, 7.866] |
| n-queens/variant-05 | 6.962 [6.572, 8.028] | 6.537 [5.911, 6.905] |
| n-queens/variant-06 | 6.829 [5.756, 6.870] | 5.783 [5.651, 7.063] |
| send-money/send-money | 14.342 [14.028, 15.494] | 15.183 [14.099, 15.368] |

Counters of report after: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 0 | 1 | 4 | 75404 | 2.428 |
| equality-generalized-tsp/02-larger | 0 | 1 | 8 | 130751 | 3.117 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 0 | 0 | 0 | 4096 | 1.291 |
| variant-01/01-basic | 0 | 1 | 2 | 5540 | 1.274 |
| variant-01/02-start-equals-end | 0 | 1 | 1 | 8051 | 1.391 |
| variant-01/03-zero-cost-detour | 0 | 1 | 2 | 4274 | 1.762 |
| variant-01/04-no-path | 0 | 0 | 0 | 1865 | 1.345 |
| variant-01/05-multi-path | 0 | 1 | 3 | 18700 | 1.602 |
| variant-01/06-layered-dag | 0 | 1 | 27 | 344578 | 4.409 |
| variant-01/07-cycles | 0 | 1 | 5 | 114322 | 2.638 |
| variant-01/08-negative-weights | 0 | 1 | 1 | 28915 | 2.298 |
| variant-02/01-basic | 0 | 1 | 2 | 5904 | 1.647 |
| variant-02/02-start-equals-end | 0 | 1 | 1 | 10761 | 1.687 |
| variant-02/03-before-forces-detour | 0 | 1 | 1 | 7017 | 1.535 |
| variant-02/04-after-forces-extension | 0 | 1 | 1 | 6405 | 1.619 |
| variant-02/05-ordering-unsat | 0 | 0 | 0 | 2519 | 1.718 |
| variant-02/06-layered-dag-before | 0 | 2 | 9 | 239298 | 4.267 |
| variant-02/07-layered-dag-before-after | 0 | 1 | 7 | 209967 | 3.858 |
| variant-02/08-tie-break-under-ordering | 0 | 1 | 2 | 10614 | 1.807 |
| variant-02/09-negative-weights | 0 | 1 | 1 | 32506 | 2.120 |
| variant-03/01-basic | 0 | 1 | 2 | 5734 | 1.607 |
| variant-03/02-start-equals-end | 0 | 1 | 1 | 9929 | 1.924 |
| variant-03/03-budget-forces-detour | 0 | 1 | 1 | 6301 | 2.180 |
| variant-03/04-cost-at-cap-allowed | 0 | 1 | 1 | 3269 | 1.470 |
| variant-03/05-budget-unsat | 0 | 0 | 0 | 16082 | 1.846 |
| variant-03/06-layered-dag-cap | 0 | 2 | 25 | 611524 | 7.237 |
| variant-03/07-layered-dag-tight-cap | 0 | 4 | 6 | 363131 | 5.323 |
| variant-03/08-tie-break-under-cap | 0 | 1 | 2 | 12810 | 2.107 |
| variant-03/09-negative-weights | 0 | 1 | 1 | 32853 | 2.461 |
| variant-04/01-basic | 0 | 1 | 2 | 6098 | 1.701 |
| variant-04/02-start-equals-end | 0 | 1 | 1 | 12639 | 1.817 |
| variant-04/03-before-forces-detour-within-budget | 0 | 1 | 1 | 8170 | 2.098 |
| variant-04/04-ordering-violates-budget | 0 | 0 | 0 | 6098 | 1.848 |
| variant-04/05-after-and-budget-interact | 0 | 1 | 1 | 13637 | 1.844 |
| variant-04/06-layered-dag-ordering-cap | 0 | 2 | 8 | 363501 | 6.428 |
| variant-04/07-layered-dag-combined | 0 | 1 | 5 | 313657 | 5.951 |
| variant-04/08-tie-break-under-ordering-and-cap | 0 | 1 | 2 | 27409 | 2.929 |
| variant-04/09-negative-weights | 0 | 1 | 1 | 36453 | 2.386 |
| variant-01/01-basic | 0 | 1 | 4 | 5980 | 1.522 |
| variant-01/02-agent-reuse | 0 | 1 | 8 | 15903 | 1.239 |
| variant-01/03-selective-compatibility | 0 | 1 | 4 | 7209 | 1.454 |
| variant-01/04-no-compatible-agent-unsat | 0 | 0 | 0 | 2289 | 0.906 |
| variant-01/05-larger-mix | 0 | 1 | 64 | 182539 | 2.532 |
| variant-02/01-basic | 0 | 1 | 4 | 14404 | 1.475 |
| variant-02/02-makespan-tiebreak | 0 | 3 | 8 | 25447 | 1.681 |
| variant-02/03-cost-dominates | 0 | 1 | 8 | 29126 | 2.282 |
| variant-02/04-no-compatible-agent-unsat | 0 | 0 | 0 | 4725 | 1.875 |
| variant-02/05-larger-mix | 0 | 1 | 67 | 844697 | 6.754 |
| variant-03/01-basic | 0 | 2 | 2 | 7431 | 1.573 |
| variant-03/02-multiple-groups | 0 | 4 | 4 | 17609 | 2.132 |
| variant-03/03-mixed-grouped-ungrouped | 0 | 2 | 4 | 11285 | 2.423 |
| variant-03/04-incompatible-group-unsat | 0 | 0 | 0 | 2497 | 1.721 |
| variant-03/05-larger-mix | 0 | 1 | 27 | 51564 | 2.039 |
| variant-04/01-basic | 0 | 81 | 124 | 875111 | 16.330 |
| variant-04/02-precedence | 0 | 3 | 12 | 75546 | 3.586 |
| variant-04/03-agent-serialization | 0 | 30 | 30 | 253900 | 7.219 |
| variant-04/04-window-too-tight-unsat | 0 | 0 | 0 | 8883 | 2.699 |
| variant-04/05-larger-mix | 0 | blocked by capture_limit ×3 | | | |
| variant-01/01-basic | 0 | 1 | 2 | 45803 | 2.153 |
| variant-01/02-multiple-tours | 0 | 2 | 9 | 217292 | 3.358 |
| variant-01/03-asymmetric | 0 | 1 | 9 | 326714 | 4.645 |
| variant-01/04-subtour-unsat | 0 | 0 | 0 | 4623 | 1.639 |
| variant-01/05-ring | 0 | 2 | 44 | 1422699 | 8.338 |
| variant-02/01-basic | 0 | 2 | 2 | 50389 | 2.358 |
| variant-02/02-single-salesman | 0 | 1 | 2 | 32172 | 1.999 |
| variant-02/03-too-many-salesmen-unsat | 0 | 0 | 0 | 6906 | 1.429 |
| variant-02/04-equal-cost-split | 0 | 2 | 2 | 40903 | 2.047 |
| variant-02/05-larger-asymmetric | 0 | 2 | 2 | 60140 | 2.329 |
| variant-02/06-unreachable-edge | 0 | 2 | 2 | 69881 | 3.050 |
| variant-03/01-basic | 0 | 1 | 1 | 26134 | 1.801 |
| variant-03/02-single-salesman | 0 | 1 | 2 | 30993 | 1.924 |
| variant-03/03-depot-crossing-unsat | 0 | 0 | 0 | 6761 | 1.526 |
| variant-03/04-equal-cost-split | 0 | 2 | 2 | 70632 | 2.478 |
| variant-03/05-larger-three-depots | 0 | 1 | 1 | 73646 | 3.121 |
| variant-03/06-unreachable-edge | 0 | 1 | 1 | 44796 | 3.308 |
| variant-04/01-basic | 0 | 1 | 1 | 46886 | 2.983 |
| variant-04/02-single-salesman | 0 | 1 | 1 | 31547 | 3.095 |
| variant-04/03-window-too-tight-unsat | 0 | 0 | 0 | 6641 | 2.522 |
| variant-04/04-depot-window-too-tight-unsat | 0 | 0 | 0 | 4430 | 2.414 |
| variant-04/05-three-depots-asymmetric-times | 0 | 1 | 1 | 125685 | 5.509 |
| variant-04/06-unreachable-edge | 0 | 1 | 1 | 98235 | 4.362 |
| variant-05/01-basic | 0 | 1 | 1 | 46894 | 3.563 |
| variant-05/02-tight-bound-exact | 0 | 1 | 1 | 23047 | 2.439 |
| variant-05/03-revisit-too-tight-unsat | 0 | 0 | 0 | 6826 | 1.979 |
| variant-05/04-depot-and-vertex-revisits | 0 | 1 | 1 | 23457 | 2.608 |
| variant-05/05-three-depots-mixed | 0 | 1 | 1 | 126050 | 5.458 |
| variant-05/06-unreachable-edge | 0 | 1 | 1 | 98235 | 4.647 |
| n-queens/variant-01 | 0 | 92 | 92 | 734652 | 8.065 |
| n-queens/variant-02 | 0 | 92 | 92 | 18761080 | 86.945 |
| n-queens/variant-03 | 0 | 92 | 92 | 675790 | 7.570 |
| n-queens/variant-04 | 0 | 92 | 92 | 574291 | 7.151 |
| n-queens/variant-05 | 0 | 92 | 92 | 872980 | 37.540 |
| n-queens/variant-06 | 0 | 92 | 92 | 1004609 | 40.229 |
| send-money/send-money | 0 | 1 | 1 | 349198 | 7.076 |
