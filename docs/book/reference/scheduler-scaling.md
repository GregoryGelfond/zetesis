# CPU candidate-region scheduling

The 24 September 2026 measurements show better high-worker performance on
several larger workloads, alongside regressions on smaller ones. These are
CPU results with eager grounding, indexed formula joins and region search.
Every native run used tight-support membership checking. The compared changes
include work distribution, packed region and knowledge storage, and the work
lease size; the measurements do not isolate one mechanism.

The nine workloads are authored queens at n=8, 9 and 10; authored pigeonhole
at h=5, 6 and 7; and the unchanged corpus cases queens variant 02,
SEND+MORE=MONEY and task allocation variant 04/05. Einstein was not included.
The authored sizes use
checked constant amendments to the original fixtures. Reports retain original
and derived source hashes, amendments and executable identities. Each binary
was measured at 1, 2, 4, 8 and 14 candidate workers, with one completion worker
and a batch size of 64.

Two complete blocks ran in opposite binary order: `469d4d87 → 2845af43 →
ce8dc47b`, then the reverse. Each case/worker cell had a qualification, one
warmup, four timed fresh processes and two separate RSS processes per block.
All requested positions passed. Clingo 5.8.2 qualified complete displayed answer
families and costs; native full models agreed across profiles and repetitions
within each binary's campaign. Clingo supplied no timing or RSS population.

Wall time includes process launch, input, grounding, solving and captured
JSON/statistics output. The table shows milliseconds, with **block 1 / block 2**
medians kept separate. Each comparison uses the same worker count.

| Workload | Workers | Baseline `469d4d87` | `ce8dc47b` |
| --- | ---: | ---: | ---: |
| Corpus queens variant 02 | 14 | 150.63 / 152.86 | 54.19 / 51.37 |
| Authored queens n=10 | 14 | 49.97 / 50.76 | 26.85 / 25.34 |
| Task allocation | 14 | 55.78 / 57.52 | 33.71 / 33.75 |
| Corpus queens variant 02 | 4 | 57.85 / 57.64 | 51.39 / 51.40 |
| Authored queens n=9 | 4 | 13.03 / 13.47 | 16.81 / 14.60 |
| Authored pigeonhole h=5 | 4 | 6.24 / 6.24 | 7.60 / 7.64 |
| SEND+MORE=MONEY | 4 | 10.78 / 11.31 | 13.39 / 13.43 |

Relative to its own one-worker time, `ce8dc47b` reached about 2.6× at 14 workers
on corpus queens variant 02, 1.9× on authored queens n=10 and 1.6× on task
allocation. Increasing workers did not help every case: pigeonhole h=7 took
18.34 / 18.33 ms at 14 workers, versus 15.25 / 15.25 ms at one worker. These
curves support choosing worker counts by workload, rather than expecting
linear scaling. The earlier scheduler binary `2845af43` already showed the
large high-worker improvements; for corpus queens variant 02 at 14 workers its
medians were 52.91 / 53.01 ms.

Memory also depends on the workload and worker count. For corpus queens variant
02 at 14 workers, the median of the two separate peak-RSS observations fell
from 35.14 / 34.74 MiB to 27.34 / 27.50 MiB. These are host process resource
measurements. Worker phase totals can overlap and must not be added as
sequential wall-time components.

Other project activity was reported on the shared host. Reversing the block
order helps expose drift but does not establish isolated host conditions or
statistical significance. The results describe these nine tight-support CPU
workloads; they do not establish general residual-search or device performance.
Einstein's Riddle is outside this population; its grounding and term-representation
costs require a separate comparison.

The [complete observation projection](observations/scheduler-ce8dc47b-evidence.json)
contains all nine workloads, five worker counts and both blocks for all three
binaries, including every timed and RSS sample and every requested outcome.
It records full-report hashes and sizes, source and executable identities,
profiles, limits and host load observations. It omits private paths, process
identifiers and raw output streams. Numeric summaries can be recomputed from
this projection; rechecking complete answer-family equality requires the
retained raw captures or a new acquisition. These results remain attached to
the measured revisions below, including `ce8dc47b`, when later code changes.

## Measured identities

The baseline runtime comes from `469d4d87`; `5b766879` is its documentation-only
promotion. The same `ce8dc47b` command executable managed every campaign and RSS
helper, including measurements of the other native binaries.

| Runtime source commit | Executable SHA-256 |
| --- | --- |
| `469d4d871c9753f58e573a135ce89b53afdd9874` | `3d81eb3d55695c7ea4710e8b165a909a5853b6da22167f306545f584ba2d9d78` |
| `2845af431b2a90bcbc09d660d4e8f9dc61485d0d` | `93207e0a8a6d696f4b7214ce83e8b2dc6a8acae69a0217f6435af6ce85a3c195` |
| `ce8dc47ba08d204534380e9eb6d644c8cdd89ab3` | `d86871302822c7ee1964230cf772a68534611cf036144f2883158ef36c810056` |

The clingo executable SHA-256 is
`31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015`.
The common correctness catalog SHA-256 is
`b43df1adf17ae0c035f1e310a5c15345c26cbcad8b59596932627c46fd1c6958`;
authored fixture identities are recorded separately per workload.

## Reproduce the selection

Use the maintained CLI from the checkout containing this scalability suite,
with the same source roots for every binary. Set `NATIVE` to the executable
being measured and `RUN` to a new output prefix. Retain one command executable
for all campaigns. First qualify the nine complete families:

```sh
zetesis test scalability examples/correctness --examples examples \
  --zetesis "$NATIVE" --clingo clingo --threads 1,2,4,8,14 \
  --timeout-seconds 30 --campaign-seconds 1800 --capture-bytes 8388608 \
  --total-capture-bytes 536870912 --report-bytes 1073741824 \
  --report "${RUN}-check.json"
```

Then measure with separate timing and memory rounds:

```sh
zetesis bench corpus examples/correctness --suite scalability --examples examples \
  --zetesis "$NATIVE" --native-interface solve --clingo clingo --clingo-threads 1 \
  --device cpu --grounder eager --compare-threads 1,2,4,8,14 \
  --completion-workers 1 --batch-size 64 --warmups 1 --repetitions 4 --memory-runs 2 \
  --timeout-seconds 30 --campaign-seconds 1800 \
  --sample-bytes 8388608 --native-report-bytes 8388608 \
  --capture-bytes 536870912 --report-bytes 1073741824 \
  --report "${RUN}.json" --json > "${RUN}-summary.json"
```

Run the three binary versions serially in the two orders above, with a new
output prefix for each. Keep the default native work limits unchanged and
retain any refusal. Compare full benchmark reports from each block with
`zetesis bench compare`, for example:

```sh
zetesis bench compare \
  --report baseline=baseline-block1.json \
  --report scheduler=scheduler-block1.json \
  --report current=current-block1.json \
  --report-bytes 1073741824 --output block1-comparison.json --json
```

The full reports retain raw captures, limits, source and binary identities,
outcomes, phase observations, timings and memory samples. Keep those reports
and the host/activity record with the measurement record; compact summaries
alone are insufficient to reproduce or audit the comparison.
