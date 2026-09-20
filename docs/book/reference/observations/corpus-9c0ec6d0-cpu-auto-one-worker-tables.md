Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=1, completion workers=4, batch=64.

Formula search method by report: regions=default; clauses=clauses;

| Cell | regions | clauses | clauses/regions | regions/reference | clauses/reference |
|---|---:|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 5.830 [5.006, 6.614] | 5.633 [5.565, 5.948] | 0.966 | 1.313 | 1.267 |
| equality-generalized-tsp/02-larger | 5.693 [5.678, 6.777] | 6.841 [5.711, 7.322] | 1.201 | 1.297 | 1.522 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.938 [4.831, 6.989] | 4.550 [4.462, 6.042] | 0.921 | 1.126 | 1.029 |
| variant-01/01-basic | 4.915 [4.741, 4.950] | 5.665 [5.150, 5.675] | 1.153 | 0.899 | 1.285 |
| variant-01/02-start-equals-end | 5.366 [4.175, 5.536] | 6.369 [4.794, 6.837] | 1.187 | 1.225 | 1.418 |
| variant-01/03-zero-cost-detour | 4.972 [4.458, 6.801] | 4.744 [4.688, 6.070] | 0.954 | 1.135 | 1.081 |
| variant-01/04-no-path | 5.146 [5.036, 5.237] | 6.049 [4.626, 6.645] | 1.175 | 0.957 | 1.364 |
| variant-01/05-multi-path | 5.019 [4.875, 5.532] | 5.768 [4.970, 6.150] | 1.149 | 0.920 | 1.315 |
| variant-01/06-layered-dag | 8.878 [7.970, 8.963] | 7.951 [7.886, 8.033] | 0.896 | 1.623 | 1.442 |
| variant-01/07-cycles | 6.562 [6.289, 7.063] | 6.591 [5.910, 6.712] | 1.004 | 1.202 | 1.202 |
| variant-01/08-negative-weights | 6.343 [5.483, 6.427] | 5.941 [5.541, 6.563] | 0.937 | 1.427 | 1.338 |
| variant-02/01-basic | 5.243 [4.646, 5.463] | 5.738 [4.876, 6.032] | 1.094 | 1.180 | 1.291 |
| variant-02/02-start-equals-end | 4.654 [4.624, 6.677] | 5.791 [5.531, 6.390] | 1.244 | 1.044 | 1.054 |
| variant-02/03-before-forces-detour | 4.592 [4.445, 5.513] | 5.336 [5.162, 6.318] | 1.162 | 1.055 | 1.206 |
| variant-02/04-after-forces-extension | 4.823 [4.645, 6.041] | 5.533 [5.291, 6.610] | 1.147 | 1.099 | 1.179 |
| variant-02/05-ordering-unsat | 4.775 [4.404, 6.198] | 5.552 [5.188, 8.071] | 1.163 | 1.082 | 1.244 |
| variant-02/06-layered-dag-before | 8.185 [8.073, 8.930] | 8.822 [8.797, 8.825] | 1.078 | 1.206 | 1.434 |
| variant-02/07-layered-dag-before-after | 7.301 [7.180, 7.879] | 8.124 [7.301, 8.789] | 1.113 | 1.120 | 1.238 |
| variant-02/08-tie-break-under-ordering | 5.802 [5.461, 6.989] | 5.800 [5.560, 6.105] | 1.000 | 1.309 | 1.305 |
| variant-02/09-negative-weights | 5.826 [5.492, 6.783] | 5.675 [5.593, 6.295] | 0.974 | 1.329 | 1.280 |
| variant-03/01-basic | 5.610 [4.452, 5.702] | 5.703 [5.512, 5.876] | 1.017 | 1.261 | 1.286 |
| variant-03/02-start-equals-end | 5.033 [4.698, 5.230] | 5.490 [5.287, 5.499] | 1.091 | 1.148 | 1.239 |
| variant-03/03-budget-forces-detour | 5.743 [4.467, 6.024] | 6.049 [4.466, 6.398] | 1.053 | 1.293 | 1.358 |
| variant-03/04-cost-at-cap-allowed | 5.231 [4.966, 5.497] | 7.372 [5.514, 7.421] | 1.409 | 1.192 | 1.348 |
| variant-03/05-budget-unsat | 6.612 [5.119, 6.642] | 5.569 [4.896, 6.594] | 0.842 | 1.202 | 1.009 |
| variant-03/06-layered-dag-cap | 10.459 [8.756, 10.931] | 9.906 [9.804, 11.009] | 0.947 | 1.370 | 1.299 |
| variant-03/07-layered-dag-tight-cap | 9.297 [8.855, 10.797] | 10.081 [8.426, 10.622] | 1.084 | 1.217 | 1.532 |
| variant-03/08-tie-break-under-cap | 5.842 [4.833, 6.140] | 5.973 [5.744, 6.111] | 1.022 | 1.309 | 1.102 |
| variant-03/09-negative-weights | 5.960 [5.773, 5.961] | 5.679 [5.233, 6.175] | 0.953 | 1.344 | 1.285 |
| variant-04/01-basic | 4.625 [4.593, 7.481] | 6.234 [5.723, 6.573] | 1.348 | 1.044 | 1.408 |
| variant-04/02-start-equals-end | 5.536 [5.428, 5.891] | 6.607 [5.855, 7.755] | 1.193 | 1.248 | 1.490 |
| variant-04/03-before-forces-detour-within-budget | 5.618 [5.561, 6.707] | 6.621 [6.329, 6.759] | 1.179 | 1.271 | 1.206 |
| variant-04/04-ordering-violates-budget | 5.967 [4.628, 7.049] | 5.520 [5.238, 6.601] | 0.925 | 1.355 | 1.243 |
| variant-04/05-after-and-budget-interact | 5.496 [4.815, 5.776] | 5.620 [5.486, 6.161] | 1.023 | 1.246 | 1.021 |
| variant-04/06-layered-dag-ordering-cap | 10.671 [10.139, 10.982] | 10.859 [9.238, 12.137] | 1.018 | 1.630 | 1.812 |
| variant-04/07-layered-dag-combined | 10.068 [9.420, 10.211] | 9.081 [7.847, 9.225] | 0.902 | 1.540 | 1.391 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.587 [6.036, 6.793] | 5.993 [5.851, 6.791] | 0.910 | 1.200 | 1.100 |
| variant-04/09-negative-weights | 6.377 [5.704, 7.612] | 6.157 [5.488, 6.804] | 0.965 | 1.438 | 1.397 |
| variant-01/01-basic | 4.718 [4.541, 5.068] | 4.993 [4.633, 6.167] | 1.058 | 1.062 | 1.127 |
| variant-01/02-agent-reuse | 5.922 [5.619, 5.944] | 4.872 [4.612, 5.494] | 0.823 | 1.311 | 1.095 |
| variant-01/03-selective-compatibility | 5.540 [5.157, 5.546] | 4.687 [4.584, 5.216] | 0.846 | 1.014 | 1.057 |
| variant-01/04-no-compatible-agent-unsat | 4.797 [4.470, 4.812] | 4.886 [3.816, 5.539] | 1.019 | 1.096 | 1.113 |
| variant-01/05-larger-mix | 6.111 [5.884, 7.059] | 6.742 [5.976, 8.206] | 1.103 | 1.077 | 1.468 |
| variant-02/01-basic | 5.907 [5.541, 6.166] | 5.625 [4.395, 5.801] | 0.952 | 1.327 | 1.280 |
| variant-02/02-makespan-tiebreak | 5.940 [4.764, 6.920] | 5.566 [5.534, 6.589] | 0.937 | 1.342 | 1.020 |
| variant-02/03-cost-dominates | 6.554 [6.542, 6.627] | 5.592 [5.217, 6.142] | 0.853 | 1.460 | 1.266 |
| variant-02/04-no-compatible-agent-unsat | 5.021 [4.907, 5.556] | 5.483 [5.194, 5.769] | 1.092 | 1.130 | 1.649 |
| variant-02/05-larger-mix | 9.143 [8.007, 9.285] | 10.160 [10.114, 10.434] | 1.111 | 1.667 | 1.864 |
| variant-03/01-basic | 5.629 [5.173, 5.773] | 5.762 [5.605, 6.212] | 1.024 | 1.271 | 1.298 |
| variant-03/02-multiple-groups | 5.601 [5.537, 6.271] | 5.834 [4.663, 6.081] | 1.042 | 1.683 | 1.329 |
| variant-03/03-mixed-grouped-ungrouped | 5.742 [5.488, 6.247] | 5.623 [5.474, 6.404] | 0.979 | 1.294 | 1.194 |
| variant-03/04-incompatible-group-unsat | 4.864 [4.365, 5.622] | 5.027 [4.895, 6.051] | 1.034 | 1.110 | 1.514 |
| variant-03/05-larger-mix | 5.927 [5.558, 5.964] | 5.804 [5.491, 6.923] | 0.979 | 1.338 | 1.275 |
| variant-04/01-basic | 11.362 [8.858, 13.035] | 11.659 [9.775, 12.407] | 1.026 | 0.408 | 0.455 |
| variant-04/02-precedence | 8.072 [7.690, 8.858] | 8.133 [7.898, 8.161] | 1.008 | 1.217 | 1.242 |
| variant-04/03-agent-serialization | 7.755 [7.739, 8.432] | 8.726 [7.690, 9.046] | 1.125 | 0.281 | 0.314 |
| variant-04/04-window-too-tight-unsat | 6.730 [6.718, 8.760] | 7.375 [6.578, 9.470] | 1.096 | 1.469 | 1.343 |
| variant-04/05-larger-mix | 80.186 [78.583, 81.995] | 93.672 [92.344, 94.457] | 1.168 | 0.537 | 0.619 |
| variant-01/01-basic | 6.422 [5.791, 6.642] | 5.910 [5.297, 6.106] | 0.920 | 1.394 | 1.239 |
| variant-01/02-multiple-tours | 6.409 [6.196, 7.034] | 6.751 [5.924, 7.147] | 1.053 | 1.425 | 1.534 |
| variant-01/03-asymmetric | 7.060 [5.677, 8.022] | 8.343 [7.019, 8.973] | 1.182 | 1.584 | 1.893 |
| variant-01/04-subtour-unsat | 4.555 [3.884, 4.803] | 5.773 [5.024, 6.851] | 1.267 | 1.018 | 1.301 |
| variant-01/05-ring | 8.661 [8.207, 8.910] | 11.324 [11.142, 13.026] | 1.308 | 1.585 | 2.038 |
| variant-02/01-basic | 5.725 [4.897, 5.856] | 5.569 [5.343, 5.748] | 0.973 | 1.298 | 1.214 |
| variant-02/02-single-salesman | 5.529 [5.528, 5.741] | 5.524 [5.434, 5.759] | 0.999 | 1.240 | 1.246 |
| variant-02/03-too-many-salesmen-unsat | 5.609 [5.575, 5.982] | 4.630 [4.377, 5.190] | 0.825 | 1.263 | 1.036 |
| variant-02/04-equal-cost-split | 5.854 [5.809, 6.743] | 5.871 [4.851, 6.584] | 1.003 | 1.309 | 1.321 |
| variant-02/05-larger-asymmetric | 6.587 [6.191, 6.975] | 5.935 [5.598, 6.201] | 0.901 | 1.194 | 1.339 |
| variant-02/06-unreachable-edge | 6.608 [5.846, 6.683] | 6.696 [6.074, 6.953] | 1.013 | 1.492 | 1.508 |
| variant-03/01-basic | 5.495 [5.275, 5.621] | 5.850 [5.676, 5.879] | 1.065 | 0.996 | 1.324 |
| variant-03/02-single-salesman | 6.173 [5.627, 6.553] | 5.742 [5.670, 6.186] | 0.930 | 1.387 | 1.294 |
| variant-03/03-depot-crossing-unsat | 5.591 [5.284, 5.769] | 5.336 [4.696, 5.793] | 0.954 | 1.280 | 1.199 |
| variant-03/04-equal-cost-split | 5.957 [5.557, 6.650] | 6.591 [5.453, 6.613] | 1.106 | 1.087 | 0.998 |
| variant-03/05-larger-three-depots | 6.670 [6.635, 8.699] | 8.443 [7.120, 8.773] | 1.266 | 1.211 | 1.919 |
| variant-03/06-unreachable-edge | 6.659 [5.615, 8.215] | 6.657 [6.072, 6.720] | 1.000 | 1.197 | 1.510 |
| variant-04/01-basic | 7.007 [6.865, 7.085] | 6.706 [6.635, 7.761] | 0.957 | 1.289 | 1.215 |
| variant-04/02-single-salesman | 6.887 [6.698, 7.177] | 6.675 [6.276, 7.484] | 0.969 | 1.262 | 1.504 |
| variant-04/03-window-too-tight-unsat | 6.227 [5.571, 6.891] | 5.765 [5.705, 6.057] | 0.926 | 1.413 | 1.060 |
| variant-04/04-depot-window-too-tight-unsat | 6.770 [4.972, 7.363] | 5.894 [4.877, 7.093] | 0.871 | 1.516 | 1.344 |
| variant-04/05-three-depots-asymmetric-times | 7.651 [7.104, 7.820] | 9.080 [9.051, 9.777] | 1.187 | 1.721 | 2.054 |
| variant-04/06-unreachable-edge | 7.485 [7.370, 7.882] | 7.783 [7.674, 9.803] | 1.040 | 1.689 | 1.415 |
| variant-05/01-basic | 5.754 [5.686, 6.807] | 6.748 [5.639, 6.935] | 1.173 | 1.294 | 1.513 |
| variant-05/02-tight-bound-exact | 6.238 [5.105, 7.108] | 6.362 [6.239, 8.405] | 1.020 | 1.408 | 1.160 |
| variant-05/03-revisit-too-tight-unsat | 5.534 [5.496, 5.673] | 6.678 [5.719, 6.687] | 1.207 | 1.014 | 1.217 |
| variant-05/04-depot-and-vertex-revisits | 7.082 [4.799, 8.182] | 6.583 [5.774, 6.894] | 0.929 | 1.534 | 1.201 |
| variant-05/05-three-depots-mixed | 8.726 [7.889, 9.403] | 9.753 [9.436, 10.624] | 1.118 | 1.705 | 1.801 |
| variant-05/06-unreachable-edge | 8.281 [7.490, 10.521] | 8.749 [7.754, 9.845] | 1.056 | 1.520 | 1.970 |
| n-queens/variant-01 | 9.987 [9.921, 10.133] | 8.825 [8.796, 10.115] | 0.884 | 1.516 | 1.346 |
| n-queens/variant-02 | 122.140 [121.709, 122.624] | 90.927 [90.830, 91.499] | 0.744 | 1.006 | 0.757 |
| n-queens/variant-03 | 11.150 [10.419, 11.534] | 9.076 [9.002, 10.920] | 0.814 | 1.661 | 1.642 |
| n-queens/variant-04 | 9.825 [9.259, 9.961] | 7.677 [7.673, 9.790] | 0.781 | 1.460 | 1.162 |
| n-queens/variant-05 | 12.297 [10.011, 12.930] | 12.179 [11.283, 12.651] | 0.990 | 1.878 | 1.833 |
| n-queens/variant-06 | 11.965 [10.833, 12.646] | 11.607 [11.022, 12.758] | 0.970 | 1.820 | 1.765 |
| send-money/send-money | 12.486 [12.337, 12.938] | 11.120 [9.980, 12.035] | 0.891 | 0.833 | 0.741 |

Reference wall time, ms, same notation.

| Cell | regions | clauses |
|---|---:|---:|
| equality-generalized-tsp/01-basic | 4.441 [4.403, 5.493] | 4.446 [4.418, 4.509] |
| equality-generalized-tsp/02-larger | 4.391 [3.321, 5.511] | 4.496 [4.349, 5.492] |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.385 [3.295, 4.443] | 4.422 [4.382, 5.504] |
| variant-01/01-basic | 5.469 [4.392, 5.480] | 4.408 [4.387, 5.498] |
| variant-01/02-start-equals-end | 4.380 [3.277, 4.460] | 4.493 [3.348, 4.592] |
| variant-01/03-zero-cost-detour | 4.379 [4.346, 4.397] | 4.389 [4.363, 4.630] |
| variant-01/04-no-path | 5.379 [4.430, 5.458] | 4.436 [4.417, 5.499] |
| variant-01/05-multi-path | 5.458 [4.435, 6.578] | 4.386 [4.362, 4.435] |
| variant-01/06-layered-dag | 5.471 [4.463, 5.504] | 5.516 [5.463, 5.543] |
| variant-01/07-cycles | 5.460 [4.400, 5.461] | 5.483 [4.457, 5.496] |
| variant-01/08-negative-weights | 4.445 [3.355, 5.444] | 4.441 [4.402, 4.449] |
| variant-02/01-basic | 4.442 [4.439, 4.444] | 4.443 [4.410, 5.441] |
| variant-02/02-start-equals-end | 4.457 [4.443, 4.480] | 5.497 [4.450, 6.660] |
| variant-02/03-before-forces-detour | 4.353 [3.327, 4.447] | 4.425 [4.403, 5.476] |
| variant-02/04-after-forces-extension | 4.389 [3.368, 4.415] | 4.691 [4.440, 5.491] |
| variant-02/05-ordering-unsat | 4.413 [3.328, 4.439] | 4.464 [4.442, 4.498] |
| variant-02/06-layered-dag-before | 6.785 [4.389, 7.747] | 6.151 [5.455, 6.536] |
| variant-02/07-layered-dag-before-after | 6.521 [5.443, 6.670] | 6.560 [6.538, 7.620] |
| variant-02/08-tie-break-under-ordering | 4.432 [4.426, 5.445] | 4.445 [4.348, 5.532] |
| variant-02/09-negative-weights | 4.385 [4.367, 4.446] | 4.434 [3.470, 5.500] |
| variant-03/01-basic | 4.448 [4.389, 4.455] | 4.435 [4.406, 4.519] |
| variant-03/02-start-equals-end | 4.386 [3.337, 5.545] | 4.429 [4.426, 4.482] |
| variant-03/03-budget-forces-detour | 4.443 [3.325, 5.484] | 4.454 [4.397, 5.450] |
| variant-03/04-cost-at-cap-allowed | 4.389 [3.353, 5.498] | 5.467 [4.450, 6.629] |
| variant-03/05-budget-unsat | 5.503 [4.434, 6.609] | 5.517 [4.354, 5.538] |
| variant-03/06-layered-dag-cap | 7.633 [6.560, 7.708] | 7.624 [7.581, 7.718] |
| variant-03/07-layered-dag-tight-cap | 7.638 [6.531, 7.653] | 6.578 [6.537, 6.657] |
| variant-03/08-tie-break-under-cap | 4.463 [4.387, 5.493] | 5.419 [4.445, 5.507] |
| variant-03/09-negative-weights | 4.434 [4.416, 5.564] | 4.421 [4.364, 4.430] |
| variant-04/01-basic | 4.429 [4.427, 4.446] | 4.429 [4.389, 4.489] |
| variant-04/02-start-equals-end | 4.436 [3.323, 4.582] | 4.435 [4.392, 4.452] |
| variant-04/03-before-forces-detour-within-budget | 4.421 [4.389, 4.555] | 5.492 [4.422, 5.527] |
| variant-04/04-ordering-violates-budget | 4.403 [4.341, 5.657] | 4.441 [4.365, 4.453] |
| variant-04/05-after-and-budget-interact | 4.412 [4.406, 5.454] | 5.502 [4.430, 5.575] |
| variant-04/06-layered-dag-ordering-cap | 6.548 [5.430, 7.595] | 5.993 [5.495, 6.578] |
| variant-04/07-layered-dag-combined | 6.537 [5.456, 6.548] | 6.530 [5.409, 6.585] |
| variant-04/08-tie-break-under-ordering-and-cap | 5.491 [4.340, 5.516] | 5.447 [4.435, 6.533] |
| variant-04/09-negative-weights | 4.436 [3.321, 4.448] | 4.408 [4.359, 5.479] |
| variant-01/01-basic | 4.444 [3.490, 5.486] | 4.429 [4.421, 4.432] |
| variant-01/02-agent-reuse | 4.518 [4.509, 5.470] | 4.448 [3.286, 5.524] |
| variant-01/03-selective-compatibility | 5.462 [4.445, 5.464] | 4.435 [4.392, 5.560] |
| variant-01/04-no-compatible-agent-unsat | 4.376 [3.384, 5.436] | 4.389 [3.421, 4.434] |
| variant-01/05-larger-mix | 5.674 [4.427, 5.687] | 4.594 [4.373, 6.673] |
| variant-02/01-basic | 4.451 [4.397, 5.549] | 4.395 [3.325, 4.444] |
| variant-02/02-makespan-tiebreak | 4.425 [4.362, 4.441] | 5.457 [4.448, 5.488] |
| variant-02/03-cost-dominates | 4.489 [4.423, 5.518] | 4.416 [3.290, 4.437] |
| variant-02/04-no-compatible-agent-unsat | 4.443 [3.283, 4.462] | 3.326 [3.314, 4.437] |
| variant-02/05-larger-mix | 5.485 [5.462, 5.518] | 5.451 [4.390, 5.493] |
| variant-03/01-basic | 4.428 [4.419, 4.452] | 4.437 [4.399, 4.456] |
| variant-03/02-multiple-groups | 3.328 [3.312, 4.414] | 4.389 [3.294, 4.430] |
| variant-03/03-mixed-grouped-ungrouped | 4.437 [3.373, 5.504] | 4.709 [4.436, 5.481] |
| variant-03/04-incompatible-group-unsat | 4.384 [3.320, 4.438] | 3.321 [3.262, 4.533] |
| variant-03/05-larger-mix | 4.430 [4.426, 4.436] | 4.551 [3.332, 5.493] |
| variant-04/01-basic | 27.852 [26.710, 28.201] | 25.645 [25.625, 28.911] |
| variant-04/02-precedence | 6.631 [6.593, 6.635] | 6.548 [5.509, 7.701] |
| variant-04/03-agent-serialization | 27.636 [26.637, 28.795] | 27.805 [27.704, 28.784] |
| variant-04/04-window-too-tight-unsat | 4.581 [4.448, 5.538] | 5.490 [4.478, 6.733] |
| variant-04/05-larger-mix | 149.355 [147.949, 149.367] | 151.422 [148.640, 152.513] |
| variant-01/01-basic | 4.608 [4.481, 5.513] | 4.768 [4.533, 5.578] |
| variant-01/02-multiple-tours | 4.498 [4.423, 5.463] | 4.403 [4.382, 4.493] |
| variant-01/03-asymmetric | 4.457 [4.435, 6.501] | 4.407 [4.398, 5.487] |
| variant-01/04-subtour-unsat | 4.477 [4.345, 5.452] | 4.438 [4.428, 5.746] |
| variant-01/05-ring | 5.464 [4.401, 5.481] | 5.558 [5.499, 5.685] |
| variant-02/01-basic | 4.410 [4.364, 4.542] | 4.586 [4.389, 6.522] |
| variant-02/02-single-salesman | 4.459 [4.420, 5.477] | 4.433 [4.382, 5.495] |
| variant-02/03-too-many-salesmen-unsat | 4.441 [4.414, 4.441] | 4.470 [4.452, 5.530] |
| variant-02/04-equal-cost-split | 4.471 [4.430, 5.491] | 4.444 [4.430, 4.475] |
| variant-02/05-larger-asymmetric | 5.516 [5.431, 7.255] | 4.433 [4.407, 5.468] |
| variant-02/06-unreachable-edge | 4.429 [4.419, 5.513] | 4.439 [4.434, 5.468] |
| variant-03/01-basic | 5.517 [4.478, 5.555] | 4.418 [4.396, 4.424] |
| variant-03/02-single-salesman | 4.449 [3.374, 4.497] | 4.438 [4.392, 4.526] |
| variant-03/03-depot-crossing-unsat | 4.367 [3.326, 4.367] | 4.449 [4.349, 5.464] |
| variant-03/04-equal-cost-split | 5.478 [4.464, 5.530] | 6.605 [4.385, 8.734] |
| variant-03/05-larger-three-depots | 5.509 [4.412, 5.523] | 4.400 [4.390, 4.467] |
| variant-03/06-unreachable-edge | 5.564 [4.455, 6.612] | 4.407 [3.327, 4.534] |
| variant-04/01-basic | 5.437 [4.477, 6.568] | 5.521 [5.471, 5.582] |
| variant-04/02-single-salesman | 5.456 [4.434, 5.548] | 4.437 [4.390, 5.523] |
| variant-04/03-window-too-tight-unsat | 4.406 [4.402, 4.602] | 5.441 [4.435, 5.460] |
| variant-04/04-depot-window-too-tight-unsat | 4.466 [4.439, 5.423] | 4.384 [4.346, 5.484] |
| variant-04/05-three-depots-asymmetric-times | 4.444 [4.424, 5.452] | 4.420 [4.411, 5.527] |
| variant-04/06-unreachable-edge | 4.433 [4.395, 5.460] | 5.501 [5.496, 8.758] |
| variant-05/01-basic | 4.445 [4.368, 5.496] | 4.458 [4.396, 5.422] |
| variant-05/02-tight-bound-exact | 4.430 [4.345, 5.499] | 5.487 [4.380, 5.555] |
| variant-05/03-revisit-too-tight-unsat | 5.455 [4.419, 5.501] | 5.486 [5.451, 5.515] |
| variant-05/04-depot-and-vertex-revisits | 4.616 [4.345, 5.495] | 5.482 [4.381, 6.565] |
| variant-05/05-three-depots-mixed | 5.117 [4.394, 5.469] | 5.416 [4.421, 5.516] |
| variant-05/06-unreachable-edge | 5.448 [4.621, 5.490] | 4.440 [4.436, 5.525] |
| n-queens/variant-01 | 6.590 [6.528, 6.641] | 6.558 [5.507, 6.560] |
| n-queens/variant-02 | 121.397 [119.858, 123.862] | 120.070 [118.531, 120.755] |
| n-queens/variant-03 | 6.715 [6.617, 6.716] | 5.527 [5.433, 5.543] |
| n-queens/variant-04 | 6.732 [5.497, 7.759] | 6.606 [6.591, 7.709] |
| n-queens/variant-05 | 6.548 [5.515, 6.627] | 6.646 [5.505, 7.840] |
| n-queens/variant-06 | 6.575 [5.469, 6.590] | 6.575 [5.606, 6.626] |
| send-money/send-money | 14.987 [13.998, 15.021] | 14.997 [13.966, 16.083] |

Counters of report clauses: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 0 | 1 | 4 | 75404 | 2.303 |
| equality-generalized-tsp/02-larger | 0 | 1 | 8 | 130751 | 2.891 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 0 | 0 | 0 | 4096 | 1.285 |
| variant-01/01-basic | 0 | 1 | 2 | 5540 | 1.389 |
| variant-01/02-start-equals-end | 0 | 1 | 1 | 8051 | 1.472 |
| variant-01/03-zero-cost-detour | 0 | 1 | 2 | 4274 | 1.306 |
| variant-01/04-no-path | 0 | 0 | 0 | 1865 | 1.840 |
| variant-01/05-multi-path | 0 | 1 | 3 | 18700 | 1.703 |
| variant-01/06-layered-dag | 0 | 1 | 27 | 344578 | 3.907 |
| variant-01/07-cycles | 0 | 1 | 5 | 114322 | 2.644 |
| variant-01/08-negative-weights | 0 | 1 | 1 | 28915 | 1.899 |
| variant-02/01-basic | 0 | 1 | 2 | 5904 | 2.011 |
| variant-02/02-start-equals-end | 0 | 1 | 1 | 10761 | 1.794 |
| variant-02/03-before-forces-detour | 0 | 1 | 1 | 7017 | 1.737 |
| variant-02/04-after-forces-extension | 0 | 1 | 1 | 6405 | 1.727 |
| variant-02/05-ordering-unsat | 0 | 0 | 0 | 2519 | 1.485 |
| variant-02/06-layered-dag-before | 0 | 2 | 9 | 239298 | 4.951 |
| variant-02/07-layered-dag-before-after | 0 | 1 | 7 | 209967 | 4.296 |
| variant-02/08-tie-break-under-ordering | 0 | 1 | 2 | 10614 | 1.857 |
| variant-02/09-negative-weights | 0 | 1 | 1 | 32506 | 1.977 |
| variant-03/01-basic | 0 | 1 | 2 | 5734 | 1.707 |
| variant-03/02-start-equals-end | 0 | 1 | 1 | 9929 | 1.521 |
| variant-03/03-budget-forces-detour | 0 | 1 | 1 | 6301 | 1.795 |
| variant-03/04-cost-at-cap-allowed | 0 | 1 | 1 | 3269 | 1.748 |
| variant-03/05-budget-unsat | 0 | 0 | 0 | 16082 | 1.869 |
| variant-03/06-layered-dag-cap | 0 | 2 | 25 | 611524 | 5.884 |
| variant-03/07-layered-dag-tight-cap | 0 | 4 | 6 | 363131 | 6.050 |
| variant-03/08-tie-break-under-cap | 0 | 1 | 2 | 12810 | 1.732 |
| variant-03/09-negative-weights | 0 | 1 | 1 | 32853 | 2.131 |
| variant-04/01-basic | 0 | 1 | 2 | 6098 | 2.642 |
| variant-04/02-start-equals-end | 0 | 1 | 1 | 12639 | 2.068 |
| variant-04/03-before-forces-detour-within-budget | 0 | 1 | 1 | 8170 | 2.131 |
| variant-04/04-ordering-violates-budget | 0 | 0 | 0 | 6098 | 1.795 |
| variant-04/05-after-and-budget-interact | 0 | 1 | 1 | 13637 | 1.806 |
| variant-04/06-layered-dag-ordering-cap | 0 | 2 | 8 | 363501 | 6.512 |
| variant-04/07-layered-dag-combined | 0 | 1 | 5 | 313657 | 5.235 |
| variant-04/08-tie-break-under-ordering-and-cap | 0 | 1 | 2 | 27409 | 2.350 |
| variant-04/09-negative-weights | 0 | 1 | 1 | 36453 | 2.170 |
| variant-01/01-basic | 0 | 1 | 4 | 5980 | 1.288 |
| variant-01/02-agent-reuse | 0 | 1 | 8 | 15903 | 1.178 |
| variant-01/03-selective-compatibility | 0 | 1 | 4 | 7209 | 1.230 |
| variant-01/04-no-compatible-agent-unsat | 0 | 0 | 0 | 2289 | 1.011 |
| variant-01/05-larger-mix | 0 | 1 | 64 | 182539 | 2.521 |
| variant-02/01-basic | 0 | 1 | 4 | 14404 | 1.584 |
| variant-02/02-makespan-tiebreak | 0 | 3 | 8 | 25447 | 1.642 |
| variant-02/03-cost-dominates | 0 | 1 | 8 | 29126 | 1.926 |
| variant-02/04-no-compatible-agent-unsat | 0 | 0 | 0 | 4725 | 1.548 |
| variant-02/05-larger-mix | 0 | 1 | 67 | 844697 | 6.844 |
| variant-03/01-basic | 0 | 2 | 2 | 7431 | 2.326 |
| variant-03/02-multiple-groups | 0 | 4 | 4 | 17609 | 1.678 |
| variant-03/03-mixed-grouped-ungrouped | 0 | 2 | 4 | 11285 | 1.771 |
| variant-03/04-incompatible-group-unsat | 0 | 0 | 0 | 2497 | 1.270 |
| variant-03/05-larger-mix | 0 | 1 | 27 | 51564 | 2.078 |
| variant-04/01-basic | 0 | 81 | 124 | 875111 | 7.162 |
| variant-04/02-precedence | 0 | 3 | 12 | 75546 | 4.095 |
| variant-04/03-agent-serialization | 0 | 30 | 30 | 253900 | 4.584 |
| variant-04/04-window-too-tight-unsat | 0 | 0 | 0 | 8883 | 2.821 |
| variant-04/05-larger-mix | 0 | 1176 | 1208 | 23292550 | 89.922 |
| variant-01/01-basic | 0 | 1 | 2 | 45803 | 2.047 |
| variant-01/02-multiple-tours | 0 | 2 | 9 | 217292 | 3.053 |
| variant-01/03-asymmetric | 0 | 1 | 9 | 326714 | 4.204 |
| variant-01/04-subtour-unsat | 0 | 0 | 0 | 4623 | 1.214 |
| variant-01/05-ring | 0 | 2 | 44 | 1422699 | 7.165 |
| variant-02/01-basic | 0 | 2 | 2 | 50389 | 2.096 |
| variant-02/02-single-salesman | 0 | 1 | 2 | 32172 | 1.877 |
| variant-02/03-too-many-salesmen-unsat | 0 | 0 | 0 | 6906 | 1.382 |
| variant-02/04-equal-cost-split | 0 | 2 | 2 | 40903 | 2.131 |
| variant-02/05-larger-asymmetric | 0 | 2 | 2 | 60140 | 2.446 |
| variant-02/06-unreachable-edge | 0 | 2 | 2 | 69881 | 2.958 |
| variant-03/01-basic | 0 | 1 | 1 | 26134 | 1.762 |
| variant-03/02-single-salesman | 0 | 1 | 2 | 30993 | 1.913 |
| variant-03/03-depot-crossing-unsat | 0 | 0 | 0 | 6761 | 1.498 |
| variant-03/04-equal-cost-split | 0 | 2 | 2 | 70632 | 2.479 |
| variant-03/05-larger-three-depots | 0 | 1 | 1 | 73646 | 4.178 |
| variant-03/06-unreachable-edge | 0 | 1 | 1 | 44796 | 2.492 |
| variant-04/01-basic | 0 | 1 | 1 | 46886 | 3.226 |
| variant-04/02-single-salesman | 0 | 1 | 1 | 31547 | 3.047 |
| variant-04/03-window-too-tight-unsat | 0 | 0 | 0 | 6641 | 1.990 |
| variant-04/04-depot-window-too-tight-unsat | 0 | 0 | 0 | 4430 | 2.030 |
| variant-04/05-three-depots-asymmetric-times | 0 | 1 | 1 | 125685 | 5.406 |
| variant-04/06-unreachable-edge | 0 | 1 | 1 | 98235 | 4.188 |
| variant-05/01-basic | 0 | 1 | 1 | 46894 | 2.966 |
| variant-05/02-tight-bound-exact | 0 | 1 | 1 | 23047 | 2.444 |
| variant-05/03-revisit-too-tight-unsat | 0 | 0 | 0 | 6826 | 2.174 |
| variant-05/04-depot-and-vertex-revisits | 0 | 1 | 1 | 23457 | 2.440 |
| variant-05/05-three-depots-mixed | 0 | 1 | 1 | 126050 | 5.729 |
| variant-05/06-unreachable-edge | 0 | 1 | 1 | 98235 | 4.364 |
| n-queens/variant-01 | 0 | 92 | 92 | 734652 | 5.751 |
| n-queens/variant-02 | 0 | 92 | 92 | 18761080 | 87.348 |
| n-queens/variant-03 | 0 | 92 | 92 | 675790 | 5.886 |
| n-queens/variant-04 | 0 | 92 | 92 | 574291 | 4.225 |
| n-queens/variant-05 | 0 | 92 | 92 | 872980 | 8.885 |
| n-queens/variant-06 | 0 | 92 | 92 | 1004609 | 7.797 |
| send-money/send-money | 0 | 1 | 1 | 349198 | 7.557 |

Against the reference: report regions, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=1, completion workers=4, batch=64; search default): faster on 8 of 94 cells where both passed (8.5%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/03-agent-serialization | 7.755 | 27.636 | 0.281 | 0.906 | 0.467 | 0.221 | 23.000 | 0.000 |
| variant-04/01-basic | 11.362 | 27.852 | 0.408 | 0.966 | 1.116 | 0.881 | 19.000 | 4.000 |
| variant-04/05-larger-mix | 80.186 | 149.355 | 0.537 | 3.187 | 31.854 | 24.326 | 85.000 | 59.000 |
| send-money/send-money | 12.486 | 14.987 | 0.833 | 2.547 | 3.475 | 0.305 | 9.000 | 1.000 |
| variant-01/01-basic | 4.915 | 5.469 | 0.899 | 0.166 | 0.033 | 0.012 | 1.000 | 0.000 |
| variant-01/05-multi-path | 5.019 | 5.458 | 0.920 | 0.270 | 0.060 | 0.025 | 1.000 | 0.000 |
| variant-01/04-no-path | 5.146 | 5.379 | 0.957 | 0.168 | 0.020 | 0.006 | 0.000 | 0.000 |
| variant-03/01-basic | 5.495 | 5.517 | 0.996 | 0.347 | 0.044 | 0.014 | 1.000 | 0.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-02 | 122.140 | 121.397 | 1.006 | 2.434 | 112.349 | 2.187 | 2.000 | 116.000 |
| variant-01/03-selective-compatibility | 5.540 | 5.462 | 1.014 | 0.185 | 0.088 | 0.035 | 0.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 5.534 | 5.455 | 1.014 | 0.357 | 0.035 | 0.011 | 1.000 | 0.000 |
| variant-01/04-subtour-unsat | 4.555 | 4.477 | 1.018 | 0.182 | 0.027 | 0.008 | 1.000 | 0.000 |
| variant-04/01-basic | 4.625 | 4.429 | 1.044 | 0.201 | 0.033 | 0.012 | 1.000 | 0.000 |
| variant-02/02-start-equals-end | 4.654 | 4.457 | 1.044 | 0.182 | 0.025 | 0.009 | 1.000 | 0.000 |
| variant-02/03-before-forces-detour | 4.592 | 4.353 | 1.055 | 0.216 | 0.034 | 0.016 | 1.000 | 0.000 |
| variant-01/01-basic | 4.718 | 4.444 | 1.062 | 0.152 | 0.030 | 0.013 | 0.000 | 0.000 |
| variant-01/05-larger-mix | 6.111 | 5.674 | 1.077 | 0.291 | 0.302 | 0.157 | 1.000 | 0.000 |
| variant-02/05-ordering-unsat | 4.775 | 4.413 | 1.082 | 0.169 | 0.023 | 0.006 | 1.000 | 0.000 |
| variant-03/04-equal-cost-split | 5.957 | 5.478 | 1.087 | 0.457 | 0.078 | 0.024 | 1.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 4.797 | 4.376 | 1.096 | 0.123 | 0.019 | 0.005 | 0.000 | 0.000 |
| variant-02/04-after-forces-extension | 4.823 | 4.389 | 1.099 | 0.229 | 0.034 | 0.011 | 1.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 4.864 | 4.384 | 1.110 | 0.117 | 0.021 | 0.005 | 0.000 | 0.000 |
| variant-02/07-layered-dag-before-after | 7.301 | 6.521 | 1.120 | 1.119 | 0.497 | 0.203 | 2.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.938 | 4.385 | 1.126 | 0.187 | 0.025 | 0.010 | 0.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 5.021 | 4.443 | 1.130 | 0.290 | 0.027 | 0.009 | 0.000 | 0.000 |
| variant-01/03-zero-cost-detour | 4.972 | 4.379 | 1.135 | 0.154 | 0.026 | 0.010 | 1.000 | 0.000 |
| variant-03/02-start-equals-end | 5.033 | 4.386 | 1.148 | 0.165 | 0.023 | 0.008 | 1.000 | 0.000 |
| variant-02/01-basic | 5.243 | 4.442 | 1.180 | 0.171 | 0.034 | 0.012 | 1.000 | 0.000 |
| variant-03/04-cost-at-cap-allowed | 5.231 | 4.389 | 1.192 | 0.180 | 0.024 | 0.008 | 1.000 | 0.000 |
| variant-02/05-larger-asymmetric | 6.587 | 5.516 | 1.194 | 0.445 | 0.065 | 0.022 | 1.000 | 0.000 |
| variant-03/06-unreachable-edge | 6.659 | 5.564 | 1.197 | 0.531 | 0.070 | 0.020 | 1.000 | 0.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.587 | 5.491 | 1.200 | 0.443 | 0.082 | 0.033 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 6.612 | 5.503 | 1.202 | 0.372 | 0.060 | 0.014 | 1.000 | 0.000 |
| variant-01/07-cycles | 6.562 | 5.460 | 1.202 | 0.364 | 0.093 | 0.030 | 1.000 | 0.000 |
| variant-02/06-layered-dag-before | 8.185 | 6.785 | 1.206 | 1.121 | 0.798 | 0.342 | 2.000 | 0.000 |
| variant-03/05-larger-three-depots | 6.670 | 5.509 | 1.211 | 0.841 | 0.085 | 0.026 | 1.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 9.297 | 7.638 | 1.217 | 1.686 | 1.310 | 0.304 | 2.000 | 1.000 |
| variant-04/02-precedence | 8.072 | 6.631 | 1.217 | 0.633 | 0.199 | 0.085 | 2.000 | 0.000 |
| variant-01/02-start-equals-end | 5.366 | 4.380 | 1.225 | 0.162 | 0.025 | 0.008 | 1.000 | 0.000 |
| variant-02/02-single-salesman | 5.529 | 4.459 | 1.240 | 0.249 | 0.044 | 0.015 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 5.496 | 4.412 | 1.246 | 0.304 | 0.052 | 0.017 | 1.000 | 0.000 |
| variant-04/02-start-equals-end | 5.536 | 4.436 | 1.248 | 0.208 | 0.030 | 0.010 | 1.000 | 0.000 |
| variant-03/01-basic | 5.610 | 4.448 | 1.261 | 0.218 | 0.032 | 0.012 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 6.887 | 5.456 | 1.262 | 0.514 | 0.051 | 0.018 | 1.000 | 0.000 |
| variant-02/03-too-many-salesmen-unsat | 5.609 | 4.441 | 1.263 | 0.278 | 0.038 | 0.011 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 5.618 | 4.421 | 1.271 | 0.258 | 0.036 | 0.014 | 1.000 | 0.000 |
| variant-03/01-basic | 5.629 | 4.428 | 1.271 | 0.164 | 0.034 | 0.013 | 0.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 5.591 | 4.367 | 1.280 | 0.312 | 0.036 | 0.011 | 1.000 | 0.000 |
| variant-04/01-basic | 7.007 | 5.437 | 1.289 | 0.638 | 0.067 | 0.023 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 5.743 | 4.443 | 1.293 | 0.221 | 0.036 | 0.012 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 5.742 | 4.437 | 1.294 | 0.196 | 0.050 | 0.018 | 0.000 | 0.000 |
| variant-05/01-basic | 5.754 | 4.445 | 1.294 | 0.617 | 0.068 | 0.024 | 1.000 | 0.000 |
| equality-generalized-tsp/02-larger | 5.693 | 4.391 | 1.297 | 0.586 | 0.101 | 0.037 | 1.000 | 0.000 |
| variant-02/01-basic | 5.725 | 4.410 | 1.298 | 0.351 | 0.062 | 0.019 | 1.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 5.802 | 4.432 | 1.309 | 0.251 | 0.051 | 0.018 | 1.000 | 0.000 |
| variant-02/04-equal-cost-split | 5.854 | 4.471 | 1.309 | 0.323 | 0.057 | 0.018 | 1.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 5.842 | 4.463 | 1.309 | 0.263 | 0.050 | 0.018 | 1.000 | 0.000 |
| variant-01/02-agent-reuse | 5.922 | 4.518 | 1.311 | 0.200 | 0.083 | 0.043 | 0.000 | 0.000 |
| equality-generalized-tsp/01-basic | 5.830 | 4.441 | 1.313 | 0.369 | 0.070 | 0.024 | 1.000 | 0.000 |
| variant-02/01-basic | 5.907 | 4.451 | 1.327 | 0.298 | 0.060 | 0.024 | 1.000 | 0.000 |
| variant-02/09-negative-weights | 5.826 | 4.385 | 1.329 | 0.265 | 0.049 | 0.010 | 1.000 | 0.000 |
| variant-03/05-larger-mix | 5.927 | 4.430 | 1.338 | 0.282 | 0.103 | 0.082 | 1.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 5.940 | 4.425 | 1.342 | 0.746 | 0.084 | 0.040 | 1.000 | 0.000 |
| variant-03/09-negative-weights | 5.960 | 4.434 | 1.344 | 0.295 | 0.050 | 0.008 | 1.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 5.967 | 4.403 | 1.355 | 0.289 | 0.043 | 0.010 | 1.000 | 0.000 |
| variant-03/06-layered-dag-cap | 10.459 | 7.633 | 1.370 | 1.642 | 1.369 | 0.630 | 2.000 | 1.000 |
| variant-03/02-single-salesman | 6.173 | 4.449 | 1.387 | 0.261 | 0.046 | 0.018 | 1.000 | 0.000 |
| variant-01/01-basic | 6.422 | 4.608 | 1.394 | 0.277 | 0.070 | 0.020 | 1.000 | 0.000 |
| variant-05/02-tight-bound-exact | 6.238 | 4.430 | 1.408 | 0.398 | 0.043 | 0.014 | 1.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 6.227 | 4.406 | 1.413 | 0.394 | 0.038 | 0.012 | 1.000 | 0.000 |
| variant-01/02-multiple-tours | 6.409 | 4.498 | 1.425 | 0.406 | 0.163 | 0.084 | 1.000 | 0.000 |
| variant-01/08-negative-weights | 6.343 | 4.445 | 1.427 | 0.248 | 0.044 | 0.007 | 1.000 | 0.000 |
| variant-04/09-negative-weights | 6.377 | 4.436 | 1.438 | 0.328 | 0.056 | 0.008 | 1.000 | 0.000 |
| n-queens/variant-04 | 9.825 | 6.732 | 1.460 | 0.725 | 2.503 | 0.755 | 1.000 | 1.000 |
| variant-02/03-cost-dominates | 6.554 | 4.489 | 1.460 | 0.362 | 0.077 | 0.039 | 1.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 6.730 | 4.581 | 1.469 | 0.323 | 0.047 | 0.011 | 1.000 | 0.000 |
| variant-02/06-unreachable-edge | 6.608 | 4.429 | 1.492 | 0.482 | 0.168 | 0.054 | 1.000 | 0.000 |
| n-queens/variant-01 | 9.987 | 6.590 | 1.516 | 1.824 | 2.007 | 0.948 | 1.000 | 1.000 |
| variant-04/04-depot-window-too-tight-unsat | 6.770 | 4.466 | 1.516 | 0.306 | 0.033 | 0.010 | 1.000 | 0.000 |
| variant-05/06-unreachable-edge | 8.281 | 5.448 | 1.520 | 1.200 | 0.119 | 0.042 | 1.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 7.082 | 4.616 | 1.534 | 0.390 | 0.042 | 0.014 | 1.000 | 0.000 |
| variant-04/07-layered-dag-combined | 10.068 | 6.537 | 1.540 | 1.767 | 0.837 | 0.312 | 2.000 | 0.000 |
| variant-01/03-asymmetric | 7.060 | 4.457 | 1.584 | 0.541 | 0.213 | 0.110 | 1.000 | 0.000 |
| variant-01/05-ring | 8.661 | 5.464 | 1.585 | 0.687 | 0.755 | 0.286 | 1.000 | 0.000 |
| variant-01/06-layered-dag | 8.878 | 5.471 | 1.623 | 1.042 | 0.915 | 0.444 | 1.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 10.671 | 6.548 | 1.630 | 1.758 | 0.922 | 0.364 | 3.000 | 0.000 |
| n-queens/variant-03 | 11.150 | 6.715 | 1.661 | 2.141 | 2.237 | 1.026 | 1.000 | 1.000 |
| variant-02/05-larger-mix | 9.143 | 5.485 | 1.667 | 0.733 | 1.214 | 0.439 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 5.601 | 3.328 | 1.683 | 0.217 | 0.053 | 0.021 | 0.000 | 0.000 |
| variant-04/06-unreachable-edge | 7.485 | 4.433 | 1.689 | 1.189 | 0.119 | 0.045 | 1.000 | 0.000 |
| variant-05/05-three-depots-mixed | 8.726 | 5.117 | 1.705 | 1.528 | 0.132 | 0.060 | 1.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 7.651 | 4.444 | 1.721 | 1.515 | 0.144 | 0.050 | 1.000 | 0.000 |
| n-queens/variant-06 | 11.965 | 6.575 | 1.820 | 1.062 | 2.889 | 1.275 | 1.000 | 1.000 |
| n-queens/variant-05 | 12.297 | 6.548 | 1.878 | 1.048 | 2.527 | 1.097 | 1.000 | 1.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/03-agent-serialization | 11.4 | 12.4 | n/a |
| variant-04/01-basic | 11.5 | 13.4 | n/a |
| variant-04/05-larger-mix | 13.4 | 23.5 | n/a |
| send-money/send-money | 12.3 | 12.7 | n/a |
| variant-01/01-basic | 10.5 | 10.2 | n/a |
| variant-01/05-multi-path | 10.8 | 10.6 | n/a |
| variant-01/04-no-path | 10.4 | 10.1 | n/a |
| variant-03/01-basic | 10.8 | 10.5 | n/a |
| n-queens/variant-02 | 12.9 | 12.9 | n/a |
| variant-01/03-selective-compatibility | 10.4 | 10.5 | n/a |
| variant-05/03-revisit-too-tight-unsat | 11.2 | 10.5 | n/a |
| variant-01/04-subtour-unsat | 10.5 | 10.4 | n/a |
| variant-04/01-basic | 11.0 | 10.4 | n/a |
| variant-02/02-start-equals-end | 10.4 | 10.5 | n/a |
| variant-02/03-before-forces-detour | 10.6 | 10.6 | n/a |
| variant-01/01-basic | 10.2 | 10.5 | n/a |
| variant-01/05-larger-mix | 10.8 | 10.3 | n/a |
| variant-02/05-ordering-unsat | 10.8 | 10.2 | n/a |
| variant-03/04-equal-cost-split | 10.8 | 10.5 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.3 | 10.3 | n/a |
| variant-02/04-after-forces-extension | 10.7 | 10.6 | n/a |
| variant-03/04-incompatible-group-unsat | 10.3 | 10.3 | n/a |
| variant-02/07-layered-dag-before-after | 11.5 | 10.8 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.4 | 10.4 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.6 | 10.5 | n/a |
| variant-01/03-zero-cost-detour | 10.6 | 10.4 | n/a |
| variant-03/02-start-equals-end | 10.8 | 10.5 | n/a |
| variant-02/01-basic | 10.9 | 10.5 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.8 | 10.5 | n/a |
| variant-02/05-larger-asymmetric | 10.6 | 10.5 | n/a |
| variant-03/06-unreachable-edge | 10.8 | 10.5 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 10.8 | 10.5 | n/a |
| variant-03/05-budget-unsat | 10.7 | 10.3 | n/a |
| variant-01/07-cycles | 10.7 | 10.6 | n/a |
| variant-02/06-layered-dag-before | 11.4 | 10.7 | n/a |
| variant-03/05-larger-three-depots | 11.0 | 10.4 | n/a |
| variant-03/07-layered-dag-tight-cap | 11.8 | 11.0 | n/a |
| variant-04/02-precedence | 11.5 | 10.7 | n/a |
| variant-01/02-start-equals-end | 10.5 | 10.3 | n/a |
| variant-02/02-single-salesman | 10.3 | 10.3 | n/a |
| variant-04/05-after-and-budget-interact | 10.9 | 10.5 | n/a |
| variant-04/02-start-equals-end | 10.8 | 10.6 | n/a |
| variant-03/01-basic | 10.7 | 10.5 | n/a |
| variant-04/02-single-salesman | 11.2 | 10.3 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.6 | 10.4 | n/a |
| variant-04/03-before-forces-detour-within-budget | 11.1 | 10.5 | n/a |
| variant-03/01-basic | 10.5 | 10.4 | n/a |
| variant-03/03-depot-crossing-unsat | 10.6 | 10.3 | n/a |
| variant-04/01-basic | 11.0 | 10.6 | n/a |
| variant-03/03-budget-forces-detour | 10.7 | 10.4 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.7 | 10.2 | n/a |
| variant-05/01-basic | 11.1 | 10.5 | n/a |
| equality-generalized-tsp/02-larger | 10.7 | 10.5 | n/a |
| variant-02/01-basic | 10.7 | 10.4 | n/a |
| variant-02/08-tie-break-under-ordering | 10.9 | 10.6 | n/a |
| variant-02/04-equal-cost-split | 10.8 | 10.5 | n/a |
| variant-03/08-tie-break-under-cap | 11.0 | 10.5 | n/a |
| variant-01/02-agent-reuse | 10.4 | 10.3 | n/a |
| equality-generalized-tsp/01-basic | 10.8 | 10.5 | n/a |
| variant-02/01-basic | 10.4 | 10.3 | n/a |
| variant-02/09-negative-weights | 10.7 | 10.6 | n/a |
| variant-03/05-larger-mix | 10.7 | 10.5 | n/a |
| variant-02/02-makespan-tiebreak | 10.6 | 10.5 | n/a |
| variant-03/09-negative-weights | 10.7 | 10.5 | n/a |
| variant-04/04-ordering-violates-budget | 10.7 | 10.3 | n/a |
| variant-03/06-layered-dag-cap | 11.6 | 11.0 | n/a |
| variant-03/02-single-salesman | 10.6 | 10.3 | n/a |
| variant-01/01-basic | 10.4 | 10.4 | n/a |
| variant-05/02-tight-bound-exact | 10.9 | 10.5 | n/a |
| variant-04/03-window-too-tight-unsat | 11.0 | 10.5 | n/a |
| variant-01/02-multiple-tours | 10.8 | 10.6 | n/a |
| variant-01/08-negative-weights | 10.8 | 10.5 | n/a |
| variant-04/09-negative-weights | 11.2 | 10.6 | n/a |
| n-queens/variant-04 | 10.6 | 10.6 | n/a |
| variant-02/03-cost-dominates | 10.8 | 10.3 | n/a |
| variant-04/04-window-too-tight-unsat | 11.3 | 10.6 | n/a |
| variant-02/06-unreachable-edge | 10.8 | 10.5 | n/a |
| n-queens/variant-01 | 11.2 | 10.4 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 10.9 | 10.4 | n/a |
| variant-05/06-unreachable-edge | 11.4 | 10.3 | n/a |
| variant-05/04-depot-and-vertex-revisits | 11.2 | 10.5 | n/a |
| variant-04/07-layered-dag-combined | 12.2 | 11.0 | n/a |
| variant-01/03-asymmetric | 11.3 | 10.4 | n/a |
| variant-01/05-ring | 11.2 | 10.6 | n/a |
| variant-01/06-layered-dag | 11.2 | 10.8 | n/a |
| variant-04/06-layered-dag-ordering-cap | 12.3 | 11.1 | n/a |
| n-queens/variant-03 | 11.3 | 10.5 | n/a |
| variant-02/05-larger-mix | 11.7 | 10.5 | n/a |
| variant-03/02-multiple-groups | 10.7 | 10.3 | n/a |
| variant-04/06-unreachable-edge | 11.2 | 10.5 | n/a |
| variant-05/05-three-depots-mixed | 11.4 | 10.7 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.4 | 10.8 | n/a |
| n-queens/variant-06 | 11.2 | 10.5 | n/a |
| n-queens/variant-05 | 11.1 | 10.5 | n/a |

Against the reference: report clauses, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=1, completion workers=4, batch=64; search clauses): faster on 6 of 94 cells where both passed (6.3%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/03-agent-serialization | 8.726 | 27.805 | 0.314 | 0.941 | 0.632 | 0.220 | 23.000 | 0.000 |
| variant-04/01-basic | 11.659 | 25.645 | 0.455 | 1.488 | 1.659 | 0.861 | 17.000 | 4.000 |
| variant-04/05-larger-mix | 93.672 | 151.422 | 0.619 | 3.263 | 44.450 | 23.886 | 85.000 | 60.000 |
| send-money/send-money | 11.120 | 14.997 | 0.741 | 2.517 | 2.085 | 0.346 | 10.000 | 1.000 |
| n-queens/variant-02 | 90.927 | 120.070 | 0.757 | 2.429 | 81.087 | 2.149 | 2.000 | 114.000 |
| variant-03/04-equal-cost-split | 6.591 | 6.605 | 0.998 | 0.461 | 0.073 | 0.139 | 1.000 | 0.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-03/05-budget-unsat | 5.569 | 5.517 | 1.009 | 0.364 | 0.061 | 0.014 | 1.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 5.566 | 5.457 | 1.020 | 0.340 | 0.077 | 0.039 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 5.620 | 5.502 | 1.021 | 0.290 | 0.048 | 0.017 | 1.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.550 | 4.422 | 1.029 | 0.190 | 0.020 | 0.011 | 0.000 | 0.000 |
| variant-02/03-too-many-salesmen-unsat | 4.630 | 4.470 | 1.036 | 0.286 | 0.029 | 0.011 | 1.000 | 0.000 |
| variant-02/02-start-equals-end | 5.791 | 5.497 | 1.054 | 0.184 | 0.021 | 0.030 | 1.000 | 0.000 |
| variant-01/03-selective-compatibility | 4.687 | 4.435 | 1.057 | 0.170 | 0.032 | 0.015 | 1.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 5.765 | 5.441 | 1.060 | 0.361 | 0.025 | 0.012 | 1.000 | 0.000 |
| variant-01/03-zero-cost-detour | 4.744 | 4.389 | 1.081 | 0.156 | 0.018 | 0.010 | 1.000 | 0.000 |
| variant-01/02-agent-reuse | 4.872 | 4.448 | 1.095 | 0.168 | 0.030 | 0.020 | 0.000 | 0.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 5.993 | 5.447 | 1.100 | 0.451 | 0.089 | 0.034 | 2.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 5.973 | 5.419 | 1.102 | 0.276 | 0.043 | 0.018 | 1.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 4.886 | 4.389 | 1.113 | 0.133 | 0.014 | 0.006 | 0.000 | 0.000 |
| variant-01/01-basic | 4.993 | 4.429 | 1.127 | 0.146 | 0.026 | 0.013 | 0.000 | 0.000 |
| variant-05/02-tight-bound-exact | 6.362 | 5.487 | 1.160 | 0.384 | 0.033 | 0.060 | 1.000 | 0.000 |
| n-queens/variant-04 | 7.677 | 6.606 | 1.162 | 0.719 | 1.609 | 0.649 | 1.000 | 1.000 |
| variant-02/04-after-forces-extension | 5.533 | 4.691 | 1.179 | 0.235 | 0.027 | 0.012 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 5.623 | 4.709 | 1.194 | 0.210 | 0.038 | 0.018 | 0.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 5.336 | 4.449 | 1.199 | 0.290 | 0.033 | 0.012 | 1.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 6.583 | 5.482 | 1.201 | 0.382 | 0.031 | 0.053 | 1.000 | 0.000 |
| variant-01/07-cycles | 6.591 | 5.483 | 1.202 | 0.368 | 0.092 | 0.102 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 6.621 | 5.492 | 1.206 | 0.285 | 0.034 | 0.014 | 1.000 | 0.000 |
| variant-02/03-before-forces-detour | 5.336 | 4.425 | 1.206 | 0.216 | 0.029 | 0.013 | 1.000 | 0.000 |
| variant-02/01-basic | 5.569 | 4.586 | 1.214 | 0.362 | 0.059 | 0.094 | 1.000 | 0.000 |
| variant-04/01-basic | 6.706 | 5.521 | 1.215 | 0.612 | 0.066 | 0.137 | 1.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 6.678 | 5.486 | 1.217 | 0.357 | 0.028 | 0.012 | 1.000 | 0.000 |
| variant-02/07-layered-dag-before-after | 8.124 | 6.560 | 1.238 | 1.149 | 0.667 | 0.169 | 2.000 | 0.000 |
| variant-01/01-basic | 5.910 | 4.768 | 1.239 | 0.272 | 0.063 | 0.092 | 1.000 | 0.000 |
| variant-03/02-start-equals-end | 5.490 | 4.429 | 1.239 | 0.167 | 0.015 | 0.025 | 1.000 | 0.000 |
| variant-04/02-precedence | 8.133 | 6.548 | 1.242 | 0.622 | 0.215 | 0.085 | 2.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 5.520 | 4.441 | 1.243 | 0.281 | 0.028 | 0.010 | 1.000 | 0.000 |
| variant-02/05-ordering-unsat | 5.552 | 4.464 | 1.244 | 0.165 | 0.013 | 0.005 | 1.000 | 0.000 |
| variant-02/02-single-salesman | 5.524 | 4.433 | 1.246 | 0.268 | 0.038 | 0.074 | 1.000 | 0.000 |
| variant-02/03-cost-dominates | 5.592 | 4.416 | 1.266 | 0.335 | 0.076 | 0.038 | 1.000 | 0.000 |
| equality-generalized-tsp/01-basic | 5.633 | 4.446 | 1.267 | 0.372 | 0.061 | 0.099 | 1.000 | 0.000 |
| variant-03/05-larger-mix | 5.804 | 4.551 | 1.275 | 0.277 | 0.082 | 0.081 | 0.000 | 0.000 |
| variant-02/01-basic | 5.625 | 4.395 | 1.280 | 0.301 | 0.047 | 0.025 | 1.000 | 0.000 |
| variant-02/09-negative-weights | 5.675 | 4.434 | 1.280 | 0.263 | 0.044 | 0.069 | 1.000 | 0.000 |
| variant-03/09-negative-weights | 5.679 | 4.421 | 1.285 | 0.306 | 0.049 | 0.081 | 1.000 | 0.000 |
| variant-01/01-basic | 5.665 | 4.408 | 1.285 | 0.183 | 0.024 | 0.012 | 1.000 | 0.000 |
| variant-03/01-basic | 5.703 | 4.435 | 1.286 | 0.213 | 0.025 | 0.012 | 1.000 | 0.000 |
| variant-02/01-basic | 5.738 | 4.443 | 1.291 | 0.199 | 0.022 | 0.012 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 5.742 | 4.438 | 1.294 | 0.248 | 0.036 | 0.049 | 1.000 | 0.000 |
| variant-03/01-basic | 5.762 | 4.437 | 1.298 | 0.156 | 0.034 | 0.013 | 0.000 | 0.000 |
| variant-03/06-layered-dag-cap | 9.906 | 7.624 | 1.299 | 1.672 | 1.817 | 0.536 | 3.000 | 0.000 |
| variant-01/04-subtour-unsat | 5.773 | 4.438 | 1.301 | 0.190 | 0.021 | 0.009 | 0.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 5.800 | 4.445 | 1.305 | 0.256 | 0.039 | 0.018 | 1.000 | 0.000 |
| variant-01/05-multi-path | 5.768 | 4.386 | 1.315 | 0.275 | 0.061 | 0.024 | 1.000 | 0.000 |
| variant-02/04-equal-cost-split | 5.871 | 4.444 | 1.321 | 0.319 | 0.047 | 0.088 | 1.000 | 0.000 |
| variant-03/01-basic | 5.850 | 4.418 | 1.324 | 0.316 | 0.031 | 0.073 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 5.834 | 4.389 | 1.329 | 0.221 | 0.051 | 0.023 | 1.000 | 0.000 |
| variant-01/08-negative-weights | 5.941 | 4.441 | 1.338 | 0.267 | 0.047 | 0.074 | 1.000 | 0.000 |
| variant-02/05-larger-asymmetric | 5.935 | 4.433 | 1.339 | 0.458 | 0.059 | 0.100 | 1.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 7.375 | 5.490 | 1.343 | 0.323 | 0.038 | 0.010 | 1.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 5.894 | 4.384 | 1.344 | 0.316 | 0.023 | 0.010 | 1.000 | 0.000 |
| n-queens/variant-01 | 8.825 | 6.558 | 1.346 | 1.787 | 1.918 | 0.926 | 1.000 | 1.000 |
| variant-03/04-cost-at-cap-allowed | 7.372 | 5.467 | 1.348 | 0.193 | 0.020 | 0.008 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 6.049 | 4.454 | 1.358 | 0.239 | 0.027 | 0.012 | 1.000 | 0.000 |
| variant-01/04-no-path | 6.049 | 4.436 | 1.364 | 0.168 | 0.015 | 0.006 | 1.000 | 0.000 |
| variant-04/07-layered-dag-combined | 9.081 | 6.530 | 1.391 | 1.742 | 1.136 | 0.199 | 2.000 | 0.000 |
| variant-04/09-negative-weights | 6.157 | 4.408 | 1.397 | 0.297 | 0.051 | 0.074 | 1.000 | 0.000 |
| variant-04/01-basic | 6.234 | 4.429 | 1.408 | 0.378 | 0.028 | 0.013 | 1.000 | 0.000 |
| variant-04/06-unreachable-edge | 7.783 | 5.501 | 1.415 | 1.162 | 0.142 | 0.240 | 1.000 | 0.000 |
| variant-01/02-start-equals-end | 6.369 | 4.493 | 1.418 | 0.163 | 0.016 | 0.024 | 1.000 | 0.000 |
| variant-02/06-layered-dag-before | 8.822 | 6.151 | 1.434 | 1.264 | 1.216 | 0.240 | 2.000 | 0.000 |
| variant-01/06-layered-dag | 7.951 | 5.516 | 1.442 | 1.014 | 0.880 | 0.307 | 2.000 | 0.000 |
| variant-01/05-larger-mix | 6.742 | 4.594 | 1.468 | 0.265 | 0.449 | 0.149 | 1.000 | 0.000 |
| variant-04/02-start-equals-end | 6.607 | 4.435 | 1.490 | 0.205 | 0.021 | 0.034 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 6.675 | 4.437 | 1.504 | 0.478 | 0.046 | 0.070 | 1.000 | 0.000 |
| variant-02/06-unreachable-edge | 6.696 | 4.439 | 1.508 | 0.478 | 0.069 | 0.123 | 1.000 | 0.000 |
| variant-03/06-unreachable-edge | 6.657 | 4.407 | 1.510 | 0.528 | 0.059 | 0.109 | 1.000 | 0.000 |
| variant-05/01-basic | 6.748 | 4.458 | 1.513 | 0.594 | 0.062 | 0.138 | 1.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 5.027 | 3.321 | 1.514 | 0.117 | 0.017 | 0.005 | 0.000 | 0.000 |
| equality-generalized-tsp/02-larger | 6.841 | 4.496 | 1.522 | 0.588 | 0.092 | 0.175 | 1.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 10.081 | 6.578 | 1.532 | 1.738 | 1.359 | 0.202 | 1.000 | 1.000 |
| variant-01/02-multiple-tours | 6.751 | 4.403 | 1.534 | 0.381 | 0.227 | 0.193 | 1.000 | 0.000 |
| n-queens/variant-03 | 9.076 | 5.527 | 1.642 | 1.846 | 1.676 | 1.020 | 1.000 | 1.000 |
| variant-02/04-no-compatible-agent-unsat | 5.483 | 3.326 | 1.649 | 0.267 | 0.022 | 0.008 | 0.000 | 0.000 |
| n-queens/variant-06 | 11.607 | 6.575 | 1.765 | 1.045 | 2.499 | 1.176 | 1.000 | 1.000 |
| variant-05/05-three-depots-mixed | 9.753 | 5.416 | 1.801 | 1.524 | 0.172 | 0.330 | 1.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 10.859 | 5.993 | 1.812 | 2.265 | 1.259 | 0.262 | 2.000 | 0.000 |
| n-queens/variant-05 | 12.179 | 6.646 | 1.833 | 1.031 | 2.800 | 1.116 | 1.000 | 1.000 |
| variant-02/05-larger-mix | 10.160 | 5.451 | 1.864 | 0.725 | 3.331 | 0.449 | 1.000 | 0.000 |
| variant-01/03-asymmetric | 8.343 | 4.407 | 1.893 | 0.536 | 0.303 | 0.277 | 1.000 | 0.000 |
| variant-03/05-larger-three-depots | 8.443 | 4.400 | 1.919 | 0.819 | 0.071 | 0.199 | 1.000 | 0.000 |
| variant-05/06-unreachable-edge | 8.749 | 4.440 | 1.970 | 1.213 | 0.129 | 0.238 | 1.000 | 0.000 |
| variant-01/05-ring | 11.324 | 5.558 | 2.038 | 0.700 | 0.951 | 0.538 | 1.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 9.080 | 4.420 | 2.054 | 1.916 | 0.199 | 0.368 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/03-agent-serialization | 11.6 | 12.3 | n/a |
| variant-04/01-basic | 11.6 | 13.1 | n/a |
| variant-04/05-larger-mix | 13.4 | 23.3 | n/a |
| send-money/send-money | 12.3 | 12.7 | n/a |
| n-queens/variant-02 | 11.0 | 12.7 | n/a |
| variant-03/04-equal-cost-split | 10.9 | 10.4 | n/a |
| variant-03/05-budget-unsat | 10.5 | 10.6 | n/a |
| variant-02/02-makespan-tiebreak | 10.4 | 10.3 | n/a |
| variant-04/05-after-and-budget-interact | 10.8 | 10.6 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.6 | 10.3 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.5 | 10.4 | n/a |
| variant-02/02-start-equals-end | 10.7 | 10.4 | n/a |
| variant-01/03-selective-compatibility | 10.4 | 10.3 | n/a |
| variant-04/03-window-too-tight-unsat | 10.8 | 10.3 | n/a |
| variant-01/03-zero-cost-detour | 10.5 | 10.4 | n/a |
| variant-01/02-agent-reuse | 10.3 | 10.3 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 10.9 | 10.5 | n/a |
| variant-03/08-tie-break-under-cap | 10.6 | 10.6 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.3 | 10.5 | n/a |
| variant-01/01-basic | 10.3 | 10.3 | n/a |
| variant-05/02-tight-bound-exact | 11.0 | 10.6 | n/a |
| n-queens/variant-04 | 10.6 | 10.5 | n/a |
| variant-02/04-after-forces-extension | 10.7 | 10.4 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.8 | 10.3 | n/a |
| variant-03/03-depot-crossing-unsat | 10.3 | 10.3 | n/a |
| variant-05/04-depot-and-vertex-revisits | 10.9 | 10.3 | n/a |
| variant-01/07-cycles | 10.6 | 10.5 | n/a |
| variant-04/03-before-forces-detour-within-budget | 10.8 | 10.6 | n/a |
| variant-02/03-before-forces-detour | 10.9 | 10.6 | n/a |
| variant-02/01-basic | 10.7 | 10.3 | n/a |
| variant-04/01-basic | 11.0 | 10.5 | n/a |
| variant-05/03-revisit-too-tight-unsat | 10.8 | 10.6 | n/a |
| variant-02/07-layered-dag-before-after | 11.2 | 10.7 | n/a |
| variant-01/01-basic | 10.6 | 10.3 | n/a |
| variant-03/02-start-equals-end | 10.5 | 10.3 | n/a |
| variant-04/02-precedence | 11.2 | 10.7 | n/a |
| variant-04/04-ordering-violates-budget | 10.9 | 10.5 | n/a |
| variant-02/05-ordering-unsat | 10.6 | 10.5 | n/a |
| variant-02/02-single-salesman | 10.7 | 10.5 | n/a |
| variant-02/03-cost-dominates | 10.6 | 10.2 | n/a |
| equality-generalized-tsp/01-basic | 10.4 | 10.5 | n/a |
| variant-03/05-larger-mix | 10.7 | 10.5 | n/a |
| variant-02/01-basic | 10.5 | 10.2 | n/a |
| variant-02/09-negative-weights | 10.7 | 10.5 | n/a |
| variant-03/09-negative-weights | 10.8 | 10.6 | n/a |
| variant-01/01-basic | 10.6 | 10.5 | n/a |
| variant-03/01-basic | 10.5 | 10.5 | n/a |
| variant-02/01-basic | 10.5 | 10.4 | n/a |
| variant-03/02-single-salesman | 10.5 | 10.3 | n/a |
| variant-03/01-basic | 10.3 | 10.2 | n/a |
| variant-03/06-layered-dag-cap | 11.7 | 10.8 | n/a |
| variant-01/04-subtour-unsat | 10.5 | 10.4 | n/a |
| variant-02/08-tie-break-under-ordering | 10.8 | 10.4 | n/a |
| variant-01/05-multi-path | 10.6 | 10.2 | n/a |
| variant-02/04-equal-cost-split | 10.6 | 10.6 | n/a |
| variant-03/01-basic | 10.7 | 10.5 | n/a |
| variant-03/02-multiple-groups | 10.5 | 10.3 | n/a |
| variant-01/08-negative-weights | 10.5 | 10.4 | n/a |
| variant-02/05-larger-asymmetric | 10.9 | 10.4 | n/a |
| variant-04/04-window-too-tight-unsat | 11.2 | 10.4 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 10.9 | 10.5 | n/a |
| n-queens/variant-01 | 10.6 | 10.0 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.6 | 10.3 | n/a |
| variant-03/03-budget-forces-detour | 10.6 | 10.4 | n/a |
| variant-01/04-no-path | 10.5 | 10.4 | n/a |
| variant-04/07-layered-dag-combined | 11.9 | 11.0 | n/a |
| variant-04/09-negative-weights | 11.0 | 10.3 | n/a |
| variant-04/01-basic | 10.7 | 10.4 | n/a |
| variant-04/06-unreachable-edge | 11.1 | 10.6 | n/a |
| variant-01/02-start-equals-end | 10.7 | 10.5 | n/a |
| variant-02/06-layered-dag-before | 11.2 | 10.7 | n/a |
| variant-01/06-layered-dag | 11.3 | 10.8 | n/a |
| variant-01/05-larger-mix | 10.4 | 10.3 | n/a |
| variant-04/02-start-equals-end | 10.8 | 10.4 | n/a |
| variant-04/02-single-salesman | 10.8 | 10.5 | n/a |
| variant-02/06-unreachable-edge | 10.9 | 10.6 | n/a |
| variant-03/06-unreachable-edge | 10.9 | 10.4 | n/a |
| variant-05/01-basic | 11.0 | 10.6 | n/a |
| variant-03/04-incompatible-group-unsat | 10.7 | 10.3 | n/a |
| equality-generalized-tsp/02-larger | 10.8 | 10.5 | n/a |
| variant-03/07-layered-dag-tight-cap | 11.7 | 10.9 | n/a |
| variant-01/02-multiple-tours | 11.1 | 10.4 | n/a |
| n-queens/variant-03 | 10.6 | 10.3 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.6 | 10.5 | n/a |
| n-queens/variant-06 | 10.7 | 10.6 | n/a |
| variant-05/05-three-depots-mixed | 11.0 | 10.5 | n/a |
| variant-04/06-layered-dag-ordering-cap | 11.8 | 11.0 | n/a |
| n-queens/variant-05 | 10.8 | 10.7 | n/a |
| variant-02/05-larger-mix | 11.1 | 10.5 | n/a |
| variant-01/03-asymmetric | 11.5 | 10.7 | n/a |
| variant-03/05-larger-three-depots | 11.1 | 10.4 | n/a |
| variant-05/06-unreachable-edge | 11.3 | 10.5 | n/a |
| variant-01/05-ring | 12.0 | 10.7 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.5 | 10.7 | n/a |
