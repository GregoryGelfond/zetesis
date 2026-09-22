# Benchmarks and comparisons

Use a **solver benchmark** to compare the time and memory needed to answer a
program. Use a **primitive benchmark** to investigate one operation, such as
filtering a relation. A faster operation does not necessarily make a complete
solve faster.

## What the results show

The published corpus comparison runs zetesis and clingo on the same programs on
an Apple M4 Pro. Four CPU threads gave zetesis its lowest total time. Most small
cases favored clingo; queens variant 2 and several task-allocation cases favored
zetesis. Metal did not give an overall advantage on these workloads.

These selected CPU results give a sense of scale:

| Program | zetesis, four threads | clingo, one thread |
| --- | ---: | ---: |
| Eight queens, variant 2 | 58.0–62.7 ms | 122.6–123.2 ms |
| SEND + MORE = MONEY | 11.4–11.6 ms | 10.8–11.4 ms |
| Task allocation, larger instance | 31.2–31.3 ms | 174.8–185.3 ms |

Each range spans the medians from two repeated measurement groups, rounded to
0.1 ms. These are complete command-line runs, including input, grounding,
search, statistics and output. They are not kernel timings. The task-allocation
request enumerates every tied optimum; comparing it with a request for one
answer would measure a different task.

The [full CPU and Metal comparison](worker-scaling.md) supplies every case,
memory results, settings, source revisions and raw measurements. Its measured
sources precede the 0.1.4 command-line update; these numbers do not qualify a
newer executable merely because it belongs to the same project.

A subsequent [four-program CPU comparison](grounding-row-lending.md) measures
lending completed grounding rows. Dense40 rule instantiation took 6.4–10.0%
less time; whole-process improvements were smaller, and the controls were mixed.
It does not update the full-corpus or Metal results above.

The [prepared-join comparison](hybrid-grounding.md#reusing-completed-source-join-plans)
measures a 4–5% improvement on one queens workload, mixed short-run results
elsewhere and a small RSS increase.

The [original-region comparison](hybrid-grounding.md#original-region-checks)
measures eager and hybrid CPU profiles before and after early source-constraint
checks. It reduces core candidates on several queens and monotone-choice cases,
but hybrid still trails eager on representative queens workloads; redundant
constraints and four-thread execution expose costs. The
[historical full-candidate results](hybrid-grounding.md#historical-full-candidate-comparison)
retain the earlier storage/search tradeoff. These small-workload CPU experiments
do not replace the full-corpus or Metal measurements.

## Run a benchmark

From a repository checkout, with zetesis and clingo installed:

```sh
zetesis bench corpus examples/kr-domains --suite baseline --report baseline.json
```

This small suite runs SEND, queens variant 2 and task allocation. The terminal
shows a table; the report retains settings, results and measurements. The runner
checks answers before comparing timings and records incomplete or failed runs.
It refuses to overwrite an existing report.

Omit `--suite baseline` to run the full corpus. Select CPU threads or a device
explicitly when comparing configurations:

```sh
zetesis bench corpus examples/kr-domains --threads 2 --report two-threads.json
zetesis bench corpus examples/kr-domains --device metal --grounder eager \
  --report metal.json
zetesis bench corpus examples/kr-domains --json --report run.json > summary.json
```

Benchmarks always collect their required statistics. `solve --stats` is useful
for investigating one run, but a single duration is not a repeatable comparison.
See the [benchmark command options](commands.md#measure-a-corpus) for limits,
repetition counts and other suites. Use `zetesis test corpus` when you want to
check answers without conducting a benchmark.

## Read the measurements

| Measurement | Meaning |
| --- | --- |
| Wall time | Elapsed time for a fresh solver process, including its input and output. |
| Grounding time | Time spent preparing the ground program. Lazy execution can interleave this work with solving. |
| Solving time | The recorded search and answer-set checking interval. |
| Median | The middle repeated duration; with two samples, their mean. |
| Peak RSS | Peak resident host memory reported for a separate process run. It is not GPU memory. |
| Work counters | Algorithm operations, such as joins or candidate checks. Their units differ and cannot be added indiscriminately. |

Memory runs are separate from timed runs. A GPU solve can still perform source
preparation, candidate generation and some exact checks on the CPU. Device
selection therefore does not mean that the whole request runs on the GPU.

Memory captures retain the helper process ID separately from the measured solver
ID. The runner checks that they differ before accepting a measurement. Older
matrix reports can omit the helper ID; their retained data cannot independently
repeat that identity check.

Before comparing two reports, check the program and constants, answer request,
grounder, thread count, device, limits, statistics and output settings. Keep
timeouts and failures visible. A failed run has no successful solve time to
include in an average.

For before/after comparisons, run both executables on the same quiet machine
and alternate their order. The retained reports identify the actual binaries;
version labels alone are insufficient. The
[comparison command](commands.md#compare-reports-and-consume-machine-output) reads saved reports
without rerunning the solver.

## Detailed measurements

Each report below describes a particular experiment. Earlier results retain
their original programs and executables; they are not a cumulative speedup chart.

| Question | Report |
| --- | --- |
| What changes when source constraints are checked during solving? | [Eager and hybrid grounding](hybrid-grounding.md) |
| What does lending completed eager join rows change? | [Grounding row lending](grounding-row-lending.md) |
| How do thread count, CPU and Metal compare with clingo? | [Worker scaling](worker-scaling.md) |
| What changed when CPU and GPU used the same semantic plans? | [Shared-plan execution](plan-execution.md) |
| How did candidate generation and reduct execution change? | [Execution series](execution-series.md), [reduct execution](reduct-execution.md) |
| What did grounding reuse change on CPU and Metal? | [Prepared grounding](prepared-metal.md) |
| What did atom catalogs, table joins and release optimization change? | [Grounding and representation measurements](grounding-measurements.md) |

For small, matched operation measurements, use
[`zetesis bench primitives`](commands.md#measure-primitives). For library callers,
the [measurement guide](../rust/measurements.md) explains how to observe work
without using the command line.
