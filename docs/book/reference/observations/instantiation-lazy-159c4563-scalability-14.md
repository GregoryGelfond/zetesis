# scalability: zetesis 14 threads, clingo one thread

Baseline `7c717c05` and candidate `159c4563`. Wall times are milliseconds: median [minimum, maximum] over seven timed fresh processes. RSS is the median peak in MiB from three separate memory runs. Both builds use two warmups. Cases/profiles are zero-based. The companion evidence JSON supplies complete hashes, sample arrays, phases and qualification fingerprints. Suites overlap.

| Case/profile and complete entry | Parameters | Baseline zetesis | Candidate zetesis | Baseline clingo | Candidate clingo | Zetesis RSS B → C | Clingo RSS B → C |
|---|---|---:|---:|---:|---:|---:|---:|
| 0/0: `scalability/n-queens.lp` | n=8 | 15.396 [15.323, 16.569] | 13.860 [13.837, 13.925] | 6.197 [6.175, 6.279] | 6.231 [6.179, 6.356] | 15.969 → 15.938 | 5.422 → 5.422 |
| 1/0: `scalability/n-queens.lp` | n=9 | 21.393 [21.354, 21.652] | 18.436 [18.332, 20.013] | 9.227 [7.663, 9.306] | 9.244 [7.668, 9.298] | 17.234 → 17.281 | 5.469 → 5.469 |
| 2/0: `scalability/n-queens.lp` | n=10 | 33.988 [32.276, 34.294] | 27.896 [27.410, 28.432] | 28.795 [28.748, 28.851] | 28.771 [28.589, 28.866] | 22.281 → 23.234 | 5.688 → 5.688 |
| 3/0: `scalability/pigeonhole.lp` | h=5 | 7.892 [6.310, 7.922] | 7.898 [6.330, 7.945] | 4.656 [4.635, 4.678] | 4.662 [4.641, 4.688] | 14.203 → 14.062 | 5.359 → 5.359 |
| 4/0: `scalability/pigeonhole.lp` | h=6 | 7.792 [7.736, 7.917] | 7.801 [7.747, 7.949] | 6.178 [6.154, 6.306] | 6.186 [5.982, 6.226] | 15.016 → 14.938 | 5.391 → 5.391 |
| 5/0: `scalability/pigeonhole.lp` | h=7 | 11.644 [9.784, 13.135] | 10.956 [10.582, 11.642] | 18.230 [16.693, 18.247] | 16.717 [16.706, 16.755] | 16.141 → 16.094 | 5.625 → 5.625 |
| 6/0: `standalone/n-queens/variant-02.lp` | n=8 | 33.652 [32.006, 35.216] | 33.824 [31.824, 35.462] | 122.151 [121.804, 123.526] | 122.123 [122.020, 125.212] | 27.469 → 32.922 | 8.625 → 8.203 |
| 7/0: `standalone/send-money/send-money.lp` | default source | 15.424 [15.366, 15.621] | 15.469 [15.388, 16.658] | 12.212 [12.197, 12.252] | 12.229 [12.205, 13.800] | 21.047 → 20.594 | 8.281 → 8.281 |
| 8/0: `scenarios/task-allocation/variant-04/05-larger-mix.lp` | default source | 38.300 [38.093, 38.571] | 36.978 [36.607, 38.502] | 189.908 [179.385, 193.083] | 176.451 [166.004, 192.953] | 26.438 → 27.250 | 19.422 → 19.406 |
| 9/0: `einstein-riddle.lp` | default source | 20.026 [19.956, 20.093] | 20.054 [18.473, 20.080] | 36.322 [36.304, 36.386] | 36.316 [34.794, 36.391] | 20.656 → 20.594 | 19.469 → 19.469 |

## Grounding, solving and driver intervals

Median milliseconds, baseline → candidate. Native grounding/solving use the recorded typed stages; driver uses the maintained comparison. Lazy grounding is interleaved with solving, so its separate grounding stage is unavailable. Native driver excludes source loading and statistics output. Clingo non-solving is its reported Total−Solve, including grounding and preprocessing; it is not independently measured pure grounding. These scopes differ from process wall time. Fine source phases retained in the evidence JSON are nested intervals and must not be added to these totals.

| Case/profile and complete entry | Parameters | Native grounding B → C | Native solving B → C | Native driver B → C | Clingo non-solving B → C | Clingo solving B → C |
|---|---|---:|---:|---:|---:|---:|
| 0/0: `scalability/n-queens.lp` | n=8 | 7.988 → 6.003 | 0.627 → 0.812 | 9.665 → 7.690 | 1.000 → 1.000 | 1.000 → 1.000 |
| 1/0: `scalability/n-queens.lp` | n=9 | 12.491 → 9.405 | 1.617 → 1.711 | 16.066 → 12.795 | 1.000 → 1.000 | 3.000 → 3.000 |
| 2/0: `scalability/n-queens.lp` | n=10 | 19.057 → 13.976 | 5.599 → 5.690 | 27.845 → 22.589 | 2.000 → 2.000 | 23.000 → 23.000 |
| 3/0: `scalability/pigeonhole.lp` | h=5 | 0.416 → 0.372 | 0.433 → 0.401 | 1.332 → 1.262 | 1.000 → 1.000 | 0.000 → 0.000 |
| 4/0: `scalability/pigeonhole.lp` | h=6 | 0.519 → 0.441 | 0.948 → 0.963 | 1.910 → 1.832 | 1.000 → 1.000 | 1.000 → 1.000 |
| 5/0: `scalability/pigeonhole.lp` | h=7 | 0.686 → 0.571 | 4.786 → 4.667 | 5.868 → 5.683 | 0.000 → 0.000 | 13.000 → 13.000 |
| 6/0: `standalone/n-queens/variant-02.lp` | n=8 | 6.810 → 6.220 | 19.205 → 20.408 | 27.129 → 27.693 | 1.000 → 1.000 | 117.000 → 116.000 |
| 7/0: `standalone/send-money/send-money.lp` | default source | 5.171 → 5.208 | 2.618 → 2.581 | 9.200 → 9.158 | 7.000 → 7.000 | 1.000 → 1.000 |
| 8/0: `scenarios/task-allocation/variant-04/05-larger-mix.lp` | default source | 7.205 → 5.548 | 13.016 → 13.514 | 32.246 → 30.755 | 75.000 → 74.000 | 111.000 → 99.000 |
| 9/0: `einstein-riddle.lp` | default source | 7.979 → 7.935 | 3.370 → 3.272 | 12.977 → 12.875 | 29.000 → 29.000 | 1.000 → 1.000 |

All reported positions passed; all source inputs were unchanged. The full native qualification-family fingerprint matches before and after for every row. Clingo qualification compares displayed answers and costs; it cannot establish equality of hidden clingo interpretations. Its reported non-solving time includes preprocessing.
