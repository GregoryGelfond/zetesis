Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

Formula search method by report: before=default; after=default; clauses=clauses;

| Cell | before | after | clauses | after/before | clauses/after | clauses/before | before/reference | after/reference | clauses/reference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 7.975 [7.732, 9.118] | 7.690 [7.669, 7.816] | 6.890 [6.789, 8.151] | 0.964 | 0.896 | 0.864 | 1.147 | 1.133 | 0.990 |
| independent-choice-16 | 40.472 [39.104, 41.289] | 31.455 [31.223, 32.776] | 33.363 [32.338, 35.868] | 0.777 | 1.061 | 0.824 | 1.518 | 1.137 | 1.220 |
| independent-negation-8 | 17.690 [16.694, 18.634] | 16.569 [16.098, 17.257] | 16.938 [16.681, 17.251] | 0.937 | 1.022 | 0.957 | 3.823 | 3.682 | 3.651 |
| independent-negation-10 | 53.411 [51.677, 54.403] | 49.907 [47.761, 49.972] | 49.579 [49.529, 50.749] | 0.934 | 0.993 | 0.928 | 11.844 | 11.055 | 10.482 |
| independent-negation-aggregate-16 | 22.452 [20.769, 22.907] | 20.927 [20.467, 21.740] | 27.096 [25.509, 27.429] | 0.932 | 1.295 | 1.207 | 0.720 | 0.668 | 0.838 |
| disjunction-12 | 23.103 [22.901, 24.207] | 23.015 [23.013, 23.123] | 42.829 [42.047, 45.749] | 0.996 | 1.861 | 1.854 | 1.097 | 1.135 | 2.099 |
| ties-50 | 23.149 [22.951, 23.267] | 21.900 [21.133, 22.029] | 26.751 [26.179, 27.261] | 0.946 | 1.221 | 1.156 | 1.240 | 1.224 | 1.413 |
| transitive-path-100 | 15.692 [13.904, 15.992] | 12.217 [11.818, 12.600] | 11.300 [10.550, 13.973] | 0.779 | 0.925 | 0.720 | 1.718 | 1.354 | 1.404 |
| transitive-path-200 | 46.710 [43.152, 48.217] | 30.989 [30.679, 33.353] | 31.887 [31.400, 31.895] | 0.663 | 1.029 | 0.683 | 1.970 | 1.385 | 1.447 |
| transitive-dense-40 | 35.008 [32.397, 37.160] | 34.037 [33.287, 34.178] | 32.633 [32.633, 33.327] | 0.972 | 0.959 | 0.932 | 5.239 | 4.394 | 4.886 |
| chain-1000 | 19.585 [18.946, 20.507] | 18.489 [18.359, 19.677] | 19.503 [18.408, 19.761] | 0.944 | 1.055 | 0.996 | 2.466 | 2.665 | 2.544 |
| chain-2000 | 34.715 [34.291, 37.108] | 39.821 [36.591, 40.773] | 34.790 [33.285, 36.705] | 1.147 | 0.874 | 1.002 | 3.220 | 3.938 | 3.382 |
| chain-arithmetic-1000 | 10.983 [10.939, 11.075] | 10.451 [9.936, 10.923] | 9.959 [9.885, 11.153] | 0.952 | 0.953 | 0.907 | 1.981 | 2.355 | 2.123 |
| stratified-16 | 4.482 [3.368, 4.597] | 4.505 [4.505, 4.519] | 4.496 [4.453, 4.512] | 1.005 | 0.998 | 1.003 | 1.012 | 1.005 | 0.995 |
| producer-chain-700 | 25.995 [25.746, 26.909] | 25.802 [23.577, 26.867] | 25.818 [25.630, 26.818] | 0.993 | 1.001 | 0.993 | 2.657 | 2.933 | 2.635 |
| latin-square-5 | 14.071 [13.598, 14.267] | 13.863 [13.393, 14.495] | 36.857 [36.784, 37.065] | 0.985 | 2.659 | 2.619 | 0.827 | 0.873 | 2.495 |
| planning-14 | 37.323 [35.412, 37.666] | 36.839 [36.118, 37.021] | 84.232 [84.172, 85.087] | 0.987 | 2.286 | 2.257 | 2.222 | 2.354 | 5.144 |
| n-queens/variant-01 8→10 | 26.020 [25.589, 27.518] | 26.081 [25.411, 26.304] | 59.209 [58.787, 61.996] | 1.002 | 2.270 | 2.276 | 0.851 | 0.865 | 1.961 |
| n-queens/variant-01 8→11 | 88.332 [86.632, 88.532] | 85.039 [84.943, 85.699] | 263.138 [262.049, 263.770] | 0.963 | 3.094 | 2.979 | 0.489 | 0.478 | 1.479 |
| n-queens/variant-04 8→11 | 80.218 [76.352, 81.441] | 77.431 [76.987, 80.617] | 161.004 [160.071, 162.718] | 0.965 | 2.079 | 2.007 | 0.454 | 0.439 | 0.926 |
| send-money/send-money | 13.399 [12.902, 14.138] | 13.130 [12.083, 13.308] | 10.737 [10.695, 11.382] | 0.980 | 0.818 | 0.801 | 0.825 | 0.935 | 0.762 |
| variant-04/05-larger-mix | 33.351 [33.076, 33.854] | 32.768 [32.247, 33.956] | 93.259 [93.218, 94.960] | 0.983 | 2.846 | 2.796 | 0.226 | 0.223 | 0.637 |

Reference wall time, ms, same notation.

| Cell | before | after | clauses |
|---|---:|---:|---:|
| independent-choice-12 | 6.953 [5.864, 6.960] | 6.786 [6.729, 6.972] | 6.959 [6.000, 7.041] |
| independent-choice-16 | 26.667 [24.401, 29.506] | 27.663 [26.518, 27.954] | 27.341 [27.308, 27.367] |
| independent-negation-8 | 4.628 [4.615, 4.740] | 4.500 [4.494, 4.700] | 4.640 [3.539, 4.748] |
| independent-negation-10 | 4.510 [3.410, 5.608] | 4.514 [4.513, 4.548] | 4.730 [4.471, 5.625] |
| independent-negation-aggregate-16 | 31.191 [30.308, 34.503] | 31.333 [29.854, 32.954] | 32.323 [29.111, 33.068] |
| disjunction-12 | 21.060 [20.958, 23.944] | 20.277 [19.694, 20.788] | 20.404 [19.605, 21.185] |
| ties-50 | 18.668 [18.564, 18.779] | 17.892 [16.926, 18.195] | 18.937 [18.349, 20.247] |
| transitive-path-100 | 9.132 [8.876, 9.212] | 9.025 [8.903, 9.233] | 8.051 [8.033, 8.918] |
| transitive-path-200 | 23.705 [22.158, 23.810] | 22.376 [21.669, 23.143] | 22.038 [21.818, 23.643] |
| transitive-dense-40 | 6.682 [6.661, 7.950] | 7.746 [5.582, 7.757] | 6.679 [6.635, 6.700] |
| chain-1000 | 7.942 [6.658, 10.174] | 6.936 [6.681, 7.718] | 7.665 [6.558, 7.691] |
| chain-2000 | 10.781 [10.223, 11.075] | 10.112 [9.814, 11.041] | 10.285 [9.846, 12.327] |
| chain-arithmetic-1000 | 5.545 [3.328, 5.599] | 4.438 [3.386, 5.500] | 4.691 [4.503, 5.536] |
| stratified-16 | 4.429 [4.426, 5.539] | 4.480 [4.389, 5.567] | 4.520 [4.358, 5.564] |
| producer-chain-700 | 9.784 [9.780, 9.894] | 8.797 [8.644, 9.859] | 9.800 [9.775, 10.860] |
| latin-square-5 | 17.020 [15.696, 17.913] | 15.880 [15.696, 16.724] | 14.773 [14.537, 16.681] |
| planning-14 | 16.800 [15.563, 17.759] | 15.650 [15.559, 16.327] | 16.375 [15.585, 17.315] |
| n-queens/variant-01 8→10 | 30.582 [30.217, 31.210] | 30.149 [30.133, 30.154] | 30.196 [29.427, 30.721] |
| n-queens/variant-01 8→11 | 180.523 [179.525, 182.827] | 177.986 [177.802, 179.847] | 177.874 [177.338, 178.972] |
| n-queens/variant-04 8→11 | 176.813 [174.625, 178.025] | 176.386 [175.019, 181.480] | 173.858 [173.288, 174.476] |
| send-money/send-money | 16.237 [15.042, 16.589] | 14.041 [13.975, 14.045] | 14.085 [14.071, 15.175] |
| variant-04/05-larger-mix | 147.747 [147.635, 148.923] | 146.718 [146.078, 147.611] | 146.452 [144.875, 147.073] |

Counters of report clauses: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 342835 | 4.623 |
| independent-choice-16 | 0 | 2584 | 2584 | 3198290 | 29.902 |
| independent-negation-8 | 0 | 55 | 55 | 41811 | 13.886 |
| independent-negation-10 | 0 | 144 | 144 | 136012 | 46.738 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 3137480 | 22.392 |
| disjunction-12 | 0 | 4096 | 4096 | 9004441 | 38.860 |
| ties-50 | 0 | 1225 | 1225 | 4720400 | 22.907 |
| transitive-path-100 | 0 | 1 | 1 | 209092 | 8.414 |
| transitive-path-200 | 0 | 1 | 1 | 955649 | 28.081 |
| transitive-dense-40 | 0 | 1 | 1 | 2173586 | 28.967 |
| chain-1000 | 0 | 1 | 1 | 179678 | 14.884 |
| chain-2000 | 0 | 1 | 1 | 578537 | 28.141 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 193171 | 6.257 |
| stratified-16 | 0 | 1 | 1 | 2439 | 1.457 |
| producer-chain-700 | 0 | 1 | 1 | 42445 | 21.142 |
| latin-square-5 | 0 | 1344 | 1344 | 8125616 | 32.732 |
| planning-14 | 0 | 3432 | 3432 | 19467097 | 80.621 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 11383348 | 55.141 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 57156138 | 259.049 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 35658773 | 157.111 |
| send-money/send-money | 0 | 1 | 1 | 349198 | 6.987 |
| variant-04/05-larger-mix | 0 | 1176 | 1208 | 23292550 | 89.255 |

Against the reference: report before, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 7 of 22 cells where both passed (31.8%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 33.351 | 147.747 | 0.226 | 3.249 | 32.515 | 10.890 | 84.000 | 59.000 |
| n-queens/variant-04 8→11 | 80.218 | 176.813 | 0.454 | 1.466 | 226.457 | 16.620 | 1.000 | 170.000 |
| n-queens/variant-01 8→11 | 88.332 | 180.523 | 0.489 | 5.672 | 219.689 | 24.724 | 2.000 | 174.000 |
| independent-negation-aggregate-16 | 22.452 | 31.191 | 0.720 | 0.222 | 4.154 | 4.089 | 0.000 | 27.000 |
| send-money/send-money | 13.399 | 16.237 | 0.825 | 2.517 | 6.386 | 0.283 | 10.000 | 1.000 |
| latin-square-5 | 14.071 | 17.020 | 0.827 | 1.231 | 12.007 | 8.845 | 1.000 | 11.000 |
| n-queens/variant-01 8→10 | 26.020 | 30.582 | 0.851 | 3.877 | 46.893 | 5.517 | 2.000 | 24.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stratified-16 | 4.482 | 4.429 | 1.012 | n/a | 0.484 | 0.289 | 0.000 | 0.000 |
| disjunction-12 | 23.103 | 21.060 | 1.097 | 0.124 | 2.672 | 37.672 | 0.000 | 16.000 |
| independent-choice-12 | 7.975 | 6.953 | 1.147 | n/a | 0.567 | 3.219 | 0.000 | 2.000 |
| ties-50 | 23.149 | 18.668 | 1.240 | 0.352 | 12.833 | 2.635 | 1.000 | 13.000 |
| independent-choice-16 | 40.472 | 26.667 | 1.518 | n/a | 3.251 | 22.531 | 0.000 | 22.000 |
| transitive-path-100 | 15.692 | 9.132 | 1.718 | n/a | 0.008 | 7.389 | 3.000 | 1.000 |
| transitive-path-200 | 46.710 | 23.705 | 1.970 | n/a | 0.010 | 29.370 | 14.000 | 5.000 |
| chain-arithmetic-1000 | 10.983 | 5.545 | 1.981 | 4.224 | 0.806 | 0.155 | 1.000 | 0.000 |
| planning-14 | 37.323 | 16.800 | 2.222 | 3.460 | 39.560 | 15.510 | 0.000 | 11.000 |
| chain-1000 | 19.585 | 7.942 | 2.466 | n/a | 0.009 | 3.126 | 3.000 | 0.000 |
| producer-chain-700 | 25.995 | 9.784 | 2.657 | n/a | 0.011 | 2.650 | 5.000 | 0.000 |
| chain-2000 | 34.715 | 10.781 | 3.220 | n/a | 0.011 | 5.864 | 6.000 | 0.000 |
| independent-negation-8 | 17.690 | 4.628 | 3.823 | n/a | 12.429 | 1.062 | 0.000 | 0.000 |
| transitive-dense-40 | 35.008 | 6.682 | 5.239 | 16.512 | 7.576 | 1.861 | 2.000 | 0.000 |
| independent-negation-10 | 53.411 | 4.510 | 11.844 | n/a | 44.524 | 2.874 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 15.7 | 23.6 | n/a |
| n-queens/variant-04 8→11 | 12.7 | 11.8 | n/a |
| n-queens/variant-01 8→11 | 14.0 | 11.6 | n/a |
| independent-negation-aggregate-16 | 10.9 | 10.4 | n/a |
| send-money/send-money | 14.3 | 12.7 | n/a |
| latin-square-5 | 11.3 | 10.6 | n/a |
| n-queens/variant-01 8→10 | 12.7 | 10.8 | n/a |
| stratified-16 | 8.7 | 10.4 | n/a |
| disjunction-12 | 10.3 | 10.2 | n/a |
| independent-choice-12 | 8.8 | 10.3 | n/a |
| ties-50 | 12.8 | 10.5 | n/a |
| independent-choice-16 | 9.0 | 10.2 | n/a |
| transitive-path-100 | 11.0 | 11.1 | n/a |
| transitive-path-200 | 17.4 | 13.6 | n/a |
| chain-arithmetic-1000 | 11.6 | 10.6 | n/a |
| planning-14 | 11.2 | 10.5 | n/a |
| chain-1000 | 14.5 | 10.7 | n/a |
| producer-chain-700 | 24.7 | 12.6 | n/a |
| chain-2000 | 20.5 | 11.1 | n/a |
| independent-negation-8 | 8.4 | 10.2 | n/a |
| transitive-dense-40 | 21.8 | 10.5 | n/a |
| independent-negation-10 | 9.0 | 10.3 | n/a |

Against the reference: report after, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 7 of 22 cells where both passed (31.8%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 32.768 | 146.718 | 0.223 | 3.252 | 32.094 | 11.442 | 84.000 | 57.000 |
| n-queens/variant-04 8→11 | 77.431 | 176.386 | 0.439 | 1.482 | 220.392 | 16.687 | 1.000 | 170.000 |
| n-queens/variant-01 8→11 | 85.039 | 177.986 | 0.478 | 5.691 | 217.360 | 25.951 | 2.000 | 172.000 |
| independent-negation-aggregate-16 | 20.927 | 31.333 | 0.668 | 0.223 | 3.790 | 3.865 | 1.000 | 27.000 |
| n-queens/variant-01 8→10 | 26.081 | 30.149 | 0.865 | 3.983 | 47.292 | 5.816 | 2.000 | 24.000 |
| latin-square-5 | 13.863 | 15.880 | 0.873 | 1.232 | 12.486 | 9.645 | 1.000 | 11.000 |
| send-money/send-money | 13.130 | 14.041 | 0.935 | 2.549 | 6.028 | 0.283 | 9.000 | 1.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stratified-16 | 4.505 | 4.480 | 1.005 | n/a | 0.513 | 0.224 | 0.000 | 0.000 |
| independent-choice-12 | 7.690 | 6.786 | 1.133 | n/a | 0.524 | 2.798 | 0.000 | 2.000 |
| disjunction-12 | 23.015 | 20.277 | 1.135 | 0.135 | 2.568 | 34.996 | 0.000 | 16.000 |
| independent-choice-16 | 31.455 | 27.663 | 1.137 | n/a | 2.774 | 17.472 | 0.000 | 23.000 |
| ties-50 | 21.900 | 17.892 | 1.224 | 0.347 | 13.689 | 2.902 | 0.000 | 13.000 |
| transitive-path-100 | 12.217 | 9.025 | 1.354 | n/a | 0.007 | 4.887 | 4.000 | 1.000 |
| transitive-path-200 | 30.989 | 22.376 | 1.385 | n/a | 0.008 | 17.434 | 13.000 | 4.000 |
| planning-14 | 36.839 | 15.650 | 2.354 | 2.993 | 39.552 | 16.472 | 0.000 | 11.000 |
| chain-arithmetic-1000 | 10.451 | 4.438 | 2.355 | 3.677 | 0.818 | 0.159 | 1.000 | 0.000 |
| chain-1000 | 18.489 | 6.936 | 2.665 | n/a | 0.008 | 3.219 | 3.000 | 0.000 |
| producer-chain-700 | 25.802 | 8.797 | 2.933 | n/a | 0.011 | 2.444 | 5.000 | 0.000 |
| independent-negation-8 | 16.569 | 4.500 | 3.682 | n/a | 12.250 | 0.770 | 0.000 | 0.000 |
| chain-2000 | 39.821 | 10.112 | 3.938 | n/a | 0.011 | 6.054 | 6.000 | 0.000 |
| transitive-dense-40 | 34.037 | 7.746 | 4.394 | 15.639 | 7.552 | 1.312 | 2.000 | 0.000 |
| independent-negation-10 | 49.907 | 4.514 | 11.055 | n/a | 43.569 | 2.252 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 15.8 | 23.8 | n/a |
| n-queens/variant-04 8→11 | 12.5 | 11.9 | n/a |
| n-queens/variant-01 8→11 | 14.2 | 11.6 | n/a |
| independent-negation-aggregate-16 | 10.8 | 10.4 | n/a |
| n-queens/variant-01 8→10 | 12.8 | 10.6 | n/a |
| latin-square-5 | 11.6 | 10.7 | n/a |
| send-money/send-money | 14.0 | 12.9 | n/a |
| stratified-16 | 8.9 | 10.3 | n/a |
| independent-choice-12 | 8.7 | 10.4 | n/a |
| disjunction-12 | 10.3 | 10.3 | n/a |
| independent-choice-16 | 8.9 | 10.2 | n/a |
| ties-50 | 12.9 | 10.4 | n/a |
| transitive-path-100 | 10.2 | 11.2 | n/a |
| transitive-path-200 | 14.5 | 13.6 | n/a |
| planning-14 | 11.3 | 10.6 | n/a |
| chain-arithmetic-1000 | 11.7 | 10.6 | n/a |
| chain-1000 | 14.6 | 10.9 | n/a |
| producer-chain-700 | 24.2 | 12.6 | n/a |
| independent-negation-8 | 8.4 | 10.2 | n/a |
| chain-2000 | 21.0 | 11.1 | n/a |
| transitive-dense-40 | 22.0 | 10.7 | n/a |
| independent-negation-10 | 8.5 | 10.2 | n/a |

Against the reference: report clauses, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search clauses): faster on 6 of 22 cells where both passed (27.2%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 93.259 | 146.452 | 0.637 | 3.250 | 43.545 | 25.516 | 83.000 | 57.000 |
| send-money/send-money | 10.737 | 14.085 | 0.762 | 2.566 | 1.741 | 0.239 | 9.000 | 1.000 |
| independent-negation-aggregate-16 | 27.096 | 32.323 | 0.838 | 0.225 | 2.415 | 6.387 | 1.000 | 28.000 |
| n-queens/variant-04 8→11 | 161.004 | 173.858 | 0.926 | 1.465 | 110.109 | 34.777 | 1.000 | 168.000 |
| independent-choice-12 | 6.890 | 6.959 | 0.990 | n/a | 0.515 | 2.707 | 0.000 | 2.000 |
| stratified-16 | 4.496 | 4.520 | 0.995 | n/a | 0.506 | 0.203 | 1.000 | 0.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-16 | 33.363 | 27.341 | 1.220 | n/a | 2.860 | 18.132 | 0.000 | 23.000 |
| transitive-path-100 | 11.300 | 8.051 | 1.404 | n/a | 0.009 | 4.183 | 4.000 | 0.000 |
| ties-50 | 26.751 | 18.937 | 1.413 | 0.346 | 6.834 | 5.139 | 0.000 | 14.000 |
| transitive-path-200 | 31.887 | 22.038 | 1.447 | n/a | 0.007 | 17.250 | 13.000 | 4.000 |
| n-queens/variant-01 8→11 | 263.138 | 177.874 | 1.479 | 5.736 | 188.228 | 55.059 | 2.000 | 171.000 |
| n-queens/variant-01 8→10 | 59.209 | 30.196 | 1.961 | 3.980 | 35.857 | 12.096 | 2.000 | 24.000 |
| disjunction-12 | 42.829 | 20.404 | 2.099 | 0.131 | 3.355 | 2.306 | 0.000 | 16.000 |
| chain-arithmetic-1000 | 9.959 | 4.691 | 2.123 | 3.693 | 0.584 | 0.119 | 1.000 | 0.000 |
| latin-square-5 | 36.857 | 14.773 | 2.495 | 1.266 | 7.021 | 18.311 | 1.000 | 10.000 |
| chain-1000 | 19.503 | 7.665 | 2.544 | n/a | 0.008 | 2.580 | 3.000 | 0.000 |
| producer-chain-700 | 25.818 | 9.800 | 2.635 | n/a | 0.013 | 2.657 | 6.000 | 0.000 |
| chain-2000 | 34.790 | 10.285 | 3.382 | n/a | 0.012 | 5.204 | 6.000 | 0.000 |
| independent-negation-8 | 16.938 | 4.640 | 3.651 | n/a | 12.451 | 0.810 | 0.000 | 0.000 |
| transitive-dense-40 | 32.633 | 6.679 | 4.886 | 15.996 | 9.005 | 1.067 | 2.000 | 0.000 |
| planning-14 | 84.232 | 16.375 | 5.144 | 2.991 | 23.699 | 30.908 | 1.000 | 10.000 |
| independent-negation-10 | 49.579 | 4.730 | 10.482 | n/a | 43.249 | 2.411 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 13.4 | 23.5 | n/a |
| send-money/send-money | 12.2 | 12.5 | n/a |
| independent-negation-aggregate-16 | 10.6 | 10.2 | n/a |
| n-queens/variant-04 8→11 | 12.4 | 11.6 | n/a |
| independent-choice-12 | 8.5 | 10.2 | n/a |
| stratified-16 | 8.8 | 10.3 | n/a |
| independent-choice-16 | 8.6 | 10.4 | n/a |
| transitive-path-100 | 10.2 | 11.0 | n/a |
| ties-50 | 11.1 | 10.5 | n/a |
| transitive-path-200 | 14.5 | 13.5 | n/a |
| n-queens/variant-01 8→11 | 12.7 | 12.0 | n/a |
| n-queens/variant-01 8→10 | 11.2 | 10.9 | n/a |
| disjunction-12 | 10.5 | 10.3 | n/a |
| chain-arithmetic-1000 | 11.2 | 10.5 | n/a |
| latin-square-5 | 11.0 | 10.6 | n/a |
| chain-1000 | 14.3 | 10.9 | n/a |
| producer-chain-700 | 24.1 | 12.7 | n/a |
| chain-2000 | 20.9 | 11.2 | n/a |
| independent-negation-8 | 8.7 | 10.4 | n/a |
| transitive-dense-40 | 22.3 | 10.6 | n/a |
| planning-14 | 13.5 | 10.6 | n/a |
| independent-negation-10 | 8.9 | 10.3 | n/a |
