Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | before | after | after/before | before/reference | after/reference |
|---|---:|---:|---:|---:|---:|
| independent-choice-12 | 6.682 [6.670, 6.993] | 7.027 [6.926, 8.055] | 1.052 | 0.989 | 1.026 |
| independent-choice-16 | 25.945 [25.338, 27.528] | 27.086 [26.169, 27.161] | 1.044 | 0.970 | 0.998 |
| independent-negation-8 | 11.174 [9.959, 11.929] | 8.848 [8.735, 9.963] | 0.792 | 2.367 | 1.856 |
| independent-negation-10 | 24.575 [23.759, 24.728] | 22.824 [22.554, 23.583] | 0.929 | 5.212 | 5.140 |
| independent-negation-aggregate-16 | 21.375 [19.913, 23.657] | 22.900 [22.410, 22.907] | 1.071 | 0.678 | 0.743 |
| disjunction-12 | 23.507 [22.501, 24.233] | 24.191 [23.532, 26.136] | 1.029 | 1.019 | 1.098 |
| ties-50 | 22.872 [21.944, 23.505] | 23.146 [22.447, 23.585] | 1.012 | 1.203 | 1.189 |
| transitive-path-100 | 9.243 [9.156, 10.684] | 9.251 [8.391, 9.313] | 1.001 | 1.041 | 1.043 |
| transitive-path-200 | 21.138 [20.951, 22.105] | 22.461 [22.280, 22.824] | 1.063 | 0.924 | 0.940 |
| transitive-dense-40 | 34.208 [33.819, 38.751] | 36.629 [36.559, 38.053] | 1.071 | 4.932 | 5.519 |
| chain-1000 | 19.524 [17.403, 19.814] | 16.281 [15.195, 18.465] | 0.834 | 2.517 | 2.112 |
| chain-2000 | 38.051 [36.212, 38.797] | 28.131 [27.067, 28.178] | 0.739 | 3.155 | 2.798 |
| chain-arithmetic-1000 | 10.008 [8.725, 11.095] | 12.176 [8.739, 12.853] | 1.217 | 1.807 | 2.706 |
| stratified-16 | 4.640 [4.503, 4.673] | 3.403 [3.360, 4.478] | 0.733 | 1.035 | 0.770 |
| producer-chain-700 | 27.396 [26.822, 28.332] | 20.398 [19.343, 23.714] | 0.745 | 2.508 | 2.086 |
| latin-square-5 | 14.434 [13.791, 15.488] | 15.368 [14.351, 15.456] | 1.065 | 0.859 | 0.977 |
| planning-14 | 38.328 [37.596, 39.569] | 44.338 [43.232, 50.244] | 1.157 | 2.448 | 2.736 |
| n-queens/variant-01 8→10 | 27.512 [27.147, 28.536] | 27.764 [27.355, 29.728] | 1.009 | 0.822 | 0.891 |
| n-queens/variant-01 8→11 | 89.018 [87.719, 93.563] | 97.357 [95.397, 97.425] | 1.094 | 0.482 | 0.530 |
| n-queens/variant-04 8→11 | 82.508 [82.372, 85.034] | 85.122 [84.762, 85.367] | 1.032 | 0.450 | 0.481 |
| send-money/send-money | 13.076 [13.051, 14.121] | 14.214 [13.469, 14.868] | 1.087 | 0.863 | 0.875 |
| variant-04/05-larger-mix | 35.336 [35.290, 38.987] | 35.299 [34.460, 38.292] | 0.999 | 0.235 | 0.231 |

Reference wall time, ms, same notation.

| Cell | before | after |
|---|---:|---:|
| independent-choice-12 | 6.757 [6.002, 6.862] | 6.852 [6.812, 7.090] |
| independent-choice-16 | 26.746 [26.698, 27.387] | 27.136 [26.018, 27.186] |
| independent-negation-8 | 4.722 [4.486, 7.729] | 4.767 [4.528, 5.450] |
| independent-negation-10 | 4.715 [3.431, 5.790] | 4.441 [4.438, 4.480] |
| independent-negation-aggregate-16 | 31.522 [30.598, 33.846] | 30.815 [29.462, 31.398] |
| disjunction-12 | 23.066 [21.978, 23.935] | 22.036 [21.535, 23.840] |
| ties-50 | 19.011 [18.036, 19.028] | 19.472 [17.900, 20.030] |
| transitive-path-100 | 8.882 [8.826, 8.960] | 8.866 [8.815, 8.892] |
| transitive-path-200 | 22.866 [21.620, 24.038] | 23.887 [22.684, 24.183] |
| transitive-dense-40 | 6.935 [6.622, 7.691] | 6.637 [6.608, 6.689] |
| chain-1000 | 7.756 [7.706, 7.853] | 7.709 [6.520, 7.979] |
| chain-2000 | 12.059 [10.986, 12.120] | 10.053 [9.918, 11.010] |
| chain-arithmetic-1000 | 5.539 [4.432, 7.799] | 4.499 [4.379, 8.794] |
| stratified-16 | 4.483 [3.390, 5.422] | 4.422 [4.340, 4.482] |
| producer-chain-700 | 10.922 [9.754, 10.977] | 9.780 [8.640, 9.781] |
| latin-square-5 | 16.811 [16.580, 16.822] | 15.726 [14.452, 15.729] |
| planning-14 | 15.656 [15.584, 16.465] | 16.206 [14.777, 19.205] |
| n-queens/variant-01 8→10 | 33.456 [31.262, 34.598] | 31.162 [30.881, 32.354] |
| n-queens/variant-01 8→11 | 184.715 [183.536, 187.219] | 183.744 [183.350, 184.240] |
| n-queens/variant-04 8→11 | 183.243 [181.377, 183.272] | 177.075 [174.702, 182.430] |
| send-money/send-money | 15.154 [15.115, 16.200] | 16.239 [14.069, 17.479] |
| variant-04/05-larger-mix | 150.346 [149.958, 164.146] | 152.638 [151.297, 156.539] |

Counters of report after: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 227172 | 4.043 |
| independent-choice-16 | 0 | 2584 | 2584 | 2117111 | 24.351 |
| independent-negation-8 | 0 | 55 | 55 | 28784 | 6.344 |
| independent-negation-10 | 0 | 144 | 144 | 91687 | 19.700 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 170945 | 19.032 |
| disjunction-12 | 0 | 4096 | 4096 | 2029095 | 20.651 |
| ties-50 | 0 | 1225 | 1225 | 901852 | 19.038 |
| transitive-path-100 | 0 | 1 | 1 | 111564 | 6.394 |
| transitive-path-200 | 0 | 1 | 1 | 664776 | 18.449 |
| transitive-dense-40 | 0 | 1 | 1 | 770070 | 31.919 |
| chain-1000 | 0 | 1 | 1 | 211573 | 11.705 |
| chain-2000 | 0 | 1 | 1 | 736073 | 21.939 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 82123 | 6.100 |
| stratified-16 | 0 | 1 | 1 | 1669 | 0.983 |
| producer-chain-700 | 0 | 1 | 1 | 48256 | 16.073 |
| latin-square-5 | 0 | 1344 | 1344 | 791445 | 11.635 |
| planning-14 | 0 | 3432 | 3432 | 2522390 | 40.631 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 3452280 | 24.079 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 16394772 | 92.630 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 15197393 | 81.000 |
| send-money/send-money | 0 | 1 | 1 | 409024 | 9.485 |
| variant-04/05-larger-mix | 0 | 1176 | 1179 | 2118520 | 31.540 |

Against the reference: report before, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 10 of 22 cells where both passed (45.4%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 35.336 | 150.346 | 0.235 | 3.337 | 35.674 | 13.449 | 88.000 | 58.000 |
| n-queens/variant-04 8→11 | 82.508 | 183.243 | 0.450 | 1.483 | 233.803 | 18.890 | 1.000 | 177.000 |
| n-queens/variant-01 8→11 | 89.018 | 184.715 | 0.482 | 5.829 | 231.205 | 28.604 | 2.000 | 179.000 |
| independent-negation-aggregate-16 | 21.375 | 31.522 | 0.678 | 0.230 | 3.951 | 4.141 | 1.000 | 26.000 |
| n-queens/variant-01 8→10 | 27.512 | 33.456 | 0.822 | 4.719 | 48.042 | 6.411 | 3.000 | 26.000 |
| latin-square-5 | 14.434 | 16.811 | 0.859 | 1.243 | 12.403 | 10.247 | 1.000 | 11.000 |
| send-money/send-money | 13.076 | 15.154 | 0.863 | 2.539 | 6.724 | 0.301 | 9.000 | 1.000 |
| transitive-path-200 | 21.138 | 22.866 | 0.924 | n/a | 0.009 | 6.672 | 13.000 | 4.000 |
| independent-choice-16 | 25.945 | 26.746 | 0.970 | n/a | 2.742 | 11.292 | 0.000 | 21.000 |
| independent-choice-12 | 6.682 | 6.757 | 0.989 | n/a | 0.414 | 2.026 | 0.000 | 2.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| disjunction-12 | 23.507 | 23.066 | 1.019 | 0.138 | 2.687 | 35.549 | 1.000 | 17.000 |
| stratified-16 | 4.640 | 4.483 | 1.035 | n/a | 0.228 | 0.173 | 0.000 | 0.000 |
| transitive-path-100 | 9.243 | 8.882 | 1.041 | n/a | 0.007 | 1.919 | 3.000 | 1.000 |
| ties-50 | 22.872 | 19.011 | 1.203 | 0.351 | 13.403 | 3.070 | 1.000 | 13.000 |
| chain-arithmetic-1000 | 10.008 | 5.539 | 1.807 | 3.711 | 0.786 | 0.154 | 1.000 | 0.000 |
| independent-negation-8 | 11.174 | 4.722 | 2.367 | n/a | 6.394 | 0.559 | 0.000 | 0.000 |
| planning-14 | 38.328 | 15.656 | 2.448 | 3.141 | 40.217 | 17.695 | 1.000 | 11.000 |
| producer-chain-700 | 27.396 | 10.922 | 2.508 | n/a | 0.013 | 3.005 | 6.000 | 0.000 |
| chain-1000 | 19.524 | 7.756 | 2.517 | n/a | 0.008 | 1.871 | 3.000 | 0.000 |
| chain-2000 | 38.051 | 12.059 | 3.155 | n/a | 0.010 | 4.439 | 6.000 | 0.000 |
| transitive-dense-40 | 34.208 | 6.935 | 4.932 | 16.398 | 7.731 | 1.341 | 2.000 | 0.000 |
| independent-negation-10 | 24.575 | 4.715 | 5.212 | n/a | 18.790 | 1.229 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 15.7 | 23.6 | n/a |
| n-queens/variant-04 8→11 | 12.7 | 11.7 | n/a |
| n-queens/variant-01 8→11 | 13.8 | 11.5 | n/a |
| independent-negation-aggregate-16 | 10.7 | 10.3 | n/a |
| n-queens/variant-01 8→10 | 12.8 | 10.6 | n/a |
| latin-square-5 | 11.4 | 10.8 | n/a |
| send-money/send-money | 14.3 | 12.7 | n/a |
| transitive-path-200 | 14.6 | 13.4 | n/a |
| independent-choice-16 | 8.4 | 10.3 | n/a |
| independent-choice-12 | 8.6 | 10.3 | n/a |
| disjunction-12 | 10.3 | 10.3 | n/a |
| stratified-16 | 8.6 | 10.4 | n/a |
| transitive-path-100 | 10.1 | 11.0 | n/a |
| ties-50 | 12.5 | 10.5 | n/a |
| chain-arithmetic-1000 | 11.6 | 10.5 | n/a |
| independent-negation-8 | 8.3 | 10.2 | n/a |
| planning-14 | 11.5 | 10.5 | n/a |
| producer-chain-700 | 24.1 | 12.6 | n/a |
| chain-1000 | 14.3 | 10.7 | n/a |
| chain-2000 | 21.2 | 11.1 | n/a |
| transitive-dense-40 | 21.9 | 10.7 | n/a |
| independent-negation-10 | 8.5 | 10.2 | n/a |

Against the reference: report after, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 10 of 22 cells where both passed (45.4%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 35.299 | 152.638 | 0.231 | 3.402 | 34.416 | 12.406 | 88.000 | 60.000 |
| n-queens/variant-04 8→11 | 85.122 | 177.075 | 0.481 | 1.500 | 235.088 | 17.984 | 1.000 | 171.000 |
| n-queens/variant-01 8→11 | 97.357 | 183.744 | 0.530 | 5.658 | 236.487 | 28.600 | 2.000 | 177.000 |
| independent-negation-aggregate-16 | 22.900 | 30.815 | 0.743 | 0.230 | 3.872 | 3.982 | 1.000 | 26.000 |
| stratified-16 | 3.403 | 4.422 | 0.770 | n/a | 0.222 | 0.177 | 0.000 | 0.000 |
| send-money/send-money | 14.214 | 16.239 | 0.875 | 2.658 | 7.162 | 0.327 | 10.000 | 1.000 |
| n-queens/variant-01 8→10 | 27.764 | 31.162 | 0.891 | 3.904 | 49.120 | 6.234 | 2.000 | 25.000 |
| transitive-path-200 | 22.461 | 23.887 | 0.940 | n/a | 0.004 | 7.390 | 13.000 | 5.000 |
| latin-square-5 | 15.368 | 15.726 | 0.977 | 1.238 | 12.892 | 11.193 | 1.000 | 10.000 |
| independent-choice-16 | 27.086 | 27.136 | 0.998 | n/a | 2.651 | 11.101 | 1.000 | 22.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 7.027 | 6.852 | 1.026 | n/a | 0.403 | 2.073 | 0.000 | 2.000 |
| transitive-path-100 | 9.251 | 8.866 | 1.043 | n/a | 0.007 | 2.047 | 3.000 | 1.000 |
| disjunction-12 | 24.191 | 22.036 | 1.098 | 0.136 | 2.893 | 38.133 | 0.000 | 18.000 |
| ties-50 | 23.146 | 19.472 | 1.189 | 0.338 | 14.084 | 2.955 | 1.000 | 14.000 |
| independent-negation-8 | 8.848 | 4.767 | 1.856 | n/a | 5.267 | 0.516 | 0.000 | 0.000 |
| producer-chain-700 | 20.398 | 9.780 | 2.086 | n/a | 0.011 | 3.101 | 5.000 | 0.000 |
| chain-1000 | 16.281 | 7.709 | 2.112 | n/a | 0.009 | 2.436 | 3.000 | 0.000 |
| chain-arithmetic-1000 | 12.176 | 4.499 | 2.706 | 3.545 | 0.847 | 0.144 | 1.000 | 0.000 |
| planning-14 | 44.338 | 16.206 | 2.736 | 3.073 | 41.531 | 18.035 | 1.000 | 10.000 |
| chain-2000 | 28.131 | 10.053 | 2.798 | n/a | 0.011 | 4.213 | 6.000 | 0.000 |
| independent-negation-10 | 22.824 | 4.441 | 5.140 | n/a | 17.423 | 1.160 | 1.000 | 0.000 |
| transitive-dense-40 | 36.629 | 6.637 | 5.519 | 18.861 | 7.984 | 1.358 | 2.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 17.5 | 23.5 | n/a |
| n-queens/variant-04 8→11 | 13.5 | 11.8 | n/a |
| n-queens/variant-01 8→11 | 15.6 | 11.8 | n/a |
| independent-negation-aggregate-16 | 10.8 | 10.4 | n/a |
| stratified-16 | 8.5 | 10.2 | n/a |
| send-money/send-money | 15.0 | 12.7 | n/a |
| n-queens/variant-01 8→10 | 13.6 | 10.8 | n/a |
| transitive-path-200 | 14.2 | 13.3 | n/a |
| latin-square-5 | 12.1 | 10.5 | n/a |
| independent-choice-16 | 8.4 | 10.3 | n/a |
| independent-choice-12 | 8.5 | 10.2 | n/a |
| transitive-path-100 | 9.7 | 11.0 | n/a |
| disjunction-12 | 10.7 | 10.2 | n/a |
| ties-50 | 13.2 | 10.5 | n/a |
| independent-negation-8 | 8.0 | 10.3 | n/a |
| producer-chain-700 | 23.7 | 12.7 | n/a |
| chain-1000 | 14.3 | 10.7 | n/a |
| chain-arithmetic-1000 | 11.4 | 10.3 | n/a |
| planning-14 | 11.2 | 10.2 | n/a |
| chain-2000 | 20.9 | 11.1 | n/a |
| independent-negation-10 | 8.5 | 10.4 | n/a |
| transitive-dense-40 | 21.8 | 10.7 | n/a |
