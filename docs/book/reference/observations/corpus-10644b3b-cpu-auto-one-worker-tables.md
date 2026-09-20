Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=1, completion workers=4, batch=64.

Formula search method by report: regions=default; clauses=clauses;

| Cell | regions | clauses | clauses/regions | regions/reference | clauses/reference |
|---|---:|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 5.703 [5.665, 5.802] | 5.573 [5.467, 7.074] | 0.977 | 1.285 | 1.252 |
| equality-generalized-tsp/02-larger | 6.061 [5.850, 7.012] | 7.003 [6.686, 7.080] | 1.155 | 1.370 | 1.594 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.665 [4.425, 4.743] | 5.546 [5.111, 5.914] | 1.189 | 1.047 | 1.256 |
| variant-01/01-basic | 4.796 [4.402, 4.851] | 5.136 [5.079, 5.196] | 1.071 | 1.088 | 1.157 |
| variant-01/02-start-equals-end | 4.819 [4.369, 5.756] | 5.507 [4.534, 5.573] | 1.143 | 1.093 | 1.237 |
| variant-01/03-zero-cost-detour | 5.199 [4.373, 5.543] | 4.999 [4.407, 5.332] | 0.962 | 1.160 | 0.909 |
| variant-01/04-no-path | 4.986 [4.125, 5.550] | 4.832 [4.603, 5.103] | 0.969 | 1.124 | 0.883 |
| variant-01/05-multi-path | 4.978 [4.859, 4.978] | 5.484 [4.468, 5.664] | 1.102 | 1.132 | 1.244 |
| variant-01/06-layered-dag | 8.363 [7.678, 9.005] | 7.972 [7.851, 9.238] | 0.953 | 1.515 | 1.456 |
| variant-01/07-cycles | 5.460 [5.084, 5.987] | 7.045 [6.325, 7.181] | 1.290 | 1.245 | 1.287 |
| variant-01/08-negative-weights | 4.617 [4.558, 5.640] | 5.567 [5.212, 6.116] | 1.206 | 1.384 | 1.250 |
| variant-02/01-basic | 4.978 [4.400, 5.551] | 5.490 [5.053, 5.508] | 1.103 | 1.132 | 1.225 |
| variant-02/02-start-equals-end | 5.297 [5.053, 5.542] | 5.067 [4.704, 6.209] | 0.957 | 1.201 | 1.140 |
| variant-02/03-before-forces-detour | 5.051 [4.823, 5.538] | 5.164 [4.933, 6.049] | 1.023 | 1.538 | 1.169 |
| variant-02/04-after-forces-extension | 5.050 [5.030, 5.319] | 5.849 [5.843, 5.879] | 1.158 | 1.134 | 1.314 |
| variant-02/05-ordering-unsat | 4.737 [4.451, 5.241] | 5.580 [4.630, 5.657] | 1.178 | 1.066 | 1.261 |
| variant-02/06-layered-dag-before | 7.082 [6.667, 7.693] | 7.969 [6.657, 8.036] | 1.125 | 1.286 | 1.222 |
| variant-02/07-layered-dag-before-after | 7.686 [6.789, 7.835] | 7.699 [7.088, 7.900] | 1.002 | 1.763 | 1.399 |
| variant-02/08-tie-break-under-ordering | 5.545 [5.444, 5.645] | 5.543 [4.797, 6.121] | 1.000 | 1.252 | 1.249 |
| variant-02/09-negative-weights | 5.437 [4.905, 5.520] | 6.307 [4.895, 6.597] | 1.160 | 1.230 | 1.434 |
| variant-03/01-basic | 5.531 [4.799, 5.741] | 5.568 [5.131, 6.049] | 1.007 | 1.250 | 1.254 |
| variant-03/02-start-equals-end | 5.050 [4.831, 5.704] | 5.717 [5.523, 5.770] | 1.132 | 1.148 | 1.307 |
| variant-03/03-budget-forces-detour | 5.114 [4.527, 5.496] | 5.498 [5.108, 5.628] | 1.075 | 1.529 | 1.242 |
| variant-03/04-cost-at-cap-allowed | 5.658 [5.487, 6.589] | 5.359 [5.135, 5.524] | 0.947 | 1.276 | 1.215 |
| variant-03/05-budget-unsat | 6.191 [5.524, 6.657] | 5.549 [4.718, 6.268] | 0.896 | 1.426 | 1.250 |
| variant-03/06-layered-dag-cap | 9.331 [9.310, 9.826] | 10.346 [9.789, 11.304] | 1.109 | 1.226 | 1.563 |
| variant-03/07-layered-dag-tight-cap | 9.040 [8.960, 10.517] | 9.481 [8.794, 10.059] | 1.049 | 1.378 | 1.718 |
| variant-03/08-tie-break-under-cap | 5.547 [4.676, 5.673] | 5.699 [4.671, 6.367] | 1.027 | 1.249 | 1.052 |
| variant-03/09-negative-weights | 5.553 [5.523, 7.931] | 5.852 [5.548, 6.062] | 1.054 | 1.260 | 1.318 |
| variant-04/01-basic | 5.539 [4.624, 5.566] | 4.700 [4.620, 5.439] | 0.849 | 1.252 | 1.058 |
| variant-04/02-start-equals-end | 5.827 [4.800, 5.884] | 6.187 [6.117, 6.615] | 1.062 | 1.323 | 1.392 |
| variant-04/03-before-forces-detour-within-budget | 5.975 [5.264, 6.116] | 5.836 [5.473, 6.608] | 0.977 | 1.355 | 1.315 |
| variant-04/04-ordering-violates-budget | 5.710 [5.517, 5.812] | 5.630 [5.542, 6.061] | 0.986 | 1.287 | 1.278 |
| variant-04/05-after-and-budget-interact | 5.621 [5.503, 6.685] | 5.589 [5.584, 5.750] | 0.994 | 1.024 | 1.267 |
| variant-04/06-layered-dag-ordering-cap | 9.441 [9.261, 10.146] | 9.950 [9.140, 10.479] | 1.054 | 1.439 | 1.509 |
| variant-04/07-layered-dag-combined | 9.129 [8.878, 11.925] | 9.356 [9.144, 9.716] | 1.025 | 1.406 | 1.426 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.069 [5.921, 6.242] | 5.994 [5.419, 6.053] | 0.988 | 1.375 | 1.349 |
| variant-04/09-negative-weights | 6.217 [4.334, 6.370] | 5.938 [5.639, 6.733] | 0.955 | 1.134 | 1.306 |
| variant-01/01-basic | 5.251 [4.561, 6.291] | 4.881 [4.727, 4.903] | 0.929 | 1.190 | 1.093 |
| variant-01/02-agent-reuse | 5.466 [4.872, 5.653] | 4.785 [4.080, 5.109] | 0.875 | 1.623 | 1.090 |
| variant-01/03-selective-compatibility | 4.851 [4.644, 5.523] | 4.727 [3.948, 4.875] | 0.974 | 1.092 | 1.399 |
| variant-01/04-no-compatible-agent-unsat | 4.645 [3.708, 4.752] | 4.419 [3.843, 4.582] | 0.951 | 1.393 | 1.309 |
| variant-01/05-larger-mix | 5.820 [5.496, 6.637] | 6.630 [5.652, 6.784] | 1.139 | 1.314 | 1.496 |
| variant-02/01-basic | 4.894 [4.600, 6.050] | 4.618 [4.609, 5.971] | 0.944 | 1.108 | 1.051 |
| variant-02/02-makespan-tiebreak | 6.276 [5.565, 6.333] | 5.599 [5.524, 5.606] | 0.892 | 1.409 | 1.021 |
| variant-02/03-cost-dominates | 5.608 [5.039, 7.090] | 5.442 [4.769, 6.646] | 0.970 | 1.267 | 0.997 |
| variant-02/04-no-compatible-agent-unsat | 5.647 [5.523, 5.867] | 5.580 [4.814, 5.584] | 0.988 | 1.271 | 1.280 |
| variant-02/05-larger-mix | 9.171 [7.894, 9.329] | 11.244 [10.803, 11.456] | 1.226 | 1.670 | 2.045 |
| variant-03/01-basic | 5.340 [5.109, 5.604] | 5.551 [5.109, 5.872] | 1.039 | 1.198 | 1.242 |
| variant-03/02-multiple-groups | 5.813 [5.480, 6.854] | 5.721 [5.180, 6.891] | 0.984 | 1.311 | 1.288 |
| variant-03/03-mixed-grouped-ungrouped | 5.793 [4.493, 6.860] | 5.172 [5.058, 6.014] | 0.893 | 1.303 | 1.166 |
| variant-03/04-incompatible-group-unsat | 4.717 [4.369, 4.964] | 5.399 [4.375, 5.550] | 1.145 | 1.071 | 1.218 |
| variant-03/05-larger-mix | 5.994 [4.916, 6.620] | 6.642 [6.443, 6.894] | 1.108 | 1.350 | 1.493 |
| variant-04/01-basic | 9.748 [9.411, 10.818] | 11.249 [10.555, 11.512] | 1.154 | 0.380 | 0.438 |
| variant-04/02-precedence | 7.496 [7.163, 7.923] | 7.868 [7.698, 8.099] | 1.050 | 1.156 | 1.178 |
| variant-04/03-agent-serialization | 7.945 [7.709, 8.028] | 8.116 [7.121, 8.929] | 1.022 | 0.297 | 0.282 |
| variant-04/04-window-too-tight-unsat | 6.641 [6.621, 7.112] | 6.612 [5.901, 7.284] | 0.996 | 1.475 | 1.219 |
| variant-04/05-larger-mix | 79.698 [78.797, 80.256] | 93.882 [92.897, 93.923] | 1.178 | 0.553 | 0.636 |
| variant-01/01-basic | 5.964 [5.558, 6.736] | 6.164 [6.148, 7.214] | 1.033 | 1.316 | 1.106 |
| variant-01/02-multiple-tours | 5.832 [4.623, 7.267] | 6.754 [5.740, 7.734] | 1.158 | 1.308 | 1.496 |
| variant-01/03-asymmetric | 7.056 [6.654, 7.683] | 7.857 [7.676, 8.729] | 1.113 | 1.287 | 1.769 |
| variant-01/04-subtour-unsat | 4.839 [4.478, 5.137] | 5.031 [4.942, 5.487] | 1.040 | 1.086 | 1.135 |
| variant-01/05-ring | 8.699 [7.691, 9.400] | 11.242 [10.947, 12.367] | 1.292 | 1.576 | 2.044 |
| variant-02/01-basic | 6.172 [5.972, 6.602] | 6.473 [4.999, 6.613] | 1.049 | 1.405 | 1.450 |
| variant-02/02-single-salesman | 5.860 [5.544, 6.284] | 6.592 [5.570, 6.902] | 1.125 | 1.329 | 1.467 |
| variant-02/03-too-many-salesmen-unsat | 5.075 [4.904, 5.520] | 5.491 [5.159, 5.502] | 1.082 | 1.142 | 1.180 |
| variant-02/04-equal-cost-split | 5.864 [5.551, 6.275] | 5.987 [5.875, 6.031] | 1.021 | 1.324 | 1.351 |
| variant-02/05-larger-asymmetric | 4.889 [4.855, 5.908] | 5.670 [5.474, 6.329] | 1.160 | 1.123 | 1.278 |
| variant-02/06-unreachable-edge | 5.935 [5.746, 6.208] | 6.102 [5.568, 6.860] | 1.028 | 1.353 | 1.374 |
| variant-03/01-basic | 5.535 [5.233, 6.586] | 5.697 [5.620, 6.627] | 1.029 | 1.240 | 1.036 |
| variant-03/02-single-salesman | 5.508 [4.807, 5.797] | 5.601 [5.582, 5.643] | 1.017 | 1.252 | 1.265 |
| variant-03/03-depot-crossing-unsat | 5.329 [5.019, 6.571] | 5.273 [5.064, 5.503] | 0.989 | 1.201 | 1.186 |
| variant-03/04-equal-cost-split | 5.932 [4.412, 6.957] | 5.980 [5.486, 6.042] | 1.008 | 1.338 | 1.093 |
| variant-03/05-larger-three-depots | 5.706 [5.654, 5.924] | 7.361 [6.531, 8.873] | 1.290 | 1.300 | 1.658 |
| variant-03/06-unreachable-edge | 6.137 [4.747, 6.272] | 6.539 [6.207, 6.611] | 1.066 | 1.399 | 1.194 |
| variant-04/01-basic | 6.297 [6.262, 6.304] | 6.689 [6.616, 6.756] | 1.062 | 1.427 | 1.506 |
| variant-04/02-single-salesman | 6.189 [6.039, 6.596] | 6.304 [5.550, 6.625] | 1.019 | 1.400 | 1.144 |
| variant-04/03-window-too-tight-unsat | 6.065 [5.732, 7.661] | 5.820 [5.543, 6.754] | 0.960 | 1.369 | 1.285 |
| variant-04/04-depot-window-too-tight-unsat | 5.903 [5.602, 6.552] | 5.546 [5.546, 5.811] | 0.940 | 1.318 | 1.011 |
| variant-04/05-three-depots-asymmetric-times | 7.947 [6.919, 9.252] | 8.885 [8.826, 9.267] | 1.118 | 1.793 | 1.616 |
| variant-04/06-unreachable-edge | 7.758 [6.634, 7.966] | 8.342 [7.650, 9.304] | 1.075 | 1.747 | 1.526 |
| variant-05/01-basic | 6.571 [5.815, 6.656] | 6.691 [6.512, 7.395] | 1.018 | 1.195 | 1.221 |
| variant-05/02-tight-bound-exact | 6.688 [5.744, 6.837] | 6.255 [5.883, 6.293] | 0.935 | 1.228 | 1.140 |
| variant-05/03-revisit-too-tight-unsat | 6.102 [5.749, 6.129] | 5.871 [5.606, 6.219] | 0.962 | 1.374 | 1.331 |
| variant-05/04-depot-and-vertex-revisits | 6.560 [5.888, 6.575] | 6.394 [6.092, 6.595] | 0.975 | 1.505 | 1.434 |
| variant-05/05-three-depots-mixed | 7.736 [7.622, 8.193] | 9.416 [9.180, 9.787] | 1.217 | 1.416 | 1.724 |
| variant-05/06-unreachable-edge | 8.146 [7.033, 8.253] | 8.083 [7.965, 8.263] | 0.992 | 1.829 | 1.456 |
| n-queens/variant-01 | 9.446 [9.045, 10.468] | 9.733 [9.513, 9.945] | 1.030 | 1.698 | 1.486 |
| n-queens/variant-02 | 117.939 [116.804, 119.722] | 88.464 [88.116, 89.696] | 0.750 | 1.010 | 0.750 |
| n-queens/variant-03 | 9.959 [9.908, 10.102] | 9.076 [8.898, 9.340] | 0.911 | 1.811 | 1.374 |
| n-queens/variant-04 | 9.839 [7.724, 9.957] | 8.245 [7.830, 8.960] | 0.838 | 1.797 | 1.477 |
| n-queens/variant-05 | 10.971 [10.963, 11.993] | 10.862 [10.464, 11.406] | 0.990 | 1.663 | 1.650 |
| n-queens/variant-06 | 11.433 [11.358, 11.642] | 11.070 [10.294, 13.356] | 0.968 | 2.064 | 1.685 |
| send-money/send-money | 12.892 [12.881, 13.137] | 10.974 [10.832, 11.313] | 0.851 | 0.861 | 0.732 |

Reference wall time, ms, same notation.

| Cell | regions | clauses |
|---|---:|---:|
| equality-generalized-tsp/01-basic | 4.438 [3.328, 4.460] | 4.452 [4.422, 4.481] |
| equality-generalized-tsp/02-larger | 4.423 [3.285, 4.429] | 4.392 [4.372, 4.473] |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.457 [3.328, 4.487] | 4.414 [3.359, 4.462] |
| variant-01/01-basic | 4.409 [4.382, 5.479] | 4.439 [3.276, 4.442] |
| variant-01/02-start-equals-end | 4.411 [3.347, 4.441] | 4.451 [3.345, 5.496] |
| variant-01/03-zero-cost-detour | 4.483 [4.408, 5.489] | 5.500 [3.283, 5.507] |
| variant-01/04-no-path | 4.436 [4.407, 4.457] | 5.475 [4.429, 5.553] |
| variant-01/05-multi-path | 4.398 [4.348, 4.481] | 4.407 [4.383, 4.429] |
| variant-01/06-layered-dag | 5.521 [5.487, 6.552] | 5.475 [5.463, 5.519] |
| variant-01/07-cycles | 4.386 [4.353, 4.436] | 5.476 [4.475, 5.503] |
| variant-01/08-negative-weights | 3.335 [3.319, 3.336] | 4.455 [4.381, 5.451] |
| variant-02/01-basic | 4.396 [4.374, 4.431] | 4.482 [4.453, 5.472] |
| variant-02/02-start-equals-end | 4.409 [3.317, 4.449] | 4.444 [4.389, 5.496] |
| variant-02/03-before-forces-detour | 3.284 [3.277, 3.339] | 4.417 [4.395, 4.441] |
| variant-02/04-after-forces-extension | 4.455 [3.261, 6.559] | 4.451 [4.358, 4.475] |
| variant-02/05-ordering-unsat | 4.444 [4.389, 5.446] | 4.425 [4.339, 4.434] |
| variant-02/06-layered-dag-before | 5.507 [5.451, 6.490] | 6.523 [5.434, 6.560] |
| variant-02/07-layered-dag-before-after | 4.361 [4.320, 5.504] | 5.504 [5.473, 5.505] |
| variant-02/08-tie-break-under-ordering | 4.428 [4.424, 5.523] | 4.440 [4.414, 4.441] |
| variant-02/09-negative-weights | 4.420 [4.380, 4.444] | 4.398 [4.360, 4.431] |
| variant-03/01-basic | 4.426 [3.332, 4.447] | 4.439 [4.399, 4.448] |
| variant-03/02-start-equals-end | 4.400 [3.329, 4.439] | 4.375 [3.299, 4.405] |
| variant-03/03-budget-forces-detour | 3.345 [3.335, 4.451] | 4.428 [4.396, 4.431] |
| variant-03/04-cost-at-cap-allowed | 4.434 [4.407, 4.452] | 4.411 [3.327, 5.451] |
| variant-03/05-budget-unsat | 4.341 [3.333, 4.456] | 4.439 [4.394, 4.498] |
| variant-03/06-layered-dag-cap | 7.610 [7.601, 7.672] | 6.619 [6.571, 7.628] |
| variant-03/07-layered-dag-tight-cap | 6.561 [6.546, 6.574] | 5.519 [5.479, 6.583] |
| variant-03/08-tie-break-under-cap | 4.441 [4.350, 4.446] | 5.417 [4.456, 5.458] |
| variant-03/09-negative-weights | 4.405 [4.388, 5.517] | 4.441 [4.413, 4.451] |
| variant-04/01-basic | 4.425 [4.384, 5.409] | 4.443 [4.397, 5.501] |
| variant-04/02-start-equals-end | 4.406 [4.389, 4.433] | 4.445 [4.395, 5.449] |
| variant-04/03-before-forces-detour-within-budget | 4.411 [4.357, 4.419] | 4.438 [4.413, 5.512] |
| variant-04/04-ordering-violates-budget | 4.438 [4.392, 5.504] | 4.404 [4.350, 4.433] |
| variant-04/05-after-and-budget-interact | 5.488 [4.420, 6.554] | 4.412 [4.385, 4.449] |
| variant-04/06-layered-dag-ordering-cap | 6.560 [6.533, 7.594] | 6.595 [6.491, 7.647] |
| variant-04/07-layered-dag-combined | 6.493 [5.686, 7.570] | 6.562 [6.546, 6.597] |
| variant-04/08-tie-break-under-ordering-and-cap | 4.413 [4.402, 4.450] | 4.445 [4.406, 4.447] |
| variant-04/09-negative-weights | 5.480 [4.366, 5.534] | 4.546 [4.474, 5.508] |
| variant-01/01-basic | 4.415 [3.355, 4.439] | 4.464 [4.441, 4.482] |
| variant-01/02-agent-reuse | 3.368 [3.291, 3.377] | 4.391 [3.292, 4.433] |
| variant-01/03-selective-compatibility | 4.441 [3.344, 4.493] | 3.380 [3.374, 5.460] |
| variant-01/04-no-compatible-agent-unsat | 3.334 [3.282, 4.431] | 3.377 [3.293, 5.508] |
| variant-01/05-larger-mix | 4.430 [3.330, 5.489] | 4.431 [3.378, 4.484] |
| variant-02/01-basic | 4.418 [4.414, 5.450] | 4.395 [4.345, 4.413] |
| variant-02/02-makespan-tiebreak | 4.456 [4.362, 4.471] | 5.485 [4.444, 5.505] |
| variant-02/03-cost-dominates | 4.427 [3.290, 4.503] | 5.460 [4.480, 5.473] |
| variant-02/04-no-compatible-agent-unsat | 4.444 [3.377, 4.477] | 4.358 [3.372, 4.436] |
| variant-02/05-larger-mix | 5.490 [5.471, 6.518] | 5.499 [5.491, 5.517] |
| variant-03/01-basic | 4.456 [4.439, 5.499] | 4.470 [4.455, 4.523] |
| variant-03/02-multiple-groups | 4.435 [4.330, 4.454] | 4.443 [3.378, 5.510] |
| variant-03/03-mixed-grouped-ungrouped | 4.445 [4.419, 4.459] | 4.436 [3.276, 4.480] |
| variant-03/04-incompatible-group-unsat | 4.405 [3.390, 4.456] | 4.433 [4.395, 4.437] |
| variant-03/05-larger-mix | 4.440 [4.419, 4.441] | 4.448 [3.372, 4.505] |
| variant-04/01-basic | 25.627 [24.624, 25.689] | 25.668 [25.642, 27.068] |
| variant-04/02-precedence | 6.484 [5.564, 6.622] | 6.680 [6.670, 6.799] |
| variant-04/03-agent-serialization | 26.727 [26.651, 27.692] | 28.735 [28.008, 28.834] |
| variant-04/04-window-too-tight-unsat | 4.502 [4.450, 4.516] | 5.422 [4.492, 5.538] |
| variant-04/05-larger-mix | 144.076 [142.782, 144.824] | 147.687 [145.901, 150.721] |
| variant-01/01-basic | 4.531 [4.518, 5.561] | 5.572 [4.560, 5.829] |
| variant-01/02-multiple-tours | 4.458 [4.409, 4.462] | 4.515 [4.436, 5.438] |
| variant-01/03-asymmetric | 5.481 [4.453, 5.519] | 4.441 [4.415, 5.589] |
| variant-01/04-subtour-unsat | 4.455 [3.284, 5.481] | 4.432 [3.249, 4.443] |
| variant-01/05-ring | 5.518 [4.441, 6.562] | 5.500 [5.494, 5.509] |
| variant-02/01-basic | 4.394 [4.378, 4.441] | 4.464 [4.441, 5.460] |
| variant-02/02-single-salesman | 4.410 [3.322, 4.486] | 4.493 [4.474, 5.600] |
| variant-02/03-too-many-salesmen-unsat | 4.442 [3.293, 5.450] | 4.655 [4.434, 5.457] |
| variant-02/04-equal-cost-split | 4.430 [4.382, 6.515] | 4.432 [4.421, 4.462] |
| variant-02/05-larger-asymmetric | 4.352 [4.346, 4.398] | 4.436 [4.412, 4.438] |
| variant-02/06-unreachable-edge | 4.388 [3.334, 4.393] | 4.443 [4.404, 5.520] |
| variant-03/01-basic | 4.463 [3.294, 5.461] | 5.501 [4.394, 5.529] |
| variant-03/02-single-salesman | 4.399 [4.393, 4.410] | 4.427 [4.405, 4.446] |
| variant-03/03-depot-crossing-unsat | 4.436 [4.405, 4.438] | 4.444 [4.440, 5.464] |
| variant-03/04-equal-cost-split | 4.435 [4.357, 4.444] | 5.473 [4.494, 5.571] |
| variant-03/05-larger-three-depots | 4.389 [4.354, 5.467] | 4.440 [4.406, 5.472] |
| variant-03/06-unreachable-edge | 4.386 [3.286, 5.498] | 5.476 [4.433, 5.487] |
| variant-04/01-basic | 4.413 [4.411, 5.405] | 4.441 [4.409, 5.499] |
| variant-04/02-single-salesman | 4.421 [4.365, 4.427] | 5.512 [4.431, 5.528] |
| variant-04/03-window-too-tight-unsat | 4.430 [4.342, 4.601] | 4.530 [4.394, 5.502] |
| variant-04/04-depot-window-too-tight-unsat | 4.480 [4.395, 5.464] | 5.488 [5.459, 5.521] |
| variant-04/05-three-depots-asymmetric-times | 4.432 [4.420, 5.504] | 5.499 [5.479, 5.513] |
| variant-04/06-unreachable-edge | 4.442 [4.441, 5.463] | 5.469 [4.358, 5.525] |
| variant-05/01-basic | 5.498 [4.447, 5.503] | 5.482 [3.302, 5.737] |
| variant-05/02-tight-bound-exact | 5.447 [4.404, 5.495] | 5.489 [4.670, 5.496] |
| variant-05/03-revisit-too-tight-unsat | 4.443 [4.402, 5.495] | 4.410 [4.354, 5.489] |
| variant-05/04-depot-and-vertex-revisits | 4.359 [4.357, 4.447] | 4.458 [4.453, 5.499] |
| variant-05/05-three-depots-mixed | 5.463 [4.451, 5.586] | 5.463 [4.456, 6.537] |
| variant-05/06-unreachable-edge | 4.454 [4.448, 5.421] | 5.552 [4.399, 6.448] |
| n-queens/variant-01 | 5.564 [5.432, 6.613] | 6.549 [5.514, 7.678] |
| n-queens/variant-02 | 116.813 [116.739, 117.905] | 117.908 [116.866, 119.020] |
| n-queens/variant-03 | 5.498 [5.441, 5.509] | 6.607 [5.580, 6.830] |
| n-queens/variant-04 | 5.476 [5.431, 5.573] | 5.583 [5.481, 6.685] |
| n-queens/variant-05 | 6.596 [5.461, 7.699] | 6.583 [6.572, 6.695] |
| n-queens/variant-06 | 5.541 [5.428, 6.566] | 6.570 [5.490, 6.581] |
| send-money/send-money | 14.977 [14.036, 15.020] | 14.997 [13.970, 15.095] |

Counters of report clauses: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 0 | 1 | 4 | 75404 | 2.285 |
| equality-generalized-tsp/02-larger | 0 | 1 | 8 | 130751 | 2.915 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 0 | 0 | 0 | 4096 | 1.709 |
| variant-01/01-basic | 0 | 1 | 2 | 5540 | 1.333 |
| variant-01/02-start-equals-end | 0 | 1 | 1 | 8051 | 1.435 |
| variant-01/03-zero-cost-detour | 0 | 1 | 2 | 4274 | 1.266 |
| variant-01/04-no-path | 0 | 0 | 0 | 1865 | 1.315 |
| variant-01/05-multi-path | 0 | 1 | 3 | 18700 | 1.711 |
| variant-01/06-layered-dag | 0 | 1 | 27 | 344578 | 3.929 |
| variant-01/07-cycles | 0 | 1 | 5 | 114322 | 2.575 |
| variant-01/08-negative-weights | 0 | 1 | 1 | 28915 | 1.851 |
| variant-02/01-basic | 0 | 1 | 2 | 5904 | 1.489 |
| variant-02/02-start-equals-end | 0 | 1 | 1 | 10761 | 1.624 |
| variant-02/03-before-forces-detour | 0 | 1 | 1 | 7017 | 1.550 |
| variant-02/04-after-forces-extension | 0 | 1 | 1 | 6405 | 1.771 |
| variant-02/05-ordering-unsat | 0 | 0 | 0 | 2519 | 2.168 |
| variant-02/06-layered-dag-before | 0 | 2 | 9 | 239298 | 3.829 |
| variant-02/07-layered-dag-before-after | 0 | 1 | 7 | 209967 | 3.629 |
| variant-02/08-tie-break-under-ordering | 0 | 1 | 2 | 10614 | 1.828 |
| variant-02/09-negative-weights | 0 | 1 | 1 | 32506 | 2.115 |
| variant-03/01-basic | 0 | 1 | 2 | 5734 | 1.609 |
| variant-03/02-start-equals-end | 0 | 1 | 1 | 9929 | 1.656 |
| variant-03/03-budget-forces-detour | 0 | 1 | 1 | 6301 | 1.685 |
| variant-03/04-cost-at-cap-allowed | 0 | 1 | 1 | 3269 | 1.476 |
| variant-03/05-budget-unsat | 0 | 0 | 0 | 16082 | 1.719 |
| variant-03/06-layered-dag-cap | 0 | 2 | 25 | 611524 | 6.715 |
| variant-03/07-layered-dag-tight-cap | 0 | 4 | 6 | 363131 | 5.260 |
| variant-03/08-tie-break-under-cap | 0 | 1 | 2 | 12810 | 1.722 |
| variant-03/09-negative-weights | 0 | 1 | 1 | 32853 | 2.145 |
| variant-04/01-basic | 0 | 1 | 2 | 6098 | 1.659 |
| variant-04/02-start-equals-end | 0 | 1 | 1 | 12639 | 2.155 |
| variant-04/03-before-forces-detour-within-budget | 0 | 1 | 1 | 8170 | 1.878 |
| variant-04/04-ordering-violates-budget | 0 | 0 | 0 | 6098 | 1.812 |
| variant-04/05-after-and-budget-interact | 0 | 1 | 1 | 13637 | 1.849 |
| variant-04/06-layered-dag-ordering-cap | 0 | 2 | 8 | 363501 | 6.290 |
| variant-04/07-layered-dag-combined | 0 | 1 | 5 | 313657 | 5.530 |
| variant-04/08-tie-break-under-ordering-and-cap | 0 | 1 | 2 | 27409 | 2.224 |
| variant-04/09-negative-weights | 0 | 1 | 1 | 36453 | 2.278 |
| variant-01/01-basic | 0 | 1 | 4 | 5980 | 1.083 |
| variant-01/02-agent-reuse | 0 | 1 | 8 | 15903 | 1.292 |
| variant-01/03-selective-compatibility | 0 | 1 | 4 | 7209 | 1.141 |
| variant-01/04-no-compatible-agent-unsat | 0 | 0 | 0 | 2289 | 0.911 |
| variant-01/05-larger-mix | 0 | 1 | 64 | 182539 | 2.804 |
| variant-02/01-basic | 0 | 1 | 4 | 14404 | 1.483 |
| variant-02/02-makespan-tiebreak | 0 | 3 | 8 | 25447 | 1.812 |
| variant-02/03-cost-dominates | 0 | 1 | 8 | 29126 | 1.685 |
| variant-02/04-no-compatible-agent-unsat | 0 | 0 | 0 | 4725 | 1.387 |
| variant-02/05-larger-mix | 0 | 1 | 67 | 844697 | 6.472 |
| variant-03/01-basic | 0 | 2 | 2 | 7431 | 1.650 |
| variant-03/02-multiple-groups | 0 | 4 | 4 | 17609 | 1.682 |
| variant-03/03-mixed-grouped-ungrouped | 0 | 2 | 4 | 11285 | 1.696 |
| variant-03/04-incompatible-group-unsat | 0 | 0 | 0 | 2497 | 1.308 |
| variant-03/05-larger-mix | 0 | 1 | 27 | 51564 | 2.676 |
| variant-04/01-basic | 0 | 81 | 124 | 875111 | 7.464 |
| variant-04/02-precedence | 0 | 3 | 12 | 75546 | 3.599 |
| variant-04/03-agent-serialization | 0 | 30 | 30 | 253900 | 4.420 |
| variant-04/04-window-too-tight-unsat | 0 | 0 | 0 | 8883 | 2.779 |
| variant-04/05-larger-mix | 0 | 1176 | 1208 | 23292550 | 90.335 |
| variant-01/01-basic | 0 | 1 | 2 | 45803 | 2.054 |
| variant-01/02-multiple-tours | 0 | 2 | 9 | 217292 | 3.076 |
| variant-01/03-asymmetric | 0 | 1 | 9 | 326714 | 3.737 |
| variant-01/04-subtour-unsat | 0 | 0 | 0 | 4623 | 1.245 |
| variant-01/05-ring | 0 | 2 | 44 | 1422699 | 7.936 |
| variant-02/01-basic | 0 | 2 | 2 | 50389 | 2.205 |
| variant-02/02-single-salesman | 0 | 1 | 2 | 32172 | 2.302 |
| variant-02/03-too-many-salesmen-unsat | 0 | 0 | 0 | 6906 | 1.460 |
| variant-02/04-equal-cost-split | 0 | 2 | 2 | 40903 | 2.044 |
| variant-02/05-larger-asymmetric | 0 | 2 | 2 | 60140 | 2.165 |
| variant-02/06-unreachable-edge | 0 | 2 | 2 | 69881 | 2.517 |
| variant-03/01-basic | 0 | 1 | 1 | 26134 | 1.868 |
| variant-03/02-single-salesman | 0 | 1 | 2 | 30993 | 1.766 |
| variant-03/03-depot-crossing-unsat | 0 | 0 | 0 | 6761 | 1.586 |
| variant-03/04-equal-cost-split | 0 | 2 | 2 | 70632 | 2.342 |
| variant-03/05-larger-three-depots | 0 | 1 | 1 | 73646 | 3.503 |
| variant-03/06-unreachable-edge | 0 | 1 | 1 | 44796 | 2.399 |
| variant-04/01-basic | 0 | 1 | 1 | 46886 | 2.800 |
| variant-04/02-single-salesman | 0 | 1 | 1 | 31547 | 2.677 |
| variant-04/03-window-too-tight-unsat | 0 | 0 | 0 | 6641 | 2.022 |
| variant-04/04-depot-window-too-tight-unsat | 0 | 0 | 0 | 4430 | 1.944 |
| variant-04/05-three-depots-asymmetric-times | 0 | 1 | 1 | 125685 | 4.839 |
| variant-04/06-unreachable-edge | 0 | 1 | 1 | 98235 | 4.406 |
| variant-05/01-basic | 0 | 1 | 1 | 46894 | 3.142 |
| variant-05/02-tight-bound-exact | 0 | 1 | 1 | 23047 | 2.384 |
| variant-05/03-revisit-too-tight-unsat | 0 | 0 | 0 | 6826 | 2.039 |
| variant-05/04-depot-and-vertex-revisits | 0 | 1 | 1 | 23457 | 2.713 |
| variant-05/05-three-depots-mixed | 0 | 1 | 1 | 126050 | 5.707 |
| variant-05/06-unreachable-edge | 0 | 1 | 1 | 98235 | 4.248 |
| n-queens/variant-01 | 0 | 92 | 92 | 734652 | 6.518 |
| n-queens/variant-02 | 0 | 92 | 92 | 18761080 | 85.088 |
| n-queens/variant-03 | 0 | 92 | 92 | 675790 | 5.542 |
| n-queens/variant-04 | 0 | 92 | 92 | 574291 | 4.276 |
| n-queens/variant-05 | 0 | 92 | 92 | 872980 | 6.873 |
| n-queens/variant-06 | 0 | 92 | 92 | 1004609 | 7.398 |
| send-money/send-money | 0 | 1 | 1 | 349198 | 6.986 |

Against the reference: report regions, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=1, completion workers=4, batch=64; search default): faster on 4 of 94 cells where both passed (4.2%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/03-agent-serialization | 7.945 | 26.727 | 0.297 | 0.905 | 0.470 | 0.223 | 23.000 | 0.000 |
| variant-04/01-basic | 9.748 | 25.627 | 0.380 | 0.969 | 1.076 | 0.889 | 17.000 | 4.000 |
| variant-04/05-larger-mix | 79.698 | 144.076 | 0.553 | 3.269 | 30.297 | 24.970 | 82.000 | 56.000 |
| send-money/send-money | 12.892 | 14.977 | 0.861 | 2.620 | 3.443 | 0.302 | 9.000 | 1.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-02 | 117.939 | 116.813 | 1.010 | 2.449 | 107.660 | 2.146 | 1.000 | 112.000 |
| variant-04/05-after-and-budget-interact | 5.621 | 5.488 | 1.024 | 0.342 | 0.059 | 0.023 | 1.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 4.665 | 4.457 | 1.047 | 0.181 | 0.024 | 0.009 | 0.000 | 0.000 |
| variant-02/05-ordering-unsat | 4.737 | 4.444 | 1.066 | 0.178 | 0.020 | 0.005 | 1.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 4.717 | 4.405 | 1.071 | 0.115 | 0.020 | 0.005 | 0.000 | 0.000 |
| variant-01/04-subtour-unsat | 4.839 | 4.455 | 1.086 | 0.187 | 0.023 | 0.008 | 0.000 | 0.000 |
| variant-01/01-basic | 4.796 | 4.409 | 1.088 | 0.165 | 0.029 | 0.011 | 1.000 | 0.000 |
| variant-01/03-selective-compatibility | 4.851 | 4.441 | 1.092 | 0.166 | 0.031 | 0.015 | 0.000 | 0.000 |
| variant-01/02-start-equals-end | 4.819 | 4.411 | 1.093 | 0.154 | 0.021 | 0.007 | 1.000 | 0.000 |
| variant-02/01-basic | 4.894 | 4.418 | 1.108 | 0.292 | 0.056 | 0.024 | 1.000 | 0.000 |
| variant-02/05-larger-asymmetric | 4.889 | 4.352 | 1.123 | 0.442 | 0.067 | 0.021 | 1.000 | 0.000 |
| variant-01/04-no-path | 4.986 | 4.436 | 1.124 | 0.156 | 0.019 | 0.005 | 0.000 | 0.000 |
| variant-01/05-multi-path | 4.978 | 4.398 | 1.132 | 0.285 | 0.058 | 0.024 | 1.000 | 0.000 |
| variant-02/01-basic | 4.978 | 4.396 | 1.132 | 0.171 | 0.030 | 0.012 | 1.000 | 0.000 |
| variant-02/04-after-forces-extension | 5.050 | 4.455 | 1.134 | 0.229 | 0.031 | 0.012 | 1.000 | 0.000 |
| variant-04/09-negative-weights | 6.217 | 5.480 | 1.134 | 0.307 | 0.053 | 0.009 | 1.000 | 0.000 |
| variant-02/03-too-many-salesmen-unsat | 5.075 | 4.442 | 1.142 | 0.299 | 0.033 | 0.011 | 1.000 | 0.000 |
| variant-03/02-start-equals-end | 5.050 | 4.400 | 1.148 | 0.190 | 0.021 | 0.008 | 1.000 | 0.000 |
| variant-04/02-precedence | 7.496 | 6.484 | 1.156 | 0.616 | 0.206 | 0.085 | 2.000 | 0.000 |
| variant-01/03-zero-cost-detour | 5.199 | 4.483 | 1.160 | 0.182 | 0.027 | 0.010 | 1.000 | 0.000 |
| variant-01/01-basic | 5.251 | 4.415 | 1.190 | 0.145 | 0.057 | 0.029 | 0.000 | 0.000 |
| variant-05/01-basic | 6.571 | 5.498 | 1.195 | 0.612 | 0.063 | 0.023 | 1.000 | 0.000 |
| variant-03/01-basic | 5.340 | 4.456 | 1.198 | 0.161 | 0.033 | 0.016 | 0.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 5.329 | 4.436 | 1.201 | 0.319 | 0.036 | 0.012 | 1.000 | 0.000 |
| variant-02/02-start-equals-end | 5.297 | 4.409 | 1.201 | 0.160 | 0.023 | 0.008 | 1.000 | 0.000 |
| variant-03/06-layered-dag-cap | 9.331 | 7.610 | 1.226 | 1.623 | 1.371 | 0.651 | 2.000 | 0.000 |
| variant-05/02-tight-bound-exact | 6.688 | 5.447 | 1.228 | 0.383 | 0.040 | 0.013 | 1.000 | 0.000 |
| variant-02/09-negative-weights | 5.437 | 4.420 | 1.230 | 0.247 | 0.046 | 0.007 | 1.000 | 0.000 |
| variant-03/01-basic | 5.535 | 4.463 | 1.240 | 0.310 | 0.039 | 0.012 | 1.000 | 0.000 |
| variant-01/07-cycles | 5.460 | 4.386 | 1.245 | 0.359 | 0.091 | 0.026 | 1.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 5.547 | 4.441 | 1.249 | 0.261 | 0.049 | 0.018 | 1.000 | 0.000 |
| variant-03/01-basic | 5.531 | 4.426 | 1.250 | 0.210 | 0.031 | 0.012 | 1.000 | 0.000 |
| variant-04/01-basic | 5.539 | 4.425 | 1.252 | 0.198 | 0.032 | 0.013 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 5.508 | 4.399 | 1.252 | 0.252 | 0.046 | 0.015 | 1.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 5.545 | 4.428 | 1.252 | 0.241 | 0.056 | 0.018 | 1.000 | 0.000 |
| variant-03/09-negative-weights | 5.553 | 4.405 | 1.260 | 0.298 | 0.046 | 0.008 | 1.000 | 0.000 |
| variant-02/03-cost-dominates | 5.608 | 4.427 | 1.267 | 0.323 | 0.072 | 0.041 | 1.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 5.647 | 4.444 | 1.271 | 0.276 | 0.026 | 0.009 | 0.000 | 0.000 |
| variant-03/04-cost-at-cap-allowed | 5.658 | 4.434 | 1.276 | 0.196 | 0.043 | 0.016 | 1.000 | 0.000 |
| equality-generalized-tsp/01-basic | 5.703 | 4.438 | 1.285 | 0.373 | 0.067 | 0.025 | 1.000 | 0.000 |
| variant-02/06-layered-dag-before | 7.082 | 5.507 | 1.286 | 1.111 | 0.557 | 0.233 | 2.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 5.710 | 4.438 | 1.287 | 0.285 | 0.041 | 0.010 | 1.000 | 0.000 |
| variant-01/03-asymmetric | 7.056 | 5.481 | 1.287 | 0.549 | 0.206 | 0.107 | 1.000 | 0.000 |
| variant-03/05-larger-three-depots | 5.706 | 4.389 | 1.300 | 0.819 | 0.083 | 0.026 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 5.793 | 4.445 | 1.303 | 0.193 | 0.039 | 0.019 | 0.000 | 0.000 |
| variant-01/02-multiple-tours | 5.832 | 4.458 | 1.308 | 0.398 | 0.162 | 0.081 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 5.813 | 4.435 | 1.311 | 0.222 | 0.054 | 0.024 | 1.000 | 0.000 |
| variant-01/05-larger-mix | 5.820 | 4.430 | 1.314 | 0.254 | 0.304 | 0.153 | 0.000 | 0.000 |
| variant-01/01-basic | 5.964 | 4.531 | 1.316 | 0.284 | 0.062 | 0.024 | 1.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 5.903 | 4.480 | 1.318 | 0.300 | 0.028 | 0.009 | 1.000 | 0.000 |
| variant-04/02-start-equals-end | 5.827 | 4.406 | 1.323 | 0.222 | 0.027 | 0.010 | 1.000 | 0.000 |
| variant-02/04-equal-cost-split | 5.864 | 4.430 | 1.324 | 0.313 | 0.052 | 0.018 | 1.000 | 0.000 |
| variant-02/02-single-salesman | 5.860 | 4.410 | 1.329 | 0.255 | 0.046 | 0.015 | 1.000 | 0.000 |
| variant-03/04-equal-cost-split | 5.932 | 4.435 | 1.338 | 0.465 | 0.071 | 0.027 | 1.000 | 0.000 |
| variant-03/05-larger-mix | 5.994 | 4.440 | 1.350 | 0.281 | 0.095 | 0.083 | 0.000 | 0.000 |
| variant-02/06-unreachable-edge | 5.935 | 4.388 | 1.353 | 0.492 | 0.077 | 0.024 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 5.975 | 4.411 | 1.355 | 0.254 | 0.036 | 0.013 | 1.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 6.065 | 4.430 | 1.369 | 0.397 | 0.035 | 0.011 | 1.000 | 0.000 |
| equality-generalized-tsp/02-larger | 6.061 | 4.423 | 1.370 | 0.586 | 0.102 | 0.035 | 1.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 6.102 | 4.443 | 1.374 | 0.386 | 0.036 | 0.012 | 1.000 | 0.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 6.069 | 4.413 | 1.375 | 0.430 | 0.082 | 0.035 | 1.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 9.040 | 6.561 | 1.378 | 1.651 | 1.190 | 0.287 | 2.000 | 0.000 |
| variant-01/08-negative-weights | 4.617 | 3.335 | 1.384 | 0.234 | 0.039 | 0.007 | 1.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 4.645 | 3.334 | 1.393 | 0.136 | 0.017 | 0.005 | 0.000 | 0.000 |
| variant-03/06-unreachable-edge | 6.137 | 4.386 | 1.399 | 0.540 | 0.061 | 0.021 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 6.189 | 4.421 | 1.400 | 0.464 | 0.049 | 0.016 | 1.000 | 0.000 |
| variant-02/01-basic | 6.172 | 4.394 | 1.405 | 0.354 | 0.058 | 0.019 | 1.000 | 0.000 |
| variant-04/07-layered-dag-combined | 9.129 | 6.493 | 1.406 | 1.762 | 0.828 | 0.308 | 2.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 6.276 | 4.456 | 1.409 | 0.349 | 0.076 | 0.041 | 1.000 | 0.000 |
| variant-05/05-three-depots-mixed | 7.736 | 5.463 | 1.416 | 1.530 | 0.132 | 0.063 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 6.191 | 4.341 | 1.426 | 0.347 | 0.055 | 0.014 | 1.000 | 0.000 |
| variant-04/01-basic | 6.297 | 4.413 | 1.427 | 0.621 | 0.061 | 0.022 | 1.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 9.441 | 6.560 | 1.439 | 1.770 | 0.935 | 0.373 | 2.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 6.641 | 4.502 | 1.475 | 0.320 | 0.045 | 0.011 | 1.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 6.560 | 4.359 | 1.505 | 0.382 | 0.039 | 0.013 | 1.000 | 0.000 |
| variant-01/06-layered-dag | 8.363 | 5.521 | 1.515 | 1.033 | 0.762 | 0.365 | 2.000 | 0.000 |
| variant-03/03-budget-forces-detour | 5.114 | 3.345 | 1.529 | 0.214 | 0.032 | 0.012 | 1.000 | 0.000 |
| variant-02/03-before-forces-detour | 5.051 | 3.284 | 1.538 | 0.233 | 0.031 | 0.012 | 1.000 | 0.000 |
| variant-01/05-ring | 8.699 | 5.518 | 1.576 | 0.665 | 0.748 | 0.300 | 1.000 | 0.000 |
| variant-01/02-agent-reuse | 5.466 | 3.368 | 1.623 | 0.159 | 0.038 | 0.020 | 0.000 | 0.000 |
| n-queens/variant-05 | 10.971 | 6.596 | 1.663 | 0.984 | 2.593 | 1.135 | 1.000 | 1.000 |
| variant-02/05-larger-mix | 9.171 | 5.490 | 1.670 | 0.730 | 1.176 | 0.430 | 2.000 | 0.000 |
| n-queens/variant-01 | 9.446 | 5.564 | 1.698 | 1.804 | 1.960 | 0.960 | 1.000 | 1.000 |
| variant-04/06-unreachable-edge | 7.758 | 4.442 | 1.747 | 1.190 | 0.122 | 0.043 | 1.000 | 0.000 |
| variant-02/07-layered-dag-before-after | 7.686 | 4.361 | 1.763 | 1.089 | 0.491 | 0.204 | 2.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 7.947 | 4.432 | 1.793 | 1.538 | 0.147 | 0.050 | 1.000 | 0.000 |
| n-queens/variant-04 | 9.839 | 5.476 | 1.797 | 0.714 | 2.053 | 0.727 | 1.000 | 1.000 |
| n-queens/variant-03 | 9.959 | 5.498 | 1.811 | 1.823 | 2.181 | 1.044 | 1.000 | 1.000 |
| variant-05/06-unreachable-edge | 8.146 | 4.454 | 1.829 | 1.196 | 0.124 | 0.042 | 1.000 | 0.000 |
| n-queens/variant-06 | 11.433 | 5.541 | 2.064 | 1.065 | 2.693 | 1.262 | 1.000 | 1.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/03-agent-serialization | 11.6 | 12.3 | n/a |
| variant-04/01-basic | 11.3 | 13.2 | n/a |
| variant-04/05-larger-mix | 13.3 | 23.5 | n/a |
| send-money/send-money | 12.5 | 12.8 | n/a |
| n-queens/variant-02 | 13.1 | 12.8 | n/a |
| variant-04/05-after-and-budget-interact | 11.0 | 10.6 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.2 | 10.4 | n/a |
| variant-02/05-ordering-unsat | 10.9 | 10.5 | n/a |
| variant-03/04-incompatible-group-unsat | 10.4 | 10.2 | n/a |
| variant-01/04-subtour-unsat | 10.4 | 10.3 | n/a |
| variant-01/01-basic | 10.5 | 10.5 | n/a |
| variant-01/03-selective-compatibility | 10.5 | 10.3 | n/a |
| variant-01/02-start-equals-end | 10.2 | 10.3 | n/a |
| variant-02/01-basic | 10.4 | 10.4 | n/a |
| variant-02/05-larger-asymmetric | 10.4 | 10.5 | n/a |
| variant-01/04-no-path | 10.7 | 10.5 | n/a |
| variant-01/05-multi-path | 10.4 | 10.6 | n/a |
| variant-02/01-basic | 10.7 | 10.5 | n/a |
| variant-02/04-after-forces-extension | 10.9 | 10.5 | n/a |
| variant-04/09-negative-weights | 11.0 | 10.6 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.8 | 10.4 | n/a |
| variant-03/02-start-equals-end | 11.0 | 10.6 | n/a |
| variant-04/02-precedence | 11.5 | 10.7 | n/a |
| variant-01/03-zero-cost-detour | 10.8 | 10.3 | n/a |
| variant-01/01-basic | 10.4 | 10.4 | n/a |
| variant-05/01-basic | 11.1 | 10.5 | n/a |
| variant-03/01-basic | 10.6 | 10.2 | n/a |
| variant-03/03-depot-crossing-unsat | 10.6 | 10.4 | n/a |
| variant-02/02-start-equals-end | 10.6 | 10.3 | n/a |
| variant-03/06-layered-dag-cap | 11.8 | 10.6 | n/a |
| variant-05/02-tight-bound-exact | 11.0 | 10.4 | n/a |
| variant-02/09-negative-weights | 10.9 | 10.4 | n/a |
| variant-03/01-basic | 10.9 | 10.5 | n/a |
| variant-01/07-cycles | 10.6 | 10.5 | n/a |
| variant-03/08-tie-break-under-cap | 10.8 | 10.4 | n/a |
| variant-03/01-basic | 10.6 | 10.5 | n/a |
| variant-04/01-basic | 10.8 | 10.6 | n/a |
| variant-03/02-single-salesman | 10.7 | 10.4 | n/a |
| variant-02/08-tie-break-under-ordering | 10.8 | 10.6 | n/a |
| variant-03/09-negative-weights | 11.1 | 10.6 | n/a |
| variant-02/03-cost-dominates | 10.3 | 10.4 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.6 | 10.4 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.4 | 10.6 | n/a |
| equality-generalized-tsp/01-basic | 10.5 | 10.5 | n/a |
| variant-02/06-layered-dag-before | 11.4 | 10.6 | n/a |
| variant-04/04-ordering-violates-budget | 11.0 | 10.6 | n/a |
| variant-01/03-asymmetric | 11.1 | 10.6 | n/a |
| variant-03/05-larger-three-depots | 10.9 | 10.4 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.7 | 10.4 | n/a |
| variant-01/02-multiple-tours | 10.9 | 10.4 | n/a |
| variant-03/02-multiple-groups | 10.8 | 10.4 | n/a |
| variant-01/05-larger-mix | 10.8 | 10.3 | n/a |
| variant-01/01-basic | 10.5 | 10.6 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 10.9 | 10.4 | n/a |
| variant-04/02-start-equals-end | 10.7 | 10.6 | n/a |
| variant-02/04-equal-cost-split | 10.6 | 10.5 | n/a |
| variant-02/02-single-salesman | 10.8 | 10.5 | n/a |
| variant-03/04-equal-cost-split | 10.7 | 10.6 | n/a |
| variant-03/05-larger-mix | 10.6 | 10.5 | n/a |
| variant-02/06-unreachable-edge | 10.8 | 10.6 | n/a |
| variant-04/03-before-forces-detour-within-budget | 10.7 | 10.5 | n/a |
| variant-04/03-window-too-tight-unsat | 10.9 | 10.6 | n/a |
| equality-generalized-tsp/02-larger | 10.8 | 10.6 | n/a |
| variant-05/03-revisit-too-tight-unsat | 11.1 | 10.6 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 10.7 | 10.5 | n/a |
| variant-03/07-layered-dag-tight-cap | 11.8 | 11.1 | n/a |
| variant-01/08-negative-weights | 10.5 | 10.4 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.4 | 10.4 | n/a |
| variant-03/06-unreachable-edge | 10.8 | 10.4 | n/a |
| variant-04/02-single-salesman | 11.0 | 10.5 | n/a |
| variant-02/01-basic | 10.7 | 10.5 | n/a |
| variant-04/07-layered-dag-combined | 12.3 | 11.0 | n/a |
| variant-02/02-makespan-tiebreak | 10.6 | 10.2 | n/a |
| variant-05/05-three-depots-mixed | 11.1 | 10.7 | n/a |
| variant-03/05-budget-unsat | 11.0 | 10.4 | n/a |
| variant-04/01-basic | 10.7 | 10.3 | n/a |
| variant-04/06-layered-dag-ordering-cap | 12.2 | 11.0 | n/a |
| variant-04/04-window-too-tight-unsat | 11.2 | 10.6 | n/a |
| variant-05/04-depot-and-vertex-revisits | 10.9 | 10.6 | n/a |
| variant-01/06-layered-dag | 11.4 | 10.8 | n/a |
| variant-03/03-budget-forces-detour | 10.8 | 10.4 | n/a |
| variant-02/03-before-forces-detour | 11.1 | 10.6 | n/a |
| variant-01/05-ring | 11.6 | 10.6 | n/a |
| variant-01/02-agent-reuse | 10.7 | 10.3 | n/a |
| n-queens/variant-05 | 11.3 | 10.5 | n/a |
| variant-02/05-larger-mix | 11.5 | 10.5 | n/a |
| n-queens/variant-01 | 10.9 | 10.6 | n/a |
| variant-04/06-unreachable-edge | 11.2 | 10.4 | n/a |
| variant-02/07-layered-dag-before-after | 11.5 | 10.7 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.1 | 10.5 | n/a |
| n-queens/variant-04 | 11.1 | 10.5 | n/a |
| n-queens/variant-03 | 10.9 | 10.5 | n/a |
| variant-05/06-unreachable-edge | 11.2 | 10.6 | n/a |
| n-queens/variant-06 | 11.5 | 10.7 | n/a |

Against the reference: report clauses, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=1, completion workers=4, batch=64; search clauses): faster on 8 of 94 cells where both passed (8.5%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/03-agent-serialization | 8.116 | 28.735 | 0.282 | 0.945 | 0.633 | 0.231 | 24.000 | 0.000 |
| variant-04/01-basic | 11.249 | 25.668 | 0.438 | 1.194 | 1.694 | 0.871 | 17.000 | 4.000 |
| variant-04/05-larger-mix | 93.882 | 147.687 | 0.636 | 3.884 | 45.119 | 24.675 | 84.000 | 57.000 |
| send-money/send-money | 10.974 | 14.997 | 0.732 | 2.576 | 1.753 | 0.224 | 10.000 | 1.000 |
| n-queens/variant-02 | 88.464 | 117.908 | 0.750 | 2.442 | 79.187 | 2.075 | 1.000 | 113.000 |
| variant-01/04-no-path | 4.832 | 5.475 | 0.883 | 0.177 | 0.012 | 0.006 | 1.000 | 0.000 |
| variant-01/03-zero-cost-detour | 4.999 | 5.500 | 0.909 | 0.154 | 0.022 | 0.010 | 1.000 | 0.000 |
| variant-02/03-cost-dominates | 5.442 | 5.460 | 0.997 | 0.328 | 0.075 | 0.039 | 1.000 | 0.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/04-depot-window-too-tight-unsat | 5.546 | 5.488 | 1.011 | 0.310 | 0.020 | 0.010 | 1.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 5.599 | 5.485 | 1.021 | 0.358 | 0.080 | 0.041 | 1.000 | 0.000 |
| variant-03/01-basic | 5.697 | 5.501 | 1.036 | 0.335 | 0.035 | 0.077 | 1.000 | 0.000 |
| variant-02/01-basic | 4.618 | 4.395 | 1.051 | 0.291 | 0.050 | 0.026 | 1.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 5.699 | 5.417 | 1.052 | 0.264 | 0.048 | 0.018 | 1.000 | 0.000 |
| variant-04/01-basic | 4.700 | 4.443 | 1.058 | 0.197 | 0.022 | 0.013 | 1.000 | 0.000 |
| variant-01/02-agent-reuse | 4.785 | 4.391 | 1.090 | 0.154 | 0.030 | 0.020 | 0.000 | 0.000 |
| variant-03/04-equal-cost-split | 5.980 | 5.473 | 1.093 | 0.448 | 0.080 | 0.133 | 1.000 | 0.000 |
| variant-01/01-basic | 4.881 | 4.464 | 1.093 | 0.142 | 0.023 | 0.013 | 0.000 | 0.000 |
| variant-01/01-basic | 6.164 | 5.572 | 1.106 | 0.272 | 0.061 | 0.096 | 1.000 | 0.000 |
| variant-01/04-subtour-unsat | 5.031 | 4.432 | 1.135 | 0.214 | 0.020 | 0.008 | 0.000 | 0.000 |
| variant-05/02-tight-bound-exact | 6.255 | 5.489 | 1.140 | 0.379 | 0.031 | 0.061 | 1.000 | 0.000 |
| variant-02/02-start-equals-end | 5.067 | 4.444 | 1.140 | 0.178 | 0.022 | 0.030 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 6.304 | 5.512 | 1.144 | 0.459 | 0.044 | 0.063 | 1.000 | 0.000 |
| variant-01/01-basic | 5.136 | 4.439 | 1.157 | 0.173 | 0.023 | 0.012 | 1.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 5.172 | 4.436 | 1.166 | 0.202 | 0.044 | 0.019 | 0.000 | 0.000 |
| variant-02/03-before-forces-detour | 5.164 | 4.417 | 1.169 | 0.215 | 0.026 | 0.012 | 1.000 | 0.000 |
| variant-04/02-precedence | 7.868 | 6.680 | 1.178 | 0.623 | 0.213 | 0.087 | 2.000 | 0.000 |
| variant-02/03-too-many-salesmen-unsat | 5.491 | 4.655 | 1.180 | 0.292 | 0.029 | 0.011 | 1.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 5.273 | 4.444 | 1.186 | 0.298 | 0.029 | 0.013 | 1.000 | 0.000 |
| variant-03/06-unreachable-edge | 6.539 | 5.476 | 1.194 | 0.527 | 0.053 | 0.101 | 1.000 | 0.000 |
| variant-03/04-cost-at-cap-allowed | 5.359 | 4.411 | 1.215 | 0.180 | 0.016 | 0.007 | 1.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 5.399 | 4.433 | 1.218 | 0.122 | 0.016 | 0.005 | 0.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 6.612 | 5.422 | 1.219 | 0.317 | 0.044 | 0.011 | 1.000 | 0.000 |
| variant-05/01-basic | 6.691 | 5.482 | 1.221 | 0.597 | 0.058 | 0.140 | 1.000 | 0.000 |
| variant-02/06-layered-dag-before | 7.969 | 6.523 | 1.222 | 1.093 | 0.734 | 0.180 | 2.000 | 0.000 |
| variant-02/01-basic | 5.490 | 4.482 | 1.225 | 0.177 | 0.026 | 0.013 | 1.000 | 0.000 |
| variant-01/02-start-equals-end | 5.507 | 4.451 | 1.237 | 0.152 | 0.018 | 0.025 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 5.498 | 4.428 | 1.242 | 0.226 | 0.033 | 0.013 | 1.000 | 0.000 |
| variant-03/01-basic | 5.551 | 4.470 | 1.242 | 0.161 | 0.030 | 0.014 | 0.000 | 0.000 |
| variant-01/05-multi-path | 5.484 | 4.407 | 1.244 | 0.278 | 0.061 | 0.023 | 1.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 5.543 | 4.440 | 1.249 | 0.250 | 0.040 | 0.018 | 1.000 | 0.000 |
| variant-01/08-negative-weights | 5.567 | 4.455 | 1.250 | 0.255 | 0.045 | 0.063 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 5.549 | 4.439 | 1.250 | 0.352 | 0.060 | 0.014 | 1.000 | 0.000 |
| equality-generalized-tsp/01-basic | 5.573 | 4.452 | 1.252 | 0.381 | 0.062 | 0.099 | 1.000 | 0.000 |
| variant-03/01-basic | 5.568 | 4.439 | 1.254 | 0.210 | 0.024 | 0.012 | 1.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.546 | 4.414 | 1.256 | 0.184 | 0.019 | 0.012 | 1.000 | 0.000 |
| variant-02/05-ordering-unsat | 5.580 | 4.425 | 1.261 | 0.182 | 0.015 | 0.005 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 5.601 | 4.427 | 1.265 | 0.248 | 0.039 | 0.047 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 5.589 | 4.412 | 1.267 | 0.302 | 0.052 | 0.017 | 1.000 | 0.000 |
| variant-02/05-larger-asymmetric | 5.670 | 4.436 | 1.278 | 0.452 | 0.056 | 0.096 | 1.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 5.630 | 4.404 | 1.278 | 0.280 | 0.027 | 0.010 | 1.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 5.580 | 4.358 | 1.280 | 0.275 | 0.022 | 0.009 | 0.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 5.820 | 4.530 | 1.285 | 0.371 | 0.027 | 0.010 | 1.000 | 0.000 |
| variant-01/07-cycles | 7.045 | 5.476 | 1.287 | 0.358 | 0.094 | 0.104 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 5.721 | 4.443 | 1.288 | 0.226 | 0.048 | 0.024 | 1.000 | 0.000 |
| variant-04/09-negative-weights | 5.938 | 4.546 | 1.306 | 0.308 | 0.050 | 0.073 | 1.000 | 0.000 |
| variant-03/02-start-equals-end | 5.717 | 4.375 | 1.307 | 0.160 | 0.019 | 0.024 | 1.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 4.419 | 3.377 | 1.309 | 0.118 | 0.015 | 0.005 | 0.000 | 0.000 |
| variant-02/04-after-forces-extension | 5.849 | 4.451 | 1.314 | 0.231 | 0.026 | 0.012 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 5.836 | 4.438 | 1.315 | 0.261 | 0.032 | 0.015 | 1.000 | 0.000 |
| variant-03/09-negative-weights | 5.852 | 4.441 | 1.318 | 0.303 | 0.051 | 0.088 | 1.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 5.871 | 4.410 | 1.331 | 0.365 | 0.026 | 0.011 | 1.000 | 0.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 5.994 | 4.445 | 1.349 | 0.444 | 0.092 | 0.035 | 1.000 | 0.000 |
| variant-02/04-equal-cost-split | 5.987 | 4.432 | 1.351 | 0.314 | 0.050 | 0.088 | 1.000 | 0.000 |
| variant-02/06-unreachable-edge | 6.102 | 4.443 | 1.374 | 0.494 | 0.070 | 0.121 | 1.000 | 0.000 |
| n-queens/variant-03 | 9.076 | 6.607 | 1.374 | 1.821 | 1.510 | 0.998 | 1.000 | 1.000 |
| variant-04/02-start-equals-end | 6.187 | 4.445 | 1.392 | 0.222 | 0.027 | 0.035 | 1.000 | 0.000 |
| variant-01/03-selective-compatibility | 4.727 | 3.380 | 1.399 | 0.181 | 0.026 | 0.016 | 0.000 | 0.000 |
| variant-02/07-layered-dag-before-after | 7.699 | 5.504 | 1.399 | 1.090 | 0.644 | 0.170 | 2.000 | 0.000 |
| variant-04/07-layered-dag-combined | 9.356 | 6.562 | 1.426 | 1.759 | 1.205 | 0.228 | 2.000 | 0.000 |
| variant-02/09-negative-weights | 6.307 | 4.398 | 1.434 | 0.268 | 0.047 | 0.066 | 1.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 6.394 | 4.458 | 1.434 | 0.388 | 0.036 | 0.057 | 1.000 | 0.000 |
| variant-02/01-basic | 6.473 | 4.464 | 1.450 | 0.376 | 0.053 | 0.097 | 1.000 | 0.000 |
| variant-05/06-unreachable-edge | 8.083 | 5.552 | 1.456 | 1.198 | 0.134 | 0.226 | 1.000 | 0.000 |
| variant-01/06-layered-dag | 7.972 | 5.475 | 1.456 | 1.006 | 0.892 | 0.316 | 1.000 | 0.000 |
| variant-02/02-single-salesman | 6.592 | 4.493 | 1.467 | 0.253 | 0.039 | 0.073 | 1.000 | 0.000 |
| n-queens/variant-04 | 8.245 | 5.583 | 1.477 | 0.718 | 1.614 | 0.663 | 1.000 | 1.000 |
| n-queens/variant-01 | 9.733 | 6.549 | 1.486 | 1.842 | 2.210 | 1.020 | 1.000 | 1.000 |
| variant-03/05-larger-mix | 6.642 | 4.448 | 1.493 | 0.279 | 0.178 | 0.186 | 0.000 | 0.000 |
| variant-01/02-multiple-tours | 6.754 | 4.515 | 1.496 | 0.373 | 0.229 | 0.192 | 1.000 | 0.000 |
| variant-01/05-larger-mix | 6.630 | 4.431 | 1.496 | 0.368 | 0.577 | 0.241 | 0.000 | 0.000 |
| variant-04/01-basic | 6.689 | 4.441 | 1.506 | 0.618 | 0.066 | 0.130 | 1.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 9.950 | 6.595 | 1.509 | 1.800 | 1.286 | 0.283 | 2.000 | 0.000 |
| variant-04/06-unreachable-edge | 8.342 | 5.469 | 1.526 | 1.181 | 0.136 | 0.234 | 1.000 | 0.000 |
| variant-03/06-layered-dag-cap | 10.346 | 6.619 | 1.563 | 2.504 | 1.784 | 0.542 | 2.000 | 0.000 |
| equality-generalized-tsp/02-larger | 7.003 | 4.392 | 1.594 | 0.609 | 0.095 | 0.153 | 1.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 8.885 | 5.499 | 1.616 | 1.572 | 0.196 | 0.302 | 1.000 | 0.000 |
| n-queens/variant-05 | 10.862 | 6.583 | 1.650 | 1.022 | 2.164 | 1.051 | 1.000 | 1.000 |
| variant-03/05-larger-three-depots | 7.361 | 4.440 | 1.658 | 0.837 | 0.076 | 0.187 | 1.000 | 0.000 |
| n-queens/variant-06 | 11.070 | 6.570 | 1.685 | 1.054 | 2.485 | 1.187 | 1.000 | 1.000 |
| variant-03/07-layered-dag-tight-cap | 9.481 | 5.519 | 1.718 | 1.679 | 1.367 | 0.195 | 2.000 | 0.000 |
| variant-05/05-three-depots-mixed | 9.416 | 5.463 | 1.724 | 2.269 | 0.179 | 0.314 | 1.000 | 0.000 |
| variant-01/03-asymmetric | 7.857 | 4.441 | 1.769 | 0.532 | 0.305 | 0.277 | 1.000 | 0.000 |
| variant-01/05-ring | 11.242 | 5.500 | 2.044 | 0.703 | 1.255 | 0.786 | 1.000 | 0.000 |
| variant-02/05-larger-mix | 11.244 | 5.499 | 2.045 | 0.743 | 2.907 | 0.447 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/03-agent-serialization | 11.2 | 12.4 | n/a |
| variant-04/01-basic | 11.8 | 13.2 | n/a |
| variant-04/05-larger-mix | 13.4 | 23.7 | n/a |
| send-money/send-money | 12.4 | 12.7 | n/a |
| n-queens/variant-02 | 10.8 | 12.8 | n/a |
| variant-01/04-no-path | 10.8 | 10.4 | n/a |
| variant-01/03-zero-cost-detour | 10.7 | 10.5 | n/a |
| variant-02/03-cost-dominates | 10.5 | 10.4 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 11.0 | 10.5 | n/a |
| variant-02/02-makespan-tiebreak | 10.4 | 10.5 | n/a |
| variant-03/01-basic | 10.8 | 10.5 | n/a |
| variant-02/01-basic | 10.4 | 10.2 | n/a |
| variant-03/08-tie-break-under-cap | 10.8 | 10.4 | n/a |
| variant-04/01-basic | 10.9 | 10.5 | n/a |
| variant-01/02-agent-reuse | 10.4 | 10.3 | n/a |
| variant-03/04-equal-cost-split | 10.8 | 10.6 | n/a |
| variant-01/01-basic | 10.2 | 10.5 | n/a |
| variant-01/01-basic | 10.6 | 10.6 | n/a |
| variant-01/04-subtour-unsat | 10.3 | 10.4 | n/a |
| variant-05/02-tight-bound-exact | 11.1 | 10.5 | n/a |
| variant-02/02-start-equals-end | 10.6 | 10.5 | n/a |
| variant-04/02-single-salesman | 10.9 | 10.5 | n/a |
| variant-01/01-basic | 10.7 | 10.5 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 10.6 | 10.2 | n/a |
| variant-02/03-before-forces-detour | 10.9 | 10.5 | n/a |
| variant-04/02-precedence | 11.5 | 10.7 | n/a |
| variant-02/03-too-many-salesmen-unsat | 10.6 | 10.4 | n/a |
| variant-03/03-depot-crossing-unsat | 10.8 | 10.3 | n/a |
| variant-03/06-unreachable-edge | 10.9 | 10.6 | n/a |
| variant-03/04-cost-at-cap-allowed | 10.7 | 10.5 | n/a |
| variant-03/04-incompatible-group-unsat | 10.3 | 10.4 | n/a |
| variant-04/04-window-too-tight-unsat | 11.6 | 10.5 | n/a |
| variant-05/01-basic | 11.2 | 10.5 | n/a |
| variant-02/06-layered-dag-before | 11.1 | 10.7 | n/a |
| variant-02/01-basic | 10.6 | 10.6 | n/a |
| variant-01/02-start-equals-end | 10.5 | 10.5 | n/a |
| variant-03/03-budget-forces-detour | 10.7 | 10.4 | n/a |
| variant-03/01-basic | 10.6 | 10.5 | n/a |
| variant-01/05-multi-path | 10.5 | 10.5 | n/a |
| variant-02/08-tie-break-under-ordering | 10.8 | 10.6 | n/a |
| variant-01/08-negative-weights | 10.7 | 10.4 | n/a |
| variant-03/05-budget-unsat | 10.9 | 10.5 | n/a |
| equality-generalized-tsp/01-basic | 10.6 | 10.4 | n/a |
| variant-03/01-basic | 10.5 | 10.3 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 10.6 | 10.4 | n/a |
| variant-02/05-ordering-unsat | 10.8 | 10.3 | n/a |
| variant-03/02-single-salesman | 10.6 | 10.2 | n/a |
| variant-04/05-after-and-budget-interact | 10.8 | 10.5 | n/a |
| variant-02/05-larger-asymmetric | 10.8 | 10.6 | n/a |
| variant-04/04-ordering-violates-budget | 10.9 | 10.4 | n/a |
| variant-02/04-no-compatible-agent-unsat | 10.5 | 10.3 | n/a |
| variant-04/03-window-too-tight-unsat | 10.7 | 10.4 | n/a |
| variant-01/07-cycles | 10.5 | 10.6 | n/a |
| variant-03/02-multiple-groups | 10.6 | 10.4 | n/a |
| variant-04/09-negative-weights | 10.8 | 10.4 | n/a |
| variant-03/02-start-equals-end | 10.7 | 10.6 | n/a |
| variant-01/04-no-compatible-agent-unsat | 10.5 | 10.4 | n/a |
| variant-02/04-after-forces-extension | 10.7 | 10.4 | n/a |
| variant-04/03-before-forces-detour-within-budget | 11.0 | 10.4 | n/a |
| variant-03/09-negative-weights | 10.9 | 10.5 | n/a |
| variant-05/03-revisit-too-tight-unsat | 10.9 | 10.5 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 10.7 | 10.7 | n/a |
| variant-02/04-equal-cost-split | 10.7 | 10.5 | n/a |
| variant-02/06-unreachable-edge | 10.7 | 10.6 | n/a |
| n-queens/variant-03 | 10.6 | 10.3 | n/a |
| variant-04/02-start-equals-end | 10.8 | 10.5 | n/a |
| variant-01/03-selective-compatibility | 10.4 | 10.3 | n/a |
| variant-02/07-layered-dag-before-after | 11.3 | 10.8 | n/a |
| variant-04/07-layered-dag-combined | 11.7 | 11.1 | n/a |
| variant-02/09-negative-weights | 10.8 | 10.5 | n/a |
| variant-05/04-depot-and-vertex-revisits | 10.8 | 10.5 | n/a |
| variant-02/01-basic | 10.7 | 10.4 | n/a |
| variant-05/06-unreachable-edge | 11.0 | 10.6 | n/a |
| variant-01/06-layered-dag | 11.2 | 10.7 | n/a |
| variant-02/02-single-salesman | 10.5 | 10.4 | n/a |
| n-queens/variant-04 | 10.8 | 10.7 | n/a |
| n-queens/variant-01 | 10.6 | 10.5 | n/a |
| variant-03/05-larger-mix | 10.7 | 10.5 | n/a |
| variant-01/02-multiple-tours | 11.2 | 10.6 | n/a |
| variant-01/05-larger-mix | 10.6 | 10.2 | n/a |
| variant-04/01-basic | 11.1 | 10.5 | n/a |
| variant-04/06-layered-dag-ordering-cap | 11.6 | 11.0 | n/a |
| variant-04/06-unreachable-edge | 11.5 | 10.3 | n/a |
| variant-03/06-layered-dag-cap | 11.5 | 10.8 | n/a |
| equality-generalized-tsp/02-larger | 10.8 | 10.4 | n/a |
| variant-04/05-three-depots-asymmetric-times | 11.5 | 10.6 | n/a |
| n-queens/variant-05 | 11.1 | 10.7 | n/a |
| variant-03/05-larger-three-depots | 11.0 | 10.6 | n/a |
| n-queens/variant-06 | 11.1 | 10.5 | n/a |
| variant-03/07-layered-dag-tight-cap | 11.6 | 11.1 | n/a |
| variant-05/05-three-depots-mixed | 11.5 | 10.5 | n/a |
| variant-01/03-asymmetric | 11.6 | 10.6 | n/a |
| variant-01/05-ring | 11.8 | 10.6 | n/a |
| variant-02/05-larger-mix | 11.0 | 10.6 | n/a |
