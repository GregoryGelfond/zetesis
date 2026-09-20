Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

Formula search method by report: before=default; after=default; clauses=clauses;

| Cell | before | after | clauses | after/before | clauses/after | clauses/before | before/reference | after/reference | clauses/reference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 7.685 [6.741, 7.921] | 6.920 [6.797, 7.689] | 7.712 [6.821, 7.780] | 0.901 | 1.114 | 1.004 | 1.135 | 1.021 | 1.144 |
| independent-choice-16 | 34.170 [33.433, 34.574] | 25.018 [24.557, 25.121] | 24.488 [23.904, 26.162] | 0.732 | 0.979 | 0.717 | 1.349 | 0.933 | 0.910 |
| independent-negation-8 | 16.912 [16.529, 18.303] | 14.011 [13.519, 15.144] | 14.094 [13.487, 15.189] | 0.828 | 1.006 | 0.833 | 3.609 | 3.046 | 3.015 |
| independent-negation-10 | 50.024 [47.948, 52.288] | 38.604 [38.301, 38.938] | 39.494 [39.440, 40.469] | 0.772 | 1.023 | 0.790 | 11.035 | 8.415 | 7.061 |
| independent-negation-aggregate-16 | 20.827 [20.219, 21.210] | 20.930 [20.285, 21.694] | 25.872 [25.428, 26.183] | 1.005 | 1.236 | 1.242 | 0.678 | 0.680 | 0.825 |
| disjunction-12 | 23.290 [22.796, 23.851] | 22.985 [22.477, 23.216] | 46.224 [41.374, 46.508] | 0.987 | 2.011 | 1.985 | 1.009 | 0.998 | 2.120 |
| ties-50 | 21.712 [21.363, 21.893] | 21.728 [21.096, 22.488] | 26.624 [25.187, 27.191] | 1.001 | 1.225 | 1.226 | 1.209 | 1.215 | 1.375 |
| transitive-path-100 | 13.461 [12.290, 15.322] | 9.485 [9.086, 10.307] | 9.507 [9.107, 9.624] | 0.705 | 1.002 | 0.706 | 1.527 | 1.072 | 1.061 |
| transitive-path-200 | 32.211 [31.607, 33.100] | 23.051 [21.746, 23.144] | 21.935 [21.450, 24.150] | 0.716 | 0.952 | 0.681 | 1.443 | 1.058 | 0.958 |
| transitive-dense-40 | 32.861 [32.316, 35.468] | 33.007 [32.358, 36.505] | 33.731 [31.734, 34.753] | 1.004 | 1.022 | 1.026 | 4.961 | 4.956 | 5.050 |
| chain-1000 | 18.291 [17.304, 20.634] | 18.514 [17.335, 18.841] | 20.700 [19.401, 21.067] | 1.012 | 1.118 | 1.132 | 2.773 | 2.400 | 3.114 |
| chain-2000 | 35.545 [33.327, 36.536] | 33.405 [32.447, 35.587] | 33.428 [32.465, 34.394] | 0.940 | 1.001 | 0.940 | 3.221 | 3.045 | 3.389 |
| chain-arithmetic-1000 | 10.917 [9.837, 10.965] | 9.930 [9.795, 10.910] | 9.892 [9.832, 10.974] | 0.910 | 0.996 | 0.906 | 2.383 | 2.226 | 2.200 |
| stratified-16 | 4.467 [4.436, 4.486] | 3.442 [3.392, 4.457] | 4.471 [3.416, 4.662] | 0.770 | 1.299 | 1.001 | 1.294 | 0.780 | 1.028 |
| producer-chain-700 | 24.679 [24.626, 24.718] | 25.905 [25.664, 27.034] | 24.704 [23.539, 25.745] | 1.050 | 0.954 | 1.001 | 2.540 | 2.651 | 2.833 |
| latin-square-5 | 13.767 [13.515, 18.830] | 13.934 [13.839, 14.001] | 35.780 [35.543, 36.198] | 1.012 | 2.568 | 2.599 | 0.875 | 0.951 | 2.272 |
| planning-14 | 37.175 [35.994, 38.792] | 37.096 [36.772, 37.232] | 84.352 [83.954, 85.703] | 0.998 | 2.274 | 2.269 | 2.403 | 2.365 | 5.572 |
| n-queens/variant-01 8→10 | 26.920 [26.079, 27.747] | 26.770 [25.964, 28.126] | 59.278 [58.811, 59.766] | 0.994 | 2.214 | 2.202 | 0.877 | 0.880 | 1.961 |
| n-queens/variant-01 8→11 | 87.706 [87.341, 90.965] | 89.385 [87.162, 92.197] | 271.189 [262.770, 446.536] | 1.019 | 3.034 | 3.092 | 0.484 | 0.496 | 1.497 |
| n-queens/variant-04 8→11 | 78.611 [78.299, 86.201] | 80.857 [79.582, 80.932] | 163.227 [161.402, 165.605] | 1.029 | 2.019 | 2.076 | 0.441 | 0.454 | 0.912 |
| send-money/send-money | 13.414 [13.140, 13.506] | 12.969 [12.857, 13.202] | 10.964 [10.835, 13.231] | 0.967 | 0.845 | 0.817 | 0.945 | 0.927 | 0.725 |
| variant-04/05-larger-mix | 34.218 [33.890, 35.568] | 33.028 [32.452, 33.838] | 94.092 [93.563, 95.854] | 0.965 | 2.849 | 2.750 | 0.231 | 0.224 | 0.635 |

Reference wall time, ms, same notation.

| Cell | before | after | clauses |
|---|---:|---:|---:|
| independent-choice-12 | 6.773 [6.706, 6.916] | 6.778 [6.698, 7.118] | 6.738 [5.741, 6.804] |
| independent-choice-16 | 25.331 [24.839, 28.090] | 26.826 [25.596, 27.302] | 26.902 [26.618, 27.054] |
| independent-negation-8 | 4.686 [3.440, 5.552] | 4.599 [4.441, 4.624] | 4.674 [3.361, 5.540] |
| independent-negation-10 | 4.533 [4.506, 5.574] | 4.587 [4.544, 6.630] | 5.593 [4.529, 6.697] |
| independent-negation-aggregate-16 | 30.708 [30.516, 31.159] | 30.787 [29.520, 31.236] | 31.360 [30.223, 32.492] |
| disjunction-12 | 23.091 [22.090, 23.187] | 23.020 [20.033, 23.721] | 21.800 [19.113, 22.234] |
| ties-50 | 17.960 [17.836, 18.976] | 17.879 [17.762, 18.594] | 19.361 [17.537, 19.543] |
| transitive-path-100 | 8.816 [7.739, 11.059] | 8.846 [7.798, 9.861] | 8.964 [7.953, 9.929] |
| transitive-path-200 | 22.321 [21.681, 23.765] | 21.791 [21.563, 22.914] | 22.896 [21.629, 24.140] |
| transitive-dense-40 | 6.624 [6.619, 6.663] | 6.659 [6.644, 6.693] | 6.680 [5.552, 6.683] |
| chain-1000 | 6.595 [6.525, 8.908] | 7.715 [7.665, 7.897] | 6.647 [6.516, 7.691] |
| chain-2000 | 11.037 [10.868, 12.088] | 10.972 [9.733, 10.974] | 9.865 [9.773, 9.910] |
| chain-arithmetic-1000 | 4.581 [4.453, 5.459] | 4.461 [4.424, 4.583] | 4.496 [4.416, 4.664] |
| stratified-16 | 3.453 [3.348, 5.452] | 4.415 [4.400, 4.563] | 4.351 [3.341, 4.660] |
| producer-chain-700 | 9.716 [9.026, 10.784] | 9.772 [8.705, 11.127] | 8.721 [8.646, 9.723] |
| latin-square-5 | 15.725 [14.678, 15.732] | 14.644 [14.512, 15.739] | 15.748 [14.461, 16.726] |
| planning-14 | 15.467 [14.700, 16.596] | 15.686 [15.437, 15.700] | 15.139 [14.854, 15.733] |
| n-queens/variant-01 8→10 | 30.682 [30.142, 31.832] | 30.422 [30.225, 31.240] | 30.227 [30.175, 31.197] |
| n-queens/variant-01 8→11 | 181.346 [178.193, 183.303] | 180.318 [179.277, 181.177] | 181.152 [181.109, 186.472] |
| n-queens/variant-04 8→11 | 178.278 [177.655, 179.439] | 177.937 [177.356, 179.528] | 178.996 [177.340, 180.617] |
| send-money/send-money | 14.189 [12.869, 15.230] | 13.990 [12.903, 15.229] | 15.126 [14.118, 18.514] |
| variant-04/05-larger-mix | 147.917 [147.751, 148.429] | 147.441 [146.857, 147.685] | 148.149 [147.998, 148.461] |

Counters of report clauses: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 227172 | 4.602 |
| independent-choice-16 | 0 | 2584 | 2584 | 2117111 | 21.309 |
| independent-negation-8 | 0 | 55 | 55 | 28784 | 11.067 |
| independent-negation-10 | 0 | 144 | 144 | 91687 | 37.147 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 3137480 | 21.702 |
| disjunction-12 | 0 | 4096 | 4096 | 9004441 | 42.052 |
| ties-50 | 0 | 1225 | 1225 | 4720400 | 23.159 |
| transitive-path-100 | 0 | 1 | 1 | 148047 | 6.500 |
| transitive-path-200 | 0 | 1 | 1 | 804204 | 18.299 |
| transitive-dense-40 | 0 | 1 | 1 | 2173586 | 29.194 |
| chain-1000 | 0 | 1 | 1 | 211573 | 16.181 |
| chain-2000 | 0 | 1 | 1 | 736073 | 27.104 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 193171 | 6.266 |
| stratified-16 | 0 | 1 | 1 | 1681 | 1.273 |
| producer-chain-700 | 0 | 1 | 1 | 48256 | 19.938 |
| latin-square-5 | 0 | 1344 | 1344 | 8125616 | 32.039 |
| planning-14 | 0 | 3432 | 3432 | 19467097 | 80.826 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 11383348 | 54.992 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 57156138 | 266.996 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 35658773 | 159.039 |
| send-money/send-money | 0 | 1 | 1 | 349198 | 7.178 |
| variant-04/05-larger-mix | 0 | 1176 | 1208 | 23292550 | 89.991 |

Against the reference: report before, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 7 of 22 cells where both passed (31.8%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 34.218 | 147.917 | 0.231 | 3.280 | 33.365 | 12.432 | 84.000 | 58.000 |
| n-queens/variant-04 8→11 | 78.611 | 178.278 | 0.441 | 1.490 | 224.445 | 17.203 | 1.000 | 173.000 |
| n-queens/variant-01 8→11 | 87.706 | 181.346 | 0.484 | 5.680 | 224.280 | 26.730 | 2.000 | 174.000 |
| independent-negation-aggregate-16 | 20.827 | 30.708 | 0.678 | 0.218 | 3.855 | 3.930 | 0.000 | 26.000 |
| latin-square-5 | 13.767 | 15.725 | 0.875 | 1.226 | 12.616 | 9.789 | 1.000 | 10.000 |
| n-queens/variant-01 8→10 | 26.920 | 30.682 | 0.877 | 3.891 | 47.240 | 5.896 | 1.000 | 25.000 |
| send-money/send-money | 13.414 | 14.189 | 0.945 | 2.562 | 6.303 | 0.284 | 9.000 | 2.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| disjunction-12 | 23.290 | 23.091 | 1.009 | 0.141 | 2.503 | 34.301 | 1.000 | 18.000 |
| independent-choice-12 | 7.685 | 6.773 | 1.135 | n/a | 0.529 | 2.537 | 0.000 | 2.000 |
| ties-50 | 21.712 | 17.960 | 1.209 | 0.356 | 13.067 | 2.812 | 0.000 | 13.000 |
| stratified-16 | 4.467 | 3.453 | 1.294 | n/a | 0.498 | 0.215 | 0.000 | 0.000 |
| independent-choice-16 | 34.170 | 25.331 | 1.349 | n/a | 2.827 | 19.068 | 1.000 | 21.000 |
| transitive-path-200 | 32.211 | 22.321 | 1.443 | n/a | 0.009 | 18.066 | 13.000 | 4.000 |
| transitive-path-100 | 13.461 | 8.816 | 1.527 | n/a | 0.008 | 4.762 | 4.000 | 1.000 |
| chain-arithmetic-1000 | 10.917 | 4.581 | 2.383 | 4.588 | 0.800 | 0.157 | 1.000 | 0.000 |
| planning-14 | 37.175 | 15.467 | 2.403 | 3.092 | 39.354 | 16.447 | 1.000 | 10.000 |
| producer-chain-700 | 24.679 | 9.716 | 2.540 | n/a | 0.009 | 2.903 | 6.000 | 0.000 |
| chain-1000 | 18.291 | 6.595 | 2.773 | n/a | 0.009 | 2.600 | 3.000 | 0.000 |
| chain-2000 | 35.545 | 11.037 | 3.221 | n/a | 0.008 | 5.492 | 7.000 | 0.000 |
| independent-negation-8 | 16.912 | 4.686 | 3.609 | n/a | 12.174 | 0.755 | 0.000 | 0.000 |
| transitive-dense-40 | 32.861 | 6.624 | 4.961 | 15.759 | 8.055 | 1.328 | 2.000 | 0.000 |
| independent-negation-10 | 50.024 | 4.533 | 11.035 | n/a | 43.427 | 2.192 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 16.0 | 23.6 | n/a |
| n-queens/variant-04 8→11 | 13.0 | 11.6 | n/a |
| n-queens/variant-01 8→11 | 14.1 | 11.8 | n/a |
| independent-negation-aggregate-16 | 10.9 | 10.4 | n/a |
| latin-square-5 | 11.6 | 10.6 | n/a |
| n-queens/variant-01 8→10 | 12.8 | 10.8 | n/a |
| send-money/send-money | 14.4 | 12.6 | n/a |
| disjunction-12 | 10.5 | 10.4 | n/a |
| independent-choice-12 | 8.8 | 10.2 | n/a |
| ties-50 | 13.1 | 10.5 | n/a |
| stratified-16 | 8.8 | 10.1 | n/a |
| independent-choice-16 | 8.6 | 10.3 | n/a |
| transitive-path-200 | 14.5 | 13.4 | n/a |
| transitive-path-100 | 10.5 | 11.0 | n/a |
| chain-arithmetic-1000 | 11.7 | 10.5 | n/a |
| planning-14 | 11.4 | 10.6 | n/a |
| producer-chain-700 | 24.3 | 12.5 | n/a |
| chain-1000 | 14.5 | 10.7 | n/a |
| chain-2000 | 21.0 | 11.2 | n/a |
| independent-negation-8 | 8.4 | 10.1 | n/a |
| transitive-dense-40 | 21.7 | 10.6 | n/a |
| independent-negation-10 | 8.8 | 10.3 | n/a |

Against the reference: report after, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 10 of 22 cells where both passed (45.4%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 33.028 | 147.441 | 0.224 | 3.299 | 32.465 | 11.697 | 84.000 | 58.000 |
| n-queens/variant-04 8→11 | 80.857 | 177.937 | 0.454 | 1.514 | 227.324 | 17.284 | 1.000 | 172.000 |
| n-queens/variant-01 8→11 | 89.385 | 180.318 | 0.496 | 5.679 | 234.643 | 27.660 | 2.000 | 174.000 |
| independent-negation-aggregate-16 | 20.930 | 30.787 | 0.680 | 0.241 | 3.752 | 3.877 | 0.000 | 26.000 |
| stratified-16 | 3.442 | 4.415 | 0.780 | n/a | 0.383 | 0.183 | 0.000 | 0.000 |
| n-queens/variant-01 8→10 | 26.770 | 30.422 | 0.880 | 4.135 | 47.993 | 5.964 | 1.000 | 25.000 |
| send-money/send-money | 12.969 | 13.990 | 0.927 | 2.580 | 6.061 | 0.286 | 9.000 | 1.000 |
| independent-choice-16 | 25.018 | 26.826 | 0.933 | n/a | 2.694 | 11.021 | 0.000 | 22.000 |
| latin-square-5 | 13.934 | 14.644 | 0.951 | 1.226 | 12.480 | 9.754 | 1.000 | 10.000 |
| disjunction-12 | 22.985 | 23.020 | 0.998 | 0.133 | 2.628 | 36.120 | 0.000 | 18.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 6.920 | 6.778 | 1.021 | n/a | 0.473 | 1.940 | 1.000 | 2.000 |
| transitive-path-200 | 23.051 | 21.791 | 1.058 | n/a | 0.007 | 8.289 | 12.000 | 4.000 |
| transitive-path-100 | 9.485 | 8.846 | 1.072 | n/a | 0.009 | 2.221 | 3.000 | 1.000 |
| ties-50 | 21.728 | 17.879 | 1.215 | 0.354 | 13.517 | 2.778 | 1.000 | 13.000 |
| chain-arithmetic-1000 | 9.930 | 4.461 | 2.226 | 3.615 | 0.810 | 0.145 | 1.000 | 0.000 |
| planning-14 | 37.096 | 15.686 | 2.365 | 3.170 | 40.023 | 16.910 | 1.000 | 11.000 |
| chain-1000 | 18.514 | 7.715 | 2.400 | n/a | 0.008 | 1.953 | 4.000 | 0.000 |
| producer-chain-700 | 25.905 | 9.772 | 2.651 | n/a | 0.013 | 2.734 | 5.000 | 0.000 |
| chain-2000 | 33.405 | 10.972 | 3.045 | n/a | 0.009 | 4.298 | 6.000 | 0.000 |
| independent-negation-8 | 14.011 | 4.599 | 3.046 | n/a | 9.931 | 0.547 | 0.000 | 0.000 |
| transitive-dense-40 | 33.007 | 6.659 | 4.956 | 15.820 | 7.707 | 1.262 | 2.000 | 0.000 |
| independent-negation-10 | 38.604 | 4.587 | 8.415 | n/a | 33.382 | 1.506 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 16.0 | 23.5 | n/a |
| n-queens/variant-04 8→11 | 12.5 | 11.6 | n/a |
| n-queens/variant-01 8→11 | 13.9 | 11.6 | n/a |
| independent-negation-aggregate-16 | 10.7 | 10.4 | n/a |
| stratified-16 | 8.7 | 10.5 | n/a |
| n-queens/variant-01 8→10 | 12.9 | 10.7 | n/a |
| send-money/send-money | 14.0 | 12.7 | n/a |
| independent-choice-16 | 8.4 | 10.3 | n/a |
| latin-square-5 | 11.2 | 10.7 | n/a |
| disjunction-12 | 10.8 | 10.2 | n/a |
| independent-choice-12 | 8.6 | 10.1 | n/a |
| transitive-path-200 | 14.7 | 13.4 | n/a |
| transitive-path-100 | 10.2 | 11.1 | n/a |
| ties-50 | 13.0 | 10.5 | n/a |
| chain-arithmetic-1000 | 11.6 | 10.5 | n/a |
| planning-14 | 11.3 | 10.3 | n/a |
| chain-1000 | 14.6 | 10.7 | n/a |
| producer-chain-700 | 24.1 | 12.7 | n/a |
| chain-2000 | 21.5 | 11.2 | n/a |
| independent-negation-8 | 8.4 | 10.4 | n/a |
| transitive-dense-40 | 21.9 | 10.3 | n/a |
| independent-negation-10 | 8.8 | 10.3 | n/a |

Against the reference: report clauses, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search clauses): faster on 6 of 22 cells where both passed (27.2%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 94.092 | 148.149 | 0.635 | 3.437 | 44.433 | 24.795 | 84.000 | 58.000 |
| send-money/send-money | 10.964 | 15.126 | 0.725 | 2.638 | 1.898 | 0.249 | 9.000 | 1.000 |
| independent-negation-aggregate-16 | 25.872 | 31.360 | 0.825 | 0.232 | 2.309 | 5.985 | 0.000 | 26.000 |
| independent-choice-16 | 24.488 | 26.902 | 0.910 | n/a | 2.636 | 10.606 | 1.000 | 22.000 |
| n-queens/variant-04 8→11 | 163.227 | 178.996 | 0.912 | 1.491 | 112.314 | 34.048 | 2.000 | 173.000 |
| transitive-path-200 | 21.935 | 22.896 | 0.958 | n/a | 0.007 | 8.021 | 13.000 | 4.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stratified-16 | 4.471 | 4.351 | 1.028 | n/a | 0.387 | 0.169 | 0.000 | 0.000 |
| transitive-path-100 | 9.507 | 8.964 | 1.061 | n/a | 0.007 | 2.337 | 4.000 | 0.000 |
| independent-choice-12 | 7.712 | 6.738 | 1.144 | n/a | 0.724 | 1.740 | 1.000 | 2.000 |
| ties-50 | 26.624 | 19.361 | 1.375 | 0.353 | 7.237 | 5.277 | 1.000 | 14.000 |
| n-queens/variant-01 8→11 | 271.189 | 181.152 | 1.497 | 5.958 | 194.169 | 53.228 | 2.000 | 175.000 |
| n-queens/variant-01 8→10 | 59.278 | 30.227 | 1.961 | 4.109 | 35.864 | 11.648 | 2.000 | 24.000 |
| disjunction-12 | 46.224 | 21.800 | 2.120 | 0.137 | 3.674 | 2.284 | 1.000 | 17.000 |
| chain-arithmetic-1000 | 9.892 | 4.496 | 2.200 | 3.673 | 0.612 | 0.117 | 1.000 | 0.000 |
| latin-square-5 | 35.780 | 15.748 | 2.272 | 1.234 | 7.216 | 17.791 | 1.000 | 10.000 |
| producer-chain-700 | 24.704 | 8.721 | 2.833 | n/a | 0.010 | 2.690 | 5.000 | 0.000 |
| independent-negation-8 | 14.094 | 4.674 | 3.015 | n/a | 9.898 | 0.429 | 0.000 | 0.000 |
| chain-1000 | 20.700 | 6.647 | 3.114 | n/a | 0.009 | 2.352 | 3.000 | 0.000 |
| chain-2000 | 33.428 | 9.865 | 3.389 | n/a | 0.011 | 3.972 | 6.000 | 0.000 |
| transitive-dense-40 | 33.731 | 6.680 | 5.050 | 16.088 | 9.674 | 1.021 | 2.000 | 0.000 |
| planning-14 | 84.352 | 15.139 | 5.572 | 3.114 | 24.181 | 30.175 | 0.000 | 11.000 |
| independent-negation-10 | 39.494 | 5.593 | 7.061 | n/a | 34.493 | 1.327 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 13.6 | 23.4 | n/a |
| send-money/send-money | 12.3 | 12.7 | n/a |
| independent-negation-aggregate-16 | 10.5 | 10.2 | n/a |
| independent-choice-16 | 8.5 | 10.2 | n/a |
| n-queens/variant-04 8→11 | 12.4 | 11.8 | n/a |
| transitive-path-200 | 14.7 | 13.5 | n/a |
| stratified-16 | 8.8 | 10.5 | n/a |
| transitive-path-100 | 10.1 | 11.2 | n/a |
| independent-choice-12 | 8.7 | 10.3 | n/a |
| ties-50 | 11.3 | 10.5 | n/a |
| n-queens/variant-01 8→11 | 12.6 | 11.9 | n/a |
| n-queens/variant-01 8→10 | 11.1 | 10.7 | n/a |
| disjunction-12 | 10.6 | 10.4 | n/a |
| chain-arithmetic-1000 | 11.2 | 10.5 | n/a |
| latin-square-5 | 11.2 | 10.5 | n/a |
| producer-chain-700 | 24.2 | 12.6 | n/a |
| independent-negation-8 | 8.5 | 10.3 | n/a |
| chain-1000 | 14.7 | 10.9 | n/a |
| chain-2000 | 21.4 | 11.1 | n/a |
| transitive-dense-40 | 22.2 | 10.4 | n/a |
| planning-14 | 13.5 | 10.5 | n/a |
| independent-negation-10 | 8.4 | 10.2 | n/a |
