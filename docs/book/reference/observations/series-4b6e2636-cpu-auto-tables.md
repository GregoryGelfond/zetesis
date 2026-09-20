Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

Formula search method by report: main=default; before=default; after=default; clauses=clauses;

| Cell | main | before | after | clauses | after/before | before/main | clauses/after | clauses/main | main/reference | before/reference | after/reference | clauses/reference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 23.463 [22.511, 24.098] | 8.918 [7.876, 9.142] | 8.917 [8.805, 9.045] | 9.881 [8.816, 10.309] | 1.000 | 0.380 | 1.108 | 0.421 | 3.339 | 1.305 | 1.315 | 1.405 |
| independent-choice-16 | 198.762 [196.468, 207.041] | 38.349 [37.821, 42.193] | 37.675 [37.222, 39.279] | 38.704 [37.600, 39.930] | 0.982 | 0.193 | 1.027 | 0.195 | 7.297 | 1.390 | 1.356 | 1.390 |
| independent-negation-8 | 110.692 [105.142, 112.847] | 17.902 [17.436, 18.556] | 17.521 [17.189, 17.908] | 17.143 [16.852, 19.634] | 0.979 | 0.162 | 0.978 | 0.155 | 23.156 | 3.670 | 3.892 | 3.768 |
| independent-negation-10 | 1395.823 [1382.367, 1401.123] | 52.555 [52.261, 53.510] | 52.258 [51.499, 52.503] | 50.252 [50.221, 53.576] | 0.994 | 0.038 | 0.962 | 0.036 | 245.809 | 11.344 | 11.440 | 11.031 |
| independent-negation-aggregate-16 | 316.195 [290.619, 350.196] | 20.522 [20.352, 21.465] | 20.386 [20.220, 20.781] | 25.537 [24.854, 26.569] | 0.993 | 0.065 | 1.253 | 0.081 | 10.426 | 0.679 | 0.655 | 0.844 |
| disjunction-12 | 217.385 [213.667, 218.552] | 22.295 [21.857, 22.762] | 21.989 [21.405, 22.290] | 42.525 [42.371, 44.878] | 0.986 | 0.103 | 1.934 | 0.196 | 11.081 | 1.080 | 1.142 | 1.851 |
| ties-50 | 185.397 [167.557, 185.976] | 21.277 [21.101, 21.502] | 21.106 [20.994, 21.593] | 27.812 [25.906, 28.774] | 0.992 | 0.115 | 1.318 | 0.150 | 10.056 | 1.151 | 1.144 | 1.466 |
| transitive-path-100 | 32.039 [31.503, 32.422] | 14.577 [13.947, 16.090] | 14.494 [13.853, 14.953] | 16.588 [14.732, 17.117] | 0.994 | 0.455 | 1.144 | 0.518 | 3.994 | 1.630 | 1.820 | 1.854 |
| transitive-path-200 | 182.472 [181.950, 183.455] | 44.332 [43.007, 44.857] | 46.014 [45.518, 49.066] | 45.896 [44.528, 51.142] | 1.038 | 0.243 | 0.997 | 0.252 | 8.017 | 1.928 | 2.117 | 1.949 |
| transitive-dense-40 | 41.694 [38.791, 42.764] | 30.325 [29.573, 32.303] | 30.238 [29.072, 30.527] | 34.282 [32.731, 36.531] | 0.997 | 0.727 | 1.134 | 0.822 | 5.942 | 4.539 | 4.526 | 5.132 |
| chain-1000 | 101.210 [98.547, 103.779] | 19.699 [19.525, 22.987] | 19.540 [19.538, 20.572] | 19.584 [19.489, 20.623] | 0.992 | 0.195 | 1.002 | 0.193 | 12.821 | 2.563 | 2.542 | 2.828 |
| chain-2000 | 358.781 [357.675, 358.861] | 34.489 [33.724, 37.148] | 35.522 [34.589, 35.744] | 35.547 [35.404, 37.921] | 1.030 | 0.096 | 1.001 | 0.099 | 32.027 | 3.131 | 3.560 | 3.243 |
| chain-arithmetic-1000 | 10.357 [9.911, 10.961] | 9.903 [9.824, 10.566] | 9.995 [9.852, 10.302] | 9.943 [9.865, 12.021] | 1.009 | 0.956 | 0.995 | 0.960 | 2.197 | 2.161 | 2.213 | 2.189 |
| stratified-16 | blocked by timeout ×3 | 4.502 [4.487, 4.582] | 4.524 [4.504, 4.680] | 4.500 [4.486, 4.508] | 1.005 | n/a | 0.995 | n/a | n/a | 1.002 | 1.015 | 1.002 |
| producer-chain-700 | 26.864 [25.679, 28.390] | 26.934 [26.888, 27.937] | 25.759 [24.748, 25.796] | 25.654 [25.647, 25.711] | 0.956 | 1.003 | 0.996 | 0.955 | 2.737 | 2.710 | 2.613 | 2.641 |
| n-queens/variant-01 8→10 | 126.042 [125.024, 129.967] | 32.773 [32.445, 34.075] | 33.339 [33.284, 33.921] | 59.159 [58.234, 61.010] | 1.017 | 0.260 | 1.774 | 0.469 | 4.181 | 1.051 | 1.067 | 1.897 |
| n-queens/variant-01 8→11 | 449.943 [449.190, 462.199] | 122.645 [121.361, 123.644] | 120.827 [120.744, 123.657] | 262.524 [261.114, 265.301] | 0.985 | 0.273 | 2.173 | 0.583 | 2.502 | 0.668 | 0.661 | 1.435 |
| n-queens/variant-04 8→11 | 310.699 [306.891, 312.195] | 101.502 [96.546, 102.879] | 98.671 [98.466, 100.558] | 160.287 [159.794, 161.356] | 0.972 | 0.327 | 1.624 | 0.516 | 1.754 | 0.561 | 0.549 | 0.892 |
| send-money/send-money | 51.522 [50.159, 60.792] | 13.874 [13.663, 13.882] | 13.489 [13.454, 14.451] | 10.555 [10.512, 11.576] | 0.972 | 0.269 | 0.782 | 0.205 | 3.563 | 0.985 | 0.891 | 0.684 |
| variant-04/05-larger-mix | 345.639 [344.271, 346.751] | 44.784 [43.584, 45.474] | 44.392 [43.550, 46.736] | 92.854 [92.610, 92.893] | 0.991 | 0.130 | 2.092 | 0.269 | 2.338 | 0.303 | 0.301 | 0.615 |

Reference wall time, ms, same notation.

| Cell | main | before | after | clauses |
|---|---:|---:|---:|---:|
| independent-choice-12 | 7.026 [5.838, 7.190] | 6.831 [5.924, 8.209] | 6.781 [6.143, 6.864] | 7.033 [6.825, 8.223] |
| independent-choice-16 | 27.238 [26.442, 30.605] | 27.583 [27.238, 28.370] | 27.783 [27.635, 29.705] | 27.849 [27.011, 28.175] |
| independent-negation-8 | 4.780 [4.679, 5.583] | 4.878 [4.533, 5.564] | 4.502 [4.451, 4.766] | 4.549 [4.536, 5.641] |
| independent-negation-10 | 5.678 [4.526, 5.711] | 4.633 [4.525, 5.561] | 4.568 [4.512, 5.579] | 4.556 [4.541, 4.632] |
| independent-negation-aggregate-16 | 30.328 [30.271, 30.898] | 30.233 [29.861, 30.418] | 31.131 [30.299, 35.043] | 30.245 [29.533, 32.940] |
| disjunction-12 | 19.618 [18.818, 19.801] | 20.639 [19.621, 21.280] | 19.254 [18.973, 23.173] | 22.971 [20.565, 23.328] |
| ties-50 | 18.436 [17.866, 18.920] | 18.485 [18.138, 18.577] | 18.452 [17.480, 18.713] | 18.977 [17.124, 19.103] |
| transitive-path-100 | 8.022 [7.885, 8.930] | 8.943 [8.038, 9.176] | 7.964 [7.835, 8.060] | 8.948 [8.872, 8.959] |
| transitive-path-200 | 22.762 [22.524, 22.784] | 22.991 [22.372, 23.657] | 21.731 [21.622, 22.395] | 23.548 [21.664, 23.699] |
| transitive-dense-40 | 7.017 [6.686, 7.900] | 6.681 [6.659, 7.734] | 6.681 [6.607, 7.060] | 6.680 [6.582, 6.899] |
| chain-1000 | 7.894 [6.876, 7.991] | 7.687 [6.645, 7.811] | 7.688 [6.823, 7.788] | 6.925 [6.630, 7.732] |
| chain-2000 | 11.203 [10.366, 11.224] | 11.014 [9.789, 11.278] | 9.977 [9.770, 11.054] | 10.960 [9.759, 10.973] |
| chain-arithmetic-1000 | 4.715 [4.437, 7.740] | 4.582 [4.534, 6.596] | 4.516 [4.454, 5.542] | 4.541 [4.465, 5.546] |
| stratified-16 | 5.562 [4.468, 5.675] | 4.495 [4.413, 4.570] | 4.458 [3.354, 4.516] | 4.490 [4.479, 5.455] |
| producer-chain-700 | 9.814 [9.811, 10.964] | 9.940 [8.696, 10.908] | 9.856 [9.803, 11.020] | 9.715 [8.701, 9.759] |
| n-queens/variant-01 8→10 | 30.144 [30.001, 30.589] | 31.173 [30.439, 31.195] | 31.240 [30.177, 32.265] | 31.191 [30.665, 31.231] |
| n-queens/variant-01 8→11 | 179.822 [179.763, 181.453] | 183.493 [182.210, 184.274] | 182.777 [182.571, 183.724] | 182.928 [182.600, 183.442] |
| n-queens/variant-04 8→11 | 177.180 [176.632, 177.447] | 180.812 [180.050, 180.814] | 179.762 [178.475, 181.722] | 179.697 [179.653, 179.759] |
| send-money/send-money | 14.461 [13.485, 16.374] | 14.087 [14.063, 14.483] | 15.143 [13.030, 15.593] | 15.426 [15.157, 15.513] |
| variant-04/05-larger-mix | 147.824 [147.811, 177.277] | 147.653 [147.192, 148.573] | 147.667 [146.981, 149.689] | 151.012 [148.483, 152.107] |

Counters of report clauses: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 1090269 | 6.428 |
| independent-choice-16 | 0 | 2584 | 2584 | 10427471 | 36.208 |
| independent-negation-8 | 0 | 55 | 55 | 123109 | 15.104 |
| independent-negation-10 | 0 | 144 | 144 | 425724 | 47.635 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 3137480 | 21.458 |
| disjunction-12 | 0 | 4096 | 4096 | 9004441 | 38.802 |
| ties-50 | 0 | 1225 | 1225 | 4720400 | 22.979 |
| transitive-path-100 | 0 | 1 | 1 | 1197772 | 13.210 |
| transitive-path-200 | 0 | 1 | 1 | 5378096 | 41.901 |
| transitive-dense-40 | 0 | 1 | 1 | 2173586 | 30.277 |
| chain-1000 | 0 | 1 | 1 | 475452 | 14.726 |
| chain-2000 | 0 | 1 | 1 | 1025311 | 29.555 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 193171 | 6.177 |
| stratified-16 | 0 | 1 | 1 | 9133 | 1.554 |
| producer-chain-700 | 0 | 1 | 1 | 81187 | 21.104 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 11383348 | 55.284 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 57156138 | 258.527 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 35658773 | 156.566 |
| send-money/send-money | 0 | 1 | 1 | 349198 | 6.842 |
| variant-04/05-larger-mix | 0 | 1176 | 1208 | 23292550 | 89.094 |

Against the reference: report main, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 0 of 19 cells where both passed (0.0%).

Wins, fastest first: none.

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-04 8→11 | 310.699 | 177.180 | 1.754 | 2.089 | 141.033 | 33.340 | 2.000 | 171.000 |
| chain-arithmetic-1000 | 10.357 | 4.715 | 2.197 | 3.845 | 0.584 | 0.169 | 1.000 | 0.000 |
| variant-04/05-larger-mix | 345.639 | 147.824 | 2.338 | 4.262 | 79.812 | 23.986 | 84.000 | 58.000 |
| n-queens/variant-01 8→11 | 449.943 | 179.822 | 2.502 | 35.748 | 228.322 | 54.991 | 2.000 | 174.000 |
| producer-chain-700 | 26.864 | 9.814 | 2.737 | n/a | 0.005 | 3.436 | 5.000 | 0.000 |
| independent-choice-12 | 23.463 | 7.026 | 3.339 | n/a | 0.295 | 6.117 | 1.000 | 2.000 |
| send-money/send-money | 51.522 | 14.461 | 3.563 | 27.793 | 16.671 | 0.989 | 9.000 | 1.000 |
| transitive-path-100 | 32.039 | 8.022 | 3.994 | n/a | 0.006 | 24.750 | 3.000 | 1.000 |
| n-queens/variant-01 8→10 | 126.042 | 30.144 | 4.181 | 24.534 | 48.390 | 12.580 | 2.000 | 24.000 |
| transitive-dense-40 | 41.694 | 7.017 | 5.942 | 21.639 | 9.607 | 1.102 | 2.000 | 0.000 |
| independent-choice-16 | 198.762 | 27.238 | 7.297 | n/a | 3.431 | 63.679 | 0.000 | 23.000 |
| transitive-path-200 | 182.472 | 22.762 | 8.017 | n/a | 0.006 | 167.490 | 13.000 | 4.000 |
| ties-50 | 185.397 | 18.436 | 10.056 | 0.416 | 12.288 | 4.872 | 0.000 | 13.000 |
| independent-negation-aggregate-16 | 316.195 | 30.328 | 10.426 | 0.261 | 8.851 | 9.129 | 1.000 | 26.000 |
| disjunction-12 | 217.385 | 19.618 | 11.081 | 0.152 | 14.507 | 2.932 | 1.000 | 15.000 |
| chain-1000 | 101.210 | 7.894 | 12.821 | n/a | 0.006 | 84.069 | 3.000 | 0.000 |
| independent-negation-8 | 110.692 | 4.780 | 23.156 | n/a | 8.976 | 97.204 | 0.000 | 0.000 |
| chain-2000 | 358.781 | 11.203 | 32.027 | n/a | 0.009 | 328.393 | 6.000 | 0.000 |
| independent-negation-10 | 1395.823 | 5.678 | 245.809 | n/a | 114.200 | 1266.916 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| n-queens/variant-04 8→11 | 12.0 | 11.8 | n/a |
| chain-arithmetic-1000 | 11.0 | 10.4 | n/a |
| variant-04/05-larger-mix | 12.7 | 23.7 | n/a |
| n-queens/variant-01 8→11 | 11.9 | 11.6 | n/a |
| producer-chain-700 | 25.0 | 12.7 | n/a |
| independent-choice-12 | 8.6 | 10.2 | n/a |
| send-money/send-money | 19.0 | 12.8 | n/a |
| transitive-path-100 | 10.6 | 11.2 | n/a |
| n-queens/variant-01 8→10 | 10.7 | 10.7 | n/a |
| transitive-dense-40 | 21.9 | 10.6 | n/a |
| independent-choice-16 | 8.6 | 10.3 | n/a |
| transitive-path-200 | 17.8 | 13.5 | n/a |
| ties-50 | 10.6 | 10.4 | n/a |
| independent-negation-aggregate-16 | 10.1 | 10.4 | n/a |
| disjunction-12 | 10.0 | 10.2 | n/a |
| chain-1000 | 14.0 | 10.9 | n/a |
| independent-negation-8 | 8.3 | 10.3 | n/a |
| chain-2000 | 20.3 | 11.1 | n/a |
| independent-negation-10 | 8.4 | 10.2 | n/a |

Against the reference: report before, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 5 of 20 cells where both passed (25.0%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 44.784 | 147.653 | 0.303 | 3.302 | 19.643 | 0.140 | 84.000 | 58.000 |
| n-queens/variant-04 8→11 | 101.502 | 180.812 | 0.561 | 1.750 | 73.856 | 0.107 | 2.000 | 174.000 |
| n-queens/variant-01 8→11 | 122.645 | 183.493 | 0.668 | 5.750 | 95.114 | 0.183 | 2.000 | 177.000 |
| independent-negation-aggregate-16 | 20.522 | 30.233 | 0.679 | 0.224 | 2.476 | 0.011 | 0.000 | 25.000 |
| send-money/send-money | 13.874 | 14.087 | 0.985 | 2.590 | 4.082 | 0.275 | 9.000 | 1.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stratified-16 | 4.502 | 4.495 | 1.002 | n/a | 0.481 | 0.294 | 0.000 | 0.000 |
| n-queens/variant-01 8→10 | 32.773 | 31.173 | 1.051 | 3.883 | 20.134 | 0.143 | 1.000 | 25.000 |
| disjunction-12 | 22.295 | 20.639 | 1.080 | 0.144 | 3.626 | 0.004 | 1.000 | 16.000 |
| ties-50 | 21.277 | 18.485 | 1.151 | 0.345 | 6.497 | 0.039 | 1.000 | 13.000 |
| independent-choice-12 | 8.918 | 6.831 | 1.305 | n/a | 0.557 | 3.206 | 1.000 | 2.000 |
| independent-choice-16 | 38.349 | 27.583 | 1.390 | n/a | 3.141 | 23.054 | 0.000 | 23.000 |
| transitive-path-100 | 14.577 | 8.943 | 1.630 | n/a | 0.010 | 7.602 | 3.000 | 1.000 |
| transitive-path-200 | 44.332 | 22.991 | 1.928 | n/a | 0.008 | 30.032 | 13.000 | 4.000 |
| chain-arithmetic-1000 | 9.903 | 4.582 | 2.161 | 3.580 | 0.970 | 0.125 | 1.000 | 0.000 |
| chain-1000 | 19.699 | 7.687 | 2.563 | n/a | 0.005 | 3.510 | 3.000 | 0.000 |
| producer-chain-700 | 26.934 | 9.940 | 2.710 | n/a | 0.009 | 2.874 | 6.000 | 0.000 |
| chain-2000 | 34.489 | 11.014 | 3.131 | n/a | 0.010 | 6.031 | 6.000 | 0.000 |
| independent-negation-8 | 17.902 | 4.878 | 3.670 | n/a | 12.814 | 0.971 | 0.000 | 0.000 |
| transitive-dense-40 | 30.325 | 6.681 | 4.539 | 16.103 | 5.052 | 1.228 | 2.000 | 0.000 |
| independent-negation-10 | 52.555 | 4.633 | 11.344 | n/a | 44.609 | 3.163 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 13.4 | 23.8 | n/a |
| n-queens/variant-04 8→11 | 11.1 | 11.9 | n/a |
| n-queens/variant-01 8→11 | 11.9 | 11.8 | n/a |
| independent-negation-aggregate-16 | 10.5 | 10.3 | n/a |
| send-money/send-money | 12.6 | 12.8 | n/a |
| stratified-16 | 8.6 | 10.2 | n/a |
| n-queens/variant-01 8→10 | 11.5 | 10.6 | n/a |
| disjunction-12 | 10.3 | 10.4 | n/a |
| ties-50 | 11.6 | 10.5 | n/a |
| independent-choice-12 | 9.0 | 10.2 | n/a |
| independent-choice-16 | 8.9 | 10.2 | n/a |
| transitive-path-100 | 10.8 | 11.0 | n/a |
| transitive-path-200 | 17.4 | 13.5 | n/a |
| chain-arithmetic-1000 | 11.2 | 10.6 | n/a |
| chain-1000 | 14.4 | 10.8 | n/a |
| producer-chain-700 | 24.7 | 12.5 | n/a |
| chain-2000 | 20.8 | 11.1 | n/a |
| independent-negation-8 | 8.4 | 10.2 | n/a |
| transitive-dense-40 | 18.5 | 10.7 | n/a |
| independent-negation-10 | 8.8 | 10.1 | n/a |

Against the reference: report after, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 5 of 20 cells where both passed (25.0%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 44.392 | 147.667 | 0.301 | 3.237 | 85.644 | 13.182 | 84.000 | 58.000 |
| n-queens/variant-04 8→11 | 98.671 | 179.762 | 0.549 | 1.636 | 324.535 | 18.568 | 1.000 | 173.000 |
| independent-negation-aggregate-16 | 20.386 | 31.131 | 0.655 | 0.238 | 8.544 | 4.016 | 1.000 | 27.000 |
| n-queens/variant-01 8→11 | 120.827 | 182.777 | 0.661 | 5.723 | 385.879 | 28.227 | 2.000 | 177.000 |
| send-money/send-money | 13.489 | 15.143 | 0.891 | 2.813 | 11.298 | 0.295 | 9.000 | 1.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stratified-16 | 4.524 | 4.458 | 1.015 | n/a | 0.813 | 0.329 | 0.000 | 0.000 |
| n-queens/variant-01 8→10 | 33.339 | 31.240 | 1.067 | 4.425 | 81.745 | 6.371 | 1.000 | 25.000 |
| disjunction-12 | 21.989 | 19.254 | 1.142 | 0.141 | 6.861 | 31.162 | 0.000 | 14.000 |
| ties-50 | 21.106 | 18.452 | 1.144 | 0.349 | 18.902 | 3.068 | 1.000 | 13.000 |
| independent-choice-12 | 8.917 | 6.781 | 1.315 | n/a | 0.544 | 3.720 | 0.000 | 2.000 |
| independent-choice-16 | 37.675 | 27.783 | 1.356 | n/a | 2.945 | 22.408 | 0.000 | 23.000 |
| transitive-path-100 | 14.494 | 7.964 | 1.820 | n/a | 0.010 | 7.021 | 4.000 | 0.000 |
| transitive-path-200 | 46.014 | 21.731 | 2.117 | n/a | 0.005 | 30.790 | 13.000 | 4.000 |
| chain-arithmetic-1000 | 9.995 | 4.516 | 2.213 | 3.624 | 0.693 | 0.161 | 1.000 | 0.000 |
| chain-1000 | 19.540 | 7.688 | 2.542 | n/a | 0.011 | 3.443 | 3.000 | 0.000 |
| producer-chain-700 | 25.759 | 9.856 | 2.613 | n/a | 0.010 | 2.739 | 5.000 | 0.000 |
| chain-2000 | 35.522 | 9.977 | 3.560 | n/a | 0.009 | 6.224 | 6.000 | 0.000 |
| independent-negation-8 | 17.521 | 4.502 | 3.892 | n/a | 12.801 | 1.015 | 0.000 | 0.000 |
| transitive-dense-40 | 30.238 | 6.681 | 4.526 | 16.293 | 4.861 | 1.341 | 2.000 | 0.000 |
| independent-negation-10 | 52.258 | 4.568 | 11.440 | n/a | 45.051 | 2.758 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 13.5 | 23.5 | n/a |
| n-queens/variant-04 8→11 | 11.3 | 11.9 | n/a |
| independent-negation-aggregate-16 | 10.5 | 10.2 | n/a |
| n-queens/variant-01 8→11 | 11.8 | 11.6 | n/a |
| send-money/send-money | 12.5 | 12.8 | n/a |
| stratified-16 | 8.9 | 10.4 | n/a |
| n-queens/variant-01 8→10 | 11.3 | 10.7 | n/a |
| disjunction-12 | 10.1 | 10.2 | n/a |
| ties-50 | 11.8 | 10.6 | n/a |
| independent-choice-12 | 8.9 | 10.2 | n/a |
| independent-choice-16 | 8.8 | 10.2 | n/a |
| transitive-path-100 | 10.7 | 11.2 | n/a |
| transitive-path-200 | 17.5 | 13.4 | n/a |
| chain-arithmetic-1000 | 11.1 | 10.5 | n/a |
| chain-1000 | 14.6 | 10.8 | n/a |
| producer-chain-700 | 24.6 | 12.5 | n/a |
| chain-2000 | 20.6 | 11.1 | n/a |
| independent-negation-8 | 8.3 | 10.3 | n/a |
| transitive-dense-40 | 18.5 | 10.7 | n/a |
| independent-negation-10 | 8.9 | 10.3 | n/a |

Against the reference: report clauses, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search clauses): faster on 4 of 20 cells where both passed (20.0%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 92.854 | 151.012 | 0.615 | 3.263 | 44.416 | 24.026 | 86.000 | 59.000 |
| send-money/send-money | 10.555 | 15.426 | 0.684 | 2.543 | 1.771 | 0.253 | 9.000 | 1.000 |
| independent-negation-aggregate-16 | 25.537 | 30.245 | 0.844 | 0.223 | 2.352 | 6.016 | 1.000 | 26.000 |
| n-queens/variant-04 8→11 | 160.287 | 179.697 | 0.892 | 1.478 | 110.989 | 33.014 | 2.000 | 173.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stratified-16 | 4.500 | 4.490 | 1.002 | n/a | 0.481 | 0.330 | 0.000 | 0.000 |
| independent-choice-16 | 38.704 | 27.849 | 1.390 | n/a | 2.883 | 22.944 | 0.000 | 23.000 |
| independent-choice-12 | 9.881 | 7.033 | 1.405 | n/a | 0.643 | 3.500 | 0.000 | 2.000 |
| n-queens/variant-01 8→11 | 262.524 | 182.928 | 1.435 | 5.754 | 188.906 | 52.876 | 2.000 | 176.000 |
| ties-50 | 27.812 | 18.977 | 1.466 | 0.347 | 6.976 | 5.060 | 1.000 | 13.000 |
| disjunction-12 | 42.525 | 22.971 | 1.851 | 0.137 | 3.448 | 1.993 | 0.000 | 18.000 |
| transitive-path-100 | 16.588 | 8.948 | 1.854 | n/a | 0.003 | 7.588 | 3.000 | 1.000 |
| n-queens/variant-01 8→10 | 59.159 | 31.191 | 1.897 | 3.918 | 36.388 | 11.597 | 2.000 | 25.000 |
| transitive-path-200 | 45.896 | 23.548 | 1.949 | n/a | 0.005 | 29.821 | 13.000 | 4.000 |
| chain-arithmetic-1000 | 9.943 | 4.541 | 2.189 | 3.698 | 0.601 | 0.121 | 1.000 | 0.000 |
| producer-chain-700 | 25.654 | 9.715 | 2.641 | n/a | 0.010 | 2.701 | 5.000 | 0.000 |
| chain-1000 | 19.584 | 6.925 | 2.828 | n/a | 0.009 | 3.416 | 3.000 | 0.000 |
| chain-2000 | 35.547 | 10.960 | 3.243 | n/a | 0.006 | 6.072 | 6.000 | 0.000 |
| independent-negation-8 | 17.143 | 4.549 | 3.768 | n/a | 13.443 | 1.008 | 0.000 | 0.000 |
| transitive-dense-40 | 34.282 | 6.680 | 5.132 | 16.210 | 9.466 | 1.059 | 2.000 | 0.000 |
| independent-negation-10 | 50.252 | 4.556 | 11.031 | n/a | 43.387 | 2.813 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 13.3 | 23.5 | n/a |
| send-money/send-money | 12.3 | 12.8 | n/a |
| independent-negation-aggregate-16 | 10.5 | 10.3 | n/a |
| n-queens/variant-04 8→11 | 12.3 | 11.7 | n/a |
| stratified-16 | 8.4 | 10.2 | n/a |
| independent-choice-16 | 8.7 | 10.3 | n/a |
| independent-choice-12 | 9.2 | 10.2 | n/a |
| n-queens/variant-01 8→11 | 12.5 | 11.7 | n/a |
| ties-50 | 11.3 | 10.5 | n/a |
| disjunction-12 | 10.5 | 10.3 | n/a |
| transitive-path-100 | 10.7 | 10.9 | n/a |
| n-queens/variant-01 8→10 | 11.0 | 10.8 | n/a |
| transitive-path-200 | 17.4 | 13.4 | n/a |
| chain-arithmetic-1000 | 11.2 | 10.3 | n/a |
| producer-chain-700 | 24.5 | 12.7 | n/a |
| chain-1000 | 14.4 | 10.8 | n/a |
| chain-2000 | 20.4 | 11.2 | n/a |
| independent-negation-8 | 8.4 | 10.1 | n/a |
| transitive-dense-40 | 22.2 | 10.7 | n/a |
| independent-negation-10 | 8.7 | 10.2 | n/a |
