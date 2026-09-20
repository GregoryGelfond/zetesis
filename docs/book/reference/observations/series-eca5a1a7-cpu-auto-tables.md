Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | A1-main | B1-candidate | B2-candidate | A2-main | A2-main/A1-main | A2-main/B2-candidate | B1-candidate/A1-main | B2-candidate/B1-candidate | A1-main/reference | B1-candidate/reference | B2-candidate/reference | A2-main/reference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 26.710 [26.627, 26.792] | 7.882 [7.819, 7.946] | 7.887 [7.798, 7.976] | 26.671 [26.650, 26.692] | 0.999 | 3.382 | 0.295 | 1.001 | 3.369 | 1.004 | 1.003 | 3.361 |
| independent-choice-16 | 210.461 [209.894, 211.027] | 21.507 [21.430, 21.584] | 22.277 [21.457, 23.096] | 209.887 [209.760, 210.015] | 0.997 | 9.422 | 0.102 | 1.036 | 5.861 | 0.597 | 0.620 | 5.845 |
| independent-negation-8 | 105.949 [104.352, 107.546] | 9.361 [9.286, 9.435] | 9.340 [9.289, 9.390] | 107.392 [107.202, 107.582] | 1.014 | 11.499 | 0.088 | 0.998 | 22.355 | 1.993 | 1.991 | 22.709 |
| independent-negation-10 | 1307.576 [1303.296, 1311.856] | 17.365 [16.734, 17.996] | 18.284 [18.274, 18.294] | 1317.820 [1317.190, 1318.451] | 1.008 | 72.074 | 0.013 | 1.053 | 288.204 | 3.697 | 3.904 | 276.316 |
| independent-negation-aggregate-16 | 272.386 [272.315, 272.457] | 26.134 [26.096, 26.171] | 26.179 [26.044, 26.313] | 272.672 [272.468, 272.876] | 1.001 | 10.416 | 0.096 | 1.002 | 6.353 | 0.607 | 0.599 | 6.350 |
| disjunction-12 | 197.619 [197.494, 197.744] | 26.147 [26.128, 26.167] | 26.380 [26.290, 26.470] | 196.928 [196.596, 197.259] | 0.997 | 7.465 | 0.132 | 1.009 | 6.338 | 0.838 | 0.848 | 6.316 |
| ties-50 | 144.858 [143.843, 145.873] | 25.716 [25.340, 26.092] | 26.070 [25.465, 26.675] | 144.531 [144.001, 145.060] | 0.998 | 5.544 | 0.178 | 1.014 | 6.643 | 1.177 | 1.197 | 6.626 |
| transitive-path-100 | 23.903 [22.991, 24.816] | 9.553 [9.448, 9.658] | 9.550 [9.526, 9.574] | 24.639 [24.551, 24.726] | 1.031 | 2.580 | 0.400 | 1.000 | 3.081 | 1.235 | 1.234 | 3.178 |
| transitive-path-200 | 121.651 [121.518, 121.785] | 16.090 [16.078, 16.101] | 16.177 [16.155, 16.200] | 124.009 [123.090, 124.929] | 1.019 | 7.666 | 0.132 | 1.005 | 6.326 | 0.805 | 0.840 | 6.203 |
| transitive-dense-40 | 28.320 [27.665, 28.976] | 24.383 [24.315, 24.451] | 24.537 [24.508, 24.567] | 27.488 [27.466, 27.510] | 0.971 | 1.120 | 0.861 | 1.006 | 4.557 | 3.925 | 3.951 | 4.421 |
| chain-1000 | 65.828 [65.015, 66.642] | 15.466 [15.435, 15.498] | 16.233 [15.464, 17.002] | 66.613 [66.589, 66.636] | 1.012 | 4.103 | 0.235 | 1.050 | 10.681 | 2.234 | 2.341 | 9.590 |
| chain-2000 | 219.137 [218.375, 219.900] | 25.210 [24.449, 25.972] | 25.945 [25.914, 25.977] | 226.223 [226.216, 226.230] | 1.032 | 8.719 | 0.115 | 1.029 | 23.793 | 2.734 | 2.819 | 24.478 |
| chain-arithmetic-1000 | 9.419 [9.347, 9.491] | 10.089 [9.324, 10.854] | 10.115 [9.310, 10.920] | 9.426 [9.369, 9.483] | 1.001 | 0.932 | 1.071 | 1.003 | 2.097 | 2.167 | 2.180 | 2.013 |
| stratified-16 | blocked by timeout ×2 | 6.217 [6.189, 6.244] | 5.455 [4.683, 6.227] | blocked by timeout ×2 | n/a | n/a | n/a | 0.877 | n/a | 1.338 | 1.173 | n/a |
| producer-chain-700 | 15.284 [15.269, 15.299] | 13.807 [13.804, 13.809] | 13.780 [13.767, 13.793] | 16.627 [16.410, 16.844] | 1.088 | 1.207 | 0.903 | 0.998 | 1.659 | 1.503 | 1.501 | 1.674 |
| latin-square-5 | 160.031 [157.794, 162.268] | 14.225 [13.920, 14.530] | 13.920 [13.841, 13.999] | 161.113 [158.016, 164.211] | 1.007 | 11.574 | 0.089 | 0.979 | 8.214 | 0.762 | 0.747 | 8.597 |
| planning-14 | blocked by capture_limit ×2 | 35.248 [35.188, 35.308] | 35.186 [35.159, 35.213] | blocked by capture_limit ×2 | n/a | n/a | n/a | 0.998 | n/a | 1.913 | 1.993 | n/a |
| n-queens/variant-01 8→10 | 100.285 [99.547, 101.023] | 31.340 [29.950, 32.730] | 30.863 [29.770, 31.956] | 100.989 [100.949, 101.030] | 1.007 | 3.272 | 0.313 | 0.985 | 3.572 | 1.089 | 1.072 | 3.500 |
| n-queens/variant-01 8→11 | 395.742 [395.211, 396.274] | 117.708 [114.763, 120.652] | 104.945 [104.220, 105.670] | 400.552 [398.280, 402.824] | 1.012 | 3.817 | 0.297 | 0.892 | 2.196 | 0.637 | 0.570 | 2.178 |
| n-queens/variant-04 8→11 | 267.829 [266.026, 269.632] | 95.165 [95.164, 95.167] | 96.816 [95.511, 98.122] | 270.437 [266.728, 274.146] | 1.010 | 2.793 | 0.355 | 1.017 | 1.531 | 0.528 | 0.539 | 1.502 |
| send-money/send-money | 37.263 [36.420, 38.107] | 12.412 [12.365, 12.459] | 12.408 [12.366, 12.451] | 38.021 [37.954, 38.088] | 1.020 | 3.064 | 0.333 | 1.000 | 3.046 | 1.015 | 1.017 | 3.109 |
| variant-04/05-larger-mix | 360.531 [359.895, 361.168] | 34.958 [34.505, 35.411] | 35.276 [35.098, 35.454] | 361.474 [361.397, 361.550] | 1.003 | 10.247 | 0.097 | 1.009 | 1.959 | 0.181 | 0.198 | 2.023 |

Reference wall time, ms, same notation.

| Cell | A1-main | B1-candidate | B2-candidate | A2-main |
|---|---:|---:|---:|---:|
| independent-choice-12 | 7.927 [7.883, 7.972] | 7.850 [7.799, 7.901] | 7.864 [7.819, 7.909] | 7.936 [7.895, 7.977] |
| independent-choice-16 | 35.907 [35.792, 36.022] | 36.047 [35.983, 36.111] | 35.928 [35.870, 35.986] | 35.910 [35.832, 35.988] |
| independent-negation-8 | 4.739 [4.726, 4.753] | 4.698 [4.695, 4.701] | 4.691 [4.676, 4.705] | 4.729 [4.701, 4.757] |
| independent-negation-10 | 4.537 [4.253, 4.821] | 4.697 [4.684, 4.711] | 4.684 [4.671, 4.697] | 4.769 [4.706, 4.833] |
| independent-negation-aggregate-16 | 42.878 [42.179, 43.577] | 43.023 [42.232, 43.814] | 43.668 [43.545, 43.791] | 42.940 [42.220, 43.660] |
| disjunction-12 | 31.178 [31.116, 31.239] | 31.190 [31.084, 31.295] | 31.125 [31.070, 31.180] | 31.180 [31.154, 31.206] |
| ties-50 | 21.806 [21.704, 21.909] | 21.842 [21.825, 21.860] | 21.774 [21.720, 21.829] | 21.812 [21.739, 21.885] |
| transitive-path-100 | 7.758 [7.699, 7.817] | 7.737 [7.708, 7.766] | 7.740 [7.703, 7.777] | 7.753 [7.708, 7.797] |
| transitive-path-200 | 19.231 [18.399, 20.064] | 19.994 [19.892, 20.096] | 19.267 [18.384, 20.151] | 19.991 [19.900, 20.081] |
| transitive-dense-40 | 6.215 [6.167, 6.262] | 6.212 [6.168, 6.257] | 6.210 [6.179, 6.242] | 6.218 [6.197, 6.239] |
| chain-1000 | 6.163 [6.151, 6.176] | 6.924 [6.150, 7.698] | 6.933 [6.157, 7.709] | 6.946 [6.145, 7.746] |
| chain-2000 | 9.210 [9.180, 9.240] | 9.222 [9.182, 9.262] | 9.203 [9.185, 9.222] | 9.242 [9.193, 9.291] |
| chain-arithmetic-1000 | 4.491 [4.335, 4.647] | 4.655 [4.649, 4.662] | 4.641 [4.636, 4.645] | 4.683 [4.671, 4.695] |
| stratified-16 | 4.644 [4.639, 4.648] | 4.646 [4.643, 4.648] | 4.649 [4.642, 4.656] | 4.522 [4.384, 4.660] |
| producer-chain-700 | 9.214 [9.187, 9.240] | 9.187 [9.154, 9.219] | 9.181 [9.178, 9.183] | 9.933 [9.150, 10.716] |
| latin-square-5 | 19.482 [18.612, 20.352] | 18.665 [18.625, 18.706] | 18.629 [18.539, 18.719] | 18.740 [18.724, 18.756] |
| planning-14 | 18.463 [18.327, 18.598] | 18.425 [18.331, 18.519] | 17.659 [16.852, 18.465] | 18.430 [18.336, 18.525] |
| n-queens/variant-01 8→10 | 28.077 [27.257, 28.897] | 28.780 [28.745, 28.816] | 28.799 [28.792, 28.807] | 28.858 [28.802, 28.913] |
| n-queens/variant-01 8→11 | 180.195 [179.335, 181.055] | 184.726 [183.850, 185.601] | 183.983 [183.862, 184.104] | 183.916 [183.888, 183.944] |
| n-queens/variant-04 8→11 | 174.966 [174.904, 175.027] | 180.276 [179.466, 181.087] | 179.527 [178.037, 181.018] | 180.084 [179.597, 180.570] |
| send-money/send-money | 12.232 [12.210, 12.255] | 12.225 [12.220, 12.229] | 12.197 [12.189, 12.206] | 12.230 [12.219, 12.240] |
| variant-04/05-larger-mix | 184.002 [182.507, 185.498] | 192.852 [191.700, 194.004] | 177.964 [163.027, 192.901] | 178.678 [177.853, 179.504] |

Counters of report A2-main: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | n/a | 22.062 |
| independent-choice-16 | 0 | 2584 | 2584 | n/a | 205.367 |
| independent-negation-8 | 0 | 55 | 14080 | n/a | 102.943 |
| independent-negation-10 | 0 | 144 | 147456 | n/a | 1314.002 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 4956117 | 266.377 |
| disjunction-12 | 0 | 4096 | 4096 | 12273049 | 191.433 |
| ties-50 | 0 | 1225 | 1225 | 8053295 | 139.797 |
| transitive-path-100 | 0 | 1 | 1 | n/a | 20.060 |
| transitive-path-200 | 0 | 1 | 1 | n/a | 119.887 |
| transitive-dense-40 | 0 | 1 | 1 | 2345227 | 23.136 |
| chain-1000 | 0 | 1 | 1 | n/a | 61.902 |
| chain-2000 | 0 | 1 | 1 | n/a | 221.226 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 205173 | 4.980 |
| stratified-16 | 0 | blocked by timeout ×2 | | | |
| producer-chain-700 | 0 | 1 | 1 | n/a | 12.287 |
| latin-square-5 | 0 | 1344 | 1344 | 16443632 | 156.979 |
| planning-14 | 0 | blocked by capture_limit ×2 | | | |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 17317112 | 96.223 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 85071370 | 395.337 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 55533813 | 265.969 |
| send-money/send-money | 0 | 1 | 1 | 3629789 | 33.232 |
| variant-04/05-larger-mix | 0 | 1176 | 1208 | 44210323 | 355.721 |

Against the reference: report A1-main, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 0 of 20 cells where both passed (0.0%).

Wins, fastest first: none.

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-04 8→11 | 267.829 | 174.966 | 1.531 | 1.321 | 119.609 | 23.777 | 1.000 | 169.500 |
| producer-chain-700 | 15.284 | 9.214 | 1.659 | n/a | 0.004 | 1.442 | 5.000 | 0.000 |
| variant-04/05-larger-mix | 360.531 | 184.002 | 1.959 | 3.243 | 66.258 | 16.806 | 71.000 | 108.000 |
| chain-arithmetic-1000 | 9.419 | 4.491 | 2.097 | 2.704 | 0.360 | 0.114 | 1.000 | 0.000 |
| n-queens/variant-01 8→11 | 395.742 | 180.195 | 2.196 | 26.981 | 209.025 | 35.143 | 1.500 | 174.000 |
| send-money/send-money | 37.263 | 12.232 | 3.046 | 18.339 | 12.287 | 0.850 | 7.000 | 1.000 |
| transitive-path-100 | 23.903 | 7.758 | 3.081 | n/a | 0.003 | 15.336 | 2.500 | 0.500 |
| independent-choice-12 | 26.710 | 7.927 | 3.369 | n/a | 0.251 | 2.541 | 0.000 | 3.000 |
| n-queens/variant-01 8→10 | 100.285 | 28.077 | 3.572 | 18.450 | 40.171 | 7.621 | 1.500 | 22.500 |
| transitive-dense-40 | 28.320 | 6.215 | 4.557 | 14.541 | 5.267 | 0.926 | 2.000 | 0.000 |
| independent-choice-16 | 210.461 | 35.907 | 5.861 | n/a | 3.053 | 27.976 | 0.000 | 31.000 |
| transitive-path-200 | 121.651 | 19.231 | 6.326 | n/a | 0.003 | 106.130 | 9.000 | 5.500 |
| disjunction-12 | 197.619 | 31.178 | 6.338 | 0.126 | 9.198 | 1.713 | 0.500 | 26.000 |
| independent-negation-aggregate-16 | 272.386 | 42.878 | 6.353 | 0.217 | 7.123 | 7.030 | 0.000 | 38.000 |
| ties-50 | 144.858 | 21.806 | 6.643 | 0.319 | 9.547 | 3.636 | 1.000 | 16.000 |
| latin-square-5 | 160.031 | 19.482 | 8.214 | 1.514 | 21.850 | 14.690 | 0.500 | 14.000 |
| chain-1000 | 65.828 | 6.163 | 10.681 | n/a | 0.004 | 49.922 | 3.000 | 0.000 |
| independent-negation-8 | 105.949 | 4.739 | 22.355 | n/a | 7.048 | 93.443 | 0.000 | 0.000 |
| chain-2000 | 219.137 | 9.210 | 23.793 | n/a | 0.008 | 193.112 | 5.000 | 0.000 |
| independent-negation-10 | 1307.576 | 4.537 | 288.204 | n/a | 92.804 | 1206.702 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| n-queens/variant-04 8→11 | 15.7 | 7.4 | 0.0 |
| producer-chain-700 | 29.8 | 7.1 | n/a |
| variant-04/05-larger-mix | 16.8 | 19.3 | 0.0 |
| chain-arithmetic-1000 | 13.2 | 5.1 | 0.0 |
| n-queens/variant-01 8→11 | 15.9 | 7.2 | 0.0 |
| send-money/send-money | 24.6 | 8.2 | 0.0 |
| transitive-path-100 | 13.0 | 6.0 | n/a |
| independent-choice-12 | 10.6 | 5.0 | n/a |
| n-queens/variant-01 8→10 | 13.2 | 5.6 | 0.0 |
| transitive-dense-40 | 25.5 | 5.3 | 0.0 |
| independent-choice-16 | 10.7 | 5.0 | n/a |
| transitive-path-200 | 20.7 | 8.8 | n/a |
| disjunction-12 | 12.1 | 5.0 | 0.0 |
| independent-negation-aggregate-16 | 12.2 | 5.2 | 0.0 |
| ties-50 | 13.1 | 5.4 | 0.0 |
| latin-square-5 | 13.1 | 5.4 | 0.0 |
| chain-1000 | 16.7 | 5.4 | n/a |
| independent-negation-8 | 10.7 | 5.0 | n/a |
| chain-2000 | 23.3 | 5.9 | n/a |
| independent-negation-10 | 11.3 | 5.0 | n/a |

Against the reference: report B1-candidate, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 8 of 22 cells where both passed (36.3%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 34.958 | 192.852 | 0.181 | 2.660 | 37.859 | 8.634 | 75.000 | 113.000 |
| n-queens/variant-04 8→11 | 95.165 | 180.276 | 0.528 | 1.132 | 257.147 | 12.726 | 1.500 | 174.500 |
| independent-choice-16 | 21.507 | 36.047 | 0.597 | n/a | 2.011 | 7.091 | 0.000 | 31.000 |
| independent-negation-aggregate-16 | 26.134 | 43.023 | 0.607 | 0.200 | 2.903 | 3.182 | 0.500 | 38.000 |
| n-queens/variant-01 8→11 | 117.708 | 184.726 | 0.637 | 4.648 | 309.302 | 18.660 | 1.500 | 178.500 |
| latin-square-5 | 14.225 | 18.665 | 0.762 | 0.871 | 14.249 | 6.515 | 1.000 | 13.000 |
| transitive-path-200 | 16.090 | 19.994 | 0.805 | n/a | 0.005 | 3.106 | 9.000 | 6.000 |
| disjunction-12 | 26.147 | 31.190 | 0.838 | 0.132 | 4.021 | 38.042 | 1.000 | 26.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 7.882 | 7.850 | 1.004 | n/a | 0.334 | 0.964 | 0.000 | 3.000 |
| send-money/send-money | 12.412 | 12.225 | 1.015 | 2.441 | 6.780 | 0.154 | 7.000 | 1.000 |
| n-queens/variant-01 8→10 | 31.340 | 28.780 | 1.089 | 3.249 | 60.605 | 4.187 | 1.500 | 23.500 |
| ties-50 | 25.716 | 21.842 | 1.177 | 0.309 | 14.518 | 2.217 | 0.000 | 17.000 |
| transitive-path-100 | 9.553 | 7.737 | 1.235 | n/a | 0.005 | 0.731 | 2.500 | 0.500 |
| stratified-16 | 6.217 | 4.646 | 1.338 | n/a | 0.176 | 0.065 | 0.000 | 0.000 |
| producer-chain-700 | 13.807 | 9.187 | 1.503 | n/a | 0.007 | 1.759 | 5.000 | 0.000 |
| planning-14 | 35.248 | 18.425 | 1.913 | 2.525 | 44.878 | 12.804 | 1.000 | 13.000 |
| independent-negation-8 | 9.361 | 4.698 | 1.993 | n/a | 3.547 | 0.155 | 0.000 | 0.000 |
| chain-arithmetic-1000 | 10.089 | 4.655 | 2.167 | 2.802 | 0.313 | 0.094 | 1.000 | 0.000 |
| chain-1000 | 15.466 | 6.924 | 2.234 | n/a | 0.006 | 1.083 | 3.000 | 0.000 |
| chain-2000 | 25.210 | 9.222 | 2.734 | n/a | 0.007 | 2.518 | 5.000 | 0.000 |
| independent-negation-10 | 17.365 | 4.697 | 3.697 | n/a | 11.617 | 0.414 | 1.000 | 0.000 |
| transitive-dense-40 | 24.383 | 6.212 | 3.925 | 10.741 | 4.114 | 0.738 | 2.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 22.5 | 19.5 | n/a |
| n-queens/variant-04 8→11 | 16.5 | 7.5 | n/a |
| independent-choice-16 | 10.8 | 5.0 | n/a |
| independent-negation-aggregate-16 | 12.7 | 5.2 | n/a |
| n-queens/variant-01 8→11 | 18.4 | 7.2 | n/a |
| latin-square-5 | 14.5 | 5.4 | n/a |
| transitive-path-200 | 18.7 | 8.8 | n/a |
| disjunction-12 | 12.1 | 5.0 | n/a |
| independent-choice-12 | 10.5 | 5.0 | n/a |
| send-money/send-money | 18.0 | 8.2 | n/a |
| n-queens/variant-01 8→10 | 16.7 | 5.6 | n/a |
| ties-50 | 15.5 | 5.4 | n/a |
| transitive-path-100 | 11.7 | 6.0 | n/a |
| stratified-16 | 10.4 | 5.0 | n/a |
| producer-chain-700 | 28.5 | 7.1 | n/a |
| planning-14 | 13.8 | 5.4 | n/a |
| independent-negation-8 | 10.4 | 5.0 | n/a |
| chain-arithmetic-1000 | 13.5 | 5.1 | n/a |
| chain-1000 | 17.0 | 5.4 | n/a |
| chain-2000 | 24.7 | 5.9 | n/a |
| independent-negation-10 | 10.8 | 5.0 | n/a |
| transitive-dense-40 | 25.9 | 5.3 | n/a |

Against the reference: report B2-candidate, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 8 of 22 cells where both passed (36.3%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 35.276 | 177.964 | 0.198 | 2.655 | 37.372 | 8.558 | 73.500 | 100.000 |
| n-queens/variant-04 8→11 | 96.816 | 179.527 | 0.539 | 1.116 | 263.517 | 12.714 | 1.000 | 173.500 |
| n-queens/variant-01 8→11 | 104.945 | 183.983 | 0.570 | 4.664 | 263.959 | 18.217 | 1.500 | 178.500 |
| independent-negation-aggregate-16 | 26.179 | 43.668 | 0.599 | 0.201 | 3.011 | 3.192 | 0.500 | 38.500 |
| independent-choice-16 | 22.277 | 35.928 | 0.620 | n/a | 1.902 | 6.989 | 0.000 | 31.000 |
| latin-square-5 | 13.920 | 18.629 | 0.747 | 0.880 | 12.318 | 6.390 | 0.500 | 13.500 |
| transitive-path-200 | 16.177 | 19.267 | 0.840 | n/a | 0.005 | 3.071 | 9.000 | 5.500 |
| disjunction-12 | 26.380 | 31.125 | 0.848 | 0.126 | 3.050 | 34.347 | 1.000 | 26.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 7.887 | 7.864 | 1.003 | n/a | 0.347 | 0.974 | 0.000 | 3.000 |
| send-money/send-money | 12.408 | 12.197 | 1.017 | 2.456 | 6.080 | 0.155 | 7.000 | 1.000 |
| n-queens/variant-01 8→10 | 30.863 | 28.799 | 1.072 | 3.260 | 57.675 | 4.247 | 1.500 | 23.000 |
| stratified-16 | 5.455 | 4.649 | 1.173 | n/a | 0.175 | 0.065 | 0.000 | 0.000 |
| ties-50 | 26.070 | 21.774 | 1.197 | 0.306 | 14.368 | 2.220 | 0.000 | 17.000 |
| transitive-path-100 | 9.550 | 7.740 | 1.234 | n/a | 0.005 | 0.724 | 2.500 | 0.500 |
| producer-chain-700 | 13.780 | 9.181 | 1.501 | n/a | 0.007 | 1.762 | 5.000 | 0.000 |
| independent-negation-8 | 9.340 | 4.691 | 1.991 | n/a | 3.607 | 0.160 | 0.000 | 0.000 |
| planning-14 | 35.186 | 17.659 | 1.993 | 2.488 | 41.914 | 12.664 | 0.500 | 13.000 |
| chain-arithmetic-1000 | 10.115 | 4.641 | 2.180 | 2.779 | 0.315 | 0.094 | 1.000 | 0.000 |
| chain-1000 | 16.233 | 6.933 | 2.341 | n/a | 0.006 | 1.096 | 3.000 | 0.000 |
| chain-2000 | 25.945 | 9.203 | 2.819 | n/a | 0.007 | 2.509 | 5.000 | 0.000 |
| independent-negation-10 | 18.284 | 4.684 | 3.904 | n/a | 11.521 | 0.403 | 1.000 | 0.000 |
| transitive-dense-40 | 24.537 | 6.210 | 3.951 | 10.703 | 4.037 | 0.743 | 2.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 22.3 | 19.4 | n/a |
| n-queens/variant-04 8→11 | 16.4 | 7.2 | n/a |
| n-queens/variant-01 8→11 | 17.9 | 7.2 | n/a |
| independent-negation-aggregate-16 | 12.6 | 5.2 | n/a |
| independent-choice-16 | 10.8 | 5.0 | n/a |
| latin-square-5 | 14.5 | 5.4 | n/a |
| transitive-path-200 | 18.6 | 8.8 | n/a |
| disjunction-12 | 12.0 | 5.0 | n/a |
| independent-choice-12 | 10.6 | 5.0 | n/a |
| send-money/send-money | 18.0 | 8.2 | n/a |
| n-queens/variant-01 8→10 | 16.7 | 5.6 | n/a |
| stratified-16 | 10.3 | 5.0 | n/a |
| ties-50 | 15.5 | 5.4 | n/a |
| transitive-path-100 | 11.8 | 6.0 | n/a |
| producer-chain-700 | 28.5 | 7.1 | n/a |
| independent-negation-8 | 10.5 | 5.0 | n/a |
| planning-14 | 13.9 | 5.4 | n/a |
| chain-arithmetic-1000 | 13.7 | 5.1 | n/a |
| chain-1000 | 16.9 | 5.4 | n/a |
| chain-2000 | 24.5 | 5.9 | n/a |
| independent-negation-10 | 10.7 | 5.0 | n/a |
| transitive-dense-40 | 26.0 | 5.3 | n/a |

Against the reference: report A2-main, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 0 of 20 cells where both passed (0.0%).

Wins, fastest first: none.

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-04 8→11 | 270.437 | 180.084 | 1.502 | 1.387 | 121.972 | 24.161 | 1.500 | 173.500 |
| producer-chain-700 | 16.627 | 9.933 | 1.674 | n/a | 0.005 | 1.524 | 5.000 | 0.000 |
| chain-arithmetic-1000 | 9.426 | 4.683 | 2.013 | 2.753 | 0.372 | 0.108 | 1.000 | 0.000 |
| variant-04/05-larger-mix | 361.474 | 178.678 | 2.023 | 3.375 | 67.306 | 16.873 | 72.000 | 102.000 |
| n-queens/variant-01 8→11 | 400.552 | 183.916 | 2.178 | 27.331 | 212.829 | 35.868 | 2.000 | 177.500 |
| send-money/send-money | 38.021 | 12.230 | 3.109 | 18.823 | 12.563 | 0.879 | 7.000 | 1.000 |
| transitive-path-100 | 24.639 | 7.753 | 3.178 | n/a | 0.003 | 15.699 | 2.500 | 0.500 |
| independent-choice-12 | 26.671 | 7.936 | 3.361 | n/a | 0.251 | 2.652 | 0.000 | 3.000 |
| n-queens/variant-01 8→10 | 100.989 | 28.858 | 3.500 | 18.552 | 41.161 | 7.806 | 2.000 | 23.000 |
| transitive-dense-40 | 27.488 | 6.218 | 4.421 | 14.742 | 5.372 | 0.928 | 2.000 | 0.000 |
| independent-choice-16 | 209.887 | 35.910 | 5.845 | n/a | 3.058 | 27.712 | 0.000 | 31.000 |
| transitive-path-200 | 124.009 | 19.991 | 6.203 | n/a | 0.003 | 108.333 | 9.000 | 6.000 |
| disjunction-12 | 196.928 | 31.180 | 6.316 | 0.123 | 9.911 | 1.828 | 1.000 | 26.000 |
| independent-negation-aggregate-16 | 272.672 | 42.940 | 6.350 | 0.220 | 7.207 | 7.015 | 0.000 | 38.000 |
| ties-50 | 144.531 | 21.812 | 6.626 | 0.315 | 9.723 | 3.770 | 0.500 | 16.500 |
| latin-square-5 | 161.113 | 18.740 | 8.597 | 1.583 | 22.259 | 14.895 | 1.000 | 13.500 |
| chain-1000 | 66.613 | 6.946 | 9.590 | n/a | 0.004 | 50.613 | 3.000 | 0.000 |
| independent-negation-8 | 107.392 | 4.729 | 22.709 | n/a | 6.867 | 95.013 | 0.000 | 0.000 |
| chain-2000 | 226.223 | 9.242 | 24.478 | n/a | 0.006 | 198.263 | 5.000 | 0.000 |
| independent-negation-10 | 1317.820 | 4.769 | 276.316 | n/a | 100.576 | 1209.125 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| n-queens/variant-04 8→11 | 15.7 | 7.2 | 0.0 |
| producer-chain-700 | 28.6 | 7.1 | n/a |
| chain-arithmetic-1000 | 13.3 | 5.1 | 0.0 |
| variant-04/05-larger-mix | 16.7 | 19.3 | 0.0 |
| n-queens/variant-01 8→11 | 15.8 | 7.3 | 0.0 |
| send-money/send-money | 24.7 | 8.2 | 0.0 |
| transitive-path-100 | 13.0 | 6.0 | n/a |
| independent-choice-12 | 10.5 | 5.0 | n/a |
| n-queens/variant-01 8→10 | 13.2 | 5.6 | 0.0 |
| transitive-dense-40 | 25.5 | 5.3 | 0.0 |
| independent-choice-16 | 10.8 | 5.0 | n/a |
| transitive-path-200 | 20.6 | 8.8 | n/a |
| disjunction-12 | 11.9 | 5.0 | 0.0 |
| independent-negation-aggregate-16 | 12.2 | 5.2 | 0.0 |
| ties-50 | 13.0 | 5.4 | 0.0 |
| latin-square-5 | 13.2 | 5.4 | 0.0 |
| chain-1000 | 16.7 | 5.4 | n/a |
| independent-negation-8 | 10.8 | 5.0 | n/a |
| chain-2000 | 23.2 | 5.9 | n/a |
| independent-negation-10 | 11.1 | 5.0 | n/a |
