Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=metal, grounder=eager, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | A1-main | B1-candidate | B2-candidate | A2-main | A2-main/A1-main | A2-main/B2-candidate | B1-candidate/A1-main | B2-candidate/B1-candidate | A1-main/reference | B1-candidate/reference | B2-candidate/reference | A2-main/reference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| send-money/send-money | 65.708 [65.683, 68.288] | 26.703 [26.584, 29.286] | 29.230 [26.735, 29.251] | 68.262 [68.254, 68.329] | 1.039 | 2.335 | 0.406 | 1.095 | 5.654 | 2.303 | 2.517 | 5.883 |
| n-queens/variant-02 | 104.666 [104.643, 107.120] | 150.958 [150.908, 152.239] | 151.001 [150.775, 152.231] | 108.402 [108.316, 109.565] | 1.036 | 0.718 | 1.442 | 1.000 | 0.885 | 1.252 | 1.251 | 0.898 |
| variant-04/05-larger-mix | 599.765 [598.361, 606.399] | 275.666 [269.186, 278.108] | 266.884 [266.639, 273.186] | 606.040 [605.897, 620.464] | 1.010 | 2.271 | 0.460 | 0.968 | 3.430 | 1.554 | 1.686 | 3.622 |

Reference wall time, ms, same notation.

| Cell | A1-main | B1-candidate | B2-candidate | A2-main |
|---|---:|---:|---:|---:|
| send-money/send-money | 11.622 [11.546, 11.626] | 11.597 [11.505, 11.627] | 11.615 [11.522, 11.643] | 11.603 [11.549, 11.620] |
| n-queens/variant-02 | 118.223 [116.846, 118.300] | 120.585 [119.516, 120.815] | 120.736 [119.344, 120.797] | 120.734 [119.559, 120.771] |
| variant-04/05-larger-mix | 174.848 [160.848, 182.327] | 177.337 [168.410, 184.845] | 158.313 [157.038, 166.030] | 167.322 [155.819, 176.070] |

Counters of report A2-main: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| send-money/send-money | 0 | 1 | 1 | 3267961 | 61.503 |
| n-queens/variant-02 | 0 | 92 | 92 | 19099683 | 102.479 |
| variant-04/05-larger-mix | 0 | 1176 | 1208 | 70803794 | 597.314 |

Against the reference: report A1-main, profile 0 (backend=metal, grounder=eager, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 1 of 3 cells where both passed (33.3%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-02 | 104.666 | 118.223 | 0.885 | 9.067 | 71.576 | 5.663 | 1.000 | 112.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 599.765 | 174.848 | 3.430 | 3.110 | 65.069 | 171.201 | 67.000 | 103.000 |
| send-money/send-money | 65.708 | 11.622 | 5.654 | 17.582 | 11.869 | 20.331 | 6.000 | 1.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| n-queens/variant-02 | 24.4 | 8.6 | 2.3 |
| variant-04/05-larger-mix | 32.0 | 19.7 | 2.2 |
| send-money/send-money | 36.4 | 8.4 | 1.8 |

Against the reference: report B1-candidate, profile 0 (backend=metal, grounder=eager, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 0 of 3 cells where both passed (0.0%).

Wins, fastest first: none.

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-02 | 150.958 | 120.585 | 1.252 | 1.691 | 123.701 | 8.794 | 1.000 | 115.000 |
| variant-04/05-larger-mix | 275.666 | 177.337 | 1.554 | 2.506 | 29.913 | 165.940 | 66.000 | 107.000 |
| send-money/send-money | 26.703 | 11.597 | 2.303 | 2.322 | 3.343 | 3.694 | 6.000 | 1.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| n-queens/variant-02 | 28.1 | 9.1 | 2.3 |
| variant-04/05-larger-mix | 30.3 | 19.7 | 2.2 |
| send-money/send-money | 27.0 | 8.4 | 0.3 |

Against the reference: report B2-candidate, profile 0 (backend=metal, grounder=eager, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 0 of 3 cells where both passed (0.0%).

Wins, fastest first: none.

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-02 | 151.001 | 120.736 | 1.251 | 1.674 | 123.964 | 9.034 | 1.000 | 115.000 |
| variant-04/05-larger-mix | 266.884 | 158.313 | 1.686 | 2.516 | 30.085 | 157.379 | 67.000 | 88.000 |
| send-money/send-money | 29.230 | 11.615 | 2.517 | 2.323 | 3.313 | 5.845 | 6.000 | 1.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| n-queens/variant-02 | 28.1 | 8.3 | 2.3 |
| variant-04/05-larger-mix | 30.3 | 21.8 | 2.2 |
| send-money/send-money | 27.0 | 8.4 | 0.3 |

Against the reference: report A2-main, profile 0 (backend=metal, grounder=eager, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 1 of 3 cells where both passed (33.3%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-02 | 108.402 | 120.734 | 0.898 | 9.318 | 72.659 | 9.037 | 1.000 | 115.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 606.040 | 167.322 | 3.622 | 3.115 | 65.430 | 175.005 | 68.000 | 95.000 |
| send-money/send-money | 68.262 | 11.603 | 5.883 | 17.922 | 12.109 | 21.843 | 6.000 | 1.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| n-queens/variant-02 | 24.4 | 8.3 | 2.3 |
| variant-04/05-larger-mix | 32.0 | 19.8 | 2.2 |
| send-money/send-money | 36.4 | 8.4 | 1.8 |
