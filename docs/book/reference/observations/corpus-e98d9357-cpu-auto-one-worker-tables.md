Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=1, completion workers=4, batch=64.

Formula search method by report: regions=default; clauses=clauses;

| Cell | regions | clauses | clauses/regions | regions/reference | clauses/reference |
|---|---:|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 5.946 [4.919, 6.136] | 6.672 [5.694, 7.170] | 1.122 | 1.072 | 1.494 |
| equality-generalized-tsp/02-larger | 6.924 [6.008, 7.764] | 6.774 [6.769, 7.150] | 0.978 | 1.561 | 1.510 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.756 [4.646, 5.948] | 5.533 [4.529, 5.575] | 1.163 | 1.380 | 1.243 |
| variant-01/01-basic | 4.730 [4.695, 5.617] | 4.698 [4.603, 5.648] | 0.993 | 1.065 | 1.057 |
| variant-01/02-start-equals-end | 5.851 [5.659, 5.905] | 4.915 [4.602, 5.723] | 0.840 | 1.337 | 1.116 |
| variant-01/03-zero-cost-detour | 4.922 [4.674, 5.754] | 4.793 [4.601, 5.999] | 0.974 | 1.107 | 1.085 |
| variant-01/04-no-path | 5.300 [4.442, 5.845] | 5.615 [4.497, 5.619] | 1.060 | 1.196 | 1.268 |
| variant-01/05-multi-path | 5.034 [5.029, 5.568] | 5.531 [5.017, 5.817] | 1.099 | 1.121 | 1.253 |
| variant-01/06-layered-dag | 7.690 [6.908, 7.869] | 8.370 [7.687, 8.713] | 1.088 | 1.410 | 1.277 |
| variant-01/07-cycles | 5.945 [5.634, 6.752] | 6.333 [6.182, 6.354] | 1.065 | 1.341 | 1.436 |
| variant-01/08-negative-weights | 5.617 [4.624, 6.013] | 5.886 [5.861, 6.330] | 1.048 | 1.266 | 1.324 |
| variant-02/01-basic | 5.564 [4.396, 6.480] | 5.572 [5.131, 5.717] | 1.001 | 1.265 | 1.253 |
| variant-02/02-start-equals-end | 5.212 [5.040, 6.053] | 5.430 [5.163, 5.612] | 1.042 | 1.176 | 1.221 |
| variant-02/03-before-forces-detour | 5.572 [4.519, 5.737] | 6.014 [4.819, 6.920] | 1.079 | 1.014 | 1.100 |
| variant-02/04-after-forces-extension | 5.915 [5.539, 6.263] | 5.298 [4.917, 5.538] | 0.896 | 1.333 | 1.180 |
| variant-02/05-ordering-unsat | 5.566 [4.614, 5.920] | 5.204 [4.890, 5.557] | 0.935 | 1.254 | 1.184 |
| variant-02/06-layered-dag-before | 8.734 [7.303, 8.786] | 7.907 [7.644, 7.925] | 0.905 | 1.595 | 1.435 |
| variant-02/07-layered-dag-before-after | 7.756 [7.600, 7.875] | 7.697 [7.146, 7.970] | 0.992 | 1.180 | 1.420 |
| variant-02/08-tie-break-under-ordering | 5.535 [4.638, 5.826] | 6.146 [5.672, 6.308] | 1.110 | 1.246 | 1.394 |
| variant-02/09-negative-weights | 5.864 [5.580, 5.897] | 6.081 [4.869, 6.144] | 1.037 | 1.320 | 1.362 |
| variant-03/01-basic | 5.734 [4.379, 6.731] | 5.673 [5.295, 5.680] | 0.989 | 1.289 | 1.278 |
| variant-03/02-start-equals-end | 5.174 [4.553, 5.344] | 5.714 [5.096, 6.919] | 1.104 | 1.190 | 1.710 |
| variant-03/03-budget-forces-detour | 5.709 [5.229, 5.824] | 5.542 [5.503, 5.978] | 0.971 | 1.044 | 1.222 |
| variant-03/04-cost-at-cap-allowed | 5.072 [4.862, 6.046] | 5.245 [5.015, 5.797] | 1.034 | 1.141 | 1.184 |
| variant-03/05-budget-unsat | 6.055 [5.692, 6.073] | 5.613 [5.327, 6.542] | 0.927 | 1.366 | 1.024 |
| variant-03/06-layered-dag-cap | 10.996 [8.763, 11.399] | 9.737 [9.495, 11.648] | 0.885 | 1.654 | 1.495 |
| variant-03/07-layered-dag-tight-cap | 9.945 [9.417, 10.507] | 9.901 [9.044, 10.515] | 0.996 | 1.497 | 1.500 |
| variant-03/08-tie-break-under-cap | 6.115 [5.497, 6.243] | 5.998 [5.585, 6.096] | 0.981 | 1.375 | 1.097 |
| variant-03/09-negative-weights | 6.309 [6.217, 6.557] | 5.895 [5.566, 6.857] | 0.934 | 1.427 | 1.326 |
| variant-04/01-basic | 5.335 [5.037, 6.158] | 5.779 [5.515, 6.025] | 1.083 | 1.220 | 1.304 |
| variant-04/02-start-equals-end | 5.590 [5.001, 6.194] | 5.799 [5.621, 6.497] | 1.037 | 1.260 | 1.306 |
| variant-04/03-before-forces-detour-within-budget | 5.680 [5.261, 6.533] | 5.594 [4.588, 6.819] | 0.985 | 1.280 | 1.257 |
| variant-04/04-ordering-violates-budget | 5.533 [4.575, 6.588] | 5.538 [4.819, 5.567] | 1.001 | 1.248 | 1.008 |
| variant-04/05-after-and-budget-interact | 5.875 [4.813, 6.521] | 5.943 [5.450, 6.201] | 1.012 | 1.338 | 1.334 |
| variant-04/06-layered-dag-ordering-cap | 9.028 [8.968, 9.752] | 9.758 [9.575, 9.910] | 1.081 | 1.185 | 1.495 |
| variant-04/07-layered-dag-combined | 9.330 [9.189, 9.744] | 9.768 [9.075, 9.926] | 1.047 | 1.414 | 1.477 |
| variant-04/08-tie-break-under-ordering-and-cap | 5.861 [5.803, 6.819] | 6.282 [6.034, 6.832] | 1.072 | 1.066 | 1.410 |
| variant-04/09-negative-weights | 5.749 [5.544, 6.136] | 5.912 [5.852, 6.912] | 1.028 | 1.046 | 1.346 |
| variant-01/01-basic | 4.851 [3.909, 5.544] | 4.911 [4.850, 5.355] | 1.012 | 1.089 | 1.475 |
| variant-01/02-agent-reuse | 4.551 [4.359, 5.695] | 5.671 [4.489, 6.212] | 1.246 | 1.023 | 1.290 |
| variant-01/03-selective-compatibility | 5.533 [5.255, 6.036] | 5.631 [4.628, 6.219] | 1.018 | 1.242 | 1.269 |
| variant-01/04-no-compatible-agent-unsat | 4.428 [4.426, 5.318] | 4.546 [3.751, 4.695] | 1.027 | 1.327 | 1.024 |
| variant-01/05-larger-mix | 6.743 [5.028, 6.785] | 6.354 [5.769, 6.588] | 0.942 | 1.522 | 1.438 |
| variant-02/01-basic | 5.456 [4.453, 5.561] | 5.153 [4.788, 5.515] | 0.944 | 1.229 | 1.161 |
| variant-02/02-makespan-tiebreak | 5.085 [4.727, 6.588] | 6.146 [4.739, 6.637] | 1.209 | 1.145 | 1.380 |
| variant-02/03-cost-dominates | 5.477 [4.594, 5.582] | 5.218 [5.036, 6.223] | 0.953 | 1.231 | 1.178 |
| variant-02/04-no-compatible-agent-unsat | 5.749 [5.266, 5.955] | 5.545 [4.933, 6.207] | 0.964 | 1.261 | 1.256 |
| variant-02/05-larger-mix | 8.893 [8.893, 9.292] | 9.945 [9.135, 11.215] | 1.118 | 1.994 | 1.816 |
| variant-03/01-basic | 5.575 [5.357, 5.604] | 6.047 [5.229, 6.849] | 1.085 | 1.676 | 1.372 |
| variant-03/02-multiple-groups | 5.519 [5.306, 5.672] | 5.998 [5.193, 6.030] | 1.087 | 1.238 | 1.360 |
| variant-03/03-mixed-grouped-ungrouped | 5.456 [4.425, 5.490] | 5.201 [4.835, 5.575] | 0.953 | 1.228 | 1.181 |
| variant-03/04-incompatible-group-unsat | 5.262 [4.700, 5.876] | 4.844 [4.388, 5.243] | 0.921 | 1.187 | 1.091 |
| variant-03/05-larger-mix | 5.989 [5.560, 6.305] | 5.651 [5.510, 6.157] | 0.944 | 1.347 | 1.277 |
| variant-04/01-basic | 9.901 [9.775, 10.794] | 10.888 [9.042, 11.404] | 1.100 | 0.370 | 0.425 |
| variant-04/02-precedence | 7.771 [7.692, 8.223] | 8.044 [7.162, 8.241] | 1.035 | 1.179 | 1.214 |
| variant-04/03-agent-serialization | 8.150 [8.051, 9.160] | 7.963 [7.331, 7.978] | 0.977 | 0.283 | 0.277 |
| variant-04/04-window-too-tight-unsat | 6.610 [5.705, 7.230] | 6.875 [6.114, 6.953] | 1.040 | 1.208 | 1.257 |
| variant-04/05-larger-mix | 82.590 [81.556, 82.944] | 93.894 [92.965, 94.005] | 1.137 | 0.550 | 0.628 |
| variant-01/01-basic | 4.847 [4.748, 5.483] | 6.008 [5.948, 6.276] | 1.239 | 1.077 | 1.320 |
| variant-01/02-multiple-tours | 5.441 [5.434, 6.569] | 6.692 [6.185, 7.253] | 1.230 | 1.235 | 1.487 |
| variant-01/03-asymmetric | 6.792 [6.562, 8.283] | 8.758 [7.833, 8.918] | 1.289 | 1.528 | 1.956 |
| variant-01/04-subtour-unsat | 4.875 [4.750, 5.058] | 5.857 [4.471, 5.917] | 1.202 | 1.109 | 1.324 |
| variant-01/05-ring | 8.670 [8.659, 9.266] | 11.395 [11.096, 11.745] | 1.314 | 1.955 | 2.063 |
| variant-02/01-basic | 5.843 [5.279, 6.812] | 6.049 [5.785, 6.741] | 1.035 | 1.327 | 1.352 |
| variant-02/02-single-salesman | 5.168 [4.821, 6.223] | 5.821 [5.781, 5.905] | 1.126 | 1.172 | 1.306 |
| variant-02/03-too-many-salesmen-unsat | 5.212 [4.388, 5.998] | 5.478 [4.393, 6.070] | 1.051 | 1.172 | 1.235 |
| variant-02/04-equal-cost-split | 5.154 [5.008, 5.503] | 6.752 [5.840, 7.637] | 1.310 | 0.949 | 1.532 |
| variant-02/05-larger-asymmetric | 5.509 [4.830, 6.554] | 6.016 [5.943, 7.264] | 1.092 | 1.245 | 1.366 |
| variant-02/06-unreachable-edge | 5.782 [5.729, 6.605] | 6.572 [6.413, 6.700] | 1.136 | 1.059 | 1.470 |
| variant-03/01-basic | 5.614 [4.717, 6.336] | 5.997 [5.135, 6.158] | 1.068 | 1.266 | 1.353 |
| variant-03/02-single-salesman | 5.705 [5.069, 5.929] | 5.512 [5.498, 5.907] | 0.966 | 1.291 | 1.238 |
| variant-03/03-depot-crossing-unsat | 5.544 [4.702, 6.720] | 4.897 [4.799, 4.980] | 0.883 | 1.260 | 0.889 |
| variant-03/04-equal-cost-split | 5.845 [5.521, 6.134] | 6.845 [5.706, 6.961] | 1.171 | 1.068 | 1.549 |
| variant-03/05-larger-three-depots | 6.587 [5.871, 6.777] | 7.662 [5.954, 7.672] | 1.163 | 1.198 | 1.396 |
| variant-03/06-unreachable-edge | 6.191 [5.943, 6.512] | 6.119 [5.818, 6.278] | 0.988 | 1.392 | 1.119 |
| variant-04/01-basic | 6.106 [5.648, 7.327] | 7.089 [5.658, 7.131] | 1.161 | 1.372 | 1.593 |
| variant-04/02-single-salesman | 5.930 [5.611, 6.992] | 6.604 [5.627, 7.090] | 1.114 | 1.339 | 1.488 |
| variant-04/03-window-too-tight-unsat | 5.902 [5.210, 6.745] | 6.006 [5.529, 6.069] | 1.018 | 1.339 | 1.095 |
| variant-04/04-depot-window-too-tight-unsat | 5.699 [5.445, 6.628] | 5.527 [5.503, 5.846] | 0.970 | 1.296 | 1.008 |
| variant-04/05-three-depots-asymmetric-times | 8.724 [7.663, 8.754] | 8.513 [8.218, 8.771] | 0.976 | 1.586 | 1.935 |
| variant-04/06-unreachable-edge | 7.732 [6.840, 7.988] | 8.075 [7.731, 9.773] | 1.044 | 1.758 | 1.806 |
| variant-05/01-basic | 7.178 [5.560, 7.483] | 6.501 [6.272, 6.920] | 0.906 | 1.627 | 1.179 |
| variant-05/02-tight-bound-exact | 5.798 [5.662, 5.867] | 6.111 [5.442, 6.348] | 1.054 | 1.309 | 1.122 |
| variant-05/03-revisit-too-tight-unsat | 6.350 [6.280, 7.137] | 5.988 [5.498, 6.039] | 0.943 | 1.414 | 1.095 |
| variant-05/04-depot-and-vertex-revisits | 7.015 [6.581, 7.480] | 6.717 [6.375, 6.880] | 0.958 | 1.284 | 1.506 |
| variant-05/05-three-depots-mixed | 7.888 [7.692, 8.776] | 8.693 [8.005, 8.806] | 1.102 | 1.440 | 1.959 |
| variant-05/06-unreachable-edge | 7.665 [7.646, 7.885] | 8.741 [7.872, 8.888] | 1.140 | 1.666 | 1.590 |
| n-queens/variant-01 | 9.865 [9.319, 10.088] | 10.455 [9.498, 10.485] | 1.060 | 1.515 | 1.915 |
| n-queens/variant-02 | 121.596 [120.979, 122.099] | 89.608 [89.560, 92.208] | 0.737 | 1.003 | 0.733 |
| n-queens/variant-03 | 10.031 [9.752, 10.276] | 10.084 [9.191, 10.496] | 1.005 | 1.515 | 1.548 |
| n-queens/variant-04 | 8.860 [8.510, 9.937] | 8.445 [7.845, 9.038] | 0.953 | 1.352 | 1.518 |
| n-queens/variant-05 | 11.994 [11.476, 12.244] | 10.309 [10.153, 10.742] | 0.859 | 1.818 | 1.567 |
| n-queens/variant-06 | 12.770 [12.759, 13.028] | 12.306 [10.607, 13.084] | 0.964 | 1.935 | 2.208 |
| send-money/send-money | 12.655 [12.580, 12.985] | 11.407 [11.344, 12.255] | 0.901 | 0.788 | 0.760 |

Reference wall time, ms, same notation.

| Cell | regions | clauses |
|---|---:|---:|
| equality-generalized-tsp/01-basic | 5.548 [4.446, 5.618] | 4.466 [4.451, 4.476] |
| equality-generalized-tsp/02-larger | 4.435 [4.402, 4.580] | 4.486 [4.348, 5.469] |
| equality-generalized-tsp/03-unreachable-subset-unsat | 3.448 [3.321, 5.450] | 4.451 [4.439, 5.498] |
| variant-01/01-basic | 4.442 [3.280, 4.445] | 4.446 [4.407, 5.519] |
| variant-01/02-start-equals-end | 4.375 [3.373, 4.430] | 4.404 [3.369, 4.434] |
| variant-01/03-zero-cost-detour | 4.446 [4.398, 4.511] | 4.415 [4.397, 5.499] |
| variant-01/04-no-path | 4.429 [4.408, 4.497] | 4.429 [4.389, 4.460] |
| variant-01/05-multi-path | 4.489 [4.355, 5.493] | 4.413 [4.407, 4.422] |
| variant-01/06-layered-dag | 5.452 [4.442, 6.556] | 6.557 [5.512, 6.574] |
| variant-01/07-cycles | 4.433 [4.358, 5.526] | 4.410 [4.345, 4.437] |
| variant-01/08-negative-weights | 4.437 [3.330, 4.450] | 4.447 [4.424, 5.511] |
| variant-02/01-basic | 4.398 [3.319, 4.473] | 4.446 [4.443, 5.429] |
| variant-02/02-start-equals-end | 4.433 [4.415, 4.443] | 4.447 [4.393, 4.457] |
| variant-02/03-before-forces-detour | 5.497 [4.396, 5.506] | 5.469 [4.483, 5.541] |
| variant-02/04-after-forces-extension | 4.436 [4.433, 4.475] | 4.490 [4.460, 5.489] |
| variant-02/05-ordering-unsat | 4.437 [4.430, 5.496] | 4.395 [4.375, 5.538] |
| variant-02/06-layered-dag-before | 5.474 [5.395, 5.517] | 5.511 [5.467, 5.513] |
| variant-02/07-layered-dag-before-after | 6.572 [6.565, 6.599] | 5.421 [5.410, 5.474] |
| variant-02/08-tie-break-under-ordering | 4.443 [4.347, 4.445] | 4.408 [3.296, 4.537] |
| variant-02/09-negative-weights | 4.441 [4.428, 4.446] | 4.463 [4.392, 5.432] |
| variant-03/01-basic | 4.449 [4.387, 5.498] | 4.440 [4.402, 4.440] |
| variant-03/02-start-equals-end | 4.349 [3.322, 4.461] | 3.341 [3.285, 4.494] |
| variant-03/03-budget-forces-detour | 5.467 [4.403, 5.493] | 4.536 [4.443, 5.512] |
| variant-03/04-cost-at-cap-allowed | 4.447 [3.297, 6.538] | 4.430 [4.417, 4.445] |
| variant-03/05-budget-unsat | 4.433 [4.394, 5.518] | 5.480 [4.442, 5.499] |
| variant-03/06-layered-dag-cap | 6.647 [6.552, 6.841] | 6.513 [6.512, 6.536] |
| variant-03/07-layered-dag-tight-cap | 6.645 [6.573, 8.697] | 6.600 [5.485, 7.623] |
| variant-03/08-tie-break-under-cap | 4.448 [4.392, 5.501] | 5.469 [4.442, 5.517] |
| variant-03/09-negative-weights | 4.422 [4.399, 4.440] | 4.445 [4.436, 4.451] |
| variant-04/01-basic | 4.375 [4.361, 5.493] | 4.430 [4.360, 5.501] |
| variant-04/02-start-equals-end | 4.436 [3.332, 5.462] | 4.442 [4.436, 5.499] |
| variant-04/03-before-forces-detour-within-budget | 4.439 [4.435, 5.467] | 4.450 [4.405, 5.547] |
| variant-04/04-ordering-violates-budget | 4.433 [4.417, 4.442] | 5.496 [5.483, 5.498] |
| variant-04/05-after-and-budget-interact | 4.390 [4.387, 4.403] | 4.455 [4.454, 5.463] |
| variant-04/06-layered-dag-ordering-cap | 7.617 [6.582, 7.625] | 6.526 [6.515, 7.626] |
| variant-04/07-layered-dag-combined | 6.599 [5.426, 6.705] | 6.613 [6.542, 7.648] |
| variant-04/08-tie-break-under-ordering-and-cap | 5.495 [4.415, 5.518] | 4.454 [4.365, 4.492] |
| variant-04/09-negative-weights | 5.496 [5.459, 5.501] | 4.392 [4.389, 5.480] |
| variant-01/01-basic | 4.453 [4.433, 4.537] | 3.330 [3.266, 4.404] |
| variant-01/02-agent-reuse | 4.451 [3.389, 4.480] | 4.396 [4.381, 4.448] |
| variant-01/03-selective-compatibility | 4.454 [4.403, 4.462] | 4.438 [3.328, 4.573] |
| variant-01/04-no-compatible-agent-unsat | 3.338 [3.324, 3.415] | 4.438 [4.344, 4.444] |
| variant-01/05-larger-mix | 4.430 [4.405, 4.449] | 4.419 [4.360, 4.439] |
| variant-02/01-basic | 4.441 [3.323, 4.452] | 4.440 [4.408, 4.512] |
| variant-02/02-makespan-tiebreak | 4.440 [4.359, 5.493] | 4.454 [4.408, 4.521] |
| variant-02/03-cost-dominates | 4.449 [3.327, 4.480] | 4.430 [4.363, 5.416] |
| variant-02/04-no-compatible-agent-unsat | 4.560 [4.433, 5.773] | 4.414 [3.337, 4.421] |
| variant-02/05-larger-mix | 4.460 [4.428, 5.658] | 5.476 [4.349, 5.496] |
| variant-03/01-basic | 3.326 [3.290, 4.450] | 4.406 [4.393, 4.407] |
| variant-03/02-multiple-groups | 4.457 [4.393, 5.429] | 4.409 [4.320, 4.432] |
| variant-03/03-mixed-grouped-ungrouped | 4.443 [3.325, 4.443] | 4.404 [3.367, 4.450] |
| variant-03/04-incompatible-group-unsat | 4.435 [4.397, 5.452] | 4.441 [4.419, 4.470] |
| variant-03/05-larger-mix | 4.445 [4.417, 5.491] | 4.427 [4.425, 4.436] |
| variant-04/01-basic | 26.748 [25.921, 27.720] | 25.623 [25.606, 26.719] |
| variant-04/02-precedence | 6.593 [5.478, 6.659] | 6.626 [6.568, 7.645] |
| variant-04/03-agent-serialization | 28.818 [27.806, 28.873] | 28.780 [27.757, 28.865] |
| variant-04/04-window-too-tight-unsat | 5.472 [4.468, 5.577] | 5.468 [4.504, 5.762] |
| variant-04/05-larger-mix | 150.208 [149.148, 151.791] | 149.570 [147.869, 150.646] |
| variant-01/01-basic | 4.500 [4.391, 4.532] | 4.551 [4.538, 5.712] |
| variant-01/02-multiple-tours | 4.407 [4.396, 5.502] | 4.501 [4.410, 5.585] |
| variant-01/03-asymmetric | 4.445 [4.433, 5.539] | 4.478 [4.416, 5.461] |
| variant-01/04-subtour-unsat | 4.395 [3.298, 4.500] | 4.425 [4.421, 5.548] |
| variant-01/05-ring | 4.434 [4.373, 5.456] | 5.523 [4.399, 5.526] |
| variant-02/01-basic | 4.402 [4.401, 4.441] | 4.473 [4.392, 5.483] |
| variant-02/02-single-salesman | 4.410 [4.397, 4.479] | 4.455 [4.422, 5.465] |
| variant-02/03-too-many-salesmen-unsat | 4.445 [3.288, 4.448] | 4.437 [4.364, 5.488] |
| variant-02/04-equal-cost-split | 5.430 [4.436, 5.500] | 4.406 [4.392, 4.430] |
| variant-02/05-larger-asymmetric | 4.425 [4.402, 5.464] | 4.403 [4.399, 5.505] |
| variant-02/06-unreachable-edge | 5.458 [4.437, 5.492] | 4.470 [4.442, 5.455] |
| variant-03/01-basic | 4.435 [4.384, 5.562] | 4.433 [4.411, 5.482] |
| variant-03/02-single-salesman | 4.421 [4.398, 4.436] | 4.452 [4.404, 5.465] |
| variant-03/03-depot-crossing-unsat | 4.399 [4.398, 4.444] | 5.507 [5.448, 5.516] |
| variant-03/04-equal-cost-split | 5.472 [4.393, 5.534] | 4.420 [4.408, 4.429] |
| variant-03/05-larger-three-depots | 5.497 [4.444, 5.522] | 5.491 [5.466, 5.507] |
| variant-03/06-unreachable-edge | 4.449 [4.388, 5.509] | 5.470 [4.442, 5.539] |
| variant-04/01-basic | 4.450 [4.358, 5.501] | 4.450 [4.354, 5.469] |
| variant-04/02-single-salesman | 4.430 [4.395, 4.476] | 4.438 [4.436, 4.445] |
| variant-04/03-window-too-tight-unsat | 4.407 [4.405, 5.506] | 5.485 [4.442, 5.493] |
| variant-04/04-depot-window-too-tight-unsat | 4.397 [4.394, 5.495] | 5.483 [4.430, 5.506] |
| variant-04/05-three-depots-asymmetric-times | 5.502 [4.421, 5.524] | 4.399 [4.374, 5.494] |
| variant-04/06-unreachable-edge | 4.399 [4.373, 4.458] | 4.470 [4.455, 5.493] |
| variant-05/01-basic | 4.411 [4.366, 5.460] | 5.514 [5.490, 6.511] |
| variant-05/02-tight-bound-exact | 4.430 [4.424, 5.468] | 5.448 [4.451, 5.489] |
| variant-05/03-revisit-too-tight-unsat | 4.489 [4.338, 5.483] | 5.467 [4.430, 5.497] |
| variant-05/04-depot-and-vertex-revisits | 5.463 [4.434, 5.488] | 4.460 [4.388, 4.488] |
| variant-05/05-three-depots-mixed | 5.478 [4.398, 5.738] | 4.438 [4.403, 6.570] |
| variant-05/06-unreachable-edge | 4.601 [4.440, 5.503] | 5.497 [4.431, 5.498] |
| n-queens/variant-01 | 6.510 [5.520, 6.555] | 5.460 [5.437, 5.487] |
| n-queens/variant-02 | 121.225 [121.186, 122.873] | 122.325 [121.130, 122.648] |
| n-queens/variant-03 | 6.622 [6.515, 6.659] | 6.513 [5.465, 6.555] |
| n-queens/variant-04 | 6.551 [5.448, 6.620] | 5.563 [5.559, 6.618] |
| n-queens/variant-05 | 6.596 [5.525, 7.653] | 6.578 [5.502, 6.616] |
| n-queens/variant-06 | 6.600 [5.524, 6.630] | 5.574 [5.491, 6.620] |
| send-money/send-money | 16.054 [14.028, 16.088] | 15.008 [12.817, 16.086] |

Counters of report clauses: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 0 | 1 | 4 | 75404 | 2.277 |
| equality-generalized-tsp/02-larger | 0 | 1 | 8 | 130751 | 3.107 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 0 | 0 | 0 | 4096 | 1.331 |
| variant-01/01-basic | 0 | 1 | 2 | 5540 | 1.422 |
| variant-01/02-start-equals-end | 0 | 1 | 1 | 8051 | 1.473 |
| variant-01/03-zero-cost-detour | 0 | 1 | 2 | 4274 | 1.303 |
| variant-01/04-no-path | 0 | 0 | 0 | 1865 | 1.546 |
| variant-01/05-multi-path | 0 | 1 | 3 | 18700 | 1.691 |
| variant-01/06-layered-dag | 0 | 1 | 27 | 344578 | 4.556 |
| variant-01/07-cycles | 0 | 1 | 5 | 114322 | 2.597 |
| variant-01/08-negative-weights | 0 | 1 | 1 | 28915 | 2.403 |
| variant-02/01-basic | 0 | 1 | 2 | 5904 | 1.735 |
| variant-02/02-start-equals-end | 0 | 1 | 1 | 10761 | 1.678 |
| variant-02/03-before-forces-detour | 0 | 1 | 1 | 7017 | 1.710 |
| variant-02/04-after-forces-extension | 0 | 1 | 1 | 6405 | 1.657 |
| variant-02/05-ordering-unsat | 0 | 0 | 0 | 2519 | 1.652 |
| variant-02/06-layered-dag-before | 0 | 2 | 9 | 239298 | 4.053 |
| variant-02/07-layered-dag-before-after | 0 | 1 | 7 | 209967 | 3.838 |
| variant-02/08-tie-break-under-ordering | 0 | 1 | 2 | 10614 | 1.757 |
| variant-02/09-negative-weights | 0 | 1 | 1 | 32506 | 2.087 |
| variant-03/01-basic | 0 | 1 | 2 | 5734 | 1.741 |
| variant-03/02-start-equals-end | 0 | 1 | 1 | 9929 | 1.681 |
| variant-03/03-budget-forces-detour | 0 | 1 | 1 | 6301 | 1.695 |
| variant-03/04-cost-at-cap-allowed | 0 | 1 | 1 | 3269 | 1.854 |
| variant-03/05-budget-unsat | 0 | 0 | 0 | 16082 | 1.816 |
| variant-03/06-layered-dag-cap | 0 | 2 | 25 | 611524 | 6.179 |
| variant-03/07-layered-dag-tight-cap | 0 | 4 | 6 | 363131 | 6.123 |
| variant-03/08-tie-break-under-cap | 0 | 1 | 2 | 12810 | 2.104 |
| variant-03/09-negative-weights | 0 | 1 | 1 | 32853 | 2.118 |
| variant-04/01-basic | 0 | 1 | 2 | 6098 | 2.154 |
| variant-04/02-start-equals-end | 0 | 1 | 1 | 12639 | 2.018 |
| variant-04/03-before-forces-detour-within-budget | 0 | 1 | 1 | 8170 | 1.785 |
| variant-04/04-ordering-violates-budget | 0 | 0 | 0 | 6098 | 1.855 |
| variant-04/05-after-and-budget-interact | 0 | 1 | 1 | 13637 | 2.023 |
| variant-04/06-layered-dag-ordering-cap | 0 | 2 | 8 | 363501 | 5.367 |
| variant-04/07-layered-dag-combined | 0 | 1 | 5 | 313657 | 5.641 |
| variant-04/08-tie-break-under-ordering-and-cap | 0 | 1 | 2 | 27409 | 2.287 |
| variant-04/09-negative-weights | 0 | 1 | 1 | 36453 | 2.236 |
| variant-01/01-basic | 0 | 1 | 4 | 5980 | 1.031 |
| variant-01/02-agent-reuse | 0 | 1 | 8 | 15903 | 1.253 |
| variant-01/03-selective-compatibility | 0 | 1 | 4 | 7209 | 1.131 |
| variant-01/04-no-compatible-agent-unsat | 0 | 0 | 0 | 2289 | 0.890 |
| variant-01/05-larger-mix | 0 | 1 | 64 | 182539 | 2.324 |
| variant-02/01-basic | 0 | 1 | 4 | 14404 | 1.446 |
| variant-02/02-makespan-tiebreak | 0 | 3 | 8 | 25447 | 1.877 |
| variant-02/03-cost-dominates | 0 | 1 | 8 | 29126 | 1.640 |
| variant-02/04-no-compatible-agent-unsat | 0 | 0 | 0 | 4725 | 1.585 |
| variant-02/05-larger-mix | 0 | 1 | 67 | 844697 | 6.107 |
| variant-03/01-basic | 0 | 2 | 2 | 7431 | 2.051 |
| variant-03/02-multiple-groups | 0 | 4 | 4 | 17609 | 1.996 |
| variant-03/03-mixed-grouped-ungrouped | 0 | 2 | 4 | 11285 | 1.601 |
| variant-03/04-incompatible-group-unsat | 0 | 0 | 0 | 2497 | 1.270 |
| variant-03/05-larger-mix | 0 | 1 | 27 | 51564 | 2.081 |
| variant-04/01-basic | 0 | 81 | 124 | 875111 | 6.980 |
| variant-04/02-precedence | 0 | 3 | 12 | 75546 | 3.981 |
| variant-04/03-agent-serialization | 0 | 30 | 30 | 253900 | 4.193 |
| variant-04/04-window-too-tight-unsat | 0 | 0 | 0 | 8883 | 2.887 |
| variant-04/05-larger-mix | 0 | 1176 | 1208 | 23292550 | 89.857 |
| variant-01/01-basic | 0 | 1 | 2 | 45803 | 2.033 |
| variant-01/02-multiple-tours | 0 | 2 | 9 | 217292 | 3.056 |
| variant-01/03-asymmetric | 0 | 1 | 9 | 326714 | 4.003 |
| variant-01/04-subtour-unsat | 0 | 0 | 0 | 4623 | 1.909 |
| variant-01/05-ring | 0 | 2 | 44 | 1422699 | 7.448 |
| variant-02/01-basic | 0 | 2 | 2 | 50389 | 2.296 |
| variant-02/02-single-salesman | 0 | 1 | 2 | 32172 | 1.752 |
| variant-02/03-too-many-salesmen-unsat | 0 | 0 | 0 | 6906 | 1.363 |
| variant-02/04-equal-cost-split | 0 | 2 | 2 | 40903 | 1.984 |
| variant-02/05-larger-asymmetric | 0 | 2 | 2 | 60140 | 2.171 |
| variant-02/06-unreachable-edge | 0 | 2 | 2 | 69881 | 2.439 |
| variant-03/01-basic | 0 | 1 | 1 | 26134 | 1.910 |
| variant-03/02-single-salesman | 0 | 1 | 2 | 30993 | 1.751 |
| variant-03/03-depot-crossing-unsat | 0 | 0 | 0 | 6761 | 1.407 |
| variant-03/04-equal-cost-split | 0 | 2 | 2 | 70632 | 3.071 |
| variant-03/05-larger-three-depots | 0 | 1 | 1 | 73646 | 3.744 |
| variant-03/06-unreachable-edge | 0 | 1 | 1 | 44796 | 2.378 |
| variant-04/01-basic | 0 | 1 | 1 | 46886 | 2.983 |
| variant-04/02-single-salesman | 0 | 1 | 1 | 31547 | 2.443 |
| variant-04/03-window-too-tight-unsat | 0 | 0 | 0 | 6641 | 2.129 |
| variant-04/04-depot-window-too-tight-unsat | 0 | 0 | 0 | 4430 | 1.957 |
| variant-04/05-three-depots-asymmetric-times | 0 | 1 | 1 | 125685 | 4.580 |
| variant-04/06-unreachable-edge | 0 | 1 | 1 | 98235 | 4.252 |
| variant-05/01-basic | 0 | 1 | 1 | 46894 | 2.804 |
| variant-05/02-tight-bound-exact | 0 | 1 | 1 | 23047 | 2.338 |
| variant-05/03-revisit-too-tight-unsat | 0 | 0 | 0 | 6826 | 2.261 |
| variant-05/04-depot-and-vertex-revisits | 0 | 1 | 1 | 23457 | 2.382 |
| variant-05/05-three-depots-mixed | 0 | 1 | 1 | 126050 | 4.756 |
| variant-05/06-unreachable-edge | 0 | 1 | 1 | 98235 | 4.572 |
| n-queens/variant-01 | 0 | 92 | 92 | 734652 | 6.372 |
| n-queens/variant-02 | 0 | 92 | 92 | 18761080 | 85.902 |
| n-queens/variant-03 | 0 | 92 | 92 | 675790 | 6.353 |
| n-queens/variant-04 | 0 | 92 | 92 | 574291 | 4.274 |
| n-queens/variant-05 | 0 | 92 | 92 | 872980 | 6.802 |
| n-queens/variant-06 | 0 | 92 | 92 | 1004609 | 8.094 |
| send-money/send-money | 0 | 1 | 1 | 349198 | 7.936 |

Against the reference: report regions, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=1, completion workers=4, batch=64; search default): faster on 5 of 94 cells where both passed (5.3%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/03-agent-serialization | 8.150 | 28.818 | 0.283 | 0.938 | 0.484 | 0.232 | 24.000 | 0.000 |
| variant-04/01-basic | 9.901 | 26.748 | 0.370 | 0.982 | 1.097 | 0.955 | 18.000 | 4.000 |
| variant-04/05-larger-mix | 82.590 | 150.208 | 0.550 | 3.252 | 30.854 | 25.900 | 86.000 | 59.000 |
| send-money/send-money | 12.655 | 16.054 | 0.788 | 2.608 | 3.530 | 0.317 | 10.000 | 1.000 |
| variant-02/04-equal-cost-split | 5.154 | 5.430 | 0.949 | 0.308 | 0.052 | 0.018 | 1.000 | 0.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-02 | 121.596 | 121.225 | 1.003 | 2.407 | 111.587 | 2.352 | 1.000 | 115.000 |
| variant-02/03-before-forces-detour | 5.572 | 5.497 | 1.014 | 0.213 | 0.036 | 0.013 | 1.000 | 0.000 |
| variant-01/02-agent-reuse | 4.551 | 4.451 | 1.023 | 0.151 | 0.038 | 0.020 | 0.000 | 0.000 |
| variant-03/03-budget-forces-detour | 5.709 | 5.467 | 1.044 | 0.213 | 0.034 | 0.012 | 1.000 | 0.000 |
| variant-04/09-negative-weights | 5.749 | 5.496 | 1.046 | 0.299 | 0.051 | 0.010 | 1.000 | 0.000 |
| variant-02/06-unreachable-edge | 5.782 | 5.458 | 1.059 | 0.501 | 0.069 | 0.025 | 1.000 | 0.000 |
| variant-01/01-basic | 4.730 | 4.442 | 1.065 | 0.164 | 0.030 | 0.012 | 1.000 | 0.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 5.861 | 5.495 | 1.066 | 0.434 | 0.081 | 0.033 | 1.000 | 0.000 |
| variant-03/04-equal-cost-split | 5.845 | 5.472 | 1.068 | 0.456 | 0.072 | 0.025 | 1.000 | 0.000 |
| equality-generalized-tsp/01-basic | 5.946 | 5.548 | 1.072 | 0.374 | 0.069 | 0.026 | 1.000 | 0.000 |
| variant-01/01-basic | 4.847 | 4.500 | 1.077 | 0.265 | 0.059 | 0.027 | 1.000 | 0.000 |
| variant-01/01-basic | 4.851 | 4.453 | 1.089 | 0.138 | 0.031 | 0.013 | 0.000 | 0.000 |
| variant-01/03-zero-cost-detour | 4.922 | 4.446 | 1.107 | 0.154 | 0.025 | 0.010 | 1.000 | 0.000 |
| variant-01/04-subtour-unsat | 4.875 | 4.395 | 1.109 | 0.177 | 0.023 | 0.010 | 0.000 | 0.000 |
| variant-01/05-multi-path | 5.034 | 4.489 | 1.121 | 0.273 | 0.057 | 0.023 | 1.000 | 0.000 |
| variant-03/04-cost-at-cap-allowed | 5.072 | 4.447 | 1.141 | 0.175 | 0.022 | 0.008 | 1.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 5.085 | 4.440 | 1.145 | 0.337 | 0.078 | 0.042 | 1.000 | 0.000 |
| variant-02/02-single-salesman | 5.168 | 4.410 | 1.172 | 0.243 | 0.044 | 0.016 | 1.000 | 0.000 |
| variant-02/03-too-many-salesmen-unsat | 5.212 | 4.445 | 1.172 | 0.281 | 0.035 | 0.012 | 1.000 | 0.000 |
| variant-02/02-start-equals-end | 5.212 | 4.433 | 1.176 | 0.167 | 0.026 | 0.010 | 1.000 | 0.000 |
| variant-04/02-precedence | 7.771 | 6.593 | 1.179 | 0.614 | 0.200 | 0.089 | 2.000 | 0.000 |
| variant-02/07-layered-dag-before-after | 7.756 | 6.572 | 1.180 | 1.113 | 0.502 | 0.223 | 3.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 9.028 | 7.617 | 1.185 | 1.789 | 0.972 | 0.385 | 3.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 5.262 | 4.435 | 1.187 | 0.116 | 0.017 | 0.005 | 0.000 | 0.000 |
| variant-03/02-start-equals-end | 5.174 | 4.349 | 1.190 | 0.157 | 0.021 | 0.009 | 1.000 | 0.000 |
| variant-01/04-no-path | 5.300 | 4.429 | 1.196 | 0.171 | 0.020 | 0.006 | 1.000 | 0.000 |
| variant-03/05-larger-three-depots | 6.587 | 5.497 | 1.198 | 0.832 | 0.081 | 0.026 | 1.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 6.610 | 5.472 | 1.208 | 0.311 | 0.061 | 0.011 | 1.000 | 0.000 |
| variant-04/01-basic | 5.335 | 4.375 | 1.220 | 0.203 | 0.034 | 0.013 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 5.456 | 4.443 | 1.228 | 0.190 | 0.040 | 0.019 | 0.000 | 0.000 |
| variant-02/01-basic | 5.456 | 4.441 | 1.229 | 0.279 | 0.058 | 0.024 | 1.000 | 0.000 |
| variant-02/03-cost-dominates | 5.477 | 4.449 | 1.231 | 0.318 | 0.078 | 0.039 | 1.000 | 0.000 |
| variant-01/02-multiple-tours | 5.441 | 4.407 | 1.235 | 0.394 | 0.180 | 0.076 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 5.519 | 4.457 | 1.238 | 0.223 | 0.050 | 0.023 | 1.000 | 0.000 |
| variant-01/03-selective-compatibility | 5.533 | 4.454 | 1.242 | 0.167 | 0.037 | 0.015 | 0.000 | 0.000 |
| variant-02/05-larger-asymmetric | 5.509 | 4.425 | 1.245 | 0.434 | 0.066 | 0.022 | 1.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 5.535 | 4.443 | 1.246 | 0.241 | 0.047 | 0.017 | 1.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 5.533 | 4.433 | 1.248 | 0.273 | 0.040 | 0.009 | 1.000 | 0.000 |
| variant-02/05-ordering-unsat | 5.566 | 4.437 | 1.254 | 0.165 | 0.051 | 0.012 | 1.000 | 0.000 |
| variant-04/02-start-equals-end | 5.590 | 4.436 | 1.260 | 0.207 | 0.026 | 0.010 | 1.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 5.544 | 4.399 | 1.260 | 0.295 | 0.033 | 0.012 | 1.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 5.749 | 4.560 | 1.261 | 0.273 | 0.025 | 0.008 | 1.000 | 0.000 |
| variant-02/01-basic | 5.564 | 4.398 | 1.265 | 0.173 | 0.033 | 0.012 | 1.000 | 0.000 |
| variant-03/01-basic | 5.614 | 4.435 | 1.266 | 0.310 | 0.050 | 0.017 | 1.000 | 0.000 |
| variant-01/08-negative-weights | 5.617 | 4.437 | 1.266 | 0.235 | 0.042 | 0.008 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 5.680 | 4.439 | 1.280 | 0.264 | 0.038 | 0.013 | 1.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 7.015 | 5.463 | 1.284 | 0.381 | 0.038 | 0.015 | 1.000 | 0.000 |
| variant-03/01-basic | 5.734 | 4.449 | 1.289 | 0.207 | 0.030 | 0.012 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 5.705 | 4.421 | 1.291 | 0.263 | 0.044 | 0.015 | 1.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 5.699 | 4.397 | 1.296 | 0.297 | 0.028 | 0.010 | 1.000 | 0.000 |
| variant-05/02-tight-bound-exact | 5.798 | 4.430 | 1.309 | 0.370 | 0.039 | 0.015 | 1.000 | 0.000 |
| variant-02/09-negative-weights | 5.864 | 4.441 | 1.320 | 0.278 | 0.052 | 0.009 | 1.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 4.428 | 3.338 | 1.327 | 0.116 | 0.014 | 0.005 | 0.000 | 0.000 |
| variant-02/01-basic | 5.843 | 4.402 | 1.327 | 0.363 | 0.059 | 0.022 | 1.000 | 0.000 |
| variant-02/04-after-forces-extension | 5.915 | 4.436 | 1.333 | 0.231 | 0.032 | 0.011 | 1.000 | 0.000 |
| variant-01/02-start-equals-end | 5.851 | 4.375 | 1.337 | 0.146 | 0.046 | 0.014 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 5.875 | 4.390 | 1.338 | 0.305 | 0.050 | 0.017 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 5.930 | 4.430 | 1.339 | 0.473 | 0.051 | 0.018 | 1.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 5.902 | 4.407 | 1.339 | 0.381 | 0.035 | 0.012 | 1.000 | 0.000 |
| variant-01/07-cycles | 5.945 | 4.433 | 1.341 | 0.351 | 0.090 | 0.028 | 1.000 | 0.000 |
| variant-03/05-larger-mix | 5.989 | 4.445 | 1.347 | 0.266 | 0.097 | 0.090 | 1.000 | 0.000 |
| n-queens/variant-04 | 8.860 | 6.551 | 1.352 | 0.691 | 2.088 | 0.809 | 1.000 | 1.000 |
| variant-03/05-budget-unsat | 6.055 | 4.433 | 1.366 | 0.365 | 0.060 | 0.021 | 1.000 | 0.000 |
| variant-04/01-basic | 6.106 | 4.450 | 1.372 | 0.608 | 0.062 | 0.023 | 1.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 6.115 | 4.448 | 1.375 | 0.267 | 0.048 | 0.019 | 1.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.756 | 3.448 | 1.380 | 0.180 | 0.027 | 0.011 | 0.000 | 0.000 |
| variant-03/06-unreachable-edge | 6.191 | 4.449 | 1.392 | 0.523 | 0.062 | 0.021 | 1.000 | 0.000 |
| variant-01/06-layered-dag | 7.690 | 5.452 | 1.410 | 0.992 | 0.752 | 0.403 | 2.000 | 0.000 |
| variant-04/07-layered-dag-combined | 9.330 | 6.599 | 1.414 | 1.805 | 0.855 | 0.317 | 2.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 6.350 | 4.489 | 1.414 | 0.743 | 0.034 | 0.012 | 1.000 | 0.000 |
| variant-03/09-negative-weights | 6.309 | 4.422 | 1.427 | 0.293 | 0.048 | 0.009 | 1.000 | 0.000 |
| variant-05/05-three-depots-mixed | 7.888 | 5.478 | 1.440 | 1.536 | 0.135 | 0.062 | 1.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 9.945 | 6.645 | 1.497 | 1.689 | 1.671 | 0.294 | 2.000 | 0.000 |
| n-queens/variant-03 | 10.031 | 6.622 | 1.515 | 1.821 | 2.252 | 1.123 | 1.000 | 1.000 |
| n-queens/variant-01 | 9.865 | 6.510 | 1.515 | 1.844 | 2.022 | 1.023 | 1.000 | 1.000 |
| variant-01/05-larger-mix | 6.743 | 4.430 | 1.522 | 0.247 | 0.349 | 0.181 | 0.000 | 0.000 |
| variant-01/03-asymmetric | 6.792 | 4.445 | 1.528 | 0.533 | 0.208 | 0.109 | 1.000 | 0.000 |
| equality-generalized-tsp/02-larger | 6.924 | 4.435 | 1.561 | 0.605 | 0.220 | 0.086 | 1.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 8.724 | 5.502 | 1.586 | 1.546 | 0.152 | 0.054 | 1.000 | 0.000 |
| variant-02/06-layered-dag-before | 8.734 | 5.474 | 1.595 | 1.137 | 0.628 | 0.246 | 2.000 | 0.000 |
| variant-05/01-basic | 7.178 | 4.411 | 1.627 | 0.600 | 0.065 | 0.024 | 1.000 | 0.000 |
| variant-03/06-layered-dag-cap | 10.996 | 6.647 | 1.654 | 1.640 | 1.335 | 0.655 | 3.000 | 0.000 |
| variant-05/06-unreachable-edge | 7.665 | 4.601 | 1.666 | 1.185 | 0.115 | 0.043 | 1.000 | 0.000 |
| variant-03/01-basic | 5.575 | 3.326 | 1.676 | 0.151 | 0.032 | 0.013 | 0.000 | 0.000 |
| variant-04/06-unreachable-edge | 7.732 | 4.399 | 1.758 | 1.191 | 0.117 | 0.045 | 1.000 | 0.000 |
| n-queens/variant-05 | 11.994 | 6.596 | 1.818 | 1.045 | 2.603 | 1.174 | 1.000 | 1.000 |
| n-queens/variant-06 | 12.770 | 6.600 | 1.935 | 1.717 | 2.994 | 1.408 | 1.000 | 1.000 |
| variant-01/05-ring | 8.670 | 4.434 | 1.955 | 0.662 | 0.773 | 0.313 | 1.000 | 0.000 |
| variant-02/05-larger-mix | 8.893 | 4.460 | 1.994 | 0.728 | 1.163 | 0.442 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/03-agent-serialization | 11.3 | 12.2 | n/a |
| variant-04/01-basic | 11.7 | 13.3 | n/a |
| variant-04/05-larger-mix | 13.4 | 23.7 | n/a |
| send-money/send-money | 12.3 | 12.8 | n/a |
| variant-02/04-equal-cost-split | 10.8 | 10.3 | n/a |
| n-queens/variant-02 | 13.0 | 12.8 | n/a |
| variant-02/03-before-forces-detour | 10.9 | 10.6 | n/a |
| variant-01/02-agent-reuse | 10.7 | 10.3 | n/a |
| variant-03/03-budget-forces-detour | 10.4 | 10.5 | n/a |
| variant-04/09-negative-weights | 11.1 | 10.6 | n/a |
| variant-02/06-unreachable-edge | 10.9 | 10.5 | n/a |
| variant-01/01-basic | 10.6 | 10.3 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 11.0 | 10.6 | n/a |
| variant-03/04-equal-cost-split | 10.7 | 10.2 | n/a |
| equality-generalized-tsp/01-basic | 10.4 | 10.6 | n/a |
| variant-01/01-basic | 10.7 | 10.5 | n/a |
| variant-01/01-basic | 10.2 | 10.3 | n/a |
| variant-01/03-zero-cost-detour | 10.7 | 10.4 | n/a |
| variant-01/04-subtour-unsat | 10.5 | 10.1 | n/a |
| variant-01/05-multi-path | 10.7 | 10.6 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.6 | 10.5 | n/a |
| variant-02/02-makespan-tiebreak | 10.6 | 10.3 | n/a |
| variant-02/02-single-salesman | 10.7 | 10.4 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.9 | 10.3 | n/a |
| variant-02/02-start-equals-end | 10.8 | 10.4 | n/a |
| variant-04/02-precedence | 11.3 | 10.6 | n/a |
| variant-02/07-layered-dag-before-after | 11.5 | 10.6 | n/a |
| variant-04/06-layered-dag-ordering-cap | 11.9 | 10.9 | n/a |
| variant-03/04-incompatible-group-unsat | 10.5 | 10.1 | n/a |
| variant-03/02-start-equals-end | 10.4 | 10.6 | n/a |
| variant-01/04-no-path | 10.7 | 10.5 | n/a |
| variant-03/05-larger-three-depots | 10.8 | 10.6 | n/a |
| variant-04/04-window-too-tight-unsat | 11.2 | 10.5 | n/a |
| variant-04/01-basic | 10.7 | 10.4 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.6 | 10.4 | n/a |
| variant-02/01-basic | 10.4 | 10.2 | n/a |
| variant-02/03-cost-dominates | 10.8 | 10.4 | n/a |
| variant-01/02-multiple-tours | 10.8 | 10.6 | n/a |
| variant-03/02-multiple-groups | 10.5 | 10.5 | n/a |
| variant-01/03-selective-compatibility | 10.4 | 10.2 | n/a |
| variant-02/05-larger-asymmetric | 10.5 | 10.6 | n/a |
| variant-02/08-tie-break-under-ordering | 10.9 | 10.5 | n/a |
| variant-04/04-ordering-violates-budget | 10.7 | 10.5 | n/a |
| variant-02/05-ordering-unsat | 11.0 | 10.3 | n/a |
| variant-04/02-start-equals-end | 10.8 | 10.5 | n/a |
| variant-03/03-depot-crossing-unsat | 10.9 | 10.5 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.5 | 10.3 | n/a |
| variant-02/01-basic | 10.5 | 10.5 | n/a |
| variant-03/01-basic | 10.6 | 10.4 | n/a |
| variant-01/08-negative-weights | 10.7 | 10.3 | n/a |
| variant-04/03-before-forces-detour-within-budget | 11.0 | 10.5 | n/a |
| variant-05/04-depot-and-vertex-revisits | 10.9 | 10.4 | n/a |
| variant-03/01-basic | 10.6 | 10.5 | n/a |
| variant-03/02-single-salesman | 10.4 | 10.3 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 10.7 | 10.5 | n/a |
| variant-05/02-tight-bound-exact | 11.1 | 10.4 | n/a |
| variant-02/09-negative-weights | 10.5 | 10.5 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.3 | 10.4 | n/a |
| variant-02/01-basic | 10.6 | 10.5 | n/a |
| variant-02/04-after-forces-extension | 11.0 | 10.2 | n/a |
| variant-01/02-start-equals-end | 10.5 | 10.5 | n/a |
| variant-04/05-after-and-budget-interact | 10.9 | 10.3 | n/a |
| variant-04/02-single-salesman | 11.0 | 10.5 | n/a |
| variant-04/03-window-too-tight-unsat | 10.9 | 10.5 | n/a |
| variant-01/07-cycles | 10.5 | 10.6 | n/a |
| variant-03/05-larger-mix | 10.6 | 10.3 | n/a |
| n-queens/variant-04 | 11.0 | 10.5 | n/a |
| variant-03/05-budget-unsat | 10.6 | 10.3 | n/a |
| variant-04/01-basic | 11.1 | 10.5 | n/a |
| variant-03/08-tie-break-under-cap | 10.7 | 10.6 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.6 | 10.3 | n/a |
| variant-03/06-unreachable-edge | 10.9 | 10.5 | n/a |
| variant-01/06-layered-dag | 11.4 | 10.7 | n/a |
| variant-04/07-layered-dag-combined | 12.3 | 10.9 | n/a |
| variant-05/03-revisit-too-tight-unsat | 11.2 | 10.4 | n/a |
| variant-03/09-negative-weights | 10.7 | 10.6 | n/a |
| variant-05/05-three-depots-mixed | 11.1 | 10.7 | n/a |
| variant-03/07-layered-dag-tight-cap | 12.1 | 10.9 | n/a |
| n-queens/variant-03 | 10.9 | 10.5 | n/a |
| n-queens/variant-01 | 10.7 | 10.5 | n/a |
| variant-01/05-larger-mix | 10.7 | 10.5 | n/a |
| variant-01/03-asymmetric | 11.2 | 10.8 | n/a |
| equality-generalized-tsp/02-larger | 11.1 | 10.5 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.4 | 10.6 | n/a |
| variant-02/06-layered-dag-before | 11.5 | 10.7 | n/a |
| variant-05/01-basic | 11.1 | 10.5 | n/a |
| variant-03/06-layered-dag-cap | 11.6 | 10.9 | n/a |
| variant-05/06-unreachable-edge | 11.0 | 10.8 | n/a |
| variant-03/01-basic | 10.6 | 10.3 | n/a |
| variant-04/06-unreachable-edge | 11.5 | 10.5 | n/a |
| n-queens/variant-05 | 11.3 | 10.7 | n/a |
| n-queens/variant-06 | 11.2 | 10.6 | n/a |
| variant-01/05-ring | 11.6 | 10.7 | n/a |
| variant-02/05-larger-mix | 11.4 | 10.5 | n/a |

Against the reference: report clauses, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=1, completion workers=4, batch=64; search clauses): faster on 6 of 94 cells where both passed (6.3%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/03-agent-serialization | 7.963 | 28.780 | 0.277 | 0.933 | 0.647 | 0.240 | 24.000 | 0.000 |
| variant-04/01-basic | 10.888 | 25.623 | 0.425 | 0.983 | 1.697 | 0.908 | 18.000 | 4.000 |
| variant-04/05-larger-mix | 93.894 | 149.570 | 0.628 | 3.283 | 43.354 | 25.489 | 85.000 | 58.000 |
| n-queens/variant-02 | 89.608 | 122.325 | 0.733 | 2.480 | 80.173 | 2.197 | 1.000 | 116.000 |
| send-money/send-money | 11.407 | 15.008 | 0.760 | 3.232 | 1.854 | 0.209 | 9.000 | 1.000 |
| variant-03/03-depot-crossing-unsat | 4.897 | 5.507 | 0.889 | 0.289 | 0.026 | 0.011 | 1.000 | 0.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/04-ordering-violates-budget | 5.538 | 5.496 | 1.008 | 0.287 | 0.028 | 0.010 | 1.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 5.527 | 5.483 | 1.008 | 0.300 | 0.021 | 0.011 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 5.613 | 5.480 | 1.024 | 0.357 | 0.062 | 0.013 | 1.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 4.546 | 4.438 | 1.024 | 0.114 | 0.011 | 0.005 | 0.000 | 0.000 |
| variant-01/01-basic | 4.698 | 4.446 | 1.057 | 0.163 | 0.041 | 0.025 | 1.000 | 0.000 |
| variant-01/03-zero-cost-detour | 4.793 | 4.415 | 1.085 | 0.152 | 0.021 | 0.011 | 1.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 4.844 | 4.441 | 1.091 | 0.118 | 0.015 | 0.005 | 0.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 6.006 | 5.485 | 1.095 | 0.394 | 0.025 | 0.012 | 1.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 5.988 | 5.467 | 1.095 | 0.385 | 0.029 | 0.017 | 1.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 5.998 | 5.469 | 1.097 | 0.281 | 0.051 | 0.019 | 1.000 | 0.000 |
| variant-02/03-before-forces-detour | 6.014 | 5.469 | 1.100 | 0.213 | 0.029 | 0.013 | 1.000 | 0.000 |
| variant-01/02-start-equals-end | 4.915 | 4.404 | 1.116 | 0.159 | 0.019 | 0.026 | 1.000 | 0.000 |
| variant-03/06-unreachable-edge | 6.119 | 5.470 | 1.119 | 0.522 | 0.053 | 0.107 | 1.000 | 0.000 |
| variant-05/02-tight-bound-exact | 6.111 | 5.448 | 1.122 | 0.361 | 0.031 | 0.062 | 1.000 | 0.000 |
| variant-02/01-basic | 5.153 | 4.440 | 1.161 | 0.283 | 0.048 | 0.025 | 1.000 | 0.000 |
| variant-02/03-cost-dominates | 5.218 | 4.430 | 1.178 | 0.327 | 0.072 | 0.037 | 1.000 | 0.000 |
| variant-05/01-basic | 6.501 | 5.514 | 1.179 | 0.586 | 0.058 | 0.137 | 1.000 | 0.000 |
| variant-02/04-after-forces-extension | 5.298 | 4.490 | 1.180 | 0.236 | 0.029 | 0.011 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 5.201 | 4.404 | 1.181 | 0.194 | 0.039 | 0.018 | 0.000 | 0.000 |
| variant-03/04-cost-at-cap-allowed | 5.245 | 4.430 | 1.184 | 0.183 | 0.014 | 0.007 | 1.000 | 0.000 |
| variant-02/05-ordering-unsat | 5.204 | 4.395 | 1.184 | 0.201 | 0.015 | 0.005 | 1.000 | 0.000 |
| variant-04/02-precedence | 8.044 | 6.626 | 1.214 | 0.614 | 0.209 | 0.089 | 2.000 | 0.000 |
| variant-02/02-start-equals-end | 5.430 | 4.447 | 1.221 | 0.170 | 0.021 | 0.030 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 5.542 | 4.536 | 1.222 | 0.226 | 0.029 | 0.012 | 1.000 | 0.000 |
| variant-02/03-too-many-salesmen-unsat | 5.478 | 4.437 | 1.235 | 0.286 | 0.026 | 0.012 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 5.512 | 4.452 | 1.238 | 0.249 | 0.039 | 0.050 | 1.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.533 | 4.451 | 1.243 | 0.188 | 0.022 | 0.012 | 0.000 | 0.000 |
| variant-02/01-basic | 5.572 | 4.446 | 1.253 | 0.203 | 0.024 | 0.012 | 1.000 | 0.000 |
| variant-01/05-multi-path | 5.531 | 4.413 | 1.253 | 0.310 | 0.062 | 0.024 | 1.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 5.545 | 4.414 | 1.256 | 0.266 | 0.051 | 0.017 | 0.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 5.594 | 4.450 | 1.257 | 0.256 | 0.031 | 0.013 | 1.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 6.875 | 5.468 | 1.257 | 0.315 | 0.047 | 0.013 | 1.000 | 0.000 |
| variant-01/04-no-path | 5.615 | 4.429 | 1.268 | 0.163 | 0.013 | 0.006 | 0.000 | 0.000 |
| variant-01/03-selective-compatibility | 5.631 | 4.438 | 1.269 | 0.166 | 0.029 | 0.015 | 0.000 | 0.000 |
| variant-03/05-larger-mix | 5.651 | 4.427 | 1.277 | 0.280 | 0.082 | 0.082 | 1.000 | 0.000 |
| variant-01/06-layered-dag | 8.370 | 6.557 | 1.277 | 1.030 | 0.885 | 0.316 | 2.000 | 0.000 |
| variant-03/01-basic | 5.673 | 4.440 | 1.278 | 0.210 | 0.031 | 0.013 | 1.000 | 0.000 |
| variant-01/02-agent-reuse | 5.671 | 4.396 | 1.290 | 0.166 | 0.034 | 0.024 | 1.000 | 0.000 |
| variant-04/01-basic | 5.779 | 4.430 | 1.304 | 0.197 | 0.022 | 0.012 | 1.000 | 0.000 |
| variant-04/02-start-equals-end | 5.799 | 4.442 | 1.306 | 0.202 | 0.022 | 0.035 | 1.000 | 0.000 |
| variant-02/02-single-salesman | 5.821 | 4.455 | 1.306 | 0.255 | 0.038 | 0.071 | 1.000 | 0.000 |
| variant-01/01-basic | 6.008 | 4.551 | 1.320 | 0.256 | 0.061 | 0.106 | 1.000 | 0.000 |
| variant-01/08-negative-weights | 5.886 | 4.447 | 1.324 | 0.461 | 0.100 | 0.145 | 1.000 | 0.000 |
| variant-01/04-subtour-unsat | 5.857 | 4.425 | 1.324 | 0.420 | 0.024 | 0.011 | 0.000 | 0.000 |
| variant-03/09-negative-weights | 5.895 | 4.445 | 1.326 | 0.290 | 0.050 | 0.075 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 5.943 | 4.455 | 1.334 | 0.304 | 0.053 | 0.018 | 1.000 | 0.000 |
| variant-04/09-negative-weights | 5.912 | 4.392 | 1.346 | 0.312 | 0.052 | 0.070 | 1.000 | 0.000 |
| variant-02/01-basic | 6.049 | 4.473 | 1.352 | 0.351 | 0.056 | 0.131 | 1.000 | 0.000 |
| variant-03/01-basic | 5.997 | 4.433 | 1.353 | 0.301 | 0.036 | 0.165 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 5.998 | 4.409 | 1.360 | 0.228 | 0.049 | 0.022 | 1.000 | 0.000 |
| variant-02/09-negative-weights | 6.081 | 4.463 | 1.362 | 0.286 | 0.047 | 0.078 | 1.000 | 0.000 |
| variant-02/05-larger-asymmetric | 6.016 | 4.403 | 1.366 | 0.448 | 0.056 | 0.099 | 1.000 | 0.000 |
| variant-03/01-basic | 6.047 | 4.406 | 1.372 | 0.150 | 0.037 | 0.013 | 0.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 6.146 | 4.454 | 1.380 | 0.344 | 0.076 | 0.041 | 1.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 6.146 | 4.408 | 1.394 | 0.248 | 0.038 | 0.017 | 1.000 | 0.000 |
| variant-03/05-larger-three-depots | 7.662 | 5.491 | 1.396 | 1.347 | 0.068 | 0.189 | 1.000 | 0.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.282 | 4.454 | 1.410 | 0.442 | 0.087 | 0.033 | 1.000 | 0.000 |
| variant-02/07-layered-dag-before-after | 7.697 | 5.421 | 1.420 | 1.157 | 0.647 | 0.177 | 2.000 | 0.000 |
| variant-02/06-layered-dag-before | 7.907 | 5.511 | 1.435 | 1.154 | 0.724 | 0.175 | 2.000 | 0.000 |
| variant-01/07-cycles | 6.333 | 4.410 | 1.436 | 0.393 | 0.102 | 0.111 | 1.000 | 0.000 |
| variant-01/05-larger-mix | 6.354 | 4.419 | 1.438 | 0.259 | 0.443 | 0.154 | 0.000 | 0.000 |
| variant-02/06-unreachable-edge | 6.572 | 4.470 | 1.470 | 0.485 | 0.066 | 0.122 | 1.000 | 0.000 |
| variant-01/01-basic | 4.911 | 3.330 | 1.475 | 0.141 | 0.025 | 0.013 | 0.000 | 0.000 |
| variant-04/07-layered-dag-combined | 9.768 | 6.613 | 1.477 | 1.806 | 1.630 | 0.214 | 2.000 | 0.000 |
| variant-01/02-multiple-tours | 6.692 | 4.501 | 1.487 | 0.374 | 0.222 | 0.188 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 6.604 | 4.438 | 1.488 | 0.449 | 0.042 | 0.066 | 1.000 | 0.000 |
| equality-generalized-tsp/01-basic | 6.672 | 4.466 | 1.494 | 0.362 | 0.065 | 0.108 | 1.000 | 0.000 |
| variant-03/06-layered-dag-cap | 9.737 | 6.513 | 1.495 | 2.133 | 1.779 | 0.571 | 2.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 9.758 | 6.526 | 1.495 | 1.765 | 1.274 | 0.278 | 2.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 9.901 | 6.600 | 1.500 | 1.955 | 1.396 | 0.270 | 2.000 | 1.000 |
| variant-05/04-depot-and-vertex-revisits | 6.717 | 4.460 | 1.506 | 0.375 | 0.032 | 0.055 | 1.000 | 0.000 |
| equality-generalized-tsp/02-larger | 6.774 | 4.486 | 1.510 | 0.588 | 0.093 | 0.166 | 1.000 | 0.000 |
| n-queens/variant-04 | 8.445 | 5.563 | 1.518 | 0.711 | 1.648 | 0.694 | 1.000 | 1.000 |
| variant-02/04-equal-cost-split | 6.752 | 4.406 | 1.532 | 0.317 | 0.045 | 0.089 | 1.000 | 0.000 |
| n-queens/variant-03 | 10.084 | 6.513 | 1.548 | 2.465 | 1.533 | 1.062 | 1.000 | 1.000 |
| variant-03/04-equal-cost-split | 6.845 | 4.420 | 1.549 | 0.970 | 0.072 | 0.137 | 1.000 | 0.000 |
| n-queens/variant-05 | 10.309 | 6.578 | 1.567 | 1.004 | 2.150 | 1.084 | 1.000 | 1.000 |
| variant-05/06-unreachable-edge | 8.741 | 5.497 | 1.590 | 1.260 | 0.279 | 0.489 | 1.000 | 0.000 |
| variant-04/01-basic | 7.089 | 4.450 | 1.593 | 0.591 | 0.063 | 0.127 | 1.000 | 0.000 |
| variant-03/02-start-equals-end | 5.714 | 3.341 | 1.710 | 0.159 | 0.018 | 0.026 | 1.000 | 0.000 |
| variant-04/06-unreachable-edge | 8.075 | 4.470 | 1.806 | 1.201 | 0.146 | 0.237 | 1.000 | 0.000 |
| variant-02/05-larger-mix | 9.945 | 5.476 | 1.816 | 0.710 | 2.941 | 0.444 | 1.000 | 0.000 |
| n-queens/variant-01 | 10.455 | 5.460 | 1.915 | 1.825 | 1.938 | 0.983 | 1.000 | 1.000 |
| variant-04/05-three-depots-asymmetric-times | 8.513 | 4.399 | 1.935 | 1.521 | 0.200 | 0.342 | 1.000 | 0.000 |
| variant-01/03-asymmetric | 8.758 | 4.478 | 1.956 | 0.548 | 0.301 | 0.276 | 1.000 | 0.000 |
| variant-05/05-three-depots-mixed | 8.693 | 4.438 | 1.959 | 1.535 | 0.175 | 0.331 | 1.000 | 0.000 |
| variant-01/05-ring | 11.395 | 5.523 | 2.063 | 0.682 | 0.941 | 0.545 | 1.000 | 0.000 |
| n-queens/variant-06 | 12.306 | 5.574 | 2.208 | 1.065 | 2.557 | 1.269 | 1.000 | 1.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/03-agent-serialization | 11.3 | 12.5 | n/a |
| variant-04/01-basic | 11.8 | 13.2 | n/a |
| variant-04/05-larger-mix | 13.6 | 23.6 | n/a |
| n-queens/variant-02 | 11.3 | 13.0 | n/a |
| send-money/send-money | 12.4 | 12.5 | n/a |
| variant-03/03-depot-crossing-unsat | 10.7 | 10.4 | n/a |
| variant-04/04-ordering-violates-budget | 10.6 | 10.5 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 11.0 | 10.6 | n/a |
| variant-03/05-budget-unsat | 10.7 | 10.4 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.2 | 10.2 | n/a |
| variant-01/01-basic | 10.7 | 10.5 | n/a |
| variant-01/03-zero-cost-detour | 10.5 | 10.4 | n/a |
| variant-03/04-incompatible-group-unsat | 10.7 | 10.1 | n/a |
| variant-04/03-window-too-tight-unsat | 10.9 | 10.5 | n/a |
| variant-05/03-revisit-too-tight-unsat | 10.9 | 10.6 | n/a |
| variant-03/08-tie-break-under-cap | 10.6 | 10.5 | n/a |
| variant-02/03-before-forces-detour | 10.9 | 10.6 | n/a |
| variant-01/02-start-equals-end | 10.4 | 10.4 | n/a |
| variant-03/06-unreachable-edge | 11.0 | 10.6 | n/a |
| variant-05/02-tight-bound-exact | 11.1 | 10.5 | n/a |
| variant-02/01-basic | 10.6 | 10.3 | n/a |
| variant-02/03-cost-dominates | 10.5 | 10.3 | n/a |
| variant-05/01-basic | 10.9 | 10.5 | n/a |
| variant-02/04-after-forces-extension | 11.1 | 10.5 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.7 | 10.3 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.9 | 10.5 | n/a |
| variant-02/05-ordering-unsat | 10.7 | 10.5 | n/a |
| variant-04/02-precedence | 11.6 | 10.8 | n/a |
| variant-02/02-start-equals-end | 10.6 | 10.6 | n/a |
| variant-03/03-budget-forces-detour | 10.7 | 10.4 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.9 | 10.3 | n/a |
| variant-03/02-single-salesman | 10.7 | 10.5 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.6 | 10.5 | n/a |
| variant-02/01-basic | 10.5 | 10.4 | n/a |
| variant-01/05-multi-path | 10.5 | 10.4 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.2 | 10.4 | n/a |
| variant-04/03-before-forces-detour-within-budget | 10.9 | 10.5 | n/a |
| variant-04/04-window-too-tight-unsat | 11.6 | 10.5 | n/a |
| variant-01/04-no-path | 10.5 | 10.4 | n/a |
| variant-01/03-selective-compatibility | 10.5 | 10.2 | n/a |
| variant-03/05-larger-mix | 10.8 | 10.4 | n/a |
| variant-01/06-layered-dag | 11.2 | 10.8 | n/a |
| variant-03/01-basic | 10.6 | 10.3 | n/a |
| variant-01/02-agent-reuse | 10.1 | 10.5 | n/a |
| variant-04/01-basic | 11.0 | 10.5 | n/a |
| variant-04/02-start-equals-end | 10.9 | 10.5 | n/a |
| variant-02/02-single-salesman | 10.7 | 10.5 | n/a |
| variant-01/01-basic | 10.6 | 10.5 | n/a |
| variant-01/08-negative-weights | 10.4 | 10.4 | n/a |
| variant-01/04-subtour-unsat | 10.5 | 10.4 | n/a |
| variant-03/09-negative-weights | 11.0 | 10.6 | n/a |
| variant-04/05-after-and-budget-interact | 10.8 | 10.5 | n/a |
| variant-04/09-negative-weights | 11.0 | 10.4 | n/a |
| variant-02/01-basic | 10.5 | 10.6 | n/a |
| variant-03/01-basic | 10.4 | 10.4 | n/a |
| variant-03/02-multiple-groups | 10.6 | 10.5 | n/a |
| variant-02/09-negative-weights | 10.7 | 10.5 | n/a |
| variant-02/05-larger-asymmetric | 10.8 | 10.6 | n/a |
| variant-03/01-basic | 10.8 | 10.2 | n/a |
| variant-02/02-makespan-tiebreak | 10.7 | 10.4 | n/a |
| variant-02/08-tie-break-under-ordering | 10.8 | 10.5 | n/a |
| variant-03/05-larger-three-depots | 11.1 | 10.6 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 11.1 | 10.6 | n/a |
| variant-02/07-layered-dag-before-after | 11.1 | 10.8 | n/a |
| variant-02/06-layered-dag-before | 11.2 | 10.9 | n/a |
| variant-01/07-cycles | 10.7 | 10.5 | n/a |
| variant-01/05-larger-mix | 10.8 | 10.5 | n/a |
| variant-02/06-unreachable-edge | 10.9 | 10.5 | n/a |
| variant-01/01-basic | 10.4 | 10.2 | n/a |
| variant-04/07-layered-dag-combined | 11.9 | 10.9 | n/a |
| variant-01/02-multiple-tours | 11.3 | 10.6 | n/a |
| variant-04/02-single-salesman | 11.0 | 10.4 | n/a |
| equality-generalized-tsp/01-basic | 10.6 | 10.6 | n/a |
| variant-03/06-layered-dag-cap | 11.6 | 10.8 | n/a |
| variant-04/06-layered-dag-ordering-cap | 11.6 | 11.0 | n/a |
| variant-03/07-layered-dag-tight-cap | 11.5 | 11.0 | n/a |
| variant-05/04-depot-and-vertex-revisits | 11.2 | 10.6 | n/a |
| equality-generalized-tsp/02-larger | 10.9 | 10.4 | n/a |
| n-queens/variant-04 | 10.9 | 10.7 | n/a |
| variant-02/04-equal-cost-split | 10.7 | 10.5 | n/a |
| n-queens/variant-03 | 10.8 | 10.6 | n/a |
| variant-03/04-equal-cost-split | 10.6 | 10.5 | n/a |
| n-queens/variant-05 | 11.0 | 10.5 | n/a |
| variant-05/06-unreachable-edge | 11.5 | 10.7 | n/a |
| variant-04/01-basic | 11.2 | 10.5 | n/a |
| variant-03/02-start-equals-end | 11.1 | 10.3 | n/a |
| variant-04/06-unreachable-edge | 11.6 | 10.7 | n/a |
| variant-02/05-larger-mix | 11.2 | 10.5 | n/a |
| n-queens/variant-01 | 10.6 | 10.4 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.3 | 10.5 | n/a |
| variant-01/03-asymmetric | 11.4 | 10.4 | n/a |
| variant-05/05-three-depots-mixed | 11.6 | 10.6 | n/a |
| variant-01/05-ring | 11.8 | 10.8 | n/a |
| n-queens/variant-06 | 10.8 | 10.6 | n/a |
