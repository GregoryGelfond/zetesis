Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=8, completion workers=4, batch=64.

| Cell | main | after |
|---|---:|---:|
| independent-choice-12 | blocked by invocation_failure ×3 | 21.031 [20.781, 24.254] |
| independent-choice-16 | blocked by invocation_failure ×3 | 179.971 [178.326, 183.765] |
| independent-negation-8 | blocked by invocation_failure ×3 | 73.272 [73.202, 81.905] |
| independent-negation-10 | blocked by invocation_failure ×3 | 955.228 [945.530, 978.955] |
| independent-negation-aggregate-16 | 312.010 [310.920, 351.390] | 348.784 [325.986, 355.791] |
| disjunction-12 | 214.549 [213.300, 215.180] | 213.159 [206.304, 216.414] |
| ties-50 | 180.209 [161.015, 187.498] | 185.164 [171.012, 185.177] |
| transitive-path-100 | 33.179 [32.405, 34.203] | 32.412 [30.636, 39.276] |
| transitive-path-200 | 184.354 [184.328, 191.246] | 179.891 [178.790, 180.391] |
| transitive-dense-40 | 40.027 [38.712, 42.408] | 38.143 [37.382, 40.943] |
| chain-1000 | 100.195 [98.959, 100.620] | 97.745 [96.518, 97.911] |
| chain-2000 | 357.699 [355.355, 360.180] | 343.753 [343.634, 347.291] |
| chain-arithmetic-1000 | 9.880 [9.839, 11.298] | 9.993 [9.937, 10.094] |
| stratified-16 | blocked by invocation_failure ×3 | blocked by timeout ×3 |
| producer-chain-700 | 25.325 [23.643, 25.916] | 24.805 [24.755, 27.287] |
| n-queens/variant-01 8→10 | 124.886 [122.815, 126.511] | 122.363 [118.759, 123.959] |
| n-queens/variant-01 8→11 | 452.245 [452.190, 457.499] | 463.921 [459.036, 465.867] |
| n-queens/variant-04 8→11 | 310.712 [306.829, 314.830] | 312.045 [311.649, 313.950] |
| send-money/send-money | 50.293 [49.516, 52.352] | 50.046 [49.402, 50.140] |
| variant-04/05-larger-mix | 340.419 [338.894, 345.058] | 334.132 [332.603, 334.240] |

Reference wall time, ms, same notation.

| Cell | main | after |
|---|---:|---:|
| independent-choice-12 | 7.085 [6.389, 7.088] | 6.797 [6.165, 6.822] |
| independent-choice-16 | 27.700 [26.412, 27.788] | 27.738 [27.102, 28.613] |
| independent-negation-8 | 4.769 [4.634, 4.773] | 5.651 [4.499, 5.703] |
| independent-negation-10 | 4.714 [3.528, 5.688] | 5.669 [4.520, 6.736] |
| independent-negation-aggregate-16 | 32.903 [31.497, 33.933] | 30.307 [28.777, 32.248] |
| disjunction-12 | 23.339 [20.896, 23.506] | 23.350 [19.950, 23.478] |
| ties-50 | 18.925 [18.144, 18.967] | 19.054 [18.964, 19.106] |
| transitive-path-100 | 8.889 [8.888, 8.912] | 8.831 [7.810, 8.926] |
| transitive-path-200 | 22.181 [22.140, 22.738] | 22.510 [21.620, 22.662] |
| transitive-dense-40 | 6.683 [6.668, 6.717] | 6.680 [6.673, 6.698] |
| chain-1000 | 7.697 [7.673, 7.705] | 7.714 [7.678, 7.729] |
| chain-2000 | 11.260 [9.823, 12.073] | 11.286 [9.828, 11.304] |
| chain-arithmetic-1000 | 5.537 [4.536, 5.713] | 4.690 [4.519, 5.542] |
| stratified-16 | 4.436 [4.413, 4.515] | 4.472 [3.373, 5.611] |
| producer-chain-700 | 9.782 [8.690, 9.802] | 9.815 [9.763, 10.849] |
| n-queens/variant-01 8→10 | 30.358 [30.002, 31.129] | 30.645 [30.512, 30.654] |
| n-queens/variant-01 8→11 | 180.633 [179.138, 181.179] | 182.241 [181.730, 183.893] |
| n-queens/variant-04 8→11 | 176.481 [175.311, 176.886] | 178.998 [178.146, 179.412] |
| send-money/send-money | 15.433 [14.091, 15.618] | 14.075 [14.061, 14.242] |
| variant-04/05-larger-mix | 147.474 [147.311, 149.491] | 147.793 [146.562, 149.729] |

Counters of report after: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | 1103492 | 18.107 |
| independent-choice-16 | 0 | 2584 | 2584 | 10522469 | 176.297 |
| independent-negation-8 | 0 | 55 | 14080 | 34020957 | 70.422 |
| independent-negation-10 | 0 | 144 | 147456 | 470272972 | 952.052 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 4956117 | 343.847 |
| disjunction-12 | 0 | 4096 | 4096 | 12273049 | 207.590 |
| ties-50 | 0 | 1225 | 1225 | 8053295 | 180.073 |
| transitive-path-100 | 0 | 1 | 1 | 3434089 | 29.265 |
| transitive-path-200 | 0 | 1 | 1 | 24161820 | 175.793 |
| transitive-dense-40 | 0 | 1 | 1 | 2345227 | 34.448 |
| chain-1000 | 0 | 1 | 1 | 8945042 | 93.312 |
| chain-2000 | 0 | 1 | 1 | 34955080 | 337.500 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 205173 | 6.025 |
| stratified-16 | 0 | blocked by timeout ×3 | | | |
| producer-chain-700 | 0 | 1 | 1 | 88672 | 21.934 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 17317112 | 118.229 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 85071370 | 460.393 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 55533813 | 308.102 |
| send-money/send-money | 0 | 1 | 1 | 3629789 | 45.927 |
| variant-04/05-larger-mix | 0 | 1176 | 1208 | 44210323 | 329.618 |
