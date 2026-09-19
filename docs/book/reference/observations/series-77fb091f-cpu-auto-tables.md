Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | before | after | after/before | before/reference | after/reference |
|---|---:|---:|---:|---:|---:|
| independent-choice-12 | 6.796 [6.715, 7.845] | 7.068 [6.656, 7.706] | 1.040 | 0.968 | 1.045 |
| independent-choice-16 | 24.982 [22.808, 26.216] | 25.610 [24.954, 28.288] | 1.025 | 0.905 | 0.944 |
| independent-negation-8 | 10.140 [9.914, 10.790] | 9.844 [9.093, 10.068] | 0.971 | 2.247 | 2.145 |
| independent-negation-10 | 23.284 [22.593, 23.650] | 24.532 [23.455, 26.748] | 1.054 | 4.217 | 4.458 |
| independent-negation-aggregate-16 | 22.025 [20.593, 23.064] | 20.248 [20.007, 21.733] | 0.919 | 0.701 | 0.684 |
| disjunction-12 | 22.830 [22.790, 23.385] | 22.029 [21.766, 22.100] | 0.965 | 1.055 | 1.055 |
| ties-50 | 21.920 [21.726, 21.992] | 22.115 [21.955, 22.245] | 1.009 | 1.261 | 1.269 |
| transitive-path-100 | 9.489 [9.452, 10.198] | 8.210 [8.012, 8.521] | 0.865 | 1.077 | 1.008 |
| transitive-path-200 | 20.977 [20.798, 22.283] | 20.353 [19.801, 25.737] | 0.970 | 0.958 | 0.914 |
| transitive-dense-40 | 33.358 [33.129, 35.018] | 33.274 [33.040, 33.283] | 0.997 | 5.033 | 5.016 |
| chain-1000 | 17.469 [17.387, 18.364] | 15.367 [15.184, 16.254] | 0.880 | 2.286 | 2.276 |
| chain-2000 | 33.991 [33.326, 35.682] | 28.682 [28.184, 31.730] | 0.844 | 3.455 | 2.618 |
| chain-arithmetic-1000 | 11.151 [9.983, 11.216] | 10.911 [10.416, 11.240] | 0.978 | 2.513 | 2.410 |
| stratified-16 | 3.414 [3.388, 4.398] | 3.537 [3.409, 4.484] | 1.036 | 0.767 | 0.787 |
| producer-chain-700 | 25.689 [24.696, 28.454] | 21.575 [21.468, 22.663] | 0.840 | 2.643 | 2.177 |
| latin-square-5 | 14.287 [13.434, 15.325] | 14.300 [13.380, 14.810] | 1.001 | 0.851 | 0.915 |
| planning-14 | 37.575 [37.521, 38.097] | 36.541 [36.020, 36.915] | 0.972 | 2.378 | 2.295 |
| n-queens/variant-01 8→10 | 26.334 [25.547, 26.468] | 26.916 [26.803, 27.329] | 1.022 | 0.872 | 0.894 |
| n-queens/variant-01 8→11 | 90.391 [87.401, 94.089] | 89.922 [88.196, 92.529] | 0.995 | 0.492 | 0.502 |
| n-queens/variant-04 8→11 | 81.338 [80.806, 81.343] | 81.734 [80.551, 82.015] | 1.005 | 0.449 | 0.465 |
| send-money/send-money | 13.598 [12.274, 14.443] | 14.057 [13.336, 14.744] | 1.034 | 0.964 | 0.913 |
| variant-04/05-larger-mix | 34.016 [31.993, 37.019] | 32.739 [32.562, 34.248] | 0.962 | 0.223 | 0.223 |

Reference wall time, ms, same notation.

| Cell | before | after |
|---|---:|---:|
| independent-choice-12 | 7.021 [6.093, 7.213] | 6.765 [6.702, 6.886] |
| independent-choice-16 | 27.614 [27.159, 28.578] | 27.121 [26.270, 27.272] |
| independent-negation-8 | 4.513 [4.440, 4.566] | 4.589 [4.569, 5.811] |
| independent-negation-10 | 5.521 [5.485, 6.800] | 5.503 [4.544, 5.583] |
| independent-negation-aggregate-16 | 31.412 [31.285, 33.000] | 29.584 [29.126, 31.281] |
| disjunction-12 | 21.643 [20.712, 22.492] | 20.873 [20.626, 20.976] |
| ties-50 | 17.382 [16.716, 19.464] | 17.423 [16.956, 18.487] |
| transitive-path-100 | 8.811 [7.763, 11.102] | 8.147 [7.994, 8.875] |
| transitive-path-200 | 21.898 [21.566, 24.051] | 22.279 [21.641, 22.736] |
| transitive-dense-40 | 6.628 [6.610, 6.845] | 6.634 [6.632, 6.673] |
| chain-1000 | 7.642 [5.470, 7.873] | 6.751 [6.626, 7.660] |
| chain-2000 | 9.837 [9.827, 11.346] | 10.957 [9.812, 10.971] |
| chain-arithmetic-1000 | 4.438 [3.301, 4.662] | 4.528 [4.485, 5.555] |
| stratified-16 | 4.449 [4.387, 5.480] | 4.495 [4.483, 4.517] |
| producer-chain-700 | 9.721 [8.689, 9.852] | 9.910 [9.716, 10.809] |
| latin-square-5 | 16.781 [16.012, 16.897] | 15.634 [14.503, 15.683] |
| planning-14 | 15.803 [15.752, 17.331] | 15.922 [15.207, 16.725] |
| n-queens/variant-01 8→10 | 30.183 [30.126, 32.850] | 30.097 [30.014, 30.125] |
| n-queens/variant-01 8→11 | 183.889 [183.517, 184.613] | 179.241 [176.747, 179.245] |
| n-queens/variant-04 8→11 | 181.055 [180.077, 181.650] | 175.863 [174.365, 175.921] |
| send-money/send-money | 14.110 [14.103, 14.125] | 15.395 [14.063, 15.462] |
| variant-04/05-larger-mix | 152.244 [149.670, 152.438] | 146.871 [145.972, 149.975] |

Counters of report after: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 227172 | 3.920 |
| independent-choice-16 | 0 | 2584 | 2584 | 2117111 | 22.477 |
| independent-negation-8 | 0 | 55 | 55 | 28784 | 7.059 |
| independent-negation-10 | 0 | 144 | 144 | 91687 | 21.822 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 170945 | 17.063 |
| disjunction-12 | 0 | 4096 | 4096 | 2029095 | 18.243 |
| ties-50 | 0 | 1225 | 1225 | 900812 | 18.322 |
| transitive-path-100 | 0 | 1 | 1 | 111564 | 5.865 |
| transitive-path-200 | 0 | 1 | 1 | 664776 | 17.053 |
| transitive-dense-40 | 0 | 1 | 1 | 770070 | 28.641 |
| chain-1000 | 0 | 1 | 1 | 211573 | 10.875 |
| chain-2000 | 0 | 1 | 1 | 736073 | 22.882 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 82123 | 6.945 |
| stratified-16 | 0 | 1 | 1 | 1669 | 0.987 |
| producer-chain-700 | 0 | 1 | 1 | 48256 | 16.428 |
| latin-square-5 | 0 | 1344 | 1344 | 791445 | 10.007 |
| planning-14 | 0 | 3432 | 3432 | 2522390 | 32.659 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 3452280 | 23.275 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 16394772 | 85.596 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 15197393 | 77.912 |
| send-money/send-money | 0 | 1 | 1 | 409024 | 9.187 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 2065554 | 29.311 |

Against the reference: report before, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 11 of 22 cells where both passed (50.0%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 34.016 | 152.244 | 0.223 | 3.237 | 33.269 | 12.630 | 86.000 | 59.000 |
| n-queens/variant-04 8→11 | 81.338 | 181.055 | 0.449 | 1.484 | 230.441 | 18.366 | 1.000 | 174.000 |
| n-queens/variant-01 8→11 | 90.391 | 183.889 | 0.492 | 6.888 | 224.697 | 28.233 | 2.000 | 176.000 |
| independent-negation-aggregate-16 | 22.025 | 31.412 | 0.701 | 0.238 | 4.064 | 4.337 | 0.000 | 27.000 |
| stratified-16 | 3.414 | 4.449 | 0.767 | n/a | 0.239 | 0.193 | 0.000 | 0.000 |
| latin-square-5 | 14.287 | 16.781 | 0.851 | 1.261 | 12.240 | 10.009 | 1.000 | 11.000 |
| n-queens/variant-01 8→10 | 26.334 | 30.183 | 0.872 | 3.861 | 47.714 | 6.208 | 2.000 | 24.000 |
| independent-choice-16 | 24.982 | 27.614 | 0.905 | n/a | 2.743 | 10.542 | 0.000 | 23.000 |
| transitive-path-200 | 20.977 | 21.898 | 0.958 | n/a | 0.007 | 7.070 | 13.000 | 4.000 |
| send-money/send-money | 13.598 | 14.110 | 0.964 | 2.536 | 6.473 | 0.277 | 9.000 | 1.000 |
| independent-choice-12 | 6.796 | 7.021 | 0.968 | n/a | 0.444 | 2.342 | 1.000 | 2.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| disjunction-12 | 22.830 | 21.643 | 1.055 | 0.134 | 2.697 | 36.291 | 0.000 | 17.000 |
| transitive-path-100 | 9.489 | 8.811 | 1.077 | n/a | 0.007 | 2.330 | 3.000 | 1.000 |
| ties-50 | 21.920 | 17.382 | 1.261 | 0.345 | 13.508 | 2.957 | 1.000 | 13.000 |
| independent-negation-8 | 10.140 | 4.513 | 2.247 | n/a | 5.382 | 0.521 | 0.000 | 0.000 |
| chain-1000 | 17.469 | 7.642 | 2.286 | n/a | 0.008 | 1.987 | 3.000 | 0.000 |
| planning-14 | 37.575 | 15.803 | 2.378 | 3.033 | 40.961 | 18.040 | 1.000 | 11.000 |
| chain-arithmetic-1000 | 11.151 | 4.438 | 2.513 | 4.198 | 0.794 | 0.157 | 1.000 | 0.000 |
| producer-chain-700 | 25.689 | 9.721 | 2.643 | n/a | 0.008 | 3.043 | 5.000 | 0.000 |
| chain-2000 | 33.991 | 9.837 | 3.455 | n/a | 0.012 | 5.772 | 6.000 | 0.000 |
| independent-negation-10 | 23.284 | 5.521 | 4.217 | n/a | 18.375 | 1.297 | 1.000 | 0.000 |
| transitive-dense-40 | 33.358 | 6.628 | 5.033 | 16.164 | 7.661 | 1.285 | 2.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 16.0 | 23.7 | n/a |
| n-queens/variant-04 8→11 | 12.7 | 11.9 | n/a |
| n-queens/variant-01 8→11 | 13.9 | 11.7 | n/a |
| independent-negation-aggregate-16 | 10.7 | 10.3 | n/a |
| stratified-16 | 8.9 | 10.2 | n/a |
| latin-square-5 | 11.5 | 10.6 | n/a |
| n-queens/variant-01 8→10 | 12.8 | 10.9 | n/a |
| independent-choice-16 | 8.5 | 10.3 | n/a |
| transitive-path-200 | 14.3 | 13.6 | n/a |
| send-money/send-money | 14.0 | 12.8 | n/a |
| independent-choice-12 | 8.5 | 10.2 | n/a |
| disjunction-12 | 10.5 | 10.2 | n/a |
| transitive-path-100 | 10.0 | 10.9 | n/a |
| ties-50 | 12.7 | 10.5 | n/a |
| independent-negation-8 | 8.4 | 10.3 | n/a |
| chain-1000 | 14.4 | 10.7 | n/a |
| planning-14 | 10.9 | 10.5 | n/a |
| chain-arithmetic-1000 | 11.7 | 10.5 | n/a |
| producer-chain-700 | 24.1 | 12.7 | n/a |
| chain-2000 | 21.4 | 11.1 | n/a |
| independent-negation-10 | 8.6 | 10.3 | n/a |
| transitive-dense-40 | 22.1 | 10.5 | n/a |

Against the reference: report after, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 10 of 22 cells where both passed (45.4%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 32.739 | 146.871 | 0.223 | 3.264 | 32.074 | 10.783 | 84.000 | 57.000 |
| n-queens/variant-04 8→11 | 81.734 | 175.863 | 0.465 | 1.466 | 231.173 | 16.397 | 1.000 | 169.000 |
| n-queens/variant-01 8→11 | 89.922 | 179.241 | 0.502 | 5.562 | 225.249 | 24.693 | 2.000 | 172.000 |
| independent-negation-aggregate-16 | 20.248 | 29.584 | 0.684 | 0.217 | 3.919 | 3.936 | 1.000 | 25.000 |
| stratified-16 | 3.537 | 4.495 | 0.787 | n/a | 0.230 | 0.174 | 0.000 | 0.000 |
| n-queens/variant-01 8→10 | 26.916 | 30.097 | 0.894 | 3.852 | 50.057 | 5.558 | 2.000 | 24.000 |
| send-money/send-money | 14.057 | 15.395 | 0.913 | 2.553 | 6.238 | 0.295 | 9.000 | 1.000 |
| transitive-path-200 | 20.353 | 22.279 | 0.914 | n/a | 0.004 | 7.256 | 13.000 | 4.000 |
| latin-square-5 | 14.300 | 15.634 | 0.915 | 1.216 | 12.475 | 8.941 | 1.000 | 10.000 |
| independent-choice-16 | 25.610 | 27.121 | 0.944 | n/a | 2.856 | 10.681 | 0.000 | 22.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| transitive-path-100 | 8.210 | 8.147 | 1.008 | n/a | 0.005 | 1.919 | 3.000 | 1.000 |
| independent-choice-12 | 7.068 | 6.765 | 1.045 | n/a | 0.518 | 2.071 | 1.000 | 2.000 |
| disjunction-12 | 22.029 | 20.873 | 1.055 | 0.122 | 2.713 | 36.900 | 0.000 | 16.000 |
| ties-50 | 22.115 | 17.423 | 1.269 | 0.329 | 12.797 | 2.561 | 0.000 | 13.000 |
| independent-negation-8 | 9.844 | 4.589 | 2.145 | n/a | 5.855 | 0.502 | 0.000 | 0.000 |
| producer-chain-700 | 21.575 | 9.910 | 2.177 | n/a | 0.007 | 2.885 | 5.000 | 0.000 |
| chain-1000 | 15.367 | 6.751 | 2.276 | n/a | 0.006 | 2.142 | 3.000 | 0.000 |
| planning-14 | 36.541 | 15.922 | 2.295 | 2.955 | 40.819 | 15.654 | 1.000 | 11.000 |
| chain-arithmetic-1000 | 10.911 | 4.528 | 2.410 | 3.857 | 0.838 | 0.150 | 1.000 | 0.000 |
| chain-2000 | 28.682 | 10.957 | 2.618 | n/a | 0.012 | 5.365 | 6.000 | 0.000 |
| independent-negation-10 | 24.532 | 5.503 | 4.458 | n/a | 19.295 | 1.246 | 1.000 | 0.000 |
| transitive-dense-40 | 33.274 | 6.634 | 5.016 | 15.742 | 7.468 | 1.351 | 3.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 17.5 | 23.5 | n/a |
| n-queens/variant-04 8→11 | 13.5 | 11.8 | n/a |
| n-queens/variant-01 8→11 | 15.4 | 11.6 | n/a |
| independent-negation-aggregate-16 | 10.9 | 10.3 | n/a |
| stratified-16 | 8.6 | 10.4 | n/a |
| n-queens/variant-01 8→10 | 13.6 | 10.5 | n/a |
| send-money/send-money | 14.9 | 12.6 | n/a |
| transitive-path-200 | 14.2 | 13.5 | n/a |
| latin-square-5 | 11.9 | 10.5 | n/a |
| independent-choice-16 | 8.6 | 10.3 | n/a |
| transitive-path-100 | 9.8 | 10.9 | n/a |
| independent-choice-12 | 8.4 | 10.2 | n/a |
| disjunction-12 | 10.3 | 10.1 | n/a |
| ties-50 | 13.3 | 10.2 | n/a |
| independent-negation-8 | 8.2 | 10.2 | n/a |
| producer-chain-700 | 23.8 | 12.7 | n/a |
| chain-1000 | 13.9 | 10.6 | n/a |
| planning-14 | 11.1 | 10.5 | n/a |
| chain-arithmetic-1000 | 11.4 | 10.6 | n/a |
| chain-2000 | 20.9 | 11.3 | n/a |
| independent-negation-10 | 8.6 | 10.3 | n/a |
| transitive-dense-40 | 21.9 | 10.7 | n/a |
