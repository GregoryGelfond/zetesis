Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=eager, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | A1-main | B1-candidate | B2-candidate | A2-main | A2-main/A1-main | A2-main/B2-candidate | B1-candidate/A1-main | B2-candidate/B1-candidate | A1-main/reference | B1-candidate/reference | B2-candidate/reference | A2-main/reference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| send-money/send-money | 12.835 [12.559, 12.867] | 12.847 [12.726, 13.026] | 12.822 [11.708, 13.009] | 12.934 [12.826, 13.020] | 1.008 | 1.009 | 1.001 | 0.998 | 0.996 | 0.997 | 0.990 | 1.001 |
| n-queens/variant-02 | 79.023 [76.076, 91.140] | 78.810 [76.315, 78.837] | 80.702 [77.701, 91.524] | 78.767 [74.935, 80.636] | 0.997 | 0.976 | 0.997 | 1.024 | 0.647 | 0.638 | 0.655 | 0.636 |
| variant-04/05-larger-mix | 34.704 [34.443, 35.905] | 35.944 [33.385, 36.985] | 34.626 [34.103, 34.709] | 35.456 [34.589, 35.901] | 1.022 | 1.024 | 1.036 | 0.963 | 0.188 | 0.194 | 0.184 | 0.194 |

Reference wall time, ms, same notation.

| Cell | A1-main | B1-candidate | B2-candidate | A2-main |
|---|---:|---:|---:|---:|
| send-money/send-money | 12.881 [12.823, 12.974] | 12.886 [12.830, 12.911] | 12.948 [12.775, 12.950] | 12.920 [12.804, 12.953] |
| n-queens/variant-02 | 122.177 [121.843, 123.132] | 123.529 [122.936, 124.027] | 123.234 [123.046, 124.250] | 123.830 [122.089, 124.523] |
| variant-04/05-larger-mix | 184.603 [168.868, 191.347] | 185.058 [184.326, 186.126] | 188.330 [188.158, 190.774] | 183.008 [173.255, 185.098] |

Counters of report A2-main: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| send-money/send-money | 0 | 1 | 1 | 472288 | 6.356 |
| n-queens/variant-02 | 0 | 92 | 92 | 15076428 | 72.751 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 8459738 | 28.313 |

Against the reference: report A1-main, profile 0 (backend=cpu, grounder=eager, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 3 of 3 cells where both passed (100.0%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 34.704 | 184.603 | 0.188 | 2.700 | 36.275 | 8.473 | 72.000 | 107.000 |
| n-queens/variant-02 | 79.023 | 122.177 | 0.647 | 1.767 | 226.615 | 0.747 | 1.000 | 116.000 |
| send-money/send-money | 12.835 | 12.881 | 0.996 | 2.408 | 6.173 | 0.147 | 7.000 | 1.000 |

Losses, closest first: none.

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 22.7 | 22.0 | n/a |
| n-queens/variant-02 | 22.3 | 9.2 | n/a |
| send-money/send-money | 18.4 | 8.2 | n/a |

Against the reference: report B1-candidate, profile 0 (backend=cpu, grounder=eager, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 3 of 3 cells where both passed (100.0%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 35.944 | 185.058 | 0.194 | 2.652 | 37.527 | 8.540 | 73.000 | 108.000 |
| n-queens/variant-02 | 78.810 | 123.529 | 0.638 | 1.766 | 223.204 | 0.762 | 1.000 | 117.000 |
| send-money/send-money | 12.847 | 12.886 | 0.997 | 2.460 | 6.524 | 0.146 | 7.000 | 1.000 |

Losses, closest first: none.

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 23.1 | 22.6 | n/a |
| n-queens/variant-02 | 21.9 | 9.2 | n/a |
| send-money/send-money | 18.4 | 8.2 | n/a |

Against the reference: report B2-candidate, profile 0 (backend=cpu, grounder=eager, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 3 of 3 cells where both passed (100.0%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 34.626 | 188.330 | 0.184 | 2.758 | 37.257 | 8.597 | 74.000 | 111.000 |
| n-queens/variant-02 | 80.702 | 123.234 | 0.655 | 1.799 | 224.265 | 0.778 | 1.000 | 117.000 |
| send-money/send-money | 12.822 | 12.948 | 0.990 | 2.359 | 6.116 | 0.145 | 7.000 | 1.000 |

Losses, closest first: none.

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 22.4 | 21.7 | n/a |
| n-queens/variant-02 | 22.1 | 9.9 | n/a |
| send-money/send-money | 18.4 | 8.2 | n/a |

Against the reference: report A2-main, profile 0 (backend=cpu, grounder=eager, oracle=auto, workers=4, completion workers=4, batch=64; search default): faster on 2 of 3 cells where both passed (66.6%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 35.456 | 183.008 | 0.194 | 2.753 | 36.611 | 8.635 | 74.000 | 104.000 |
| n-queens/variant-02 | 78.767 | 123.830 | 0.636 | 1.789 | 219.741 | 0.766 | 1.000 | 118.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| send-money/send-money | 12.934 | 12.920 | 1.001 | 2.447 | 6.035 | 0.153 | 7.000 | 1.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 22.6 | 22.0 | n/a |
| n-queens/variant-02 | 21.9 | 8.6 | n/a |
| send-money/send-money | 18.5 | 8.2 | n/a |
