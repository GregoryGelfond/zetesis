Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=1, batch=64.

| Cell | baseline | candidate | candidate/baseline | baseline/reference | candidate/reference |
|---|---:|---:|---:|---:|---:|
| independent-choice-12 | 10.420 [10.412, 10.619] | 10.407 [10.397, 11.917] | 0.999 | 1.320 | 1.319 |
| independent-choice-16 | 39.489 [38.161, 40.400] | 39.438 [38.168, 41.151] | 0.999 | 1.257 | 1.250 |
| independent-negation-8 | 17.956 [17.771, 19.455] | 19.139 [17.939, 19.377] | 1.066 | 3.426 | 3.649 |
| independent-negation-10 | 45.338 [44.231, 49.204] | 45.573 [44.141, 47.023] | 1.005 | 8.595 | 8.638 |
| independent-negation-aggregate-16 | 23.337 [23.284, 24.344] | 23.278 [23.133, 24.425] | 0.997 | 0.635 | 0.626 |
| disjunction-12 | 23.297 [23.232, 24.508] | 23.372 [23.179, 24.749] | 1.003 | 0.844 | 0.845 |
| ties-50 | 20.873 [20.677, 21.986] | 21.032 [20.480, 22.420] | 1.008 | 1.066 | 1.077 |
| transitive-path-100 | 18.090 [16.845, 18.351] | 18.112 [16.863, 19.671] | 1.001 | 2.317 | 2.307 |
| transitive-path-200 | 51.394 [49.988, 51.523] | 51.454 [51.363, 53.801] | 1.001 | 2.675 | 2.658 |
| transitive-dense-40 | 35.673 [34.328, 35.810] | 35.596 [35.536, 38.307] | 0.998 | 5.442 | 5.432 |
| chain-1000 | 20.535 [19.180, 20.613] | 21.785 [19.195, 25.424] | 1.061 | 2.665 | 3.377 |
| chain-2000 | 35.700 [35.562, 36.896] | 35.753 [34.264, 37.097] | 1.001 | 3.526 | 3.455 |
| chain-arithmetic-1000 | 20.551 [20.252, 21.637] | 17.957 [17.855, 19.365] | 0.874 | 3.971 | 3.459 |
| stratified-16 | 6.554 [6.500, 6.631] | 7.870 [6.537, 8.041] | 1.201 | 1.679 | 1.516 |
| producer-chain-700 | 21.687 [21.661, 21.763] | 21.743 [21.384, 23.099] | 1.003 | 2.130 | 2.124 |
| latin-square-5 | 15.537 [15.248, 16.793] | 16.750 [15.267, 16.928] | 1.078 | 0.914 | 0.983 |
| planning-14 | 39.555 [38.225, 39.750] | 39.580 [39.530, 39.776] | 1.001 | 2.376 | 2.372 |
| n-queens/variant-01 8→10 | 41.366 [40.533, 42.592] | 41.765 [40.526, 44.633] | 1.010 | 1.358 | 1.370 |
| n-queens/variant-01 8→11 | 99.107 [97.899, 101.803] | 101.261 [98.867, 105.798] | 1.022 | 0.526 | 0.534 |
| n-queens/variant-04 8→11 | 75.079 [74.501, 76.446] | 75.219 [73.732, 80.238] | 1.002 | 0.412 | 0.405 |
| send-money/send-money | 15.514 [15.415, 16.890] | 15.474 [15.450, 16.914] | 0.997 | 1.217 | 1.204 |
| variant-04/05-larger-mix | 37.166 [36.140, 38.573] | 38.535 [37.064, 38.698] | 1.037 | 0.187 | 0.192 |

Reference wall time, ms, same notation.

| Cell | baseline | candidate |
|---|---:|---:|
| independent-choice-12 | 7.896 [6.552, 8.002] | 7.889 [7.837, 7.966] |
| independent-choice-16 | 31.414 [30.165, 31.739] | 31.542 [29.305, 31.613] |
| independent-negation-8 | 5.241 [3.917, 5.347] | 5.245 [5.117, 5.272] |
| independent-negation-10 | 5.275 [5.178, 5.368] | 5.276 [5.208, 5.304] |
| independent-negation-aggregate-16 | 36.740 [36.190, 38.002] | 37.184 [35.789, 38.115] |
| disjunction-12 | 27.590 [27.521, 27.796] | 27.666 [27.605, 27.675] |
| ties-50 | 19.581 [19.378, 19.735] | 19.536 [19.308, 19.841] |
| transitive-path-100 | 7.808 [7.803, 7.835] | 7.852 [7.776, 9.130] |
| transitive-path-200 | 19.215 [17.922, 19.282] | 19.356 [19.160, 20.722] |
| transitive-dense-40 | 6.555 [6.474, 6.562] | 6.553 [6.459, 6.632] |
| chain-1000 | 7.706 [7.686, 7.816] | 6.450 [6.409, 9.212] |
| chain-2000 | 10.124 [8.926, 10.342] | 10.348 [8.929, 11.650] |
| chain-arithmetic-1000 | 5.176 [5.160, 5.263] | 5.192 [5.162, 6.423] |
| stratified-16 | 3.904 [3.901, 5.192] | 5.191 [3.908, 5.314] |
| producer-chain-700 | 10.179 [10.057, 10.325] | 10.239 [10.222, 10.340] |
| latin-square-5 | 17.005 [16.906, 17.088] | 17.033 [16.952, 18.445] |
| planning-14 | 16.648 [16.564, 16.782] | 16.690 [15.309, 16.838] |
| n-queens/variant-01 8→10 | 30.451 [29.129, 32.942] | 30.492 [30.389, 31.308] |
| n-queens/variant-01 8→11 | 188.559 [187.494, 192.482] | 189.795 [187.358, 191.735] |
| n-queens/variant-04 8→11 | 182.441 [182.340, 183.767] | 185.694 [182.499, 194.651] |
| send-money/send-money | 12.746 [12.643, 12.825] | 12.853 [12.656, 14.105] |
| variant-04/05-larger-mix | 198.611 [168.130, 202.441] | 201.112 [199.234, 203.037] |

Counters of report candidate: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 1040394 | 4.432 |
| independent-choice-16 | 0 | 2584 | 2584 | 9919459 | 33.608 |
| independent-negation-8 | 0 | 55 | 55 | 152413 | 12.248 |
| independent-negation-10 | 0 | 144 | 144 | 483768 | 39.657 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 1972826 | 17.304 |
| disjunction-12 | 0 | 4096 | 4096 | 2029230 | 17.334 |
| ties-50 | 0 | 1225 | 1225 | 2438104 | 15.084 |
| transitive-path-100 | 0 | 1 | 1 | 1682966 | 11.511 |
| transitive-path-200 | 0 | 1 | 1 | 7678960 | 45.646 |
| transitive-dense-40 | 0 | 1 | 1 | 939917 | 29.293 |
| chain-1000 | 0 | 1 | 1 | 905618 | 15.203 |
| chain-2000 | 0 | 1 | 1 | 2231759 | 28.304 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 105151 | 12.316 |
| stratified-16 | 0 | 1 | 1 | 20081 | 1.288 |
| producer-chain-700 | 0 | 1 | 1 | 2115649 | 15.819 |
| latin-square-5 | 0 | 1344 | 1344 | 5659430 | 9.841 |
| planning-14 | 0 | 3432 | 3432 | 11390399 | 32.870 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 6686540 | 34.809 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 31378525 | 94.003 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 23637618 | 68.346 |
| send-money/send-money | 0 | 1 | 1 | 472288 | 9.238 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 8538591 | 30.680 |

Against the reference: report baseline, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=1, batch=64; search default): faster on 6 of 22 cells where both passed (27.2%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership, each summed over its intervals, which can overlap within or across threads, so the parts need not add up to the native median; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 37.166 | 198.611 | 0.187 | 7.436 | 27.048 | 8.362 | 77.000 | 117.000 |
| n-queens/variant-04 8→11 | 75.079 | 182.441 | 0.412 | 4.127 | 181.946 | 12.334 | 1.000 | 177.000 |
| n-queens/variant-01 8→11 | 99.107 | 188.559 | 0.526 | 27.712 | 174.413 | 18.072 | 1.000 | 182.000 |
| independent-negation-aggregate-16 | 23.337 | 36.740 | 0.635 | 0.399 | 3.066 | 3.281 | 0.000 | 32.000 |
| disjunction-12 | 23.297 | 27.590 | 0.844 | 0.216 | 2.608 | 29.420 | 0.000 | 23.000 |
| latin-square-5 | 15.537 | 17.005 | 0.914 | 2.387 | 9.078 | 6.454 | 1.000 | 11.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership, each summed over its intervals, which can overlap within or across threads, so the parts need not add up to the native median; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| ties-50 | 20.873 | 19.581 | 1.066 | 0.598 | 8.994 | 2.115 | 1.000 | 14.000 |
| send-money/send-money | 15.514 | 12.746 | 1.217 | 5.209 | 3.552 | 0.151 | 7.000 | 1.000 |
| independent-choice-16 | 39.489 | 31.414 | 1.257 | n/a | 7.715 | 20.608 | 1.000 | 26.000 |
| independent-choice-12 | 10.420 | 7.896 | 1.320 | n/a | 1.075 | 2.412 | 0.000 | 3.000 |
| n-queens/variant-01 8→10 | 41.366 | 30.451 | 1.358 | 19.028 | 39.402 | 4.298 | 1.000 | 24.000 |
| stratified-16 | 6.554 | 3.904 | 1.679 | n/a | 0.580 | 0.205 | 0.000 | 0.000 |
| producer-chain-700 | 21.687 | 10.179 | 2.130 | n/a | 0.013 | 8.507 | 5.000 | 0.000 |
| transitive-path-100 | 18.090 | 7.808 | 2.317 | n/a | 0.007 | 8.180 | 3.000 | 0.000 |
| planning-14 | 39.555 | 16.648 | 2.376 | 10.052 | 35.719 | 12.707 | 0.000 | 12.000 |
| chain-1000 | 20.535 | 7.706 | 2.665 | n/a | 0.010 | 5.023 | 3.000 | 0.000 |
| transitive-path-200 | 51.394 | 19.215 | 2.675 | n/a | 0.007 | 36.023 | 10.000 | 5.000 |
| independent-negation-8 | 17.956 | 5.241 | 3.426 | n/a | 11.397 | 0.431 | 0.000 | 0.000 |
| chain-2000 | 35.700 | 10.124 | 3.526 | n/a | 0.013 | 10.984 | 5.000 | 0.000 |
| chain-arithmetic-1000 | 20.551 | 5.176 | 3.971 | 12.325 | 0.357 | 0.094 | 1.000 | 0.000 |
| transitive-dense-40 | 35.673 | 6.555 | 5.442 | 20.410 | 4.316 | 0.790 | 2.000 | 0.000 |
| independent-negation-10 | 45.338 | 5.275 | 8.595 | n/a | 38.224 | 1.134 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 22.8 | 21.4 | n/a |
| n-queens/variant-04 8→11 | 17.2 | 7.3 | n/a |
| n-queens/variant-01 8→11 | 18.8 | 7.4 | n/a |
| independent-negation-aggregate-16 | 14.2 | 5.2 | n/a |
| disjunction-12 | 13.5 | 5.0 | n/a |
| latin-square-5 | 15.7 | 5.5 | n/a |
| ties-50 | 16.9 | 5.5 | n/a |
| send-money/send-money | 18.9 | 8.2 | n/a |
| independent-choice-16 | 11.5 | 5.0 | n/a |
| independent-choice-12 | 11.5 | 5.0 | n/a |
| n-queens/variant-01 8→10 | 17.2 | 5.6 | n/a |
| stratified-16 | 11.6 | 5.0 | n/a |
| producer-chain-700 | 30.1 | 7.2 | n/a |
| transitive-path-100 | 13.5 | 6.0 | n/a |
| planning-14 | 15.1 | 5.4 | n/a |
| chain-1000 | 18.1 | 5.4 | n/a |
| transitive-path-200 | 21.2 | 8.8 | n/a |
| independent-negation-8 | 11.3 | 5.0 | n/a |
| chain-2000 | 25.9 | 5.9 | n/a |
| chain-arithmetic-1000 | 16.0 | 5.2 | n/a |
| transitive-dense-40 | 27.7 | 5.3 | n/a |
| independent-negation-10 | 11.4 | 5.0 | n/a |

Against the reference: report candidate, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=1, batch=64; search default): faster on 6 of 22 cells where both passed (27.2%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership, each summed over its intervals, which can overlap within or across threads, so the parts need not add up to the native median; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 38.535 | 201.112 | 0.192 | 7.471 | 27.082 | 8.235 | 77.000 | 119.000 |
| n-queens/variant-04 8→11 | 75.219 | 185.694 | 0.405 | 4.203 | 179.779 | 12.333 | 1.000 | 179.000 |
| n-queens/variant-01 8→11 | 101.261 | 189.795 | 0.534 | 27.438 | 175.352 | 18.047 | 2.000 | 183.000 |
| independent-negation-aggregate-16 | 23.278 | 37.184 | 0.626 | 0.383 | 2.846 | 3.125 | 0.000 | 32.000 |
| disjunction-12 | 23.372 | 27.666 | 0.845 | 0.227 | 2.612 | 28.353 | 0.000 | 23.000 |
| latin-square-5 | 16.750 | 17.033 | 0.983 | 2.339 | 8.885 | 6.348 | 1.000 | 12.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership, each summed over its intervals, which can overlap within or across threads, so the parts need not add up to the native median; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| ties-50 | 21.032 | 19.536 | 1.077 | 0.628 | 9.245 | 2.115 | 1.000 | 14.000 |
| send-money/send-money | 15.474 | 12.853 | 1.204 | 5.264 | 3.554 | 0.156 | 7.000 | 1.000 |
| independent-choice-16 | 39.438 | 31.542 | 1.250 | n/a | 7.812 | 21.012 | 1.000 | 26.000 |
| independent-choice-12 | 10.407 | 7.889 | 1.319 | n/a | 1.068 | 2.429 | 0.000 | 3.000 |
| n-queens/variant-01 8→10 | 41.765 | 30.492 | 1.370 | 18.915 | 38.928 | 4.231 | 1.000 | 24.000 |
| stratified-16 | 7.870 | 5.191 | 1.516 | n/a | 0.579 | 0.205 | 0.000 | 0.000 |
| producer-chain-700 | 21.743 | 10.239 | 2.124 | n/a | 0.011 | 8.555 | 5.000 | 0.000 |
| transitive-path-100 | 18.112 | 7.852 | 2.307 | n/a | 0.006 | 8.320 | 3.000 | 1.000 |
| planning-14 | 39.580 | 16.690 | 2.372 | 10.158 | 35.235 | 12.476 | 0.000 | 12.000 |
| transitive-path-200 | 51.454 | 19.356 | 2.658 | n/a | 0.007 | 36.529 | 10.000 | 5.000 |
| chain-1000 | 21.785 | 6.450 | 3.377 | n/a | 0.010 | 5.116 | 3.000 | 0.000 |
| chain-2000 | 35.753 | 10.348 | 3.455 | n/a | 0.012 | 10.963 | 5.000 | 0.000 |
| chain-arithmetic-1000 | 17.957 | 5.192 | 3.459 | 9.538 | 0.350 | 0.097 | 1.000 | 0.000 |
| independent-negation-8 | 19.139 | 5.245 | 3.649 | n/a | 11.377 | 0.431 | 0.000 | 0.000 |
| transitive-dense-40 | 35.596 | 6.553 | 5.432 | 20.780 | 4.375 | 0.832 | 2.000 | 0.000 |
| independent-negation-10 | 45.573 | 5.276 | 8.638 | n/a | 37.974 | 1.112 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 22.3 | 19.4 | n/a |
| n-queens/variant-04 8→11 | 17.2 | 7.4 | n/a |
| n-queens/variant-01 8→11 | 18.6 | 7.2 | n/a |
| independent-negation-aggregate-16 | 13.9 | 5.2 | n/a |
| disjunction-12 | 13.6 | 5.0 | n/a |
| latin-square-5 | 15.5 | 5.5 | n/a |
| ties-50 | 17.0 | 5.5 | n/a |
| send-money/send-money | 18.9 | 8.2 | n/a |
| independent-choice-16 | 11.5 | 5.1 | n/a |
| independent-choice-12 | 11.3 | 5.0 | n/a |
| n-queens/variant-01 8→10 | 17.0 | 5.8 | n/a |
| stratified-16 | 11.4 | 5.0 | n/a |
| producer-chain-700 | 30.0 | 7.2 | n/a |
| transitive-path-100 | 13.4 | 6.0 | n/a |
| planning-14 | 14.8 | 5.4 | n/a |
| transitive-path-200 | 21.0 | 8.8 | n/a |
| chain-1000 | 17.9 | 5.4 | n/a |
| chain-2000 | 25.7 | 5.9 | n/a |
| chain-arithmetic-1000 | 15.5 | 5.2 | n/a |
| independent-negation-8 | 11.2 | 5.0 | n/a |
| transitive-dense-40 | 27.5 | 5.3 | n/a |
| independent-negation-10 | 11.4 | 5.0 | n/a |
