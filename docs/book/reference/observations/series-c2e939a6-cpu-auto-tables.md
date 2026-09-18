Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

Formula search method by report: main=default; before=default; after=default; clauses=clauses;

| Cell | main | before | after | clauses | after/before | before/main | clauses/after | clauses/main | main/reference | before/reference | after/reference | clauses/reference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 23.713 [22.840, 24.256] | 9.209 [8.926, 9.969] | 8.993 [8.784, 10.219] | 7.791 [7.691, 8.109] | 0.977 | 0.388 | 0.866 | 0.329 | 3.347 | 1.301 | 1.340 | 1.283 |
| independent-choice-16 | 195.735 [184.496, 198.475] | 39.287 [38.047, 40.932] | 39.457 [38.961, 41.073] | 40.891 [38.153, 43.510] | 1.004 | 0.201 | 1.036 | 0.209 | 6.829 | 1.415 | 1.408 | 1.441 |
| independent-negation-8 | 114.157 [112.046, 119.023] | 18.634 [18.262, 19.440] | 18.565 [18.414, 20.740] | 18.262 [18.071, 19.265] | 0.996 | 0.163 | 0.984 | 0.160 | 20.297 | 4.083 | 3.328 | 3.080 |
| independent-negation-10 | 1430.897 [1413.123, 1562.193] | 53.276 [49.956, 56.165] | 52.270 [51.448, 53.357] | 51.583 [51.474, 52.137] | 0.981 | 0.037 | 0.987 | 0.036 | 253.137 | 9.493 | 11.125 | 9.227 |
| independent-negation-aggregate-16 | 314.604 [313.529, 319.317] | 24.692 [24.580, 28.818] | 21.701 [20.234, 21.799] | 26.015 [25.810, 26.427] | 0.879 | 0.078 | 1.199 | 0.083 | 10.293 | 0.765 | 0.664 | 0.833 |
| disjunction-12 | 206.672 [205.580, 212.003] | 45.389 [45.285, 45.642] | 22.013 [21.681, 22.396] | 45.501 [44.537, 45.604] | 0.485 | 0.220 | 2.067 | 0.220 | 9.695 | 2.034 | 1.050 | 2.046 |
| ties-50 | 181.468 [173.836, 184.590] | 26.345 [25.967, 26.660] | 21.103 [20.418, 23.217] | 27.144 [25.614, 27.365] | 0.801 | 0.145 | 1.286 | 0.150 | 9.053 | 1.391 | 1.173 | 1.443 |
| transitive-path-100 | 33.481 [32.087, 35.634] | 16.741 [14.492, 18.350] | 15.663 [13.401, 17.156] | 15.972 [15.814, 17.170] | 0.936 | 0.500 | 1.020 | 0.477 | 3.765 | 1.684 | 1.944 | 1.760 |
| transitive-path-200 | 191.380 [185.894, 193.306] | 46.367 [44.383, 69.625] | 46.566 [46.114, 50.884] | 48.136 [46.010, 49.294] | 1.004 | 0.242 | 1.034 | 0.252 | 8.458 | 1.947 | 2.100 | 1.911 |
| transitive-dense-40 | 40.229 [39.938, 41.244] | 34.165 [32.527, 34.225] | 30.374 [28.251, 32.267] | 34.464 [32.655, 34.772] | 0.889 | 0.849 | 1.135 | 0.857 | 5.993 | 5.114 | 4.543 | 5.170 |
| chain-1000 | 101.926 [101.141, 103.633] | 22.528 [18.761, 22.763] | 21.829 [19.691, 21.840] | 20.829 [19.999, 21.592] | 0.969 | 0.221 | 0.954 | 0.204 | 13.020 | 2.932 | 2.825 | 3.138 |
| chain-2000 | 365.261 [361.241, 383.320] | 36.671 [35.626, 36.727] | 38.044 [34.722, 38.344] | 37.525 [34.598, 38.444] | 1.037 | 0.100 | 0.986 | 0.103 | 33.216 | 3.693 | 3.495 | 3.362 |
| chain-arithmetic-1000 | 10.966 [10.010, 13.847] | 9.841 [9.269, 10.964] | 9.942 [9.799, 11.483] | 10.403 [10.250, 10.746] | 1.010 | 0.897 | 1.046 | 0.949 | 1.980 | 2.185 | 1.769 | 1.811 |
| stratified-16 | blocked by timeout ×3 | 4.495 [4.459, 4.496] | 4.536 [4.487, 5.668] | 4.545 [4.531, 4.562] | 1.009 | n/a | 1.002 | n/a | n/a | 1.014 | 1.029 | 0.996 |
| producer-chain-700 | 28.989 [28.019, 30.207] | 26.914 [24.729, 30.176] | 27.962 [25.811, 28.158] | 30.111 [26.863, 32.983] | 1.039 | 0.928 | 1.077 | 1.039 | 2.651 | 2.770 | 2.852 | 3.039 |
| n-queens/variant-01 8→10 | 126.746 [125.642, 128.609] | 58.947 [57.550, 59.959] | 33.260 [33.063, 33.892] | 59.883 [59.230, 60.542] | 0.564 | 0.465 | 1.800 | 0.472 | 4.058 | 1.843 | 1.051 | 1.849 |
| n-queens/variant-01 8→11 | 475.021 [458.909, 485.528] | 256.192 [255.880, 257.962] | 121.300 [121.095, 121.452] | 264.540 [262.335, 266.777] | 0.473 | 0.539 | 2.181 | 0.557 | 2.561 | 1.406 | 0.661 | 1.449 |
| n-queens/variant-04 8→11 | 317.752 [305.164, 319.374] | 157.993 [157.286, 158.390] | 98.519 [98.114, 99.310] | 162.744 [161.759, 163.517] | 0.624 | 0.497 | 1.652 | 0.512 | 1.737 | 0.874 | 0.541 | 0.908 |
| send-money/send-money | 52.666 [52.215, 53.756] | 11.293 [10.494, 12.937] | 15.232 [13.764, 15.491] | 11.446 [10.961, 12.440] | 1.349 | 0.214 | 0.751 | 0.217 | 3.433 | 0.728 | 1.004 | 0.787 |
| variant-04/05-larger-mix | 343.333 [317.945, 347.546] | 89.950 [88.957, 90.945] | 44.348 [43.811, 44.701] | 95.690 [95.648, 96.509] | 0.493 | 0.262 | 2.158 | 0.279 | 2.201 | 0.582 | 0.293 | 0.640 |

Reference wall time, ms, same notation.

| Cell | main | before | after | clauses |
|---|---:|---:|---:|---:|
| independent-choice-12 | 7.085 [6.817, 7.130] | 7.076 [6.889, 7.222] | 6.710 [6.085, 6.862] | 6.071 [5.818, 6.935] |
| independent-choice-16 | 28.662 [28.130, 29.319] | 27.765 [27.071, 28.275] | 28.024 [27.678, 29.662] | 28.381 [27.678, 30.779] |
| independent-negation-8 | 5.624 [4.586, 5.660] | 4.564 [4.493, 5.533] | 5.578 [4.499, 5.981] | 5.930 [4.539, 6.687] |
| independent-negation-10 | 5.653 [4.531, 5.665] | 5.612 [4.551, 5.679] | 4.699 [4.525, 5.605] | 5.590 [4.544, 5.775] |
| independent-negation-aggregate-16 | 30.565 [30.363, 33.456] | 32.296 [31.439, 34.317] | 32.691 [31.395, 33.400] | 31.243 [31.169, 31.580] |
| disjunction-12 | 21.318 [21.223, 22.716] | 22.312 [20.919, 22.322] | 20.970 [19.733, 24.317] | 22.242 [19.542, 24.322] |
| ties-50 | 20.044 [19.143, 21.440] | 18.946 [17.426, 19.517] | 17.987 [17.174, 18.535] | 18.812 [18.721, 20.008] |
| transitive-path-100 | 8.893 [8.882, 10.296] | 9.941 [8.907, 11.119] | 8.058 [7.815, 9.949] | 9.074 [7.884, 9.147] |
| transitive-path-200 | 22.627 [21.645, 25.081] | 23.811 [22.324, 24.886] | 22.175 [21.191, 25.245] | 25.194 [23.229, 26.372] |
| transitive-dense-40 | 6.713 [6.678, 6.784] | 6.681 [6.625, 6.940] | 6.685 [5.594, 6.710] | 6.666 [6.659, 6.696] |
| chain-1000 | 7.828 [7.809, 7.976] | 7.683 [6.840, 7.718] | 7.727 [6.577, 7.742] | 6.638 [6.516, 8.931] |
| chain-2000 | 10.996 [10.991, 12.155] | 9.930 [9.926, 12.078] | 10.884 [9.937, 11.268] | 11.160 [9.963, 11.185] |
| chain-arithmetic-1000 | 5.539 [4.514, 5.612] | 4.505 [4.480, 6.816] | 5.620 [4.510, 5.753] | 5.745 [5.529, 5.781] |
| stratified-16 | 4.612 [4.458, 5.654] | 4.433 [4.399, 4.629] | 4.407 [3.534, 4.424] | 4.562 [3.346, 4.582] |
| producer-chain-700 | 10.936 [10.173, 10.940] | 9.718 [9.703, 9.790] | 9.804 [8.686, 10.853] | 9.908 [8.703, 10.889] |
| n-queens/variant-01 8→10 | 31.236 [31.180, 33.602] | 31.978 [30.099, 32.343] | 31.649 [31.120, 32.394] | 32.394 [30.679, 33.474] |
| n-queens/variant-01 8→11 | 185.491 [184.038, 196.246] | 182.244 [181.819, 187.297] | 183.448 [183.148, 184.758] | 182.578 [181.868, 183.350] |
| n-queens/variant-04 8→11 | 182.962 [181.660, 185.846] | 180.815 [179.643, 181.000] | 181.947 [180.547, 183.604] | 179.223 [178.320, 179.968] |
| send-money/send-money | 15.339 [14.091, 16.599] | 15.517 [15.172, 16.448] | 15.171 [14.108, 16.109] | 14.544 [12.908, 15.443] |
| variant-04/05-larger-mix | 155.963 [149.891, 159.834] | 154.427 [151.110, 165.394] | 151.165 [150.910, 152.925] | 149.405 [148.092, 150.284] |

Counters of report clauses: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 1090269 | 5.362 |
| independent-choice-16 | 0 | 2584 | 2584 | 10427471 | 37.560 |
| independent-negation-8 | 0 | 55 | 55 | 123109 | 15.192 |
| independent-negation-10 | 0 | 144 | 144 | 425724 | 48.725 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 3137480 | 22.028 |
| disjunction-12 | 0 | 4096 | 4096 | 9004441 | 41.458 |
| ties-50 | 0 | 1225 | 1225 | 4720400 | 23.192 |
| transitive-path-100 | 0 | 1 | 1 | 1197772 | 12.233 |
| transitive-path-200 | 0 | 1 | 1 | 5378096 | 43.894 |
| transitive-dense-40 | 0 | 1 | 1 | 2173586 | 30.372 |
| chain-1000 | 0 | 1 | 1 | 475452 | 15.473 |
| chain-2000 | 0 | 1 | 1 | 1025311 | 31.072 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 193171 | 6.155 |
| stratified-16 | 0 | 1 | 1 | 9133 | 1.597 |
| producer-chain-700 | 0 | 1 | 1 | 81187 | 25.276 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 11383348 | 56.038 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 57156138 | 260.467 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 35658773 | 158.960 |
| send-money/send-money | 0 | 1 | 1 | 349198 | 7.164 |
| variant-04/05-larger-mix | 0 | 1176 | 1208 | 23292550 | 91.801 |

Against the reference: report main, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 0 of 19 cells where both passed (0.0%).

Wins, fastest first: none.

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-04 8→11 | 317.752 | 182.962 | 1.737 | 2.140 | 145.527 | 34.948 | 1.000 | 176.000 |
| chain-arithmetic-1000 | 10.966 | 5.539 | 1.980 | 4.047 | 0.651 | 0.176 | 1.000 | 0.000 |
| variant-04/05-larger-mix | 343.333 | 155.963 | 2.201 | 4.352 | 81.554 | 26.117 | 87.000 | 62.000 |
| n-queens/variant-01 8→11 | 475.021 | 185.491 | 2.561 | 37.684 | 238.761 | 56.681 | 2.000 | 177.000 |
| producer-chain-700 | 28.989 | 10.936 | 2.651 | n/a | 0.011 | 3.174 | 5.000 | 0.000 |
| independent-choice-12 | 23.713 | 7.085 | 3.347 | n/a | 0.313 | 6.121 | 0.000 | 2.000 |
| send-money/send-money | 52.666 | 15.339 | 3.433 | 28.509 | 16.589 | 1.056 | 9.000 | 1.000 |
| transitive-path-100 | 33.481 | 8.893 | 3.765 | n/a | 0.004 | 25.370 | 4.000 | 0.000 |
| n-queens/variant-01 8→10 | 126.746 | 31.236 | 4.058 | 27.423 | 49.740 | 13.029 | 2.000 | 26.000 |
| transitive-dense-40 | 40.229 | 6.713 | 5.993 | 22.564 | 9.894 | 1.248 | 2.000 | 0.000 |
| independent-choice-16 | 195.735 | 28.662 | 6.829 | n/a | 3.227 | 61.219 | 0.000 | 24.000 |
| transitive-path-200 | 191.380 | 22.627 | 8.458 | n/a | 0.007 | 173.380 | 13.000 | 4.000 |
| ties-50 | 181.468 | 20.044 | 9.053 | 0.422 | 13.587 | 5.040 | 1.000 | 14.000 |
| disjunction-12 | 206.672 | 21.318 | 9.695 | 0.158 | 12.686 | 2.550 | 0.000 | 17.000 |
| independent-negation-aggregate-16 | 314.604 | 30.565 | 10.293 | 0.279 | 7.865 | 8.067 | 0.000 | 26.000 |
| chain-1000 | 101.926 | 7.828 | 13.020 | n/a | 0.005 | 85.097 | 4.000 | 0.000 |
| independent-negation-8 | 114.157 | 5.624 | 20.297 | n/a | 8.864 | 99.933 | 1.000 | 0.000 |
| chain-2000 | 365.261 | 10.996 | 33.216 | n/a | 0.011 | 334.428 | 6.000 | 0.000 |
| independent-negation-10 | 1430.897 | 5.653 | 253.137 | n/a | 122.484 | 1299.179 | 1.000 | 0.000 |

Against the reference: report before, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 4 of 20 cells where both passed (20.0%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 89.950 | 154.427 | 0.582 | 3.269 | 43.604 | 20.713 | 89.000 | 60.000 |
| send-money/send-money | 11.293 | 15.517 | 0.728 | 2.577 | 1.729 | 0.224 | 9.000 | 1.000 |
| independent-negation-aggregate-16 | 24.692 | 32.296 | 0.765 | 0.237 | 2.344 | 5.243 | 0.000 | 27.000 |
| n-queens/variant-04 8→11 | 157.993 | 180.815 | 0.874 | 1.515 | 111.956 | 28.922 | 2.000 | 174.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stratified-16 | 4.495 | 4.433 | 1.014 | n/a | 0.490 | 0.276 | 0.000 | 0.000 |
| independent-choice-12 | 9.209 | 7.076 | 1.301 | n/a | 0.704 | 3.445 | 0.000 | 2.000 |
| ties-50 | 26.345 | 18.946 | 1.391 | 0.348 | 7.057 | 4.214 | 0.000 | 14.000 |
| n-queens/variant-01 8→11 | 256.192 | 182.244 | 1.406 | 5.704 | 190.474 | 45.112 | 2.000 | 176.000 |
| independent-choice-16 | 39.287 | 27.765 | 1.415 | n/a | 3.101 | 22.801 | 0.000 | 23.000 |
| transitive-path-100 | 16.741 | 9.941 | 1.684 | n/a | 0.003 | 7.898 | 3.000 | 1.000 |
| n-queens/variant-01 8→10 | 58.947 | 31.978 | 1.843 | 4.044 | 36.046 | 9.992 | 2.000 | 25.000 |
| transitive-path-200 | 46.367 | 23.811 | 1.947 | n/a | 0.010 | 29.638 | 14.000 | 4.000 |
| disjunction-12 | 45.389 | 22.312 | 2.034 | 0.132 | 3.489 | 1.758 | 0.000 | 18.000 |
| chain-arithmetic-1000 | 9.841 | 4.505 | 2.185 | 3.696 | 0.585 | 0.112 | 1.000 | 0.000 |
| producer-chain-700 | 26.914 | 9.718 | 2.770 | n/a | 0.007 | 3.090 | 5.000 | 0.000 |
| chain-1000 | 22.528 | 7.683 | 2.932 | n/a | 0.012 | 3.644 | 3.000 | 0.000 |
| chain-2000 | 36.671 | 9.930 | 3.693 | n/a | 0.013 | 5.993 | 6.000 | 0.000 |
| independent-negation-8 | 18.634 | 4.564 | 4.083 | n/a | 14.211 | 0.967 | 0.000 | 0.000 |
| transitive-dense-40 | 34.165 | 6.681 | 5.114 | 15.823 | 9.129 | 1.004 | 2.000 | 0.000 |
| independent-negation-10 | 53.276 | 5.612 | 9.493 | n/a | 45.096 | 2.851 | 1.000 | 0.000 |

Against the reference: report after, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 4 of 20 cells where both passed (20.0%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 44.348 | 151.165 | 0.293 | 3.266 | 18.770 | 0.139 | 85.000 | 60.000 |
| n-queens/variant-04 8→11 | 98.519 | 181.947 | 0.541 | 1.518 | 74.605 | 0.105 | 1.000 | 175.000 |
| n-queens/variant-01 8→11 | 121.300 | 183.448 | 0.661 | 5.741 | 94.160 | 0.183 | 2.000 | 177.000 |
| independent-negation-aggregate-16 | 21.701 | 32.691 | 0.664 | 0.236 | 2.637 | 0.011 | 0.000 | 28.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| send-money/send-money | 15.232 | 15.171 | 1.004 | 2.574 | 4.093 | 0.267 | 9.000 | 1.000 |
| stratified-16 | 4.536 | 4.407 | 1.029 | n/a | 0.477 | 0.313 | 0.000 | 0.000 |
| disjunction-12 | 22.013 | 20.970 | 1.050 | 0.137 | 3.860 | 0.004 | 0.000 | 16.000 |
| n-queens/variant-01 8→10 | 33.260 | 31.649 | 1.051 | 4.321 | 19.631 | 0.147 | 1.000 | 25.000 |
| ties-50 | 21.103 | 17.987 | 1.173 | 0.344 | 6.991 | 0.038 | 1.000 | 13.000 |
| independent-choice-12 | 8.993 | 6.710 | 1.340 | n/a | 0.628 | 3.365 | 0.000 | 2.000 |
| independent-choice-16 | 39.457 | 28.024 | 1.408 | n/a | 2.921 | 24.044 | 0.000 | 23.000 |
| chain-arithmetic-1000 | 9.942 | 5.620 | 1.769 | 3.695 | 0.919 | 0.123 | 1.000 | 0.000 |
| transitive-path-100 | 15.663 | 8.058 | 1.944 | n/a | 0.004 | 8.451 | 4.000 | 0.000 |
| transitive-path-200 | 46.566 | 22.175 | 2.100 | n/a | 0.012 | 29.032 | 12.000 | 4.000 |
| chain-1000 | 21.829 | 7.727 | 2.825 | n/a | 0.008 | 3.663 | 3.000 | 0.000 |
| producer-chain-700 | 27.962 | 9.804 | 2.852 | n/a | 0.013 | 2.694 | 5.000 | 0.000 |
| independent-negation-8 | 18.565 | 5.578 | 3.328 | n/a | 13.423 | 0.977 | 0.000 | 0.000 |
| chain-2000 | 38.044 | 10.884 | 3.495 | n/a | 0.014 | 6.187 | 6.000 | 0.000 |
| transitive-dense-40 | 30.374 | 6.685 | 4.543 | 15.878 | 4.967 | 1.206 | 2.000 | 0.000 |
| independent-negation-10 | 52.270 | 4.699 | 11.125 | n/a | 45.066 | 3.022 | 1.000 | 0.000 |

Against the reference: report clauses, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search clauses): faster on 5 of 20 cells where both passed (25.0%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 95.690 | 149.405 | 0.640 | 3.250 | 43.574 | 27.097 | 85.000 | 58.000 |
| send-money/send-money | 11.446 | 14.544 | 0.787 | 2.554 | 1.757 | 0.234 | 9.000 | 1.000 |
| independent-negation-aggregate-16 | 26.015 | 31.243 | 0.833 | 0.236 | 2.412 | 5.987 | 1.000 | 27.000 |
| n-queens/variant-04 8→11 | 162.744 | 179.223 | 0.908 | 1.494 | 111.996 | 33.720 | 2.000 | 172.000 |
| stratified-16 | 4.545 | 4.562 | 0.996 | n/a | 0.488 | 0.305 | 0.000 | 0.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 7.791 | 6.071 | 1.283 | n/a | 0.578 | 3.186 | 1.000 | 1.000 |
| independent-choice-16 | 40.891 | 28.381 | 1.441 | n/a | 3.251 | 22.791 | 0.000 | 23.000 |
| ties-50 | 27.144 | 18.812 | 1.443 | 0.345 | 6.881 | 5.036 | 1.000 | 14.000 |
| n-queens/variant-01 8→11 | 264.540 | 182.578 | 1.449 | 5.779 | 190.200 | 52.866 | 2.000 | 177.000 |
| transitive-path-100 | 15.972 | 9.074 | 1.760 | n/a | 0.007 | 7.433 | 4.000 | 0.000 |
| chain-arithmetic-1000 | 10.403 | 5.745 | 1.811 | 3.681 | 0.596 | 0.117 | 1.000 | 0.000 |
| n-queens/variant-01 8→10 | 59.883 | 32.394 | 1.849 | 3.958 | 36.450 | 11.413 | 2.000 | 26.000 |
| transitive-path-200 | 48.136 | 25.194 | 1.911 | n/a | 0.008 | 30.232 | 15.000 | 5.000 |
| disjunction-12 | 45.501 | 22.242 | 2.046 | 0.136 | 3.841 | 2.317 | 0.000 | 18.000 |
| producer-chain-700 | 30.111 | 9.908 | 3.039 | n/a | 0.006 | 3.623 | 6.000 | 0.000 |
| independent-negation-8 | 18.262 | 5.930 | 3.080 | n/a | 13.595 | 0.928 | 0.000 | 0.000 |
| chain-1000 | 20.829 | 6.638 | 3.138 | n/a | 0.006 | 3.166 | 3.000 | 0.000 |
| chain-2000 | 37.525 | 11.160 | 3.362 | n/a | 0.010 | 6.045 | 6.000 | 0.000 |
| transitive-dense-40 | 34.464 | 6.666 | 5.170 | 16.573 | 9.321 | 1.042 | 2.000 | 0.000 |
| independent-negation-10 | 51.583 | 5.590 | 9.227 | n/a | 44.482 | 2.856 | 1.000 | 0.000 |
