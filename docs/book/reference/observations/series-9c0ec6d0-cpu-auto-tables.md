Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | before | after | after/before | before/reference | after/reference |
|---|---:|---:|---:|---:|---:|
| independent-choice-12 | 5.659 [5.562, 6.940] | 6.796 [6.718, 7.751] | 1.201 | 0.834 | 1.012 |
| independent-choice-16 | 24.795 [21.912, 27.416] | 25.294 [25.047, 25.784] | 1.020 | 0.951 | 0.912 |
| independent-negation-8 | 14.226 [13.639, 14.241] | 9.425 [8.782, 11.237] | 0.663 | 3.718 | 2.053 |
| independent-negation-10 | 38.774 [37.329, 40.548] | 23.501 [22.701, 23.536] | 0.606 | 8.573 | 5.176 |
| independent-negation-aggregate-16 | 19.954 [19.157, 22.756] | 21.967 [20.106, 22.665] | 1.101 | 0.640 | 0.731 |
| disjunction-12 | 22.860 [21.674, 24.561] | 22.685 [22.620, 22.951] | 0.992 | 0.981 | 1.025 |
| ties-50 | 21.939 [21.502, 21.965] | 22.285 [21.919, 23.879] | 1.016 | 1.225 | 1.307 |
| transitive-path-100 | 15.002 [8.484, 15.995] | 10.262 [9.560, 11.223] | 0.684 | 1.924 | 1.154 |
| transitive-path-200 | 21.721 [19.830, 21.998] | 20.941 [20.475, 22.132] | 0.964 | 0.943 | 0.920 |
| transitive-dense-40 | 32.334 [32.321, 32.733] | 33.787 [32.819, 34.245] | 1.045 | 4.889 | 6.025 |
| chain-1000 | 17.303 [16.195, 17.715] | 18.469 [18.342, 18.533] | 1.067 | 2.595 | 2.788 |
| chain-2000 | 32.253 [31.290, 32.942] | 32.511 [32.192, 35.686] | 1.008 | 3.257 | 3.286 |
| chain-arithmetic-1000 | 9.940 [9.903, 10.609] | 10.476 [8.743, 10.918] | 1.054 | 1.791 | 2.353 |
| stratified-16 | 3.561 [3.433, 4.627] | 3.423 [3.391, 4.665] | 0.961 | 0.796 | 0.783 |
| producer-chain-700 | 25.866 [25.614, 27.280] | 27.902 [24.633, 31.100] | 1.079 | 2.388 | 2.860 |
| latin-square-5 | 13.528 [12.888, 15.433] | 13.820 [13.340, 14.311] | 1.022 | 0.869 | 0.839 |
| planning-14 | 36.061 [35.234, 42.149] | 37.891 [37.052, 39.364] | 1.051 | 2.016 | 2.516 |
| n-queens/variant-01 8→10 | 25.686 [25.113, 25.887] | 25.960 [25.772, 26.119] | 1.011 | 0.853 | 0.833 |
| n-queens/variant-01 8→11 | 85.670 [83.565, 88.161] | 87.663 [85.883, 90.347] | 1.023 | 0.471 | 0.487 |
| n-queens/variant-04 8→11 | 77.470 [77.092, 78.210] | 78.348 [77.824, 79.523] | 1.011 | 0.439 | 0.444 |
| send-money/send-money | 12.647 [12.385, 14.243] | 13.388 [12.306, 13.579] | 1.059 | 0.843 | 0.890 |
| variant-04/05-larger-mix | 34.126 [33.873, 35.765] | 33.536 [32.158, 34.655] | 0.983 | 0.232 | 0.227 |

Reference wall time, ms, same notation.

| Cell | before | after |
|---|---:|---:|
| independent-choice-12 | 6.785 [5.956, 7.019] | 6.712 [5.880, 6.773] |
| independent-choice-16 | 26.084 [24.565, 27.638] | 27.734 [26.795, 28.762] |
| independent-negation-8 | 3.826 [3.470, 5.683] | 4.591 [3.429, 4.619] |
| independent-negation-10 | 4.523 [4.517, 4.536] | 4.540 [4.483, 4.872] |
| independent-negation-aggregate-16 | 31.184 [30.601, 31.334] | 30.035 [29.930, 30.751] |
| disjunction-12 | 23.307 [21.485, 23.350] | 22.132 [21.954, 22.170] |
| ties-50 | 17.914 [16.822, 19.942] | 17.048 [16.827, 18.000] |
| transitive-path-100 | 7.796 [7.731, 8.911] | 8.896 [8.842, 10.135] |
| transitive-path-200 | 23.037 [21.639, 23.038] | 22.764 [22.651, 22.926] |
| transitive-dense-40 | 6.613 [5.715, 6.653] | 5.608 [5.582, 6.662] |
| chain-1000 | 6.667 [6.555, 7.734] | 6.625 [6.616, 7.709] |
| chain-2000 | 9.902 [9.768, 9.999] | 9.894 [9.822, 9.955] |
| chain-arithmetic-1000 | 5.550 [4.502, 6.727] | 4.452 [4.425, 4.517] |
| stratified-16 | 4.476 [3.357, 4.896] | 4.371 [4.368, 4.495] |
| producer-chain-700 | 10.833 [8.729, 10.925] | 9.755 [8.647, 9.761] |
| latin-square-5 | 15.576 [14.676, 16.637] | 16.469 [15.602, 16.803] |
| planning-14 | 17.883 [14.571, 18.002] | 15.060 [14.608, 16.215] |
| n-queens/variant-01 8→10 | 30.127 [30.111, 31.176] | 31.159 [30.205, 32.517] |
| n-queens/variant-01 8→11 | 181.997 [180.833, 182.164] | 180.069 [178.378, 180.070] |
| n-queens/variant-04 8→11 | 176.399 [176.357, 176.674] | 176.450 [174.207, 176.812] |
| send-money/send-money | 15.005 [14.089, 15.272] | 15.039 [14.092, 15.255] |
| variant-04/05-larger-mix | 147.151 [145.539, 173.075] | 147.681 [145.871, 151.616] |

Counters of report after: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 227172 | 3.957 |
| independent-choice-16 | 0 | 2584 | 2584 | 2117111 | 22.504 |
| independent-negation-8 | 0 | 55 | 55 | 28784 | 6.429 |
| independent-negation-10 | 0 | 144 | 144 | 91687 | 20.661 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 170945 | 18.244 |
| disjunction-12 | 0 | 4096 | 4096 | 2029095 | 18.902 |
| ties-50 | 0 | 1225 | 1225 | 897205 | 17.968 |
| transitive-path-100 | 0 | 1 | 1 | 111564 | 6.929 |
| transitive-path-200 | 0 | 1 | 1 | 664776 | 17.566 |
| transitive-dense-40 | 0 | 1 | 1 | 770070 | 28.843 |
| chain-1000 | 0 | 1 | 1 | 211573 | 13.583 |
| chain-2000 | 0 | 1 | 1 | 736073 | 26.534 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 82123 | 6.138 |
| stratified-16 | 0 | 1 | 1 | 1669 | 1.232 |
| producer-chain-700 | 0 | 1 | 1 | 48256 | 21.327 |
| latin-square-5 | 0 | 1344 | 1344 | 791445 | 10.024 |
| planning-14 | 0 | 3432 | 3432 | 2522390 | 33.712 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 3452280 | 22.145 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 16394772 | 83.676 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 15197393 | 75.110 |
| send-money/send-money | 0 | 1 | 1 | 409024 | 9.048 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 2225392 | 29.399 |

Against the reference: report before, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 12 of 22 cells where both passed (54.5%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 34.126 | 147.151 | 0.232 | 3.524 | 32.829 | 10.809 | 85.000 | 57.000 |
| n-queens/variant-04 8→11 | 77.470 | 176.399 | 0.439 | 1.482 | 221.375 | 16.584 | 1.000 | 171.000 |
| n-queens/variant-01 8→11 | 85.670 | 181.997 | 0.471 | 5.481 | 219.755 | 25.056 | 2.000 | 175.000 |
| independent-negation-aggregate-16 | 19.954 | 31.184 | 0.640 | 0.218 | 3.964 | 3.852 | 0.000 | 27.000 |
| stratified-16 | 3.561 | 4.476 | 0.796 | n/a | 0.384 | 0.168 | 0.000 | 0.000 |
| independent-choice-12 | 5.659 | 6.785 | 0.834 | n/a | 0.468 | 1.929 | 0.000 | 2.000 |
| send-money/send-money | 12.647 | 15.005 | 0.843 | 2.506 | 6.240 | 0.282 | 9.000 | 1.000 |
| n-queens/variant-01 8→10 | 25.686 | 30.127 | 0.853 | 3.864 | 47.511 | 5.518 | 1.000 | 24.000 |
| latin-square-5 | 13.528 | 15.576 | 0.869 | 1.211 | 12.292 | 9.120 | 1.000 | 10.000 |
| transitive-path-200 | 21.721 | 23.037 | 0.943 | n/a | 0.004 | 7.524 | 13.000 | 4.000 |
| independent-choice-16 | 24.795 | 26.084 | 0.951 | n/a | 2.660 | 11.334 | 1.000 | 21.000 |
| disjunction-12 | 22.860 | 23.307 | 0.981 | 0.114 | 2.517 | 35.559 | 0.000 | 18.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| ties-50 | 21.939 | 17.914 | 1.225 | 0.329 | 12.701 | 2.621 | 0.000 | 13.000 |
| chain-arithmetic-1000 | 9.940 | 5.550 | 1.791 | 3.725 | 0.821 | 0.156 | 1.000 | 0.000 |
| transitive-path-100 | 15.002 | 7.796 | 1.924 | n/a | 0.006 | 1.938 | 4.000 | 0.000 |
| planning-14 | 36.061 | 17.883 | 2.016 | 3.797 | 42.180 | 16.746 | 1.000 | 13.000 |
| producer-chain-700 | 25.866 | 10.833 | 2.388 | n/a | 0.007 | 2.829 | 5.000 | 0.000 |
| chain-1000 | 17.303 | 6.667 | 2.595 | n/a | 0.004 | 2.298 | 3.000 | 0.000 |
| chain-2000 | 32.253 | 9.902 | 3.257 | n/a | 0.005 | 3.837 | 6.000 | 0.000 |
| independent-negation-8 | 14.226 | 3.826 | 3.718 | n/a | 9.602 | 0.538 | 0.000 | 0.000 |
| transitive-dense-40 | 32.334 | 6.613 | 4.889 | 15.379 | 7.463 | 1.272 | 2.000 | 0.000 |
| independent-negation-10 | 38.774 | 4.523 | 8.573 | n/a | 33.857 | 1.304 | 1.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 16.3 | 23.4 | n/a |
| n-queens/variant-04 8→11 | 13.0 | 11.8 | n/a |
| n-queens/variant-01 8→11 | 14.3 | 11.7 | n/a |
| independent-negation-aggregate-16 | 11.1 | 10.3 | n/a |
| stratified-16 | 9.5 | 10.2 | n/a |
| independent-choice-12 | 8.9 | 10.3 | n/a |
| send-money/send-money | 14.6 | 12.7 | n/a |
| n-queens/variant-01 8→10 | 13.3 | 10.7 | n/a |
| latin-square-5 | 11.8 | 10.7 | n/a |
| transitive-path-200 | 14.9 | 13.6 | n/a |
| independent-choice-16 | 9.1 | 10.4 | n/a |
| disjunction-12 | 10.9 | 10.2 | n/a |
| ties-50 | 13.1 | 10.5 | n/a |
| chain-arithmetic-1000 | 11.8 | 10.5 | n/a |
| transitive-path-100 | 10.7 | 11.0 | n/a |
| planning-14 | 11.7 | 10.3 | n/a |
| producer-chain-700 | 25.1 | 12.6 | n/a |
| chain-1000 | 15.1 | 10.7 | n/a |
| chain-2000 | 22.2 | 11.2 | n/a |
| independent-negation-8 | 9.1 | 10.4 | n/a |
| transitive-dense-40 | 22.4 | 10.7 | n/a |
| independent-negation-10 | 9.1 | 10.4 | n/a |

Against the reference: report after, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 10 of 22 cells where both passed (45.4%).

Wins, fastest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 33.536 | 147.681 | 0.227 | 3.182 | 32.979 | 12.887 | 84.000 | 58.000 |
| n-queens/variant-04 8→11 | 78.348 | 176.450 | 0.444 | 1.453 | 223.563 | 17.799 | 1.000 | 171.000 |
| n-queens/variant-01 8→11 | 87.663 | 180.069 | 0.487 | 5.626 | 222.688 | 27.901 | 2.000 | 173.000 |
| independent-negation-aggregate-16 | 21.967 | 30.035 | 0.731 | 0.223 | 4.182 | 4.420 | 0.000 | 26.000 |
| stratified-16 | 3.423 | 4.371 | 0.783 | n/a | 0.237 | 0.178 | 0.000 | 0.000 |
| n-queens/variant-01 8→10 | 25.960 | 31.159 | 0.833 | 3.866 | 47.413 | 6.232 | 2.000 | 24.000 |
| latin-square-5 | 13.820 | 16.469 | 0.839 | 1.219 | 12.109 | 9.811 | 1.000 | 10.000 |
| send-money/send-money | 13.388 | 15.039 | 0.890 | 2.529 | 5.965 | 0.286 | 9.000 | 1.000 |
| independent-choice-16 | 25.294 | 27.734 | 0.912 | n/a | 2.908 | 10.874 | 0.000 | 23.000 |
| transitive-path-200 | 20.941 | 22.764 | 0.920 | n/a | 0.008 | 6.939 | 14.000 | 4.000 |

Losses, closest first. Milliseconds: ours and the reference's medians and their ratio; ours split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | ours | reference | ours/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| independent-choice-12 | 6.796 | 6.712 | 1.012 | n/a | 0.531 | 1.986 | 0.000 | 2.000 |
| disjunction-12 | 22.685 | 22.132 | 1.025 | 0.133 | 2.747 | 38.049 | 0.000 | 17.000 |
| transitive-path-100 | 10.262 | 8.896 | 1.154 | n/a | 0.006 | 2.193 | 4.000 | 1.000 |
| ties-50 | 22.285 | 17.048 | 1.307 | 0.347 | 12.882 | 2.842 | 1.000 | 13.000 |
| independent-negation-8 | 9.425 | 4.591 | 2.053 | n/a | 5.295 | 0.509 | 0.000 | 0.000 |
| chain-arithmetic-1000 | 10.476 | 4.452 | 2.353 | 3.630 | 0.803 | 0.151 | 1.000 | 0.000 |
| planning-14 | 37.891 | 15.060 | 2.516 | 3.078 | 42.524 | 18.565 | 1.000 | 10.000 |
| chain-1000 | 18.469 | 6.625 | 2.788 | n/a | 0.008 | 2.059 | 3.000 | 0.000 |
| producer-chain-700 | 27.902 | 9.755 | 2.860 | n/a | 0.009 | 2.791 | 5.000 | 0.000 |
| chain-2000 | 32.511 | 9.894 | 3.286 | n/a | 0.009 | 3.995 | 6.000 | 0.000 |
| independent-negation-10 | 23.501 | 4.540 | 5.176 | n/a | 18.099 | 1.321 | 1.000 | 0.000 |
| transitive-dense-40 | 33.787 | 5.608 | 6.025 | 16.548 | 7.872 | 1.275 | 2.000 | 0.000 |

Peak memory, MiB: the resident set of ours and of the reference over the memory rounds, and the device memory ours accounted.

| Cell | ours | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 15.8 | 23.6 | n/a |
| n-queens/variant-04 8→11 | 12.6 | 11.8 | n/a |
| n-queens/variant-01 8→11 | 14.2 | 11.8 | n/a |
| independent-negation-aggregate-16 | 10.8 | 10.5 | n/a |
| stratified-16 | 8.4 | 10.2 | n/a |
| n-queens/variant-01 8→10 | 12.6 | 10.8 | n/a |
| latin-square-5 | 11.3 | 10.6 | n/a |
| send-money/send-money | 14.3 | 12.4 | n/a |
| independent-choice-16 | 8.6 | 10.3 | n/a |
| transitive-path-200 | 14.3 | 13.5 | n/a |
| independent-choice-12 | 8.3 | 10.3 | n/a |
| disjunction-12 | 10.5 | 10.2 | n/a |
| transitive-path-100 | 9.9 | 11.0 | n/a |
| ties-50 | 12.7 | 10.4 | n/a |
| independent-negation-8 | 8.4 | 10.3 | n/a |
| chain-arithmetic-1000 | 11.2 | 10.5 | n/a |
| planning-14 | 11.1 | 10.5 | n/a |
| chain-1000 | 14.5 | 10.8 | n/a |
| producer-chain-700 | 24.1 | 12.5 | n/a |
| chain-2000 | 21.4 | 11.0 | n/a |
| independent-negation-10 | 8.9 | 10.3 | n/a |
| transitive-dense-40 | 21.7 | 10.6 | n/a |
