Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | before | after | after/before | before/reference | after/reference |
|---|---:|---:|---:|---:|---:|
| independent-choice-12 | 8.970 [7.941, 9.088] | 8.912 [8.846, 9.947] | 0.994 | 1.325 | 1.250 |
| independent-choice-16 | 39.682 [36.849, 40.097] | 40.250 [36.780, 40.301] | 1.014 | 1.433 | 1.462 |
| independent-negation-8 | 17.422 [17.197, 17.961] | 18.264 [16.782, 20.615] | 1.048 | 3.809 | 4.063 |
| independent-negation-10 | 50.490 [50.123, 52.251] | 52.699 [52.449, 54.436] | 1.044 | 9.109 | 11.534 |
| independent-negation-aggregate-16 | 20.305 [20.112, 20.525] | 21.479 [19.865, 22.206] | 1.058 | 0.662 | 0.646 |
| disjunction-12 | 21.152 [21.033, 22.860] | 21.897 [21.731, 22.560] | 1.035 | 0.909 | 1.017 |
| ties-50 | 20.922 [20.913, 22.441] | 21.167 [20.272, 21.556] | 1.012 | 1.151 | 1.071 |
| transitive-path-100 | 14.854 [14.774, 16.089] | 17.744 [14.787, 18.758] | 1.195 | 1.883 | 2.021 |
| transitive-path-200 | 46.174 [44.342, 47.522] | 45.069 [44.393, 46.617] | 0.976 | 2.106 | 1.930 |
| transitive-dense-40 | 30.585 [30.576, 31.603] | 31.184 [31.182, 33.337] | 1.020 | 4.593 | 4.687 |
| chain-1000 | 19.631 [19.477, 20.746] | 20.669 [19.407, 21.629] | 1.053 | 2.553 | 2.679 |
| chain-2000 | 35.838 [34.914, 38.510] | 35.797 [35.431, 37.795] | 0.999 | 2.720 | 3.202 |
| chain-arithmetic-1000 | 10.497 [10.015, 11.078] | 10.932 [10.574, 13.693] | 1.041 | 1.895 | 2.431 |
| stratified-16 | 5.627 [4.469, 5.725] | 4.501 [3.346, 5.597] | 0.800 | 1.263 | 1.029 |
| producer-chain-700 | 27.863 [24.830, 28.549] | 26.903 [25.710, 27.891] | 0.966 | 2.570 | 2.753 |
| n-queens/variant-01 8→10 | 33.158 [32.813, 33.661] | 32.812 [32.432, 33.806] | 0.990 | 1.061 | 1.041 |
| n-queens/variant-01 8→11 | 122.760 [122.058, 123.978] | 124.229 [122.073, 124.722] | 1.012 | 0.688 | 0.675 |
| n-queens/variant-04 8→11 | 97.336 [96.347, 99.869] | 99.087 [98.860, 105.944] | 1.018 | 0.554 | 0.546 |
| send-money/send-money | 14.031 [13.867, 14.508] | 13.272 [13.213, 14.358] | 0.946 | 0.842 | 0.885 |
| variant-04/05-larger-mix | 42.607 [42.213, 45.539] | 42.340 [42.145, 45.739] | 0.994 | 0.287 | 0.281 |

Reference wall time, ms, same notation.

| Cell | before | after |
|---|---:|---:|
| independent-choice-12 | 6.772 [6.766, 6.887] | 7.130 [6.857, 7.203] |
| independent-choice-16 | 27.696 [27.173, 29.225] | 27.538 [27.114, 29.092] |
| independent-negation-8 | 4.574 [4.540, 4.580] | 4.495 [4.489, 4.718] |
| independent-negation-10 | 5.543 [4.585, 5.615] | 4.569 [4.522, 5.657] |
| independent-negation-aggregate-16 | 30.671 [29.323, 34.894] | 33.226 [31.952, 33.516] |
| disjunction-12 | 23.264 [22.115, 23.306] | 21.530 [20.791, 23.225] |
| ties-50 | 18.179 [17.442, 19.069] | 19.767 [16.925, 20.804] |
| transitive-path-100 | 7.889 [7.798, 9.188] | 8.779 [8.104, 9.122] |
| transitive-path-200 | 21.928 [21.685, 22.809] | 23.354 [22.851, 24.198] |
| transitive-dense-40 | 6.659 [6.657, 6.821] | 6.654 [5.810, 6.846] |
| chain-1000 | 7.690 [6.662, 8.811] | 7.715 [6.890, 10.078] |
| chain-2000 | 13.174 [11.258, 13.272] | 11.180 [9.754, 11.348] |
| chain-arithmetic-1000 | 5.538 [4.593, 5.585] | 4.497 [4.479, 5.529] |
| stratified-16 | 4.455 [4.429, 5.677] | 4.373 [3.347, 4.446] |
| producer-chain-700 | 10.843 [9.706, 10.896] | 9.773 [9.767, 9.784] |
| n-queens/variant-01 8→10 | 31.251 [29.090, 31.322] | 31.526 [28.965, 33.430] |
| n-queens/variant-01 8→11 | 178.520 [176.483, 180.660] | 184.146 [183.947, 186.357] |
| n-queens/variant-04 8→11 | 175.855 [174.580, 176.586] | 181.313 [180.175, 181.606] |
| send-money/send-money | 16.673 [15.316, 16.939] | 14.993 [14.482, 16.742] |
| variant-04/05-larger-mix | 148.487 [148.214, 148.731] | 150.677 [150.574, 151.972] |

Counters of report after: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 1090269 | 6.308 |
| independent-choice-16 | 0 | 2584 | 2584 | 10427471 | 36.754 |
| independent-negation-8 | 0 | 55 | 55 | 123109 | 14.864 |
| independent-negation-10 | 0 | 144 | 144 | 425724 | 49.861 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 255085 | 17.442 |
| disjunction-12 | 0 | 4096 | 4096 | 2061807 | 17.989 |
| ties-50 | 0 | 1225 | 1225 | 1117111 | 16.549 |
| transitive-path-100 | 0 | 1 | 1 | 1197772 | 13.594 |
| transitive-path-200 | 0 | 1 | 1 | 5378096 | 41.377 |
| transitive-dense-40 | 0 | 1 | 1 | 770070 | 26.853 |
| chain-1000 | 0 | 1 | 1 | 475452 | 15.334 |
| chain-2000 | 0 | 1 | 1 | 1025311 | 29.360 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 82123 | 6.748 |
| stratified-16 | 0 | 1 | 1 | 9133 | 1.515 |
| producer-chain-700 | 0 | 1 | 1 | 81187 | 20.902 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 6375111 | 28.857 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 30943485 | 120.040 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 22565788 | 95.070 |
| send-money/send-money | 0 | 1 | 1 | 844145 | 9.425 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 5537899 | 38.306 |

Against the reference: report before, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 6 of 20 cells where both passed (30.0%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 42.607 | 148.487 | 0.287 | 3.299 | 80.385 | 12.905 | 84.000 | 59.000 |
| n-queens/variant-04 8→11 | 97.336 | 175.855 | 0.554 | 1.496 | 323.432 | 18.292 | 2.000 | 169.000 |
| independent-negation-aggregate-16 | 20.305 | 30.671 | 0.662 | 0.238 | 8.543 | 3.980 | 1.000 | 27.000 |
| n-queens/variant-01 8→11 | 122.760 | 178.520 | 0.688 | 6.237 | 389.538 | 28.266 | 2.000 | 172.000 |
| send-money/send-money | 14.031 | 16.673 | 0.842 | 2.576 | 12.865 | 0.292 | 10.000 | 1.000 |
| disjunction-12 | 21.152 | 23.264 | 0.909 | 0.138 | 6.758 | 30.418 | 0.000 | 18.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-01 8→10 | 33.158 | 31.251 | 1.061 | 4.700 | 80.840 | 6.128 | 1.000 | 24.000 |
| ties-50 | 20.922 | 18.179 | 1.151 | 0.345 | 18.526 | 3.022 | 0.000 | 13.000 |
| stratified-16 | 5.627 | 4.455 | 1.263 | n/a | 0.933 | 0.296 | 0.000 | 0.000 |
| independent-choice-12 | 8.970 | 6.772 | 1.325 | n/a | 0.593 | 3.394 | 0.000 | 2.000 |
| independent-choice-16 | 39.682 | 27.696 | 1.433 | n/a | 3.230 | 22.679 | 0.000 | 23.000 |
| transitive-path-100 | 14.854 | 7.889 | 1.883 | n/a | 0.006 | 7.461 | 4.000 | 0.000 |
| chain-arithmetic-1000 | 10.497 | 5.538 | 1.895 | 3.738 | 0.577 | 0.138 | 1.000 | 0.000 |
| transitive-path-200 | 46.174 | 21.928 | 2.106 | n/a | 0.008 | 30.063 | 13.000 | 4.000 |
| chain-1000 | 19.631 | 7.690 | 2.553 | n/a | 0.005 | 3.662 | 3.000 | 0.000 |
| producer-chain-700 | 27.863 | 10.843 | 2.570 | n/a | 0.005 | 3.128 | 6.000 | 0.000 |
| chain-2000 | 35.838 | 13.174 | 2.720 | n/a | 0.007 | 5.905 | 6.000 | 0.000 |
| independent-negation-8 | 17.422 | 4.574 | 3.809 | n/a | 12.364 | 1.012 | 0.000 | 0.000 |
| transitive-dense-40 | 30.585 | 6.659 | 4.593 | 15.806 | 4.788 | 1.301 | 2.000 | 0.000 |
| independent-negation-10 | 50.490 | 5.543 | 9.109 | n/a | 43.339 | 2.848 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 13.3 | 23.7 | n/a |
| n-queens/variant-04 8→11 | 11.1 | 11.8 | n/a |
| independent-negation-aggregate-16 | 10.3 | 10.1 | n/a |
| n-queens/variant-01 8→11 | 12.3 | 11.6 | n/a |
| send-money/send-money | 12.7 | 12.6 | n/a |
| disjunction-12 | 10.1 | 10.4 | n/a |
| n-queens/variant-01 8→10 | 11.4 | 10.8 | n/a |
| ties-50 | 11.6 | 10.4 | n/a |
| stratified-16 | 8.5 | 10.3 | n/a |
| independent-choice-12 | 8.8 | 10.1 | n/a |
| independent-choice-16 | 8.9 | 10.3 | n/a |
| transitive-path-100 | 11.2 | 11.1 | n/a |
| chain-arithmetic-1000 | 11.2 | 10.1 | n/a |
| transitive-path-200 | 17.3 | 13.5 | n/a |
| chain-1000 | 14.6 | 10.9 | n/a |
| producer-chain-700 | 24.5 | 12.6 | n/a |
| chain-2000 | 20.4 | 11.0 | n/a |
| independent-negation-8 | 8.5 | 10.3 | n/a |
| transitive-dense-40 | 18.2 | 10.4 | n/a |
| independent-negation-10 | 9.0 | 10.4 | n/a |

Against the reference: report after, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 5 of 20 cells where both passed (25.0%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 42.340 | 150.677 | 0.281 | 3.371 | 76.116 | 13.217 | 85.000 | 60.000 |
| n-queens/variant-04 8→11 | 99.087 | 181.313 | 0.546 | 1.478 | 327.805 | 19.135 | 2.000 | 174.000 |
| independent-negation-aggregate-16 | 21.479 | 33.226 | 0.646 | 0.273 | 8.534 | 3.963 | 0.000 | 28.000 |
| n-queens/variant-01 8→11 | 124.229 | 184.146 | 0.675 | 5.695 | 390.118 | 29.134 | 2.000 | 178.000 |
| send-money/send-money | 13.272 | 14.993 | 0.885 | 2.644 | 11.682 | 0.265 | 9.000 | 1.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| disjunction-12 | 21.897 | 21.530 | 1.017 | 0.137 | 6.730 | 30.323 | 0.000 | 16.000 |
| stratified-16 | 4.501 | 4.373 | 1.029 | n/a | 0.481 | 0.283 | 0.000 | 0.000 |
| n-queens/variant-01 8→10 | 32.812 | 31.526 | 1.041 | 3.922 | 82.529 | 6.338 | 2.000 | 25.000 |
| ties-50 | 21.167 | 19.767 | 1.071 | 0.347 | 19.592 | 3.015 | 1.000 | 14.000 |
| independent-choice-12 | 8.912 | 7.130 | 1.250 | n/a | 0.615 | 4.087 | 0.000 | 2.000 |
| independent-choice-16 | 40.250 | 27.538 | 1.462 | n/a | 2.961 | 23.768 | 1.000 | 23.000 |
| transitive-path-200 | 45.069 | 23.354 | 1.930 | n/a | 0.004 | 29.595 | 13.000 | 5.000 |
| transitive-path-100 | 17.744 | 8.779 | 2.021 | n/a | 0.008 | 8.098 | 3.000 | 1.000 |
| chain-arithmetic-1000 | 10.932 | 4.497 | 2.431 | 4.711 | 0.668 | 0.155 | 1.000 | 0.000 |
| chain-1000 | 20.669 | 7.715 | 2.679 | n/a | 0.008 | 3.301 | 3.000 | 0.000 |
| producer-chain-700 | 26.903 | 9.773 | 2.753 | n/a | 0.010 | 3.124 | 5.000 | 0.000 |
| chain-2000 | 35.797 | 11.180 | 3.202 | n/a | 0.012 | 6.166 | 7.000 | 0.000 |
| independent-negation-8 | 18.264 | 4.495 | 4.063 | n/a | 13.299 | 0.922 | 0.000 | 0.000 |
| transitive-dense-40 | 31.184 | 6.654 | 4.687 | 16.549 | 5.055 | 1.343 | 2.000 | 0.000 |
| independent-negation-10 | 52.699 | 4.569 | 11.534 | n/a | 45.204 | 2.862 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 13.4 | 23.7 | n/a |
| n-queens/variant-04 8→11 | 11.1 | 11.6 | n/a |
| independent-negation-aggregate-16 | 10.3 | 10.5 | n/a |
| n-queens/variant-01 8→11 | 11.9 | 11.6 | n/a |
| send-money/send-money | 12.5 | 12.7 | n/a |
| disjunction-12 | 9.8 | 10.1 | n/a |
| stratified-16 | 8.4 | 10.2 | n/a |
| n-queens/variant-01 8→10 | 11.0 | 10.8 | n/a |
| ties-50 | 12.0 | 10.5 | n/a |
| independent-choice-12 | 9.0 | 10.3 | n/a |
| independent-choice-16 | 8.7 | 10.2 | n/a |
| transitive-path-200 | 17.4 | 13.5 | n/a |
| transitive-path-100 | 10.8 | 11.2 | n/a |
| chain-arithmetic-1000 | 11.0 | 10.3 | n/a |
| chain-1000 | 14.3 | 10.5 | n/a |
| producer-chain-700 | 24.7 | 12.5 | n/a |
| chain-2000 | 20.7 | 11.0 | n/a |
| independent-negation-8 | 8.4 | 10.2 | n/a |
| transitive-dense-40 | 18.7 | 10.7 | n/a |
| independent-negation-10 | 8.8 | 10.2 | n/a |
