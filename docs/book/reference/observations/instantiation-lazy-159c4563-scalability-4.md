# scalability: zetesis 4 threads, clingo one thread

Baseline `7c717c05` and candidate `159c4563`. Wall times are milliseconds: median [minimum, maximum] over seven timed fresh processes. RSS is the median peak in MiB from three separate memory runs. Both builds use two warmups. Cases/profiles are zero-based. The companion evidence JSON supplies complete hashes, sample arrays, phases and qualification fingerprints. Suites overlap.

| Case/profile and complete entry | Parameters | Baseline zetesis | Candidate zetesis | Baseline clingo | Candidate clingo | Zetesis RSS B → C | Clingo RSS B → C |
|---|---|---:|---:|---:|---:|---:|---:|
| 0/0: `scalability/n-queens.lp` | n=8 | 16.831 [15.292, 16.882] | 13.793 [13.372, 13.860] | 6.191 [4.639, 6.257] | 6.210 [6.171, 6.296] | 14.906 → 15.234 | 5.422 → 5.422 |
| 1/0: `scalability/n-queens.lp` | n=9 | 22.857 [22.713, 24.415] | 19.816 [19.672, 21.063] | 9.206 [7.664, 9.275] | 8.992 [7.668, 9.289] | 15.797 → 15.953 | 5.469 → 5.469 |
| 2/0: `scalability/n-queens.lp` | n=10 | 40.658 [39.251, 41.667] | 34.920 [34.417, 35.721] | 28.788 [28.760, 28.828] | 28.799 [28.750, 28.904] | 17.219 → 16.906 | 5.688 → 5.688 |
| 3/0: `scalability/pigeonhole.lp` | h=5 | 6.343 [6.280, 7.916] | 6.305 [6.296, 6.360] | 4.666 [4.650, 4.673] | 4.661 [4.645, 4.678] | 13.844 → 13.859 | 5.359 → 5.359 |
| 4/0: `scalability/pigeonhole.lp` | h=6 | 7.745 [7.648, 7.883] | 7.787 [7.586, 7.904] | 6.175 [6.162, 6.187] | 6.165 [6.155, 6.311] | 13.984 → 14.000 | 5.391 → 5.391 |
| 5/0: `scalability/pigeonhole.lp` | h=7 | 12.279 [12.242, 12.304] | 12.283 [10.671, 12.321] | 16.703 [16.683, 18.237] | 16.714 [16.701, 18.228] | 14.625 → 14.547 | 5.625 → 5.625 |
| 6/0: `standalone/n-queens/variant-02.lp` | n=8 | 54.577 [53.434, 55.761] | 54.361 [53.188, 54.855] | 122.117 [122.079, 123.649] | 122.093 [120.592, 122.123] | 21.594 → 21.703 | 8.203 → 7.969 |
| 7/0: `standalone/send-money/send-money.lp` | default source | 15.366 [15.296, 15.455] | 15.393 [15.356, 15.445] | 12.222 [12.197, 12.232] | 12.198 [12.140, 12.232] | 18.984 → 19.094 | 8.281 → 8.281 |
| 8/0: `scenarios/task-allocation/variant-04/05-larger-mix.lp` | default source | 38.226 [36.104, 38.545] | 35.376 [34.411, 36.996] | 186.893 [171.863, 194.550] | 185.394 [170.052, 191.548] | 22.625 → 22.422 | 19.500 → 19.453 |
| 9/0: `einstein-riddle.lp` | default source | 19.964 [19.823, 20.062] | 19.984 [19.866, 20.065] | 36.310 [36.291, 36.395] | 36.332 [34.793, 36.358] | 20.391 → 20.453 | 19.469 → 19.469 |

## Grounding, solving and driver intervals

Median milliseconds, baseline → candidate. Native grounding/solving use the recorded typed stages; driver uses the maintained comparison. Lazy grounding is interleaved with solving, so its separate grounding stage is unavailable. Native driver excludes source loading and statistics output. Clingo non-solving is its reported Total−Solve, including grounding and preprocessing; it is not independently measured pure grounding. These scopes differ from process wall time. Fine source phases retained in the evidence JSON are nested intervals and must not be added to these totals.

| Case/profile and complete entry | Parameters | Native grounding B → C | Native solving B → C | Native driver B → C | Clingo non-solving B → C | Clingo solving B → C |
|---|---|---:|---:|---:|---:|---:|
| 0/0: `scalability/n-queens.lp` | n=8 | 7.967 → 5.963 | 1.835 → 1.851 | 10.705 → 8.669 | 1.000 → 1.000 | 1.000 → 1.000 |
| 1/0: `scalability/n-queens.lp` | n=9 | 12.536 → 9.372 | 3.924 → 4.080 | 17.980 → 14.856 | 1.000 → 1.000 | 3.000 → 3.000 |
| 2/0: `scalability/n-queens.lp` | n=10 | 18.957 → 14.020 | 13.084 → 13.325 | 34.621 → 29.783 | 1.000 → 2.000 | 24.000 → 23.000 |
| 3/0: `scalability/pigeonhole.lp` | h=5 | 0.414 → 0.374 | 0.357 → 0.352 | 1.224 → 1.152 | 1.000 → 1.000 | 0.000 → 0.000 |
| 4/0: `scalability/pigeonhole.lp` | h=6 | 0.508 → 0.436 | 1.436 → 1.445 | 2.353 → 2.281 | 1.000 → 1.000 | 1.000 → 1.000 |
| 5/0: `scalability/pigeonhole.lp` | h=7 | 0.669 → 0.560 | 5.426 → 5.342 | 6.505 → 6.311 | 0.000 → 0.000 | 13.000 → 13.000 |
| 6/0: `standalone/n-queens/variant-02.lp` | n=8 | 6.795 → 6.226 | 41.418 → 41.199 | 49.273 → 48.336 | 1.000 → 1.000 | 117.000 → 116.000 |
| 7/0: `standalone/send-money/send-money.lp` | default source | 5.126 → 5.093 | 2.656 → 2.634 | 9.126 → 9.052 | 7.000 → 7.000 | 1.000 → 1.000 |
| 8/0: `scenarios/task-allocation/variant-04/05-larger-mix.lp` | default source | 7.211 → 5.555 | 12.976 → 12.763 | 32.090 → 29.205 | 75.000 → 72.000 | 111.000 → 109.000 |
| 9/0: `einstein-riddle.lp` | default source | 7.959 → 7.973 | 3.353 → 3.393 | 13.025 → 12.984 | 29.000 → 29.000 | 1.000 → 1.000 |

All reported positions passed; all source inputs were unchanged. The full native qualification-family fingerprint matches before and after for every row. Clingo qualification compares displayed answers and costs; it cannot establish equality of hidden clingo interpretations. Its reported non-solving time includes preprocessing.
