Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=1, batch=64.

| Cell | baseline | candidate | candidate/baseline | baseline/reference | candidate/reference |
|---|---:|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 7.867 [7.681, 7.911] | 7.900 [7.885, 7.935] | 1.004 | 1.514 | 1.518 |
| equality-generalized-tsp/02-larger | 7.719 [7.544, 9.162] | 7.831 [7.553, 9.153] | 1.015 | 1.483 | 1.501 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 6.501 [6.433, 6.576] | 6.574 [6.492, 7.835] | 1.011 | 1.260 | 1.268 |
| variant-01/01-basic | 6.468 [6.432, 6.533] | 6.543 [6.489, 7.888] | 1.012 | 1.248 | 1.262 |
| variant-01/02-start-equals-end | 6.521 [6.478, 7.887] | 6.602 [6.389, 6.651] | 1.012 | 1.259 | 1.267 |
| variant-01/03-zero-cost-detour | 6.497 [6.465, 6.594] | 6.517 [6.483, 7.927] | 1.003 | 1.253 | 1.254 |
| variant-01/04-no-path | 6.543 [6.477, 6.575] | 6.618 [6.507, 7.876] | 1.011 | 1.259 | 1.273 |
| variant-01/05-multi-path | 7.751 [7.728, 7.832] | 7.772 [6.503, 7.888] | 1.003 | 1.498 | 1.500 |
| variant-01/06-layered-dag | 9.003 [8.977, 10.335] | 10.319 [10.291, 10.408] | 1.146 | 1.737 | 1.991 |
| variant-01/07-cycles | 7.808 [7.539, 7.867] | 7.809 [7.708, 7.870] | 1.000 | 1.510 | 1.507 |
| variant-01/08-negative-weights | 6.561 [6.464, 7.884] | 7.793 [6.487, 7.975] | 1.188 | 1.267 | 1.500 |
| variant-02/01-basic | 6.504 [6.470, 7.848] | 6.538 [6.490, 7.919] | 1.005 | 1.258 | 1.259 |
| variant-02/02-start-equals-end | 6.567 [6.499, 7.890] | 7.771 [6.465, 7.883] | 1.183 | 1.263 | 1.500 |
| variant-02/03-before-forces-detour | 7.608 [6.473, 7.887] | 7.857 [6.464, 7.895] | 1.033 | 1.460 | 1.511 |
| variant-02/04-after-forces-extension | 7.873 [6.456, 7.883] | 7.875 [6.485, 7.947] | 1.000 | 1.515 | 1.515 |
| variant-02/05-ordering-unsat | 6.519 [6.480, 7.860] | 6.541 [6.493, 7.988] | 1.003 | 1.253 | 1.259 |
| variant-02/06-layered-dag-before | 10.289 [10.260, 10.357] | 10.269 [10.152, 11.674] | 0.998 | 1.599 | 1.589 |
| variant-02/07-layered-dag-before-after | 10.300 [10.203, 10.367] | 10.347 [10.146, 10.415] | 1.005 | 1.602 | 1.614 |
| variant-02/08-tie-break-under-ordering | 7.842 [7.756, 7.928] | 7.853 [7.779, 8.001] | 1.001 | 1.513 | 1.513 |
| variant-02/09-negative-weights | 7.775 [6.467, 7.890] | 7.800 [7.741, 7.927] | 1.003 | 1.502 | 1.506 |
| variant-03/01-basic | 6.540 [6.378, 6.545] | 7.752 [6.515, 7.898] | 1.185 | 1.258 | 1.495 |
| variant-03/02-start-equals-end | 6.511 [6.361, 6.545] | 6.618 [6.486, 7.871] | 1.016 | 1.253 | 1.276 |
| variant-03/03-budget-forces-detour | 6.521 [6.475, 7.856] | 6.548 [6.490, 7.879] | 1.004 | 1.258 | 1.261 |
| variant-03/04-cost-at-cap-allowed | 6.533 [6.396, 7.534] | 7.546 [6.473, 7.896] | 1.155 | 1.256 | 1.454 |
| variant-03/05-budget-unsat | 7.766 [6.449, 7.899] | 7.877 [7.733, 9.060] | 1.014 | 1.495 | 1.520 |
| variant-03/06-layered-dag-cap | 11.582 [11.533, 12.773] | 11.621 [11.522, 12.817] | 1.003 | 1.800 | 1.798 |
| variant-03/07-layered-dag-tight-cap | 12.916 [12.556, 14.266] | 12.793 [11.531, 12.980] | 0.990 | 2.012 | 1.987 |
| variant-03/08-tie-break-under-cap | 7.824 [7.785, 9.364] | 7.821 [7.775, 7.917] | 1.000 | 1.503 | 1.503 |
| variant-03/09-negative-weights | 7.851 [6.456, 7.895] | 7.837 [7.560, 7.905] | 0.998 | 1.516 | 1.512 |
| variant-04/01-basic | 6.490 [6.455, 7.809] | 7.799 [6.464, 7.909] | 1.202 | 1.251 | 1.504 |
| variant-04/02-start-equals-end | 7.721 [6.466, 7.805] | 7.833 [6.475, 7.855] | 1.015 | 1.497 | 1.509 |
| variant-04/03-before-forces-detour-within-budget | 7.765 [7.751, 7.859] | 7.760 [6.536, 7.860] | 0.999 | 1.499 | 1.493 |
| variant-04/04-ordering-violates-budget | 6.546 [6.444, 7.799] | 7.839 [6.486, 7.920] | 1.198 | 1.264 | 1.511 |
| variant-04/05-after-and-budget-interact | 7.780 [7.713, 7.831] | 7.788 [7.733, 9.046] | 1.001 | 1.501 | 1.500 |
| variant-04/06-layered-dag-ordering-cap | 11.486 [11.298, 11.562] | 11.569 [11.429, 12.966] | 1.007 | 1.785 | 1.797 |
| variant-04/07-layered-dag-combined | 11.545 [11.318, 11.556] | 11.666 [11.535, 12.940] | 1.011 | 1.794 | 1.813 |
| variant-04/08-tie-break-under-ordering-and-cap | 7.801 [7.639, 7.881] | 7.802 [7.675, 7.906] | 1.000 | 1.509 | 1.503 |
| variant-04/09-negative-weights | 7.796 [7.723, 7.854] | 7.783 [7.712, 7.823] | 0.998 | 1.501 | 1.503 |
| variant-01/01-basic | 6.540 [6.526, 6.637] | 6.502 [6.427, 6.693] | 0.994 | 1.665 | 1.655 |
| variant-01/02-agent-reuse | 6.514 [6.472, 6.542] | 6.507 [6.493, 6.541] | 0.999 | 1.669 | 1.641 |
| variant-01/03-selective-compatibility | 6.539 [6.373, 6.619] | 6.507 [6.481, 6.604] | 0.995 | 1.672 | 1.667 |
| variant-01/04-no-compatible-agent-unsat | 6.511 [6.344, 6.556] | 6.500 [6.398, 6.558] | 0.998 | 1.659 | 1.664 |
| variant-01/05-larger-mix | 7.686 [6.467, 7.805] | 7.785 [6.135, 7.815] | 1.013 | 1.933 | 1.567 |
| variant-02/01-basic | 7.725 [6.447, 7.820] | 7.781 [6.469, 7.816] | 1.007 | 1.494 | 1.505 |
| variant-02/02-makespan-tiebreak | 6.515 [6.452, 7.990] | 7.802 [6.458, 7.855] | 1.198 | 1.255 | 1.509 |
| variant-02/03-cost-dominates | 7.791 [6.469, 7.879] | 7.775 [7.731, 7.803] | 0.998 | 1.500 | 1.496 |
| variant-02/04-no-compatible-agent-unsat | 7.766 [6.453, 7.836] | 6.497 [6.465, 7.820] | 0.837 | 1.959 | 1.653 |
| variant-02/05-larger-mix | 10.307 [10.219, 10.380] | 10.319 [9.014, 10.380] | 1.001 | 1.985 | 1.992 |
| variant-03/01-basic | 6.519 [6.478, 7.939] | 6.522 [6.510, 6.634] | 1.000 | 1.260 | 1.260 |
| variant-03/02-multiple-groups | 6.489 [6.447, 7.801] | 6.502 [6.322, 6.562] | 1.002 | 1.258 | 1.250 |
| variant-03/03-mixed-grouped-ungrouped | 6.501 [6.354, 7.847] | 6.502 [6.480, 7.823] | 1.000 | 1.659 | 1.660 |
| variant-03/04-incompatible-group-unsat | 6.510 [6.471, 6.566] | 6.516 [6.497, 6.543] | 1.001 | 1.658 | 1.666 |
| variant-03/05-larger-mix | 7.765 [6.431, 9.027] | 7.777 [6.490, 7.835] | 1.002 | 1.506 | 1.509 |
| variant-04/01-basic | 11.626 [10.184, 11.734] | 11.550 [10.041, 11.714] | 0.993 | 0.483 | 0.480 |
| variant-04/02-precedence | 9.120 [9.001, 9.180] | 9.116 [9.031, 9.223] | 1.000 | 1.407 | 1.410 |
| variant-04/03-agent-serialization | 10.302 [10.163, 10.421] | 10.327 [10.252, 10.434] | 1.002 | 0.388 | 0.392 |
| variant-04/04-window-too-tight-unsat | 7.862 [7.763, 7.920] | 7.875 [7.636, 7.949] | 1.002 | 1.508 | 1.515 |
| variant-04/05-larger-mix | 37.099 [35.769, 37.321] | 36.034 [35.794, 38.298] | 0.971 | 0.187 | 0.183 |
| variant-01/01-basic | 7.933 [7.897, 8.057] | 7.992 [7.924, 8.069] | 1.007 | 1.507 | 1.518 |
| variant-01/02-multiple-tours | 7.855 [7.758, 7.883] | 7.826 [7.782, 7.891] | 0.996 | 1.515 | 1.508 |
| variant-01/03-asymmetric | 7.848 [7.732, 9.049] | 7.817 [7.735, 8.906] | 0.996 | 1.514 | 1.508 |
| variant-01/04-subtour-unsat | 6.519 [6.477, 6.574] | 6.508 [6.442, 6.520] | 0.998 | 1.258 | 1.255 |
| variant-01/05-ring | 9.048 [8.913, 10.394] | 8.963 [8.888, 10.092] | 0.991 | 1.744 | 1.723 |
| variant-02/01-basic | 7.782 [7.670, 7.795] | 7.869 [6.486, 7.907] | 1.011 | 1.505 | 1.517 |
| variant-02/02-single-salesman | 7.773 [6.485, 8.176] | 6.544 [6.460, 7.851] | 0.842 | 1.494 | 1.257 |
| variant-02/03-too-many-salesmen-unsat | 6.497 [6.459, 7.843] | 7.742 [6.465, 9.198] | 1.192 | 1.246 | 1.485 |
| variant-02/04-equal-cost-split | 7.727 [6.453, 7.895] | 7.828 [6.479, 9.237] | 1.013 | 1.488 | 1.507 |
| variant-02/05-larger-asymmetric | 7.873 [7.730, 7.917] | 7.734 [7.709, 7.879] | 0.982 | 1.518 | 1.490 |
| variant-02/06-unreachable-edge | 7.726 [7.617, 7.902] | 7.779 [7.600, 7.895] | 1.007 | 1.484 | 1.496 |
| variant-03/01-basic | 7.779 [7.578, 7.904] | 6.552 [6.510, 7.845] | 0.842 | 1.502 | 1.261 |
| variant-03/02-single-salesman | 7.769 [6.463, 7.855] | 6.596 [6.447, 7.892] | 0.849 | 1.498 | 1.270 |
| variant-03/03-depot-crossing-unsat | 7.826 [6.476, 7.850] | 7.776 [6.466, 7.846] | 0.994 | 1.508 | 1.498 |
| variant-03/04-equal-cost-split | 7.888 [7.784, 9.076] | 7.830 [6.478, 7.917] | 0.993 | 1.515 | 1.511 |
| variant-03/05-larger-three-depots | 9.017 [7.781, 9.114] | 7.797 [7.721, 9.159] | 0.865 | 1.737 | 1.505 |
| variant-03/06-unreachable-edge | 7.812 [7.772, 9.101] | 7.828 [7.656, 7.854] | 1.002 | 1.509 | 1.505 |
| variant-04/01-basic | 8.956 [7.740, 10.669] | 8.855 [7.725, 9.128] | 0.989 | 1.724 | 1.704 |
| variant-04/02-single-salesman | 7.836 [7.681, 7.891] | 7.781 [7.738, 7.894] | 0.993 | 1.510 | 1.506 |
| variant-04/03-window-too-tight-unsat | 7.814 [7.559, 7.896] | 7.830 [7.738, 7.876] | 1.002 | 1.503 | 1.505 |
| variant-04/04-depot-window-too-tight-unsat | 7.860 [7.747, 7.955] | 7.845 [7.731, 7.883] | 0.998 | 1.513 | 1.512 |
| variant-04/05-three-depots-asymmetric-times | 10.271 [10.176, 11.652] | 10.323 [10.272, 11.661] | 1.005 | 1.983 | 1.992 |
| variant-04/06-unreachable-edge | 10.304 [10.083, 10.362] | 10.393 [10.266, 10.462] | 1.009 | 1.995 | 2.012 |
| variant-05/01-basic | 9.103 [7.728, 9.174] | 9.074 [8.998, 9.202] | 0.997 | 1.762 | 1.752 |
| variant-05/02-tight-bound-exact | 7.793 [7.744, 7.879] | 7.871 [7.712, 8.883] | 1.010 | 1.509 | 1.515 |
| variant-05/03-revisit-too-tight-unsat | 7.772 [7.665, 7.966] | 7.748 [7.741, 7.851] | 0.997 | 1.498 | 1.492 |
| variant-05/04-depot-and-vertex-revisits | 7.821 [7.790, 7.909] | 7.811 [7.729, 7.875] | 0.999 | 1.509 | 1.508 |
| variant-05/05-three-depots-mixed | 10.307 [10.232, 11.566] | 10.303 [10.276, 11.575] | 1.000 | 1.989 | 1.995 |
| variant-05/06-unreachable-edge | 10.380 [10.166, 11.550] | 10.378 [9.007, 10.412] | 1.000 | 2.007 | 2.005 |
| n-queens/variant-01 | 16.585 [16.461, 17.963] | 16.618 [16.434, 16.670] | 1.002 | 2.550 | 2.567 |
| n-queens/variant-02 | 56.495 [56.374, 58.505] | 57.418 [56.318, 58.279] | 1.016 | 0.454 | 0.466 |
| n-queens/variant-03 | 15.520 [15.142, 15.548] | 15.500 [14.297, 15.564] | 0.999 | 2.389 | 2.378 |
| n-queens/variant-04 | 10.319 [10.190, 10.446] | 10.395 [10.355, 10.566] | 1.007 | 1.599 | 1.608 |
| n-queens/variant-05 | 10.349 [10.321, 10.413] | 10.363 [10.140, 10.406] | 1.001 | 1.601 | 1.600 |
| n-queens/variant-06 | 10.373 [10.351, 10.404] | 10.419 [10.339, 10.427] | 1.004 | 1.611 | 1.599 |
| send-money/send-money | 15.366 [15.310, 15.564] | 15.472 [15.223, 15.505] | 1.007 | 1.210 | 1.215 |

Reference wall time, ms, same notation.

| Cell | baseline | candidate |
|---|---:|---:|
| equality-generalized-tsp/01-basic | 5.196 [5.053, 5.242] | 5.205 [5.193, 5.266] |
| equality-generalized-tsp/02-larger | 5.206 [5.087, 5.237] | 5.218 [5.186, 5.236] |
| equality-generalized-tsp/03-unreachable-subset-unsat | 5.160 [3.910, 5.187] | 5.182 [3.917, 5.193] |
| variant-01/01-basic | 5.181 [5.172, 5.220] | 5.185 [5.164, 5.208] |
| variant-01/02-start-equals-end | 5.181 [5.169, 5.247] | 5.210 [5.167, 5.329] |
| variant-01/03-zero-cost-detour | 5.187 [5.163, 5.206] | 5.198 [5.150, 5.204] |
| variant-01/04-no-path | 5.196 [5.171, 5.225] | 5.198 [3.935, 5.205] |
| variant-01/05-multi-path | 5.173 [5.164, 5.203] | 5.183 [5.169, 5.205] |
| variant-01/06-layered-dag | 5.183 [5.161, 6.461] | 5.182 [5.160, 6.468] |
| variant-01/07-cycles | 5.171 [5.164, 5.206] | 5.182 [5.158, 5.223] |
| variant-01/08-negative-weights | 5.178 [4.961, 5.198] | 5.194 [5.188, 5.198] |
| variant-02/01-basic | 5.171 [5.140, 5.196] | 5.193 [5.181, 5.310] |
| variant-02/02-start-equals-end | 5.199 [5.016, 5.205] | 5.179 [5.169, 5.203] |
| variant-02/03-before-forces-detour | 5.210 [5.186, 5.237] | 5.200 [5.169, 5.213] |
| variant-02/04-after-forces-extension | 5.198 [5.177, 5.306] | 5.197 [5.078, 5.214] |
| variant-02/05-ordering-unsat | 5.203 [5.133, 5.205] | 5.197 [5.179, 5.263] |
| variant-02/06-layered-dag-before | 6.434 [5.187, 6.461] | 6.464 [6.453, 6.526] |
| variant-02/07-layered-dag-before-after | 6.428 [5.194, 6.458] | 6.411 [5.194, 6.519] |
| variant-02/08-tie-break-under-ordering | 5.181 [5.163, 5.214] | 5.189 [5.172, 6.639] |
| variant-02/09-negative-weights | 5.177 [5.048, 5.198] | 5.180 [5.106, 5.224] |
| variant-03/01-basic | 5.196 [5.157, 5.201] | 5.186 [5.177, 5.235] |
| variant-03/02-start-equals-end | 5.197 [4.989, 5.203] | 5.185 [5.068, 5.206] |
| variant-03/03-budget-forces-detour | 5.183 [5.159, 5.208] | 5.191 [5.188, 5.243] |
| variant-03/04-cost-at-cap-allowed | 5.201 [5.179, 5.206] | 5.189 [5.182, 5.215] |
| variant-03/05-budget-unsat | 5.193 [5.141, 5.212] | 5.182 [5.175, 5.213] |
| variant-03/06-layered-dag-cap | 6.433 [6.419, 6.477] | 6.462 [6.451, 7.754] |
| variant-03/07-layered-dag-tight-cap | 6.420 [6.412, 7.739] | 6.438 [6.399, 7.725] |
| variant-03/08-tie-break-under-cap | 5.206 [5.174, 5.222] | 5.204 [5.177, 5.232] |
| variant-03/09-negative-weights | 5.178 [5.140, 5.205] | 5.182 [5.162, 5.210] |
| variant-04/01-basic | 5.189 [4.923, 5.199] | 5.186 [5.174, 5.217] |
| variant-04/02-start-equals-end | 5.159 [5.155, 5.203] | 5.192 [5.181, 5.206] |
| variant-04/03-before-forces-detour-within-budget | 5.179 [5.165, 5.205] | 5.198 [5.173, 5.250] |
| variant-04/04-ordering-violates-budget | 5.179 [4.946, 5.212] | 5.187 [5.157, 5.242] |
| variant-04/05-after-and-budget-interact | 5.184 [5.169, 5.242] | 5.191 [5.173, 5.249] |
| variant-04/06-layered-dag-ordering-cap | 6.436 [6.432, 6.476] | 6.438 [6.426, 7.791] |
| variant-04/07-layered-dag-combined | 6.436 [6.412, 6.448] | 6.435 [6.408, 6.464] |
| variant-04/08-tie-break-under-ordering-and-cap | 5.171 [5.000, 5.181] | 5.190 [5.155, 5.217] |
| variant-04/09-negative-weights | 5.196 [5.185, 5.248] | 5.178 [5.157, 5.191] |
| variant-01/01-basic | 3.927 [3.903, 5.200] | 3.929 [3.902, 5.184] |
| variant-01/02-agent-reuse | 3.903 [3.872, 5.198] | 3.965 [3.879, 5.197] |
| variant-01/03-selective-compatibility | 3.912 [3.883, 5.188] | 3.903 [3.795, 5.196] |
| variant-01/04-no-compatible-agent-unsat | 3.924 [3.891, 5.184] | 3.907 [3.883, 4.974] |
| variant-01/05-larger-mix | 3.977 [3.890, 5.182] | 4.968 [3.903, 5.186] |
| variant-02/01-basic | 5.169 [3.957, 5.275] | 5.172 [3.917, 5.202] |
| variant-02/02-makespan-tiebreak | 5.192 [5.173, 5.231] | 5.169 [5.145, 5.233] |
| variant-02/03-cost-dominates | 5.193 [3.913, 5.199] | 5.196 [5.185, 5.208] |
| variant-02/04-no-compatible-agent-unsat | 3.964 [3.881, 5.182] | 3.930 [3.888, 5.366] |
| variant-02/05-larger-mix | 5.191 [5.144, 5.220] | 5.181 [5.030, 5.195] |
| variant-03/01-basic | 5.176 [3.887, 5.184] | 5.175 [3.905, 5.224] |
| variant-03/02-multiple-groups | 5.157 [3.922, 5.222] | 5.203 [3.928, 5.205] |
| variant-03/03-mixed-grouped-ungrouped | 3.918 [3.894, 5.162] | 3.917 [3.897, 5.172] |
| variant-03/04-incompatible-group-unsat | 3.927 [3.882, 5.183] | 3.912 [3.901, 5.172] |
| variant-03/05-larger-mix | 5.157 [3.878, 5.180] | 5.152 [3.883, 5.173] |
| variant-04/01-basic | 24.058 [23.845, 25.341] | 24.037 [23.997, 24.040] |
| variant-04/02-precedence | 6.483 [6.188, 6.523] | 6.463 [6.443, 6.493] |
| variant-04/03-agent-serialization | 26.544 [25.078, 26.563] | 26.344 [25.299, 26.548] |
| variant-04/04-window-too-tight-unsat | 5.215 [5.192, 5.282] | 5.199 [5.184, 5.245] |
| variant-04/05-larger-mix | 198.406 [179.843, 202.342] | 197.394 [193.566, 205.004] |
| variant-01/01-basic | 5.263 [5.208, 5.300] | 5.266 [5.204, 5.307] |
| variant-01/02-multiple-tours | 5.185 [5.166, 5.204] | 5.190 [5.175, 5.254] |
| variant-01/03-asymmetric | 5.185 [5.128, 5.209] | 5.184 [5.161, 5.202] |
| variant-01/04-subtour-unsat | 5.184 [3.907, 5.204] | 5.185 [3.890, 5.214] |
| variant-01/05-ring | 5.189 [5.091, 5.204] | 5.201 [5.183, 5.208] |
| variant-02/01-basic | 5.171 [5.022, 5.211] | 5.188 [5.171, 5.196] |
| variant-02/02-single-salesman | 5.204 [5.196, 5.227] | 5.205 [5.161, 5.217] |
| variant-02/03-too-many-salesmen-unsat | 5.213 [5.168, 5.227] | 5.213 [5.175, 5.234] |
| variant-02/04-equal-cost-split | 5.194 [5.168, 5.222] | 5.195 [5.100, 5.211] |
| variant-02/05-larger-asymmetric | 5.188 [5.166, 5.215] | 5.192 [5.187, 5.246] |
| variant-02/06-unreachable-edge | 5.206 [5.149, 6.456] | 5.201 [5.185, 5.231] |
| variant-03/01-basic | 5.180 [5.156, 5.203] | 5.195 [5.180, 5.211] |
| variant-03/02-single-salesman | 5.185 [5.106, 5.226] | 5.194 [5.190, 5.204] |
| variant-03/03-depot-crossing-unsat | 5.191 [5.178, 5.199] | 5.191 [5.169, 5.214] |
| variant-03/04-equal-cost-split | 5.205 [5.175, 5.228] | 5.182 [5.160, 5.216] |
| variant-03/05-larger-three-depots | 5.190 [5.156, 5.203] | 5.181 [5.162, 5.197] |
| variant-03/06-unreachable-edge | 5.176 [5.161, 5.209] | 5.203 [5.177, 5.234] |
| variant-04/01-basic | 5.196 [5.184, 5.228] | 5.197 [5.175, 5.215] |
| variant-04/02-single-salesman | 5.190 [5.021, 5.224] | 5.166 [4.938, 5.219] |
| variant-04/03-window-too-tight-unsat | 5.199 [5.186, 5.203] | 5.202 [5.174, 5.221] |
| variant-04/04-depot-window-too-tight-unsat | 5.194 [5.166, 5.219] | 5.190 [5.171, 5.238] |
| variant-04/05-three-depots-asymmetric-times | 5.180 [5.160, 5.198] | 5.183 [5.148, 5.220] |
| variant-04/06-unreachable-edge | 5.165 [5.161, 5.212] | 5.165 [5.092, 5.180] |
| variant-05/01-basic | 5.167 [5.162, 5.200] | 5.178 [5.160, 5.204] |
| variant-05/02-tight-bound-exact | 5.165 [5.045, 5.193] | 5.197 [5.177, 5.221] |
| variant-05/03-revisit-too-tight-unsat | 5.187 [5.160, 5.204] | 5.193 [5.172, 5.208] |
| variant-05/04-depot-and-vertex-revisits | 5.183 [5.174, 5.213] | 5.178 [5.159, 5.217] |
| variant-05/05-three-depots-mixed | 5.181 [5.157, 5.199] | 5.164 [5.146, 5.208] |
| variant-05/06-unreachable-edge | 5.172 [5.152, 6.525] | 5.177 [5.148, 5.194] |
| n-queens/variant-01 | 6.504 [5.224, 6.572] | 6.475 [5.111, 6.510] |
| n-queens/variant-02 | 124.516 [124.100, 125.726] | 123.173 [123.126, 124.620] |
| n-queens/variant-03 | 6.496 [6.472, 6.560] | 6.518 [6.495, 6.537] |
| n-queens/variant-04 | 6.455 [6.441, 6.492] | 6.464 [6.432, 6.492] |
| n-queens/variant-05 | 6.464 [6.437, 6.522] | 6.476 [6.449, 6.555] |
| n-queens/variant-06 | 6.440 [6.421, 6.466] | 6.517 [6.450, 6.533] |
| send-money/send-money | 12.704 [12.571, 12.780] | 12.732 [12.547, 12.771] |

Counters of report candidate: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| equality-generalized-tsp/01-basic | 0 | 1 | 4 | 12964 | 1.886 |
| equality-generalized-tsp/02-larger | 0 | 1 | 6 | 24442 | 2.540 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 0 | 0 | 0 | 2452 | 1.255 |
| variant-01/01-basic | 0 | 1 | 2 | 2685 | 1.326 |
| variant-01/02-start-equals-end | 0 | 1 | 1 | 1769 | 1.253 |
| variant-01/03-zero-cost-detour | 0 | 1 | 2 | 2243 | 1.292 |
| variant-01/04-no-path | 0 | 0 | 0 | 1226 | 1.280 |
| variant-01/05-multi-path | 0 | 1 | 3 | 7958 | 1.644 |
| variant-01/06-layered-dag | 0 | 1 | 6 | 124450 | 4.204 |
| variant-01/07-cycles | 0 | 1 | 3 | 15771 | 2.099 |
| variant-01/08-negative-weights | 0 | 1 | 1 | 4238 | 1.567 |
| variant-02/01-basic | 0 | 1 | 2 | 2943 | 1.500 |
| variant-02/02-start-equals-end | 0 | 1 | 1 | 2274 | 1.390 |
| variant-02/03-before-forces-detour | 0 | 1 | 1 | 3537 | 1.640 |
| variant-02/04-after-forces-extension | 0 | 1 | 1 | 3324 | 1.584 |
| variant-02/05-ordering-unsat | 0 | 0 | 0 | 1653 | 1.394 |
| variant-02/06-layered-dag-before | 0 | 2 | 6 | 141386 | 4.792 |
| variant-02/07-layered-dag-before-after | 0 | 1 | 4 | 111619 | 4.521 |
| variant-02/08-tie-break-under-ordering | 0 | 1 | 2 | 4851 | 1.757 |
| variant-02/09-negative-weights | 0 | 1 | 1 | 4756 | 1.846 |
| variant-03/01-basic | 0 | 1 | 2 | 2861 | 1.540 |
| variant-03/02-start-equals-end | 0 | 1 | 1 | 2140 | 1.437 |
| variant-03/03-budget-forces-detour | 0 | 1 | 1 | 3158 | 1.531 |
| variant-03/04-cost-at-cap-allowed | 0 | 1 | 1 | 1873 | 1.521 |
| variant-03/05-budget-unsat | 0 | 0 | 0 | 6513 | 1.928 |
| variant-03/06-layered-dag-cap | 0 | 2 | 4 | 244377 | 6.108 |
| variant-03/07-layered-dag-tight-cap | 0 | 4 | 4 | 320181 | 6.618 |
| variant-03/08-tie-break-under-cap | 0 | 1 | 2 | 5684 | 1.830 |
| variant-03/09-negative-weights | 0 | 1 | 1 | 4874 | 1.876 |
| variant-04/01-basic | 0 | 1 | 2 | 3119 | 1.626 |
| variant-04/02-start-equals-end | 0 | 1 | 1 | 2645 | 1.617 |
| variant-04/03-before-forces-detour-within-budget | 0 | 1 | 1 | 4098 | 1.822 |
| variant-04/04-ordering-violates-budget | 0 | 0 | 0 | 3479 | 1.832 |
| variant-04/05-after-and-budget-interact | 0 | 1 | 1 | 6185 | 1.957 |
| variant-04/06-layered-dag-ordering-cap | 0 | 2 | 4 | 171295 | 6.331 |
| variant-04/07-layered-dag-combined | 0 | 1 | 5 | 194650 | 6.366 |
| variant-04/08-tie-break-under-ordering-and-cap | 0 | 1 | 2 | 11319 | 2.358 |
| variant-04/09-negative-weights | 0 | 1 | 1 | 5400 | 2.023 |
| variant-01/01-basic | 0 | 1 | 4 | 3124 | 1.033 |
| variant-01/02-agent-reuse | 0 | 1 | 8 | 7091 | 1.242 |
| variant-01/03-selective-compatibility | 0 | 1 | 4 | 3878 | 1.098 |
| variant-01/04-no-compatible-agent-unsat | 0 | 0 | 0 | 1324 | 0.876 |
| variant-01/05-larger-mix | 0 | 1 | 29 | 55072 | 1.809 |
| variant-02/01-basic | 0 | 1 | 4 | 6592 | 1.626 |
| variant-02/02-makespan-tiebreak | 0 | 3 | 7 | 11829 | 1.864 |
| variant-02/03-cost-dominates | 0 | 1 | 8 | 12794 | 1.941 |
| variant-02/04-no-compatible-agent-unsat | 0 | 0 | 0 | 2599 | 1.489 |
| variant-02/05-larger-mix | 0 | 1 | 16 | 253765 | 4.515 |
| variant-03/01-basic | 0 | 2 | 2 | 3255 | 1.183 |
| variant-03/02-multiple-groups | 0 | 4 | 4 | 7717 | 1.431 |
| variant-03/03-mixed-grouped-ungrouped | 0 | 2 | 4 | 5291 | 1.314 |
| variant-03/04-incompatible-group-unsat | 0 | 0 | 0 | 1485 | 1.020 |
| variant-03/05-larger-mix | 0 | 1 | 21 | 25698 | 1.705 |
| variant-04/01-basic | 0 | 81 | 83 | 230275 | 5.460 |
| variant-04/02-precedence | 0 | 3 | 8 | 29218 | 3.145 |
| variant-04/03-agent-serialization | 0 | 30 | 30 | 91655 | 4.415 |
| variant-04/04-window-too-tight-unsat | 0 | 0 | 0 | 3719 | 1.976 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 8530606 | 30.682 |
| variant-01/01-basic | 0 | 1 | 2 | 8394 | 1.563 |
| variant-01/02-multiple-tours | 0 | 2 | 5 | 39369 | 2.134 |
| variant-01/03-asymmetric | 0 | 1 | 4 | 63254 | 2.526 |
| variant-01/04-subtour-unsat | 0 | 0 | 0 | 2619 | 1.167 |
| variant-01/05-ring | 0 | 2 | 4 | 164307 | 3.837 |
| variant-02/01-basic | 0 | 2 | 2 | 9345 | 1.861 |
| variant-02/02-single-salesman | 0 | 1 | 2 | 6221 | 1.588 |
| variant-02/03-too-many-salesmen-unsat | 0 | 0 | 0 | 4097 | 1.556 |
| variant-02/04-equal-cost-split | 0 | 2 | 2 | 7609 | 1.676 |
| variant-02/05-larger-asymmetric | 0 | 2 | 2 | 11113 | 2.012 |
| variant-02/06-unreachable-edge | 0 | 2 | 2 | 12838 | 2.158 |
| variant-03/01-basic | 0 | 1 | 1 | 5853 | 1.654 |
| variant-03/02-single-salesman | 0 | 1 | 2 | 5982 | 1.523 |
| variant-03/03-depot-crossing-unsat | 0 | 0 | 0 | 4036 | 1.620 |
| variant-03/04-equal-cost-split | 0 | 2 | 2 | 13113 | 2.039 |
| variant-03/05-larger-three-depots | 0 | 1 | 1 | 16313 | 2.912 |
| variant-03/06-unreachable-edge | 0 | 1 | 1 | 9790 | 2.153 |
| variant-04/01-basic | 0 | 1 | 1 | 10027 | 2.786 |
| variant-04/02-single-salesman | 0 | 1 | 1 | 6792 | 2.399 |
| variant-04/03-window-too-tight-unsat | 0 | 0 | 0 | 3985 | 2.109 |
| variant-04/04-depot-window-too-tight-unsat | 0 | 0 | 0 | 2725 | 1.973 |
| variant-04/05-three-depots-asymmetric-times | 0 | 1 | 1 | 25990 | 4.858 |
| variant-04/06-unreachable-edge | 0 | 1 | 1 | 20302 | 4.258 |
| variant-05/01-basic | 0 | 1 | 1 | 10029 | 2.851 |
| variant-05/02-tight-bound-exact | 0 | 1 | 1 | 4987 | 2.283 |
| variant-05/03-revisit-too-tight-unsat | 0 | 0 | 0 | 4113 | 2.155 |
| variant-05/04-depot-and-vertex-revisits | 0 | 1 | 1 | 5081 | 2.275 |
| variant-05/05-three-depots-mixed | 0 | 1 | 1 | 26048 | 5.021 |
| variant-05/06-unreachable-edge | 0 | 1 | 1 | 20302 | 4.325 |
| n-queens/variant-01 | 0 | 92 | 92 | 454988 | 10.662 |
| n-queens/variant-02 | 0 | 92 | 92 | 15076428 | 51.295 |
| n-queens/variant-03 | 0 | 92 | 92 | 550518 | 8.921 |
| n-queens/variant-04 | 0 | 92 | 92 | 362933 | 4.494 |
| n-queens/variant-05 | 0 | 92 | 92 | 521733 | 4.551 |
| n-queens/variant-06 | 0 | 92 | 92 | 586927 | 4.761 |
| send-money/send-money | 0 | 1 | 1 | 472288 | 9.298 |

Against the reference: report baseline, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=1, batch=64; search default): faster on 4 of 94 cells where both passed (4.2%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership, each summed over its intervals, which can overlap within or across threads, so the parts need not add up to the native median; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 37.099 | 198.406 | 0.187 | 7.458 | 27.179 | 8.256 | 77.000 | 118.000 |
| variant-04/03-agent-serialization | 10.302 | 26.544 | 0.388 | 2.226 | 0.505 | 0.108 | 22.000 | 0.000 |
| n-queens/variant-02 | 56.495 | 124.516 | 0.454 | 6.966 | 131.275 | 0.762 | 1.000 | 118.000 |
| variant-04/01-basic | 11.626 | 24.058 | 0.483 | 2.329 | 1.010 | 0.256 | 15.000 | 5.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership, each summed over its intervals, which can overlap within or across threads, so the parts need not add up to the native median; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| send-money/send-money | 15.366 | 12.704 | 1.210 | 5.216 | 3.526 | 0.150 | 7.000 | 1.000 |
| variant-02/03-too-many-salesmen-unsat | 6.497 | 5.213 | 1.246 | 0.596 | 0.048 | 0.012 | 1.000 | 0.000 |
| variant-01/01-basic | 6.468 | 5.181 | 1.248 | 0.368 | 0.044 | 0.011 | 1.000 | 0.000 |
| variant-04/01-basic | 6.490 | 5.189 | 1.251 | 0.445 | 0.041 | 0.012 | 1.000 | 0.000 |
| variant-01/03-zero-cost-detour | 6.497 | 5.187 | 1.253 | 0.337 | 0.036 | 0.010 | 1.000 | 0.000 |
| variant-03/02-start-equals-end | 6.511 | 5.197 | 1.253 | 0.377 | 0.032 | 0.020 | 1.000 | 0.000 |
| variant-02/05-ordering-unsat | 6.519 | 5.203 | 1.253 | 0.359 | 0.032 | 0.007 | 1.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 6.515 | 5.192 | 1.255 | 0.719 | 0.097 | 0.029 | 1.000 | 0.000 |
| variant-03/04-cost-at-cap-allowed | 6.533 | 5.201 | 1.256 | 0.400 | 0.030 | 0.009 | 1.000 | 0.000 |
| variant-02/01-basic | 6.504 | 5.171 | 1.258 | 0.382 | 0.044 | 0.012 | 1.000 | 0.000 |
| variant-01/04-subtour-unsat | 6.519 | 5.184 | 1.258 | 0.398 | 0.032 | 0.012 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 6.521 | 5.183 | 1.258 | 0.433 | 0.042 | 0.012 | 1.000 | 0.000 |
| variant-03/02-multiple-groups | 6.489 | 5.157 | 1.258 | 0.422 | 0.060 | 0.018 | 1.000 | 0.000 |
| variant-03/01-basic | 6.540 | 5.196 | 1.258 | 0.413 | 0.043 | 0.012 | 1.000 | 0.000 |
| variant-01/02-start-equals-end | 6.521 | 5.181 | 1.259 | 0.305 | 0.029 | 0.018 | 1.000 | 0.000 |
| variant-01/04-no-path | 6.543 | 5.196 | 1.259 | 0.312 | 0.026 | 0.007 | 1.000 | 0.000 |
| variant-03/01-basic | 6.519 | 5.176 | 1.260 | 0.298 | 0.047 | 0.013 | 1.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 6.501 | 5.160 | 1.260 | 0.345 | 0.033 | 0.012 | 1.000 | 0.000 |
| variant-02/02-start-equals-end | 6.567 | 5.199 | 1.263 | 0.344 | 0.032 | 0.021 | 1.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 6.546 | 5.179 | 1.264 | 0.587 | 0.047 | 0.011 | 1.000 | 0.000 |
| variant-01/08-negative-weights | 6.561 | 5.178 | 1.267 | 0.487 | 0.055 | 0.023 | 1.000 | 0.000 |
| variant-04/02-precedence | 9.120 | 6.483 | 1.407 | 1.297 | 0.233 | 0.048 | 2.000 | 0.000 |
| variant-02/03-before-forces-detour | 7.608 | 5.210 | 1.460 | 0.469 | 0.046 | 0.011 | 1.000 | 0.000 |
| equality-generalized-tsp/02-larger | 7.719 | 5.206 | 1.483 | 1.102 | 0.163 | 0.130 | 1.000 | 0.000 |
| variant-02/06-unreachable-edge | 7.726 | 5.206 | 1.484 | 0.882 | 0.089 | 0.065 | 1.000 | 0.000 |
| variant-02/04-equal-cost-split | 7.727 | 5.194 | 1.488 | 0.574 | 0.067 | 0.046 | 1.000 | 0.000 |
| variant-02/02-single-salesman | 7.773 | 5.204 | 1.494 | 0.486 | 0.057 | 0.041 | 1.000 | 0.000 |
| variant-02/01-basic | 7.725 | 5.169 | 1.494 | 0.647 | 0.070 | 0.019 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 7.766 | 5.193 | 1.495 | 0.718 | 0.080 | 0.015 | 1.000 | 0.000 |
| variant-04/02-start-equals-end | 7.721 | 5.159 | 1.497 | 0.420 | 0.033 | 0.022 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 7.769 | 5.185 | 1.498 | 0.462 | 0.054 | 0.038 | 1.000 | 0.000 |
| variant-01/05-multi-path | 7.751 | 5.173 | 1.498 | 0.584 | 0.074 | 0.020 | 1.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 7.772 | 5.187 | 1.498 | 0.744 | 0.051 | 0.012 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 7.765 | 5.179 | 1.499 | 0.590 | 0.052 | 0.013 | 1.000 | 0.000 |
| variant-02/03-cost-dominates | 7.791 | 5.193 | 1.500 | 0.712 | 0.094 | 0.028 | 1.000 | 0.000 |
| variant-04/09-negative-weights | 7.796 | 5.196 | 1.501 | 0.657 | 0.068 | 0.027 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 7.780 | 5.184 | 1.501 | 0.638 | 0.067 | 0.018 | 1.000 | 0.000 |
| variant-03/01-basic | 7.779 | 5.180 | 1.502 | 0.642 | 0.056 | 0.029 | 1.000 | 0.000 |
| variant-02/09-negative-weights | 7.775 | 5.177 | 1.502 | 0.524 | 0.058 | 0.025 | 1.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 7.824 | 5.206 | 1.503 | 0.583 | 0.070 | 0.017 | 1.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 7.814 | 5.199 | 1.503 | 0.733 | 0.047 | 0.012 | 1.000 | 0.000 |
| variant-02/01-basic | 7.782 | 5.171 | 1.505 | 0.686 | 0.073 | 0.052 | 1.000 | 0.000 |
| variant-03/05-larger-mix | 7.765 | 5.157 | 1.506 | 0.504 | 0.136 | 0.036 | 1.000 | 0.000 |
| variant-01/01-basic | 7.933 | 5.263 | 1.507 | 0.507 | 0.068 | 0.051 | 1.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 7.862 | 5.215 | 1.508 | 0.606 | 0.055 | 0.012 | 1.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 7.826 | 5.191 | 1.508 | 0.608 | 0.048 | 0.012 | 1.000 | 0.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 7.801 | 5.171 | 1.509 | 0.884 | 0.098 | 0.025 | 1.000 | 0.000 |
| variant-05/02-tight-bound-exact | 7.793 | 5.165 | 1.509 | 0.738 | 0.051 | 0.029 | 1.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 7.821 | 5.183 | 1.509 | 0.738 | 0.052 | 0.030 | 1.000 | 0.000 |
| variant-03/06-unreachable-edge | 7.812 | 5.176 | 1.509 | 0.959 | 0.076 | 0.039 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 7.836 | 5.190 | 1.510 | 0.917 | 0.062 | 0.034 | 1.000 | 0.000 |
| variant-01/07-cycles | 7.808 | 5.171 | 1.510 | 0.756 | 0.119 | 0.079 | 1.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 7.860 | 5.194 | 1.513 | 0.580 | 0.035 | 0.010 | 1.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 7.842 | 5.181 | 1.513 | 0.487 | 0.057 | 0.016 | 1.000 | 0.000 |
| variant-01/03-asymmetric | 7.848 | 5.185 | 1.514 | 0.872 | 0.410 | 0.167 | 1.000 | 0.000 |
| equality-generalized-tsp/01-basic | 7.867 | 5.196 | 1.514 | 0.693 | 0.084 | 0.079 | 1.000 | 0.000 |
| variant-02/04-after-forces-extension | 7.873 | 5.198 | 1.515 | 0.462 | 0.043 | 0.012 | 1.000 | 0.000 |
| variant-01/02-multiple-tours | 7.855 | 5.185 | 1.515 | 0.718 | 0.242 | 0.153 | 1.000 | 0.000 |
| variant-03/04-equal-cost-split | 7.888 | 5.205 | 1.515 | 0.777 | 0.090 | 0.067 | 1.000 | 0.000 |
| variant-03/09-negative-weights | 7.851 | 5.178 | 1.516 | 0.612 | 0.062 | 0.026 | 1.000 | 0.000 |
| variant-02/05-larger-asymmetric | 7.873 | 5.188 | 1.518 | 0.852 | 0.082 | 0.059 | 1.000 | 0.000 |
| n-queens/variant-04 | 10.319 | 6.455 | 1.599 | 1.877 | 2.347 | 0.313 | 1.000 | 1.000 |
| variant-02/06-layered-dag-before | 10.289 | 6.434 | 1.599 | 1.822 | 1.038 | 0.088 | 2.000 | 0.000 |
| n-queens/variant-05 | 10.349 | 6.464 | 1.601 | 1.604 | 2.756 | 0.457 | 1.000 | 1.000 |
| variant-02/07-layered-dag-before-after | 10.300 | 6.428 | 1.602 | 1.823 | 0.820 | 0.081 | 2.000 | 0.000 |
| n-queens/variant-06 | 10.373 | 6.440 | 1.611 | 1.627 | 3.047 | 0.511 | 1.000 | 1.000 |
| variant-03/04-incompatible-group-unsat | 6.510 | 3.927 | 1.658 | 0.245 | 0.028 | 0.007 | 0.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 6.501 | 3.918 | 1.659 | 0.346 | 0.052 | 0.015 | 1.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 6.511 | 3.924 | 1.659 | 0.242 | 0.024 | 0.006 | 0.000 | 0.000 |
| variant-01/01-basic | 6.540 | 3.927 | 1.665 | 0.271 | 0.039 | 0.012 | 0.000 | 0.000 |
| variant-01/02-agent-reuse | 6.514 | 3.903 | 1.669 | 0.330 | 0.049 | 0.018 | 0.000 | 0.000 |
| variant-01/03-selective-compatibility | 6.539 | 3.912 | 1.672 | 0.311 | 0.044 | 0.013 | 0.000 | 0.000 |
| variant-04/01-basic | 8.956 | 5.196 | 1.724 | 1.228 | 0.080 | 0.043 | 1.000 | 0.000 |
| variant-01/06-layered-dag | 9.003 | 5.183 | 1.737 | 1.697 | 1.013 | 0.069 | 2.000 | 0.000 |
| variant-03/05-larger-three-depots | 9.017 | 5.190 | 1.737 | 1.497 | 0.099 | 0.052 | 1.000 | 0.000 |
| variant-01/05-ring | 9.048 | 5.189 | 1.744 | 1.058 | 1.314 | 0.226 | 1.000 | 0.000 |
| variant-05/01-basic | 9.103 | 5.167 | 1.762 | 1.204 | 0.080 | 0.042 | 1.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 11.486 | 6.436 | 1.785 | 2.871 | 1.226 | 0.129 | 3.000 | 0.000 |
| variant-04/07-layered-dag-combined | 11.545 | 6.436 | 1.794 | 2.880 | 1.451 | 0.139 | 3.000 | 0.000 |
| variant-03/06-layered-dag-cap | 11.582 | 6.433 | 1.800 | 2.635 | 2.163 | 0.110 | 3.000 | 0.000 |
| variant-01/05-larger-mix | 7.686 | 3.977 | 1.933 | 0.463 | 0.374 | 0.041 | 1.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 7.766 | 3.964 | 1.959 | 0.581 | 0.035 | 0.010 | 1.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 10.271 | 5.180 | 1.983 | 3.044 | 0.162 | 0.087 | 1.000 | 0.000 |
| variant-02/05-larger-mix | 10.307 | 5.191 | 1.985 | 1.509 | 2.459 | 0.077 | 1.000 | 0.000 |
| variant-05/05-three-depots-mixed | 10.307 | 5.181 | 1.989 | 3.030 | 0.158 | 0.087 | 1.000 | 0.000 |
| variant-04/06-unreachable-edge | 10.304 | 5.165 | 1.995 | 2.327 | 0.137 | 0.074 | 1.000 | 0.000 |
| variant-05/06-unreachable-edge | 10.380 | 5.172 | 2.007 | 2.316 | 0.135 | 0.075 | 1.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 12.916 | 6.420 | 2.012 | 2.642 | 2.832 | 0.110 | 3.000 | 0.000 |
| n-queens/variant-03 | 15.520 | 6.496 | 2.389 | 6.044 | 2.511 | 0.435 | 1.000 | 1.000 |
| n-queens/variant-01 | 16.585 | 6.504 | 2.550 | 8.189 | 2.316 | 0.404 | 1.000 | 1.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 22.6 | 21.4 | n/a |
| variant-04/03-agent-serialization | 15.6 | 8.0 | n/a |
| n-queens/variant-02 | 21.7 | 8.2 | n/a |
| variant-04/01-basic | 16.0 | 9.1 | n/a |
| send-money/send-money | 19.0 | 8.2 | n/a |
| variant-02/03-too-many-salesmen-unsat | 14.5 | 5.4 | n/a |
| variant-01/01-basic | 14.4 | 5.4 | n/a |
| variant-04/01-basic | 14.8 | 5.5 | n/a |
| variant-01/03-zero-cost-detour | 14.4 | 5.3 | n/a |
| variant-03/02-start-equals-end | 14.7 | 5.3 | n/a |
| variant-02/05-ordering-unsat | 14.5 | 5.3 | n/a |
| variant-02/02-makespan-tiebreak | 14.6 | 5.4 | n/a |
| variant-03/04-cost-at-cap-allowed | 14.6 | 5.3 | n/a |
| variant-02/01-basic | 14.6 | 5.4 | n/a |
| variant-01/04-subtour-unsat | 14.1 | 5.2 | n/a |
| variant-03/03-budget-forces-detour | 14.7 | 5.4 | n/a |
| variant-03/02-multiple-groups | 14.5 | 5.3 | n/a |
| variant-03/01-basic | 14.7 | 5.4 | n/a |
| variant-01/02-start-equals-end | 14.3 | 5.3 | n/a |
| variant-01/04-no-path | 14.3 | 5.2 | n/a |
| variant-03/01-basic | 14.2 | 5.3 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 14.3 | 5.2 | n/a |
| variant-02/02-start-equals-end | 14.5 | 5.4 | n/a |
| variant-04/04-ordering-violates-budget | 14.8 | 5.4 | n/a |
| variant-01/08-negative-weights | 14.7 | 5.4 | n/a |
| variant-04/02-precedence | 15.6 | 5.8 | n/a |
| variant-02/03-before-forces-detour | 14.7 | 5.4 | n/a |
| equality-generalized-tsp/02-larger | 14.8 | 5.4 | n/a |
| variant-02/06-unreachable-edge | 14.9 | 5.4 | n/a |
| variant-02/04-equal-cost-split | 14.5 | 5.5 | n/a |
| variant-02/02-single-salesman | 14.6 | 5.4 | n/a |
| variant-02/01-basic | 14.4 | 5.4 | n/a |
| variant-03/05-budget-unsat | 14.9 | 5.5 | n/a |
| variant-04/02-start-equals-end | 14.7 | 5.4 | n/a |
| variant-03/02-single-salesman | 14.6 | 5.4 | n/a |
| variant-01/05-multi-path | 14.6 | 5.4 | n/a |
| variant-05/03-revisit-too-tight-unsat | 15.0 | 5.5 | n/a |
| variant-04/03-before-forces-detour-within-budget | 14.8 | 5.5 | n/a |
| variant-02/03-cost-dominates | 14.7 | 5.4 | n/a |
| variant-04/09-negative-weights | 15.1 | 5.4 | n/a |
| variant-04/05-after-and-budget-interact | 15.0 | 5.4 | n/a |
| variant-03/01-basic | 14.6 | 5.4 | n/a |
| variant-02/09-negative-weights | 14.9 | 5.4 | n/a |
| variant-03/08-tie-break-under-cap | 14.8 | 5.4 | n/a |
| variant-04/03-window-too-tight-unsat | 15.0 | 5.5 | n/a |
| variant-02/01-basic | 14.7 | 5.5 | n/a |
| variant-03/05-larger-mix | 14.7 | 5.3 | n/a |
| variant-01/01-basic | 14.4 | 5.4 | n/a |
| variant-04/04-window-too-tight-unsat | 15.2 | 5.5 | n/a |
| variant-03/03-depot-crossing-unsat | 14.5 | 5.4 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 15.2 | 5.5 | n/a |
| variant-05/02-tight-bound-exact | 15.0 | 5.5 | n/a |
| variant-05/04-depot-and-vertex-revisits | 15.2 | 5.5 | n/a |
| variant-03/06-unreachable-edge | 14.8 | 5.4 | n/a |
| variant-04/02-single-salesman | 15.1 | 5.5 | n/a |
| variant-01/07-cycles | 14.9 | 5.5 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 14.8 | 5.5 | n/a |
| variant-02/08-tie-break-under-ordering | 14.8 | 5.4 | n/a |
| variant-01/03-asymmetric | 15.2 | 5.6 | n/a |
| equality-generalized-tsp/01-basic | 14.7 | 5.4 | n/a |
| variant-02/04-after-forces-extension | 14.6 | 5.4 | n/a |
| variant-01/02-multiple-tours | 15.1 | 5.5 | n/a |
| variant-03/04-equal-cost-split | 14.8 | 5.5 | n/a |
| variant-03/09-negative-weights | 14.9 | 5.4 | n/a |
| variant-02/05-larger-asymmetric | 14.8 | 5.4 | n/a |
| n-queens/variant-04 | 14.9 | 5.4 | n/a |
| variant-02/06-layered-dag-before | 17.5 | 5.6 | n/a |
| n-queens/variant-05 | 15.6 | 5.5 | n/a |
| variant-02/07-layered-dag-before-after | 17.5 | 5.6 | n/a |
| n-queens/variant-06 | 15.8 | 5.5 | n/a |
| variant-03/04-incompatible-group-unsat | 14.0 | 5.2 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 14.3 | 5.3 | n/a |
| variant-01/04-no-compatible-agent-unsat | 13.9 | 5.1 | n/a |
| variant-01/01-basic | 14.0 | 5.2 | n/a |
| variant-01/02-agent-reuse | 14.2 | 5.3 | n/a |
| variant-01/03-selective-compatibility | 14.1 | 5.3 | n/a |
| variant-04/01-basic | 15.2 | 5.5 | n/a |
| variant-01/06-layered-dag | 15.9 | 5.6 | n/a |
| variant-03/05-larger-three-depots | 15.0 | 5.4 | n/a |
| variant-01/05-ring | 16.6 | 5.6 | n/a |
| variant-05/01-basic | 15.3 | 5.5 | n/a |
| variant-04/06-layered-dag-ordering-cap | 19.5 | 6.0 | n/a |
| variant-04/07-layered-dag-combined | 19.3 | 6.0 | n/a |
| variant-03/06-layered-dag-cap | 19.7 | 5.9 | n/a |
| variant-01/05-larger-mix | 14.8 | 5.3 | n/a |
| variant-02/04-no-compatible-agent-unsat | 14.4 | 5.2 | n/a |
| variant-04/05-three-depots-asymmetric-times | 15.5 | 5.5 | n/a |
| variant-02/05-larger-mix | 16.1 | 5.5 | n/a |
| variant-05/05-three-depots-mixed | 15.6 | 5.5 | n/a |
| variant-04/06-unreachable-edge | 15.6 | 5.5 | n/a |
| variant-05/06-unreachable-edge | 15.6 | 5.5 | n/a |
| variant-03/07-layered-dag-tight-cap | 25.1 | 5.9 | n/a |
| n-queens/variant-03 | 15.5 | 5.4 | n/a |
| n-queens/variant-01 | 15.1 | 5.4 | n/a |

Against the reference: report candidate, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=1, batch=64; search default): faster on 4 of 94 cells where both passed (4.2%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership, each summed over its intervals, which can overlap within or across threads, so the parts need not add up to the native median; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 36.034 | 197.394 | 0.183 | 7.525 | 27.216 | 8.195 | 76.000 | 119.000 |
| variant-04/03-agent-serialization | 10.327 | 26.344 | 0.392 | 2.272 | 0.510 | 0.103 | 22.000 | 0.000 |
| n-queens/variant-02 | 57.418 | 123.173 | 0.466 | 7.019 | 130.328 | 0.750 | 1.000 | 118.000 |
| variant-04/01-basic | 11.550 | 24.037 | 0.480 | 2.351 | 0.996 | 0.260 | 15.000 | 5.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership, each summed over its intervals, which can overlap within or across threads, so the parts need not add up to the native median; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| send-money/send-money | 15.472 | 12.732 | 1.215 | 5.304 | 3.518 | 0.153 | 7.000 | 1.000 |
| variant-03/02-multiple-groups | 6.502 | 5.203 | 1.250 | 0.413 | 0.065 | 0.019 | 1.000 | 0.000 |
| variant-01/03-zero-cost-detour | 6.517 | 5.198 | 1.254 | 0.331 | 0.037 | 0.011 | 1.000 | 0.000 |
| variant-01/04-subtour-unsat | 6.508 | 5.185 | 1.255 | 0.396 | 0.029 | 0.011 | 1.000 | 0.000 |
| variant-02/02-single-salesman | 6.544 | 5.205 | 1.257 | 0.488 | 0.053 | 0.041 | 1.000 | 0.000 |
| variant-02/05-ordering-unsat | 6.541 | 5.197 | 1.259 | 0.370 | 0.030 | 0.007 | 1.000 | 0.000 |
| variant-02/01-basic | 6.538 | 5.193 | 1.259 | 0.398 | 0.046 | 0.013 | 1.000 | 0.000 |
| variant-03/01-basic | 6.522 | 5.175 | 1.260 | 0.301 | 0.044 | 0.013 | 0.000 | 0.000 |
| variant-03/01-basic | 6.552 | 5.195 | 1.261 | 0.604 | 0.051 | 0.028 | 1.000 | 0.000 |
| variant-03/03-budget-forces-detour | 6.548 | 5.191 | 1.261 | 0.449 | 0.045 | 0.012 | 1.000 | 0.000 |
| variant-01/01-basic | 6.543 | 5.185 | 1.262 | 0.361 | 0.046 | 0.013 | 1.000 | 0.000 |
| variant-01/02-start-equals-end | 6.602 | 5.210 | 1.267 | 0.332 | 0.028 | 0.017 | 1.000 | 0.000 |
| equality-generalized-tsp/03-unreachable-subset-unsat | 6.574 | 5.182 | 1.268 | 0.374 | 0.034 | 0.011 | 1.000 | 0.000 |
| variant-03/02-single-salesman | 6.596 | 5.194 | 1.270 | 0.466 | 0.053 | 0.038 | 1.000 | 0.000 |
| variant-01/04-no-path | 6.618 | 5.198 | 1.273 | 0.349 | 0.026 | 0.007 | 1.000 | 0.000 |
| variant-03/02-start-equals-end | 6.618 | 5.185 | 1.276 | 0.386 | 0.030 | 0.020 | 1.000 | 0.000 |
| variant-04/02-precedence | 9.116 | 6.463 | 1.410 | 1.318 | 0.236 | 0.046 | 2.000 | 0.000 |
| variant-03/04-cost-at-cap-allowed | 7.546 | 5.189 | 1.454 | 0.441 | 0.032 | 0.009 | 1.000 | 0.000 |
| variant-02/03-too-many-salesmen-unsat | 7.742 | 5.213 | 1.485 | 0.591 | 0.048 | 0.012 | 1.000 | 0.000 |
| variant-02/05-larger-asymmetric | 7.734 | 5.192 | 1.490 | 0.851 | 0.081 | 0.061 | 1.000 | 0.000 |
| variant-05/03-revisit-too-tight-unsat | 7.748 | 5.193 | 1.492 | 0.756 | 0.049 | 0.013 | 1.000 | 0.000 |
| variant-04/03-before-forces-detour-within-budget | 7.760 | 5.198 | 1.493 | 0.584 | 0.052 | 0.013 | 1.000 | 0.000 |
| variant-03/01-basic | 7.752 | 5.186 | 1.495 | 0.429 | 0.046 | 0.012 | 1.000 | 0.000 |
| variant-02/06-unreachable-edge | 7.779 | 5.201 | 1.496 | 0.902 | 0.089 | 0.066 | 1.000 | 0.000 |
| variant-02/03-cost-dominates | 7.775 | 5.196 | 1.496 | 0.742 | 0.090 | 0.029 | 1.000 | 0.000 |
| variant-03/03-depot-crossing-unsat | 7.776 | 5.191 | 1.498 | 0.603 | 0.047 | 0.012 | 1.000 | 0.000 |
| variant-01/05-multi-path | 7.772 | 5.183 | 1.500 | 0.577 | 0.079 | 0.020 | 1.000 | 0.000 |
| variant-04/05-after-and-budget-interact | 7.788 | 5.191 | 1.500 | 0.642 | 0.066 | 0.018 | 1.000 | 0.000 |
| variant-01/08-negative-weights | 7.793 | 5.194 | 1.500 | 0.484 | 0.055 | 0.024 | 1.000 | 0.000 |
| variant-02/02-start-equals-end | 7.771 | 5.179 | 1.500 | 0.345 | 0.031 | 0.020 | 1.000 | 0.000 |
| equality-generalized-tsp/02-larger | 7.831 | 5.218 | 1.501 | 1.120 | 0.162 | 0.135 | 1.000 | 0.000 |
| variant-03/08-tie-break-under-cap | 7.821 | 5.204 | 1.503 | 0.589 | 0.068 | 0.018 | 1.000 | 0.000 |
| variant-04/08-tie-break-under-ordering-and-cap | 7.802 | 5.190 | 1.503 | 0.896 | 0.104 | 0.029 | 1.000 | 0.000 |
| variant-04/09-negative-weights | 7.783 | 5.178 | 1.503 | 0.669 | 0.066 | 0.028 | 1.000 | 0.000 |
| variant-04/01-basic | 7.799 | 5.186 | 1.504 | 0.451 | 0.046 | 0.012 | 1.000 | 0.000 |
| variant-03/06-unreachable-edge | 7.828 | 5.203 | 1.505 | 0.957 | 0.073 | 0.040 | 1.000 | 0.000 |
| variant-02/01-basic | 7.781 | 5.172 | 1.505 | 0.635 | 0.069 | 0.020 | 1.000 | 0.000 |
| variant-03/05-larger-three-depots | 7.797 | 5.181 | 1.505 | 1.510 | 0.098 | 0.051 | 1.000 | 0.000 |
| variant-04/03-window-too-tight-unsat | 7.830 | 5.202 | 1.505 | 0.745 | 0.044 | 0.012 | 1.000 | 0.000 |
| variant-02/09-negative-weights | 7.800 | 5.180 | 1.506 | 0.545 | 0.058 | 0.025 | 1.000 | 0.000 |
| variant-04/02-single-salesman | 7.781 | 5.166 | 1.506 | 0.913 | 0.060 | 0.034 | 1.000 | 0.000 |
| variant-02/04-equal-cost-split | 7.828 | 5.195 | 1.507 | 0.587 | 0.067 | 0.047 | 1.000 | 0.000 |
| variant-01/07-cycles | 7.809 | 5.182 | 1.507 | 0.785 | 0.127 | 0.079 | 1.000 | 0.000 |
| variant-01/02-multiple-tours | 7.826 | 5.190 | 1.508 | 0.731 | 0.249 | 0.153 | 1.000 | 0.000 |
| variant-01/03-asymmetric | 7.817 | 5.184 | 1.508 | 0.884 | 0.404 | 0.171 | 1.000 | 0.000 |
| variant-05/04-depot-and-vertex-revisits | 7.811 | 5.178 | 1.508 | 0.772 | 0.050 | 0.031 | 1.000 | 0.000 |
| variant-04/02-start-equals-end | 7.833 | 5.192 | 1.509 | 0.437 | 0.037 | 0.023 | 1.000 | 0.000 |
| variant-03/05-larger-mix | 7.777 | 5.152 | 1.509 | 0.507 | 0.143 | 0.038 | 1.000 | 0.000 |
| variant-02/02-makespan-tiebreak | 7.802 | 5.169 | 1.509 | 0.733 | 0.098 | 0.027 | 1.000 | 0.000 |
| variant-03/04-equal-cost-split | 7.830 | 5.182 | 1.511 | 0.781 | 0.093 | 0.065 | 1.000 | 0.000 |
| variant-02/03-before-forces-detour | 7.857 | 5.200 | 1.511 | 0.482 | 0.045 | 0.013 | 1.000 | 0.000 |
| variant-04/04-ordering-violates-budget | 7.839 | 5.187 | 1.511 | 0.582 | 0.050 | 0.011 | 1.000 | 0.000 |
| variant-04/04-depot-window-too-tight-unsat | 7.845 | 5.190 | 1.512 | 0.610 | 0.038 | 0.011 | 1.000 | 0.000 |
| variant-03/09-negative-weights | 7.837 | 5.182 | 1.512 | 0.614 | 0.061 | 0.027 | 1.000 | 0.000 |
| variant-02/08-tie-break-under-ordering | 7.853 | 5.189 | 1.513 | 0.518 | 0.061 | 0.017 | 1.000 | 0.000 |
| variant-05/02-tight-bound-exact | 7.871 | 5.197 | 1.515 | 0.769 | 0.050 | 0.028 | 1.000 | 0.000 |
| variant-04/04-window-too-tight-unsat | 7.875 | 5.199 | 1.515 | 0.608 | 0.059 | 0.012 | 1.000 | 0.000 |
| variant-02/04-after-forces-extension | 7.875 | 5.197 | 1.515 | 0.456 | 0.042 | 0.013 | 1.000 | 0.000 |
| variant-02/01-basic | 7.869 | 5.188 | 1.517 | 0.698 | 0.075 | 0.051 | 1.000 | 0.000 |
| equality-generalized-tsp/01-basic | 7.900 | 5.205 | 1.518 | 0.692 | 0.089 | 0.078 | 1.000 | 0.000 |
| variant-01/01-basic | 7.992 | 5.266 | 1.518 | 0.531 | 0.073 | 0.050 | 1.000 | 0.000 |
| variant-03/05-budget-unsat | 7.877 | 5.182 | 1.520 | 0.733 | 0.079 | 0.019 | 1.000 | 0.000 |
| variant-01/05-larger-mix | 7.785 | 4.968 | 1.567 | 0.478 | 0.376 | 0.043 | 1.000 | 0.000 |
| variant-02/06-layered-dag-before | 10.269 | 6.464 | 1.589 | 1.856 | 1.040 | 0.089 | 2.000 | 0.000 |
| n-queens/variant-06 | 10.419 | 6.517 | 1.599 | 1.688 | 3.111 | 0.512 | 1.000 | 1.000 |
| n-queens/variant-05 | 10.363 | 6.476 | 1.600 | 1.619 | 2.708 | 0.469 | 1.000 | 1.000 |
| n-queens/variant-04 | 10.395 | 6.464 | 1.608 | 1.869 | 2.357 | 0.311 | 1.000 | 1.000 |
| variant-02/07-layered-dag-before-after | 10.347 | 6.411 | 1.614 | 1.866 | 0.824 | 0.081 | 2.000 | 0.000 |
| variant-01/02-agent-reuse | 6.507 | 3.965 | 1.641 | 0.323 | 0.050 | 0.019 | 0.000 | 0.000 |
| variant-02/04-no-compatible-agent-unsat | 6.497 | 3.930 | 1.653 | 0.568 | 0.034 | 0.010 | 1.000 | 0.000 |
| variant-01/01-basic | 6.502 | 3.929 | 1.655 | 0.268 | 0.041 | 0.013 | 0.000 | 0.000 |
| variant-03/03-mixed-grouped-ungrouped | 6.502 | 3.917 | 1.660 | 0.355 | 0.052 | 0.015 | 0.000 | 0.000 |
| variant-01/04-no-compatible-agent-unsat | 6.500 | 3.907 | 1.664 | 0.235 | 0.024 | 0.007 | 0.000 | 0.000 |
| variant-03/04-incompatible-group-unsat | 6.516 | 3.912 | 1.666 | 0.261 | 0.028 | 0.007 | 0.000 | 0.000 |
| variant-01/03-selective-compatibility | 6.507 | 3.903 | 1.667 | 0.316 | 0.046 | 0.014 | 0.000 | 0.000 |
| variant-04/01-basic | 8.855 | 5.197 | 1.704 | 1.223 | 0.078 | 0.046 | 1.000 | 0.000 |
| variant-01/05-ring | 8.963 | 5.201 | 1.723 | 1.056 | 1.259 | 0.226 | 1.000 | 0.000 |
| variant-05/01-basic | 9.074 | 5.178 | 1.752 | 1.229 | 0.079 | 0.045 | 1.000 | 0.000 |
| variant-04/06-layered-dag-ordering-cap | 11.569 | 6.438 | 1.797 | 2.929 | 1.270 | 0.133 | 3.000 | 0.000 |
| variant-03/06-layered-dag-cap | 11.621 | 6.462 | 1.798 | 2.660 | 2.111 | 0.112 | 3.000 | 0.000 |
| variant-04/07-layered-dag-combined | 11.666 | 6.435 | 1.813 | 2.911 | 1.346 | 0.139 | 3.000 | 0.000 |
| variant-03/07-layered-dag-tight-cap | 12.793 | 6.438 | 1.987 | 2.655 | 2.923 | 0.112 | 3.000 | 0.000 |
| variant-01/06-layered-dag | 10.319 | 5.182 | 1.991 | 1.705 | 1.007 | 0.069 | 2.000 | 0.000 |
| variant-04/05-three-depots-asymmetric-times | 10.323 | 5.183 | 1.992 | 3.050 | 0.166 | 0.085 | 1.000 | 0.000 |
| variant-02/05-larger-mix | 10.319 | 5.181 | 1.992 | 1.522 | 2.468 | 0.077 | 1.000 | 0.000 |
| variant-05/05-three-depots-mixed | 10.303 | 5.164 | 1.995 | 3.116 | 0.165 | 0.089 | 1.000 | 0.000 |
| variant-05/06-unreachable-edge | 10.378 | 5.177 | 2.005 | 2.380 | 0.136 | 0.073 | 1.000 | 0.000 |
| variant-04/06-unreachable-edge | 10.393 | 5.165 | 2.012 | 2.361 | 0.140 | 0.074 | 1.000 | 0.000 |
| n-queens/variant-03 | 15.500 | 6.518 | 2.378 | 6.127 | 2.529 | 0.437 | 1.000 | 1.000 |
| n-queens/variant-01 | 16.618 | 6.475 | 2.567 | 8.065 | 2.331 | 0.408 | 1.000 | 1.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 22.5 | 19.4 | n/a |
| variant-04/03-agent-serialization | 15.7 | 8.0 | n/a |
| n-queens/variant-02 | 21.3 | 8.2 | n/a |
| variant-04/01-basic | 15.9 | 8.7 | n/a |
| send-money/send-money | 18.5 | 8.2 | n/a |
| variant-03/02-multiple-groups | 14.3 | 5.3 | n/a |
| variant-01/03-zero-cost-detour | 14.4 | 5.3 | n/a |
| variant-01/04-subtour-unsat | 14.1 | 5.2 | n/a |
| variant-02/02-single-salesman | 14.6 | 5.4 | n/a |
| variant-02/05-ordering-unsat | 14.5 | 5.3 | n/a |
| variant-02/01-basic | 14.6 | 5.4 | n/a |
| variant-03/01-basic | 14.1 | 5.3 | n/a |
| variant-03/01-basic | 14.5 | 5.4 | n/a |
| variant-03/03-budget-forces-detour | 14.6 | 5.4 | n/a |
| variant-01/01-basic | 14.5 | 5.4 | n/a |
| variant-01/02-start-equals-end | 14.4 | 5.3 | n/a |
| equality-generalized-tsp/03-unreachable-subset-unsat | 14.2 | 5.2 | n/a |
| variant-03/02-single-salesman | 14.5 | 5.4 | n/a |
| variant-01/04-no-path | 14.3 | 5.2 | n/a |
| variant-03/02-start-equals-end | 14.5 | 5.3 | n/a |
| variant-04/02-precedence | 15.5 | 5.8 | n/a |
| variant-03/04-cost-at-cap-allowed | 14.6 | 5.3 | n/a |
| variant-02/03-too-many-salesmen-unsat | 14.4 | 5.4 | n/a |
| variant-02/05-larger-asymmetric | 14.7 | 5.4 | n/a |
| variant-05/03-revisit-too-tight-unsat | 15.0 | 5.5 | n/a |
| variant-04/03-before-forces-detour-within-budget | 14.8 | 5.5 | n/a |
| variant-03/01-basic | 14.7 | 5.4 | n/a |
| variant-02/06-unreachable-edge | 14.9 | 5.4 | n/a |
| variant-02/03-cost-dominates | 14.6 | 5.4 | n/a |
| variant-03/03-depot-crossing-unsat | 14.4 | 5.4 | n/a |
| variant-01/05-multi-path | 14.5 | 5.4 | n/a |
| variant-04/05-after-and-budget-interact | 14.8 | 5.4 | n/a |
| variant-01/08-negative-weights | 14.7 | 5.4 | n/a |
| variant-02/02-start-equals-end | 14.5 | 5.4 | n/a |
| equality-generalized-tsp/02-larger | 14.9 | 5.4 | n/a |
| variant-03/08-tie-break-under-cap | 14.7 | 5.4 | n/a |
| variant-04/08-tie-break-under-ordering-and-cap | 15.1 | 5.5 | n/a |
| variant-04/09-negative-weights | 15.0 | 5.4 | n/a |
| variant-04/01-basic | 14.8 | 5.5 | n/a |
| variant-03/06-unreachable-edge | 14.7 | 5.4 | n/a |
| variant-02/01-basic | 14.3 | 5.4 | n/a |
| variant-03/05-larger-three-depots | 14.9 | 5.4 | n/a |
| variant-04/03-window-too-tight-unsat | 14.9 | 5.5 | n/a |
| variant-02/09-negative-weights | 14.9 | 5.4 | n/a |
| variant-04/02-single-salesman | 15.0 | 5.5 | n/a |
| variant-02/04-equal-cost-split | 14.5 | 5.5 | n/a |
| variant-01/07-cycles | 14.8 | 5.5 | n/a |
| variant-01/02-multiple-tours | 15.2 | 5.5 | n/a |
| variant-01/03-asymmetric | 15.6 | 5.6 | n/a |
| variant-05/04-depot-and-vertex-revisits | 15.1 | 5.5 | n/a |
| variant-04/02-start-equals-end | 14.7 | 5.4 | n/a |
| variant-03/05-larger-mix | 14.6 | 5.3 | n/a |
| variant-02/02-makespan-tiebreak | 14.5 | 5.4 | n/a |
| variant-03/04-equal-cost-split | 14.7 | 5.5 | n/a |
| variant-02/03-before-forces-detour | 14.6 | 5.4 | n/a |
| variant-04/04-ordering-violates-budget | 14.7 | 5.4 | n/a |
| variant-04/04-depot-window-too-tight-unsat | 15.0 | 5.5 | n/a |
| variant-03/09-negative-weights | 14.9 | 5.4 | n/a |
| variant-02/08-tie-break-under-ordering | 14.7 | 5.4 | n/a |
| variant-05/02-tight-bound-exact | 15.1 | 5.5 | n/a |
| variant-04/04-window-too-tight-unsat | 15.0 | 5.5 | n/a |
| variant-02/04-after-forces-extension | 14.7 | 5.4 | n/a |
| variant-02/01-basic | 14.5 | 5.5 | n/a |
| equality-generalized-tsp/01-basic | 14.6 | 5.4 | n/a |
| variant-01/01-basic | 14.3 | 5.4 | n/a |
| variant-03/05-budget-unsat | 14.8 | 5.5 | n/a |
| variant-01/05-larger-mix | 14.7 | 5.3 | n/a |
| variant-02/06-layered-dag-before | 17.7 | 5.6 | n/a |
| n-queens/variant-06 | 15.8 | 5.5 | n/a |
| n-queens/variant-05 | 15.5 | 5.5 | n/a |
| n-queens/variant-04 | 14.9 | 5.4 | n/a |
| variant-02/07-layered-dag-before-after | 17.5 | 5.6 | n/a |
| variant-01/02-agent-reuse | 14.2 | 5.3 | n/a |
| variant-02/04-no-compatible-agent-unsat | 14.3 | 5.2 | n/a |
| variant-01/01-basic | 13.9 | 5.2 | n/a |
| variant-03/03-mixed-grouped-ungrouped | 14.2 | 5.3 | n/a |
| variant-01/04-no-compatible-agent-unsat | 13.7 | 5.1 | n/a |
| variant-03/04-incompatible-group-unsat | 13.9 | 5.2 | n/a |
| variant-01/03-selective-compatibility | 14.0 | 5.3 | n/a |
| variant-04/01-basic | 15.1 | 5.5 | n/a |
| variant-01/05-ring | 16.6 | 5.6 | n/a |
| variant-05/01-basic | 15.3 | 5.5 | n/a |
| variant-04/06-layered-dag-ordering-cap | 19.4 | 6.0 | n/a |
| variant-03/06-layered-dag-cap | 19.5 | 5.9 | n/a |
| variant-04/07-layered-dag-combined | 19.4 | 6.0 | n/a |
| variant-03/07-layered-dag-tight-cap | 22.9 | 5.9 | n/a |
| variant-01/06-layered-dag | 16.0 | 5.6 | n/a |
| variant-04/05-three-depots-asymmetric-times | 15.5 | 5.5 | n/a |
| variant-02/05-larger-mix | 16.5 | 5.5 | n/a |
| variant-05/05-three-depots-mixed | 15.6 | 5.5 | n/a |
| variant-05/06-unreachable-edge | 15.7 | 5.5 | n/a |
| variant-04/06-unreachable-edge | 15.6 | 5.5 | n/a |
| n-queens/variant-03 | 15.3 | 5.4 | n/a |
| n-queens/variant-01 | 15.0 | 5.4 | n/a |
