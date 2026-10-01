# series: zetesis 4 threads, clingo one thread

Baseline `7c717c05` and candidate `159c4563`. Wall times are milliseconds: median [minimum, maximum] over seven timed fresh processes. RSS is the median peak in MiB from three separate memory runs. Both builds use two warmups. Cases/profiles are zero-based. The companion evidence JSON supplies complete hashes, sample arrays, phases and qualification fingerprints. Suites overlap.

| Case/profile and complete entry | Parameters | Baseline zetesis | Candidate zetesis | Baseline clingo | Candidate clingo | Zetesis RSS B → C | Clingo RSS B → C |
|---|---|---:|---:|---:|---:|---:|---:|
| 0/0: `generated/independent-choice-12.lp` | independent-choice; size=12 | 9.360 [9.318, 11.049] | 9.370 [9.331, 9.534] | 7.855 [7.817, 7.918] | 7.861 [7.801, 7.907] | 11.547 → 11.438 | 5.094 → 5.094 |
| 1/0: `generated/independent-choice-16.lp` | independent-choice; size=16 | 38.106 [37.127, 38.112] | 35.128 [35.018, 36.811] | 35.897 [35.680, 35.983] | 35.973 [35.806, 36.191] | 11.609 → 11.594 | 5.094 → 5.094 |
| 2/0: `generated/independent-negation-8.lp` | independent-negation; size=8 | 18.413 [16.800, 18.522] | 16.929 [16.812, 17.065] | 4.697 [4.682, 4.743] | 4.722 [4.666, 4.725] | 11.344 → 11.344 | 5.016 → 5.016 |
| 3/0: `generated/independent-negation-10.lp` | independent-negation; size=10 | 43.901 [42.362, 46.906] | 40.881 [40.841, 40.953] | 4.707 [4.403, 4.729] | 4.708 [4.680, 4.728] | 11.500 → 11.484 | 5.031 → 5.031 |
| 4/0: `generated/independent-negation-aggregate-16.lp` | independent-negation-aggregate; size=16 | 26.468 [26.012, 26.580] | 26.543 [26.034, 27.821] | 43.594 [43.491, 43.818] | 43.601 [42.334, 43.795] | 14.172 → 14.109 | 5.219 → 5.219 |
| 5/0: `generated/disjunction-12.lp` | disjunction; size=12 | 26.331 [25.832, 26.532] | 26.528 [26.407, 26.678] | 31.225 [31.014, 31.306] | 31.330 [31.046, 31.423] | 13.672 → 13.547 | 5.047 → 5.047 |
| 6/0: `generated/ties-50.lp` | ties; size=50 | 22.976 [22.671, 23.513] | 23.135 [22.877, 23.370] | 21.792 [21.731, 21.909] | 21.776 [21.693, 21.828] | 16.906 → 16.812 | 5.500 → 5.500 |
| 7/0: `generated/transitive-path-100.lp` | transitive-path; size=100 | 17.145 [17.055, 17.228] | 17.105 [17.039, 17.226] | 7.776 [7.701, 7.796] | 7.767 [7.699, 7.808] | 13.562 → 13.578 | 6.031 → 6.031 |
| 8/0: `generated/transitive-path-200.lp` | transitive-path; size=200 | 50.574 [49.140, 54.254] | 50.725 [50.606, 52.339] | 18.413 [18.277, 20.128] | 19.957 [18.354, 20.099] | 21.172 → 21.156 | 8.859 → 8.859 |
| 9/0: `generated/transitive-dense-40.lp` | transitive-dense; size=40 | 35.066 [34.953, 36.556] | 35.198 [33.425, 35.226] | 6.235 [6.181, 6.270] | 6.222 [6.177, 6.254] | 27.750 → 27.594 | 5.375 → 5.375 |
| 10/0: `generated/chain-1000.lp` | chain; size=1000 | 19.972 [18.411, 19.997] | 19.978 [19.941, 20.024] | 6.176 [6.149, 7.737] | 6.162 [6.147, 7.715] | 18.109 → 18.016 | 5.438 → 5.438 |
| 11/0: `generated/chain-2000.lp` | chain; size=2000 | 33.573 [33.489, 35.189] | 35.054 [34.985, 36.573] | 9.269 [9.151, 9.308] | 9.243 [9.166, 9.304] | 25.859 → 25.703 | 5.938 → 5.938 |
| 12/0: `generated/chain-arithmetic-1000.lp` | chain-arithmetic; size=1000 | 19.952 [19.920, 20.029] | 16.966 [16.913, 17.027] | 4.670 [4.648, 4.709] | 4.656 [4.652, 4.685] | 15.984 → 15.766 | 5.203 → 5.203 |
| 13/0: `generated/stratified-16.lp` | stratified; size=16 | 6.266 [6.233, 6.279] | 6.252 [6.202, 6.297] | 4.649 [4.637, 4.674] | 4.656 [4.651, 4.698] | 11.500 → 11.516 | 5.062 → 5.062 |
| 14/0: `generated/producer-chain-700.lp` | producer-chain; size=700 | 21.360 [19.801, 21.381] | 21.341 [21.277, 22.871] | 9.197 [9.160, 10.709] | 9.196 [9.184, 10.732] | 30.250 → 30.156 | 7.203 → 7.203 |
| 15/0: `generated/latin-square-5.lp` | latin-square; size=5 | 16.978 [15.469, 17.030] | 15.495 [15.450, 15.543] | 18.726 [18.555, 20.115] | 18.746 [18.622, 18.827] | 15.594 → 15.719 | 5.500 → 5.500 |
| 16/0: `generated/planning-14.lp` | planning; size=14 | 42.776 [41.937, 42.831] | 41.148 [40.877, 41.444] | 18.365 [16.832, 18.587] | 18.271 [16.830, 18.522] | 15.156 → 15.047 | 5.438 → 5.438 |
| 17/0: `standalone/n-queens/variant-01.lp` | n=10 | 40.761 [40.421, 41.554] | 35.920 [34.856, 36.266] | 28.825 [28.788, 28.870] | 28.833 [28.800, 28.854] | 17.375 → 17.234 | 5.688 → 5.688 |
| 18/0: `standalone/n-queens/variant-01.lp` | n=11 | 97.831 [97.302, 98.401] | 91.378 [89.248, 91.892] | 182.381 [179.330, 184.088] | 182.489 [182.233, 184.023] | 18.719 → 18.750 | 7.531 → 7.266 |
| 19/0: `standalone/n-queens/variant-04.lp` | n=11 | 73.717 [72.281, 74.908] | 72.982 [72.224, 73.803] | 178.055 [176.454, 179.507] | 178.027 [177.869, 179.522] | 17.375 → 17.266 | 7.188 → 7.203 |
| 20/0: `standalone/send-money/send-money.lp` | default source | 15.434 [15.391, 15.531] | 15.491 [15.406, 15.517] | 12.206 [11.967, 12.300] | 12.205 [12.186, 12.226] | 19.141 → 18.828 | 8.281 → 8.281 |
| 21/0: `scenarios/task-allocation/variant-04/05-larger-mix.lp` | default source | 37.972 [36.030, 38.514] | 35.397 [34.978, 36.643] | 188.445 [159.511, 192.907] | 181.099 [162.838, 189.927] | 22.609 → 22.391 | 19.453 → 19.406 |

## Grounding, solving and driver intervals

Median milliseconds, baseline → candidate. Native grounding/solving use the recorded typed stages; driver uses the maintained comparison. Lazy grounding is interleaved with solving, so its separate grounding stage is unavailable. Native driver excludes source loading and statistics output. Clingo non-solving is its reported Total−Solve, including grounding and preprocessing; it is not independently measured pure grounding. These scopes differ from process wall time. Fine source phases retained in the evidence JSON are nested intervals and must not be added to these totals.

| Case/profile and complete entry | Parameters | Native grounding B → C | Native solving B → C | Native driver B → C | Clingo non-solving B → C | Clingo solving B → C |
|---|---|---:|---:|---:|---:|---:|
| 0/0: `generated/independent-choice-12.lp` | independent-choice; size=12 | unavailable → unavailable | 3.472 → 3.209 | 4.350 → 4.056 | 0.000 → 0.000 | 3.000 → 3.000 |
| 1/0: `generated/independent-choice-16.lp` | independent-choice; size=16 | unavailable → unavailable | 28.056 → 25.946 | 32.671 → 30.325 | 0.000 → 0.000 | 31.000 → 31.000 |
| 2/0: `generated/independent-negation-8.lp` | independent-negation; size=8 | unavailable → unavailable | 11.434 → 10.634 | 11.813 → 10.977 | 0.000 → 0.000 | 0.000 → 0.000 |
| 3/0: `generated/independent-negation-10.lp` | independent-negation; size=10 | unavailable → unavailable | 38.088 → 34.954 | 38.579 → 35.429 | 1.000 → 1.000 | 0.000 → 0.000 |
| 4/0: `generated/independent-negation-aggregate-16.lp` | independent-negation-aggregate; size=16 | 0.391 → 0.389 | 4.845 → 5.184 | 20.753 → 20.651 | 0.000 → 0.000 | 39.000 → 39.000 |
| 5/0: `generated/disjunction-12.lp` | disjunction; size=12 | 0.216 → 0.215 | 5.720 → 5.252 | 20.346 → 20.663 | 1.000 → 1.000 | 26.000 → 26.000 |
| 6/0: `generated/ties-50.lp` | ties; size=50 | 0.573 → 0.574 | 5.199 → 5.195 | 17.034 → 16.984 | 0.000 → 0.000 | 17.000 → 17.000 |
| 7/0: `generated/transitive-path-100.lp` | transitive-path; size=100 | unavailable → unavailable | 7.992 → 8.181 | 11.136 → 10.966 | 2.000 → 2.000 | 1.000 → 1.000 |
| 8/0: `generated/transitive-path-200.lp` | transitive-path; size=200 | unavailable → unavailable | 35.579 → 36.086 | 44.707 → 44.794 | 9.000 → 9.000 | 5.000 → 6.000 |
| 9/0: `generated/transitive-dense-40.lp` | transitive-dense; size=40 | 20.286 → 19.496 | 5.571 → 5.629 | 28.385 → 28.269 | 2.000 → 2.000 | 0.000 → 0.000 |
| 10/0: `generated/chain-1000.lp` | chain; size=1000 | unavailable → unavailable | 5.002 → 5.131 | 13.752 → 13.767 | 3.000 → 3.000 | 0.000 → 0.000 |
| 11/0: `generated/chain-2000.lp` | chain; size=2000 | unavailable → unavailable | 10.858 → 11.087 | 27.141 → 28.722 | 5.000 → 5.000 | 0.000 → 0.000 |
| 12/0: `generated/chain-arithmetic-1000.lp` | chain-arithmetic; size=1000 | 11.558 → 8.094 | 1.057 → 1.071 | 14.236 → 11.251 | 1.000 → 1.000 | 0.000 → 0.000 |
| 13/0: `generated/stratified-16.lp` | stratified; size=16 | unavailable → unavailable | 0.782 → 0.755 | 1.194 → 1.178 | 0.000 → 0.000 | 0.000 → 0.000 |
| 14/0: `generated/producer-chain-700.lp` | producer-chain; size=700 | unavailable → unavailable | 8.334 → 9.170 | 15.651 → 15.867 | 5.000 → 5.000 | 0.000 → 0.000 |
| 15/0: `generated/latin-square-5.lp` | latin-square; size=5 | 2.215 → 1.917 | 2.947 → 2.900 | 10.446 → 10.262 | 1.000 → 1.000 | 14.000 → 13.000 |
| 16/0: `generated/planning-14.lp` | planning; size=14 | 10.303 → 8.520 | 8.430 → 8.015 | 37.077 → 35.418 | 1.000 → 1.000 | 13.000 → 13.000 |
| 17/0: `standalone/n-queens/variant-01.lp` | n=10 | 18.646 → 13.832 | 13.049 → 13.112 | 34.388 → 29.549 | 1.000 → 2.000 | 23.000 → 23.000 |
| 18/0: `standalone/n-queens/variant-01.lp` | n=11 | 26.890 → 20.057 | 56.858 → 57.172 | 91.650 → 85.091 | 2.000 → 1.000 | 176.000 → 177.000 |
| 19/0: `standalone/n-queens/variant-04.lp` | n=11 | 3.953 → 3.430 | 55.698 → 55.323 | 67.636 → 66.748 | 1.000 → 1.000 | 172.000 → 173.000 |
| 20/0: `standalone/send-money/send-money.lp` | default source | 5.052 → 5.136 | 2.648 → 2.658 | 9.040 → 9.224 | 7.000 → 7.000 | 1.000 → 1.000 |
| 21/0: `scenarios/task-allocation/variant-04/05-larger-mix.lp` | default source | 7.192 → 5.544 | 12.873 → 12.869 | 31.692 → 29.412 | 73.000 → 73.000 | 110.000 → 104.000 |

All reported positions passed; all source inputs were unchanged. The full native qualification-family fingerprint matches before and after for every row. Clingo qualification compares displayed answers and costs; it cannot establish equality of hidden clingo interpretations. Its reported non-solving time includes preprocessing.
