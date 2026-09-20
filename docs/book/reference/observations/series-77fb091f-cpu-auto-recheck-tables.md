Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | before | after | before2 | after2 | after/before | after2/before | after2/before2 | before2/after | before/reference | after/reference | before2/reference | after2/reference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 7.686 [7.055, 7.805] | 5.757 [5.599, 6.776] | 6.713 [6.621, 6.839] | 7.077 [6.643, 7.651] | 0.749 | 0.921 | 1.054 | 1.166 | 1.137 | 0.855 | 0.989 | 1.038 |
| independent-choice-16 | 24.608 [23.783, 25.045] | 23.445 [23.050, 23.613] | 24.937 [24.804, 25.204] | 23.997 [23.847, 24.722] | 0.953 | 0.975 | 0.962 | 1.064 | 0.958 | 0.852 | 0.937 | 0.894 |
| independent-negation-8 | 11.408 [9.822, 12.280] | 9.909 [9.015, 11.125] | 10.768 [8.761, 11.331] | 9.744 [9.328, 10.100] | 0.869 | 0.854 | 0.905 | 1.087 | 2.482 | 1.821 | 2.291 | 1.766 |
| independent-negation-10 | 23.485 [21.475, 23.811] | 24.737 [22.563, 25.900] | 24.536 [23.784, 24.672] | 22.480 [22.440, 23.784] | 1.053 | 0.957 | 0.916 | 0.992 | 5.153 | 5.491 | 5.400 | 4.038 |
| independent-negation-aggregate-16 | 20.121 [20.097, 20.271] | 20.312 [19.824, 22.943] | 20.397 [20.264, 20.730] | 20.529 [19.946, 20.597] | 1.009 | 1.020 | 1.006 | 1.004 | 0.647 | 0.668 | 0.658 | 0.682 |
| disjunction-12 | 22.920 [22.798, 25.536] | 22.100 [21.883, 22.955] | 22.547 [22.520, 23.064] | 22.752 [21.799, 22.788] | 0.964 | 0.993 | 1.009 | 1.020 | 1.036 | 0.999 | 1.092 | 1.161 |
| ties-50 | 22.170 [21.051, 22.396] | 22.536 [21.943, 22.710] | 22.003 [20.521, 23.102] | 23.223 [21.586, 24.141] | 1.016 | 1.047 | 1.055 | 0.976 | 1.218 | 1.195 | 1.214 | 1.229 |
| transitive-path-100 | 9.115 [8.462, 10.286] | 9.504 [8.380, 10.108] | 9.233 [8.319, 10.343] | 8.455 [8.104, 8.473] | 1.043 | 0.928 | 0.916 | 0.971 | 1.037 | 1.163 | 1.042 | 1.084 |
| transitive-path-200 | 21.891 [20.698, 21.949] | 20.274 [20.206, 22.074] | 20.859 [19.806, 20.906] | 19.806 [19.721, 20.618] | 0.926 | 0.905 | 0.949 | 1.029 | 0.995 | 0.888 | 0.975 | 0.908 |
| transitive-dense-40 | 33.773 [32.194, 33.997] | 34.416 [33.531, 35.428] | 34.110 [31.209, 34.928] | 33.141 [32.296, 34.291] | 1.019 | 0.981 | 0.972 | 0.991 | 4.985 | 5.030 | 5.138 | 5.610 |
| chain-1000 | 18.410 [17.343, 18.462] | 17.387 [15.165, 18.881] | 18.642 [17.278, 19.398] | 15.308 [15.279, 18.424] | 0.944 | 0.831 | 0.821 | 1.072 | 2.621 | 2.263 | 2.815 | 1.996 |
| chain-2000 | 33.756 [33.416, 34.942] | 27.100 [26.191, 27.415] | 32.241 [31.295, 36.619] | 27.038 [25.866, 28.219] | 0.803 | 0.801 | 0.839 | 1.190 | 3.089 | 2.726 | 3.220 | 2.747 |
| chain-arithmetic-1000 | 10.948 [10.606, 11.304] | 9.894 [9.892, 10.927] | 9.919 [9.854, 11.037] | 10.958 [9.930, 11.036] | 0.904 | 1.001 | 1.105 | 1.002 | 2.429 | 2.231 | 1.779 | 1.988 |
| stratified-16 | 3.460 [3.363, 4.481] | 3.450 [3.432, 4.483] | 3.498 [3.431, 3.530] | 3.553 [3.426, 4.537] | 0.997 | 1.027 | 1.016 | 1.014 | 0.770 | 0.775 | 0.786 | 0.807 |
| producer-chain-700 | 26.782 [24.652, 28.052] | 21.770 [21.513, 23.947] | 24.684 [24.648, 26.841] | 21.476 [20.616, 21.527] | 0.813 | 0.802 | 0.870 | 1.134 | 2.765 | 2.000 | 2.536 | 2.468 |
| latin-square-5 | 13.522 [13.436, 13.815] | 14.797 [14.521, 15.311] | 13.595 [13.518, 14.123] | 13.627 [13.423, 13.764] | 1.094 | 1.008 | 1.002 | 0.919 | 0.911 | 0.940 | 0.870 | 0.872 |
| planning-14 | 37.436 [37.395, 38.360] | 37.564 [37.234, 38.660] | 36.688 [36.241, 36.744] | 36.116 [36.013, 37.574] | 1.003 | 0.965 | 0.984 | 0.977 | 2.396 | 2.380 | 2.477 | 2.306 |
| n-queens/variant-01 8→10 | 28.335 [25.951, 28.349] | 28.275 [26.380, 29.387] | 26.003 [25.486, 26.549] | 27.129 [26.405, 27.428] | 0.998 | 0.957 | 1.043 | 0.920 | 0.937 | 0.873 | 0.863 | 0.870 |
| n-queens/variant-01 8→11 | 88.101 [87.221, 88.762] | 91.750 [90.415, 92.805] | 85.877 [85.570, 86.235] | 88.689 [88.612, 90.315] | 1.041 | 1.007 | 1.033 | 0.936 | 0.477 | 0.500 | 0.482 | 0.501 |
| n-queens/variant-04 8→11 | 79.869 [79.223, 80.360] | 83.118 [82.994, 84.158] | 77.798 [77.717, 80.146] | 81.055 [79.827, 82.574] | 1.041 | 1.015 | 1.042 | 0.936 | 0.450 | 0.470 | 0.446 | 0.464 |
| send-money/send-money | 13.063 [12.668, 16.630] | 13.365 [12.989, 13.651] | 13.000 [12.475, 13.201] | 12.623 [12.513, 13.612] | 1.023 | 0.966 | 0.971 | 0.973 | 0.901 | 0.871 | 0.855 | 0.900 |
| variant-04/05-larger-mix | 33.343 [33.216, 34.870] | 33.074 [31.557, 33.588] | 32.852 [32.804, 33.891] | 32.451 [32.235, 33.085] | 0.992 | 0.973 | 0.988 | 0.993 | 0.216 | 0.220 | 0.225 | 0.224 |

Reference wall time, ms, same notation.

| Cell | before | after | before2 | after2 |
|---|---:|---:|---:|---:|
| independent-choice-12 | 6.758 [6.012, 6.807] | 6.737 [6.108, 6.749] | 6.786 [6.764, 7.108] | 6.817 [5.750, 6.862] |
| independent-choice-16 | 25.698 [23.823, 26.571] | 27.510 [26.659, 28.169] | 26.605 [23.735, 27.996] | 26.855 [26.095, 27.933] |
| independent-negation-8 | 4.596 [4.478, 5.734] | 5.441 [4.665, 6.644] | 4.701 [4.384, 5.650] | 5.516 [4.678, 5.919] |
| independent-negation-10 | 4.558 [4.457, 5.520] | 4.505 [4.504, 4.519] | 4.543 [4.519, 5.562] | 5.567 [4.441, 5.663] |
| independent-negation-aggregate-16 | 31.108 [29.699, 32.196] | 30.396 [30.117, 30.925] | 31.017 [30.068, 32.328] | 30.111 [29.091, 30.395] |
| disjunction-12 | 22.116 [21.495, 22.396] | 22.121 [21.508, 23.868] | 20.641 [19.107, 22.251] | 19.597 [18.757, 22.264] |
| ties-50 | 18.202 [17.945, 19.234] | 18.860 [17.952, 20.160] | 18.130 [16.993, 18.901] | 18.889 [18.373, 19.439] |
| transitive-path-100 | 8.787 [8.093, 8.838] | 8.169 [7.910, 8.809] | 8.861 [7.926, 8.889] | 7.801 [7.771, 9.142] |
| transitive-path-200 | 21.998 [21.752, 25.051] | 22.839 [22.756, 23.015] | 21.387 [20.624, 21.738] | 21.809 [20.638, 21.820] |
| transitive-dense-40 | 6.775 [6.628, 6.805] | 6.843 [5.600, 7.738] | 6.638 [5.575, 7.727] | 5.907 [5.759, 6.664] |
| chain-1000 | 7.024 [6.593, 9.856] | 7.682 [6.604, 7.733] | 6.623 [6.537, 7.679] | 7.667 [6.612, 7.850] |
| chain-2000 | 10.926 [9.712, 10.974] | 9.942 [9.726, 10.981] | 10.012 [8.751, 10.907] | 9.844 [8.646, 10.906] |
| chain-arithmetic-1000 | 4.507 [4.459, 5.536] | 4.435 [3.331, 5.592] | 5.577 [4.479, 5.735] | 5.512 [4.479, 5.585] |
| stratified-16 | 4.493 [4.395, 5.583] | 4.453 [4.393, 4.461] | 4.450 [3.370, 4.480] | 4.403 [4.367, 4.481] |
| producer-chain-700 | 9.686 [8.665, 9.856] | 10.885 [8.628, 11.135] | 9.733 [8.736, 10.868] | 8.701 [8.683, 9.897] |
| latin-square-5 | 14.849 [14.481, 15.834] | 15.747 [15.714, 17.104] | 15.633 [15.574, 15.835] | 15.621 [15.533, 15.687] |
| planning-14 | 15.626 [14.901, 16.091] | 15.784 [15.291, 16.049] | 14.812 [14.670, 14.985] | 15.665 [14.484, 16.684] |
| n-queens/variant-01 8→10 | 30.227 [29.923, 31.284] | 32.378 [31.248, 32.534] | 30.124 [29.011, 31.208] | 31.193 [29.994, 32.255] |
| n-queens/variant-01 8→11 | 184.780 [181.879, 186.886] | 183.381 [182.502, 183.841] | 178.243 [178.156, 179.950] | 177.106 [176.868, 177.710] |
| n-queens/variant-04 8→11 | 177.330 [176.589, 179.172] | 176.936 [176.852, 179.107] | 174.314 [174.151, 175.330] | 174.694 [173.839, 175.606] |
| send-money/send-money | 14.493 [14.030, 15.132] | 15.347 [15.135, 16.150] | 15.199 [13.376, 15.259] | 14.022 [13.997, 14.409] |
| variant-04/05-larger-mix | 154.198 [150.371, 160.218] | 150.004 [148.181, 150.114] | 145.868 [145.425, 148.153] | 144.817 [144.355, 144.966] |

Counters of report after2: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 227172 | 4.160 |
| independent-choice-16 | 0 | 2584 | 2584 | 2117111 | 21.471 |
| independent-negation-8 | 0 | 55 | 55 | 28784 | 6.469 |
| independent-negation-10 | 0 | 144 | 144 | 91687 | 20.021 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 170945 | 16.523 |
| disjunction-12 | 0 | 4096 | 4096 | 2029095 | 18.624 |
| ties-50 | 0 | 1225 | 1225 | 916857 | 19.173 |
| transitive-path-100 | 0 | 1 | 1 | 111564 | 5.690 |
| transitive-path-200 | 0 | 1 | 1 | 664776 | 16.200 |
| transitive-dense-40 | 0 | 1 | 1 | 770070 | 27.957 |
| chain-1000 | 0 | 1 | 1 | 211573 | 11.216 |
| chain-2000 | 0 | 1 | 1 | 736073 | 21.450 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 82123 | 7.011 |
| stratified-16 | 0 | 1 | 1 | 1669 | 0.972 |
| producer-chain-700 | 0 | 1 | 1 | 48256 | 16.489 |
| latin-square-5 | 0 | 1344 | 1344 | 791445 | 10.079 |
| planning-14 | 0 | 3432 | 3432 | 2522390 | 32.698 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 3452280 | 23.243 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 16394772 | 85.166 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 15197393 | 76.976 |
| send-money/send-money | 0 | 1 | 1 | 409024 | 8.877 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 2179108 | 28.682 |

Against the reference: report before, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 10 of 22 cells where both passed (45.4%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 33.343 | 154.198 | 0.216 | 3.272 | 32.280 | 12.596 | 88.000 | 60.000 |
| n-queens/variant-04 8→11 | 79.869 | 177.330 | 0.450 | 1.966 | 223.173 | 18.186 | 1.000 | 171.000 |
| n-queens/variant-01 8→11 | 88.101 | 184.780 | 0.477 | 5.590 | 224.486 | 28.081 | 2.000 | 178.000 |
| independent-negation-aggregate-16 | 20.121 | 31.108 | 0.647 | 0.244 | 3.820 | 4.101 | 0.000 | 27.000 |
| stratified-16 | 3.460 | 4.493 | 0.770 | n/a | 0.227 | 0.155 | 0.000 | 0.000 |
| send-money/send-money | 13.063 | 14.493 | 0.901 | 2.513 | 6.035 | 0.286 | 9.000 | 1.000 |
| latin-square-5 | 13.522 | 14.849 | 0.911 | 1.210 | 12.537 | 10.297 | 1.000 | 10.000 |
| n-queens/variant-01 8→10 | 28.335 | 30.227 | 0.937 | 3.939 | 47.954 | 6.153 | 2.000 | 24.000 |
| independent-choice-16 | 24.608 | 25.698 | 0.958 | n/a | 2.577 | 11.194 | 0.000 | 21.000 |
| transitive-path-200 | 21.891 | 21.998 | 0.995 | n/a | 0.008 | 6.896 | 13.000 | 4.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| disjunction-12 | 22.920 | 22.116 | 1.036 | 0.135 | 2.821 | 38.811 | 0.000 | 17.000 |
| transitive-path-100 | 9.115 | 8.787 | 1.037 | n/a | 0.008 | 2.437 | 3.000 | 1.000 |
| independent-choice-12 | 7.686 | 6.758 | 1.137 | n/a | 0.410 | 1.999 | 0.000 | 2.000 |
| ties-50 | 22.170 | 18.202 | 1.218 | 0.346 | 14.127 | 3.188 | 1.000 | 13.000 |
| planning-14 | 37.436 | 15.626 | 2.396 | 4.063 | 41.570 | 18.339 | 1.000 | 10.000 |
| chain-arithmetic-1000 | 10.948 | 4.507 | 2.429 | 3.656 | 0.799 | 0.152 | 1.000 | 0.000 |
| independent-negation-8 | 11.408 | 4.596 | 2.482 | n/a | 7.304 | 0.565 | 0.000 | 0.000 |
| chain-1000 | 18.410 | 7.024 | 2.621 | n/a | 0.008 | 2.133 | 3.000 | 0.000 |
| producer-chain-700 | 26.782 | 9.686 | 2.765 | n/a | 0.009 | 3.227 | 5.000 | 0.000 |
| chain-2000 | 33.756 | 10.926 | 3.089 | n/a | 0.009 | 4.848 | 6.000 | 0.000 |
| transitive-dense-40 | 33.773 | 6.775 | 4.985 | 16.299 | 7.255 | 1.284 | 2.000 | 0.000 |
| independent-negation-10 | 23.485 | 4.558 | 5.153 | n/a | 18.564 | 1.302 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 15.7 | 23.6 | n/a |
| n-queens/variant-04 8→11 | 12.6 | 11.8 | n/a |
| n-queens/variant-01 8→11 | 14.0 | 11.5 | n/a |
| independent-negation-aggregate-16 | 10.9 | 10.3 | n/a |
| stratified-16 | 8.8 | 10.2 | n/a |
| send-money/send-money | 14.0 | 12.8 | n/a |
| latin-square-5 | 11.6 | 10.6 | n/a |
| n-queens/variant-01 8→10 | 12.8 | 10.6 | n/a |
| independent-choice-16 | 8.3 | 10.3 | n/a |
| transitive-path-200 | 14.3 | 13.5 | n/a |
| disjunction-12 | 10.2 | 10.3 | n/a |
| transitive-path-100 | 9.7 | 11.1 | n/a |
| independent-choice-12 | 8.6 | 10.3 | n/a |
| ties-50 | 12.7 | 10.5 | n/a |
| planning-14 | 11.1 | 10.6 | n/a |
| chain-arithmetic-1000 | 11.6 | 10.6 | n/a |
| independent-negation-8 | 8.5 | 10.3 | n/a |
| chain-1000 | 14.4 | 10.7 | n/a |
| producer-chain-700 | 24.1 | 12.5 | n/a |
| chain-2000 | 21.3 | 11.1 | n/a |
| transitive-dense-40 | 21.6 | 10.6 | n/a |
| independent-negation-10 | 8.7 | 10.3 | n/a |

Against the reference: report after, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 12 of 22 cells where both passed (54.5%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 33.074 | 150.004 | 0.220 | 3.300 | 34.471 | 11.253 | 85.000 | 59.000 |
| n-queens/variant-04 8→11 | 83.118 | 176.936 | 0.470 | 1.477 | 234.470 | 16.716 | 1.000 | 171.000 |
| n-queens/variant-01 8→11 | 91.750 | 183.381 | 0.500 | 5.639 | 231.963 | 24.824 | 2.000 | 176.000 |
| independent-negation-aggregate-16 | 20.312 | 30.396 | 0.668 | 0.217 | 3.737 | 3.717 | 0.000 | 26.000 |
| stratified-16 | 3.450 | 4.453 | 0.775 | n/a | 0.225 | 0.162 | 0.000 | 0.000 |
| independent-choice-16 | 23.445 | 27.510 | 0.852 | n/a | 2.507 | 10.048 | 0.000 | 23.000 |
| independent-choice-12 | 5.757 | 6.737 | 0.855 | n/a | 0.401 | 1.896 | 0.000 | 2.000 |
| send-money/send-money | 13.365 | 15.347 | 0.871 | 2.517 | 6.527 | 0.313 | 9.000 | 1.000 |
| n-queens/variant-01 8→10 | 28.275 | 32.378 | 0.873 | 3.821 | 49.940 | 5.644 | 2.000 | 25.000 |
| transitive-path-200 | 20.274 | 22.839 | 0.888 | n/a | 0.006 | 6.682 | 12.000 | 5.000 |
| latin-square-5 | 14.797 | 15.747 | 0.940 | 1.235 | 12.515 | 8.850 | 1.000 | 11.000 |
| disjunction-12 | 22.100 | 22.121 | 0.999 | 0.128 | 2.726 | 38.217 | 0.000 | 17.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| transitive-path-100 | 9.504 | 8.169 | 1.163 | n/a | 0.005 | 2.186 | 3.000 | 1.000 |
| ties-50 | 22.536 | 18.860 | 1.195 | 0.332 | 13.251 | 2.708 | 1.000 | 13.000 |
| independent-negation-8 | 9.909 | 5.441 | 1.821 | n/a | 5.347 | 0.466 | 0.000 | 0.000 |
| producer-chain-700 | 21.770 | 10.885 | 2.000 | n/a | 0.007 | 2.982 | 6.000 | 0.000 |
| chain-arithmetic-1000 | 9.894 | 4.435 | 2.231 | 3.643 | 0.844 | 0.154 | 1.000 | 0.000 |
| chain-1000 | 17.387 | 7.682 | 2.263 | n/a | 0.007 | 2.615 | 3.000 | 0.000 |
| planning-14 | 37.564 | 15.784 | 2.380 | 3.040 | 44.431 | 16.887 | 0.000 | 11.000 |
| chain-2000 | 27.100 | 9.942 | 2.726 | n/a | 0.010 | 3.966 | 6.000 | 0.000 |
| transitive-dense-40 | 34.416 | 6.843 | 5.030 | 16.290 | 7.633 | 1.361 | 2.000 | 0.000 |
| independent-negation-10 | 24.737 | 4.505 | 5.491 | n/a | 18.285 | 1.404 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 17.5 | 23.6 | n/a |
| n-queens/variant-04 8→11 | 13.7 | 12.0 | n/a |
| n-queens/variant-01 8→11 | 15.6 | 11.8 | n/a |
| independent-negation-aggregate-16 | 10.8 | 10.5 | n/a |
| stratified-16 | 8.6 | 10.3 | n/a |
| independent-choice-16 | 8.5 | 10.3 | n/a |
| independent-choice-12 | 8.5 | 10.4 | n/a |
| send-money/send-money | 15.0 | 12.8 | n/a |
| n-queens/variant-01 8→10 | 13.7 | 10.7 | n/a |
| transitive-path-200 | 14.2 | 13.5 | n/a |
| latin-square-5 | 11.8 | 10.5 | n/a |
| disjunction-12 | 10.4 | 10.2 | n/a |
| transitive-path-100 | 9.9 | 11.2 | n/a |
| ties-50 | 13.4 | 10.5 | n/a |
| independent-negation-8 | 8.4 | 10.2 | n/a |
| producer-chain-700 | 23.7 | 12.8 | n/a |
| chain-arithmetic-1000 | 11.5 | 10.3 | n/a |
| chain-1000 | 14.2 | 10.7 | n/a |
| planning-14 | 11.1 | 10.4 | n/a |
| chain-2000 | 21.0 | 10.9 | n/a |
| transitive-dense-40 | 22.0 | 10.6 | n/a |
| independent-negation-10 | 8.6 | 10.2 | n/a |

Against the reference: report before2, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 11 of 22 cells where both passed (50.0%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 32.852 | 145.868 | 0.225 | 3.188 | 32.837 | 12.264 | 83.000 | 57.000 |
| n-queens/variant-04 8→11 | 77.798 | 174.314 | 0.446 | 1.452 | 220.737 | 17.984 | 1.000 | 169.000 |
| n-queens/variant-01 8→11 | 85.877 | 178.243 | 0.482 | 6.331 | 218.276 | 27.170 | 2.000 | 172.000 |
| independent-negation-aggregate-16 | 20.397 | 31.017 | 0.658 | 0.228 | 3.777 | 4.019 | 0.000 | 28.000 |
| stratified-16 | 3.498 | 4.450 | 0.786 | n/a | 0.219 | 0.150 | 0.000 | 0.000 |
| send-money/send-money | 13.000 | 15.199 | 0.855 | 2.520 | 6.120 | 0.285 | 9.000 | 1.000 |
| n-queens/variant-01 8→10 | 26.003 | 30.124 | 0.863 | 3.788 | 47.347 | 6.141 | 2.000 | 24.000 |
| latin-square-5 | 13.595 | 15.633 | 0.870 | 1.194 | 11.720 | 9.569 | 1.000 | 10.000 |
| independent-choice-16 | 24.937 | 26.605 | 0.937 | n/a | 2.672 | 11.163 | 1.000 | 22.000 |
| transitive-path-200 | 20.859 | 21.387 | 0.975 | n/a | 0.008 | 6.518 | 12.000 | 4.000 |
| independent-choice-12 | 6.713 | 6.786 | 0.989 | n/a | 0.405 | 1.985 | 1.000 | 2.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| transitive-path-100 | 9.233 | 8.861 | 1.042 | n/a | 0.006 | 1.926 | 4.000 | 0.000 |
| disjunction-12 | 22.547 | 20.641 | 1.092 | 0.133 | 2.781 | 38.743 | 0.000 | 16.000 |
| ties-50 | 22.003 | 18.130 | 1.214 | 0.349 | 13.142 | 3.023 | 1.000 | 13.000 |
| chain-arithmetic-1000 | 9.919 | 5.577 | 1.779 | 4.284 | 0.806 | 0.150 | 1.000 | 0.000 |
| independent-negation-8 | 10.768 | 4.701 | 2.291 | n/a | 6.485 | 0.566 | 0.000 | 0.000 |
| planning-14 | 36.688 | 14.812 | 2.477 | 3.007 | 40.946 | 18.083 | 1.000 | 10.000 |
| producer-chain-700 | 24.684 | 9.733 | 2.536 | n/a | 0.008 | 2.590 | 5.000 | 0.000 |
| chain-1000 | 18.642 | 6.623 | 2.815 | n/a | 0.007 | 2.269 | 3.000 | 0.000 |
| chain-2000 | 32.241 | 10.012 | 3.220 | n/a | 0.009 | 4.253 | 6.000 | 0.000 |
| transitive-dense-40 | 34.110 | 6.638 | 5.138 | 16.384 | 7.509 | 1.272 | 2.000 | 0.000 |
| independent-negation-10 | 24.536 | 4.543 | 5.400 | n/a | 18.742 | 1.372 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 16.2 | 23.5 | n/a |
| n-queens/variant-04 8→11 | 12.6 | 11.7 | n/a |
| n-queens/variant-01 8→11 | 13.7 | 11.9 | n/a |
| independent-negation-aggregate-16 | 10.5 | 10.4 | n/a |
| stratified-16 | 8.9 | 10.4 | n/a |
| send-money/send-money | 14.0 | 12.7 | n/a |
| n-queens/variant-01 8→10 | 12.7 | 10.8 | n/a |
| latin-square-5 | 11.5 | 10.7 | n/a |
| independent-choice-16 | 8.3 | 10.3 | n/a |
| transitive-path-200 | 14.6 | 13.6 | n/a |
| independent-choice-12 | 8.5 | 10.3 | n/a |
| transitive-path-100 | 10.0 | 11.0 | n/a |
| disjunction-12 | 10.5 | 10.4 | n/a |
| ties-50 | 12.8 | 10.5 | n/a |
| chain-arithmetic-1000 | 11.6 | 10.6 | n/a |
| independent-negation-8 | 8.5 | 10.3 | n/a |
| planning-14 | 11.3 | 10.6 | n/a |
| producer-chain-700 | 24.2 | 12.6 | n/a |
| chain-1000 | 14.3 | 10.9 | n/a |
| chain-2000 | 21.2 | 11.1 | n/a |
| transitive-dense-40 | 22.1 | 10.7 | n/a |
| independent-negation-10 | 8.5 | 10.4 | n/a |

Against the reference: report after2, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 10 of 22 cells where both passed (45.4%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 32.451 | 144.817 | 0.224 | 3.258 | 32.750 | 10.878 | 83.000 | 57.000 |
| n-queens/variant-04 8→11 | 81.055 | 174.694 | 0.464 | 1.467 | 228.881 | 16.094 | 1.000 | 169.000 |
| n-queens/variant-01 8→11 | 88.689 | 177.106 | 0.501 | 5.505 | 225.734 | 24.722 | 2.000 | 171.000 |
| independent-negation-aggregate-16 | 20.529 | 30.111 | 0.682 | 0.212 | 3.628 | 3.606 | 1.000 | 26.000 |
| stratified-16 | 3.553 | 4.403 | 0.807 | n/a | 0.218 | 0.155 | 0.000 | 0.000 |
| n-queens/variant-01 8→10 | 27.129 | 31.193 | 0.870 | 3.783 | 49.401 | 5.593 | 1.000 | 25.000 |
| latin-square-5 | 13.627 | 15.621 | 0.872 | 1.203 | 12.102 | 8.574 | 1.000 | 10.000 |
| independent-choice-16 | 23.997 | 26.855 | 0.894 | n/a | 2.636 | 10.868 | 1.000 | 22.000 |
| send-money/send-money | 12.623 | 14.022 | 0.900 | 2.495 | 6.057 | 0.291 | 9.000 | 1.000 |
| transitive-path-200 | 19.806 | 21.809 | 0.908 | n/a | 0.005 | 6.569 | 12.000 | 4.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 7.077 | 6.817 | 1.038 | n/a | 0.507 | 2.382 | 0.000 | 2.000 |
| transitive-path-100 | 8.455 | 7.801 | 1.084 | n/a | 0.003 | 1.829 | 3.000 | 0.000 |
| disjunction-12 | 22.752 | 19.597 | 1.161 | 0.124 | 2.596 | 34.246 | 0.000 | 15.000 |
| ties-50 | 23.223 | 18.889 | 1.229 | 0.329 | 15.474 | 2.880 | 1.000 | 13.000 |
| independent-negation-8 | 9.744 | 5.516 | 1.766 | n/a | 5.306 | 0.516 | 0.000 | 0.000 |
| chain-arithmetic-1000 | 10.958 | 5.512 | 1.988 | 4.252 | 0.825 | 0.156 | 1.000 | 0.000 |
| chain-1000 | 15.308 | 7.667 | 1.996 | n/a | 0.010 | 2.045 | 3.000 | 0.000 |
| planning-14 | 36.116 | 15.665 | 2.306 | 2.949 | 40.465 | 15.685 | 1.000 | 11.000 |
| producer-chain-700 | 21.476 | 8.701 | 2.468 | n/a | 0.007 | 3.324 | 5.000 | 0.000 |
| chain-2000 | 27.038 | 9.844 | 2.747 | n/a | 0.009 | 3.977 | 6.000 | 0.000 |
| independent-negation-10 | 22.480 | 5.567 | 4.038 | n/a | 18.004 | 1.357 | 1.000 | 0.000 |
| transitive-dense-40 | 33.141 | 5.907 | 5.610 | 15.349 | 7.655 | 1.306 | 2.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 18.1 | 23.6 | n/a |
| n-queens/variant-04 8→11 | 13.3 | 11.9 | n/a |
| n-queens/variant-01 8→11 | 15.7 | 11.8 | n/a |
| independent-negation-aggregate-16 | 10.9 | 10.4 | n/a |
| stratified-16 | 8.6 | 10.4 | n/a |
| n-queens/variant-01 8→10 | 13.7 | 10.6 | n/a |
| latin-square-5 | 11.7 | 10.5 | n/a |
| independent-choice-16 | 8.3 | 10.3 | n/a |
| send-money/send-money | 15.4 | 12.7 | n/a |
| transitive-path-200 | 14.3 | 13.4 | n/a |
| independent-choice-12 | 8.2 | 10.2 | n/a |
| transitive-path-100 | 9.9 | 11.1 | n/a |
| disjunction-12 | 10.4 | 10.4 | n/a |
| ties-50 | 13.5 | 10.6 | n/a |
| independent-negation-8 | 8.3 | 10.3 | n/a |
| chain-arithmetic-1000 | 11.5 | 10.5 | n/a |
| chain-1000 | 14.2 | 10.6 | n/a |
| planning-14 | 11.1 | 10.5 | n/a |
| producer-chain-700 | 23.9 | 12.7 | n/a |
| chain-2000 | 21.1 | 11.0 | n/a |
| independent-negation-10 | 8.5 | 10.2 | n/a |
| transitive-dense-40 | 21.9 | 10.7 | n/a |
