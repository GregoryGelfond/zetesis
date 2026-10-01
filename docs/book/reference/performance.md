# Benchmarks and comparisons

Use a **solver benchmark** to compare the time and memory needed to answer a
program. The [README overview](https://github.com/GregoryGelfond/zetesis/blob/main/README.md#performance-at-a-glance)
summarizes the latest complete CPU corpus comparison. The studies below retain
their own source revisions, workloads and measurement conditions.

## Recorded comparisons

The [grounding and prepared-closure comparison](instantiation-lazy.md) measures
all three maintained CPU selections at four and fourteen workers. The arithmetic
chain takes about 15% less process time and independent negation about 7% less;
the ordinary 2,000-step chain regresses about 4%. Corpus sums are nearly unchanged,
and some faster queens cells use more memory. Complete tables retain every
workload and the combined source and output changes being compared.

The [0.1.5 CPU comparison](canonical-storage.md) includes every case in the
94-program corpus, the 22-case execution series and the ten-case scalability
selection, with separate measurements at 1, 2, 4, 8 and 14 workers. These
selections overlap. The measured executable admits Einstein's Riddle within its
default limits and takes about 20.4 ms against clingo's 35.4–36.1 ms. It also
regresses on many other cases: the corpus retains four timing wins over clingo,
while the series falls from nine to four. Full timing, memory, refusal and
reproduction details accompany the results; this is not an overall speedup.

The earlier 94-case CPU and Metal comparison runs both solvers on the same
programs on an Apple M4 Pro. Four CPU threads gave zetesis its lowest total time. Most small
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

The [nine-workload CPU scheduler comparison](scheduler-scaling.md) measures
1, 2, 4, 8 and 14 workers in two opposite-order blocks. Larger queens and
task-allocation cases improve at high worker counts; several smaller cases
regress at four workers. The page retains both outcomes, individual timing/RSS
observations and the shared-host conditions of the measurement.

## Run a benchmark

From a repository checkout, with zetesis installed, and clingo to compare with:

```sh
zetesis-bench run --suite baseline --report baseline.json
```

This small suite runs SEND, queens variant 2 and task allocation. The terminal
shows a table; the report retains settings, results and measurements. The runner
checks answers before comparing timings and records incomplete or failed runs.
It refuses to overwrite an existing report. Without clingo, it times zetesis
alone and checks its answers against the ones each workload records.

Omit `--suite baseline` to run the full corpus. Select CPU threads or a backend
explicitly when comparing configurations:

```sh
zetesis-bench run --threads 2 --report two-threads.json
zetesis-bench run --backend metal --grounder eager --report metal.json
zetesis-bench run --json --report run.json > summary.json
```

Benchmarks always collect their required statistics. `solve --stats` is useful
for investigating one run, but a single duration is not a repeatable comparison.
See the [benchmark command options](benchmarking.md#run-the-suite) for limits,
repetition counts and other suites. Use `zetesis test corpus` when you want to
check answers without conducting a benchmark.

The maintained scalability selection is shared by testing and benchmarking:

```sh
zetesis test scalability --threads 1,2,4,8,14 --report scalability-check.json
zetesis-bench run --suite scalability --grounder eager \
  --compare-threads 1,2,4,8,14 --repetitions 4 --memory-runs 2 \
  --timeout-seconds 30 --campaign-seconds 1800 --report scalability-timing.json
```

Both use authored queens at n=8/9/10, pigeonhole at h=5/6/7 and three unchanged
corpus cases: queens variant 02, SEND+MORE=MONEY and task allocation.
`--include-einstein` adds the unchanged riddle. The test runs complete-family
qualifications only. The benchmark qualifies clingo once per case, then measures
each native thread profile separately. Reports retain all refusals and limits;
the workload's inclusion is not a scaling claim. `--max-expansion-work` can set
an explicit common grounding ceiling when needed, and its value remains part of
the profile identity. See the [command guide](commands.md#check-conformance) for
source roots, process bounds and the required new evidence destination.

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
[comparison command](benchmarking.md#compare-reports) reads saved reports
without rerunning the solver.

## Detailed measurements

Each report below describes a particular experiment. Earlier results retain
their original programs and executables; they are not a cumulative speedup chart.

| Question | Report |
| --- | --- |
| What changes with grounding reuse and prepared CPU closure? | [Grounding and prepared CPU closure](instantiation-lazy.md) |
| What changes when exclusive support-publication directories are reused? | [Support-publication CPU comparison](support-publication.md) |
| What changed with canonical storage and answer construction? | [0.1.5 CPU comparison and worker scaling](canonical-storage.md) |
| What changes when source constraints are checked during solving? | [Eager and hybrid grounding](hybrid-grounding.md) |
| What does lending completed eager join rows change? | [Grounding row lending](grounding-row-lending.md) |
| How do thread count, CPU and Metal compare with clingo? | [Worker scaling](worker-scaling.md) |
| What changed when CPU and GPU used the same semantic plans? | [Shared-plan execution](plan-execution.md) |
| How did candidate generation and reduct execution change? | [Execution series](execution-series.md), [reduct execution](reduct-execution.md) |
| What did grounding reuse change on CPU and Metal? | [Prepared grounding](prepared-metal.md) |
| What did atom catalogs, table joins and release optimization change? | [Grounding and representation measurements](grounding-measurements.md) |

For library callers, the [measurement guide](../rust/measurements.md) explains
how to observe work without using the command line.

The records below keep the command spellings of the binaries they identify.
Those made before benchmarking took its present form spell it as it was then:
`zetesis bench corpus`, `zetesis-bench corpus` and the instrumented matrix of
`zetesis-perf` and `zetesis-bench perf` are now `zetesis-bench run`, and their
ordinary uninstrumented campaign has no command; `zetesis bench compare`,
`zetesis-series` and `zetesis-bench series` are now `zetesis-bench compare`,
whose `--markdown` prints what `series` printed and whose reports, once given
as `--report LABEL=PATH`, are operands.
