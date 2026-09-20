Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

Formula search method by report: before=default; after=default; clauses=clauses;

| Cell | before | after | clauses | after/before | clauses/after | clauses/before | before/reference | after/reference | clauses/reference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 9.943 [9.158, 10.046] | 8.955 [8.120, 8.970] | 9.145 [8.895, 10.065] | 0.901 | 1.021 | 0.920 | 1.414 | 1.276 | 1.320 |
| independent-choice-16 | 41.145 [38.356, 42.099] | 39.871 [39.700, 42.593] | 39.178 [38.233, 39.210] | 0.969 | 0.983 | 0.952 | 1.457 | 1.324 | 1.462 |
| independent-negation-8 | 18.343 [18.254, 20.012] | 17.949 [17.360, 20.471] | 18.634 [17.993, 19.395] | 0.979 | 1.038 | 1.016 | 3.275 | 3.840 | 4.084 |
| independent-negation-10 | 55.429 [53.312, 56.863] | 51.620 [50.577, 52.462] | 52.609 [51.527, 54.524] | 0.931 | 1.019 | 0.949 | 9.802 | 9.226 | 9.275 |
| independent-negation-aggregate-16 | 22.125 [21.732, 22.160] | 20.610 [20.497, 22.295] | 27.402 [26.220, 27.524] | 0.932 | 1.330 | 1.238 | 0.681 | 0.647 | 0.901 |
| disjunction-12 | 22.524 [21.570, 23.219] | 21.822 [21.464, 23.598] | 45.359 [42.682, 45.770] | 0.969 | 2.079 | 2.014 | 0.997 | 0.944 | 2.163 |
| ties-50 | 21.358 [21.096, 22.279] | 21.136 [21.033, 21.176] | 27.483 [27.105, 27.607] | 0.990 | 1.300 | 1.287 | 1.156 | 1.071 | 1.493 |
| transitive-path-100 | 16.665 [16.636, 16.801] | 14.862 [14.518, 16.699] | 16.025 [15.617, 16.728] | 0.892 | 1.078 | 0.962 | 1.869 | 1.815 | 1.754 |
| transitive-path-200 | 48.983 [46.247, 52.063] | 44.431 [44.306, 45.770] | 48.521 [46.254, 49.691] | 0.907 | 1.092 | 0.991 | 2.034 | 1.867 | 2.132 |
| transitive-dense-40 | 33.004 [31.610, 33.342] | 29.910 [28.690, 35.637] | 35.038 [34.498, 38.238] | 0.906 | 1.171 | 1.062 | 4.907 | 4.375 | 5.235 |
| chain-1000 | 20.570 [19.545, 21.800] | 21.740 [20.646, 22.055] | 21.672 [20.890, 22.046] | 1.057 | 0.997 | 1.054 | 2.666 | 2.810 | 2.796 |
| chain-2000 | 39.185 [38.840, 39.196] | 36.777 [34.513, 37.785] | 39.020 [38.866, 39.884] | 0.939 | 1.061 | 0.996 | 3.254 | 3.386 | 3.504 |
| chain-arithmetic-1000 | 10.983 [9.953, 11.063] | 9.975 [9.799, 11.603] | 9.985 [9.876, 10.298] | 0.908 | 1.001 | 0.909 | 1.940 | 1.831 | 2.200 |
| stratified-16 | 4.539 [4.439, 4.695] | 4.528 [4.475, 4.574] | 4.542 [4.476, 4.885] | 0.998 | 1.003 | 1.001 | 1.019 | 0.821 | 1.006 |
| producer-chain-700 | 28.146 [26.890, 29.349] | 27.979 [26.798, 32.413] | 29.130 [26.890, 29.220] | 0.994 | 1.041 | 1.035 | 2.804 | 2.568 | 2.439 |
| latin-square-5 | 15.425 [14.964, 15.805] | 16.209 [15.779, 16.774] | 36.668 [36.349, 37.408] | 1.051 | 2.262 | 2.377 | 0.980 | 1.032 | 2.293 |
| planning-14 | 40.080 [38.943, 40.863] | 37.982 [37.049, 38.308] | 86.456 [85.338, 87.618] | 0.948 | 2.276 | 2.157 | 2.376 | 2.315 | 5.116 |
| n-queens/variant-01 8→10 | 35.005 [34.847, 35.362] | 34.011 [33.382, 34.253] | 62.700 [59.548, 64.326] | 0.972 | 1.843 | 1.791 | 1.042 | 1.056 | 1.926 |
| n-queens/variant-01 8→11 | 129.749 [125.983, 132.096] | 121.428 [121.114, 123.268] | 273.827 [269.737, 274.777] | 0.936 | 2.255 | 2.110 | 0.688 | 0.658 | 1.442 |
| n-queens/variant-04 8→11 | 106.131 [105.560, 106.733] | 101.219 [99.474, 101.831] | 164.753 [164.199, 165.774] | 0.954 | 1.628 | 1.552 | 0.574 | 0.561 | 0.887 |
| send-money/send-money | 14.156 [12.373, 14.933] | 13.339 [13.299, 15.419] | 10.935 [10.454, 11.342] | 0.942 | 0.820 | 0.772 | 0.935 | 0.882 | 0.672 |
| variant-04/05-larger-mix | 48.354 [47.801, 52.766] | 44.519 [43.289, 47.475] | 94.605 [94.194, 95.030] | 0.921 | 2.125 | 1.957 | 0.306 | 0.295 | 0.591 |

Reference wall time, ms, same notation.

| Cell | before | after | clauses |
|---|---:|---:|---:|
| independent-choice-12 | 7.030 [6.815, 7.991] | 7.020 [6.917, 7.904] | 6.926 [6.806, 7.103] |
| independent-choice-16 | 28.234 [27.258, 28.346] | 30.114 [28.255, 30.246] | 26.794 [25.346, 26.859] |
| independent-negation-8 | 5.601 [4.868, 5.635] | 4.674 [4.611, 5.546] | 4.563 [4.529, 4.607] |
| independent-negation-10 | 5.655 [5.590, 5.674] | 5.595 [3.389, 5.718] | 5.672 [5.621, 5.781] |
| independent-negation-aggregate-16 | 32.484 [32.019, 33.313] | 31.837 [30.359, 32.373] | 30.408 [30.403, 30.563] |
| disjunction-12 | 22.595 [21.128, 23.073] | 23.109 [21.550, 23.421] | 20.971 [20.742, 21.335] |
| ties-50 | 18.483 [18.123, 18.865] | 19.737 [17.694, 19.889] | 18.404 [18.245, 18.888] |
| transitive-path-100 | 8.918 [8.023, 9.133] | 8.188 [8.025, 8.874] | 9.137 [7.764, 9.177] |
| transitive-path-200 | 24.076 [23.208, 25.093] | 23.803 [22.341, 23.843] | 22.760 [21.881, 23.252] |
| transitive-dense-40 | 6.726 [6.689, 7.799] | 6.836 [5.577, 7.938] | 6.693 [6.665, 6.729] |
| chain-1000 | 7.717 [7.622, 7.945] | 7.738 [6.571, 8.013] | 7.751 [7.639, 7.885] |
| chain-2000 | 12.040 [10.913, 12.220] | 10.860 [9.889, 13.227] | 11.136 [10.874, 12.144] |
| chain-arithmetic-1000 | 5.662 [5.508, 5.723] | 5.448 [4.457, 5.740] | 4.538 [4.536, 5.510] |
| stratified-16 | 4.453 [4.386, 4.527] | 5.514 [5.507, 6.673] | 4.513 [4.462, 4.571] |
| producer-chain-700 | 10.038 [9.901, 10.064] | 10.896 [9.764, 11.934] | 11.943 [10.890, 11.952] |
| latin-square-5 | 15.746 [15.727, 16.742] | 15.714 [15.543, 18.371] | 15.992 [15.927, 16.144] |
| planning-14 | 16.867 [16.771, 17.909] | 16.404 [15.511, 16.408] | 16.900 [15.981, 17.734] |
| n-queens/variant-01 8→10 | 33.610 [32.533, 36.786] | 32.198 [30.520, 32.938] | 32.548 [31.395, 32.673] |
| n-queens/variant-01 8→11 | 188.695 [188.240, 193.300] | 184.458 [183.213, 185.169] | 189.913 [187.420, 191.579] |
| n-queens/variant-04 8→11 | 184.962 [183.400, 185.391] | 180.406 [179.088, 183.579] | 185.830 [184.498, 185.932] |
| send-money/send-money | 15.147 [13.939, 15.396] | 15.123 [14.075, 15.278] | 16.262 [15.320, 16.388] |
| variant-04/05-larger-mix | 158.199 [157.714, 159.362] | 151.064 [150.836, 151.252] | 160.060 [159.578, 161.144] |

Counters of report clauses: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 1090269 | 5.809 |
| independent-choice-16 | 0 | 2584 | 2584 | 10427471 | 35.562 |
| independent-negation-8 | 0 | 55 | 55 | 123109 | 15.784 |
| independent-negation-10 | 0 | 144 | 144 | 425724 | 49.107 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 3137480 | 23.315 |
| disjunction-12 | 0 | 4096 | 4096 | 9004441 | 41.230 |
| ties-50 | 0 | 1225 | 1225 | 4720400 | 23.130 |
| transitive-path-100 | 0 | 1 | 1 | 1197772 | 12.539 |
| transitive-path-200 | 0 | 1 | 1 | 5378096 | 44.805 |
| transitive-dense-40 | 0 | 1 | 1 | 2173586 | 31.789 |
| chain-1000 | 0 | 1 | 1 | 475452 | 16.929 |
| chain-2000 | 0 | 1 | 1 | 1025311 | 32.611 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 193171 | 6.210 |
| stratified-16 | 0 | 1 | 1 | 9133 | 1.562 |
| producer-chain-700 | 0 | 1 | 1 | 81187 | 23.423 |
| latin-square-5 | 0 | 1344 | 1344 | 8125616 | 32.717 |
| planning-14 | 0 | 3432 | 3432 | 19467097 | 82.405 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 11383348 | 58.297 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 57156138 | 269.481 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 35658773 | 160.776 |
| send-money/send-money | 0 | 1 | 1 | 349198 | 7.066 |
| variant-04/05-larger-mix | 0 | 1176 | 1208 | 23292550 | 90.480 |

Against the reference: report before, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 7 of 22 cells where both passed (31.8%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 48.354 | 158.199 | 0.306 | 3.462 | 93.594 | 14.323 | 90.000 | 62.000 |
| n-queens/variant-04 8→11 | 106.131 | 184.962 | 0.574 | 1.565 | 352.605 | 20.115 | 1.000 | 178.000 |
| independent-negation-aggregate-16 | 22.125 | 32.484 | 0.681 | 0.234 | 8.204 | 3.842 | 0.000 | 28.000 |
| n-queens/variant-01 8→11 | 129.749 | 188.695 | 0.688 | 5.912 | 412.157 | 30.044 | 2.000 | 182.000 |
| send-money/send-money | 14.156 | 15.147 | 0.935 | 2.677 | 11.082 | 0.265 | 9.000 | 1.000 |
| latin-square-5 | 15.425 | 15.746 | 0.980 | 1.248 | 23.611 | 9.885 | 1.000 | 10.000 |
| disjunction-12 | 22.524 | 22.595 | 0.997 | 0.139 | 6.792 | 30.838 | 1.000 | 17.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stratified-16 | 4.539 | 4.453 | 1.019 | n/a | 0.481 | 0.272 | 0.000 | 0.000 |
| n-queens/variant-01 8→10 | 35.005 | 33.610 | 1.042 | 4.068 | 88.376 | 6.865 | 2.000 | 26.000 |
| ties-50 | 21.358 | 18.483 | 1.156 | 0.390 | 19.136 | 3.120 | 1.000 | 13.000 |
| independent-choice-12 | 9.943 | 7.030 | 1.414 | n/a | 0.691 | 3.442 | 0.000 | 2.000 |
| independent-choice-16 | 41.145 | 28.234 | 1.457 | n/a | 3.178 | 24.158 | 1.000 | 22.000 |
| transitive-path-100 | 16.665 | 8.918 | 1.869 | n/a | 0.011 | 8.125 | 4.000 | 0.000 |
| chain-arithmetic-1000 | 10.983 | 5.662 | 1.940 | 3.774 | 0.655 | 0.157 | 1.000 | 0.000 |
| transitive-path-200 | 48.983 | 24.076 | 2.034 | n/a | 0.006 | 31.301 | 15.000 | 5.000 |
| planning-14 | 40.080 | 16.867 | 2.376 | 3.185 | 75.624 | 19.213 | 1.000 | 11.000 |
| chain-1000 | 20.570 | 7.717 | 2.666 | n/a | 0.010 | 3.467 | 3.000 | 0.000 |
| producer-chain-700 | 28.146 | 10.038 | 2.804 | n/a | 0.010 | 3.165 | 5.000 | 0.000 |
| chain-2000 | 39.185 | 12.040 | 3.254 | n/a | 0.014 | 6.427 | 7.000 | 0.000 |
| independent-negation-8 | 18.343 | 5.601 | 3.275 | n/a | 13.243 | 0.942 | 1.000 | 0.000 |
| transitive-dense-40 | 33.004 | 6.726 | 4.907 | 17.010 | 5.544 | 1.373 | 2.000 | 0.000 |
| independent-negation-10 | 55.429 | 5.655 | 9.802 | n/a | 47.228 | 2.889 | 0.000 | 1.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 13.7 | 23.6 | n/a |
| n-queens/variant-04 8→11 | 11.2 | 11.6 | n/a |
| independent-negation-aggregate-16 | 10.3 | 10.4 | n/a |
| n-queens/variant-01 8→11 | 12.2 | 11.5 | n/a |
| send-money/send-money | 12.1 | 12.8 | n/a |
| latin-square-5 | 10.7 | 10.5 | n/a |
| disjunction-12 | 10.2 | 10.3 | n/a |
| stratified-16 | 8.4 | 10.5 | n/a |
| n-queens/variant-01 8→10 | 11.3 | 10.7 | n/a |
| ties-50 | 11.5 | 10.4 | n/a |
| independent-choice-12 | 8.8 | 10.4 | n/a |
| independent-choice-16 | 8.9 | 10.3 | n/a |
| transitive-path-100 | 10.9 | 11.2 | n/a |
| chain-arithmetic-1000 | 11.1 | 10.6 | n/a |
| transitive-path-200 | 17.4 | 13.5 | n/a |
| planning-14 | 10.5 | 10.2 | n/a |
| chain-1000 | 14.4 | 10.9 | n/a |
| producer-chain-700 | 24.3 | 12.5 | n/a |
| chain-2000 | 20.7 | 11.1 | n/a |
| independent-negation-8 | 8.4 | 10.2 | n/a |
| transitive-dense-40 | 18.2 | 10.7 | n/a |
| independent-negation-10 | 9.0 | 10.3 | n/a |

Against the reference: report after, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 7 of 22 cells where both passed (31.8%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 44.519 | 151.064 | 0.295 | 3.255 | 85.532 | 13.748 | 87.000 | 58.000 |
| n-queens/variant-04 8→11 | 101.219 | 180.406 | 0.561 | 1.453 | 333.665 | 18.784 | 2.000 | 174.000 |
| independent-negation-aggregate-16 | 20.610 | 31.837 | 0.647 | 0.232 | 8.312 | 3.914 | 0.000 | 27.000 |
| n-queens/variant-01 8→11 | 121.428 | 184.458 | 0.658 | 5.719 | 388.283 | 28.387 | 2.000 | 177.000 |
| stratified-16 | 4.528 | 5.514 | 0.821 | n/a | 0.483 | 0.242 | 0.000 | 0.000 |
| send-money/send-money | 13.339 | 15.123 | 0.882 | 2.567 | 11.656 | 0.255 | 9.000 | 1.000 |
| disjunction-12 | 21.822 | 23.109 | 0.944 | 0.129 | 6.742 | 30.560 | 1.000 | 18.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| latin-square-5 | 16.209 | 15.714 | 1.032 | 1.232 | 23.639 | 10.274 | 1.000 | 10.000 |
| n-queens/variant-01 8→10 | 34.011 | 32.198 | 1.056 | 4.648 | 81.204 | 6.369 | 2.000 | 25.000 |
| ties-50 | 21.136 | 19.737 | 1.071 | 0.344 | 19.587 | 3.127 | 1.000 | 14.000 |
| independent-choice-12 | 8.955 | 7.020 | 1.276 | n/a | 0.545 | 3.400 | 1.000 | 2.000 |
| independent-choice-16 | 39.871 | 30.114 | 1.324 | n/a | 2.908 | 23.886 | 0.000 | 24.000 |
| transitive-path-100 | 14.862 | 8.188 | 1.815 | n/a | 0.009 | 7.833 | 4.000 | 0.000 |
| chain-arithmetic-1000 | 9.975 | 5.448 | 1.831 | 3.737 | 0.616 | 0.153 | 1.000 | 0.000 |
| transitive-path-200 | 44.431 | 23.803 | 1.867 | n/a | 0.011 | 29.282 | 14.000 | 4.000 |
| planning-14 | 37.982 | 16.404 | 2.315 | 3.031 | 72.735 | 18.432 | 1.000 | 10.000 |
| producer-chain-700 | 27.979 | 10.896 | 2.568 | n/a | 0.009 | 2.909 | 6.000 | 0.000 |
| chain-1000 | 21.740 | 7.738 | 2.810 | n/a | 0.008 | 4.885 | 3.000 | 0.000 |
| chain-2000 | 36.777 | 10.860 | 3.386 | n/a | 0.006 | 6.101 | 6.000 | 0.000 |
| independent-negation-8 | 17.949 | 4.674 | 3.840 | n/a | 13.000 | 1.058 | 0.000 | 0.000 |
| transitive-dense-40 | 29.910 | 6.836 | 4.375 | 16.163 | 4.752 | 1.328 | 2.000 | 0.000 |
| independent-negation-10 | 51.620 | 5.595 | 9.226 | n/a | 44.242 | 2.833 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 13.4 | 23.7 | n/a |
| n-queens/variant-04 8→11 | 11.3 | 12.0 | n/a |
| independent-negation-aggregate-16 | 10.4 | 10.5 | n/a |
| n-queens/variant-01 8→11 | 11.9 | 11.5 | n/a |
| stratified-16 | 8.6 | 10.3 | n/a |
| send-money/send-money | 12.4 | 12.8 | n/a |
| disjunction-12 | 10.1 | 10.3 | n/a |
| latin-square-5 | 10.5 | 10.6 | n/a |
| n-queens/variant-01 8→10 | 11.5 | 10.7 | n/a |
| ties-50 | 11.8 | 10.5 | n/a |
| independent-choice-12 | 8.8 | 10.4 | n/a |
| independent-choice-16 | 9.0 | 10.3 | n/a |
| transitive-path-100 | 10.8 | 10.9 | n/a |
| chain-arithmetic-1000 | 11.1 | 10.5 | n/a |
| transitive-path-200 | 17.4 | 13.6 | n/a |
| planning-14 | 10.6 | 10.6 | n/a |
| producer-chain-700 | 24.5 | 12.6 | n/a |
| chain-1000 | 14.6 | 10.7 | n/a |
| chain-2000 | 20.7 | 11.1 | n/a |
| independent-negation-8 | 8.7 | 10.4 | n/a |
| transitive-dense-40 | 18.3 | 10.6 | n/a |
| independent-negation-10 | 9.0 | 10.3 | n/a |

Against the reference: report clauses, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search clauses): faster on 4 of 22 cells where both passed (18.1%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 94.605 | 160.060 | 0.591 | 3.396 | 45.673 | 23.637 | 92.000 | 62.000 |
| send-money/send-money | 10.935 | 16.262 | 0.672 | 2.641 | 1.798 | 0.249 | 10.000 | 1.000 |
| n-queens/variant-04 8→11 | 164.753 | 185.830 | 0.887 | 1.505 | 114.674 | 32.742 | 1.000 | 178.000 |
| independent-negation-aggregate-16 | 27.402 | 30.408 | 0.901 | 0.227 | 2.593 | 6.485 | 0.000 | 26.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stratified-16 | 4.542 | 4.513 | 1.006 | n/a | 0.493 | 0.262 | 0.000 | 0.000 |
| independent-choice-12 | 9.145 | 6.926 | 1.320 | n/a | 0.599 | 3.366 | 1.000 | 2.000 |
| n-queens/variant-01 8→11 | 273.827 | 189.913 | 1.442 | 6.306 | 199.149 | 51.703 | 2.000 | 182.000 |
| independent-choice-16 | 39.178 | 26.794 | 1.462 | n/a | 2.986 | 22.544 | 0.000 | 22.000 |
| ties-50 | 27.483 | 18.404 | 1.493 | 0.360 | 7.201 | 4.821 | 1.000 | 13.000 |
| transitive-path-100 | 16.025 | 9.137 | 1.754 | n/a | 0.008 | 8.189 | 3.000 | 1.000 |
| n-queens/variant-01 8→10 | 62.700 | 32.548 | 1.926 | 4.090 | 38.578 | 11.743 | 2.000 | 25.000 |
| transitive-path-200 | 48.521 | 22.760 | 2.132 | n/a | 0.008 | 31.365 | 13.000 | 4.000 |
| disjunction-12 | 45.359 | 20.971 | 2.163 | 0.133 | 3.505 | 1.970 | 0.000 | 16.000 |
| chain-arithmetic-1000 | 9.985 | 4.538 | 2.200 | 3.749 | 0.637 | 0.123 | 1.000 | 0.000 |
| latin-square-5 | 36.668 | 15.992 | 2.293 | 1.307 | 7.532 | 17.641 | 1.000 | 11.000 |
| producer-chain-700 | 29.130 | 11.943 | 2.439 | n/a | 0.009 | 2.579 | 6.000 | 0.000 |
| chain-1000 | 21.672 | 7.751 | 2.796 | n/a | 0.009 | 3.529 | 4.000 | 0.000 |
| chain-2000 | 39.020 | 11.136 | 3.504 | n/a | 0.019 | 7.289 | 6.000 | 0.000 |
| independent-negation-8 | 18.634 | 4.563 | 4.084 | n/a | 13.663 | 1.051 | 1.000 | 0.000 |
| planning-14 | 86.456 | 16.900 | 5.116 | 3.070 | 25.446 | 29.231 | 1.000 | 11.000 |
| transitive-dense-40 | 35.038 | 6.693 | 5.235 | 18.170 | 10.179 | 1.069 | 2.000 | 0.000 |
| independent-negation-10 | 52.609 | 5.672 | 9.275 | n/a | 45.328 | 2.771 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 13.3 | 23.6 | n/a |
| send-money/send-money | 12.5 | 12.7 | n/a |
| n-queens/variant-04 8→11 | 12.4 | 11.8 | n/a |
| independent-negation-aggregate-16 | 10.5 | 10.3 | n/a |
| stratified-16 | 8.5 | 10.4 | n/a |
| independent-choice-12 | 9.0 | 10.2 | n/a |
| n-queens/variant-01 8→11 | 12.8 | 11.6 | n/a |
| independent-choice-16 | 8.9 | 10.4 | n/a |
| ties-50 | 11.0 | 10.6 | n/a |
| transitive-path-100 | 10.9 | 11.1 | n/a |
| n-queens/variant-01 8→10 | 10.9 | 10.5 | n/a |
| transitive-path-200 | 17.4 | 13.5 | n/a |
| disjunction-12 | 10.6 | 10.2 | n/a |
| chain-arithmetic-1000 | 11.0 | 10.5 | n/a |
| latin-square-5 | 11.1 | 10.6 | n/a |
| producer-chain-700 | 24.7 | 12.7 | n/a |
| chain-1000 | 14.4 | 10.7 | n/a |
| chain-2000 | 20.5 | 10.9 | n/a |
| independent-negation-8 | 8.6 | 10.3 | n/a |
| planning-14 | 13.2 | 10.4 | n/a |
| transitive-dense-40 | 22.2 | 10.6 | n/a |
| independent-negation-10 | 8.7 | 10.2 | n/a |
