Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | before | after | after/before | before/reference | after/reference |
|---|---:|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 5.524 [5.489, 5.753] | 5.701 [5.666, 5.885] | 1.032 | 1.003 | 1.041 |
| equality-generalized-tsp/02-larger | 6.823 [5.915, 7.852] | 6.575 [6.012, 6.595] | 0.964 | 1.247 | 1.482 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.787 [5.091, 6.331] | 5.063 [4.631, 5.701] | 0.875 | 1.300 | 1.134 |
| variant-01/01-basic | 5.093 [4.759, 6.236] | 5.502 [5.170, 5.673] | 1.080 | 0.933 | 1.240 |
| variant-01/02-start-equals-end | 5.515 [4.795, 5.795] | 4.794 [4.761, 5.521] | 0.869 | 1.009 | 1.087 |
| variant-01/03-zero-cost-detour | 5.538 [4.925, 5.794] | 5.612 [5.522, 5.661] | 1.013 | 0.969 | 1.267 |
| variant-01/04-no-path | 4.791 [4.599, 5.106] | 4.868 [4.720, 6.879] | 1.016 | 1.075 | 1.101 |
| variant-01/05-multi-path | 5.588 [5.483, 6.256] | 5.519 [4.856, 5.550] | 0.988 | 1.253 | 1.254 |
| variant-01/06-layered-dag | 8.777 [7.872, 9.441] | 8.928 [8.080, 9.952] | 1.017 | 1.552 | 1.619 |
| variant-01/07-cycles | 6.845 [5.889, 7.437] | 6.751 [5.489, 6.790] | 0.986 | 1.341 | 1.517 |
| variant-01/08-negative-weights | 6.560 [5.522, 6.952] | 5.556 [5.012, 5.658] | 0.847 | 1.474 | 1.238 |
| variant-02/01-basic | 5.127 [4.735, 5.912] | 5.526 [4.515, 5.553] | 1.078 | 0.929 | 1.244 |
| variant-02/02-start-equals-end | 5.263 [4.830, 5.598] | 5.208 [4.925, 5.532] | 0.990 | 1.178 | 1.173 |
| variant-02/03-before-forces-detour | 6.600 [5.234, 6.777] | 5.223 [4.836, 5.537] | 0.791 | 1.488 | 1.174 |
| variant-02/04-after-forces-extension | 5.556 [5.517, 5.610] | 5.521 [4.670, 5.887] | 0.994 | 1.012 | 1.255 |
| variant-02/05-ordering-unsat | 5.077 [4.754, 5.921] | 5.532 [4.753, 5.729] | 1.090 | 1.153 | 1.251 |
| variant-02/06-layered-dag-before | 8.085 [7.971, 8.242] | 8.092 [7.266, 9.902] | 1.001 | 1.482 | 1.449 |
| variant-02/07-layered-dag-before-after | 8.878 [7.962, 10.520] | 9.842 [7.641, 11.358] | 1.109 | 1.607 | 1.696 |
| variant-02/08-tie-break-under-ordering | 5.595 [5.580, 5.677] | 6.044 [5.345, 6.106] | 1.080 | 1.016 | 0.922 |
| variant-02/09-negative-weights | 6.805 [6.756, 6.976] | 6.092 [5.230, 6.094] | 0.895 | 1.248 | 1.378 |
| variant-03/01-basic | 5.522 [5.224, 6.097] | 4.724 [4.621, 5.069] | 0.856 | 1.247 | 1.065 |
| variant-03/02-start-equals-end | 5.504 [4.712, 5.516] | 5.544 [4.387, 5.670] | 1.007 | 1.249 | 1.243 |
| variant-03/03-budget-forces-detour | 6.073 [5.328, 6.678] | 5.524 [5.152, 5.824] | 0.910 | 1.380 | 1.247 |
| variant-03/04-cost-at-cap-allowed | 5.455 [5.450, 5.701] | 5.207 [4.479, 5.534] | 0.955 | 0.993 | 0.963 |
| variant-03/05-budget-unsat | 6.577 [4.867, 6.580] | 5.526 [4.687, 5.554] | 0.840 | 1.205 | 1.258 |
| variant-03/06-layered-dag-cap | 11.883 [10.879, 14.337] | 13.056 [9.843, 14.145] | 1.099 | 1.563 | 1.979 |
| variant-03/07-layered-dag-tight-cap | 12.971 [11.708, 13.253] | 15.317 [10.379, 15.576] | 1.181 | 1.978 | 2.314 |
| variant-03/08-tie-break-under-cap | 6.099 [5.039, 8.036] | 6.269 [5.492, 6.599] | 1.028 | 1.130 | 1.157 |
| variant-03/09-negative-weights | 5.820 [5.646, 5.844] | 5.314 [4.965, 7.763] | 0.913 | 1.060 | 0.980 |
| variant-04/01-basic | 5.479 [5.270, 5.526] | 5.565 [5.022, 5.845] | 1.016 | 1.236 | 1.005 |
| variant-04/02-start-equals-end | 5.568 [5.248, 6.298] | 5.881 [4.602, 5.889] | 1.056 | 1.263 | 1.326 |
| variant-04/03-before-forces-detour-within-budget | 5.623 [5.519, 6.976] | 5.805 [5.078, 5.887] | 1.032 | 1.263 | 1.316 |
| variant-04/04-ordering-violates-budget | 6.093 [5.954, 6.294] | 5.891 [4.794, 6.771] | 0.967 | 1.112 | 1.268 |
| variant-04/05-after-and-budget-interact | 5.691 [4.492, 6.147] | 5.847 [5.661, 6.256] | 1.027 | 1.037 | 1.070 |
| variant-04/06-layered-dag-ordering-cap | 11.646 [10.086, 11.700] | 10.509 [9.407, 10.782] | 0.902 | 1.330 | 1.606 |
| variant-04/07-layered-dag-combined | 9.751 [8.991, 11.069] | 10.481 [9.993, 11.043] | 1.075 | 1.284 | 1.373 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.968 [6.819, 7.653] | 6.649 [5.857, 7.764] | 0.954 | 1.274 | 1.204 |
| variant-04/09-negative-weights | 6.409 [6.057, 6.816] | 6.550 [5.511, 7.097] | 1.022 | 1.171 | 1.477 |
| variant-01/01-basic | 4.616 [4.511, 4.790] | 4.677 [4.638, 4.834] | 1.013 | 1.047 | 1.057 |
| variant-01/02-agent-reuse | 5.497 [5.481, 5.839] | 4.536 [4.424, 5.157] | 0.825 | 1.242 | 1.023 |
| variant-01/03-selective-compatibility | 5.781 [4.869, 6.579] | 4.566 [4.433, 5.165] | 0.790 | 1.303 | 1.026 |
| variant-01/04-no-compatible-agent-unsat | 4.786 [4.528, 5.538] | 4.460 [4.410, 4.729] | 0.932 | 1.073 | 1.007 |
| variant-01/05-larger-mix | 5.951 [5.926, 6.109] | 5.484 [5.215, 5.534] | 0.921 | 1.341 | 1.237 |
| variant-02/01-basic | 5.988 [5.431, 7.070] | 5.539 [5.531, 5.577] | 0.925 | 1.346 | 1.015 |
| variant-02/02-makespan-tiebreak | 5.726 [4.663, 5.774] | 5.720 [5.125, 7.002] | 0.999 | 1.291 | 1.042 |
| variant-02/03-cost-dominates | 6.455 [5.805, 7.697] | 5.212 [4.852, 5.624] | 0.807 | 1.188 | 1.180 |
| variant-02/04-no-compatible-agent-unsat | 6.298 [5.059, 6.353] | 5.526 [5.220, 5.635] | 0.877 | 1.423 | 1.249 |
| variant-02/05-larger-mix | 9.348 [8.920, 11.063] | 9.804 [7.678, 10.222] | 1.049 | 1.708 | 1.767 |
| variant-03/01-basic | 5.526 [4.611, 5.646] | 4.900 [4.611, 5.995] | 0.887 | 1.255 | 1.103 |
| variant-03/02-multiple-groups | 5.189 [4.576, 5.520] | 4.877 [4.413, 5.241] | 0.940 | 1.190 | 1.107 |
| variant-03/03-mixed-grouped-ungrouped | 5.491 [4.516, 5.887] | 4.901 [4.082, 5.171] | 0.893 | 1.251 | 1.111 |
| variant-03/04-incompatible-group-unsat | 4.698 [4.410, 7.088] | 4.620 [3.865, 4.652] | 0.983 | 1.060 | 1.062 |
| variant-03/05-larger-mix | 6.611 [5.984, 6.615] | 5.139 [4.484, 5.604] | 0.777 | 1.490 | 1.163 |
| variant-04/01-basic | 9.240 [9.033, 11.072] | 9.079 [8.204, 10.780] | 0.983 | 0.330 | 0.336 |
| variant-04/02-precedence | 8.354 [7.710, 8.445] | 6.725 [6.626, 8.759] | 0.805 | 1.509 | 1.020 |
| variant-04/03-agent-serialization | 8.258 [8.202, 9.855] | 8.241 [7.912, 8.440] | 0.998 | 0.297 | 0.294 |
| variant-04/04-window-too-tight-unsat | 6.027 [5.724, 8.888] | 5.983 [5.714, 6.793] | 0.993 | 1.084 | 1.330 |
| variant-04/05-larger-mix | 33.864 [32.890, 34.791] | 34.283 [32.850, 36.025] | 1.012 | 0.229 | 0.231 |
| variant-01/01-basic | 6.039 [5.684, 6.142] | 5.739 [5.455, 5.884] | 0.950 | 1.352 | 1.018 |
| variant-01/02-multiple-tours | 6.186 [5.706, 6.893] | 5.887 [5.562, 6.673] | 0.952 | 1.375 | 1.077 |
| variant-01/03-asymmetric | 6.744 [6.596, 7.199] | 7.738 [7.722, 8.299] | 1.147 | 1.235 | 1.734 |
| variant-01/04-subtour-unsat | 4.916 [4.615, 4.920] | 4.845 [4.801, 4.897] | 0.985 | 1.476 | 0.729 |
| variant-01/05-ring | 9.451 [8.408, 9.876] | 9.425 [7.996, 10.266] | 0.997 | 2.113 | 1.716 |
| variant-02/01-basic | 6.176 [5.869, 6.714] | 5.936 [5.542, 6.775] | 0.961 | 1.390 | 1.088 |
| variant-02/02-single-salesman | 6.068 [5.480, 6.347] | 6.040 [5.979, 6.277] | 0.995 | 1.103 | 1.374 |
| variant-02/03-too-many-salesmen-unsat | 5.169 [5.142, 6.078] | 5.549 [4.930, 7.413] | 1.074 | 1.165 | 1.016 |
| variant-02/04-equal-cost-split | 5.932 [5.541, 6.816] | 5.764 [4.558, 5.765] | 0.972 | 1.332 | 1.293 |
| variant-02/05-larger-asymmetric | 5.719 [4.864, 6.553] | 5.650 [5.520, 6.889] | 0.988 | 1.285 | 1.040 |
| variant-02/06-unreachable-edge | 5.655 [5.508, 6.927] | 6.320 [5.586, 7.648] | 1.118 | 1.026 | 1.424 |
| variant-03/01-basic | 5.673 [5.613, 6.763] | 5.531 [5.525, 6.172] | 0.975 | 1.283 | 1.249 |
| variant-03/02-single-salesman | 6.129 [5.642, 6.129] | 6.134 [5.743, 6.558] | 1.001 | 1.378 | 1.373 |
| variant-03/03-depot-crossing-unsat | 5.538 [4.903, 5.682] | 5.523 [4.963, 6.585] | 0.997 | 1.247 | 1.254 |
| variant-03/04-equal-cost-split | 5.620 [5.488, 7.741] | 5.976 [5.673, 6.856] | 1.063 | 1.260 | 1.084 |
| variant-03/05-larger-three-depots | 7.131 [5.755, 7.639] | 6.635 [6.632, 8.510] | 0.930 | 1.556 | 1.211 |
| variant-03/06-unreachable-edge | 6.503 [5.487, 6.855] | 6.190 [5.290, 6.681] | 0.952 | 1.471 | 1.382 |
| variant-04/01-basic | 6.633 [6.594, 6.657] | 6.531 [6.407, 7.228] | 0.985 | 1.497 | 1.459 |
| variant-04/02-single-salesman | 6.101 [6.028, 7.583] | 6.090 [5.513, 6.923] | 0.998 | 1.102 | 1.105 |
| variant-04/03-window-too-tight-unsat | 6.125 [5.902, 6.378] | 5.858 [5.564, 6.598] | 0.956 | 1.382 | 1.073 |
| variant-04/04-depot-window-too-tight-unsat | 6.674 [4.557, 6.713] | 5.796 [4.775, 6.070] | 0.868 | 1.508 | 1.310 |
| variant-04/05-three-depots-asymmetric-times | 8.155 [7.714, 8.815] | 9.194 [8.189, 9.653] | 1.127 | 1.486 | 1.683 |
| variant-04/06-unreachable-edge | 7.653 [7.327, 7.853] | 8.359 [7.931, 9.532] | 1.092 | 1.725 | 1.579 |
| variant-05/01-basic | 6.562 [6.545, 8.821] | 6.864 [6.556, 6.949] | 1.046 | 1.476 | 1.261 |
| variant-05/02-tight-bound-exact | 6.164 [6.147, 7.975] | 5.641 [5.479, 6.777] | 0.915 | 1.391 | 1.263 |
| variant-05/03-revisit-too-tight-unsat | 5.929 [5.844, 6.596] | 6.570 [5.674, 6.932] | 1.108 | 1.333 | 1.443 |
| variant-05/04-depot-and-vertex-revisits | 6.082 [5.865, 7.352] | 6.603 [6.580, 6.757] | 1.086 | 1.112 | 1.432 |
| variant-05/05-three-depots-mixed | 8.814 [8.700, 9.158] | 8.105 [7.678, 8.589] | 0.920 | 1.986 | 1.483 |
| variant-05/06-unreachable-edge | 8.838 [7.618, 9.238] | 7.905 [7.338, 8.311] | 0.894 | 1.990 | 1.448 |
| n-queens/variant-01 | 8.941 [8.704, 9.511] | 9.803 [7.592, 10.099] | 1.096 | 1.632 | 1.491 |
| n-queens/variant-02 | 62.990 [62.284, 63.630] | 71.078 [67.866, 71.351] | 1.128 | 0.528 | 0.594 |
| n-queens/variant-03 | 8.932 [8.816, 9.900] | 9.284 [8.806, 9.870] | 1.039 | 1.360 | 1.398 |
| n-queens/variant-04 | 8.252 [8.243, 8.637] | 7.956 [7.826, 8.158] | 0.964 | 1.253 | 1.209 |
| n-queens/variant-05 | 8.302 [7.733, 8.886] | 8.370 [8.096, 9.182] | 1.008 | 1.266 | 1.273 |
| n-queens/variant-06 | 8.475 [8.306, 9.355] | 9.837 [9.303, 10.148] | 1.161 | 1.287 | 1.776 |
| send-money/send-money | 14.351 [13.358, 14.575] | 13.518 [13.138, 13.674] | 0.942 | 0.957 | 0.899 |

Reference wall time, ms, same notation.

| Cell | before | after |
|---|---:|---:|
| equality-generalized-tsp/01-basic | 5.509 [4.491, 5.595] | 5.479 [4.453, 5.540] |
| equality-generalized-tsp/02-larger | 5.469 [4.445, 7.657] | 4.438 [4.408, 5.540] |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.452 [4.450, 5.459] | 4.463 [4.418, 5.490] |
| variant-01/01-basic | 5.459 [4.401, 5.515] | 4.438 [4.435, 5.454] |
| variant-01/02-start-equals-end | 5.464 [4.406, 5.466] | 4.411 [4.386, 4.428] |
| variant-01/03-zero-cost-detour | 5.714 [4.428, 6.733] | 4.431 [3.321, 4.434] |
| variant-01/04-no-path | 4.458 [3.296, 4.489] | 4.423 [4.412, 4.434] |
| variant-01/05-multi-path | 4.458 [4.423, 5.479] | 4.401 [4.331, 7.671] |
| variant-01/06-layered-dag | 5.654 [5.515, 6.602] | 5.515 [5.493, 5.536] |
| variant-01/07-cycles | 5.103 [4.348, 5.431] | 4.450 [4.388, 6.601] |
| variant-01/08-negative-weights | 4.451 [4.390, 5.507] | 4.486 [4.450, 5.465] |
| variant-02/01-basic | 5.518 [5.455, 5.550] | 4.443 [4.437, 4.443] |
| variant-02/02-start-equals-end | 4.467 [4.429, 5.454] | 4.440 [4.437, 5.418] |
| variant-02/03-before-forces-detour | 4.436 [4.374, 5.508] | 4.449 [4.389, 5.444] |
| variant-02/04-after-forces-extension | 5.489 [4.465, 5.496] | 4.399 [4.387, 4.434] |
| variant-02/05-ordering-unsat | 4.403 [4.396, 4.466] | 4.422 [4.330, 5.731] |
| variant-02/06-layered-dag-before | 5.455 [5.447, 6.553] | 5.585 [5.496, 6.516] |
| variant-02/07-layered-dag-before-after | 5.524 [5.516, 7.625] | 5.803 [5.429, 7.664] |
| variant-02/08-tie-break-under-ordering | 5.509 [5.506, 5.510] | 6.559 [4.487, 7.785] |
| variant-02/09-negative-weights | 5.455 [5.410, 5.497] | 4.422 [4.380, 5.458] |
| variant-03/01-basic | 4.429 [4.396, 4.432] | 4.436 [4.425, 4.450] |
| variant-03/02-start-equals-end | 4.407 [4.401, 4.421] | 4.460 [4.444, 5.409] |
| variant-03/03-budget-forces-detour | 4.400 [4.394, 4.431] | 4.430 [4.401, 4.438] |
| variant-03/04-cost-at-cap-allowed | 5.493 [3.341, 5.498] | 5.408 [4.386, 5.482] |
| variant-03/05-budget-unsat | 5.457 [4.437, 6.532] | 4.392 [4.382, 5.497] |
| variant-03/06-layered-dag-cap | 7.600 [6.566, 8.657] | 6.597 [5.448, 6.834] |
| variant-03/07-layered-dag-tight-cap | 6.557 [6.511, 7.761] | 6.620 [6.564, 7.631] |
| variant-03/08-tie-break-under-cap | 5.398 [4.364, 6.197] | 5.418 [4.422, 5.510] |
| variant-03/09-negative-weights | 5.489 [4.396, 5.496] | 5.423 [4.430, 5.487] |
| variant-04/01-basic | 4.431 [4.398, 5.511] | 5.538 [5.503, 5.558] |
| variant-04/02-start-equals-end | 4.407 [4.384, 4.419] | 4.434 [4.406, 5.541] |
| variant-04/03-before-forces-detour-within-budget | 4.450 [4.382, 5.418] | 4.411 [4.377, 4.435] |
| variant-04/04-ordering-violates-budget | 5.479 [4.357, 5.534] | 4.646 [4.344, 5.479] |
| variant-04/05-after-and-budget-interact | 5.490 [4.384, 5.626] | 5.463 [4.457, 5.506] |
| variant-04/06-layered-dag-ordering-cap | 8.755 [6.561, 8.784] | 6.544 [5.484, 6.562] |
| variant-04/07-layered-dag-combined | 7.591 [6.576, 7.642] | 7.635 [6.550, 8.755] |
| variant-04/08-tie-break-under-ordering-and-cap | 5.468 [4.376, 5.517] | 5.523 [4.486, 5.523] |
| variant-04/09-negative-weights | 5.472 [4.455, 5.553] | 4.435 [4.399, 5.511] |
| variant-01/01-basic | 4.407 [3.376, 4.519] | 4.423 [3.281, 4.456] |
| variant-01/02-agent-reuse | 4.425 [4.389, 4.433] | 4.435 [4.431, 5.482] |
| variant-01/03-selective-compatibility | 4.436 [4.346, 4.461] | 4.450 [4.446, 5.513] |
| variant-01/04-no-compatible-agent-unsat | 4.460 [3.286, 6.633] | 4.430 [4.418, 4.436] |
| variant-01/05-larger-mix | 4.437 [4.395, 4.467] | 4.432 [4.413, 5.486] |
| variant-02/01-basic | 4.449 [4.440, 4.566] | 5.456 [4.497, 5.494] |
| variant-02/02-makespan-tiebreak | 4.435 [4.372, 4.442] | 5.489 [4.464, 6.593] |
| variant-02/03-cost-dominates | 5.431 [4.454, 5.493] | 4.417 [4.377, 4.455] |
| variant-02/04-no-compatible-agent-unsat | 4.426 [4.423, 5.514] | 4.426 [4.418, 4.435] |
| variant-02/05-larger-mix | 5.474 [5.468, 6.552] | 5.548 [5.495, 7.737] |
| variant-03/01-basic | 4.404 [4.397, 4.429] | 4.441 [4.439, 5.503] |
| variant-03/02-multiple-groups | 4.362 [3.373, 4.436] | 4.406 [4.364, 4.432] |
| variant-03/03-mixed-grouped-ungrouped | 4.388 [3.362, 4.428] | 4.412 [4.325, 4.433] |
| variant-03/04-incompatible-group-unsat | 4.433 [3.285, 5.470] | 4.349 [3.353, 4.384] |
| variant-03/05-larger-mix | 4.437 [4.361, 4.438] | 4.417 [4.403, 4.619] |
| variant-04/01-basic | 27.959 [27.864, 28.865] | 27.007 [25.696, 27.138] |
| variant-04/02-precedence | 5.536 [5.505, 6.567] | 6.596 [5.570, 7.708] |
| variant-04/03-agent-serialization | 27.806 [27.746, 28.067] | 27.991 [26.645, 28.818] |
| variant-04/04-window-too-tight-unsat | 5.558 [5.440, 5.577] | 4.497 [4.396, 4.521] |
| variant-04/05-larger-mix | 148.066 [147.730, 148.143] | 148.426 [146.295, 148.772] |
| variant-01/01-basic | 4.467 [4.381, 5.706] | 5.636 [4.518, 5.800] |
| variant-01/02-multiple-tours | 4.500 [4.399, 5.487] | 5.468 [4.584, 5.519] |
| variant-01/03-asymmetric | 5.460 [4.357, 5.677] | 4.463 [4.404, 5.461] |
| variant-01/04-subtour-unsat | 3.331 [3.321, 4.389] | 6.650 [3.278, 6.655] |
| variant-01/05-ring | 4.474 [4.380, 6.586] | 5.492 [4.441, 6.726] |
| variant-02/01-basic | 4.444 [4.398, 5.505] | 5.457 [4.457, 5.510] |
| variant-02/02-single-salesman | 5.504 [4.389, 5.524] | 4.397 [4.361, 4.450] |
| variant-02/03-too-many-salesmen-unsat | 4.435 [4.397, 5.454] | 5.463 [4.429, 5.506] |
| variant-02/04-equal-cost-split | 4.453 [4.395, 5.517] | 4.459 [4.401, 5.493] |
| variant-02/05-larger-asymmetric | 4.449 [4.400, 5.450] | 5.432 [4.411, 5.444] |
| variant-02/06-unreachable-edge | 5.512 [4.393, 5.553] | 4.437 [4.411, 5.505] |
| variant-03/01-basic | 4.421 [4.401, 4.474] | 4.427 [4.385, 7.676] |
| variant-03/02-single-salesman | 4.449 [4.426, 6.605] | 4.469 [3.334, 5.476] |
| variant-03/03-depot-crossing-unsat | 4.441 [3.352, 6.664] | 4.404 [3.316, 5.510] |
| variant-03/04-equal-cost-split | 4.459 [4.455, 4.471] | 5.512 [4.479, 6.964] |
| variant-03/05-larger-three-depots | 4.583 [4.456, 5.502] | 5.481 [4.518, 5.486] |
| variant-03/06-unreachable-edge | 4.421 [4.357, 4.466] | 4.478 [4.436, 5.507] |
| variant-04/01-basic | 4.430 [4.394, 5.498] | 4.476 [4.425, 5.461] |
| variant-04/02-single-salesman | 5.536 [5.450, 6.560] | 5.514 [4.430, 6.511] |
| variant-04/03-window-too-tight-unsat | 4.433 [4.381, 5.461] | 5.457 [4.460, 5.503] |
| variant-04/04-depot-window-too-tight-unsat | 4.425 [4.411, 5.494] | 4.425 [4.357, 4.457] |
| variant-04/05-three-depots-asymmetric-times | 5.488 [5.486, 5.515] | 5.462 [4.421, 5.497] |
| variant-04/06-unreachable-edge | 4.438 [4.398, 6.557] | 5.295 [4.388, 5.486] |
| variant-05/01-basic | 4.445 [4.412, 5.501] | 5.445 [4.463, 5.783] |
| variant-05/02-tight-bound-exact | 4.431 [4.424, 5.419] | 4.466 [4.394, 5.478] |
| variant-05/03-revisit-too-tight-unsat | 4.447 [4.431, 5.518] | 4.555 [4.390, 7.669] |
| variant-05/04-depot-and-vertex-revisits | 5.470 [4.649, 5.526] | 4.611 [4.502, 5.490] |
| variant-05/05-three-depots-mixed | 4.438 [4.387, 4.452] | 5.464 [4.433, 5.500] |
| variant-05/06-unreachable-edge | 4.442 [4.438, 5.448] | 5.460 [4.406, 5.579] |
| n-queens/variant-01 | 5.478 [5.465, 5.507] | 6.575 [5.467, 8.778] |
| n-queens/variant-02 | 119.410 [118.923, 121.277] | 119.661 [117.852, 122.142] |
| n-queens/variant-03 | 6.568 [5.680, 6.892] | 6.641 [6.629, 7.683] |
| n-queens/variant-04 | 6.584 [6.558, 6.858] | 6.583 [6.510, 6.589] |
| n-queens/variant-05 | 6.555 [5.671, 6.594] | 6.574 [6.561, 6.807] |
| n-queens/variant-06 | 6.587 [5.712, 6.598] | 5.540 [5.527, 9.876] |
| send-money/send-money | 14.993 [13.939, 16.162] | 15.033 [13.980, 15.069] |

Counters of report after: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 0 | 1 | 4 | 9009 | 2.137 |
| equality-generalized-tsp/02-larger | 0 | 1 | 8 | 24727 | 2.560 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 0 | 0 | 0 | 2258 | 1.437 |
| variant-01/01-basic | 0 | 1 | 2 | 1559 | 1.547 |
| variant-01/02-start-equals-end | 0 | 1 | 1 | 1208 | 1.360 |
| variant-01/03-zero-cost-detour | 0 | 1 | 2 | 1291 | 1.838 |
| variant-01/04-no-path | 0 | 0 | 0 | 1085 | 1.386 |
| variant-01/05-multi-path | 0 | 1 | 3 | 3958 | 1.569 |
| variant-01/06-layered-dag | 0 | 1 | 14 | 119074 | 4.906 |
| variant-01/07-cycles | 0 | 1 | 5 | 14519 | 2.406 |
| variant-01/08-negative-weights | 0 | 1 | 1 | 3191 | 1.748 |
| variant-02/01-basic | 0 | 1 | 2 | 1700 | 1.471 |
| variant-02/02-start-equals-end | 0 | 1 | 1 | 1557 | 1.572 |
| variant-02/03-before-forces-detour | 0 | 1 | 1 | 2383 | 1.675 |
| variant-02/04-after-forces-extension | 0 | 1 | 1 | 2245 | 1.578 |
| variant-02/05-ordering-unsat | 0 | 0 | 0 | 1322 | 1.874 |
| variant-02/06-layered-dag-before | 0 | 2 | 8 | 54149 | 4.447 |
| variant-02/07-layered-dag-before-after | 0 | 1 | 7 | 45572 | 6.115 |
| variant-02/08-tie-break-under-ordering | 0 | 1 | 2 | 2777 | 1.926 |
| variant-02/09-negative-weights | 0 | 1 | 1 | 3573 | 2.043 |
| variant-03/01-basic | 0 | 1 | 2 | 1667 | 1.574 |
| variant-03/02-start-equals-end | 0 | 1 | 1 | 1467 | 1.537 |
| variant-03/03-budget-forces-detour | 0 | 1 | 1 | 2141 | 1.698 |
| variant-03/04-cost-at-cap-allowed | 0 | 1 | 1 | 1272 | 1.496 |
| variant-03/05-budget-unsat | 0 | 0 | 0 | 4970 | 1.782 |
| variant-03/06-layered-dag-cap | 0 | 2 | 22 | 164927 | 9.147 |
| variant-03/07-layered-dag-tight-cap | 0 | 4 | 6 | 561439 | 11.532 |
| variant-03/08-tie-break-under-cap | 0 | 1 | 2 | 3301 | 1.873 |
| variant-03/09-negative-weights | 0 | 1 | 1 | 3627 | 1.928 |
| variant-04/01-basic | 0 | 1 | 2 | 1808 | 1.886 |
| variant-04/02-start-equals-end | 0 | 1 | 1 | 1816 | 2.105 |
| variant-04/03-before-forces-detour-within-budget | 0 | 1 | 1 | 2774 | 1.769 |
| variant-04/04-ordering-violates-budget | 0 | 0 | 0 | 2816 | 1.845 |
| variant-04/05-after-and-budget-interact | 0 | 1 | 1 | 4169 | 1.921 |
| variant-04/06-layered-dag-ordering-cap | 0 | 2 | 8 | 71408 | 6.527 |
| variant-04/07-layered-dag-combined | 0 | 1 | 5 | 63717 | 6.271 |
| variant-04/08-tie-break-under-ordering-and-cap | 0 | 1 | 2 | 6554 | 2.404 |
| variant-04/09-negative-weights | 0 | 1 | 1 | 4009 | 2.676 |
| variant-01/01-basic | 0 | 1 | 4 | 1462 | 1.116 |
| variant-01/02-agent-reuse | 0 | 1 | 8 | 6363 | 1.213 |
| variant-01/03-selective-compatibility | 0 | 1 | 4 | 1873 | 1.116 |
| variant-01/04-no-compatible-agent-unsat | 0 | 0 | 0 | 1217 | 0.939 |
| variant-01/05-larger-mix | 0 | 1 | 49 | 45883 | 1.988 |
| variant-02/01-basic | 0 | 1 | 4 | 3047 | 1.581 |
| variant-02/02-makespan-tiebreak | 0 | 3 | 8 | 4226 | 1.686 |
| variant-02/03-cost-dominates | 0 | 1 | 8 | 4450 | 1.591 |
| variant-02/04-no-compatible-agent-unsat | 0 | 0 | 0 | 2395 | 1.433 |
| variant-02/05-larger-mix | 0 | 1 | 11 | 271892 | 5.743 |
| variant-03/01-basic | 0 | 2 | 2 | 1970 | 1.328 |
| variant-03/02-multiple-groups | 0 | 4 | 4 | 3816 | 1.421 |
| variant-03/03-mixed-grouped-ungrouped | 0 | 2 | 4 | 2639 | 1.318 |
| variant-03/04-incompatible-group-unsat | 0 | 0 | 0 | 1224 | 1.216 |
| variant-03/05-larger-mix | 0 | 1 | 24 | 7327 | 1.885 |
| variant-04/01-basic | 0 | 81 | 93 | 111422 | 5.225 |
| variant-04/02-precedence | 0 | 3 | 11 | 13769 | 3.202 |
| variant-04/03-agent-serialization | 0 | 30 | 30 | 32832 | 4.060 |
| variant-04/04-window-too-tight-unsat | 0 | 0 | 0 | 2865 | 2.261 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 2164110 | 30.375 |
| variant-01/01-basic | 0 | 1 | 2 | 5304 | 1.825 |
| variant-01/02-multiple-tours | 0 | 2 | 9 | 38015 | 2.559 |
| variant-01/03-asymmetric | 0 | 1 | 9 | 58489 | 4.304 |
| variant-01/04-subtour-unsat | 0 | 0 | 0 | 2473 | 1.193 |
| variant-01/05-ring | 0 | 2 | 11 | 251398 | 5.457 |
| variant-02/01-basic | 0 | 2 | 2 | 5859 | 1.882 |
| variant-02/02-single-salesman | 0 | 1 | 2 | 3958 | 2.172 |
| variant-02/03-too-many-salesmen-unsat | 0 | 0 | 0 | 3466 | 1.554 |
| variant-02/04-equal-cost-split | 0 | 2 | 2 | 4429 | 1.842 |
| variant-02/05-larger-asymmetric | 0 | 2 | 2 | 6981 | 2.097 |
| variant-02/06-unreachable-edge | 0 | 2 | 2 | 8166 | 2.370 |
| variant-03/01-basic | 0 | 1 | 1 | 3939 | 1.774 |
| variant-03/02-single-salesman | 0 | 1 | 2 | 3808 | 1.885 |
| variant-03/03-depot-crossing-unsat | 0 | 0 | 0 | 3422 | 1.549 |
| variant-03/04-equal-cost-split | 0 | 2 | 2 | 8166 | 2.139 |
| variant-03/05-larger-three-depots | 0 | 1 | 1 | 10821 | 2.871 |
| variant-03/06-unreachable-edge | 0 | 1 | 1 | 6365 | 2.617 |
| variant-04/01-basic | 0 | 1 | 1 | 6955 | 2.758 |
| variant-04/02-single-salesman | 0 | 1 | 1 | 4700 | 2.478 |
| variant-04/03-window-too-tight-unsat | 0 | 0 | 0 | 3400 | 2.081 |
| variant-04/04-depot-window-too-tight-unsat | 0 | 0 | 0 | 2325 | 1.987 |
| variant-04/05-three-depots-asymmetric-times | 0 | 1 | 1 | 18328 | 4.545 |
| variant-04/06-unreachable-edge | 0 | 1 | 1 | 14062 | 4.775 |
| variant-05/01-basic | 0 | 1 | 1 | 6957 | 2.693 |
| variant-05/02-tight-bound-exact | 0 | 1 | 1 | 3439 | 2.403 |
| variant-05/03-revisit-too-tight-unsat | 0 | 0 | 0 | 3413 | 2.452 |
| variant-05/04-depot-and-vertex-revisits | 0 | 1 | 1 | 3504 | 2.420 |
| variant-05/05-three-depots-mixed | 0 | 1 | 1 | 18378 | 4.255 |
| variant-05/06-unreachable-edge | 0 | 1 | 1 | 14062 | 3.956 |
| n-queens/variant-01 | 0 | 92 | 92 | 199032 | 5.847 |
| n-queens/variant-02 | 0 | 92 | 92 | 14461401 | 66.343 |
| n-queens/variant-03 | 0 | 92 | 92 | 256834 | 5.212 |
| n-queens/variant-04 | 0 | 92 | 92 | 195309 | 4.271 |
| n-queens/variant-05 | 0 | 92 | 92 | 223549 | 4.871 |
| n-queens/variant-06 | 0 | 92 | 92 | 251111 | 5.369 |
| send-money/send-money | 0 | 1 | 1 | 409024 | 9.071 |

Against the reference: report before, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 9 of 94 cells where both passed (9.5%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 33.864 | 148.066 | 0.229 | 3.662 | 33.311 | 12.619 | 86.000 | 57.000 |
| variant-04/03-agent-serialization | 8.258 | 27.806 | 0.297 | 0.909 | 1.401 | 0.303 | 24.000 | 0.000 |
| variant-04/01-basic | 9.240 | 27.959 | 0.330 | 0.994 | 4.107 | 0.826 | 20.000 | 4.000 |
| n-queens/variant-02 | 62.990 | 119.410 | 0.528 | 2.438 | 179.034 | 1.203 | 1.000 | 114.000 |
| variant-02/01-basic | 5.127 | 5.518 | 0.929 | 0.224 | 0.069 | 0.016 | 1.000 | 0.000 |
| variant-01/01-basic | 5.093 | 5.459 | 0.933 | 0.170 | 0.065 | 0.013 | 1.000 | 0.000 |
| send-money/send-money | 14.351 | 14.993 | 0.957 | 2.550 | 6.986 | 0.290 | 9.000 | 1.000 |
| variant-01/03-zero-cost-detour | 5.538 | 5.714 | 0.969 | 0.174 | 0.044 | 0.011 | 1.000 | 0.000 |
| variant-03/04-cost-at-cap-allowed | 5.455 | 5.493 | 0.993 | 0.177 | 0.041 | 0.008 | 1.000 | 0.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 5.524 | 5.509 | 1.003 | 0.373 | 0.201 | 0.226 | 1.000 | 0.000 |
| variant-01/02-start-equals-end | 5.515 | 5.464 | 1.009 | 0.163 | 0.052 | 0.027 | 1.000 | 0.000 |
| variant-02/04-after-forces-extension | 5.556 | 5.489 | 1.012 | 0.234 | 0.070 | 0.012 | 1.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 5.595 | 5.509 | 1.016 | 0.255 | 0.099 | 0.021 | 1.000 | 0.000 |
| variant-02/06-unreachable-edge | 5.655 | 5.512 | 1.026 | 0.489 | 0.170 | 0.154 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 5.691 | 5.490 | 1.037 | 0.314 | 0.094 | 0.021 | 1.000 | 0.000 |
| variant-01/01-basic | 4.616 | 4.407 | 1.047 | 0.142 | 0.072 | 0.021 | 0.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 4.698 | 4.433 | 1.060 | 0.118 | 0.040 | 0.005 | 0.000 | 0.000 |
| variant-03/09-negative-weights | 5.820 | 5.489 | 1.060 | 0.315 | 0.089 | 0.050 | 1.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 4.786 | 4.460 | 1.073 | 0.119 | 0.032 | 0.005 | 0.000 | 0.000 |
| variant-01/04-no-path | 4.791 | 4.458 | 1.075 | 0.169 | 0.038 | 0.005 | 0.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 6.027 | 5.558 | 1.084 | 0.324 | 0.085 | 0.011 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 6.101 | 5.536 | 1.102 | 0.474 | 0.091 | 0.059 | 1.000 | 0.000 |
| variant-02/02-single-salesman | 6.068 | 5.504 | 1.103 | 0.260 | 0.102 | 0.086 | 1.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 6.082 | 5.470 | 1.112 | 0.394 | 0.086 | 0.048 | 1.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 6.093 | 5.479 | 1.112 | 0.286 | 0.071 | 0.009 | 1.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 6.099 | 5.398 | 1.130 | 0.271 | 0.111 | 0.022 | 1.000 | 0.000 |
| variant-02/05-ordering-unsat | 5.077 | 4.403 | 1.153 | 0.169 | 0.045 | 0.005 | 1.000 | 0.000 |
| variant-02/03-too-many-salesmen-unsat | 5.169 | 4.435 | 1.165 | 0.296 | 0.064 | 0.010 | 1.000 | 0.000 |
| variant-04/09-negative-weights | 6.409 | 5.472 | 1.171 | 0.309 | 0.088 | 0.053 | 1.000 | 0.000 |
| variant-02/02-start-equals-end | 5.263 | 4.467 | 1.178 | 0.176 | 0.052 | 0.029 | 1.000 | 0.000 |
| variant-02/03-cost-dominates | 6.455 | 5.431 | 1.188 | 0.322 | 0.255 | 0.059 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 5.189 | 4.362 | 1.190 | 0.221 | 0.139 | 0.036 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 6.577 | 5.457 | 1.205 | 0.363 | 0.129 | 0.013 | 1.000 | 0.000 |
| variant-01/03-asymmetric | 6.744 | 5.460 | 1.235 | 0.533 | 0.764 | 1.175 | 1.000 | 0.000 |
| variant-04/01-basic | 5.479 | 4.431 | 1.236 | 0.193 | 0.074 | 0.018 | 1.000 | 0.000 |
| variant-01/02-agent-reuse | 5.497 | 4.425 | 1.242 | 0.155 | 0.178 | 0.033 | 0.000 | 0.000 |
| variant-03/01-basic | 5.522 | 4.429 | 1.247 | 0.216 | 0.073 | 0.014 | 1.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 5.538 | 4.441 | 1.247 | 0.330 | 0.066 | 0.011 | 1.000 | 0.000 |
| equality-generalized-tsp/02-larger | 6.823 | 5.469 | 1.247 | 0.602 | 0.359 | 0.557 | 1.000 | 0.000 |
| variant-02/09-negative-weights | 6.805 | 5.455 | 1.248 | 0.250 | 0.093 | 0.051 | 1.000 | 0.000 |
| variant-03/02-start-equals-end | 5.504 | 4.407 | 1.249 | 0.161 | 0.045 | 0.027 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 5.491 | 4.388 | 1.251 | 0.195 | 0.099 | 0.025 | 0.000 | 0.000 |
| variant-01/05-multi-path | 5.588 | 4.458 | 1.253 | 0.508 | 0.124 | 0.033 | 1.000 | 0.000 |
| n-queens/variant-04 | 8.252 | 6.584 | 1.253 | 0.747 | 4.863 | 0.624 | 1.000 | 1.000 |
| variant-03/01-basic | 5.526 | 4.404 | 1.255 | 0.171 | 0.078 | 0.015 | 1.000 | 0.000 |
| variant-03/04-equal-cost-split | 5.620 | 4.459 | 1.260 | 0.667 | 0.171 | 0.153 | 1.000 | 0.000 |
| variant-04/02-start-equals-end | 5.568 | 4.407 | 1.263 | 0.222 | 0.053 | 0.039 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 5.623 | 4.450 | 1.263 | 0.294 | 0.067 | 0.014 | 1.000 | 0.000 |
| n-queens/variant-05 | 8.302 | 6.555 | 1.266 | 1.016 | 4.555 | 0.746 | 1.000 | 1.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.968 | 5.468 | 1.274 | 0.817 | 0.177 | 0.039 | 1.000 | 0.000 |
| variant-03/01-basic | 5.673 | 4.421 | 1.283 | 0.328 | 0.096 | 0.055 | 1.000 | 0.000 |
| variant-04/07-layered-dag-combined | 9.751 | 7.591 | 1.284 | 1.768 | 2.191 | 0.350 | 2.000 | 0.000 |
| variant-02/05-larger-asymmetric | 5.719 | 4.449 | 1.285 | 0.435 | 0.150 | 0.156 | 1.000 | 0.000 |
| n-queens/variant-06 | 8.475 | 6.587 | 1.287 | 1.121 | 5.031 | 0.915 | 1.000 | 1.000 |
| variant-02/02-makespan-tiebreak | 5.726 | 4.435 | 1.291 | 0.343 | 0.258 | 0.061 | 1.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.787 | 4.452 | 1.300 | 0.181 | 0.042 | 0.009 | 0.000 | 0.000 |
| variant-01/03-selective-compatibility | 5.781 | 4.436 | 1.303 | 0.170 | 0.084 | 0.021 | 0.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 11.646 | 8.755 | 1.330 | 1.810 | 2.310 | 0.389 | 3.000 | 0.000 |
| variant-02/04-equal-cost-split | 5.932 | 4.453 | 1.332 | 0.310 | 0.126 | 0.106 | 1.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 5.929 | 4.447 | 1.333 | 0.366 | 0.071 | 0.011 | 1.000 | 0.000 |
| variant-01/05-larger-mix | 5.951 | 4.437 | 1.341 | 0.271 | 1.089 | 0.178 | 1.000 | 0.000 |
| variant-01/07-cycles | 6.845 | 5.103 | 1.341 | 0.399 | 0.277 | 0.372 | 1.000 | 0.000 |
| variant-02/01-basic | 5.988 | 4.449 | 1.346 | 0.443 | 0.169 | 0.032 | 1.000 | 0.000 |
| variant-01/01-basic | 6.039 | 4.467 | 1.352 | 0.271 | 0.148 | 0.116 | 1.000 | 0.000 |
| n-queens/variant-03 | 8.932 | 6.568 | 1.360 | 1.849 | 4.660 | 0.794 | 1.000 | 1.000 |
| variant-01/02-multiple-tours | 6.186 | 4.500 | 1.375 | 0.423 | 0.502 | 1.030 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 6.129 | 4.449 | 1.378 | 0.261 | 0.092 | 0.083 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 6.073 | 4.400 | 1.380 | 0.412 | 0.065 | 0.015 | 1.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 6.125 | 4.433 | 1.382 | 0.361 | 0.072 | 0.016 | 1.000 | 0.000 |
| variant-02/01-basic | 6.176 | 4.444 | 1.390 | 0.376 | 0.129 | 0.122 | 1.000 | 0.000 |
| variant-05/02-tight-bound-exact | 6.164 | 4.431 | 1.391 | 0.386 | 0.073 | 0.053 | 1.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 6.298 | 4.426 | 1.423 | 0.306 | 0.042 | 0.008 | 1.000 | 0.000 |
| variant-03/06-unreachable-edge | 6.503 | 4.421 | 1.471 | 0.539 | 0.134 | 0.088 | 1.000 | 0.000 |
| variant-01/08-negative-weights | 6.560 | 4.451 | 1.474 | 0.322 | 0.078 | 0.042 | 1.000 | 0.000 |
| variant-01/04-subtour-unsat | 4.916 | 3.331 | 1.476 | 0.174 | 0.039 | 0.008 | 0.000 | 0.000 |
| variant-05/01-basic | 6.562 | 4.445 | 1.476 | 0.600 | 0.130 | 0.083 | 1.000 | 0.000 |
| variant-02/06-layered-dag-before | 8.085 | 5.455 | 1.482 | 1.105 | 1.726 | 0.287 | 2.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 8.155 | 5.488 | 1.486 | 1.942 | 0.238 | 0.189 | 1.000 | 0.000 |
| variant-02/03-before-forces-detour | 6.600 | 4.436 | 1.488 | 0.220 | 0.055 | 0.012 | 1.000 | 0.000 |
| variant-03/05-larger-mix | 6.611 | 4.437 | 1.490 | 0.287 | 0.587 | 0.095 | 1.000 | 0.000 |
| variant-04/01-basic | 6.633 | 4.430 | 1.497 | 0.615 | 0.117 | 0.082 | 1.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 6.674 | 4.425 | 1.508 | 0.304 | 0.051 | 0.010 | 1.000 | 0.000 |
| variant-04/02-precedence | 8.354 | 5.536 | 1.509 | 0.619 | 0.721 | 0.113 | 2.000 | 0.000 |
| variant-01/06-layered-dag | 8.777 | 5.654 | 1.552 | 0.998 | 3.791 | 0.340 | 1.000 | 0.000 |
| variant-03/05-larger-three-depots | 7.131 | 4.583 | 1.556 | 1.088 | 0.150 | 0.120 | 1.000 | 0.000 |
| variant-03/06-layered-dag-cap | 11.883 | 7.600 | 1.563 | 2.082 | 5.485 | 0.461 | 2.000 | 1.000 |
| variant-02/07-layered-dag-before-after | 8.878 | 5.524 | 1.607 | 1.126 | 1.535 | 0.241 | 2.000 | 0.000 |
| n-queens/variant-01 | 8.941 | 5.478 | 1.632 | 1.843 | 4.694 | 0.972 | 1.000 | 1.000 |
| variant-02/05-larger-mix | 9.348 | 5.474 | 1.708 | 0.717 | 5.544 | 0.204 | 1.000 | 0.000 |
| variant-04/06-unreachable-edge | 7.653 | 4.438 | 1.725 | 1.186 | 0.202 | 0.163 | 1.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 12.971 | 6.557 | 1.978 | 1.714 | 6.631 | 0.287 | 2.000 | 0.000 |
| variant-05/05-three-depots-mixed | 8.814 | 4.438 | 1.986 | 1.539 | 0.251 | 0.202 | 1.000 | 0.000 |
| variant-05/06-unreachable-edge | 8.838 | 4.442 | 1.990 | 1.224 | 0.219 | 0.165 | 1.000 | 0.000 |
| variant-01/05-ring | 9.451 | 4.474 | 2.113 | 0.656 | 2.406 | 1.629 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 15.9 | 23.6 | n/a |
| variant-04/03-agent-serialization | 11.7 | 12.4 | n/a |
| variant-04/01-basic | 12.0 | 13.3 | n/a |
| n-queens/variant-02 | 16.6 | 12.8 | n/a |
| variant-02/01-basic | 10.5 | 10.5 | n/a |
| variant-01/01-basic | 10.2 | 10.6 | n/a |
| send-money/send-money | 14.3 | 12.8 | n/a |
| variant-01/03-zero-cost-detour | 10.4 | 10.4 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.8 | 10.5 | n/a |
| equality-generalized-tsp/01-basic | 10.6 | 10.5 | n/a |
| variant-01/02-start-equals-end | 10.6 | 10.4 | n/a |
| variant-02/04-after-forces-extension | 10.5 | 10.5 | n/a |
| variant-02/08-tie-break-under-ordering | 10.5 | 10.6 | n/a |
| variant-02/06-unreachable-edge | 10.7 | 10.5 | n/a |
| variant-04/05-after-and-budget-interact | 10.8 | 10.3 | n/a |
| variant-01/01-basic | 10.2 | 10.5 | n/a |
| variant-03/04-incompatible-group-unsat | 10.5 | 10.1 | n/a |
| variant-03/09-negative-weights | 11.0 | 10.5 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.4 | 10.4 | n/a |
| variant-01/04-no-path | 10.7 | 10.4 | n/a |
| variant-04/04-window-too-tight-unsat | 11.3 | 10.5 | n/a |
| variant-04/02-single-salesman | 11.1 | 10.4 | n/a |
| variant-02/02-single-salesman | 10.6 | 10.5 | n/a |
| variant-05/04-depot-and-vertex-revisits | 10.9 | 10.6 | n/a |
| variant-04/04-ordering-violates-budget | 11.0 | 10.5 | n/a |
| variant-03/08-tie-break-under-cap | 10.5 | 10.4 | n/a |
| variant-02/05-ordering-unsat | 10.4 | 10.4 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.4 | 10.4 | n/a |
| variant-04/09-negative-weights | 11.1 | 10.6 | n/a |
| variant-02/02-start-equals-end | 10.8 | 10.6 | n/a |
| variant-02/03-cost-dominates | 10.4 | 10.4 | n/a |
| variant-03/02-multiple-groups | 10.8 | 10.2 | n/a |
| variant-03/05-budget-unsat | 10.9 | 10.5 | n/a |
| variant-01/03-asymmetric | 12.1 | 10.6 | n/a |
| variant-04/01-basic | 11.0 | 10.3 | n/a |
| variant-01/02-agent-reuse | 10.5 | 10.2 | n/a |
| variant-03/01-basic | 10.6 | 10.4 | n/a |
| variant-03/03-depot-crossing-unsat | 10.7 | 10.2 | n/a |
| equality-generalized-tsp/02-larger | 11.1 | 10.5 | n/a |
| variant-02/09-negative-weights | 10.6 | 10.6 | n/a |
| variant-03/02-start-equals-end | 10.5 | 10.6 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.5 | 10.3 | n/a |
| variant-01/05-multi-path | 10.4 | 10.6 | n/a |
| n-queens/variant-04 | 11.2 | 10.4 | n/a |
| variant-03/01-basic | 10.4 | 10.5 | n/a |
| variant-03/04-equal-cost-split | 10.7 | 10.6 | n/a |
| variant-04/02-start-equals-end | 10.9 | 10.4 | n/a |
| variant-04/03-before-forces-detour-within-budget | 10.9 | 10.6 | n/a |
| n-queens/variant-05 | 11.4 | 10.6 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 10.9 | 10.7 | n/a |
| variant-03/01-basic | 10.4 | 10.4 | n/a |
| variant-04/07-layered-dag-combined | 12.6 | 11.0 | n/a |
| variant-02/05-larger-asymmetric | 10.5 | 10.6 | n/a |
| n-queens/variant-06 | 11.7 | 10.6 | n/a |
| variant-02/02-makespan-tiebreak | 10.5 | 10.5 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.4 | 10.5 | n/a |
| variant-01/03-selective-compatibility | 10.3 | 10.4 | n/a |
| variant-04/06-layered-dag-ordering-cap | 12.9 | 11.1 | n/a |
| variant-02/04-equal-cost-split | 10.8 | 10.2 | n/a |
| variant-05/03-revisit-too-tight-unsat | 10.9 | 10.4 | n/a |
| variant-01/05-larger-mix | 10.7 | 10.4 | n/a |
| variant-01/07-cycles | 10.8 | 10.5 | n/a |
| variant-02/01-basic | 10.4 | 10.4 | n/a |
| variant-01/01-basic | 10.3 | 10.5 | n/a |
| n-queens/variant-03 | 11.0 | 10.5 | n/a |
| variant-01/02-multiple-tours | 11.4 | 10.6 | n/a |
| variant-03/02-single-salesman | 10.6 | 10.2 | n/a |
| variant-03/03-budget-forces-detour | 10.6 | 10.5 | n/a |
| variant-04/03-window-too-tight-unsat | 11.0 | 10.4 | n/a |
| variant-02/01-basic | 10.5 | 10.4 | n/a |
| variant-05/02-tight-bound-exact | 11.0 | 10.4 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.3 | 10.4 | n/a |
| variant-03/06-unreachable-edge | 10.9 | 10.5 | n/a |
| variant-01/08-negative-weights | 10.8 | 10.2 | n/a |
| variant-01/04-subtour-unsat | 10.3 | 10.4 | n/a |
| variant-05/01-basic | 11.2 | 10.6 | n/a |
| variant-02/06-layered-dag-before | 11.5 | 10.8 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.3 | 10.6 | n/a |
| variant-02/03-before-forces-detour | 10.6 | 10.5 | n/a |
| variant-03/05-larger-mix | 10.6 | 10.3 | n/a |
| variant-04/01-basic | 10.9 | 10.6 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 10.8 | 10.6 | n/a |
| variant-04/02-precedence | 11.4 | 10.8 | n/a |
| variant-01/06-layered-dag | 12.5 | 10.5 | n/a |
| variant-03/05-larger-three-depots | 11.1 | 10.3 | n/a |
| variant-03/06-layered-dag-cap | 13.4 | 11.1 | n/a |
| variant-02/07-layered-dag-before-after | 11.7 | 10.5 | n/a |
| n-queens/variant-01 | 11.3 | 10.6 | n/a |
| variant-02/05-larger-mix | 11.7 | 10.6 | n/a |
| variant-04/06-unreachable-edge | 11.4 | 10.4 | n/a |
| variant-03/07-layered-dag-tight-cap | 15.5 | 11.0 | n/a |
| variant-05/05-three-depots-mixed | 11.5 | 10.4 | n/a |
| variant-05/06-unreachable-edge | 11.4 | 10.5 | n/a |
| variant-01/05-ring | 13.6 | 10.6 | n/a |

Against the reference: report after, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 9 of 94 cells where both passed (9.5%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 34.283 | 148.426 | 0.231 | 4.144 | 33.811 | 11.057 | 84.000 | 57.000 |
| variant-04/03-agent-serialization | 8.241 | 27.991 | 0.294 | 0.975 | 1.488 | 0.313 | 24.000 | 0.000 |
| variant-04/01-basic | 9.079 | 27.007 | 0.336 | 0.997 | 3.932 | 0.640 | 18.000 | 4.000 |
| n-queens/variant-02 | 71.078 | 119.661 | 0.594 | 3.029 | 189.515 | 1.128 | 2.000 | 114.000 |
| variant-01/04-subtour-unsat | 4.845 | 6.650 | 0.729 | 0.183 | 0.042 | 0.010 | 2.000 | 0.000 |
| send-money/send-money | 13.518 | 15.033 | 0.899 | 2.588 | 6.340 | 0.331 | 9.000 | 1.000 |
| variant-02/08-tie-break-under-ordering | 6.044 | 6.559 | 0.922 | 0.250 | 0.096 | 0.019 | 1.000 | 0.000 |
| variant-03/04-cost-at-cap-allowed | 5.207 | 5.408 | 0.963 | 0.178 | 0.047 | 0.008 | 1.000 | 0.000 |
| variant-03/09-negative-weights | 5.314 | 5.423 | 0.980 | 0.290 | 0.097 | 0.049 | 1.000 | 0.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/01-basic | 5.565 | 5.538 | 1.005 | 0.229 | 0.084 | 0.019 | 1.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 4.460 | 4.430 | 1.007 | 0.112 | 0.036 | 0.005 | 0.000 | 0.000 |
| variant-02/01-basic | 5.539 | 5.456 | 1.015 | 0.289 | 0.160 | 0.022 | 1.000 | 0.000 |
| variant-02/03-too-many-salesmen-unsat | 5.549 | 5.463 | 1.016 | 0.301 | 0.081 | 0.013 | 1.000 | 0.000 |
| variant-01/01-basic | 5.739 | 5.636 | 1.018 | 0.260 | 0.150 | 0.126 | 1.000 | 0.000 |
| variant-04/02-precedence | 6.725 | 6.596 | 1.020 | 0.641 | 0.856 | 0.112 | 2.000 | 0.000 |
| variant-01/02-agent-reuse | 4.536 | 4.435 | 1.023 | 0.149 | 0.104 | 0.031 | 0.000 | 0.000 |
| variant-01/03-selective-compatibility | 4.566 | 4.450 | 1.026 | 0.170 | 0.076 | 0.018 | 0.000 | 0.000 |
| variant-02/05-larger-asymmetric | 5.650 | 5.432 | 1.040 | 0.464 | 0.160 | 0.152 | 1.000 | 0.000 |
| equality-generalized-tsp/01-basic | 5.701 | 5.479 | 1.041 | 0.371 | 0.201 | 0.220 | 1.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 5.720 | 5.489 | 1.042 | 0.346 | 0.274 | 0.062 | 1.000 | 0.000 |
| variant-01/01-basic | 4.677 | 4.423 | 1.057 | 0.138 | 0.067 | 0.016 | 0.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 4.620 | 4.349 | 1.062 | 0.113 | 0.048 | 0.005 | 0.000 | 0.000 |
| variant-03/01-basic | 4.724 | 4.436 | 1.065 | 0.204 | 0.080 | 0.015 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 5.847 | 5.463 | 1.070 | 0.297 | 0.099 | 0.019 | 1.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 5.858 | 5.457 | 1.073 | 0.361 | 0.070 | 0.011 | 1.000 | 0.000 |
| variant-01/02-multiple-tours | 5.887 | 5.468 | 1.077 | 0.421 | 0.391 | 0.798 | 1.000 | 0.000 |
| variant-03/04-equal-cost-split | 5.976 | 5.512 | 1.084 | 0.440 | 0.186 | 0.176 | 1.000 | 0.000 |
| variant-01/02-start-equals-end | 4.794 | 4.411 | 1.087 | 0.148 | 0.045 | 0.025 | 1.000 | 0.000 |
| variant-02/01-basic | 5.936 | 5.457 | 1.088 | 0.354 | 0.145 | 0.126 | 1.000 | 0.000 |
| variant-01/04-no-path | 4.868 | 4.423 | 1.101 | 0.155 | 0.040 | 0.006 | 1.000 | 0.000 |
| variant-03/01-basic | 4.900 | 4.441 | 1.103 | 0.163 | 0.080 | 0.015 | 0.000 | 0.000 |
| variant-04/02-single-salesman | 6.090 | 5.514 | 1.105 | 0.456 | 0.112 | 0.060 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 4.877 | 4.406 | 1.107 | 0.208 | 0.129 | 0.033 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 4.901 | 4.412 | 1.111 | 0.185 | 0.113 | 0.027 | 0.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.063 | 4.463 | 1.134 | 0.188 | 0.044 | 0.010 | 0.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 6.269 | 5.418 | 1.157 | 0.275 | 0.121 | 0.023 | 1.000 | 0.000 |
| variant-03/05-larger-mix | 5.139 | 4.417 | 1.163 | 0.276 | 0.479 | 0.109 | 0.000 | 0.000 |
| variant-02/02-start-equals-end | 5.208 | 4.440 | 1.173 | 0.169 | 0.054 | 0.030 | 1.000 | 0.000 |
| variant-02/03-before-forces-detour | 5.223 | 4.449 | 1.174 | 0.231 | 0.083 | 0.014 | 1.000 | 0.000 |
| variant-02/03-cost-dominates | 5.212 | 4.417 | 1.180 | 0.330 | 0.224 | 0.051 | 1.000 | 0.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.649 | 5.523 | 1.204 | 0.437 | 0.184 | 0.036 | 1.000 | 0.000 |
| n-queens/variant-04 | 7.956 | 6.583 | 1.209 | 0.717 | 4.942 | 0.588 | 1.000 | 1.000 |
| variant-03/05-larger-three-depots | 6.635 | 5.481 | 1.211 | 0.833 | 0.169 | 0.123 | 1.000 | 0.000 |
| variant-01/05-larger-mix | 5.484 | 4.432 | 1.237 | 0.249 | 1.388 | 0.158 | 0.000 | 0.000 |
| variant-01/08-negative-weights | 5.556 | 4.486 | 1.238 | 0.242 | 0.083 | 0.044 | 1.000 | 0.000 |
| variant-01/01-basic | 5.502 | 4.438 | 1.240 | 0.161 | 0.075 | 0.015 | 1.000 | 0.000 |
| variant-03/02-start-equals-end | 5.544 | 4.460 | 1.243 | 0.152 | 0.049 | 0.027 | 1.000 | 0.000 |
| variant-02/01-basic | 5.526 | 4.443 | 1.244 | 0.169 | 0.059 | 0.013 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 5.524 | 4.430 | 1.247 | 0.211 | 0.066 | 0.015 | 1.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 5.526 | 4.426 | 1.249 | 0.284 | 0.047 | 0.010 | 0.000 | 0.000 |
| variant-03/01-basic | 5.531 | 4.427 | 1.249 | 0.297 | 0.091 | 0.054 | 1.000 | 0.000 |
| variant-02/05-ordering-unsat | 5.532 | 4.422 | 1.251 | 0.180 | 0.047 | 0.006 | 1.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 5.523 | 4.404 | 1.254 | 0.285 | 0.068 | 0.010 | 1.000 | 0.000 |
| variant-01/05-multi-path | 5.519 | 4.401 | 1.254 | 0.277 | 0.152 | 0.028 | 1.000 | 0.000 |
| variant-02/04-after-forces-extension | 5.521 | 4.399 | 1.255 | 0.227 | 0.067 | 0.013 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 5.526 | 4.392 | 1.258 | 0.339 | 0.137 | 0.014 | 1.000 | 0.000 |
| variant-05/01-basic | 6.864 | 5.445 | 1.261 | 0.606 | 0.126 | 0.085 | 1.000 | 0.000 |
| variant-05/02-tight-bound-exact | 5.641 | 4.466 | 1.263 | 0.371 | 0.088 | 0.049 | 1.000 | 0.000 |
| variant-01/03-zero-cost-detour | 5.612 | 4.431 | 1.267 | 0.166 | 0.062 | 0.015 | 1.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 5.891 | 4.646 | 1.268 | 0.274 | 0.071 | 0.010 | 1.000 | 0.000 |
| n-queens/variant-05 | 8.370 | 6.574 | 1.273 | 1.020 | 4.677 | 0.699 | 1.000 | 1.000 |
| variant-02/04-equal-cost-split | 5.764 | 4.459 | 1.293 | 0.305 | 0.121 | 0.081 | 1.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 5.796 | 4.425 | 1.310 | 0.295 | 0.059 | 0.010 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 5.805 | 4.411 | 1.316 | 0.256 | 0.072 | 0.015 | 1.000 | 0.000 |
| variant-04/02-start-equals-end | 5.881 | 4.434 | 1.326 | 0.307 | 0.056 | 0.028 | 1.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 5.983 | 4.497 | 1.330 | 0.310 | 0.098 | 0.011 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 6.134 | 4.469 | 1.373 | 0.268 | 0.094 | 0.084 | 1.000 | 0.000 |
| variant-04/07-layered-dag-combined | 10.481 | 7.635 | 1.373 | 1.792 | 2.295 | 0.340 | 2.000 | 0.000 |
| variant-02/02-single-salesman | 6.040 | 4.397 | 1.374 | 0.258 | 0.098 | 0.066 | 1.000 | 0.000 |
| variant-02/09-negative-weights | 6.092 | 4.422 | 1.378 | 0.243 | 0.098 | 0.048 | 1.000 | 0.000 |
| variant-03/06-unreachable-edge | 6.190 | 4.478 | 1.382 | 0.543 | 0.124 | 0.082 | 1.000 | 0.000 |
| n-queens/variant-03 | 9.284 | 6.641 | 1.398 | 1.787 | 4.787 | 0.715 | 1.000 | 1.000 |
| variant-02/06-unreachable-edge | 6.320 | 4.437 | 1.424 | 0.470 | 0.172 | 0.166 | 1.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 6.603 | 4.611 | 1.432 | 0.369 | 0.082 | 0.048 | 1.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 6.570 | 4.555 | 1.443 | 0.368 | 0.071 | 0.011 | 1.000 | 0.000 |
| variant-05/06-unreachable-edge | 7.905 | 5.460 | 1.448 | 1.157 | 0.211 | 0.181 | 1.000 | 0.000 |
| variant-02/06-layered-dag-before | 8.092 | 5.585 | 1.449 | 1.632 | 1.832 | 0.286 | 2.000 | 0.000 |
| variant-04/01-basic | 6.531 | 4.476 | 1.459 | 0.587 | 0.123 | 0.081 | 1.000 | 0.000 |
| variant-04/09-negative-weights | 6.550 | 4.435 | 1.477 | 0.334 | 0.095 | 0.053 | 1.000 | 0.000 |
| equality-generalized-tsp/02-larger | 6.575 | 4.438 | 1.482 | 0.589 | 0.409 | 0.605 | 1.000 | 0.000 |
| variant-05/05-three-depots-mixed | 8.105 | 5.464 | 1.483 | 1.496 | 0.271 | 0.201 | 1.000 | 0.000 |
| n-queens/variant-01 | 9.803 | 6.575 | 1.491 | 2.746 | 4.215 | 0.737 | 1.000 | 1.000 |
| variant-01/07-cycles | 6.751 | 4.450 | 1.517 | 0.370 | 0.281 | 0.394 | 1.000 | 0.000 |
| variant-04/06-unreachable-edge | 8.359 | 5.295 | 1.579 | 1.603 | 0.198 | 0.107 | 1.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 10.509 | 6.544 | 1.606 | 1.820 | 2.469 | 0.426 | 2.000 | 0.000 |
| variant-01/06-layered-dag | 8.928 | 5.515 | 1.619 | 1.005 | 3.662 | 0.293 | 1.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 9.194 | 5.462 | 1.683 | 1.531 | 0.241 | 0.201 | 1.000 | 0.000 |
| variant-02/07-layered-dag-before-after | 9.842 | 5.803 | 1.696 | 1.630 | 1.602 | 0.250 | 2.000 | 0.000 |
| variant-01/05-ring | 9.425 | 5.492 | 1.716 | 0.668 | 3.268 | 2.085 | 1.000 | 0.000 |
| variant-01/03-asymmetric | 7.738 | 4.463 | 1.734 | 0.698 | 0.732 | 1.263 | 1.000 | 0.000 |
| variant-02/05-larger-mix | 9.804 | 5.548 | 1.767 | 0.729 | 6.219 | 0.156 | 1.000 | 0.000 |
| n-queens/variant-06 | 9.837 | 5.540 | 1.776 | 1.063 | 5.379 | 0.840 | 1.000 | 1.000 |
| variant-03/06-layered-dag-cap | 13.056 | 6.597 | 1.979 | 1.619 | 6.616 | 0.413 | 2.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 15.317 | 6.620 | 2.314 | 2.609 | 8.541 | 0.257 | 2.000 | 1.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 17.7 | 23.5 | n/a |
| variant-04/03-agent-serialization | 11.0 | 12.2 | n/a |
| variant-04/01-basic | 11.3 | 13.3 | n/a |
| n-queens/variant-02 | 19.4 | 12.8 | n/a |
| variant-01/04-subtour-unsat | 10.3 | 10.3 | n/a |
| send-money/send-money | 14.5 | 12.7 | n/a |
| variant-02/08-tie-break-under-ordering | 10.7 | 10.6 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.5 | 10.4 | n/a |
| variant-03/09-negative-weights | 10.6 | 10.6 | n/a |
| variant-04/01-basic | 10.6 | 10.5 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.3 | 10.2 | n/a |
| variant-02/01-basic | 10.3 | 10.5 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.5 | 10.2 | n/a |
| variant-01/01-basic | 10.5 | 10.5 | n/a |
| variant-04/02-precedence | 10.7 | 10.7 | n/a |
| variant-01/02-agent-reuse | 10.5 | 10.1 | n/a |
| variant-01/03-selective-compatibility | 10.0 | 10.4 | n/a |
| variant-02/05-larger-asymmetric | 10.4 | 10.5 | n/a |
| equality-generalized-tsp/01-basic | 10.5 | 10.4 | n/a |
| variant-02/02-makespan-tiebreak | 10.2 | 10.4 | n/a |
| variant-01/01-basic | 10.0 | 10.5 | n/a |
| variant-03/04-incompatible-group-unsat | 10.2 | 10.2 | n/a |
| variant-03/01-basic | 10.7 | 10.6 | n/a |
| variant-04/05-after-and-budget-interact | 10.7 | 10.3 | n/a |
| variant-04/03-window-too-tight-unsat | 10.7 | 10.6 | n/a |
| variant-01/02-multiple-tours | 11.1 | 10.6 | n/a |
| variant-03/04-equal-cost-split | 10.8 | 10.2 | n/a |
| variant-01/02-start-equals-end | 10.3 | 10.2 | n/a |
| variant-02/01-basic | 10.3 | 10.6 | n/a |
| variant-01/04-no-path | 10.3 | 10.5 | n/a |
| variant-03/01-basic | 10.2 | 10.5 | n/a |
| variant-04/02-single-salesman | 10.8 | 10.5 | n/a |
| variant-03/02-multiple-groups | 10.3 | 10.4 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.1 | 10.3 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.3 | 10.3 | n/a |
| variant-03/08-tie-break-under-cap | 10.7 | 10.3 | n/a |
| variant-03/05-larger-mix | 10.4 | 10.3 | n/a |
| variant-02/02-start-equals-end | 10.7 | 10.5 | n/a |
| variant-02/03-before-forces-detour | 10.7 | 10.6 | n/a |
| variant-02/03-cost-dominates | 10.6 | 10.5 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 10.8 | 10.6 | n/a |
| n-queens/variant-04 | 11.3 | 10.5 | n/a |
| variant-03/05-larger-three-depots | 10.9 | 10.5 | n/a |
| variant-01/05-larger-mix | 10.9 | 10.1 | n/a |
| variant-01/08-negative-weights | 10.4 | 10.4 | n/a |
| variant-01/01-basic | 10.3 | 10.3 | n/a |
| variant-03/02-start-equals-end | 10.6 | 10.5 | n/a |
| variant-02/01-basic | 10.5 | 10.5 | n/a |
| variant-03/03-budget-forces-detour | 10.4 | 10.2 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.4 | 10.4 | n/a |
| variant-03/01-basic | 10.4 | 10.5 | n/a |
| variant-02/05-ordering-unsat | 10.5 | 10.3 | n/a |
| variant-03/03-depot-crossing-unsat | 10.3 | 10.5 | n/a |
| variant-01/05-multi-path | 10.6 | 10.5 | n/a |
| variant-02/04-after-forces-extension | 10.6 | 10.6 | n/a |
| variant-03/05-budget-unsat | 10.6 | 10.5 | n/a |
| variant-05/01-basic | 11.0 | 10.5 | n/a |
| variant-05/02-tight-bound-exact | 10.9 | 10.6 | n/a |
| variant-01/03-zero-cost-detour | 10.5 | 10.4 | n/a |
| variant-04/04-ordering-violates-budget | 10.8 | 10.5 | n/a |
| n-queens/variant-05 | 11.3 | 10.5 | n/a |
| variant-02/04-equal-cost-split | 10.5 | 10.5 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 10.9 | 10.5 | n/a |
| variant-04/03-before-forces-detour-within-budget | 10.6 | 10.3 | n/a |
| variant-04/02-start-equals-end | 10.7 | 10.5 | n/a |
| variant-04/04-window-too-tight-unsat | 10.8 | 10.6 | n/a |
| variant-03/02-single-salesman | 10.6 | 10.5 | n/a |
| variant-04/07-layered-dag-combined | 13.0 | 11.0 | n/a |
| variant-02/02-single-salesman | 10.4 | 10.5 | n/a |
| variant-02/09-negative-weights | 10.4 | 10.5 | n/a |
| variant-03/06-unreachable-edge | 10.6 | 10.5 | n/a |
| n-queens/variant-03 | 11.9 | 10.3 | n/a |
| variant-02/06-unreachable-edge | 10.7 | 10.4 | n/a |
| variant-05/04-depot-and-vertex-revisits | 10.9 | 10.5 | n/a |
| variant-05/03-revisit-too-tight-unsat | 10.8 | 10.3 | n/a |
| variant-05/06-unreachable-edge | 11.2 | 10.8 | n/a |
| variant-02/06-layered-dag-before | 11.9 | 10.6 | n/a |
| variant-04/01-basic | 10.7 | 10.6 | n/a |
| variant-04/09-negative-weights | 10.8 | 10.6 | n/a |
| equality-generalized-tsp/02-larger | 11.1 | 10.5 | n/a |
| variant-05/05-three-depots-mixed | 11.3 | 10.6 | n/a |
| n-queens/variant-01 | 11.4 | 10.6 | n/a |
| variant-01/07-cycles | 10.9 | 10.6 | n/a |
| variant-04/06-unreachable-edge | 11.0 | 10.6 | n/a |
| variant-04/06-layered-dag-ordering-cap | 12.9 | 11.0 | n/a |
| variant-01/06-layered-dag | 13.0 | 10.4 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.0 | 10.5 | n/a |
| variant-02/07-layered-dag-before-after | 11.6 | 10.6 | n/a |
| variant-01/05-ring | 14.2 | 10.7 | n/a |
| variant-01/03-asymmetric | 12.1 | 10.7 | n/a |
| variant-02/05-larger-mix | 12.0 | 10.5 | n/a |
| n-queens/variant-06 | 11.9 | 10.6 | n/a |
| variant-03/06-layered-dag-cap | 14.2 | 10.9 | n/a |
| variant-03/07-layered-dag-tight-cap | 13.2 | 10.8 | n/a |
