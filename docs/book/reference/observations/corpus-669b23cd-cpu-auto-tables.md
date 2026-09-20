Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | before | after | after/before | before/reference | after/reference |
|---|---:|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 6.124 [5.821, 6.847] | 5.997 [5.809, 6.542] | 0.979 | 1.113 | 1.102 |
| equality-generalized-tsp/02-larger | 6.622 [6.020, 6.993] | 6.098 [5.899, 6.639] | 0.921 | 1.500 | 1.115 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.529 [4.728, 5.539] | 5.758 [5.617, 9.284] | 1.041 | 1.241 | 1.283 |
| variant-01/01-basic | 4.971 [4.837, 5.049] | 4.783 [4.408, 5.126] | 0.962 | 1.123 | 1.092 |
| variant-01/02-start-equals-end | 5.520 [5.486, 5.536] | 4.864 [4.438, 5.549] | 0.881 | 1.007 | 1.104 |
| variant-01/03-zero-cost-detour | 5.111 [4.991, 6.721] | 4.773 [4.652, 7.059] | 0.934 | 1.147 | 1.098 |
| variant-01/04-no-path | 5.267 [4.840, 6.258] | 5.109 [4.998, 5.423] | 0.970 | 0.966 | 1.154 |
| variant-01/05-multi-path | 5.061 [4.689, 6.472] | 5.529 [4.409, 6.551] | 1.093 | 0.925 | 0.850 |
| variant-01/06-layered-dag | 9.278 [6.759, 10.112] | 8.244 [7.609, 8.973] | 0.889 | 1.694 | 1.499 |
| variant-01/07-cycles | 5.851 [5.486, 6.089] | 6.345 [5.725, 6.463] | 1.085 | 1.079 | 1.454 |
| variant-01/08-negative-weights | 5.491 [5.471, 7.523] | 4.935 [4.859, 5.264] | 0.899 | 1.240 | 1.120 |
| variant-02/01-basic | 5.526 [4.992, 6.809] | 4.691 [4.463, 5.661] | 0.849 | 1.247 | 1.057 |
| variant-02/02-start-equals-end | 5.651 [4.405, 6.581] | 5.536 [4.561, 9.066] | 0.980 | 1.271 | 1.246 |
| variant-02/03-before-forces-detour | 5.163 [4.501, 5.878] | 5.713 [4.878, 6.842] | 1.107 | 1.157 | 1.273 |
| variant-02/04-after-forces-extension | 5.637 [5.476, 5.700] | 5.629 [5.284, 6.007] | 0.998 | 1.270 | 1.269 |
| variant-02/05-ordering-unsat | 4.664 [4.439, 5.731] | 4.621 [4.565, 5.516] | 0.991 | 1.057 | 1.044 |
| variant-02/06-layered-dag-before | 7.665 [6.630, 8.228] | 8.234 [7.638, 8.942] | 1.074 | 1.395 | 1.254 |
| variant-02/07-layered-dag-before-after | 9.167 [8.332, 9.844] | 8.259 [7.854, 9.865] | 0.901 | 1.627 | 1.258 |
| variant-02/08-tie-break-under-ordering | 5.656 [5.474, 8.995] | 5.696 [5.195, 6.702] | 1.007 | 1.233 | 1.041 |
| variant-02/09-negative-weights | 5.591 [5.562, 6.135] | 5.657 [5.651, 6.361] | 1.012 | 1.015 | 1.281 |
| variant-03/01-basic | 4.925 [4.826, 6.047] | 5.502 [5.085, 5.863] | 1.117 | 0.895 | 0.997 |
| variant-03/02-start-equals-end | 5.769 [4.373, 6.034] | 5.493 [5.434, 6.023] | 0.952 | 1.303 | 1.245 |
| variant-03/03-budget-forces-detour | 6.102 [4.422, 6.401] | 5.504 [5.496, 6.707] | 0.902 | 1.120 | 1.247 |
| variant-03/04-cost-at-cap-allowed | 5.498 [4.797, 5.798] | 5.024 [4.568, 6.318] | 0.914 | 1.002 | 1.132 |
| variant-03/05-budget-unsat | 5.979 [5.800, 7.029] | 5.690 [5.519, 5.768] | 0.952 | 1.090 | 1.294 |
| variant-03/06-layered-dag-cap | 10.838 [10.770, 11.544] | 13.215 [11.889, 18.445] | 1.219 | 1.412 | 1.736 |
| variant-03/07-layered-dag-tight-cap | 13.007 [12.371, 16.253] | 15.192 [9.347, 15.214] | 1.168 | 1.966 | 2.309 |
| variant-03/08-tie-break-under-cap | 6.309 [5.619, 6.320] | 5.921 [5.562, 7.322] | 0.938 | 1.147 | 1.078 |
| variant-03/09-negative-weights | 7.288 [5.807, 8.321] | 5.977 [5.550, 6.634] | 0.820 | 1.333 | 1.356 |
| variant-04/01-basic | 5.585 [5.495, 6.103] | 5.227 [4.990, 5.540] | 0.936 | 1.020 | 1.178 |
| variant-04/02-start-equals-end | 6.268 [5.210, 7.913] | 5.968 [4.659, 6.179] | 0.952 | 1.415 | 1.359 |
| variant-04/03-before-forces-detour-within-budget | 5.788 [5.665, 6.736] | 5.627 [5.488, 5.683] | 0.972 | 1.050 | 0.856 |
| variant-04/04-ordering-violates-budget | 5.918 [4.657, 6.606] | 5.528 [5.491, 5.903] | 0.934 | 1.325 | 1.248 |
| variant-04/05-after-and-budget-interact | 5.600 [5.523, 5.959] | 5.558 [5.528, 6.546] | 0.992 | 1.020 | 1.253 |
| variant-04/06-layered-dag-ordering-cap | 9.950 [9.040, 12.169] | 10.943 [10.477, 11.666] | 1.100 | 1.510 | 1.668 |
| variant-04/07-layered-dag-combined | 10.606 [9.701, 12.806] | 10.364 [8.778, 11.183] | 0.977 | 1.399 | 1.580 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.614 [5.768, 9.325] | 6.606 [6.138, 6.971] | 0.999 | 1.216 | 1.481 |
| variant-04/09-negative-weights | 6.845 [6.047, 7.216] | 6.075 [5.807, 6.920] | 0.888 | 1.544 | 1.360 |
| variant-01/01-basic | 4.729 [4.728, 5.014] | 4.806 [4.788, 7.944] | 1.016 | 1.065 | 1.090 |
| variant-01/02-agent-reuse | 4.654 [4.418, 6.883] | 4.731 [4.139, 4.977] | 1.016 | 1.058 | 1.083 |
| variant-01/03-selective-compatibility | 5.088 [4.747, 5.127] | 5.099 [3.981, 5.162] | 1.002 | 1.144 | 1.143 |
| variant-01/04-no-compatible-agent-unsat | 5.555 [4.767, 5.580] | 5.175 [4.484, 5.618] | 0.932 | 1.248 | 1.176 |
| variant-01/05-larger-mix | 6.098 [6.019, 7.160] | 6.160 [6.152, 6.864] | 1.010 | 1.386 | 1.390 |
| variant-02/01-basic | 6.009 [5.555, 7.727] | 4.979 [4.651, 5.564] | 0.829 | 1.089 | 0.912 |
| variant-02/02-makespan-tiebreak | 6.325 [5.500, 6.645] | 5.697 [4.863, 8.298] | 0.901 | 1.438 | 1.272 |
| variant-02/03-cost-dominates | 6.395 [5.925, 7.063] | 4.671 [4.431, 5.521] | 0.730 | 1.132 | 1.040 |
| variant-02/04-no-compatible-agent-unsat | 5.681 [4.531, 5.741] | 5.422 [4.933, 5.534] | 0.954 | 1.291 | 0.985 |
| variant-02/05-larger-mix | 9.202 [8.875, 9.398] | 9.437 [8.255, 10.448] | 1.026 | 1.681 | 1.728 |
| variant-03/01-basic | 6.054 [4.741, 6.614] | 5.059 [5.030, 7.846] | 0.836 | 1.364 | 0.933 |
| variant-03/02-multiple-groups | 6.219 [5.862, 7.873] | 4.864 [4.447, 6.492] | 0.782 | 1.413 | 0.888 |
| variant-03/03-mixed-grouped-ungrouped | 4.983 [4.742, 5.618] | 5.542 [4.895, 5.556] | 1.112 | 1.134 | 1.251 |
| variant-03/04-incompatible-group-unsat | 5.565 [5.093, 6.082] | 4.525 [4.502, 4.668] | 0.813 | 1.261 | 1.019 |
| variant-03/05-larger-mix | 5.736 [5.641, 6.327] | 5.857 [5.377, 6.588] | 1.021 | 1.302 | 1.323 |
| variant-04/01-basic | 9.792 [8.886, 10.354] | 9.248 [8.980, 12.157] | 0.944 | 0.366 | 0.361 |
| variant-04/02-precedence | 8.669 [8.082, 10.378] | 7.628 [7.168, 7.849] | 0.880 | 1.314 | 1.156 |
| variant-04/03-agent-serialization | 8.504 [7.905, 9.308] | 7.682 [7.371, 8.806] | 0.903 | 0.306 | 0.267 |
| variant-04/04-window-too-tight-unsat | 6.991 [5.663, 7.909] | 5.491 [5.076, 6.124] | 0.786 | 1.277 | 1.219 |
| variant-04/05-larger-mix | 33.913 [32.994, 34.878] | 34.522 [33.148, 35.640] | 1.018 | 0.225 | 0.227 |
| variant-01/01-basic | 5.813 [5.649, 6.041] | 5.863 [5.757, 5.876] | 1.009 | 1.247 | 1.051 |
| variant-01/02-multiple-tours | 6.078 [6.072, 6.777] | 6.075 [5.755, 6.757] | 1.000 | 1.315 | 1.306 |
| variant-01/03-asymmetric | 6.849 [6.645, 9.319] | 6.223 [6.137, 8.066] | 0.909 | 1.539 | 1.118 |
| variant-01/04-subtour-unsat | 6.039 [4.913, 8.240] | 4.985 [3.414, 5.121] | 0.825 | 1.356 | 1.113 |
| variant-01/05-ring | 8.811 [8.144, 11.870] | 8.700 [8.659, 9.760] | 0.987 | 1.346 | 1.574 |
| variant-02/01-basic | 6.021 [4.533, 6.125] | 6.646 [6.107, 8.847] | 1.104 | 1.360 | 1.205 |
| variant-02/02-single-salesman | 5.552 [5.503, 5.581] | 5.489 [5.346, 5.530] | 0.989 | 1.246 | 1.003 |
| variant-02/03-too-many-salesmen-unsat | 5.176 [4.520, 5.482] | 5.454 [4.776, 5.528] | 1.054 | 1.169 | 1.234 |
| variant-02/04-equal-cost-split | 5.523 [5.452, 6.239] | 5.707 [4.925, 6.413] | 1.033 | 1.013 | 1.290 |
| variant-02/05-larger-asymmetric | 5.713 [5.520, 6.338] | 6.514 [5.457, 6.811] | 1.140 | 1.291 | 1.452 |
| variant-02/06-unreachable-edge | 5.932 [5.597, 6.577] | 5.855 [4.981, 7.885] | 0.987 | 1.362 | 1.065 |
| variant-03/01-basic | 6.125 [6.005, 8.189] | 5.239 [4.545, 5.789] | 0.855 | 1.393 | 1.181 |
| variant-03/02-single-salesman | 5.686 [4.671, 5.911] | 5.612 [5.607, 5.917] | 0.987 | 1.280 | 1.233 |
| variant-03/03-depot-crossing-unsat | 5.972 [4.426, 8.839] | 5.206 [4.606, 6.692] | 0.872 | 1.351 | 1.194 |
| variant-03/04-equal-cost-split | 5.955 [5.761, 6.155] | 5.773 [5.682, 6.424] | 0.969 | 1.342 | 1.322 |
| variant-03/05-larger-three-depots | 6.107 [5.584, 7.645] | 7.123 [6.527, 8.682] | 1.166 | 1.386 | 1.528 |
| variant-03/06-unreachable-edge | 6.058 [5.477, 7.169] | 5.865 [5.531, 6.176] | 0.968 | 1.105 | 1.267 |
| variant-04/01-basic | 6.510 [6.291, 7.064] | 6.566 [5.464, 6.658] | 1.009 | 1.186 | 1.497 |
| variant-04/02-single-salesman | 6.779 [5.912, 6.949] | 6.590 [6.293, 7.315] | 0.972 | 1.237 | 1.200 |
| variant-04/03-window-too-tight-unsat | 7.136 [5.762, 9.008] | 5.472 [4.384, 9.432] | 0.767 | 1.621 | 1.000 |
| variant-04/04-depot-window-too-tight-unsat | 6.608 [6.278, 6.615] | 5.785 [5.693, 6.579] | 0.875 | 1.202 | 1.303 |
| variant-04/05-three-depots-asymmetric-times | 9.425 [8.193, 9.762] | 7.869 [7.825, 8.769] | 0.835 | 1.713 | 1.790 |
| variant-04/06-unreachable-edge | 9.263 [7.278, 10.216] | 7.674 [6.711, 8.745] | 0.828 | 1.708 | 1.708 |
| variant-05/01-basic | 6.855 [6.608, 7.196] | 6.557 [6.505, 6.643] | 0.957 | 1.555 | 1.481 |
| variant-05/02-tight-bound-exact | 6.246 [5.825, 6.247] | 7.063 [5.266, 8.470] | 1.131 | 1.133 | 1.285 |
| variant-05/03-revisit-too-tight-unsat | 5.617 [5.511, 6.607] | 5.808 [5.505, 6.046] | 1.034 | 1.019 | 1.326 |
| variant-05/04-depot-and-vertex-revisits | 6.589 [6.538, 6.671] | 5.479 [5.430, 6.234] | 0.832 | 1.481 | 1.243 |
| variant-05/05-three-depots-mixed | 8.062 [7.806, 8.744] | 9.194 [8.728, 9.241] | 1.140 | 1.817 | 1.670 |
| variant-05/06-unreachable-edge | 7.768 [7.701, 8.755] | 8.172 [6.775, 11.062] | 1.052 | 1.411 | 1.835 |
| n-queens/variant-01 | 10.092 [8.805, 10.108] | 8.683 [8.657, 9.535] | 0.860 | 1.528 | 1.526 |
| n-queens/variant-02 | 64.429 [62.819, 65.024] | 69.809 [69.066, 73.332] | 1.083 | 0.531 | 0.581 |
| n-queens/variant-03 | 9.308 [9.274, 9.541] | 9.513 [9.489, 9.761] | 1.022 | 1.412 | 1.447 |
| n-queens/variant-04 | 7.619 [6.543, 8.789] | 7.922 [7.788, 7.960] | 1.040 | 1.390 | 1.437 |
| n-queens/variant-05 | 8.785 [7.913, 8.910] | 8.883 [8.175, 9.273] | 1.011 | 1.333 | 1.351 |
| n-queens/variant-06 | 8.960 [8.106, 10.215] | 9.264 [8.749, 10.902] | 1.034 | 1.306 | 1.405 |
| send-money/send-money | 12.225 [12.086, 13.981] | 13.655 [13.459, 14.519] | 1.117 | 0.760 | 0.906 |

Reference wall time, ms, same notation.

| Cell | before | after |
|---|---:|---:|
| equality-generalized-tsp/01-basic | 5.500 [4.528, 5.517] | 5.440 [4.467, 6.600] |
| equality-generalized-tsp/02-larger | 4.414 [4.402, 4.449] | 5.467 [4.593, 5.540] |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.455 [4.403, 4.467] | 4.488 [4.481, 6.716] |
| variant-01/01-basic | 4.426 [4.406, 7.736] | 4.380 [4.356, 4.441] |
| variant-01/02-start-equals-end | 5.480 [4.642, 5.512] | 4.405 [3.280, 4.453] |
| variant-01/03-zero-cost-detour | 4.457 [4.398, 5.621] | 4.347 [3.299, 4.449] |
| variant-01/04-no-path | 5.455 [4.435, 5.512] | 4.429 [4.357, 4.448] |
| variant-01/05-multi-path | 5.472 [4.361, 8.004] | 6.505 [5.452, 6.675] |
| variant-01/06-layered-dag | 5.478 [5.469, 5.498] | 5.499 [5.469, 5.604] |
| variant-01/07-cycles | 5.421 [4.467, 5.538] | 4.365 [3.351, 5.411] |
| variant-01/08-negative-weights | 4.428 [3.324, 6.676] | 4.408 [3.390, 4.478] |
| variant-02/01-basic | 4.430 [4.414, 5.510] | 4.437 [4.348, 4.591] |
| variant-02/02-start-equals-end | 4.447 [4.441, 5.492] | 4.442 [3.384, 4.461] |
| variant-02/03-before-forces-detour | 4.464 [4.422, 7.290] | 4.487 [4.399, 5.480] |
| variant-02/04-after-forces-extension | 4.439 [4.434, 4.445] | 4.437 [4.381, 4.453] |
| variant-02/05-ordering-unsat | 4.414 [4.391, 4.432] | 4.428 [3.338, 5.665] |
| variant-02/06-layered-dag-before | 5.496 [5.472, 6.541] | 6.565 [5.534, 6.669] |
| variant-02/07-layered-dag-before-after | 5.634 [4.445, 6.610] | 6.565 [5.514, 7.583] |
| variant-02/08-tie-break-under-ordering | 4.587 [4.513, 5.492] | 5.473 [5.436, 5.549] |
| variant-02/09-negative-weights | 5.508 [4.435, 8.744] | 4.415 [3.322, 5.667] |
| variant-03/01-basic | 5.501 [4.477, 6.539] | 5.519 [4.336, 5.614] |
| variant-03/02-start-equals-end | 4.429 [3.315, 4.473] | 4.411 [4.406, 6.661] |
| variant-03/03-budget-forces-detour | 5.450 [4.540, 5.492] | 4.415 [4.397, 4.473] |
| variant-03/04-cost-at-cap-allowed | 5.490 [4.509, 5.526] | 4.439 [3.326, 5.458] |
| variant-03/05-budget-unsat | 5.483 [4.396, 5.576] | 4.396 [4.371, 5.462] |
| variant-03/06-layered-dag-cap | 7.677 [6.562, 7.683] | 7.613 [5.447, 12.006] |
| variant-03/07-layered-dag-tight-cap | 6.617 [6.494, 6.646] | 6.578 [5.502, 6.633] |
| variant-03/08-tie-break-under-cap | 5.503 [4.446, 5.508] | 5.491 [4.469, 5.491] |
| variant-03/09-negative-weights | 5.466 [4.445, 5.647] | 4.407 [4.380, 5.501] |
| variant-04/01-basic | 5.476 [4.386, 5.505] | 4.438 [4.365, 5.486] |
| variant-04/02-start-equals-end | 4.429 [3.269, 4.446] | 4.392 [4.350, 8.764] |
| variant-04/03-before-forces-detour-within-budget | 5.513 [5.487, 5.516] | 6.570 [4.447, 8.780] |
| variant-04/04-ordering-violates-budget | 4.465 [4.347, 4.540] | 4.430 [4.394, 5.478] |
| variant-04/05-after-and-budget-interact | 5.493 [4.433, 5.517] | 4.435 [4.421, 5.514] |
| variant-04/06-layered-dag-ordering-cap | 6.589 [6.500, 7.617] | 6.562 [6.550, 8.758] |
| variant-04/07-layered-dag-combined | 7.581 [6.634, 7.594] | 6.561 [6.534, 6.575] |
| variant-04/08-tie-break-under-ordering-and-cap | 5.438 [4.434, 5.511] | 4.461 [4.460, 5.510] |
| variant-04/09-negative-weights | 4.434 [4.400, 5.513] | 4.467 [4.436, 5.476] |
| variant-01/01-basic | 4.440 [4.396, 4.458] | 4.407 [4.357, 4.456] |
| variant-01/02-agent-reuse | 4.397 [3.313, 5.495] | 4.367 [3.362, 5.448] |
| variant-01/03-selective-compatibility | 4.448 [4.404, 5.455] | 4.463 [4.385, 4.478] |
| variant-01/04-no-compatible-agent-unsat | 4.451 [4.387, 5.466] | 4.401 [4.394, 4.495] |
| variant-01/05-larger-mix | 4.399 [3.299, 4.399] | 4.431 [4.399, 4.582] |
| variant-02/01-basic | 5.518 [4.376, 5.525] | 5.458 [4.453, 5.608] |
| variant-02/02-makespan-tiebreak | 4.399 [4.364, 4.443] | 4.479 [4.381, 4.546] |
| variant-02/03-cost-dominates | 5.648 [5.457, 6.646] | 4.493 [4.366, 5.518] |
| variant-02/04-no-compatible-agent-unsat | 4.401 [3.302, 5.554] | 5.502 [4.395, 5.523] |
| variant-02/05-larger-mix | 5.474 [4.443, 6.576] | 5.461 [5.458, 5.517] |
| variant-03/01-basic | 4.438 [3.303, 4.521] | 5.424 [4.344, 5.523] |
| variant-03/02-multiple-groups | 4.401 [4.397, 5.457] | 5.477 [4.394, 5.523] |
| variant-03/03-mixed-grouped-ungrouped | 4.394 [4.386, 4.397] | 4.431 [4.390, 4.433] |
| variant-03/04-incompatible-group-unsat | 4.412 [4.407, 4.447] | 4.443 [3.322, 5.498] |
| variant-03/05-larger-mix | 4.406 [3.323, 4.441] | 4.428 [4.424, 5.475] |
| variant-04/01-basic | 26.727 [25.627, 29.226] | 25.635 [25.630, 30.198] |
| variant-04/02-precedence | 6.596 [6.507, 7.893] | 6.598 [5.463, 10.027] |
| variant-04/03-agent-serialization | 27.760 [26.656, 30.244] | 28.816 [27.723, 29.168] |
| variant-04/04-window-too-tight-unsat | 5.475 [4.490, 5.600] | 4.503 [4.474, 5.622] |
| variant-04/05-larger-mix | 150.903 [147.256, 152.359] | 151.841 [148.683, 152.797] |
| variant-01/01-basic | 4.661 [4.534, 5.572] | 5.578 [4.662, 6.928] |
| variant-01/02-multiple-tours | 4.622 [4.409, 4.677] | 4.653 [4.563, 5.760] |
| variant-01/03-asymmetric | 4.451 [4.435, 4.619] | 5.566 [4.643, 7.013] |
| variant-01/04-subtour-unsat | 4.455 [3.346, 5.452] | 4.479 [4.392, 5.586] |
| variant-01/05-ring | 6.547 [5.454, 6.597] | 5.528 [5.494, 6.567] |
| variant-02/01-basic | 4.427 [4.380, 5.464] | 5.514 [5.487, 5.533] |
| variant-02/02-single-salesman | 4.457 [4.440, 4.474] | 5.469 [4.430, 5.513] |
| variant-02/03-too-many-salesmen-unsat | 4.427 [4.380, 5.613] | 4.421 [3.338, 5.482] |
| variant-02/04-equal-cost-split | 5.453 [4.399, 5.494] | 4.425 [4.346, 4.440] |
| variant-02/05-larger-asymmetric | 4.425 [4.399, 5.463] | 4.485 [4.407, 5.472] |
| variant-02/06-unreachable-edge | 4.354 [3.337, 4.376] | 5.500 [4.417, 5.517] |
| variant-03/01-basic | 4.397 [4.360, 4.475] | 4.434 [4.430, 8.774] |
| variant-03/02-single-salesman | 4.443 [4.389, 5.468] | 4.553 [4.365, 5.479] |
| variant-03/03-depot-crossing-unsat | 4.422 [4.420, 4.458] | 4.361 [3.317, 4.428] |
| variant-03/04-equal-cost-split | 4.437 [4.430, 4.450] | 4.368 [3.285, 4.400] |
| variant-03/05-larger-three-depots | 4.407 [4.345, 5.515] | 4.663 [4.432, 5.522] |
| variant-03/06-unreachable-edge | 5.482 [4.447, 5.485] | 4.627 [4.434, 5.481] |
| variant-04/01-basic | 5.488 [4.349, 9.911] | 4.387 [4.386, 5.459] |
| variant-04/02-single-salesman | 5.481 [5.454, 5.562] | 5.492 [4.424, 5.494] |
| variant-04/03-window-too-tight-unsat | 4.401 [4.388, 5.480] | 5.471 [4.432, 5.475] |
| variant-04/04-depot-window-too-tight-unsat | 5.497 [4.444, 5.501] | 4.438 [4.431, 5.469] |
| variant-04/05-three-depots-asymmetric-times | 5.502 [4.687, 6.547] | 4.396 [4.378, 6.538] |
| variant-04/06-unreachable-edge | 5.424 [4.438, 7.683] | 4.492 [4.460, 5.467] |
| variant-05/01-basic | 4.410 [4.406, 5.484] | 4.429 [4.399, 6.561] |
| variant-05/02-tight-bound-exact | 5.513 [4.444, 6.540] | 5.495 [4.435, 5.501] |
| variant-05/03-revisit-too-tight-unsat | 5.510 [4.398, 5.527] | 4.381 [4.357, 4.384] |
| variant-05/04-depot-and-vertex-revisits | 4.448 [3.296, 8.806] | 4.408 [4.399, 5.441] |
| variant-05/05-three-depots-mixed | 4.437 [4.435, 5.477] | 5.506 [4.414, 5.533] |
| variant-05/06-unreachable-edge | 5.503 [5.486, 5.573] | 4.455 [4.432, 5.525] |
| n-queens/variant-01 | 6.604 [5.530, 7.771] | 5.690 [5.510, 6.591] |
| n-queens/variant-02 | 121.229 [121.145, 124.689] | 120.232 [119.120, 120.443] |
| n-queens/variant-03 | 6.591 [6.575, 8.053] | 6.573 [5.701, 9.175] |
| n-queens/variant-04 | 5.480 [5.462, 6.636] | 5.513 [5.495, 5.621] |
| n-queens/variant-05 | 6.589 [5.491, 7.600] | 6.575 [5.561, 7.713] |
| n-queens/variant-06 | 6.862 [6.570, 8.798] | 6.594 [5.522, 6.598] |
| send-money/send-money | 16.090 [15.038, 18.333] | 15.065 [13.967, 16.133] |

Counters of report after: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 0 | 1 | 4 | 9336 | 1.989 |
| equality-generalized-tsp/02-larger | 0 | 1 | 8 | 23650 | 2.455 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 0 | 0 | 0 | 2258 | 2.364 |
| variant-01/01-basic | 0 | 1 | 2 | 1559 | 1.356 |
| variant-01/02-start-equals-end | 0 | 1 | 1 | 1208 | 1.318 |
| variant-01/03-zero-cost-detour | 0 | 1 | 2 | 1291 | 1.286 |
| variant-01/04-no-path | 0 | 0 | 0 | 1085 | 1.412 |
| variant-01/05-multi-path | 0 | 1 | 3 | 3958 | 1.700 |
| variant-01/06-layered-dag | 0 | 1 | 16 | 104316 | 4.805 |
| variant-01/07-cycles | 0 | 1 | 5 | 14519 | 2.247 |
| variant-01/08-negative-weights | 0 | 1 | 1 | 3191 | 1.811 |
| variant-02/01-basic | 0 | 1 | 2 | 1700 | 1.473 |
| variant-02/02-start-equals-end | 0 | 1 | 1 | 1557 | 1.570 |
| variant-02/03-before-forces-detour | 0 | 1 | 1 | 2383 | 1.795 |
| variant-02/04-after-forces-extension | 0 | 1 | 1 | 2245 | 1.833 |
| variant-02/05-ordering-unsat | 0 | 0 | 0 | 1322 | 1.438 |
| variant-02/06-layered-dag-before | 0 | 2 | 9 | 57648 | 4.350 |
| variant-02/07-layered-dag-before-after | 0 | 1 | 7 | 47034 | 4.320 |
| variant-02/08-tie-break-under-ordering | 0 | 1 | 2 | 2777 | 1.860 |
| variant-02/09-negative-weights | 0 | 1 | 1 | 3573 | 1.950 |
| variant-03/01-basic | 0 | 1 | 2 | 1667 | 1.667 |
| variant-03/02-start-equals-end | 0 | 1 | 1 | 1467 | 1.604 |
| variant-03/03-budget-forces-detour | 0 | 1 | 1 | 2141 | 2.216 |
| variant-03/04-cost-at-cap-allowed | 0 | 1 | 1 | 1272 | 1.436 |
| variant-03/05-budget-unsat | 0 | 0 | 0 | 4970 | 1.934 |
| variant-03/06-layered-dag-cap | 0 | 2 | 13 | 197685 | 9.079 |
| variant-03/07-layered-dag-tight-cap | 0 | 4 | 4 | 314736 | 10.225 |
| variant-03/08-tie-break-under-cap | 0 | 1 | 2 | 3301 | 1.886 |
| variant-03/09-negative-weights | 0 | 1 | 1 | 3627 | 2.524 |
| variant-04/01-basic | 0 | 1 | 2 | 1808 | 1.754 |
| variant-04/02-start-equals-end | 0 | 1 | 1 | 1816 | 1.986 |
| variant-04/03-before-forces-detour-within-budget | 0 | 1 | 1 | 2774 | 1.886 |
| variant-04/04-ordering-violates-budget | 0 | 0 | 0 | 2816 | 1.825 |
| variant-04/05-after-and-budget-interact | 0 | 1 | 1 | 4169 | 2.058 |
| variant-04/06-layered-dag-ordering-cap | 0 | 2 | 8 | 73286 | 6.612 |
| variant-04/07-layered-dag-combined | 0 | 1 | 5 | 63717 | 6.398 |
| variant-04/08-tie-break-under-ordering-and-cap | 0 | 1 | 2 | 6554 | 2.359 |
| variant-04/09-negative-weights | 0 | 1 | 1 | 4009 | 2.383 |
| variant-01/01-basic | 0 | 1 | 4 | 1462 | 1.777 |
| variant-01/02-agent-reuse | 0 | 1 | 8 | 2524 | 1.276 |
| variant-01/03-selective-compatibility | 0 | 1 | 4 | 1873 | 1.220 |
| variant-01/04-no-compatible-agent-unsat | 0 | 0 | 0 | 1217 | 1.362 |
| variant-01/05-larger-mix | 0 | 1 | 35 | 39244 | 2.497 |
| variant-02/01-basic | 0 | 1 | 4 | 3047 | 1.524 |
| variant-02/02-makespan-tiebreak | 0 | 3 | 8 | 4541 | 1.832 |
| variant-02/03-cost-dominates | 0 | 1 | 8 | 6082 | 1.701 |
| variant-02/04-no-compatible-agent-unsat | 0 | 0 | 0 | 2395 | 1.510 |
| variant-02/05-larger-mix | 0 | 1 | 45 | 139060 | 4.849 |
| variant-03/01-basic | 0 | 2 | 2 | 1970 | 1.326 |
| variant-03/02-multiple-groups | 0 | 4 | 4 | 3816 | 1.423 |
| variant-03/03-mixed-grouped-ungrouped | 0 | 2 | 4 | 2639 | 1.552 |
| variant-03/04-incompatible-group-unsat | 0 | 0 | 0 | 1224 | 1.051 |
| variant-03/05-larger-mix | 0 | 1 | 27 | 5729 | 2.110 |
| variant-04/01-basic | 0 | 81 | 89 | 90588 | 5.357 |
| variant-04/02-precedence | 0 | 3 | 9 | 14300 | 3.314 |
| variant-04/03-agent-serialization | 0 | 30 | 30 | 33075 | 4.086 |
| variant-04/04-window-too-tight-unsat | 0 | 0 | 0 | 2865 | 2.197 |
| variant-04/05-larger-mix | 0 | 1176 | 1178 | 2250216 | 30.420 |
| variant-01/01-basic | 0 | 1 | 2 | 5304 | 1.874 |
| variant-01/02-multiple-tours | 0 | 2 | 9 | 36978 | 2.518 |
| variant-01/03-asymmetric | 0 | 1 | 9 | 58489 | 3.024 |
| variant-01/04-subtour-unsat | 0 | 0 | 0 | 2473 | 1.195 |
| variant-01/05-ring | 0 | 2 | 31 | 341550 | 5.141 |
| variant-02/01-basic | 0 | 2 | 2 | 5859 | 2.842 |
| variant-02/02-single-salesman | 0 | 1 | 2 | 3958 | 1.738 |
| variant-02/03-too-many-salesmen-unsat | 0 | 0 | 0 | 3466 | 1.900 |
| variant-02/04-equal-cost-split | 0 | 2 | 2 | 4769 | 1.798 |
| variant-02/05-larger-asymmetric | 0 | 2 | 2 | 6981 | 2.308 |
| variant-02/06-unreachable-edge | 0 | 2 | 2 | 8166 | 2.222 |
| variant-03/01-basic | 0 | 1 | 1 | 3939 | 1.734 |
| variant-03/02-single-salesman | 0 | 1 | 2 | 3808 | 1.875 |
| variant-03/03-depot-crossing-unsat | 0 | 0 | 0 | 3422 | 1.638 |
| variant-03/04-equal-cost-split | 0 | 2 | 2 | 8166 | 2.185 |
| variant-03/05-larger-three-depots | 0 | 1 | 1 | 10821 | 3.237 |
| variant-03/06-unreachable-edge | 0 | 1 | 1 | 6365 | 2.189 |
| variant-04/01-basic | 0 | 1 | 1 | 6955 | 2.742 |
| variant-04/02-single-salesman | 0 | 1 | 1 | 4700 | 2.353 |
| variant-04/03-window-too-tight-unsat | 0 | 0 | 0 | 3400 | 1.976 |
| variant-04/04-depot-window-too-tight-unsat | 0 | 0 | 0 | 2325 | 2.072 |
| variant-04/05-three-depots-asymmetric-times | 0 | 1 | 1 | 18328 | 4.260 |
| variant-04/06-unreachable-edge | 0 | 1 | 1 | 14062 | 3.700 |
| variant-05/01-basic | 0 | 1 | 1 | 6957 | 2.649 |
| variant-05/02-tight-bound-exact | 0 | 1 | 1 | 3439 | 2.217 |
| variant-05/03-revisit-too-tight-unsat | 0 | 0 | 0 | 3413 | 2.107 |
| variant-05/04-depot-and-vertex-revisits | 0 | 1 | 1 | 3504 | 2.302 |
| variant-05/05-three-depots-mixed | 0 | 1 | 1 | 18378 | 4.943 |
| variant-05/06-unreachable-edge | 0 | 1 | 1 | 14062 | 3.951 |
| n-queens/variant-01 | 0 | 92 | 92 | 199032 | 5.115 |
| n-queens/variant-02 | 0 | 92 | 92 | 14461401 | 65.173 |
| n-queens/variant-03 | 0 | 92 | 92 | 256834 | 5.526 |
| n-queens/variant-04 | 0 | 92 | 92 | 195309 | 4.002 |
| n-queens/variant-05 | 0 | 92 | 92 | 223549 | 5.153 |
| n-queens/variant-06 | 0 | 92 | 92 | 251111 | 5.311 |
| send-money/send-money | 0 | 1 | 1 | 409024 | 9.262 |

Against the reference: report before, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 8 of 94 cells where both passed (8.5%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 33.913 | 150.903 | 0.225 | 3.281 | 32.802 | 12.673 | 86.000 | 59.000 |
| variant-04/03-agent-serialization | 8.504 | 27.760 | 0.306 | 0.898 | 1.551 | 0.332 | 23.000 | 0.000 |
| variant-04/01-basic | 9.792 | 26.727 | 0.366 | 0.991 | 3.208 | 0.732 | 17.000 | 5.000 |
| n-queens/variant-02 | 64.429 | 121.229 | 0.531 | 2.482 | 178.456 | 1.282 | 1.000 | 117.000 |
| send-money/send-money | 12.225 | 16.090 | 0.760 | 2.536 | 6.807 | 0.294 | 10.000 | 1.000 |
| variant-03/01-basic | 4.925 | 5.501 | 0.895 | 0.228 | 0.057 | 0.013 | 1.000 | 0.000 |
| variant-01/05-multi-path | 5.061 | 5.472 | 0.925 | 0.272 | 0.163 | 0.030 | 1.000 | 0.000 |
| variant-01/04-no-path | 5.267 | 5.455 | 0.966 | 0.178 | 0.044 | 0.006 | 1.000 | 0.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-03/04-cost-at-cap-allowed | 5.498 | 5.490 | 1.002 | 0.190 | 0.044 | 0.008 | 1.000 | 0.000 |
| variant-01/02-start-equals-end | 5.520 | 5.480 | 1.007 | 0.178 | 0.042 | 0.027 | 1.000 | 0.000 |
| variant-02/04-equal-cost-split | 5.523 | 5.453 | 1.013 | 0.327 | 0.136 | 0.100 | 1.000 | 0.000 |
| variant-02/09-negative-weights | 5.591 | 5.508 | 1.015 | 0.254 | 0.083 | 0.049 | 1.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 5.617 | 5.510 | 1.019 | 0.393 | 0.054 | 0.011 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 5.600 | 5.493 | 1.020 | 0.324 | 0.093 | 0.021 | 1.000 | 0.000 |
| variant-04/01-basic | 5.585 | 5.476 | 1.020 | 0.202 | 0.076 | 0.018 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 5.788 | 5.513 | 1.050 | 0.284 | 0.073 | 0.014 | 1.000 | 0.000 |
| variant-02/05-ordering-unsat | 4.664 | 4.414 | 1.057 | 0.187 | 0.044 | 0.006 | 1.000 | 0.000 |
| variant-01/02-agent-reuse | 4.654 | 4.397 | 1.058 | 0.214 | 0.124 | 0.030 | 0.000 | 0.000 |
| variant-01/01-basic | 4.729 | 4.440 | 1.065 | 0.149 | 0.076 | 0.021 | 0.000 | 0.000 |
| variant-01/07-cycles | 5.851 | 5.421 | 1.079 | 0.371 | 0.274 | 0.376 | 1.000 | 0.000 |
| variant-02/01-basic | 6.009 | 5.518 | 1.089 | 0.461 | 0.162 | 0.034 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 5.979 | 5.483 | 1.090 | 0.366 | 0.133 | 0.013 | 1.000 | 0.000 |
| variant-03/06-unreachable-edge | 6.058 | 5.482 | 1.105 | 0.517 | 0.123 | 0.089 | 1.000 | 0.000 |
| equality-generalized-tsp/01-basic | 6.124 | 5.500 | 1.113 | 0.374 | 0.204 | 0.237 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 6.102 | 5.450 | 1.120 | 0.229 | 0.068 | 0.012 | 1.000 | 0.000 |
| variant-01/01-basic | 4.971 | 4.426 | 1.123 | 0.182 | 0.068 | 0.013 | 1.000 | 0.000 |
| variant-02/03-cost-dominates | 6.395 | 5.648 | 1.132 | 0.470 | 0.269 | 0.060 | 1.000 | 0.000 |
| variant-05/02-tight-bound-exact | 6.246 | 5.513 | 1.133 | 0.386 | 0.078 | 0.050 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 4.983 | 4.394 | 1.134 | 0.186 | 0.119 | 0.026 | 0.000 | 0.000 |
| variant-01/03-selective-compatibility | 5.088 | 4.448 | 1.144 | 0.341 | 0.073 | 0.021 | 0.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 6.309 | 5.503 | 1.147 | 0.314 | 0.106 | 0.024 | 1.000 | 0.000 |
| variant-01/03-zero-cost-detour | 5.111 | 4.457 | 1.147 | 0.174 | 0.059 | 0.012 | 1.000 | 0.000 |
| variant-02/03-before-forces-detour | 5.163 | 4.464 | 1.157 | 0.227 | 0.069 | 0.014 | 1.000 | 0.000 |
| variant-02/03-too-many-salesmen-unsat | 5.176 | 4.427 | 1.169 | 0.292 | 0.061 | 0.010 | 1.000 | 0.000 |
| variant-04/01-basic | 6.510 | 5.488 | 1.186 | 0.605 | 0.127 | 0.085 | 1.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 6.608 | 5.497 | 1.202 | 0.305 | 0.051 | 0.010 | 1.000 | 0.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.614 | 5.438 | 1.216 | 0.450 | 0.175 | 0.039 | 1.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 5.656 | 4.587 | 1.233 | 0.265 | 0.115 | 0.020 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 6.779 | 5.481 | 1.237 | 0.463 | 0.084 | 0.057 | 1.000 | 0.000 |
| variant-01/08-negative-weights | 5.491 | 4.428 | 1.240 | 0.255 | 0.078 | 0.043 | 1.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.529 | 4.455 | 1.241 | 0.191 | 0.043 | 0.010 | 0.000 | 0.000 |
| variant-02/02-single-salesman | 5.552 | 4.457 | 1.246 | 0.267 | 0.101 | 0.090 | 1.000 | 0.000 |
| variant-01/01-basic | 5.813 | 4.661 | 1.247 | 0.275 | 0.156 | 0.120 | 1.000 | 0.000 |
| variant-02/01-basic | 5.526 | 4.430 | 1.247 | 0.175 | 0.068 | 0.014 | 1.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 5.555 | 4.451 | 1.248 | 0.123 | 0.032 | 0.005 | 0.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 5.565 | 4.412 | 1.261 | 0.134 | 0.045 | 0.005 | 0.000 | 0.000 |
| variant-02/04-after-forces-extension | 5.637 | 4.439 | 1.270 | 0.235 | 0.064 | 0.013 | 1.000 | 0.000 |
| variant-02/02-start-equals-end | 5.651 | 4.447 | 1.271 | 0.186 | 0.057 | 0.030 | 1.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 6.991 | 5.475 | 1.277 | 0.321 | 0.084 | 0.011 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 5.686 | 4.443 | 1.280 | 0.247 | 0.092 | 0.078 | 1.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 5.681 | 4.401 | 1.291 | 0.284 | 0.046 | 0.008 | 0.000 | 0.000 |
| variant-02/05-larger-asymmetric | 5.713 | 4.425 | 1.291 | 0.443 | 0.151 | 0.145 | 1.000 | 0.000 |
| variant-03/05-larger-mix | 5.736 | 4.406 | 1.302 | 0.272 | 0.609 | 0.101 | 0.000 | 0.000 |
| variant-03/02-start-equals-end | 5.769 | 4.429 | 1.303 | 0.163 | 0.043 | 0.027 | 1.000 | 0.000 |
| n-queens/variant-06 | 8.960 | 6.862 | 1.306 | 1.060 | 5.682 | 0.969 | 1.000 | 1.000 |
| variant-04/02-precedence | 8.669 | 6.596 | 1.314 | 0.635 | 0.604 | 0.113 | 2.000 | 0.000 |
| variant-01/02-multiple-tours | 6.078 | 4.622 | 1.315 | 0.390 | 0.510 | 1.041 | 1.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 5.918 | 4.465 | 1.325 | 0.300 | 0.068 | 0.010 | 1.000 | 0.000 |
| n-queens/variant-05 | 8.785 | 6.589 | 1.333 | 1.019 | 4.790 | 0.859 | 1.000 | 1.000 |
| variant-03/09-negative-weights | 7.288 | 5.466 | 1.333 | 0.302 | 0.085 | 0.049 | 1.000 | 0.000 |
| variant-03/04-equal-cost-split | 5.955 | 4.437 | 1.342 | 0.456 | 0.182 | 0.161 | 1.000 | 0.000 |
| variant-01/05-ring | 8.811 | 6.547 | 1.346 | 0.717 | 2.800 | 1.553 | 1.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 5.972 | 4.422 | 1.351 | 0.446 | 0.068 | 0.013 | 1.000 | 0.000 |
| variant-01/04-subtour-unsat | 6.039 | 4.455 | 1.356 | 0.210 | 0.041 | 0.009 | 0.000 | 0.000 |
| variant-02/01-basic | 6.021 | 4.427 | 1.360 | 0.381 | 0.142 | 0.123 | 1.000 | 0.000 |
| variant-02/06-unreachable-edge | 5.932 | 4.354 | 1.362 | 0.479 | 0.162 | 0.128 | 1.000 | 0.000 |
| variant-03/01-basic | 6.054 | 4.438 | 1.364 | 0.304 | 0.079 | 0.016 | 0.000 | 0.000 |
| variant-03/05-larger-three-depots | 6.107 | 4.407 | 1.386 | 0.836 | 0.168 | 0.122 | 1.000 | 0.000 |
| variant-01/05-larger-mix | 6.098 | 4.399 | 1.386 | 0.258 | 1.631 | 0.144 | 0.000 | 0.000 |
| n-queens/variant-04 | 7.619 | 5.480 | 1.390 | 0.960 | 4.167 | 0.616 | 1.000 | 1.000 |
| variant-03/01-basic | 6.125 | 4.397 | 1.393 | 0.365 | 0.124 | 0.063 | 1.000 | 0.000 |
| variant-02/06-layered-dag-before | 7.665 | 5.496 | 1.395 | 1.071 | 1.788 | 0.289 | 2.000 | 0.000 |
| variant-04/07-layered-dag-combined | 10.606 | 7.581 | 1.399 | 1.816 | 2.223 | 0.347 | 3.000 | 0.000 |
| variant-05/06-unreachable-edge | 7.768 | 5.503 | 1.411 | 1.207 | 0.212 | 0.167 | 1.000 | 0.000 |
| variant-03/06-layered-dag-cap | 10.838 | 7.677 | 1.412 | 1.670 | 5.477 | 0.525 | 3.000 | 0.000 |
| n-queens/variant-03 | 9.308 | 6.591 | 1.412 | 1.832 | 4.700 | 0.825 | 1.000 | 1.000 |
| variant-03/02-multiple-groups | 6.219 | 4.401 | 1.413 | 0.475 | 0.112 | 0.027 | 1.000 | 0.000 |
| variant-04/02-start-equals-end | 6.268 | 4.429 | 1.415 | 0.223 | 0.049 | 0.030 | 1.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 6.325 | 4.399 | 1.438 | 0.351 | 0.243 | 0.062 | 1.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 6.589 | 4.448 | 1.481 | 0.384 | 0.084 | 0.048 | 1.000 | 0.000 |
| equality-generalized-tsp/02-larger | 6.622 | 4.414 | 1.500 | 0.622 | 0.395 | 0.579 | 1.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 9.950 | 6.589 | 1.510 | 1.775 | 2.365 | 0.388 | 2.000 | 0.000 |
| n-queens/variant-01 | 10.092 | 6.604 | 1.528 | 1.931 | 4.415 | 0.775 | 1.000 | 1.000 |
| variant-01/03-asymmetric | 6.849 | 4.451 | 1.539 | 0.552 | 0.656 | 0.966 | 1.000 | 0.000 |
| variant-04/09-negative-weights | 6.845 | 4.434 | 1.544 | 0.309 | 0.092 | 0.054 | 1.000 | 0.000 |
| variant-05/01-basic | 6.855 | 4.410 | 1.555 | 0.601 | 0.119 | 0.082 | 1.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 7.136 | 4.401 | 1.621 | 0.407 | 0.093 | 0.019 | 1.000 | 0.000 |
| variant-02/07-layered-dag-before-after | 9.167 | 5.634 | 1.627 | 1.146 | 1.352 | 0.260 | 2.000 | 0.000 |
| variant-02/05-larger-mix | 9.202 | 5.474 | 1.681 | 0.732 | 5.401 | 0.181 | 1.000 | 0.000 |
| variant-01/06-layered-dag | 9.278 | 5.478 | 1.694 | 1.025 | 3.965 | 0.297 | 1.000 | 0.000 |
| variant-04/06-unreachable-edge | 9.263 | 5.424 | 1.708 | 1.209 | 0.212 | 0.165 | 1.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 9.425 | 5.502 | 1.713 | 1.723 | 0.246 | 0.190 | 1.000 | 0.000 |
| variant-05/05-three-depots-mixed | 8.062 | 4.437 | 1.817 | 1.538 | 0.249 | 0.201 | 1.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 13.007 | 6.617 | 1.966 | 1.692 | 7.938 | 0.292 | 2.000 | 1.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 15.6 | 23.6 | n/a |
| variant-04/03-agent-serialization | 11.6 | 12.4 | n/a |
| variant-04/01-basic | 11.7 | 13.3 | n/a |
| n-queens/variant-02 | 16.6 | 12.7 | n/a |
| send-money/send-money | 14.3 | 12.9 | n/a |
| variant-03/01-basic | 10.7 | 10.3 | n/a |
| variant-01/05-multi-path | 10.7 | 10.5 | n/a |
| variant-01/04-no-path | 10.6 | 10.4 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.9 | 10.4 | n/a |
| variant-01/02-start-equals-end | 10.4 | 10.4 | n/a |
| variant-02/04-equal-cost-split | 10.7 | 10.5 | n/a |
| variant-02/09-negative-weights | 10.7 | 10.5 | n/a |
| variant-05/03-revisit-too-tight-unsat | 10.9 | 10.5 | n/a |
| variant-04/05-after-and-budget-interact | 11.2 | 10.5 | n/a |
| variant-04/01-basic | 10.8 | 10.4 | n/a |
| variant-04/03-before-forces-detour-within-budget | 10.9 | 10.5 | n/a |
| variant-02/05-ordering-unsat | 10.8 | 10.4 | n/a |
| variant-01/02-agent-reuse | 10.4 | 10.3 | n/a |
| variant-01/01-basic | 10.2 | 10.3 | n/a |
| variant-01/07-cycles | 10.8 | 10.6 | n/a |
| variant-02/01-basic | 10.5 | 10.4 | n/a |
| variant-03/05-budget-unsat | 10.6 | 10.6 | n/a |
| variant-03/06-unreachable-edge | 10.8 | 10.5 | n/a |
| equality-generalized-tsp/01-basic | 10.7 | 10.5 | n/a |
| variant-03/03-budget-forces-detour | 10.8 | 10.5 | n/a |
| variant-01/01-basic | 10.6 | 10.5 | n/a |
| variant-02/03-cost-dominates | 10.7 | 10.3 | n/a |
| variant-05/02-tight-bound-exact | 11.0 | 10.6 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.7 | 10.4 | n/a |
| variant-01/03-selective-compatibility | 10.3 | 10.4 | n/a |
| variant-03/08-tie-break-under-cap | 10.8 | 10.4 | n/a |
| variant-01/03-zero-cost-detour | 10.7 | 10.2 | n/a |
| variant-02/03-before-forces-detour | 10.6 | 10.6 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.5 | 10.4 | n/a |
| variant-04/01-basic | 11.3 | 10.3 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 10.6 | 10.5 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 10.8 | 10.4 | n/a |
| variant-02/08-tie-break-under-ordering | 10.3 | 10.5 | n/a |
| variant-04/02-single-salesman | 11.0 | 10.6 | n/a |
| variant-01/08-negative-weights | 10.7 | 10.2 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.8 | 10.5 | n/a |
| variant-02/02-single-salesman | 10.8 | 10.5 | n/a |
| variant-01/01-basic | 10.5 | 10.2 | n/a |
| variant-02/01-basic | 10.5 | 10.5 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.5 | 10.4 | n/a |
| variant-03/04-incompatible-group-unsat | 10.6 | 10.4 | n/a |
| variant-02/04-after-forces-extension | 10.7 | 10.6 | n/a |
| variant-02/02-start-equals-end | 10.7 | 10.4 | n/a |
| variant-04/04-window-too-tight-unsat | 11.3 | 10.6 | n/a |
| variant-03/02-single-salesman | 10.8 | 10.5 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.6 | 10.1 | n/a |
| variant-02/05-larger-asymmetric | 10.6 | 10.4 | n/a |
| variant-03/05-larger-mix | 10.8 | 10.4 | n/a |
| variant-03/02-start-equals-end | 10.7 | 10.5 | n/a |
| n-queens/variant-06 | 11.8 | 10.6 | n/a |
| variant-04/02-precedence | 11.3 | 10.6 | n/a |
| variant-01/02-multiple-tours | 11.3 | 10.5 | n/a |
| variant-04/04-ordering-violates-budget | 10.7 | 10.6 | n/a |
| n-queens/variant-05 | 11.8 | 10.6 | n/a |
| variant-03/09-negative-weights | 10.7 | 10.5 | n/a |
| variant-03/04-equal-cost-split | 10.9 | 10.6 | n/a |
| variant-01/05-ring | 13.6 | 10.7 | n/a |
| variant-03/03-depot-crossing-unsat | 10.4 | 10.3 | n/a |
| variant-01/04-subtour-unsat | 10.3 | 10.4 | n/a |
| variant-02/01-basic | 10.8 | 10.6 | n/a |
| variant-02/06-unreachable-edge | 10.8 | 10.6 | n/a |
| variant-03/01-basic | 10.6 | 10.4 | n/a |
| variant-03/05-larger-three-depots | 10.7 | 10.6 | n/a |
| variant-01/05-larger-mix | 10.7 | 10.1 | n/a |
| n-queens/variant-04 | 11.0 | 10.6 | n/a |
| variant-03/01-basic | 10.6 | 10.5 | n/a |
| variant-02/06-layered-dag-before | 11.8 | 10.8 | n/a |
| variant-04/07-layered-dag-combined | 13.0 | 11.1 | n/a |
| variant-05/06-unreachable-edge | 11.2 | 10.5 | n/a |
| variant-03/06-layered-dag-cap | 15.2 | 10.9 | n/a |
| n-queens/variant-03 | 11.7 | 10.6 | n/a |
| variant-03/02-multiple-groups | 10.6 | 10.0 | n/a |
| variant-04/02-start-equals-end | 10.7 | 10.6 | n/a |
| variant-02/02-makespan-tiebreak | 10.5 | 10.5 | n/a |
| variant-05/04-depot-and-vertex-revisits | 10.9 | 10.4 | n/a |
| equality-generalized-tsp/02-larger | 11.0 | 10.5 | n/a |
| variant-04/06-layered-dag-ordering-cap | 13.2 | 11.0 | n/a |
| n-queens/variant-01 | 11.3 | 10.5 | n/a |
| variant-01/03-asymmetric | 11.6 | 10.7 | n/a |
| variant-04/09-negative-weights | 10.8 | 10.6 | n/a |
| variant-05/01-basic | 11.0 | 10.5 | n/a |
| variant-04/03-window-too-tight-unsat | 11.1 | 10.5 | n/a |
| variant-02/07-layered-dag-before-after | 11.8 | 10.6 | n/a |
| variant-02/05-larger-mix | 11.4 | 10.3 | n/a |
| variant-01/06-layered-dag | 12.3 | 10.6 | n/a |
| variant-04/06-unreachable-edge | 11.6 | 10.5 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.2 | 10.6 | n/a |
| variant-05/05-three-depots-mixed | 11.3 | 10.6 | n/a |
| variant-03/07-layered-dag-tight-cap | 13.2 | 11.1 | n/a |

Against the reference: report after, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 12 of 94 cells where both passed (12.7%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 34.522 | 151.841 | 0.227 | 3.362 | 33.036 | 12.515 | 86.000 | 60.000 |
| variant-04/03-agent-serialization | 7.682 | 28.816 | 0.267 | 1.051 | 1.559 | 0.321 | 24.000 | 0.000 |
| variant-04/01-basic | 9.248 | 25.635 | 0.361 | 1.320 | 4.007 | 0.840 | 17.000 | 4.000 |
| n-queens/variant-02 | 69.809 | 120.232 | 0.581 | 2.455 | 185.868 | 1.226 | 1.000 | 114.000 |
| variant-01/05-multi-path | 5.529 | 6.505 | 0.850 | 0.306 | 0.174 | 0.030 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 5.627 | 6.570 | 0.856 | 0.262 | 0.072 | 0.014 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 4.864 | 5.477 | 0.888 | 0.229 | 0.143 | 0.037 | 0.000 | 0.000 |
| send-money/send-money | 13.655 | 15.065 | 0.906 | 2.712 | 6.335 | 0.363 | 9.000 | 1.000 |
| variant-02/01-basic | 4.979 | 5.458 | 0.912 | 0.304 | 0.168 | 0.031 | 1.000 | 0.000 |
| variant-03/01-basic | 5.059 | 5.424 | 0.933 | 0.168 | 0.079 | 0.018 | 0.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 5.422 | 5.502 | 0.985 | 0.271 | 0.047 | 0.009 | 0.000 | 0.000 |
| variant-03/01-basic | 5.502 | 5.519 | 0.997 | 0.208 | 0.079 | 0.014 | 1.000 | 0.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/03-window-too-tight-unsat | 5.472 | 5.471 | 1.000 | 0.356 | 0.072 | 0.010 | 1.000 | 0.000 |
| variant-02/02-single-salesman | 5.489 | 5.469 | 1.003 | 0.257 | 0.105 | 0.091 | 1.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 4.525 | 4.443 | 1.019 | 0.115 | 0.045 | 0.005 | 0.000 | 0.000 |
| variant-02/03-cost-dominates | 4.671 | 4.493 | 1.040 | 0.344 | 0.255 | 0.057 | 1.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 5.696 | 5.473 | 1.041 | 0.254 | 0.116 | 0.022 | 1.000 | 0.000 |
| variant-02/05-ordering-unsat | 4.621 | 4.428 | 1.044 | 0.164 | 0.047 | 0.005 | 1.000 | 0.000 |
| variant-01/01-basic | 5.863 | 5.578 | 1.051 | 0.265 | 0.169 | 0.122 | 1.000 | 0.000 |
| variant-02/01-basic | 4.691 | 4.437 | 1.057 | 0.170 | 0.083 | 0.019 | 1.000 | 0.000 |
| variant-02/06-unreachable-edge | 5.855 | 5.500 | 1.065 | 0.478 | 0.182 | 0.170 | 1.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 5.921 | 5.491 | 1.078 | 0.300 | 0.121 | 0.025 | 1.000 | 0.000 |
| variant-01/02-agent-reuse | 4.731 | 4.367 | 1.083 | 0.164 | 0.116 | 0.032 | 0.000 | 0.000 |
| variant-01/01-basic | 4.806 | 4.407 | 1.090 | 0.315 | 0.091 | 0.019 | 0.000 | 0.000 |
| variant-01/01-basic | 4.783 | 4.380 | 1.092 | 0.171 | 0.070 | 0.013 | 1.000 | 0.000 |
| variant-01/03-zero-cost-detour | 4.773 | 4.347 | 1.098 | 0.153 | 0.072 | 0.012 | 1.000 | 0.000 |
| equality-generalized-tsp/01-basic | 5.997 | 5.440 | 1.102 | 0.388 | 0.211 | 0.224 | 1.000 | 0.000 |
| variant-01/02-start-equals-end | 4.864 | 4.405 | 1.104 | 0.140 | 0.050 | 0.026 | 1.000 | 0.000 |
| variant-01/04-subtour-unsat | 4.985 | 4.479 | 1.113 | 0.177 | 0.043 | 0.011 | 0.000 | 0.000 |
| equality-generalized-tsp/02-larger | 6.098 | 5.467 | 1.115 | 0.596 | 0.406 | 0.619 | 1.000 | 0.000 |
| variant-01/03-asymmetric | 6.223 | 5.566 | 1.118 | 0.547 | 0.799 | 1.122 | 1.000 | 0.000 |
| variant-01/08-negative-weights | 4.935 | 4.408 | 1.120 | 0.251 | 0.086 | 0.044 | 1.000 | 0.000 |
| variant-03/04-cost-at-cap-allowed | 5.024 | 4.439 | 1.132 | 0.174 | 0.045 | 0.008 | 1.000 | 0.000 |
| variant-01/03-selective-compatibility | 5.099 | 4.463 | 1.143 | 0.182 | 0.088 | 0.024 | 0.000 | 0.000 |
| variant-01/04-no-path | 5.109 | 4.429 | 1.154 | 0.161 | 0.041 | 0.006 | 0.000 | 0.000 |
| variant-04/02-precedence | 7.628 | 6.598 | 1.156 | 0.659 | 0.771 | 0.110 | 2.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 5.175 | 4.401 | 1.176 | 0.145 | 0.033 | 0.006 | 0.000 | 0.000 |
| variant-04/01-basic | 5.227 | 4.438 | 1.178 | 0.190 | 0.076 | 0.018 | 1.000 | 0.000 |
| variant-03/01-basic | 5.239 | 4.434 | 1.181 | 0.299 | 0.084 | 0.051 | 1.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 5.206 | 4.361 | 1.194 | 0.296 | 0.065 | 0.012 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 6.590 | 5.492 | 1.200 | 0.454 | 0.095 | 0.058 | 1.000 | 0.000 |
| variant-02/01-basic | 6.646 | 5.514 | 1.205 | 0.567 | 0.161 | 0.126 | 1.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 5.491 | 4.503 | 1.219 | 0.306 | 0.111 | 0.010 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 5.612 | 4.553 | 1.233 | 0.253 | 0.108 | 0.080 | 1.000 | 0.000 |
| variant-02/03-too-many-salesmen-unsat | 5.454 | 4.421 | 1.234 | 0.421 | 0.069 | 0.015 | 1.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 5.479 | 4.408 | 1.243 | 0.388 | 0.080 | 0.046 | 1.000 | 0.000 |
| variant-03/02-start-equals-end | 5.493 | 4.411 | 1.245 | 0.182 | 0.050 | 0.026 | 1.000 | 0.000 |
| variant-02/02-start-equals-end | 5.536 | 4.442 | 1.246 | 0.168 | 0.056 | 0.027 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 5.504 | 4.415 | 1.247 | 0.225 | 0.063 | 0.013 | 1.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 5.528 | 4.430 | 1.248 | 0.275 | 0.071 | 0.009 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 5.542 | 4.431 | 1.251 | 0.191 | 0.082 | 0.024 | 0.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 5.558 | 4.435 | 1.253 | 0.302 | 0.104 | 0.020 | 1.000 | 0.000 |
| variant-02/06-layered-dag-before | 8.234 | 6.565 | 1.254 | 1.098 | 1.799 | 0.304 | 2.000 | 0.000 |
| variant-02/07-layered-dag-before-after | 8.259 | 6.565 | 1.258 | 1.128 | 1.557 | 0.239 | 2.000 | 0.000 |
| variant-03/06-unreachable-edge | 5.865 | 4.627 | 1.267 | 0.528 | 0.124 | 0.078 | 1.000 | 0.000 |
| variant-02/04-after-forces-extension | 5.629 | 4.437 | 1.269 | 0.241 | 0.070 | 0.013 | 1.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 5.697 | 4.479 | 1.272 | 0.363 | 0.314 | 0.065 | 1.000 | 0.000 |
| variant-02/03-before-forces-detour | 5.713 | 4.487 | 1.273 | 0.245 | 0.072 | 0.014 | 1.000 | 0.000 |
| variant-02/09-negative-weights | 5.657 | 4.415 | 1.281 | 0.271 | 0.093 | 0.052 | 1.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.758 | 4.488 | 1.283 | 0.361 | 0.069 | 0.013 | 0.000 | 0.000 |
| variant-05/02-tight-bound-exact | 7.063 | 5.495 | 1.285 | 0.363 | 0.079 | 0.045 | 1.000 | 0.000 |
| variant-02/04-equal-cost-split | 5.707 | 4.425 | 1.290 | 0.307 | 0.139 | 0.103 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 5.690 | 4.396 | 1.294 | 0.385 | 0.138 | 0.013 | 1.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 5.785 | 4.438 | 1.303 | 0.292 | 0.059 | 0.009 | 1.000 | 0.000 |
| variant-01/02-multiple-tours | 6.075 | 4.653 | 1.306 | 0.393 | 0.592 | 0.944 | 1.000 | 0.000 |
| variant-03/04-equal-cost-split | 5.773 | 4.368 | 1.322 | 0.451 | 0.185 | 0.163 | 1.000 | 0.000 |
| variant-03/05-larger-mix | 5.857 | 4.428 | 1.323 | 0.270 | 0.466 | 0.114 | 0.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 5.808 | 4.381 | 1.326 | 0.357 | 0.078 | 0.011 | 1.000 | 0.000 |
| n-queens/variant-05 | 8.883 | 6.575 | 1.351 | 1.015 | 4.666 | 0.818 | 1.000 | 1.000 |
| variant-03/09-negative-weights | 5.977 | 4.407 | 1.356 | 0.482 | 0.155 | 0.056 | 1.000 | 0.000 |
| variant-04/02-start-equals-end | 5.968 | 4.392 | 1.359 | 0.199 | 0.053 | 0.029 | 1.000 | 0.000 |
| variant-04/09-negative-weights | 6.075 | 4.467 | 1.360 | 0.329 | 0.098 | 0.053 | 1.000 | 0.000 |
| variant-01/05-larger-mix | 6.160 | 4.431 | 1.390 | 0.260 | 1.449 | 0.149 | 1.000 | 0.000 |
| n-queens/variant-06 | 9.264 | 6.594 | 1.405 | 1.088 | 5.594 | 0.971 | 1.000 | 1.000 |
| n-queens/variant-04 | 7.922 | 5.513 | 1.437 | 0.732 | 4.720 | 0.686 | 1.000 | 1.000 |
| n-queens/variant-03 | 9.513 | 6.573 | 1.447 | 1.808 | 5.317 | 0.953 | 1.000 | 1.000 |
| variant-02/05-larger-asymmetric | 6.514 | 4.485 | 1.452 | 0.432 | 0.155 | 0.139 | 1.000 | 0.000 |
| variant-01/07-cycles | 6.345 | 4.365 | 1.454 | 0.368 | 0.263 | 0.332 | 1.000 | 0.000 |
| variant-05/01-basic | 6.557 | 4.429 | 1.481 | 0.588 | 0.122 | 0.079 | 1.000 | 0.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.606 | 4.461 | 1.481 | 0.441 | 0.202 | 0.038 | 1.000 | 0.000 |
| variant-04/01-basic | 6.566 | 4.387 | 1.497 | 0.589 | 0.129 | 0.080 | 1.000 | 0.000 |
| variant-01/06-layered-dag | 8.244 | 5.499 | 1.499 | 1.014 | 3.181 | 0.321 | 1.000 | 0.000 |
| n-queens/variant-01 | 8.683 | 5.690 | 1.526 | 1.787 | 4.479 | 0.811 | 1.000 | 1.000 |
| variant-03/05-larger-three-depots | 7.123 | 4.663 | 1.528 | 0.814 | 0.169 | 0.130 | 1.000 | 0.000 |
| variant-01/05-ring | 8.700 | 5.528 | 1.574 | 0.716 | 2.735 | 1.811 | 1.000 | 0.000 |
| variant-04/07-layered-dag-combined | 10.364 | 6.561 | 1.580 | 1.824 | 2.033 | 0.335 | 2.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 10.943 | 6.562 | 1.668 | 1.852 | 2.614 | 0.363 | 2.000 | 0.000 |
| variant-05/05-three-depots-mixed | 9.194 | 5.506 | 1.670 | 2.183 | 0.245 | 0.186 | 1.000 | 0.000 |
| variant-04/06-unreachable-edge | 7.674 | 4.492 | 1.708 | 1.176 | 0.218 | 0.166 | 1.000 | 0.000 |
| variant-02/05-larger-mix | 9.437 | 5.461 | 1.728 | 0.742 | 5.705 | 0.189 | 1.000 | 0.000 |
| variant-03/06-layered-dag-cap | 13.215 | 7.613 | 1.736 | 1.659 | 6.693 | 0.435 | 2.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 7.869 | 4.396 | 1.790 | 1.518 | 0.247 | 0.207 | 1.000 | 0.000 |
| variant-05/06-unreachable-edge | 8.172 | 4.455 | 1.835 | 1.174 | 0.223 | 0.196 | 1.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 15.192 | 6.578 | 2.309 | 1.687 | 6.476 | 0.287 | 1.000 | 1.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 17.1 | 23.4 | n/a |
| variant-04/03-agent-serialization | 11.2 | 12.4 | n/a |
| variant-04/01-basic | 11.8 | 13.4 | n/a |
| n-queens/variant-02 | 19.5 | 12.7 | n/a |
| variant-01/05-multi-path | 10.4 | 10.6 | n/a |
| variant-04/03-before-forces-detour-within-budget | 10.7 | 10.5 | n/a |
| variant-03/02-multiple-groups | 10.2 | 10.1 | n/a |
| send-money/send-money | 14.8 | 12.9 | n/a |
| variant-02/01-basic | 10.4 | 10.3 | n/a |
| variant-03/01-basic | 10.3 | 10.5 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.5 | 10.4 | n/a |
| variant-03/01-basic | 10.6 | 10.5 | n/a |
| variant-04/03-window-too-tight-unsat | 11.0 | 10.6 | n/a |
| variant-02/02-single-salesman | 10.5 | 10.5 | n/a |
| variant-03/04-incompatible-group-unsat | 10.2 | 10.3 | n/a |
| variant-02/03-cost-dominates | 10.5 | 10.4 | n/a |
| variant-02/08-tie-break-under-ordering | 10.6 | 10.5 | n/a |
| variant-02/05-ordering-unsat | 10.7 | 10.4 | n/a |
| variant-01/01-basic | 10.4 | 10.2 | n/a |
| variant-02/01-basic | 10.8 | 10.4 | n/a |
| variant-02/06-unreachable-edge | 10.7 | 10.5 | n/a |
| variant-03/08-tie-break-under-cap | 10.7 | 10.5 | n/a |
| variant-01/02-agent-reuse | 10.1 | 10.5 | n/a |
| variant-01/01-basic | 10.3 | 10.3 | n/a |
| variant-01/01-basic | 10.5 | 10.5 | n/a |
| variant-01/03-zero-cost-detour | 10.4 | 10.3 | n/a |
| equality-generalized-tsp/01-basic | 10.5 | 10.5 | n/a |
| variant-01/02-start-equals-end | 10.2 | 10.5 | n/a |
| variant-01/04-subtour-unsat | 10.3 | 10.3 | n/a |
| equality-generalized-tsp/02-larger | 11.2 | 10.6 | n/a |
| variant-01/03-asymmetric | 12.2 | 10.6 | n/a |
| variant-01/08-negative-weights | 10.6 | 10.3 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.8 | 10.5 | n/a |
| variant-01/03-selective-compatibility | 10.1 | 10.1 | n/a |
| variant-01/04-no-path | 10.7 | 10.4 | n/a |
| variant-04/02-precedence | 11.1 | 10.7 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.2 | 10.4 | n/a |
| variant-04/01-basic | 10.7 | 10.3 | n/a |
| variant-03/01-basic | 10.6 | 10.4 | n/a |
| variant-03/03-depot-crossing-unsat | 10.3 | 10.3 | n/a |
| variant-04/02-single-salesman | 10.9 | 10.5 | n/a |
| variant-02/01-basic | 10.7 | 10.3 | n/a |
| variant-04/04-window-too-tight-unsat | 10.9 | 10.6 | n/a |
| variant-03/02-single-salesman | 10.6 | 10.3 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.6 | 10.4 | n/a |
| variant-05/04-depot-and-vertex-revisits | 10.9 | 10.5 | n/a |
| variant-03/02-start-equals-end | 10.7 | 10.3 | n/a |
| variant-02/02-start-equals-end | 10.4 | 10.3 | n/a |
| variant-03/03-budget-forces-detour | 10.5 | 10.3 | n/a |
| variant-04/04-ordering-violates-budget | 10.6 | 10.5 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.3 | 10.3 | n/a |
| variant-04/05-after-and-budget-interact | 10.8 | 10.5 | n/a |
| variant-02/06-layered-dag-before | 11.8 | 10.6 | n/a |
| variant-02/07-layered-dag-before-after | 11.8 | 10.6 | n/a |
| variant-03/06-unreachable-edge | 10.8 | 10.4 | n/a |
| variant-02/04-after-forces-extension | 10.5 | 10.6 | n/a |
| variant-02/02-makespan-tiebreak | 10.4 | 10.4 | n/a |
| variant-02/03-before-forces-detour | 10.5 | 10.4 | n/a |
| variant-02/09-negative-weights | 10.8 | 10.5 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.3 | 10.2 | n/a |
| variant-05/02-tight-bound-exact | 10.9 | 10.6 | n/a |
| variant-02/04-equal-cost-split | 10.7 | 10.4 | n/a |
| variant-03/05-budget-unsat | 10.6 | 10.3 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 10.7 | 10.2 | n/a |
| variant-01/02-multiple-tours | 11.5 | 10.6 | n/a |
| variant-03/04-equal-cost-split | 10.7 | 10.4 | n/a |
| variant-03/05-larger-mix | 10.5 | 10.5 | n/a |
| variant-05/03-revisit-too-tight-unsat | 10.9 | 10.5 | n/a |
| n-queens/variant-05 | 11.6 | 10.5 | n/a |
| variant-03/09-negative-weights | 10.6 | 10.5 | n/a |
| variant-04/02-start-equals-end | 11.0 | 10.5 | n/a |
| variant-04/09-negative-weights | 10.8 | 10.5 | n/a |
| variant-01/05-larger-mix | 10.7 | 10.3 | n/a |
| n-queens/variant-06 | 12.0 | 10.4 | n/a |
| n-queens/variant-04 | 11.1 | 10.5 | n/a |
| n-queens/variant-03 | 11.8 | 10.5 | n/a |
| variant-02/05-larger-asymmetric | 10.8 | 10.6 | n/a |
| variant-01/07-cycles | 11.0 | 10.6 | n/a |
| variant-05/01-basic | 11.2 | 10.6 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 10.7 | 10.5 | n/a |
| variant-04/01-basic | 10.8 | 10.6 | n/a |
| variant-01/06-layered-dag | 12.6 | 10.7 | n/a |
| n-queens/variant-01 | 11.7 | 10.4 | n/a |
| variant-03/05-larger-three-depots | 11.0 | 10.5 | n/a |
| variant-01/05-ring | 14.7 | 10.6 | n/a |
| variant-04/07-layered-dag-combined | 13.1 | 11.1 | n/a |
| variant-04/06-layered-dag-ordering-cap | 12.6 | 11.0 | n/a |
| variant-05/05-three-depots-mixed | 11.4 | 10.5 | n/a |
| variant-04/06-unreachable-edge | 11.5 | 10.6 | n/a |
| variant-02/05-larger-mix | 12.2 | 10.5 | n/a |
| variant-03/06-layered-dag-cap | 15.2 | 10.9 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.2 | 10.7 | n/a |
| variant-05/06-unreachable-edge | 11.5 | 10.7 | n/a |
| variant-03/07-layered-dag-tight-cap | 19.4 | 10.9 | n/a |
