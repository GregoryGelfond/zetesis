Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=1, batch=64.

Formula search method by report: baseline=regions; candidate=regions;

| Cell | baseline | candidate | candidate/baseline | baseline/reference | candidate/reference |
|---|---:|---:|---:|---:|---:|
| scalability/n-queens | 16.693 [16.487, 17.919] | 16.707 [16.684, 20.632] | 1.001 | 2.547 | 2.541 |
| scalability/n-queens 8→9 | 23.964 [23.648, 24.342] | 24.062 [22.953, 28.338] | 1.004 | 2.656 | 2.659 |
| scalability/n-queens 8→10 | 41.755 [40.635, 43.019] | 40.452 [40.318, 43.154] | 0.969 | 1.375 | 1.391 |
| scalability/pigeonhole 7→5 | 7.967 [6.564, 8.040] | 7.966 [6.645, 7.991] | 1.000 | 1.531 | 1.534 |
| scalability/pigeonhole 7→6 | 7.844 [7.710, 9.106] | 7.824 [7.502, 7.909] | 0.998 | 1.213 | 1.213 |
| scalability/pigeonhole | 11.641 [11.475, 12.844] | 11.647 [11.462, 12.772] | 1.000 | 0.655 | 0.656 |
| n-queens/variant-02 | 57.409 [56.226, 59.802] | 57.492 [55.271, 58.349] | 1.001 | 0.462 | 0.466 |
| send-money/send-money | 15.470 [15.347, 15.541] | 15.497 [15.469, 16.643] | 1.002 | 1.209 | 1.215 |
| variant-04/05-larger-mix | 37.220 [35.605, 38.335] | 37.242 [35.957, 37.281] | 1.001 | 0.190 | 0.193 |
| einstein-riddle | 19.304 [19.253, 20.617] | 19.353 [19.231, 20.682] | 1.002 | 0.527 | 0.528 |

Reference wall time, ms, same notation.

| Cell | baseline | candidate |
|---|---:|---:|
| scalability/n-queens | 6.554 [6.449, 6.597] | 6.575 [6.490, 6.589] |
| scalability/n-queens 8→9 | 9.022 [8.959, 9.125] | 9.050 [8.752, 9.119] |
| scalability/n-queens 8→10 | 30.359 [29.151, 30.478] | 29.077 [29.040, 30.446] |
| scalability/pigeonhole 7→5 | 5.203 [5.151, 5.237] | 5.194 [5.170, 5.217] |
| scalability/pigeonhole 7→6 | 6.465 [6.412, 6.500] | 6.450 [6.439, 6.494] |
| scalability/pigeonhole | 17.768 [17.715, 17.779] | 17.751 [17.706, 17.779] |
| n-queens/variant-02 | 124.195 [123.281, 125.649] | 123.277 [122.948, 124.566] |
| send-money/send-money | 12.792 [12.741, 14.023] | 12.758 [12.731, 12.854] |
| variant-04/05-larger-mix | 195.957 [171.190, 202.431] | 193.452 [188.807, 200.808] |
| einstein-riddle | 36.654 [36.362, 37.882] | 36.670 [36.651, 36.682] |

Counters of report candidate: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| scalability/n-queens | 0 | 92 | 92 | 454988 | 10.637 |
| scalability/n-queens 8→9 | 0 | 352 | 352 | 2049594 | 18.018 |
| scalability/n-queens 8→10 | 0 | 724 | 724 | 6686540 | 34.737 |
| scalability/pigeonhole 7→5 | 0 | 0 | 0 | 27906 | 1.331 |
| scalability/pigeonhole 7→6 | 0 | 0 | 0 | 147078 | 2.483 |
| scalability/pigeonhole | 0 | 0 | 0 | 1045350 | 6.313 |
| n-queens/variant-02 | 0 | 92 | 92 | 15076428 | 51.720 |
| send-money/send-money | 0 | 1 | 1 | 472288 | 9.197 |
| variant-04/05-larger-mix | 0 | 1176 | 1177 | 8418375 | 30.200 |
| einstein-riddle | 0 | 1 | 1 | 485677 | 13.227 |

Against the reference: report baseline, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=1, batch=64; search regions): faster on 4 of 10 cells where both passed (40.0%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership, each summed over its intervals, which can overlap within or across threads, so the parts need not add up to the native median; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 37.220 | 195.957 | 0.190 | 7.487 | 27.266 | 8.136 | 77.000 | 110.000 |
| n-queens/variant-02 | 57.409 | 124.195 | 0.462 | 6.961 | 129.147 | 0.760 | 1.000 | 119.000 |
| einstein-riddle | 19.304 | 36.654 | 0.527 | 8.066 | 3.271 | 0.404 | 30.000 | 1.000 |
| scalability/pigeonhole | 11.641 | 17.768 | 0.655 | 0.722 | 9.420 | 0.026 | 1.000 | 13.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership, each summed over its intervals, which can overlap within or across threads, so the parts need not add up to the native median; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| send-money/send-money | 15.470 | 12.792 | 1.209 | 5.245 | 3.527 | 0.151 | 7.000 | 1.000 |
| scalability/pigeonhole 7→6 | 7.844 | 6.465 | 1.213 | 0.550 | 1.106 | 0.020 | 0.000 | 2.000 |
| scalability/n-queens 8→10 | 41.755 | 30.359 | 1.375 | 19.236 | 39.021 | 4.197 | 1.000 | 24.000 |
| scalability/pigeonhole 7→5 | 7.967 | 5.203 | 1.531 | 0.437 | 0.233 | 0.015 | 1.000 | 0.000 |
| scalability/n-queens | 16.693 | 6.554 | 2.547 | 8.231 | 2.279 | 0.391 | 1.000 | 1.000 |
| scalability/n-queens 8→9 | 23.964 | 9.022 | 2.656 | 12.794 | 9.622 | 1.768 | 2.000 | 3.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 22.7 | 19.4 | n/a |
| n-queens/variant-02 | 20.9 | 8.2 | n/a |
| einstein-riddle | 20.8 | 19.4 | n/a |
| scalability/pigeonhole | 14.6 | 5.6 | n/a |
| send-money/send-money | 18.7 | 8.2 | n/a |
| scalability/pigeonhole 7→6 | 13.9 | 5.3 | n/a |
| scalability/n-queens 8→10 | 17.3 | 5.8 | n/a |
| scalability/pigeonhole 7→5 | 13.8 | 5.3 | n/a |
| scalability/n-queens | 15.2 | 5.4 | n/a |
| scalability/n-queens 8→9 | 16.0 | 5.4 | n/a |

Against the reference: report candidate, profile 0 (backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=1, batch=64; search regions): faster on 4 of 10 cells where both passed (40.0%).

Wins, fastest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership, each summed over its intervals, which can overlap within or across threads, so the parts need not add up to the native median; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| variant-04/05-larger-mix | 37.242 | 193.452 | 0.193 | 7.589 | 27.035 | 8.431 | 77.000 | 112.000 |
| n-queens/variant-02 | 57.492 | 123.277 | 0.466 | 7.014 | 131.760 | 0.750 | 1.000 | 118.000 |
| einstein-riddle | 19.353 | 36.670 | 0.528 | 8.267 | 3.258 | 0.412 | 30.000 | 1.000 |
| scalability/pigeonhole | 11.647 | 17.751 | 0.656 | 0.714 | 9.385 | 0.026 | 1.000 | 13.000 |

Losses, closest first. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership, each summed over its intervals, which can overlap within or across threads, so the parts need not add up to the native median; the reference's into grounding and solving from its own report.

| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| scalability/pigeonhole 7→6 | 7.824 | 6.450 | 1.213 | 0.550 | 1.119 | 0.023 | 0.000 | 2.000 |
| send-money/send-money | 15.497 | 12.758 | 1.215 | 5.246 | 3.558 | 0.157 | 7.000 | 1.000 |
| scalability/n-queens 8→10 | 40.452 | 29.077 | 1.391 | 18.917 | 39.615 | 4.332 | 1.000 | 24.000 |
| scalability/pigeonhole 7→5 | 7.966 | 5.194 | 1.534 | 0.447 | 0.228 | 0.017 | 1.000 | 0.000 |
| scalability/n-queens | 16.707 | 6.575 | 2.541 | 8.024 | 2.331 | 0.417 | 1.000 | 1.000 |
| scalability/n-queens 8→9 | 24.062 | 9.050 | 2.659 | 12.616 | 9.631 | 1.781 | 2.000 | 3.000 |

Peak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.

| Cell | native | reference | device |
|---|---:|---:|---:|
| variant-04/05-larger-mix | 22.5 | 19.4 | n/a |
| n-queens/variant-02 | 20.7 | 8.5 | n/a |
| einstein-riddle | 20.4 | 20.6 | n/a |
| scalability/pigeonhole | 14.5 | 5.6 | n/a |
| scalability/pigeonhole 7→6 | 13.8 | 5.3 | n/a |
| send-money/send-money | 19.0 | 8.2 | n/a |
| scalability/n-queens 8→10 | 17.1 | 5.6 | n/a |
| scalability/pigeonhole 7→5 | 13.7 | 5.3 | n/a |
| scalability/n-queens | 14.9 | 5.4 | n/a |
| scalability/n-queens 8→9 | 15.8 | 5.4 | n/a |
