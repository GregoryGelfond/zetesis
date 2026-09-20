Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=metal, grounder=eager, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | A1-main | B1-candidate | B2-candidate | A2-main | A2-main/A1-main | A2-main/B2-candidate | B1-candidate/A1-main | B2-candidate/B1-candidate | A1-main/reference | B1-candidate/reference | B2-candidate/reference | A2-main/reference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| send-money/send-money | 27.968 [26.665, 29.295] | 31.820 [31.791, 31.834] | 31.788 [31.782, 31.815] | 29.275 [26.709, 30.308] | 1.047 | 0.921 | 1.138 | 0.999 | 2.411 | 2.739 | 2.742 | 2.524 |
| n-queens/variant-02 | 154.796 [154.742, 155.989] | 99.590 [98.404, 101.786] | 100.974 [98.460, 104.699] | 154.805 [154.699, 156.043] | 1.000 | 1.533 | 0.643 | 1.014 | 1.258 | 0.799 | 0.819 | 1.243 |
| variant-04/05-larger-mix | 278.458 [277.034, 280.566] | 98.710 [96.268, 102.739] | 97.290 [93.744, 97.613] | 275.861 [274.037, 275.970] | 0.991 | 2.835 | 0.354 | 0.986 | 1.652 | 0.585 | 0.556 | 1.624 |

Reference wall time, ms, same notation.

| Cell | A1-main | B1-candidate | B2-candidate | A2-main |
|---|---:|---:|---:|---:|
| send-money/send-money | 11.599 [11.539, 11.609] | 11.616 [11.538, 11.621] | 11.593 [11.418, 11.616] | 11.600 [11.524, 11.611] |
| n-queens/variant-02 | 123.095 [121.980, 123.333] | 124.575 [123.238, 124.587] | 123.321 [121.969, 124.520] | 124.546 [123.254, 124.645] |
| variant-04/05-larger-mix | 168.582 [166.081, 169.765] | 168.645 [168.454, 174.898] | 174.851 [168.306, 179.742] | 169.844 [169.522, 179.731] |

Counters of report A2-main: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| send-money/send-money | 0 | 1 | 1 | 409024 | 21.835 |
| n-queens/variant-02 | 0 | 92 | 92 | 14461401 | 147.455 |
| variant-04/05-larger-mix | 0 | 1176 | 1208 | 15235864 | 267.430 |

Against the reference: report A1-main, profile 0 (backend=metal, grounder=eager, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 0 of 3 cells where both passed (0.0%).

Wins, fastest first: none.

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-02 | 154.796 | 123.095 | 1.258 | 1.752 | 126.199 | 9.127 | 1.000 | 117.000 |
| variant-04/05-larger-mix | 278.458 | 168.582 | 1.652 | 2.648 | 30.671 | 167.040 | 70.000 | 96.000 |
| send-money/send-money | 27.968 | 11.599 | 2.411 | 2.365 | 3.506 | 3.919 | 6.000 | 1.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| n-queens/variant-02 | 28.6 | 8.7 | 2.3 |
| variant-04/05-larger-mix | 30.4 | 19.8 | 2.2 |
| send-money/send-money | 27.0 | 8.4 | 0.3 |

Against the reference: report B1-candidate, profile 0 (backend=metal, grounder=eager, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 2 of 3 cells where both passed (66.6%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 98.710 | 168.645 | 0.585 | 2.668 | 16.553 | 46.738 | 69.000 | 97.000 |
| n-queens/variant-02 | 99.590 | 124.575 | 0.799 | 1.748 | 69.538 | 11.973 | 1.000 | 118.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| send-money/send-money | 31.820 | 11.616 | 2.739 | 2.429 | 2.472 | 9.282 | 6.000 | 1.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 33.4 | 19.7 | 1.2 |
| n-queens/variant-02 | 33.4 | 8.3 | 1.2 |
| send-money/send-money | 28.9 | 8.4 | 0.2 |

Against the reference: report B2-candidate, profile 0 (backend=metal, grounder=eager, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 2 of 3 cells where both passed (66.6%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 97.290 | 174.851 | 0.556 | 2.633 | 14.621 | 46.485 | 68.000 | 100.000 |
| n-queens/variant-02 | 100.974 | 123.321 | 0.819 | 1.766 | 69.503 | 11.992 | 1.000 | 118.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| send-money/send-money | 31.788 | 11.593 | 2.742 | 2.372 | 2.422 | 9.086 | 6.000 | 1.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 33.5 | 19.7 | 1.2 |
| n-queens/variant-02 | 33.8 | 8.7 | 1.2 |
| send-money/send-money | 29.4 | 8.4 | 0.2 |

Against the reference: report A2-main, profile 0 (backend=metal, grounder=eager, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 0 of 3 cells where both passed (0.0%).

Wins, fastest first: none.

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| n-queens/variant-02 | 154.805 | 124.546 | 1.243 | 1.773 | 127.372 | 9.064 | 1.000 | 119.000 |
| variant-04/05-larger-mix | 275.861 | 169.844 | 1.624 | 2.640 | 30.840 | 164.262 | 71.000 | 96.000 |
| send-money/send-money | 29.275 | 11.600 | 2.524 | 2.440 | 3.550 | 6.019 | 6.000 | 1.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| n-queens/variant-02 | 28.0 | 8.9 | 2.3 |
| variant-04/05-larger-mix | 30.4 | 19.6 | 2.2 |
| send-money/send-money | 27.1 | 8.4 | 0.3 |
