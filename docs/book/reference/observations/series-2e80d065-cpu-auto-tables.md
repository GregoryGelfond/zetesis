Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | A1-main | B1-candidate | B2-candidate | A2-main | A2-main/A1-main | A2-main/B2-candidate | B1-candidate/A1-main | B2-candidate/B1-candidate | A1-main/reference | B1-candidate/reference | B2-candidate/reference | A2-main/reference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 8.715 [7.926, 9.504] | 8.719 [7.876, 9.562] | 7.949 [7.833, 8.065] | 7.990 [7.912, 8.067] | 0.917 | 1.005 | 1.000 | 0.912 | 1.102 | 1.101 | 1.005 | 1.009 |
| independent-choice-16 | 22.939 [22.714, 23.164] | 23.209 [23.153, 23.266] | 22.436 [21.592, 23.281] | 22.397 [21.546, 23.248] | 0.976 | 0.998 | 1.012 | 0.967 | 0.636 | 0.649 | 0.630 | 0.622 |
| independent-negation-8 | 9.462 [9.396, 9.528] | 10.014 [9.337, 10.691] | 10.101 [9.158, 11.043] | 9.435 [9.338, 9.533] | 0.997 | 0.934 | 1.058 | 1.009 | 1.995 | 2.087 | 2.093 | 1.980 |
| independent-negation-10 | 18.145 [17.890, 18.400] | 18.380 [18.363, 18.397] | 18.391 [18.365, 18.418] | 19.197 [18.372, 20.022] | 1.058 | 1.044 | 1.013 | 1.001 | 3.823 | 3.881 | 3.886 | 4.145 |
| independent-negation-aggregate-16 | 26.907 [25.914, 27.901] | 26.973 [26.139, 27.807] | 27.385 [27.023, 27.747] | 27.645 [27.321, 27.969] | 1.027 | 1.010 | 1.002 | 1.015 | 0.616 | 0.627 | 0.647 | 0.644 |
| disjunction-12 | 26.382 [26.381, 26.383] | 26.398 [26.358, 26.439] | 26.312 [26.306, 26.318] | 26.523 [26.478, 26.569] | 1.005 | 1.008 | 1.001 | 0.997 | 0.845 | 0.802 | 0.849 | 0.828 |
| ties-50 | 26.432 [25.832, 27.033] | 26.032 [25.585, 26.479] | 26.988 [26.297, 27.678] | 25.785 [25.583, 25.987] | 0.976 | 0.955 | 0.985 | 1.037 | 1.219 | 1.191 | 1.206 | 1.173 |
| transitive-path-100 | 10.465 [9.647, 11.284] | 10.420 [9.562, 11.278] | 9.615 [9.537, 9.693] | 10.448 [9.569, 11.326] | 0.998 | 1.087 | 0.996 | 0.923 | 1.210 | 1.191 | 1.028 | 1.111 |
| transitive-path-200 | 16.871 [16.405, 17.336] | 17.841 [17.778, 17.905] | 17.546 [17.390, 17.701] | 19.374 [17.674, 21.073] | 1.148 | 1.104 | 1.058 | 0.983 | 0.843 | 0.891 | 0.888 | 0.968 |
| transitive-dense-40 | 26.747 [25.951, 27.544] | 26.873 [25.988, 27.759] | 25.917 [25.712, 26.123] | 25.848 [25.509, 26.186] | 0.966 | 0.997 | 1.005 | 0.964 | 4.230 | 4.267 | 4.105 | 4.125 |
| chain-1000 | 15.542 [15.463, 15.621] | 17.079 [16.996, 17.162] | 17.067 [16.983, 17.151] | 16.299 [15.461, 17.137] | 1.049 | 0.955 | 1.099 | 0.999 | 2.004 | 2.442 | 2.433 | 2.100 |
| chain-2000 | 26.075 [26.058, 26.093] | 27.494 [27.442, 27.547] | 26.817 [25.982, 27.652] | 27.648 [27.624, 27.672] | 1.060 | 1.031 | 1.054 | 0.975 | 2.611 | 2.791 | 2.890 | 2.558 |
| chain-arithmetic-1000 | 10.955 [10.868, 11.042] | 10.811 [10.640, 10.982] | 11.003 [10.975, 11.032] | 10.962 [10.882, 11.043] | 1.001 | 0.996 | 0.987 | 1.018 | 2.324 | 2.386 | 2.344 | 2.325 |
| stratified-16 | 6.371 [6.302, 6.440] | 6.309 [6.295, 6.322] | 6.346 [6.304, 6.388] | 6.417 [6.368, 6.465] | 1.007 | 1.011 | 0.990 | 1.006 | 1.362 | 1.342 | 1.417 | 1.364 |
| producer-chain-700 | 15.464 [15.399, 15.530] | 15.309 [15.231, 15.386] | 15.398 [15.385, 15.412] | 15.285 [15.086, 15.485] | 0.988 | 0.993 | 0.990 | 1.006 | 1.586 | 1.537 | 1.541 | 1.417 |
| latin-square-5 | 14.846 [14.028, 15.664] | 14.669 [14.169, 15.168] | 15.619 [15.578, 15.659] | 15.646 [15.608, 15.685] | 1.054 | 1.002 | 0.988 | 1.065 | 0.796 | 0.726 | 0.803 | 0.810 |
| planning-14 | 36.711 [36.547, 36.876] | 37.250 [36.651, 37.849] | 37.344 [36.682, 38.006] | 36.762 [36.563, 36.960] | 1.001 | 0.984 | 1.015 | 1.003 | 1.917 | 2.009 | 2.019 | 1.981 |
| n-queens/variant-01 8→10 | 31.300 [31.018, 31.583] | 32.025 [31.899, 32.152] | 32.722 [31.065, 34.379] | 32.217 [31.413, 33.021] | 1.029 | 0.985 | 1.023 | 1.022 | 1.056 | 1.053 | 1.074 | 1.055 |
| n-queens/variant-01 8→11 | 105.101 [105.035, 105.167] | 105.572 [104.063, 107.080] | 103.948 [102.894, 105.002] | 108.777 [108.092, 109.461] | 1.035 | 1.046 | 1.004 | 0.985 | 0.566 | 0.561 | 0.554 | 0.573 |
| n-queens/variant-04 8→11 | 96.619 [94.914, 98.324] | 98.737 [98.384, 99.089] | 100.408 [100.331, 100.485] | 105.454 [97.751, 113.157] | 1.091 | 1.050 | 1.022 | 1.017 | 0.533 | 0.538 | 0.546 | 0.572 |
| send-money/send-money | 12.496 [12.461, 12.532] | 12.494 [12.461, 12.527] | 13.786 [13.412, 14.160] | 12.410 [12.194, 12.627] | 0.993 | 0.900 | 1.000 | 1.103 | 0.916 | 0.953 | 1.055 | 0.950 |
| variant-04/05-larger-mix | 35.276 [35.216, 35.335] | 36.495 [36.365, 36.626] | 37.457 [36.018, 38.895] | 36.257 [36.069, 36.444] | 1.028 | 0.968 | 1.035 | 1.026 | 0.193 | 0.203 | 0.190 | 0.191 |

Reference wall time, ms, same notation.

| Cell | A1-main | B1-candidate | B2-candidate | A2-main |
|---|---:|---:|---:|---:|
| independent-choice-12 | 7.910 [7.872, 7.948] | 7.917 [7.834, 8.000] | 7.910 [7.900, 7.920] | 7.921 [7.874, 7.969] |
| independent-choice-16 | 36.050 [36.039, 36.060] | 35.735 [35.702, 35.767] | 35.630 [35.421, 35.839] | 36.036 [36.034, 36.038] |
| independent-negation-8 | 4.743 [4.680, 4.806] | 4.798 [4.759, 4.837] | 4.825 [4.802, 4.849] | 4.766 [4.671, 4.862] |
| independent-negation-10 | 4.746 [4.733, 4.759] | 4.735 [4.708, 4.763] | 4.733 [4.708, 4.758] | 4.631 [4.526, 4.736] |
| independent-negation-aggregate-16 | 43.672 [43.551, 43.794] | 43.006 [42.545, 43.468] | 42.337 [41.854, 42.819] | 42.910 [42.095, 43.726] |
| disjunction-12 | 31.219 [31.216, 31.223] | 32.921 [32.915, 32.926] | 30.978 [30.640, 31.315] | 32.047 [31.286, 32.809] |
| ties-50 | 21.691 [21.455, 21.926] | 21.857 [21.854, 21.860] | 22.386 [21.940, 22.832] | 21.987 [21.953, 22.021] |
| transitive-path-100 | 8.649 [7.864, 9.433] | 8.749 [7.876, 9.623] | 9.355 [9.264, 9.445] | 9.400 [9.392, 9.408] |
| transitive-path-200 | 20.012 [19.902, 20.123] | 20.016 [19.970, 20.062] | 19.753 [19.649, 19.856] | 20.020 [19.981, 20.059] |
| transitive-dense-40 | 6.323 [6.287, 6.359] | 6.299 [6.235, 6.362] | 6.313 [6.286, 6.341] | 6.266 [6.239, 6.293] |
| chain-1000 | 7.755 [7.705, 7.805] | 6.995 [6.170, 7.819] | 7.014 [6.193, 7.835] | 7.762 [7.709, 7.815] |
| chain-2000 | 9.987 [9.364, 10.610] | 9.850 [9.203, 10.497] | 9.281 [9.234, 9.328] | 10.807 [10.763, 10.852] |
| chain-arithmetic-1000 | 4.714 [4.675, 4.753] | 4.531 [4.331, 4.732] | 4.694 [4.663, 4.724] | 4.716 [4.690, 4.741] |
| stratified-16 | 4.677 [4.660, 4.695] | 4.701 [4.689, 4.713] | 4.478 [4.281, 4.675] | 4.703 [4.696, 4.709] |
| producer-chain-700 | 9.751 [9.188, 10.315] | 9.963 [9.115, 10.811] | 9.994 [9.160, 10.829] | 10.784 [10.717, 10.851] |
| latin-square-5 | 18.653 [18.571, 18.735] | 20.217 [20.091, 20.342] | 19.442 [18.666, 20.219] | 19.315 [18.225, 20.405] |
| planning-14 | 19.154 [18.572, 19.735] | 18.539 [18.512, 18.566] | 18.501 [18.455, 18.547] | 18.561 [18.533, 18.589] |
| n-queens/variant-01 8→10 | 29.647 [29.040, 30.253] | 30.417 [30.390, 30.445] | 30.473 [30.428, 30.518] | 30.526 [30.491, 30.561] |
| n-queens/variant-01 8→11 | 185.727 [184.747, 186.708] | 188.028 [187.764, 188.293] | 187.771 [187.602, 187.940] | 189.985 [189.733, 190.236] |
| n-queens/variant-04 8→11 | 181.162 [180.670, 181.653] | 183.512 [183.262, 183.763] | 183.847 [183.389, 184.304] | 184.339 [183.690, 184.988] |
| send-money/send-money | 13.642 [13.418, 13.867] | 13.115 [12.343, 13.886] | 13.065 [12.326, 13.803] | 13.067 [12.343, 13.791] |
| variant-04/05-larger-mix | 182.864 [182.262, 183.466] | 179.915 [170.310, 189.519] | 196.921 [191.648, 202.194] | 190.206 [187.934, 192.478] |

Counters of report A2-main: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 227172 | 2.312 |
| independent-choice-16 | 0 | 2584 | 2584 | 2117111 | 16.665 |
| independent-negation-8 | 0 | 55 | 55 | 28784 | 4.206 |
| independent-negation-10 | 0 | 144 | 144 | 91687 | 13.210 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 1972826 | 20.940 |
| disjunction-12 | 0 | 4096 | 4096 | 2029958 | 20.246 |
| ties-50 | 0 | 1225 | 1225 | 2453665 | 19.383 |
| transitive-path-100 | 0 | 1 | 1 | 111564 | 4.117 |
| transitive-path-200 | 0 | 1 | 1 | 664776 | 13.006 |
| transitive-dense-40 | 0 | 1 | 1 | 939917 | 18.811 |
| chain-1000 | 0 | 1 | 1 | 211573 | 9.826 |
| chain-2000 | 0 | 1 | 1 | 736073 | 20.708 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 105151 | 4.882 |
| stratified-16 | 0 | 1 | 1 | 1669 | 0.719 |
| producer-chain-700 | 0 | 1 | 1 | 48256 | 9.355 |
| latin-square-5 | 0 | 1344 | 1344 | 5659430 | 9.590 |
| planning-14 | 0 | 3432 | 3432 | 11390399 | 29.828 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 6686540 | 25.641 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 31378525 | 101.901 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 23637618 | 98.806 |
| send-money/send-money | 0 | 1 | 1 | 472288 | 6.364 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 8375207 | 30.344 |

Against the reference: report A1-main, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 9 of 22 cells where both passed (40.9%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 35.276 | 182.864 | 0.193 | 2.620 | 36.756 | 8.403 | 71.000 | 106.000 |
| n-queens/variant-04 8→11 | 96.619 | 181.162 | 0.533 | 1.092 | 259.107 | 12.683 | 1.500 | 174.500 |
| n-queens/variant-01 8→11 | 105.101 | 185.727 | 0.566 | 4.695 | 261.591 | 18.338 | 2.000 | 179.000 |
| independent-negation-aggregate-16 | 26.907 | 43.672 | 0.616 | 0.208 | 3.150 | 3.276 | 0.500 | 38.500 |
| independent-choice-16 | 22.939 | 36.050 | 0.636 | n/a | 1.936 | 7.612 | 0.500 | 30.500 |
| latin-square-5 | 14.846 | 18.653 | 0.796 | 0.852 | 12.022 | 6.354 | 1.000 | 13.000 |
| transitive-path-200 | 16.871 | 20.012 | 0.843 | n/a | 0.006 | 3.205 | 9.000 | 6.000 |
| disjunction-12 | 26.382 | 31.219 | 0.845 | 0.129 | 4.014 | 39.063 | 0.000 | 26.000 |
| send-money/send-money | 12.496 | 13.642 | 0.916 | 2.387 | 6.871 | 0.151 | 7.000 | 1.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-01 8→10 | 31.300 | 29.647 | 1.056 | 3.186 | 57.339 | 4.241 | 1.500 | 23.500 |
| independent-choice-12 | 8.715 | 7.910 | 1.102 | n/a | 0.356 | 1.041 | 1.000 | 2.000 |
| transitive-path-100 | 10.465 | 8.649 | 1.210 | n/a | 0.006 | 0.738 | 3.000 | 0.500 |
| ties-50 | 26.432 | 21.691 | 1.219 | 0.305 | 16.062 | 2.239 | 0.000 | 17.000 |
| stratified-16 | 6.371 | 4.677 | 1.362 | n/a | 0.191 | 0.071 | 0.000 | 0.000 |
| producer-chain-700 | 15.464 | 9.751 | 1.586 | n/a | 0.008 | 1.750 | 5.000 | 0.000 |
| planning-14 | 36.711 | 19.154 | 1.917 | 2.410 | 48.386 | 12.538 | 1.000 | 13.000 |
| independent-negation-8 | 9.462 | 4.743 | 1.995 | n/a | 3.451 | 0.170 | 0.000 | 0.000 |
| chain-1000 | 15.542 | 7.755 | 2.004 | n/a | 0.007 | 1.201 | 3.000 | 0.000 |
| chain-arithmetic-1000 | 10.955 | 4.714 | 2.324 | 2.788 | 0.330 | 0.088 | 1.000 | 0.000 |
| chain-2000 | 26.075 | 9.987 | 2.611 | n/a | 0.009 | 2.505 | 5.000 | 0.000 |
| independent-negation-10 | 18.145 | 4.746 | 3.823 | n/a | 11.678 | 0.415 | 1.000 | 0.000 |
| transitive-dense-40 | 26.747 | 6.323 | 4.230 | 10.861 | 4.274 | 0.767 | 2.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 22.9 | 22.9 | n/a |
| n-queens/variant-04 8→11 | 16.4 | 7.8 | n/a |
| n-queens/variant-01 8→11 | 18.4 | 7.6 | n/a |
| independent-negation-aggregate-16 | 12.7 | 5.2 | n/a |
| independent-choice-16 | 10.9 | 5.1 | n/a |
| latin-square-5 | 14.6 | 5.4 | n/a |
| transitive-path-200 | 18.6 | 8.9 | n/a |
| disjunction-12 | 12.2 | 5.0 | n/a |
| send-money/send-money | 18.5 | 8.2 | n/a |
| n-queens/variant-01 8→10 | 16.4 | 6.0 | n/a |
| independent-choice-12 | 10.5 | 5.0 | n/a |
| transitive-path-100 | 11.8 | 6.0 | n/a |
| ties-50 | 15.2 | 5.4 | n/a |
| stratified-16 | 10.3 | 5.0 | n/a |
| producer-chain-700 | 28.6 | 7.3 | n/a |
| planning-14 | 13.9 | 5.4 | n/a |
| independent-negation-8 | 10.5 | 5.0 | n/a |
| chain-1000 | 16.9 | 5.4 | n/a |
| chain-arithmetic-1000 | 13.6 | 5.1 | n/a |
| chain-2000 | 24.5 | 5.9 | n/a |
| independent-negation-10 | 10.8 | 5.0 | n/a |
| transitive-dense-40 | 25.9 | 5.3 | n/a |

Against the reference: report B1-candidate, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 9 of 22 cells where both passed (40.9%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 36.495 | 179.915 | 0.203 | 2.685 | 37.365 | 8.643 | 74.000 | 101.500 |
| n-queens/variant-04 8→11 | 98.737 | 183.512 | 0.538 | 1.148 | 264.090 | 12.748 | 1.500 | 176.500 |
| n-queens/variant-01 8→11 | 105.572 | 188.028 | 0.561 | 4.781 | 264.734 | 19.259 | 2.000 | 181.000 |
| independent-negation-aggregate-16 | 26.973 | 43.006 | 0.627 | 0.210 | 3.178 | 3.279 | 0.500 | 37.500 |
| independent-choice-16 | 23.209 | 35.735 | 0.649 | n/a | 1.977 | 7.945 | 1.000 | 30.000 |
| latin-square-5 | 14.669 | 20.217 | 0.726 | 0.873 | 12.183 | 6.451 | 1.000 | 14.000 |
| disjunction-12 | 26.398 | 32.921 | 0.802 | 0.141 | 3.582 | 35.934 | 0.000 | 27.000 |
| transitive-path-200 | 17.841 | 20.016 | 0.891 | n/a | 0.008 | 3.146 | 9.500 | 5.500 |
| send-money/send-money | 12.494 | 13.115 | 0.953 | 2.479 | 6.051 | 0.157 | 7.000 | 1.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-01 8→10 | 32.025 | 30.417 | 1.053 | 3.296 | 60.643 | 4.257 | 1.500 | 24.000 |
| independent-choice-12 | 8.719 | 7.917 | 1.101 | n/a | 0.337 | 0.996 | 0.000 | 3.000 |
| transitive-path-100 | 10.420 | 8.749 | 1.191 | n/a | 0.007 | 0.760 | 3.000 | 0.500 |
| ties-50 | 26.032 | 21.857 | 1.191 | 0.303 | 13.574 | 2.276 | 1.000 | 16.000 |
| stratified-16 | 6.309 | 4.701 | 1.342 | n/a | 0.173 | 0.070 | 0.000 | 0.000 |
| producer-chain-700 | 15.309 | 9.963 | 1.537 | n/a | 0.010 | 1.752 | 5.000 | 0.000 |
| planning-14 | 37.250 | 18.539 | 2.009 | 2.612 | 44.824 | 13.218 | 1.000 | 13.000 |
| independent-negation-8 | 10.014 | 4.798 | 2.087 | n/a | 3.592 | 0.161 | 0.000 | 0.000 |
| chain-arithmetic-1000 | 10.811 | 4.531 | 2.386 | 2.955 | 0.321 | 0.094 | 1.000 | 0.000 |
| chain-1000 | 17.079 | 6.995 | 2.442 | n/a | 0.008 | 1.065 | 3.000 | 0.000 |
| chain-2000 | 27.494 | 9.850 | 2.791 | n/a | 0.015 | 2.556 | 5.000 | 0.000 |
| independent-negation-10 | 18.380 | 4.735 | 3.881 | n/a | 11.786 | 0.448 | 1.000 | 0.000 |
| transitive-dense-40 | 26.873 | 6.299 | 4.267 | 11.043 | 4.325 | 0.752 | 2.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 22.9 | 21.7 | n/a |
| n-queens/variant-04 8→11 | 16.6 | 7.4 | n/a |
| n-queens/variant-01 8→11 | 18.2 | 7.9 | n/a |
| independent-negation-aggregate-16 | 12.7 | 5.2 | n/a |
| independent-choice-16 | 10.9 | 5.1 | n/a |
| latin-square-5 | 14.7 | 5.5 | n/a |
| disjunction-12 | 12.2 | 5.0 | n/a |
| transitive-path-200 | 18.6 | 8.8 | n/a |
| send-money/send-money | 18.4 | 8.2 | n/a |
| n-queens/variant-01 8→10 | 16.5 | 5.6 | n/a |
| independent-choice-12 | 10.5 | 5.0 | n/a |
| transitive-path-100 | 11.7 | 6.3 | n/a |
| ties-50 | 15.9 | 5.4 | n/a |
| stratified-16 | 10.3 | 5.0 | n/a |
| producer-chain-700 | 28.5 | 7.1 | n/a |
| planning-14 | 13.9 | 5.4 | n/a |
| independent-negation-8 | 10.5 | 5.0 | n/a |
| chain-arithmetic-1000 | 13.7 | 5.1 | n/a |
| chain-1000 | 16.7 | 5.6 | n/a |
| chain-2000 | 24.5 | 5.9 | n/a |
| independent-negation-10 | 10.7 | 5.0 | n/a |
| transitive-dense-40 | 26.1 | 5.3 | n/a |

Against the reference: report B2-candidate, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 8 of 22 cells where both passed (36.3%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 37.457 | 196.921 | 0.190 | 2.728 | 38.776 | 8.556 | 76.000 | 115.500 |
| n-queens/variant-04 8→11 | 100.408 | 183.847 | 0.546 | 1.191 | 269.660 | 13.211 | 1.000 | 177.000 |
| n-queens/variant-01 8→11 | 103.948 | 187.771 | 0.554 | 4.754 | 260.793 | 18.267 | 2.000 | 181.000 |
| independent-choice-16 | 22.436 | 35.630 | 0.630 | n/a | 1.974 | 7.825 | 0.000 | 30.500 |
| independent-negation-aggregate-16 | 27.385 | 42.337 | 0.647 | 0.200 | 3.310 | 3.275 | 1.000 | 37.000 |
| latin-square-5 | 15.619 | 19.442 | 0.803 | 0.866 | 12.703 | 6.757 | 1.000 | 13.500 |
| disjunction-12 | 26.312 | 30.978 | 0.849 | 0.130 | 4.407 | 40.917 | 0.000 | 26.000 |
| transitive-path-200 | 17.546 | 19.753 | 0.888 | n/a | 0.008 | 3.162 | 9.500 | 5.500 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 7.949 | 7.910 | 1.005 | n/a | 0.345 | 0.981 | 0.500 | 2.500 |
| transitive-path-100 | 9.615 | 9.355 | 1.028 | n/a | 0.007 | 0.766 | 2.500 | 1.500 |
| send-money/send-money | 13.786 | 13.065 | 1.055 | 2.553 | 6.467 | 0.159 | 7.500 | 1.000 |
| n-queens/variant-01 8→10 | 32.722 | 30.473 | 1.074 | 3.485 | 62.331 | 4.348 | 1.000 | 24.000 |
| ties-50 | 26.988 | 22.386 | 1.206 | 0.317 | 14.796 | 2.251 | 0.500 | 17.000 |
| stratified-16 | 6.346 | 4.478 | 1.417 | n/a | 0.190 | 0.082 | 0.000 | 0.000 |
| producer-chain-700 | 15.398 | 9.994 | 1.541 | n/a | 0.011 | 1.759 | 5.000 | 0.000 |
| planning-14 | 37.344 | 18.501 | 2.019 | 2.591 | 45.508 | 12.846 | 1.000 | 13.000 |
| independent-negation-8 | 10.101 | 4.825 | 2.093 | n/a | 3.743 | 0.183 | 0.000 | 0.000 |
| chain-arithmetic-1000 | 11.003 | 4.694 | 2.344 | 2.858 | 0.324 | 0.089 | 1.000 | 0.000 |
| chain-1000 | 17.067 | 7.014 | 2.433 | n/a | 0.008 | 1.095 | 3.000 | 0.000 |
| chain-2000 | 26.817 | 9.281 | 2.890 | n/a | 0.010 | 2.544 | 5.000 | 0.000 |
| independent-negation-10 | 18.391 | 4.733 | 3.886 | n/a | 11.799 | 0.423 | 1.000 | 0.000 |
| transitive-dense-40 | 25.917 | 6.313 | 4.105 | 10.932 | 4.338 | 0.726 | 2.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 22.9 | 20.2 | n/a |
| n-queens/variant-04 8→11 | 16.6 | 7.6 | n/a |
| n-queens/variant-01 8→11 | 18.1 | 7.7 | n/a |
| independent-choice-16 | 10.9 | 5.1 | n/a |
| independent-negation-aggregate-16 | 12.7 | 5.2 | n/a |
| latin-square-5 | 14.6 | 5.5 | n/a |
| disjunction-12 | 12.2 | 5.0 | n/a |
| transitive-path-200 | 18.6 | 8.8 | n/a |
| independent-choice-12 | 10.5 | 5.1 | n/a |
| transitive-path-100 | 11.8 | 6.0 | n/a |
| send-money/send-money | 18.7 | 8.2 | n/a |
| n-queens/variant-01 8→10 | 16.4 | 5.9 | n/a |
| ties-50 | 15.3 | 5.4 | n/a |
| stratified-16 | 10.3 | 5.0 | n/a |
| producer-chain-700 | 28.5 | 7.3 | n/a |
| planning-14 | 14.0 | 5.4 | n/a |
| independent-negation-8 | 10.5 | 5.0 | n/a |
| chain-arithmetic-1000 | 13.7 | 5.1 | n/a |
| chain-1000 | 16.6 | 5.4 | n/a |
| chain-2000 | 24.3 | 6.2 | n/a |
| independent-negation-10 | 10.7 | 5.0 | n/a |
| transitive-dense-40 | 25.8 | 5.4 | n/a |

Against the reference: report A2-main, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 9 of 22 cells where both passed (40.9%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 36.257 | 190.206 | 0.191 | 2.868 | 37.167 | 8.563 | 74.500 | 110.000 |
| n-queens/variant-04 8→11 | 105.454 | 184.339 | 0.572 | 1.179 | 286.992 | 12.848 | 1.000 | 178.000 |
| n-queens/variant-01 8→11 | 108.777 | 189.985 | 0.573 | 4.862 | 275.179 | 18.562 | 2.000 | 182.500 |
| independent-choice-16 | 22.397 | 36.036 | 0.622 | n/a | 1.903 | 7.266 | 0.000 | 31.000 |
| independent-negation-aggregate-16 | 27.645 | 42.910 | 0.644 | 0.206 | 3.141 | 3.211 | 0.500 | 38.000 |
| latin-square-5 | 15.646 | 19.315 | 0.810 | 0.890 | 14.275 | 6.730 | 1.000 | 13.500 |
| disjunction-12 | 26.523 | 32.047 | 0.828 | 0.131 | 3.658 | 36.094 | 0.000 | 26.500 |
| send-money/send-money | 12.410 | 13.067 | 0.950 | 2.506 | 6.043 | 0.152 | 7.000 | 1.000 |
| transitive-path-200 | 19.374 | 20.020 | 0.968 | n/a | 0.006 | 3.134 | 9.000 | 6.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 7.990 | 7.921 | 1.009 | n/a | 0.337 | 0.930 | 1.000 | 2.000 |
| n-queens/variant-01 8→10 | 32.217 | 30.526 | 1.055 | 3.324 | 59.178 | 4.194 | 1.500 | 24.000 |
| transitive-path-100 | 10.448 | 9.400 | 1.111 | n/a | 0.006 | 0.770 | 2.000 | 2.000 |
| ties-50 | 25.785 | 21.987 | 1.173 | 0.305 | 15.025 | 2.192 | 0.000 | 17.000 |
| stratified-16 | 6.417 | 4.703 | 1.364 | n/a | 0.179 | 0.068 | 0.000 | 0.000 |
| producer-chain-700 | 15.285 | 10.784 | 1.417 | n/a | 0.009 | 1.740 | 5.000 | 0.000 |
| independent-negation-8 | 9.435 | 4.766 | 1.980 | n/a | 3.609 | 0.200 | 0.000 | 0.000 |
| planning-14 | 36.762 | 18.561 | 1.981 | 2.561 | 44.467 | 13.048 | 1.000 | 13.000 |
| chain-1000 | 16.299 | 7.762 | 2.100 | n/a | 0.010 | 1.131 | 3.000 | 0.000 |
| chain-arithmetic-1000 | 10.962 | 4.716 | 2.325 | 2.895 | 0.330 | 0.093 | 1.000 | 0.000 |
| chain-2000 | 27.648 | 10.807 | 2.558 | n/a | 0.009 | 2.532 | 6.000 | 0.000 |
| transitive-dense-40 | 25.848 | 6.266 | 4.125 | 10.943 | 4.503 | 0.804 | 2.000 | 0.000 |
| independent-negation-10 | 19.197 | 4.631 | 4.145 | n/a | 12.148 | 0.431 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 22.5 | 22.1 | n/a |
| n-queens/variant-04 8→11 | 16.4 | 7.7 | n/a |
| n-queens/variant-01 8→11 | 18.3 | 7.6 | n/a |
| independent-choice-16 | 10.9 | 5.1 | n/a |
| independent-negation-aggregate-16 | 12.6 | 5.2 | n/a |
| latin-square-5 | 14.5 | 5.5 | n/a |
| disjunction-12 | 12.2 | 5.0 | n/a |
| send-money/send-money | 18.5 | 8.7 | n/a |
| transitive-path-200 | 18.7 | 9.6 | n/a |
| independent-choice-12 | 10.6 | 5.1 | n/a |
| n-queens/variant-01 8→10 | 16.4 | 5.8 | n/a |
| transitive-path-100 | 11.8 | 6.0 | n/a |
| ties-50 | 15.4 | 5.4 | n/a |
| stratified-16 | 10.3 | 5.0 | n/a |
| producer-chain-700 | 28.6 | 7.3 | n/a |
| independent-negation-8 | 10.5 | 5.0 | n/a |
| planning-14 | 13.8 | 5.4 | n/a |
| chain-1000 | 16.8 | 5.4 | n/a |
| chain-arithmetic-1000 | 13.7 | 5.1 | n/a |
| chain-2000 | 24.5 | 5.9 | n/a |
| transitive-dense-40 | 25.9 | 5.3 | n/a |
| independent-negation-10 | 10.8 | 5.0 | n/a |
