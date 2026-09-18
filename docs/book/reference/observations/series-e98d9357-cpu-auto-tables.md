Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

Formula search method by report: before=default; window=default; after=default; clauses=clauses;

| Cell | before | window | after | clauses | after/window | clauses/after | clauses/before | window/before | before/reference | window/reference | after/reference | clauses/reference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 6.656 [5.878, 6.836] | 6.722 [6.567, 7.759] | 6.667 [5.653, 8.100] | 7.021 [5.627, 7.943] | 0.992 | 1.053 | 1.055 | 1.010 | 0.965 | 0.996 | 0.976 | 1.035 |
| independent-choice-16 | 24.427 [24.058, 25.541] | 25.045 [23.768, 25.047] | 25.552 [24.293, 25.659] | 24.880 [24.542, 24.907] | 1.020 | 0.974 | 1.019 | 1.025 | 0.890 | 0.984 | 0.943 | 0.903 |
| independent-negation-8 | 13.564 [13.022, 14.103] | 14.106 [13.489, 15.106] | 14.656 [14.069, 15.179] | 14.075 [13.989, 15.217] | 1.039 | 0.960 | 1.038 | 1.040 | 2.947 | 2.938 | 3.090 | 2.526 |
| independent-negation-10 | 38.378 [38.361, 38.396] | 40.410 [39.411, 42.572] | 39.917 [37.322, 41.881] | 39.463 [39.373, 40.451] | 0.988 | 0.989 | 1.028 | 1.053 | 8.452 | 7.232 | 8.927 | 8.673 |
| independent-negation-aggregate-16 | 21.479 [20.172, 22.208] | 21.443 [20.672, 21.472] | 20.688 [20.545, 21.244] | 26.013 [25.763, 26.400] | 0.965 | 1.257 | 1.211 | 0.998 | 0.692 | 0.687 | 0.684 | 0.857 |
| disjunction-12 | 23.591 [22.780, 24.147] | 22.609 [22.244, 23.069] | 23.752 [23.184, 24.014] | 43.242 [42.595, 45.161] | 1.051 | 1.821 | 1.833 | 0.958 | 1.129 | 0.998 | 1.079 | 1.952 |
| ties-50 | 21.628 [21.113, 23.221] | 21.941 [21.936, 23.543] | 23.306 [21.938, 23.334] | 27.293 [27.124, 27.553] | 1.062 | 1.171 | 1.262 | 1.014 | 1.139 | 1.159 | 1.290 | 1.511 |
| transitive-path-100 | 9.398 [9.172, 9.554] | 16.905 [9.078, 19.269] | 9.298 [8.450, 10.455] | 10.641 [9.146, 11.348] | 0.550 | 1.144 | 1.132 | 1.799 | 1.188 | 1.902 | 1.037 | 1.204 |
| transitive-path-200 | 22.679 [22.031, 51.967] | 22.309 [21.617, 23.369] | 22.251 [22.221, 24.013] | 21.203 [21.100, 21.925] | 0.997 | 0.953 | 0.935 | 0.984 | 1.014 | 0.981 | 0.979 | 0.964 |
| transitive-dense-40 | 33.320 [33.203, 35.912] | 35.341 [33.863, 36.146] | 34.137 [33.305, 34.995] | 33.620 [32.672, 33.794] | 0.966 | 0.985 | 1.009 | 1.061 | 4.932 | 5.328 | 5.105 | 5.078 |
| chain-1000 | 18.523 [17.340, 18.713] | 18.459 [17.305, 19.703] | 18.474 [18.428, 19.970] | 18.479 [17.401, 19.338] | 1.001 | 1.000 | 0.998 | 0.997 | 2.650 | 2.408 | 2.780 | 2.409 |
| chain-2000 | 33.347 [32.473, 34.934] | 34.288 [33.483, 34.438] | 35.516 [33.270, 35.621] | 32.728 [32.352, 33.448] | 1.036 | 0.922 | 0.981 | 1.028 | 3.267 | 2.848 | 3.258 | 3.289 |
| chain-arithmetic-1000 | 10.542 [9.853, 11.279] | 10.948 [10.917, 11.017] | 10.036 [9.866, 11.017] | 9.849 [8.775, 9.944] | 0.917 | 0.981 | 0.934 | 1.039 | 1.908 | 2.436 | 2.160 | 2.114 |
| stratified-16 | 4.576 [3.438, 5.553] | 3.544 [3.442, 4.497] | 4.466 [3.411, 4.517] | 3.430 [3.376, 4.570] | 1.260 | 0.768 | 0.750 | 0.775 | 1.021 | 0.649 | 0.996 | 0.777 |
| producer-chain-700 | 25.653 [24.971, 25.726] | 26.820 [26.789, 26.913] | 26.719 [25.722, 27.876] | 25.766 [25.718, 26.829] | 0.996 | 0.964 | 1.004 | 1.045 | 2.368 | 2.738 | 2.475 | 2.635 |
| latin-square-5 | 13.569 [13.233, 14.419] | 13.521 [13.429, 14.007] | 14.333 [13.689, 15.406] | 36.845 [36.797, 37.010] | 1.060 | 2.571 | 2.715 | 0.997 | 0.814 | 0.865 | 0.918 | 2.435 |
| planning-14 | 37.197 [36.434, 37.581] | 37.184 [37.174, 37.837] | 37.510 [37.307, 38.422] | 85.200 [84.949, 93.774] | 1.009 | 2.271 | 2.290 | 1.000 | 2.222 | 2.366 | 2.391 | 5.432 |
| n-queens/variant-01 8→10 | 26.906 [26.663, 28.162] | 26.443 [25.562, 26.933] | 26.047 [25.886, 26.421] | 59.305 [59.293, 59.308] | 0.985 | 2.277 | 2.204 | 0.983 | 0.859 | 0.824 | 0.808 | 1.900 |
| n-queens/variant-01 8→11 | 86.900 [86.798, 88.260] | 86.767 [85.586, 88.581] | 87.913 [86.400, 88.529] | 266.387 [265.022, 267.172] | 1.013 | 3.030 | 3.065 | 0.998 | 0.480 | 0.473 | 0.480 | 1.470 |
| n-queens/variant-04 8→11 | 77.942 [76.858, 78.553] | 78.201 [78.171, 81.338] | 81.855 [77.683, 84.317] | 163.836 [162.235, 165.125] | 1.047 | 2.002 | 2.102 | 1.003 | 0.438 | 0.437 | 0.451 | 0.921 |
| send-money/send-money | 13.242 [12.716, 13.406] | 13.636 [13.207, 13.647] | 13.916 [12.835, 14.361] | 10.981 [10.484, 11.709] | 1.021 | 0.789 | 0.829 | 1.030 | 0.945 | 0.904 | 0.922 | 0.774 |
| variant-04/05-larger-mix | 33.868 [33.104, 33.986] | 32.839 [32.530, 33.132] | 34.014 [33.928, 34.153] | 93.346 [93.194, 93.474] | 1.036 | 2.744 | 2.756 | 0.970 | 0.228 | 0.221 | 0.227 | 0.627 |

Reference wall time, ms, same notation.

| Cell | before | window | after | clauses |
|---|---:|---:|---:|---:|
| independent-choice-12 | 6.899 [6.055, 7.021] | 6.748 [5.995, 6.841] | 6.829 [5.799, 7.965] | 6.783 [6.036, 6.829] |
| independent-choice-16 | 27.440 [25.242, 27.606] | 25.458 [25.454, 27.011] | 27.109 [26.647, 27.200] | 27.544 [27.137, 27.930] |
| independent-negation-8 | 4.603 [4.470, 5.714] | 4.801 [4.413, 5.536] | 4.743 [4.512, 5.508] | 5.572 [4.565, 5.628] |
| independent-negation-10 | 4.541 [4.509, 5.561] | 5.588 [4.508, 5.674] | 4.471 [3.381, 5.706] | 4.550 [4.519, 4.620] |
| independent-negation-aggregate-16 | 31.048 [29.046, 32.297] | 31.204 [31.183, 32.151] | 30.246 [29.059, 31.573] | 30.343 [30.291, 33.452] |
| disjunction-12 | 20.892 [19.962, 21.505] | 22.647 [20.982, 23.456] | 22.023 [20.606, 22.832] | 22.155 [21.328, 22.221] |
| ties-50 | 18.992 [18.572, 19.183] | 18.937 [17.943, 18.953] | 18.068 [17.085, 20.002] | 18.063 [17.184, 18.905] |
| transitive-path-100 | 7.913 [7.870, 9.928] | 8.888 [7.860, 8.965] | 8.964 [7.794, 9.867] | 8.836 [7.831, 8.892] |
| transitive-path-200 | 22.362 [21.151, 23.766] | 22.733 [21.759, 23.767] | 22.739 [22.664, 22.984] | 21.996 [21.656, 22.917] |
| transitive-dense-40 | 6.756 [5.599, 7.749] | 6.633 [5.584, 7.735] | 6.687 [6.605, 6.696] | 6.621 [5.838, 6.690] |
| chain-1000 | 6.989 [6.643, 7.669] | 7.667 [6.643, 8.823] | 6.645 [6.630, 7.818] | 7.670 [6.628, 7.966] |
| chain-2000 | 10.206 [9.867, 12.275] | 12.039 [10.835, 12.073] | 10.900 [9.922, 10.943] | 9.949 [9.808, 11.049] |
| chain-arithmetic-1000 | 5.524 [4.569, 5.546] | 4.494 [4.490, 5.582] | 4.646 [4.537, 5.584] | 4.658 [4.450, 5.458] |
| stratified-16 | 4.482 [4.419, 4.491] | 5.463 [4.632, 5.507] | 4.484 [4.417, 5.553] | 4.417 [3.328, 4.502] |
| producer-chain-700 | 10.831 [9.771, 10.911] | 9.796 [8.671, 11.081] | 10.797 [9.708, 10.931] | 9.776 [9.765, 10.845] |
| latin-square-5 | 16.677 [15.426, 16.862] | 15.623 [15.487, 15.704] | 15.611 [15.555, 15.653] | 15.133 [14.517, 15.442] |
| planning-14 | 16.744 [15.957, 16.977] | 15.716 [15.543, 16.782] | 15.686 [15.653, 16.044] | 15.685 [15.265, 16.708] |
| n-queens/variant-01 8→10 | 31.323 [31.293, 32.297] | 32.109 [31.157, 32.147] | 32.220 [31.203, 34.817] | 31.206 [30.104, 31.251] |
| n-queens/variant-01 8→11 | 181.091 [179.939, 182.138] | 183.363 [183.053, 183.388] | 183.211 [182.024, 184.895] | 181.238 [179.985, 184.285] |
| n-queens/variant-04 8→11 | 177.956 [177.917, 180.126] | 179.036 [178.213, 181.268] | 181.683 [180.688, 183.101] | 177.889 [176.850, 178.048] |
| send-money/send-money | 14.017 [14.005, 15.131] | 15.088 [14.046, 15.446] | 15.095 [14.093, 16.162] | 14.194 [14.012, 15.078] |
| variant-04/05-larger-mix | 148.475 [147.905, 148.616] | 148.439 [147.941, 149.028] | 150.158 [148.638, 151.103] | 148.828 [147.783, 149.742] |

Counters of report clauses: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 227172 | 4.295 |
| independent-choice-16 | 0 | 2584 | 2584 | 2117111 | 21.548 |
| independent-negation-8 | 0 | 55 | 55 | 28784 | 11.583 |
| independent-negation-10 | 0 | 144 | 144 | 91687 | 36.373 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 3137480 | 22.108 |
| disjunction-12 | 0 | 4096 | 4096 | 9004441 | 39.161 |
| ties-50 | 0 | 1225 | 1225 | 4720400 | 23.266 |
| transitive-path-100 | 0 | 1 | 1 | 111564 | 7.459 |
| transitive-path-200 | 0 | 1 | 1 | 664776 | 17.578 |
| transitive-dense-40 | 0 | 1 | 1 | 2173586 | 29.208 |
| chain-1000 | 0 | 1 | 1 | 211573 | 13.063 |
| chain-2000 | 0 | 1 | 1 | 736073 | 27.056 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 193171 | 6.101 |
| stratified-16 | 0 | 1 | 1 | 1669 | 1.278 |
| producer-chain-700 | 0 | 1 | 1 | 48256 | 21.199 |
| latin-square-5 | 0 | 1344 | 1344 | 8125616 | 33.056 |
| planning-14 | 0 | 3432 | 3432 | 19467097 | 82.074 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 11383348 | 55.350 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 57156138 | 262.636 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 35658773 | 159.981 |
| send-money/send-money | 0 | 1 | 1 | 349198 | 6.959 |
| variant-04/05-larger-mix | 0 | 1176 | 1208 | 23292550 | 89.581 |

Against the reference: report before, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 9 of 22 cells where both passed (40.9%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 33.868 | 148.475 | 0.228 | 3.310 | 33.326 | 12.111 | 84.000 | 58.000 |
| n-queens/variant-04 8→11 | 77.942 | 177.956 | 0.438 | 1.462 | 221.658 | 16.579 | 1.000 | 172.000 |
| n-queens/variant-01 8→11 | 86.900 | 181.091 | 0.480 | 5.697 | 226.463 | 26.701 | 2.000 | 174.000 |
| independent-negation-aggregate-16 | 21.479 | 31.048 | 0.692 | 0.228 | 3.745 | 3.791 | 0.000 | 27.000 |
| latin-square-5 | 13.569 | 16.677 | 0.814 | 1.223 | 12.079 | 9.506 | 1.000 | 10.000 |
| n-queens/variant-01 8→10 | 26.906 | 31.323 | 0.859 | 3.948 | 49.740 | 5.958 | 2.000 | 25.000 |
| independent-choice-16 | 24.427 | 27.440 | 0.890 | n/a | 2.740 | 10.716 | 0.000 | 23.000 |
| send-money/send-money | 13.242 | 14.017 | 0.945 | 2.600 | 6.351 | 0.299 | 9.000 | 1.000 |
| independent-choice-12 | 6.656 | 6.899 | 0.965 | n/a | 0.460 | 1.867 | 1.000 | 2.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| transitive-path-200 | 22.679 | 22.362 | 1.014 | n/a | 0.009 | 8.486 | 13.000 | 4.000 |
| stratified-16 | 4.576 | 4.482 | 1.021 | n/a | 0.378 | 0.149 | 0.000 | 0.000 |
| disjunction-12 | 23.591 | 20.892 | 1.129 | 0.138 | 2.748 | 39.315 | 0.000 | 16.000 |
| ties-50 | 21.628 | 18.992 | 1.139 | 0.361 | 13.185 | 2.766 | 1.000 | 13.000 |
| transitive-path-100 | 9.398 | 7.913 | 1.188 | n/a | 0.007 | 2.250 | 4.000 | 0.000 |
| chain-arithmetic-1000 | 10.542 | 5.524 | 1.908 | 3.743 | 0.810 | 0.144 | 1.000 | 0.000 |
| planning-14 | 37.197 | 16.744 | 2.222 | 2.978 | 40.114 | 16.957 | 1.000 | 11.000 |
| producer-chain-700 | 25.653 | 10.831 | 2.368 | n/a | 0.009 | 3.225 | 6.000 | 0.000 |
| chain-1000 | 18.523 | 6.989 | 2.650 | n/a | 0.008 | 1.899 | 3.000 | 0.000 |
| independent-negation-8 | 13.564 | 4.603 | 2.947 | n/a | 9.845 | 0.536 | 0.000 | 0.000 |
| chain-2000 | 33.347 | 10.206 | 3.267 | n/a | 0.009 | 3.981 | 6.000 | 0.000 |
| transitive-dense-40 | 33.320 | 6.756 | 4.932 | 15.787 | 7.595 | 1.308 | 2.000 | 0.000 |
| independent-negation-10 | 38.378 | 4.541 | 8.452 | n/a | 33.354 | 1.446 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 15.8 | 23.6 | n/a |
| n-queens/variant-04 8→11 | 12.4 | 11.8 | n/a |
| n-queens/variant-01 8→11 | 14.1 | 12.0 | n/a |
| independent-negation-aggregate-16 | 11.2 | 10.4 | n/a |
| latin-square-5 | 11.4 | 10.6 | n/a |
| n-queens/variant-01 8→10 | 12.9 | 10.6 | n/a |
| independent-choice-16 | 8.8 | 10.2 | n/a |
| send-money/send-money | 13.9 | 12.8 | n/a |
| independent-choice-12 | 8.3 | 10.3 | n/a |
| transitive-path-200 | 14.6 | 13.5 | n/a |
| stratified-16 | 8.9 | 10.5 | n/a |
| disjunction-12 | 10.5 | 10.3 | n/a |
| ties-50 | 12.6 | 10.5 | n/a |
| transitive-path-100 | 10.1 | 11.1 | n/a |
| chain-arithmetic-1000 | 11.6 | 10.4 | n/a |
| planning-14 | 11.3 | 10.5 | n/a |
| producer-chain-700 | 24.1 | 12.7 | n/a |
| chain-1000 | 14.5 | 10.8 | n/a |
| independent-negation-8 | 8.3 | 10.2 | n/a |
| chain-2000 | 21.3 | 11.1 | n/a |
| transitive-dense-40 | 21.9 | 10.6 | n/a |
| independent-negation-10 | 8.7 | 10.3 | n/a |

Against the reference: report window, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 12 of 22 cells where both passed (54.5%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 32.839 | 148.439 | 0.221 | 3.284 | 31.798 | 11.059 | 85.000 | 58.000 |
| n-queens/variant-04 8→11 | 78.201 | 179.036 | 0.437 | 1.486 | 224.220 | 16.431 | 2.000 | 173.000 |
| n-queens/variant-01 8→11 | 86.767 | 183.363 | 0.473 | 5.779 | 222.216 | 25.258 | 2.000 | 176.000 |
| stratified-16 | 3.544 | 5.463 | 0.649 | n/a | 0.381 | 0.169 | 0.000 | 0.000 |
| independent-negation-aggregate-16 | 21.443 | 31.204 | 0.687 | 0.221 | 3.765 | 3.781 | 0.000 | 27.000 |
| n-queens/variant-01 8→10 | 26.443 | 32.109 | 0.824 | 3.900 | 47.571 | 5.594 | 1.000 | 25.000 |
| latin-square-5 | 13.521 | 15.623 | 0.865 | 1.209 | 12.692 | 9.818 | 1.000 | 10.000 |
| send-money/send-money | 13.636 | 15.088 | 0.904 | 2.609 | 6.336 | 0.281 | 9.000 | 1.000 |
| transitive-path-200 | 22.309 | 22.733 | 0.981 | n/a | 0.007 | 8.710 | 13.000 | 4.000 |
| independent-choice-16 | 25.045 | 25.458 | 0.984 | n/a | 2.795 | 10.765 | 0.000 | 21.000 |
| independent-choice-12 | 6.722 | 6.748 | 0.996 | n/a | 0.467 | 1.983 | 1.000 | 2.000 |
| disjunction-12 | 22.609 | 22.647 | 0.998 | 0.128 | 2.702 | 38.534 | 0.000 | 18.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| ties-50 | 21.941 | 18.937 | 1.159 | 0.351 | 13.663 | 2.827 | 1.000 | 13.000 |
| transitive-path-100 | 16.905 | 8.888 | 1.902 | n/a | 0.009 | 2.777 | 3.000 | 1.000 |
| planning-14 | 37.184 | 15.716 | 2.366 | 2.966 | 40.229 | 15.852 | 0.000 | 11.000 |
| chain-1000 | 18.459 | 7.667 | 2.408 | n/a | 0.009 | 2.005 | 3.000 | 0.000 |
| chain-arithmetic-1000 | 10.948 | 4.494 | 2.436 | 4.283 | 0.827 | 0.159 | 1.000 | 0.000 |
| producer-chain-700 | 26.820 | 9.796 | 2.738 | n/a | 0.009 | 3.125 | 5.000 | 0.000 |
| chain-2000 | 34.288 | 12.039 | 2.848 | n/a | 0.010 | 4.724 | 7.000 | 0.000 |
| independent-negation-8 | 14.106 | 4.801 | 2.938 | n/a | 10.180 | 0.624 | 0.000 | 0.000 |
| transitive-dense-40 | 35.341 | 6.633 | 5.328 | 16.540 | 8.164 | 1.349 | 2.000 | 0.000 |
| independent-negation-10 | 40.410 | 5.588 | 7.232 | n/a | 34.885 | 1.651 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 15.8 | 23.7 | n/a |
| n-queens/variant-04 8→11 | 12.7 | 11.8 | n/a |
| n-queens/variant-01 8→11 | 14.0 | 11.8 | n/a |
| stratified-16 | 8.7 | 10.3 | n/a |
| independent-negation-aggregate-16 | 10.7 | 10.2 | n/a |
| n-queens/variant-01 8→10 | 13.0 | 10.6 | n/a |
| latin-square-5 | 11.7 | 10.6 | n/a |
| send-money/send-money | 14.3 | 12.7 | n/a |
| transitive-path-200 | 14.5 | 13.7 | n/a |
| independent-choice-16 | 8.8 | 10.3 | n/a |
| independent-choice-12 | 8.7 | 10.2 | n/a |
| disjunction-12 | 10.5 | 10.3 | n/a |
| ties-50 | 12.6 | 10.5 | n/a |
| transitive-path-100 | 10.2 | 11.2 | n/a |
| planning-14 | 11.0 | 10.4 | n/a |
| chain-1000 | 14.7 | 10.9 | n/a |
| chain-arithmetic-1000 | 11.7 | 10.6 | n/a |
| producer-chain-700 | 24.3 | 12.7 | n/a |
| chain-2000 | 21.6 | 11.1 | n/a |
| independent-negation-8 | 8.7 | 10.2 | n/a |
| transitive-dense-40 | 22.0 | 10.7 | n/a |
| independent-negation-10 | 8.9 | 10.3 | n/a |

Against the reference: report after, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 11 of 22 cells where both passed (50.0%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 34.014 | 150.158 | 0.227 | 3.315 | 34.052 | 12.878 | 85.000 | 58.000 |
| n-queens/variant-04 8→11 | 81.855 | 181.683 | 0.451 | 1.465 | 230.908 | 19.024 | 1.000 | 176.000 |
| n-queens/variant-01 8→11 | 87.913 | 183.211 | 0.480 | 5.687 | 228.062 | 28.163 | 2.000 | 176.000 |
| independent-negation-aggregate-16 | 20.688 | 30.246 | 0.684 | 0.235 | 4.039 | 4.268 | 0.000 | 26.000 |
| n-queens/variant-01 8→10 | 26.047 | 32.220 | 0.808 | 3.910 | 47.657 | 6.175 | 2.000 | 26.000 |
| latin-square-5 | 14.333 | 15.611 | 0.918 | 1.224 | 12.840 | 10.495 | 1.000 | 10.000 |
| send-money/send-money | 13.916 | 15.095 | 0.922 | 2.638 | 6.637 | 0.282 | 9.000 | 1.000 |
| independent-choice-16 | 25.552 | 27.109 | 0.943 | n/a | 2.765 | 10.854 | 0.000 | 22.000 |
| independent-choice-12 | 6.667 | 6.829 | 0.976 | n/a | 0.562 | 1.822 | 0.000 | 2.000 |
| transitive-path-200 | 22.251 | 22.739 | 0.979 | n/a | 0.004 | 6.712 | 13.000 | 4.000 |
| stratified-16 | 4.466 | 4.484 | 0.996 | n/a | 0.382 | 0.163 | 0.000 | 0.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| transitive-path-100 | 9.298 | 8.964 | 1.037 | n/a | 0.006 | 2.067 | 4.000 | 0.000 |
| disjunction-12 | 23.752 | 22.023 | 1.079 | 0.129 | 2.724 | 36.964 | 0.000 | 17.000 |
| ties-50 | 23.306 | 18.068 | 1.290 | 0.353 | 14.184 | 3.099 | 0.000 | 14.000 |
| chain-arithmetic-1000 | 10.036 | 4.646 | 2.160 | 3.705 | 0.808 | 0.157 | 1.000 | 0.000 |
| planning-14 | 37.510 | 15.686 | 2.391 | 3.050 | 41.015 | 17.725 | 0.000 | 11.000 |
| producer-chain-700 | 26.719 | 10.797 | 2.475 | n/a | 0.009 | 2.939 | 5.000 | 0.000 |
| chain-1000 | 18.474 | 6.645 | 2.780 | n/a | 0.004 | 2.003 | 3.000 | 0.000 |
| independent-negation-8 | 14.656 | 4.743 | 3.090 | n/a | 9.889 | 0.554 | 0.000 | 0.000 |
| chain-2000 | 35.516 | 10.900 | 3.258 | n/a | 0.006 | 3.969 | 6.000 | 0.000 |
| transitive-dense-40 | 34.137 | 6.687 | 5.105 | 16.088 | 7.819 | 1.340 | 2.000 | 0.000 |
| independent-negation-10 | 39.917 | 4.471 | 8.927 | n/a | 33.651 | 1.375 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 16.0 | 23.4 | n/a |
| n-queens/variant-04 8→11 | 12.6 | 11.7 | n/a |
| n-queens/variant-01 8→11 | 13.9 | 11.6 | n/a |
| independent-negation-aggregate-16 | 10.8 | 10.4 | n/a |
| n-queens/variant-01 8→10 | 12.8 | 10.9 | n/a |
| latin-square-5 | 11.4 | 10.6 | n/a |
| send-money/send-money | 14.1 | 12.7 | n/a |
| independent-choice-16 | 8.5 | 10.1 | n/a |
| independent-choice-12 | 8.7 | 10.2 | n/a |
| transitive-path-200 | 14.5 | 13.3 | n/a |
| stratified-16 | 9.0 | 10.4 | n/a |
| transitive-path-100 | 10.3 | 11.0 | n/a |
| disjunction-12 | 10.7 | 10.1 | n/a |
| ties-50 | 13.0 | 10.6 | n/a |
| chain-arithmetic-1000 | 11.7 | 10.4 | n/a |
| planning-14 | 11.3 | 10.5 | n/a |
| producer-chain-700 | 24.3 | 12.5 | n/a |
| chain-1000 | 14.6 | 10.8 | n/a |
| independent-negation-8 | 8.6 | 10.2 | n/a |
| chain-2000 | 21.6 | 11.0 | n/a |
| transitive-dense-40 | 21.6 | 10.5 | n/a |
| independent-negation-10 | 8.9 | 10.1 | n/a |

Against the reference: report clauses, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search clauses): faster on 7 of 22 cells where both passed (31.8%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 93.346 | 148.828 | 0.627 | 3.266 | 43.661 | 25.641 | 85.000 | 58.000 |
| send-money/send-money | 10.981 | 14.194 | 0.774 | 2.574 | 1.925 | 0.252 | 9.000 | 1.000 |
| stratified-16 | 3.430 | 4.417 | 0.777 | n/a | 0.383 | 0.156 | 0.000 | 0.000 |
| independent-negation-aggregate-16 | 26.013 | 30.343 | 0.857 | 0.221 | 2.326 | 6.386 | 0.000 | 26.000 |
| independent-choice-16 | 24.880 | 27.544 | 0.903 | n/a | 2.704 | 10.350 | 0.000 | 23.000 |
| n-queens/variant-04 8→11 | 163.836 | 177.889 | 0.921 | 1.458 | 112.408 | 34.662 | 1.000 | 171.000 |
| transitive-path-200 | 21.203 | 21.996 | 0.964 | n/a | 0.004 | 6.914 | 13.000 | 4.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 7.021 | 6.783 | 1.035 | n/a | 0.590 | 1.874 | 1.000 | 2.000 |
| transitive-path-100 | 10.641 | 8.836 | 1.204 | n/a | 0.003 | 2.446 | 3.000 | 0.000 |
| n-queens/variant-01 8→11 | 266.387 | 181.238 | 1.470 | 5.739 | 190.512 | 55.703 | 2.000 | 175.000 |
| ties-50 | 27.293 | 18.063 | 1.511 | 0.350 | 6.868 | 5.400 | 1.000 | 13.000 |
| n-queens/variant-01 8→10 | 59.305 | 31.206 | 1.900 | 3.818 | 35.886 | 12.206 | 2.000 | 25.000 |
| disjunction-12 | 43.242 | 22.155 | 1.952 | 0.125 | 3.377 | 2.106 | 0.000 | 18.000 |
| chain-arithmetic-1000 | 9.849 | 4.658 | 2.114 | 3.605 | 0.581 | 0.124 | 1.000 | 0.000 |
| chain-1000 | 18.479 | 7.670 | 2.409 | n/a | 0.004 | 2.214 | 3.000 | 0.000 |
| latin-square-5 | 36.845 | 15.133 | 2.435 | 1.236 | 7.353 | 18.648 | 1.000 | 10.000 |
| independent-negation-8 | 14.075 | 5.572 | 2.526 | n/a | 10.510 | 0.517 | 0.000 | 0.000 |
| producer-chain-700 | 25.766 | 9.776 | 2.635 | n/a | 0.005 | 2.887 | 6.000 | 0.000 |
| chain-2000 | 32.728 | 9.949 | 3.289 | n/a | 0.009 | 4.262 | 6.000 | 0.000 |
| transitive-dense-40 | 33.620 | 6.621 | 5.078 | 15.929 | 9.115 | 1.062 | 2.000 | 0.000 |
| planning-14 | 85.200 | 15.685 | 5.432 | 3.011 | 23.974 | 31.045 | 0.000 | 11.000 |
| independent-negation-10 | 39.463 | 4.550 | 8.673 | n/a | 34.061 | 1.419 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 13.5 | 23.4 | n/a |
| send-money/send-money | 12.4 | 12.6 | n/a |
| stratified-16 | 8.9 | 10.1 | n/a |
| independent-negation-aggregate-16 | 10.7 | 10.2 | n/a |
| independent-choice-16 | 8.7 | 10.2 | n/a |
| n-queens/variant-04 8→11 | 12.4 | 11.7 | n/a |
| transitive-path-200 | 14.5 | 13.4 | n/a |
| independent-choice-12 | 8.5 | 10.2 | n/a |
| transitive-path-100 | 10.3 | 10.9 | n/a |
| n-queens/variant-01 8→11 | 12.5 | 11.8 | n/a |
| ties-50 | 11.3 | 10.5 | n/a |
| n-queens/variant-01 8→10 | 10.9 | 10.8 | n/a |
| disjunction-12 | 10.5 | 10.2 | n/a |
| chain-arithmetic-1000 | 11.3 | 10.5 | n/a |
| chain-1000 | 14.6 | 10.7 | n/a |
| latin-square-5 | 11.0 | 10.4 | n/a |
| independent-negation-8 | 8.7 | 10.3 | n/a |
| producer-chain-700 | 24.1 | 12.6 | n/a |
| chain-2000 | 21.3 | 11.2 | n/a |
| transitive-dense-40 | 22.6 | 10.5 | n/a |
| planning-14 | 13.5 | 10.4 | n/a |
| independent-negation-10 | 8.6 | 10.3 | n/a |
