# series: zetesis 14 threads, clingo one thread

Baseline `7c717c05` and candidate `159c4563`. Wall times are milliseconds: median [minimum, maximum] over seven timed fresh processes. RSS is the median peak in MiB from three separate memory runs. Both builds use two warmups. Cases/profiles are zero-based. The companion evidence JSON supplies complete hashes, sample arrays, phases and qualification fingerprints. Suites overlap.

| Case/profile and complete entry | Parameters | Baseline zetesis | Candidate zetesis | Baseline clingo | Candidate clingo | Zetesis RSS B → C | Clingo RSS B → C |
|---|---|---:|---:|---:|---:|---:|---:|
| 0/0: `generated/independent-choice-12.lp` | independent-choice; size=12 | 10.928 [10.833, 11.231] | 10.937 [9.314, 11.186] | 7.832 [7.748, 7.934] | 7.842 [7.791, 7.914] | 12.281 → 12.125 | 5.094 → 5.094 |
| 1/0: `generated/independent-choice-16.lp` | independent-choice; size=16 | 36.823 [35.037, 38.168] | 33.884 [33.420, 35.364] | 36.065 [35.127, 36.092] | 36.056 [35.759, 36.097] | 12.312 → 12.297 | 5.094 → 5.094 |
| 2/0: `generated/independent-negation-8.lp` | independent-negation; size=8 | 18.431 [18.331, 18.595] | 16.885 [16.594, 17.040] | 4.703 [4.673, 4.728] | 4.703 [4.670, 4.722] | 11.859 → 11.750 | 5.016 → 5.016 |
| 3/0: `generated/independent-negation-10.lp` | independent-negation; size=10 | 45.420 [43.924, 48.851] | 42.214 [40.886, 42.498] | 4.696 [4.659, 4.731] | 4.703 [4.663, 4.714] | 12.141 → 12.062 | 5.031 → 5.031 |
| 4/0: `generated/independent-negation-aggregate-16.lp` | independent-negation-aggregate; size=16 | 28.059 [26.333, 28.176] | 28.098 [26.364, 28.158] | 43.563 [42.824, 43.778] | 43.497 [42.284, 43.756] | 14.500 → 14.422 | 5.219 → 5.219 |
| 5/0: `generated/disjunction-12.lp` | disjunction; size=12 | 27.676 [26.135, 27.933] | 26.278 [26.050, 26.422] | 31.281 [31.127, 31.390] | 31.303 [31.074, 31.424] | 14.094 → 14.047 | 5.047 → 5.047 |
| 6/0: `generated/ties-50.lp` | ties; size=50 | 24.645 [23.308, 25.141] | 24.430 [23.377, 24.874] | 21.795 [21.442, 21.895] | 21.832 [21.743, 21.876] | 18.125 → 17.969 | 5.500 → 5.500 |
| 7/0: `generated/transitive-path-100.lp` | transitive-path; size=100 | 18.693 [17.036, 18.797] | 17.180 [17.018, 18.791] | 7.800 [7.721, 9.341] | 7.777 [7.715, 7.821] | 13.859 → 13.703 | 6.031 → 6.031 |
| 8/0: `generated/transitive-path-200.lp` | transitive-path; size=200 | 50.868 [50.596, 55.308] | 51.987 [50.674, 56.872] | 20.080 [19.958, 21.374] | 19.949 [18.396, 20.136] | 21.500 → 21.391 | 8.859 → 8.859 |
| 9/0: `generated/transitive-dense-40.lp` | transitive-dense; size=40 | 36.445 [36.201, 37.758] | 35.194 [33.488, 36.390] | 6.252 [6.186, 6.293] | 6.243 [6.180, 6.268] | 27.781 → 27.719 | 5.375 → 5.375 |
| 10/0: `generated/chain-1000.lp` | chain; size=1000 | 19.997 [19.940, 20.072] | 20.036 [19.923, 21.582] | 7.665 [6.147, 7.752] | 6.162 [6.153, 7.725] | 18.375 → 18.266 | 5.438 → 5.438 |
| 11/0: `generated/chain-2000.lp` | chain; size=2000 | 35.099 [33.478, 36.708] | 36.546 [34.990, 44.251] | 9.251 [9.173, 10.766] | 9.249 [9.166, 10.826] | 26.094 → 25.938 | 5.938 → 5.938 |
| 12/0: `generated/chain-arithmetic-1000.lp` | chain-arithmetic; size=1000 | 19.982 [19.899, 21.014] | 16.988 [16.706, 17.067] | 4.657 [4.633, 4.702] | 4.647 [4.231, 4.678] | 16.062 → 15.734 | 5.203 → 5.203 |
| 13/0: `generated/stratified-16.lp` | stratified; size=16 | 6.290 [6.235, 9.372] | 6.287 [6.222, 6.317] | 4.659 [4.644, 4.725] | 4.660 [4.649, 4.692] | 11.766 → 11.766 | 5.062 → 5.062 |
| 14/0: `generated/producer-chain-700.lp` | producer-chain; size=700 | 21.371 [21.303, 23.001] | 22.840 [21.319, 22.887] | 10.720 [9.171, 10.771] | 9.203 [9.169, 10.538] | 30.375 → 30.375 | 7.203 → 7.203 |
| 15/0: `generated/latin-square-5.lp` | latin-square; size=5 | 16.978 [16.651, 18.826] | 15.450 [15.399, 17.088] | 18.753 [18.376, 20.330] | 18.736 [18.612, 20.318] | 16.812 → 16.453 | 5.500 → 5.500 |
| 16/0: `generated/planning-14.lp` | planning; size=14 | 42.839 [42.753, 43.789] | 41.313 [41.253, 41.443] | 18.370 [16.823, 19.547] | 18.341 [16.817, 18.516] | 15.859 → 15.688 | 5.438 → 5.438 |
| 17/0: `standalone/n-queens/variant-01.lp` | n=10 | 34.383 [33.740, 35.983] | 29.271 [27.836, 29.741] | 28.854 [28.794, 30.382] | 28.827 [28.792, 30.402] | 20.609 → 24.109 | 5.688 → 5.688 |
| 18/0: `standalone/n-queens/variant-01.lp` | n=11 | 66.733 [63.230, 68.792] | 56.758 [55.472, 61.367] | 185.462 [183.919, 188.582] | 183.899 [183.837, 185.571] | 23.891 → 31.688 | 7.281 → 7.281 |
| 19/0: `standalone/n-queens/variant-04.lp` | n=11 | 41.730 [38.279, 43.253] | 40.130 [38.461, 43.076] | 181.072 [179.422, 184.052] | 179.583 [177.937, 181.082] | 23.578 → 23.875 | 7.562 → 7.250 |
| 20/0: `standalone/send-money/send-money.lp` | default source | 15.482 [15.378, 17.081] | 15.441 [15.094, 16.578] | 12.223 [12.203, 13.839] | 12.201 [12.189, 12.234] | 19.703 → 19.875 | 8.281 → 8.281 |
| 21/0: `scenarios/task-allocation/variant-04/05-larger-mix.lp` | default source | 38.223 [37.748, 40.311] | 36.767 [35.357, 37.871] | 188.597 [178.909, 206.515] | 190.071 [177.651, 197.643] | 25.438 → 25.969 | 19.359 → 20.734 |

## Grounding, solving and driver intervals

Median milliseconds, baseline → candidate. Native grounding/solving use the recorded typed stages; driver uses the maintained comparison. Lazy grounding is interleaved with solving, so its separate grounding stage is unavailable. Native driver excludes source loading and statistics output. Clingo non-solving is its reported Total−Solve, including grounding and preprocessing; it is not independently measured pure grounding. These scopes differ from process wall time. Fine source phases retained in the evidence JSON are nested intervals and must not be added to these totals.

| Case/profile and complete entry | Parameters | Native grounding B → C | Native solving B → C | Native driver B → C | Clingo non-solving B → C | Clingo solving B → C |
|---|---|---:|---:|---:|---:|---:|
| 0/0: `generated/independent-choice-12.lp` | independent-choice; size=12 | unavailable → unavailable | 3.659 → 3.550 | 4.788 → 4.639 | 0.000 → 0.000 | 3.000 → 3.000 |
| 1/0: `generated/independent-choice-16.lp` | independent-choice; size=16 | unavailable → unavailable | 25.097 → 23.825 | 30.264 → 28.937 | 0.000 → 0.000 | 31.000 → 31.000 |
| 2/0: `generated/independent-negation-8.lp` | independent-negation; size=8 | unavailable → unavailable | 11.853 → 10.911 | 12.316 → 11.397 | 0.000 → 0.000 | 0.000 → 0.000 |
| 3/0: `generated/independent-negation-10.lp` | independent-negation; size=10 | unavailable → unavailable | 38.883 → 35.576 | 39.579 → 36.273 | 1.000 → 1.000 | 0.000 → 0.000 |
| 4/0: `generated/independent-negation-aggregate-16.lp` | independent-negation-aggregate; size=16 | 0.395 → 0.393 | 5.311 → 5.539 | 21.318 → 21.193 | 0.000 → 0.000 | 39.000 → 39.000 |
| 5/0: `generated/disjunction-12.lp` | disjunction; size=12 | 0.216 → 0.218 | 10.741 → 10.475 | 20.407 → 20.255 | 1.000 → 0.000 | 26.000 → 27.000 |
| 6/0: `generated/ties-50.lp` | ties; size=50 | 0.574 → 0.578 | 6.498 → 6.452 | 17.510 → 17.472 | 1.000 → 0.000 | 16.000 → 17.000 |
| 7/0: `generated/transitive-path-100.lp` | transitive-path; size=100 | unavailable → unavailable | 8.315 → 8.275 | 12.052 → 11.272 | 3.000 → 3.000 | 1.000 → 0.000 |
| 8/0: `generated/transitive-path-200.lp` | transitive-path; size=200 | unavailable → unavailable | 35.828 → 36.464 | 44.809 → 46.025 | 9.000 → 9.000 | 6.000 → 6.000 |
| 9/0: `generated/transitive-dense-40.lp` | transitive-dense; size=40 | 20.658 → 19.579 | 5.788 → 5.721 | 29.831 → 28.318 | 2.000 → 2.000 | 0.000 → 0.000 |
| 10/0: `generated/chain-1000.lp` | chain; size=1000 | unavailable → unavailable | 5.113 → 5.289 | 13.759 → 13.955 | 3.000 → 3.000 | 0.000 → 0.000 |
| 11/0: `generated/chain-2000.lp` | chain; size=2000 | unavailable → unavailable | 10.948 → 11.303 | 28.726 → 30.091 | 5.000 → 5.000 | 0.000 → 0.000 |
| 12/0: `generated/chain-arithmetic-1000.lp` | chain-arithmetic; size=1000 | 11.794 → 8.196 | 1.237 → 1.222 | 14.273 → 11.179 | 1.000 → 1.000 | 0.000 → 0.000 |
| 13/0: `generated/stratified-16.lp` | stratified; size=16 | unavailable → unavailable | 0.860 → 0.843 | 1.364 → 1.345 | 0.000 → 0.000 | 0.000 → 0.000 |
| 14/0: `generated/producer-chain-700.lp` | producer-chain; size=700 | unavailable → unavailable | 8.660 → 9.422 | 15.774 → 17.190 | 5.000 → 5.000 | 0.000 → 0.000 |
| 15/0: `generated/latin-square-5.lp` | latin-square; size=5 | 2.262 → 1.919 | 4.514 → 4.388 | 11.380 → 10.076 | 1.000 → 1.000 | 13.000 → 14.000 |
| 16/0: `generated/planning-14.lp` | planning; size=14 | 10.436 → 8.804 | 10.813 → 10.970 | 36.973 → 35.386 | 1.000 → 1.000 | 13.000 → 13.000 |
| 17/0: `standalone/n-queens/variant-01.lp` | n=10 | 18.932 → 13.995 | 5.617 → 5.731 | 27.808 → 22.686 | 1.000 → 1.000 | 24.000 → 24.000 |
| 18/0: `standalone/n-queens/variant-01.lp` | n=11 | 27.445 → 20.428 | 22.618 → 21.159 | 60.534 → 50.728 | 2.000 → 1.000 | 179.000 → 178.000 |
| 19/0: `standalone/n-queens/variant-04.lp` | n=11 | 4.038 → 3.435 | 22.012 → 20.447 | 35.257 → 33.560 | 1.000 → 2.000 | 175.000 → 174.000 |
| 20/0: `standalone/send-money/send-money.lp` | default source | 5.193 → 5.167 | 2.611 → 2.624 | 9.323 → 9.252 | 7.000 → 7.000 | 1.000 → 1.000 |
| 21/0: `scenarios/task-allocation/variant-04/05-larger-mix.lp` | default source | 7.293 → 5.606 | 13.238 → 13.783 | 32.103 → 30.836 | 74.000 → 75.000 | 110.000 → 111.000 |

All reported positions passed; all source inputs were unchanged. The full native qualification-family fingerprint matches before and after for every row. Clingo qualification compares displayed answers and costs; it cannot establish equality of hidden clingo interpretations. Its reported non-solving time includes preprocessing.
