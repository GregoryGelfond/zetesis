Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

Formula search method by report: before=default; after=default; clauses=clauses;

| Cell | before | after | clauses | after/before | clauses/after | clauses/before | before/reference | after/reference | clauses/reference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 8.799 [7.739, 9.015] | 7.737 [7.690, 9.342] | 8.929 [8.866, 9.413] | 0.879 | 1.154 | 1.015 | 1.287 | 1.124 | 1.262 |
| independent-choice-16 | 39.417 [37.523, 39.835] | 38.794 [36.511, 41.200] | 40.868 [39.092, 42.188] | 0.984 | 1.053 | 1.037 | 1.368 | 1.488 | 1.516 |
| independent-negation-8 | 17.962 [17.119, 18.776] | 17.992 [17.832, 18.351] | 19.304 [18.204, 20.780] | 1.002 | 1.073 | 1.075 | 3.977 | 3.830 | 3.545 |
| independent-negation-10 | 51.470 [50.074, 53.220] | 51.673 [51.211, 53.728] | 52.298 [50.739, 52.717] | 1.004 | 1.012 | 1.016 | 11.423 | 11.315 | 11.531 |
| independent-negation-aggregate-16 | 21.008 [20.671, 21.255] | 21.333 [21.081, 22.001] | 26.479 [25.468, 28.248] | 1.015 | 1.241 | 1.260 | 0.683 | 0.707 | 0.861 |
| disjunction-12 | 21.631 [21.454, 22.452] | 22.586 [22.511, 23.629] | 46.042 [44.399, 48.293] | 1.044 | 2.039 | 2.128 | 1.042 | 1.110 | 1.980 |
| ties-50 | 21.175 [20.853, 21.413] | 22.043 [21.966, 22.095] | 27.238 [26.929, 27.448] | 1.041 | 1.236 | 1.286 | 1.149 | 1.161 | 1.369 |
| transitive-path-100 | 14.669 [14.500, 16.798] | 16.030 [15.570, 16.812] | 17.896 [17.082, 18.708] | 1.093 | 1.116 | 1.220 | 1.632 | 2.054 | 2.019 |
| transitive-path-200 | 44.891 [44.110, 46.973] | 43.476 [43.086, 45.032] | 44.668 [44.252, 47.337] | 0.968 | 1.027 | 0.995 | 1.980 | 2.012 | 1.958 |
| transitive-dense-40 | 31.133 [28.825, 32.366] | 33.066 [32.982, 33.874] | 35.921 [35.546, 36.160] | 1.062 | 1.086 | 1.154 | 5.363 | 4.912 | 5.329 |
| chain-1000 | 20.689 [20.570, 22.151] | 20.623 [19.561, 23.858] | 20.541 [19.760, 20.921] | 0.997 | 0.996 | 0.993 | 2.992 | 2.662 | 2.662 |
| chain-2000 | 34.685 [34.416, 36.753] | 36.481 [35.681, 37.604] | 35.526 [34.548, 37.636] | 1.052 | 0.974 | 1.024 | 3.474 | 3.659 | 3.259 |
| chain-arithmetic-1000 | 9.857 [8.741, 9.921] | 9.987 [9.816, 10.204] | 9.984 [9.971, 10.943] | 1.013 | 1.000 | 1.013 | 2.203 | 2.197 | 1.799 |
| stratified-16 | 4.489 [4.442, 5.585] | 4.489 [3.459, 4.502] | 4.586 [4.460, 5.712] | 1.000 | 1.022 | 1.022 | 1.004 | 1.008 | 1.033 |
| producer-chain-700 | 26.914 [26.806, 27.977] | 26.748 [25.910, 27.915] | 28.935 [27.837, 31.371] | 0.994 | 1.082 | 1.075 | 2.709 | 2.746 | 2.968 |
| latin-square-5 | 15.719 [15.163, 16.019] | 13.734 [13.324, 14.132] | 37.320 [36.696, 37.804] | 0.874 | 2.717 | 2.374 | 1.057 | 0.824 | 2.311 |
| planning-14 | 38.101 [37.310, 38.743] | 37.101 [36.696, 39.284] | 85.951 [85.265, 86.202] | 0.974 | 2.317 | 2.256 | 2.342 | 2.294 | 5.142 |
| n-queens/variant-01 8→10 | 33.034 [32.891, 33.113] | 26.181 [26.163, 27.211] | 60.533 [58.630, 62.756] | 0.793 | 2.312 | 1.832 | 1.078 | 0.840 | 1.921 |
| n-queens/variant-01 8→11 | 123.252 [119.954, 123.383] | 85.155 [84.451, 89.718] | 268.812 [262.611, 269.564] | 0.691 | 3.157 | 2.181 | 0.668 | 0.467 | 1.458 |
| n-queens/variant-04 8→11 | 99.073 [97.935, 100.275] | 78.826 [78.024, 79.766] | 162.987 [161.155, 169.504] | 0.796 | 2.068 | 1.645 | 0.549 | 0.440 | 0.901 |
| send-money/send-money | 13.730 [13.671, 15.755] | 14.217 [13.808, 14.299] | 11.068 [10.623, 11.982] | 1.035 | 0.778 | 0.806 | 0.896 | 0.985 | 0.732 |
| variant-04/05-larger-mix | 43.277 [43.199, 45.123] | 32.866 [32.840, 33.826] | 95.017 [92.520, 96.379] | 0.759 | 2.891 | 2.196 | 0.292 | 0.220 | 0.632 |

Reference wall time, ms, same notation.

| Cell | before | after | clauses |
|---|---:|---:|---:|
| independent-choice-12 | 6.837 [6.825, 7.271] | 6.883 [6.816, 6.949] | 7.078 [6.102, 7.978] |
| independent-choice-16 | 28.818 [26.586, 28.913] | 26.078 [25.658, 27.943] | 26.957 [25.633, 27.663] |
| independent-negation-8 | 4.516 [4.476, 4.925] | 4.698 [4.515, 5.617] | 5.445 [4.674, 6.630] |
| independent-negation-10 | 4.506 [3.390, 4.737] | 4.567 [4.562, 5.545] | 4.535 [4.510, 5.744] |
| independent-negation-aggregate-16 | 30.748 [30.200, 32.599] | 30.175 [29.439, 30.449] | 30.755 [29.164, 32.221] |
| disjunction-12 | 20.769 [19.093, 21.668] | 20.345 [18.049, 24.684] | 23.249 [21.382, 24.312] |
| ties-50 | 18.427 [17.839, 18.784] | 18.983 [18.180, 19.035] | 19.899 [18.882, 20.221] |
| transitive-path-100 | 8.987 [7.761, 9.059] | 7.806 [7.787, 8.956] | 8.865 [7.837, 8.912] |
| transitive-path-200 | 22.670 [22.268, 23.428] | 21.608 [21.601, 23.357] | 22.811 [22.125, 23.717] |
| transitive-dense-40 | 5.805 [5.574, 7.726] | 6.731 [6.642, 6.890] | 6.741 [6.629, 8.918] |
| chain-1000 | 6.915 [6.651, 8.774] | 7.747 [6.652, 7.899] | 7.715 [7.610, 7.894] |
| chain-2000 | 9.985 [9.838, 10.196] | 9.970 [9.936, 10.889] | 10.900 [9.730, 12.224] |
| chain-arithmetic-1000 | 4.475 [4.376, 5.478] | 4.546 [4.511, 6.584] | 5.549 [4.413, 6.669] |
| stratified-16 | 4.472 [4.416, 4.517] | 4.452 [4.426, 4.502] | 4.439 [4.389, 4.469] |
| producer-chain-700 | 9.934 [8.654, 10.901] | 9.741 [9.019, 10.878] | 9.750 [9.668, 12.291] |
| latin-square-5 | 14.865 [14.419, 15.801] | 16.670 [15.987, 16.843] | 16.146 [14.952, 17.064] |
| planning-14 | 16.270 [14.806, 16.771] | 16.172 [15.639, 17.062] | 16.717 [15.902, 17.953] |
| n-queens/variant-01 8→10 | 30.650 [29.708, 31.164] | 31.184 [29.783, 31.649] | 31.505 [31.265, 31.848] |
| n-queens/variant-01 8→11 | 184.637 [183.972, 196.012] | 182.498 [181.181, 182.622] | 184.426 [183.312, 185.303] |
| n-queens/variant-04 8→11 | 180.410 [179.549, 181.045] | 179.206 [177.674, 179.245] | 180.853 [179.878, 182.241] |
| send-money/send-money | 15.316 [14.465, 15.512] | 14.436 [14.112, 14.464] | 15.110 [14.124, 15.334] |
| variant-04/05-larger-mix | 148.365 [148.073, 148.372] | 149.116 [148.845, 150.375] | 150.371 [149.838, 167.627] |

Counters of report clauses: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 1090269 | 5.583 |
| independent-choice-16 | 0 | 2584 | 2584 | 10427471 | 37.338 |
| independent-negation-8 | 0 | 55 | 55 | 123109 | 15.213 |
| independent-negation-10 | 0 | 144 | 144 | 425724 | 49.657 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 3137480 | 22.613 |
| disjunction-12 | 0 | 4096 | 4096 | 9004441 | 41.222 |
| ties-50 | 0 | 1225 | 1225 | 4720400 | 23.205 |
| transitive-path-100 | 0 | 1 | 1 | 1197772 | 14.443 |
| transitive-path-200 | 0 | 1 | 1 | 5378096 | 40.358 |
| transitive-dense-40 | 0 | 1 | 1 | 2173586 | 31.896 |
| chain-1000 | 0 | 1 | 1 | 475452 | 15.492 |
| chain-2000 | 0 | 1 | 1 | 1025311 | 30.174 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 193171 | 6.214 |
| stratified-16 | 0 | 1 | 1 | 9133 | 1.683 |
| producer-chain-700 | 0 | 1 | 1 | 81187 | 23.020 |
| latin-square-5 | 0 | 1344 | 1344 | 8125616 | 33.272 |
| planning-14 | 0 | 3432 | 3432 | 19467097 | 82.278 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 11383348 | 56.210 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 57156138 | 264.664 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 35658773 | 159.382 |
| send-money/send-money | 0 | 1 | 1 | 349198 | 6.970 |
| variant-04/05-larger-mix | 0 | 1176 | 1208 | 23292550 | 90.962 |

Against the reference: report before, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 5 of 22 cells where both passed (22.7%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 43.277 | 148.365 | 0.292 | 3.320 | 80.818 | 13.202 | 85.000 | 58.000 |
| n-queens/variant-04 8→11 | 99.073 | 180.410 | 0.549 | 1.472 | 328.049 | 18.835 | 1.000 | 175.000 |
| n-queens/variant-01 8→11 | 123.252 | 184.637 | 0.668 | 6.356 | 391.768 | 28.714 | 2.000 | 179.000 |
| independent-negation-aggregate-16 | 21.008 | 30.748 | 0.683 | 0.226 | 8.639 | 4.070 | 0.000 | 27.000 |
| send-money/send-money | 13.730 | 15.316 | 0.896 | 2.622 | 11.588 | 0.250 | 9.000 | 1.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stratified-16 | 4.489 | 4.472 | 1.004 | n/a | 0.478 | 0.309 | 0.000 | 0.000 |
| disjunction-12 | 21.631 | 20.769 | 1.042 | 0.130 | 6.564 | 29.955 | 0.000 | 16.000 |
| latin-square-5 | 15.719 | 14.865 | 1.057 | 1.234 | 23.409 | 9.952 | 1.000 | 10.000 |
| n-queens/variant-01 8→10 | 33.034 | 30.650 | 1.078 | 3.952 | 82.190 | 6.314 | 1.000 | 25.000 |
| ties-50 | 21.175 | 18.427 | 1.149 | 0.339 | 20.212 | 3.139 | 0.000 | 14.000 |
| independent-choice-12 | 8.799 | 6.837 | 1.287 | n/a | 0.614 | 3.215 | 0.000 | 2.000 |
| independent-choice-16 | 39.417 | 28.818 | 1.368 | n/a | 3.071 | 22.762 | 0.000 | 23.000 |
| transitive-path-100 | 14.669 | 8.987 | 1.632 | n/a | 0.009 | 7.587 | 3.000 | 1.000 |
| transitive-path-200 | 44.891 | 22.670 | 1.980 | n/a | 0.004 | 29.661 | 12.000 | 5.000 |
| chain-arithmetic-1000 | 9.857 | 4.475 | 2.203 | 3.625 | 0.689 | 0.153 | 1.000 | 0.000 |
| planning-14 | 38.101 | 16.270 | 2.342 | 3.016 | 70.634 | 17.833 | 0.000 | 11.000 |
| producer-chain-700 | 26.914 | 9.934 | 2.709 | n/a | 0.009 | 2.861 | 5.000 | 0.000 |
| chain-1000 | 20.689 | 6.915 | 2.992 | n/a | 0.011 | 4.124 | 3.000 | 0.000 |
| chain-2000 | 34.685 | 9.985 | 3.474 | n/a | 0.013 | 6.541 | 6.000 | 0.000 |
| independent-negation-8 | 17.962 | 4.516 | 3.977 | n/a | 12.901 | 0.969 | 0.000 | 0.000 |
| transitive-dense-40 | 31.133 | 5.805 | 5.363 | 16.632 | 4.739 | 1.404 | 2.000 | 0.000 |
| independent-negation-10 | 51.470 | 4.506 | 11.423 | n/a | 45.128 | 2.847 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 13.5 | 23.6 | n/a |
| n-queens/variant-04 8→11 | 11.1 | 11.9 | n/a |
| n-queens/variant-01 8→11 | 11.9 | 11.8 | n/a |
| independent-negation-aggregate-16 | 10.5 | 10.2 | n/a |
| send-money/send-money | 12.5 | 12.6 | n/a |
| stratified-16 | 8.4 | 10.2 | n/a |
| disjunction-12 | 10.1 | 10.4 | n/a |
| latin-square-5 | 10.5 | 10.5 | n/a |
| n-queens/variant-01 8→10 | 11.5 | 10.7 | n/a |
| ties-50 | 11.6 | 10.6 | n/a |
| independent-choice-12 | 8.6 | 10.3 | n/a |
| independent-choice-16 | 8.9 | 10.3 | n/a |
| transitive-path-100 | 10.8 | 11.2 | n/a |
| transitive-path-200 | 17.7 | 13.4 | n/a |
| chain-arithmetic-1000 | 11.1 | 10.4 | n/a |
| planning-14 | 10.7 | 10.5 | n/a |
| producer-chain-700 | 24.9 | 12.6 | n/a |
| chain-1000 | 14.4 | 10.4 | n/a |
| chain-2000 | 20.4 | 11.0 | n/a |
| independent-negation-8 | 8.4 | 10.2 | n/a |
| transitive-dense-40 | 18.3 | 10.5 | n/a |
| independent-negation-10 | 8.7 | 10.3 | n/a |

Against the reference: report after, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 7 of 22 cells where both passed (31.8%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 32.866 | 149.116 | 0.220 | 3.245 | 31.618 | 12.657 | 85.000 | 58.000 |
| n-queens/variant-04 8→11 | 78.826 | 179.206 | 0.440 | 1.461 | 221.070 | 18.189 | 1.000 | 173.000 |
| n-queens/variant-01 8→11 | 85.155 | 182.498 | 0.467 | 5.683 | 215.355 | 28.025 | 2.000 | 176.000 |
| independent-negation-aggregate-16 | 21.333 | 30.175 | 0.707 | 0.242 | 3.836 | 4.090 | 1.000 | 25.000 |
| latin-square-5 | 13.734 | 16.670 | 0.824 | 1.221 | 12.102 | 10.236 | 1.000 | 11.000 |
| n-queens/variant-01 8→10 | 26.181 | 31.184 | 0.840 | 3.961 | 46.931 | 6.157 | 2.000 | 24.000 |
| send-money/send-money | 14.217 | 14.436 | 0.985 | 2.502 | 6.472 | 0.277 | 9.000 | 1.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stratified-16 | 4.489 | 4.452 | 1.008 | n/a | 0.477 | 0.287 | 0.000 | 0.000 |
| disjunction-12 | 22.586 | 20.345 | 1.110 | 0.146 | 2.602 | 37.866 | 0.000 | 16.000 |
| independent-choice-12 | 7.737 | 6.883 | 1.124 | n/a | 0.558 | 3.193 | 0.000 | 2.000 |
| ties-50 | 22.043 | 18.983 | 1.161 | 0.349 | 14.002 | 3.054 | 1.000 | 13.000 |
| independent-choice-16 | 38.794 | 26.078 | 1.488 | n/a | 2.909 | 21.862 | 0.000 | 22.000 |
| transitive-path-200 | 43.476 | 21.608 | 2.012 | n/a | 0.008 | 29.759 | 13.000 | 4.000 |
| transitive-path-100 | 16.030 | 7.806 | 2.054 | n/a | 0.007 | 7.935 | 4.000 | 0.000 |
| chain-arithmetic-1000 | 9.987 | 4.546 | 2.197 | 3.691 | 0.810 | 0.168 | 1.000 | 0.000 |
| planning-14 | 37.101 | 16.172 | 2.294 | 2.959 | 42.115 | 18.600 | 0.000 | 11.000 |
| chain-1000 | 20.623 | 7.747 | 2.662 | n/a | 0.009 | 3.964 | 3.000 | 0.000 |
| producer-chain-700 | 26.748 | 9.741 | 2.746 | n/a | 0.009 | 3.088 | 5.000 | 0.000 |
| chain-2000 | 36.481 | 9.970 | 3.659 | n/a | 0.010 | 5.947 | 6.000 | 0.000 |
| independent-negation-8 | 17.992 | 4.698 | 3.830 | n/a | 13.054 | 0.936 | 0.000 | 0.000 |
| transitive-dense-40 | 33.066 | 6.731 | 4.912 | 16.011 | 7.431 | 1.400 | 2.000 | 0.000 |
| independent-negation-10 | 51.673 | 4.567 | 11.315 | n/a | 44.330 | 2.842 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 16.2 | 23.7 | n/a |
| n-queens/variant-04 8→11 | 13.1 | 11.8 | n/a |
| n-queens/variant-01 8→11 | 14.1 | 11.8 | n/a |
| independent-negation-aggregate-16 | 10.5 | 10.4 | n/a |
| latin-square-5 | 11.3 | 10.7 | n/a |
| n-queens/variant-01 8→10 | 12.7 | 10.8 | n/a |
| send-money/send-money | 14.0 | 12.7 | n/a |
| stratified-16 | 8.6 | 10.2 | n/a |
| disjunction-12 | 10.7 | 10.3 | n/a |
| independent-choice-12 | 8.6 | 10.4 | n/a |
| ties-50 | 12.9 | 10.6 | n/a |
| independent-choice-16 | 9.1 | 10.4 | n/a |
| transitive-path-200 | 17.3 | 13.4 | n/a |
| transitive-path-100 | 11.0 | 10.8 | n/a |
| chain-arithmetic-1000 | 11.4 | 10.6 | n/a |
| planning-14 | 11.1 | 10.6 | n/a |
| chain-1000 | 14.6 | 10.6 | n/a |
| producer-chain-700 | 24.5 | 12.5 | n/a |
| chain-2000 | 20.4 | 10.8 | n/a |
| independent-negation-8 | 8.7 | 10.3 | n/a |
| transitive-dense-40 | 21.5 | 10.6 | n/a |
| independent-negation-10 | 8.9 | 10.4 | n/a |

Against the reference: report clauses, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search clauses): faster on 4 of 22 cells where both passed (18.1%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 95.017 | 150.371 | 0.632 | 3.277 | 44.539 | 24.454 | 87.000 | 58.000 |
| send-money/send-money | 11.068 | 15.110 | 0.732 | 2.559 | 1.719 | 0.241 | 9.000 | 1.000 |
| independent-negation-aggregate-16 | 26.479 | 30.755 | 0.861 | 0.236 | 2.456 | 6.381 | 1.000 | 26.000 |
| n-queens/variant-04 8→11 | 162.987 | 180.853 | 0.901 | 1.505 | 113.658 | 33.004 | 1.000 | 174.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stratified-16 | 4.586 | 4.439 | 1.033 | n/a | 0.497 | 0.170 | 0.000 | 0.000 |
| independent-choice-12 | 8.929 | 7.078 | 1.262 | n/a | 0.629 | 3.268 | 0.000 | 2.000 |
| ties-50 | 27.238 | 19.899 | 1.369 | 0.356 | 7.205 | 5.107 | 1.000 | 13.000 |
| n-queens/variant-01 8→11 | 268.812 | 184.426 | 1.458 | 5.679 | 190.211 | 53.174 | 2.000 | 177.000 |
| independent-choice-16 | 40.868 | 26.957 | 1.516 | n/a | 2.961 | 23.167 | 0.000 | 22.000 |
| chain-arithmetic-1000 | 9.984 | 5.549 | 1.799 | 4.122 | 0.579 | 0.123 | 1.000 | 0.000 |
| n-queens/variant-01 8→10 | 60.533 | 31.505 | 1.921 | 3.968 | 36.028 | 12.508 | 2.000 | 25.000 |
| transitive-path-200 | 44.668 | 22.811 | 1.958 | n/a | 0.008 | 29.322 | 13.000 | 4.000 |
| disjunction-12 | 46.042 | 23.249 | 1.980 | 0.152 | 3.685 | 2.093 | 0.000 | 18.000 |
| transitive-path-100 | 17.896 | 8.865 | 2.019 | n/a | 0.007 | 8.532 | 4.000 | 0.000 |
| latin-square-5 | 37.320 | 16.146 | 2.311 | 1.253 | 7.275 | 18.617 | 1.000 | 11.000 |
| chain-1000 | 20.541 | 7.715 | 2.662 | n/a | 0.011 | 3.403 | 3.000 | 0.000 |
| producer-chain-700 | 28.935 | 9.750 | 2.968 | n/a | 0.012 | 3.067 | 6.000 | 0.000 |
| chain-2000 | 35.526 | 10.900 | 3.259 | n/a | 0.014 | 6.354 | 7.000 | 0.000 |
| independent-negation-8 | 19.304 | 5.445 | 3.545 | n/a | 13.622 | 1.005 | 1.000 | 0.000 |
| planning-14 | 85.951 | 16.717 | 5.142 | 4.028 | 24.152 | 29.668 | 1.000 | 11.000 |
| transitive-dense-40 | 35.921 | 6.741 | 5.329 | 15.871 | 10.108 | 1.103 | 2.000 | 0.000 |
| independent-negation-10 | 52.298 | 4.535 | 11.531 | n/a | 45.747 | 2.937 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 13.5 | 23.7 | n/a |
| send-money/send-money | 12.3 | 12.6 | n/a |
| independent-negation-aggregate-16 | 10.5 | 10.5 | n/a |
| n-queens/variant-04 8→11 | 12.3 | 11.8 | n/a |
| stratified-16 | 8.5 | 10.3 | n/a |
| independent-choice-12 | 9.0 | 10.3 | n/a |
| ties-50 | 11.1 | 10.2 | n/a |
| n-queens/variant-01 8→11 | 12.8 | 11.8 | n/a |
| independent-choice-16 | 8.8 | 10.3 | n/a |
| chain-arithmetic-1000 | 11.0 | 10.6 | n/a |
| n-queens/variant-01 8→10 | 11.2 | 10.6 | n/a |
| transitive-path-200 | 17.3 | 13.6 | n/a |
| disjunction-12 | 10.7 | 10.3 | n/a |
| transitive-path-100 | 10.9 | 11.3 | n/a |
| latin-square-5 | 11.3 | 10.5 | n/a |
| chain-1000 | 14.3 | 10.7 | n/a |
| producer-chain-700 | 24.6 | 12.6 | n/a |
| chain-2000 | 20.6 | 11.1 | n/a |
| independent-negation-8 | 8.6 | 10.2 | n/a |
| planning-14 | 13.5 | 10.5 | n/a |
| transitive-dense-40 | 22.3 | 10.7 | n/a |
| independent-negation-10 | 8.9 | 10.3 | n/a |
