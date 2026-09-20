Native wall time, ms: median [minimum, maximum] of the timed intervals; profile backend=cpu, grounder=auto, oracle=auto, workers=4, completion workers=4, batch=64.

| Cell | main |
|---|---:|
| independent-choice-12 | 22.281 [20.710, 24.246] |
| independent-choice-16 | 194.883 [193.961, 200.660] |
| independent-negation-8 | 111.168 [109.778, 113.196] |
| independent-negation-10 | 1424.903 [1416.252, 1449.808] |
| independent-negation-aggregate-16 | 320.798 [303.281, 335.198] |
| disjunction-12 | 219.099 [211.848, 223.459] |
| ties-50 | 180.720 [160.516, 184.805] |
| transitive-path-100 | 35.969 [31.228, 37.940] |
| transitive-path-200 | 186.533 [186.302, 186.631] |
| transitive-dense-40 | 40.432 [39.830, 41.541] |
| chain-1000 | 99.694 [98.817, 100.303] |
| chain-2000 | 358.276 [358.231, 362.006] |
| chain-arithmetic-1000 | 9.930 [9.779, 11.036] |
| stratified-16 | blocked by timeout ×3 |
| producer-chain-700 | 25.928 [25.635, 26.847] |
| n-queens/variant-01 8→10 | 127.251 [126.179, 128.231] |
| n-queens/variant-01 8→11 | 461.981 [448.518, 464.366] |
| n-queens/variant-04 8→11 | 305.064 [301.204, 313.033] |
| send-money/send-money | 50.678 [49.865, 52.000] |
| variant-04/05-larger-mix | 335.213 [334.233, 347.528] |

Reference wall time, ms, same notation.

| Cell | main |
|---|---:|
| independent-choice-12 | 6.825 [6.331, 6.843] |
| independent-choice-16 | 27.694 [27.073, 28.113] |
| independent-negation-8 | 4.620 [4.563, 4.622] |
| independent-negation-10 | 4.643 [4.545, 5.620] |
| independent-negation-aggregate-16 | 31.242 [30.262, 31.702] |
| disjunction-12 | 21.740 [20.751, 22.187] |
| ties-50 | 18.151 [17.886, 19.026] |
| transitive-path-100 | 8.131 [8.116, 8.848] |
| transitive-path-200 | 22.632 [22.108, 22.770] |
| transitive-dense-40 | 6.645 [5.595, 6.646] |
| chain-1000 | 7.673 [6.542, 8.841] |
| chain-2000 | 11.030 [9.873, 11.067] |
| chain-arithmetic-1000 | 5.493 [4.476, 5.565] |
| stratified-16 | 4.444 [4.408, 4.476] |
| producer-chain-700 | 9.780 [8.676, 9.783] |
| n-queens/variant-01 8→10 | 31.529 [29.994, 31.861] |
| n-queens/variant-01 8→11 | 184.039 [182.777, 184.483] |
| n-queens/variant-04 8→11 | 179.809 [179.556, 179.998] |
| send-money/send-money | 14.225 [14.094, 15.136] |
| variant-04/05-larger-mix | 148.690 [148.469, 148.747] |

Counters of report main: published models, candidates examined, charged search work, driver median ms.

| Cell | profile | models | candidates | work | driver ms |
|---|---|---:|---:|---:|---:|
| independent-choice-12 | 0 | 377 | 377 | n/a | 19.945 |
| independent-choice-16 | 0 | 2584 | 2584 | n/a | 191.396 |
| independent-negation-8 | 0 | 55 | 14080 | n/a | 108.076 |
| independent-negation-10 | 0 | 144 | 147456 | n/a | 1422.724 |
| independent-negation-aggregate-16 | 0 | 2584 | 2584 | 4956117 | 316.630 |
| disjunction-12 | 0 | 4096 | 4096 | 12273049 | 215.454 |
| ties-50 | 0 | 1225 | 1225 | 8053295 | 176.344 |
| transitive-path-100 | 0 | 1 | 1 | n/a | 32.497 |
| transitive-path-200 | 0 | 1 | 1 | n/a | 183.233 |
| transitive-dense-40 | 0 | 1 | 1 | 2345227 | 36.358 |
| chain-1000 | 0 | 1 | 1 | n/a | 95.049 |
| chain-2000 | 0 | 1 | 1 | n/a | 353.055 |
| chain-arithmetic-1000 | 0 | 1 | 1 | 205173 | 6.443 |
| stratified-16 | 0 | blocked by timeout ×3 | | | |
| producer-chain-700 | 0 | 1 | 1 | n/a | 20.991 |
| n-queens/variant-01 8→10 | 0 | 724 | 724 | 17317112 | 123.118 |
| n-queens/variant-01 8→11 | 0 | 2680 | 2680 | 85071370 | 457.978 |
| n-queens/variant-04 8→11 | 0 | 2680 | 2680 | 55533813 | 301.267 |
| send-money/send-money | 0 | 1 | 1 | 3629789 | 46.882 |
| variant-04/05-larger-mix | 0 | 1176 | 1208 | 44210323 | 331.439 |
