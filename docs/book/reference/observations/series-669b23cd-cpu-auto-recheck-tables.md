Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | before | after | before2 | after2 | after/before | after2/before | after2/before2 | before2/after | before/reference | after/reference | before2/reference | after2/reference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 6.831 [6.550, 7.737] | 6.791 [6.632, 6.793] | 7.021 [6.804, 9.396] | 6.814 [6.682, 7.992] | 0.994 | 0.998 | 0.970 | 1.034 | 0.994 | 0.954 | 1.012 | 1.101 |
| independent-choice-16 | 26.167 [25.557, 27.296] | 25.760 [25.377, 26.716] | 25.703 [25.136, 27.339] | 27.104 [26.939, 27.122] | 0.984 | 1.036 | 1.054 | 0.998 | 1.039 | 0.916 | 0.929 | 0.981 |
| independent-negation-8 | 9.987 [9.811, 10.791] | 9.921 [8.762, 11.607] | 9.832 [9.049, 10.873] | 9.158 [9.148, 9.762] | 0.993 | 0.917 | 0.931 | 0.991 | 1.806 | 2.081 | 1.759 | 1.977 |
| independent-negation-10 | 25.777 [23.507, 25.813] | 23.580 [22.411, 24.466] | 23.593 [22.527, 24.848] | 24.681 [23.653, 24.858] | 0.915 | 0.957 | 1.046 | 1.001 | 5.752 | 5.237 | 5.242 | 5.260 |
| independent-negation-aggregate-16 | 22.190 [21.037, 23.152] | 23.132 [22.752, 25.416] | 22.016 [20.915, 22.692] | 22.939 [22.783, 25.354] | 1.042 | 1.034 | 1.042 | 0.952 | 0.747 | 0.720 | 0.750 | 0.721 |
| disjunction-12 | 22.954 [22.923, 24.537] | 24.542 [23.875, 26.822] | 23.415 [23.058, 25.053] | 25.114 [24.344, 25.860] | 1.069 | 1.094 | 1.073 | 0.954 | 1.128 | 1.184 | 1.158 | 1.136 |
| ties-50 | 21.567 [20.590, 21.921] | 23.205 [22.259, 25.023] | 24.323 [21.774, 25.513] | 22.210 [21.948, 22.941] | 1.076 | 1.030 | 0.913 | 1.048 | 1.107 | 1.226 | 1.362 | 1.215 |
| transitive-path-100 | 9.402 [9.196, 10.296] | 10.236 [9.170, 10.933] | 9.302 [9.188, 10.160] | 9.166 [8.416, 9.588] | 1.089 | 0.975 | 0.985 | 0.909 | 1.064 | 1.271 | 1.060 | 1.025 |
| transitive-path-200 | 21.044 [20.840, 21.991] | 19.813 [19.686, 20.840] | 23.116 [22.419, 24.051] | 19.978 [19.763, 24.939] | 0.941 | 0.949 | 0.864 | 1.167 | 0.926 | 0.914 | 1.018 | 0.832 |
| transitive-dense-40 | 33.292 [31.138, 39.278] | 36.573 [34.383, 38.864] | 34.559 [32.338, 36.004] | 36.543 [36.244, 37.262] | 1.099 | 1.098 | 1.057 | 0.945 | 4.991 | 5.313 | 5.142 | 5.503 |
| chain-1000 | 19.516 [18.430, 21.965] | 16.337 [16.224, 16.997] | 19.568 [18.552, 20.978] | 15.201 [14.184, 15.206] | 0.837 | 0.779 | 0.777 | 1.198 | 2.942 | 2.091 | 2.938 | 1.972 |
| chain-2000 | 33.447 [32.396, 34.595] | 29.096 [28.189, 33.569] | 32.371 [32.272, 33.538] | 29.181 [27.005, 30.453] | 0.870 | 0.872 | 0.901 | 1.113 | 3.060 | 2.680 | 3.292 | 2.347 |
| chain-arithmetic-1000 | 10.388 [10.026, 10.471] | 9.873 [9.117, 9.969] | 11.087 [9.975, 12.000] | 10.021 [9.888, 10.270] | 0.950 | 0.965 | 0.904 | 1.123 | 2.282 | 2.214 | 2.498 | 1.706 |
| stratified-16 | 3.464 [3.396, 3.513] | 3.475 [3.394, 5.930] | 4.565 [4.549, 6.742] | 3.445 [3.436, 4.477] | 1.003 | 0.994 | 0.755 | 1.313 | 0.789 | 0.769 | 1.003 | 0.768 |
| producer-chain-700 | 26.734 [25.728, 29.175] | 22.651 [21.408, 27.896] | 26.782 [25.715, 26.851] | 21.627 [21.446, 22.537] | 0.847 | 0.809 | 0.808 | 1.182 | 2.732 | 2.082 | 2.735 | 2.219 |
| latin-square-5 | 14.998 [13.540, 16.195] | 14.634 [14.579, 15.445] | 13.319 [13.258, 13.589] | 15.344 [14.633, 16.827] | 0.976 | 1.023 | 1.152 | 0.910 | 0.838 | 0.927 | 0.879 | 0.894 |
| planning-14 | 37.929 [36.447, 38.399] | 44.051 [42.050, 44.215] | 39.285 [38.610, 40.064] | 42.809 [42.274, 43.226] | 1.161 | 1.129 | 1.090 | 0.892 | 2.384 | 2.747 | 2.317 | 2.557 |
| n-queens/variant-01 8→10 | 27.203 [27.153, 27.718] | 29.173 [27.899, 29.569] | 27.907 [25.836, 29.419] | 27.280 [27.110, 27.975] | 1.072 | 1.003 | 0.978 | 0.957 | 0.863 | 0.970 | 0.862 | 0.874 |
| n-queens/variant-01 8→11 | 88.575 [87.685, 90.895] | 91.929 [90.843, 92.329] | 87.014 [86.988, 88.747] | 94.252 [92.601, 96.935] | 1.038 | 1.064 | 1.083 | 0.947 | 0.487 | 0.504 | 0.479 | 0.512 |
| n-queens/variant-04 8→11 | 78.452 [77.136, 80.870] | 83.420 [81.111, 84.368] | 79.091 [78.952, 82.251] | 85.047 [84.905, 86.370] | 1.063 | 1.084 | 1.075 | 0.948 | 0.438 | 0.475 | 0.441 | 0.477 |
| send-money/send-money | 13.639 [12.086, 14.206] | 13.616 [13.274, 15.350] | 13.209 [13.007, 16.158] | 13.334 [12.449, 16.165] | 0.998 | 0.978 | 1.009 | 0.970 | 0.788 | 0.969 | 0.917 | 0.878 |
| variant-04/05-larger-mix | 35.199 [32.175, 35.901] | 34.155 [33.357, 37.395] | 34.058 [33.496, 34.126] | 34.673 [33.403, 35.471] | 0.970 | 0.985 | 1.018 | 0.997 | 0.233 | 0.232 | 0.229 | 0.228 |

Reference wall time, ms, same notation.

| Cell | before | after | before2 | after2 |
|---|---:|---:|---:|---:|
| independent-choice-12 | 6.875 [6.752, 9.052] | 7.118 [7.014, 9.117] | 6.941 [6.782, 8.218] | 6.190 [5.929, 6.784] |
| independent-choice-16 | 25.172 [24.862, 27.596] | 28.118 [25.461, 28.712] | 27.668 [26.122, 28.108] | 27.635 [27.063, 27.973] |
| independent-negation-8 | 5.529 [4.428, 5.580] | 4.768 [4.516, 5.497] | 5.588 [4.619, 6.660] | 4.633 [4.447, 4.944] |
| independent-negation-10 | 4.482 [4.466, 5.624] | 4.503 [4.460, 5.548] | 4.501 [4.476, 5.566] | 4.692 [4.459, 5.582] |
| independent-negation-aggregate-16 | 29.704 [29.042, 31.648] | 32.128 [30.129, 32.208] | 29.350 [29.039, 31.209] | 31.836 [30.989, 31.938] |
| disjunction-12 | 20.356 [20.259, 20.621] | 20.729 [20.269, 22.291] | 20.225 [19.465, 20.852] | 22.114 [21.034, 22.250] |
| ties-50 | 19.491 [18.207, 20.471] | 18.924 [18.795, 19.986] | 17.858 [17.299, 17.870] | 18.283 [18.167, 19.407] |
| transitive-path-100 | 8.836 [8.035, 9.267] | 8.055 [8.036, 10.039] | 8.779 [7.774, 8.816] | 8.942 [8.912, 9.135] |
| transitive-path-200 | 22.721 [22.328, 26.137] | 21.682 [20.469, 23.161] | 22.697 [22.431, 22.726] | 24.016 [23.907, 24.321] |
| transitive-dense-40 | 6.671 [6.637, 6.677] | 6.883 [6.696, 7.773] | 6.721 [6.643, 9.348] | 6.641 [6.591, 6.687] |
| chain-1000 | 6.633 [6.622, 7.717] | 7.811 [7.688, 9.909] | 6.659 [6.624, 7.599] | 7.708 [7.673, 8.034] |
| chain-2000 | 10.932 [9.921, 13.372] | 10.858 [9.925, 10.919] | 9.833 [9.790, 9.920] | 12.436 [10.044, 13.105] |
| chain-arithmetic-1000 | 4.552 [4.535, 5.528] | 4.459 [4.423, 5.557] | 4.438 [4.429, 4.467] | 5.876 [4.484, 5.965] |
| stratified-16 | 4.390 [3.510, 4.413] | 4.518 [4.415, 5.546] | 4.553 [3.381, 5.508] | 4.485 [4.404, 5.562] |
| producer-chain-700 | 9.785 [9.752, 11.002] | 10.880 [9.730, 13.551] | 9.794 [9.722, 10.808] | 9.745 [8.665, 9.813] |
| latin-square-5 | 17.906 [16.861, 18.234] | 15.779 [14.536, 16.708] | 15.161 [14.472, 15.638] | 17.167 [15.595, 17.664] |
| planning-14 | 15.908 [15.684, 16.912] | 16.036 [15.708, 16.935] | 16.954 [14.927, 19.042] | 16.740 [14.820, 16.764] |
| n-queens/variant-01 8→10 | 31.534 [31.046, 33.316] | 30.066 [29.974, 31.166] | 32.362 [31.162, 32.916] | 31.196 [30.200, 33.295] |
| n-queens/variant-01 8→11 | 181.752 [179.274, 185.607] | 182.457 [180.329, 186.459] | 181.735 [181.116, 183.532] | 183.969 [181.826, 184.557] |
| n-queens/variant-04 8→11 | 179.235 [178.586, 181.630] | 175.588 [173.672, 177.861] | 179.345 [179.249, 181.714] | 178.422 [176.836, 178.541] |
| send-money/send-money | 17.302 [15.125, 17.305] | 14.056 [13.391, 16.219] | 14.402 [14.023, 14.448] | 15.194 [14.321, 15.241] |
| variant-04/05-larger-mix | 150.931 [148.619, 154.884] | 147.220 [146.992, 149.556] | 148.738 [146.985, 153.580] | 151.870 [149.086, 151.947] |

Counters of report after2: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 227172 | 4.216 |
| independent-choice-16 | 0 | 2584 | 2584 | 2117111 | 24.079 |
| independent-negation-8 | 0 | 55 | 55 | 28784 | 6.446 |
| independent-negation-10 | 0 | 144 | 144 | 91687 | 21.704 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 170945 | 19.172 |
| disjunction-12 | 0 | 4096 | 4096 | 2029095 | 21.203 |
| ties-50 | 0 | 1225 | 1225 | 905448 | 18.467 |
| transitive-path-100 | 0 | 1 | 1 | 111564 | 6.104 |
| transitive-path-200 | 0 | 1 | 1 | 664776 | 16.619 |
| transitive-dense-40 | 0 | 1 | 1 | 770070 | 31.671 |
| chain-1000 | 0 | 1 | 1 | 211573 | 10.721 |
| chain-2000 | 0 | 1 | 1 | 736073 | 22.312 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 82123 | 6.362 |
| stratified-16 | 0 | 1 | 1 | 1669 | 1.004 |
| producer-chain-700 | 0 | 1 | 1 | 48256 | 16.310 |
| latin-square-5 | 0 | 1344 | 1344 | 791445 | 11.705 |
| planning-14 | 0 | 3432 | 3432 | 2522390 | 38.950 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 3452280 | 23.872 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 16394772 | 90.353 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 15197393 | 80.926 |
| send-money/send-money | 0 | 1 | 1 | 409024 | 9.081 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 2183876 | 31.016 |

Against the reference: report before, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 10 of 22 cells where both passed (45.4%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 35.199 | 150.931 | 0.233 | 3.286 | 33.513 | 12.439 | 85.000 | 60.000 |
| n-queens/variant-04 8→11 | 78.452 | 179.235 | 0.438 | 1.534 | 223.241 | 17.877 | 1.000 | 173.000 |
| n-queens/variant-01 8→11 | 88.575 | 181.752 | 0.487 | 6.184 | 224.458 | 28.715 | 2.000 | 175.000 |
| independent-negation-aggregate-16 | 22.190 | 29.704 | 0.747 | 0.468 | 4.080 | 4.136 | 1.000 | 25.000 |
| send-money/send-money | 13.639 | 17.302 | 0.788 | 2.522 | 6.115 | 0.284 | 9.000 | 1.000 |
| stratified-16 | 3.464 | 4.390 | 0.789 | n/a | 0.230 | 0.156 | 0.000 | 0.000 |
| latin-square-5 | 14.998 | 17.906 | 0.838 | 1.821 | 12.661 | 10.332 | 1.000 | 10.000 |
| n-queens/variant-01 8→10 | 27.203 | 31.534 | 0.863 | 3.941 | 48.446 | 6.362 | 2.000 | 25.000 |
| transitive-path-200 | 21.044 | 22.721 | 0.926 | n/a | 0.007 | 6.556 | 13.000 | 4.000 |
| independent-choice-12 | 6.831 | 6.875 | 0.994 | n/a | 0.421 | 1.973 | 1.000 | 2.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-16 | 26.167 | 25.172 | 1.039 | n/a | 2.671 | 11.956 | 0.000 | 20.000 |
| transitive-path-100 | 9.402 | 8.836 | 1.064 | n/a | 0.007 | 2.162 | 4.000 | 0.000 |
| ties-50 | 21.567 | 19.491 | 1.107 | 0.354 | 13.453 | 2.911 | 0.000 | 14.000 |
| disjunction-12 | 22.954 | 20.356 | 1.128 | 0.136 | 2.742 | 36.805 | 0.000 | 16.000 |
| independent-negation-8 | 9.987 | 5.529 | 1.806 | n/a | 5.469 | 0.493 | 0.000 | 0.000 |
| chain-arithmetic-1000 | 10.388 | 4.552 | 2.282 | 3.667 | 0.814 | 0.155 | 1.000 | 0.000 |
| planning-14 | 37.929 | 15.908 | 2.384 | 3.067 | 43.325 | 18.862 | 1.000 | 10.000 |
| producer-chain-700 | 26.734 | 9.785 | 2.732 | n/a | 0.009 | 2.693 | 5.000 | 0.000 |
| chain-1000 | 19.516 | 6.633 | 2.942 | n/a | 0.009 | 2.171 | 3.000 | 0.000 |
| chain-2000 | 33.447 | 10.932 | 3.060 | n/a | 0.009 | 3.986 | 7.000 | 0.000 |
| transitive-dense-40 | 33.292 | 6.671 | 4.991 | 15.249 | 7.555 | 1.291 | 3.000 | 0.000 |
| independent-negation-10 | 25.777 | 4.482 | 5.752 | n/a | 20.209 | 1.272 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 15.9 | 23.5 | n/a |
| n-queens/variant-04 8→11 | 12.7 | 11.9 | n/a |
| n-queens/variant-01 8→11 | 14.1 | 11.8 | n/a |
| independent-negation-aggregate-16 | 10.5 | 10.2 | n/a |
| send-money/send-money | 14.2 | 12.4 | n/a |
| stratified-16 | 9.0 | 10.1 | n/a |
| latin-square-5 | 11.4 | 10.7 | n/a |
| n-queens/variant-01 8→10 | 12.9 | 10.7 | n/a |
| transitive-path-200 | 14.4 | 13.3 | n/a |
| independent-choice-12 | 8.7 | 10.4 | n/a |
| independent-choice-16 | 8.6 | 10.2 | n/a |
| transitive-path-100 | 9.9 | 11.1 | n/a |
| ties-50 | 12.4 | 10.3 | n/a |
| disjunction-12 | 10.4 | 10.3 | n/a |
| independent-negation-8 | 8.4 | 10.3 | n/a |
| chain-arithmetic-1000 | 11.6 | 10.5 | n/a |
| planning-14 | 11.4 | 10.4 | n/a |
| producer-chain-700 | 24.0 | 12.7 | n/a |
| chain-1000 | 14.4 | 10.8 | n/a |
| chain-2000 | 21.3 | 11.0 | n/a |
| transitive-dense-40 | 21.8 | 10.6 | n/a |
| independent-negation-10 | 8.6 | 10.1 | n/a |

Against the reference: report after, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 11 of 22 cells where both passed (50.0%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 34.155 | 147.220 | 0.232 | 3.335 | 32.822 | 12.621 | 85.000 | 59.000 |
| n-queens/variant-04 8→11 | 83.420 | 175.588 | 0.475 | 1.498 | 230.856 | 18.063 | 2.000 | 170.000 |
| n-queens/variant-01 8→11 | 91.929 | 182.457 | 0.504 | 5.688 | 223.365 | 27.131 | 2.000 | 175.000 |
| independent-negation-aggregate-16 | 23.132 | 32.128 | 0.720 | 0.223 | 4.024 | 4.244 | 0.000 | 27.000 |
| stratified-16 | 3.475 | 4.518 | 0.769 | n/a | 0.223 | 0.187 | 0.000 | 0.000 |
| transitive-path-200 | 19.813 | 21.682 | 0.914 | n/a | 0.007 | 6.671 | 13.000 | 4.000 |
| independent-choice-16 | 25.760 | 28.118 | 0.916 | n/a | 2.581 | 9.812 | 0.000 | 23.000 |
| latin-square-5 | 14.634 | 15.779 | 0.927 | 1.231 | 13.455 | 10.716 | 1.000 | 10.000 |
| independent-choice-12 | 6.791 | 7.118 | 0.954 | n/a | 0.405 | 2.058 | 1.000 | 2.000 |
| send-money/send-money | 13.616 | 14.056 | 0.969 | 2.727 | 6.507 | 0.300 | 9.000 | 1.000 |
| n-queens/variant-01 8→10 | 29.173 | 30.066 | 0.970 | 3.851 | 47.594 | 5.993 | 2.000 | 24.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| disjunction-12 | 24.542 | 20.729 | 1.184 | 0.139 | 2.899 | 37.620 | 0.000 | 16.000 |
| ties-50 | 23.205 | 18.924 | 1.226 | 0.350 | 13.383 | 2.821 | 1.000 | 14.000 |
| transitive-path-100 | 10.236 | 8.055 | 1.271 | n/a | 0.009 | 2.082 | 4.000 | 0.000 |
| independent-negation-8 | 9.921 | 4.768 | 2.081 | n/a | 5.420 | 0.541 | 0.000 | 0.000 |
| producer-chain-700 | 22.651 | 10.880 | 2.082 | n/a | 0.010 | 3.121 | 5.000 | 0.000 |
| chain-1000 | 16.337 | 7.811 | 2.091 | n/a | 0.005 | 2.123 | 4.000 | 0.000 |
| chain-arithmetic-1000 | 9.873 | 4.459 | 2.214 | 3.490 | 0.811 | 0.145 | 1.000 | 0.000 |
| chain-2000 | 29.096 | 10.858 | 2.680 | n/a | 0.008 | 4.376 | 7.000 | 0.000 |
| planning-14 | 44.051 | 16.036 | 2.747 | 3.882 | 41.371 | 17.885 | 1.000 | 11.000 |
| independent-negation-10 | 23.580 | 4.503 | 5.237 | n/a | 18.900 | 1.188 | 1.000 | 0.000 |
| transitive-dense-40 | 36.573 | 6.883 | 5.313 | 16.534 | 8.374 | 1.347 | 2.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 17.5 | 23.6 | n/a |
| n-queens/variant-04 8→11 | 13.8 | 11.8 | n/a |
| n-queens/variant-01 8→11 | 15.7 | 11.5 | n/a |
| independent-negation-aggregate-16 | 10.9 | 10.2 | n/a |
| stratified-16 | 8.5 | 10.3 | n/a |
| transitive-path-200 | 14.4 | 13.5 | n/a |
| independent-choice-16 | 8.5 | 10.2 | n/a |
| latin-square-5 | 11.8 | 10.5 | n/a |
| independent-choice-12 | 8.5 | 10.3 | n/a |
| send-money/send-money | 14.9 | 12.8 | n/a |
| n-queens/variant-01 8→10 | 14.0 | 10.9 | n/a |
| disjunction-12 | 10.4 | 10.2 | n/a |
| ties-50 | 13.0 | 10.4 | n/a |
| transitive-path-100 | 9.8 | 11.0 | n/a |
| independent-negation-8 | 8.4 | 10.3 | n/a |
| producer-chain-700 | 23.9 | 12.3 | n/a |
| chain-1000 | 14.1 | 10.6 | n/a |
| chain-arithmetic-1000 | 11.8 | 10.6 | n/a |
| chain-2000 | 21.0 | 11.2 | n/a |
| planning-14 | 11.3 | 10.2 | n/a |
| independent-negation-10 | 8.6 | 10.2 | n/a |
| transitive-dense-40 | 21.8 | 10.4 | n/a |

Against the reference: report before2, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 8 of 22 cells where both passed (36.3%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 34.058 | 148.738 | 0.229 | 3.208 | 32.601 | 12.426 | 85.000 | 57.000 |
| n-queens/variant-04 8→11 | 79.091 | 179.345 | 0.441 | 1.537 | 224.596 | 18.039 | 1.000 | 173.000 |
| n-queens/variant-01 8→11 | 87.014 | 181.735 | 0.479 | 5.702 | 223.145 | 28.134 | 2.000 | 175.000 |
| independent-negation-aggregate-16 | 22.016 | 29.350 | 0.750 | 0.245 | 4.092 | 4.195 | 0.000 | 26.000 |
| n-queens/variant-01 8→10 | 27.907 | 32.362 | 0.862 | 3.862 | 47.332 | 6.075 | 2.000 | 26.000 |
| latin-square-5 | 13.319 | 15.161 | 0.879 | 1.212 | 12.197 | 9.945 | 1.000 | 10.000 |
| send-money/send-money | 13.209 | 14.402 | 0.917 | 2.578 | 5.785 | 0.290 | 9.000 | 1.000 |
| independent-choice-16 | 25.703 | 27.668 | 0.929 | n/a | 2.789 | 11.561 | 0.000 | 23.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stratified-16 | 4.565 | 4.553 | 1.003 | n/a | 0.225 | 0.136 | 0.000 | 0.000 |
| independent-choice-12 | 7.021 | 6.941 | 1.012 | n/a | 0.581 | 1.935 | 0.000 | 2.000 |
| transitive-path-200 | 23.116 | 22.697 | 1.018 | n/a | 0.009 | 8.430 | 13.000 | 4.000 |
| transitive-path-100 | 9.302 | 8.779 | 1.060 | n/a | 0.006 | 2.483 | 3.000 | 1.000 |
| disjunction-12 | 23.415 | 20.225 | 1.158 | 0.130 | 2.814 | 37.841 | 0.000 | 16.000 |
| ties-50 | 24.323 | 17.858 | 1.362 | 0.345 | 15.010 | 3.226 | 0.000 | 13.000 |
| independent-negation-8 | 9.832 | 5.588 | 1.759 | n/a | 5.904 | 0.578 | 0.000 | 0.000 |
| planning-14 | 39.285 | 16.954 | 2.317 | 3.121 | 43.818 | 19.522 | 1.000 | 10.000 |
| chain-arithmetic-1000 | 11.087 | 4.438 | 2.498 | 4.645 | 0.773 | 0.150 | 1.000 | 0.000 |
| producer-chain-700 | 26.782 | 9.794 | 2.735 | n/a | 0.008 | 3.247 | 6.000 | 0.000 |
| chain-1000 | 19.568 | 6.659 | 2.938 | n/a | 0.008 | 2.213 | 3.000 | 0.000 |
| chain-2000 | 32.371 | 9.833 | 3.292 | n/a | 0.009 | 3.972 | 6.000 | 0.000 |
| transitive-dense-40 | 34.559 | 6.721 | 5.142 | 15.327 | 8.337 | 1.289 | 2.000 | 0.000 |
| independent-negation-10 | 23.593 | 4.501 | 5.242 | n/a | 19.233 | 1.242 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 15.8 | 23.6 | n/a |
| n-queens/variant-04 8→11 | 13.0 | 11.7 | n/a |
| n-queens/variant-01 8→11 | 14.0 | 11.9 | n/a |
| independent-negation-aggregate-16 | 10.7 | 10.3 | n/a |
| n-queens/variant-01 8→10 | 12.9 | 10.6 | n/a |
| latin-square-5 | 11.3 | 10.7 | n/a |
| send-money/send-money | 14.3 | 12.8 | n/a |
| independent-choice-16 | 8.5 | 10.2 | n/a |
| stratified-16 | 8.8 | 10.5 | n/a |
| independent-choice-12 | 8.4 | 10.3 | n/a |
| transitive-path-200 | 14.4 | 13.5 | n/a |
| transitive-path-100 | 10.1 | 10.8 | n/a |
| disjunction-12 | 10.2 | 10.2 | n/a |
| ties-50 | 12.5 | 10.4 | n/a |
| independent-negation-8 | 8.6 | 10.3 | n/a |
| planning-14 | 11.1 | 10.5 | n/a |
| chain-arithmetic-1000 | 11.4 | 10.6 | n/a |
| producer-chain-700 | 24.2 | 12.7 | n/a |
| chain-1000 | 14.5 | 10.8 | n/a |
| chain-2000 | 21.4 | 10.9 | n/a |
| transitive-dense-40 | 21.8 | 10.7 | n/a |
| independent-negation-10 | 8.5 | 10.3 | n/a |

Against the reference: report after2, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 10 of 22 cells where both passed (45.4%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 34.673 | 151.870 | 0.228 | 3.329 | 32.756 | 12.673 | 86.000 | 60.000 |
| n-queens/variant-04 8→11 | 85.047 | 178.422 | 0.477 | 1.514 | 239.825 | 18.568 | 1.000 | 172.000 |
| n-queens/variant-01 8→11 | 94.252 | 183.969 | 0.512 | 5.651 | 233.907 | 28.518 | 2.000 | 178.000 |
| independent-negation-aggregate-16 | 22.939 | 31.836 | 0.721 | 0.228 | 3.976 | 4.108 | 0.000 | 27.000 |
| stratified-16 | 3.445 | 4.485 | 0.768 | n/a | 0.222 | 0.142 | 0.000 | 0.000 |
| transitive-path-200 | 19.978 | 24.016 | 0.832 | n/a | 0.007 | 6.592 | 15.000 | 4.000 |
| n-queens/variant-01 8→10 | 27.280 | 31.196 | 0.874 | 3.828 | 49.901 | 6.297 | 2.000 | 26.000 |
| send-money/send-money | 13.334 | 15.194 | 0.878 | 2.674 | 6.608 | 0.287 | 9.000 | 1.000 |
| latin-square-5 | 15.344 | 17.167 | 0.894 | 1.241 | 12.484 | 9.920 | 1.000 | 12.000 |
| independent-choice-16 | 27.104 | 27.635 | 0.981 | n/a | 2.553 | 11.283 | 0.000 | 23.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| transitive-path-100 | 9.166 | 8.942 | 1.025 | n/a | 0.010 | 2.133 | 3.000 | 1.000 |
| independent-choice-12 | 6.814 | 6.190 | 1.101 | n/a | 0.401 | 2.110 | 0.000 | 2.000 |
| disjunction-12 | 25.114 | 22.114 | 1.136 | 0.143 | 2.846 | 36.448 | 0.000 | 17.000 |
| ties-50 | 22.210 | 18.283 | 1.215 | 0.342 | 13.227 | 2.819 | 1.000 | 13.000 |
| chain-arithmetic-1000 | 10.021 | 5.876 | 1.706 | 3.597 | 0.814 | 0.147 | 1.000 | 0.000 |
| chain-1000 | 15.201 | 7.708 | 1.972 | n/a | 0.005 | 2.039 | 3.000 | 0.000 |
| independent-negation-8 | 9.158 | 4.633 | 1.977 | n/a | 5.292 | 0.488 | 0.000 | 0.000 |
| producer-chain-700 | 21.627 | 9.745 | 2.219 | n/a | 0.006 | 3.306 | 5.000 | 0.000 |
| chain-2000 | 29.181 | 12.436 | 2.347 | n/a | 0.006 | 4.410 | 7.000 | 0.000 |
| planning-14 | 42.809 | 16.740 | 2.557 | 3.063 | 40.882 | 17.825 | 1.000 | 11.000 |
| independent-negation-10 | 24.681 | 4.692 | 5.260 | n/a | 18.685 | 1.400 | 1.000 | 0.000 |
| transitive-dense-40 | 36.543 | 6.641 | 5.503 | 18.965 | 7.466 | 1.293 | 2.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 17.6 | 23.6 | n/a |
| n-queens/variant-04 8→11 | 13.7 | 11.7 | n/a |
| n-queens/variant-01 8→11 | 15.6 | 11.5 | n/a |
| independent-negation-aggregate-16 | 11.0 | 10.4 | n/a |
| stratified-16 | 8.7 | 10.2 | n/a |
| transitive-path-200 | 14.0 | 13.3 | n/a |
| n-queens/variant-01 8→10 | 13.6 | 10.6 | n/a |
| send-money/send-money | 15.0 | 12.7 | n/a |
| latin-square-5 | 12.2 | 10.4 | n/a |
| independent-choice-16 | 8.7 | 10.3 | n/a |
| transitive-path-100 | 9.7 | 11.0 | n/a |
| independent-choice-12 | 8.6 | 10.3 | n/a |
| disjunction-12 | 10.1 | 10.3 | n/a |
| ties-50 | 12.9 | 10.6 | n/a |
| chain-arithmetic-1000 | 11.3 | 10.5 | n/a |
| chain-1000 | 14.2 | 10.8 | n/a |
| independent-negation-8 | 8.4 | 10.3 | n/a |
| producer-chain-700 | 23.9 | 12.6 | n/a |
| chain-2000 | 21.0 | 11.2 | n/a |
| planning-14 | 11.4 | 10.4 | n/a |
| independent-negation-10 | 8.6 | 10.4 | n/a |
| transitive-dense-40 | 21.8 | 10.7 | n/a |
