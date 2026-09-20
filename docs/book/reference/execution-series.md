# CPU and Metal execution series

On Apple M4 Pro, source `eca5a1a7` has lower CPU wall medians than `994fbb79`
on 19 of 20 workloads completed by both revisions. Process memory has tradeoffs.
Metal SEND and task allocation also improve, while queens variant 2 takes
39–44% longer and uses about 15% more process memory. Its dominant measured
increase is in host candidate generation. These observations qualify the
particular executables and inputs below, not a general speedup.

## Sources and retained evidence

Both acquisitions ran on 20 September 2026, on arm64 macOS 26.6.2. Each uses
the order A1, B1, B2, A2: two campaigns per executable, with fresh clingo 5.8.2
observations beside each. CPU acquisition ran from 03:29:49 to 03:36:59 UTC;
Metal ran from 12:42:40 to 12:44:11 UTC.

| Role | Compiled source | Executable SHA-256 |
| --- | --- | --- |
| A, baseline | [`994fbb79`](https://github.com/GregoryGelfond/zetesis/tree/994fbb79f9a9e0a4398293f094fa2fbe0c3fbc17) | `17f7ed2636fb715ccd887fad8ae0a5647cef88adb27ebcff5f217e6717fc9859` |
| B, current | [`eca5a1a7`](https://github.com/GregoryGelfond/zetesis/tree/eca5a1a7b35cfe5219c2f7c1dcb98c37d13a89d3) | `f1b3adbdaa93d6a7f9adff130c1b68adee216d3eb986f16f617348b0ef4a4aeb` |

The baseline's compiled source differs from `896a5f73` only in three Markdown
files. Both binaries use Rust 1.97.1 and ordinary release optimization with GPU
support compiled in. The current runner measures both. This compares complete
implementations, including arithmetic validation, search, accounting and output
changes; it does not isolate any one change or establish transitive build
equivalence. The [source arithmetic contract](language.md#numeric-boundaries-and-refusal-meaning)
and [qualification snapshot](validation.md#coverage) describe the current
semantic and assurance boundaries separately.

| Acquisition | Exact maintained view | Rendered tables |
| --- | --- | --- |
| CPU, automatic grounding, 22 cells | [JSON](observations/series-eca5a1a7-cpu-auto.json) | [all observations](observations/series-eca5a1a7-cpu-auto-tables.md) |
| Metal, eager grounding, 3 cells | [JSON](observations/series-eca5a1a7-metal-eager.json) | [all observations](observations/series-eca5a1a7-metal-eager-tables.md) |

The [provenance](observations/series-eca5a1a7-provenance.json) retains source,
binary, runner and raw-report hashes, acquisition order, workload identities,
protocols, limits and timestamps. The JSON and tables are unchanged outputs of
`zetesis-series`; they preserve integer timing summaries, counters and non-pass
decisions. Raw reports and captured streams are not published here. These views
are separate from the four historical `release_observations` datasets.

## CPU results

Each leg uses four native workers, four completion workers, one clingo worker,
batch size 64, automatic grounding and the default search. Per cell and producer
there is one qualification, two timed rounds and one separate RSS round, with
no warmup. Thus each executable has four timed and two RSS observations per
successful cell. There is no cold-cache claim.

Both current legs pass all 176 scheduled positions. The baseline times out on
`stratified-16` and reaches the capture limit on `planning-14`; neither outcome
is treated as a successful duration. On the 20 commonly completed cells, both
current leg medians are below both baseline medians on 19. Current wall medians
beat the corresponding clingo medians on 8 of 22 cells in each leg. This selected
suite does not establish general superiority.

Native ranges below span the two leg medians in milliseconds, not confidence
intervals or pooled medians. Clingo ranges span its four leg medians. RSS ranges
span the two separate native observations, in MiB.

| Workload | A wall ms | B wall ms | Clingo ms | A RSS MiB | B RSS MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| SEND | 37.26–38.02 | 12.408–12.412 | 12.197–12.232 | 24.69–24.73 | 18.05–18.06 |
| Task allocation v4, larger mix | 360.53–361.47 | 34.96–35.28 | 177.96–192.85 | 16.73–16.83 | 22.33–22.50 |
| Queens variant 1, N=10 | 100.28–100.99 | 30.86–31.34 | 28.08–28.86 | 13.20–13.25 | 16.77–16.78 |
| Queens variant 1, N=11 | 395.74–400.55 | 104.95–117.71 | 180.20–184.73 | 15.89–15.94 | 17.94–18.41 |
| Queens variant 4, N=11 | 267.83–270.44 | 95.17–96.82 | 174.97–180.28 | 15.73–15.78 | 16.44–16.59 |
| Transitive path, 200 | 121.65–124.01 | 16.09–16.18 | 19.23–19.99 | 20.69–20.77 | 18.69–18.70 |
| Arithmetic chain, 1,000 | 9.419–9.426 | 10.089–10.115 | 4.491–4.683 | 13.27–13.31 | 13.56–13.70 |

SEND uses about 27% less RSS; task allocation uses about 33–34% more, and queens
variant 1 at N=10 about 27% more. Arithmetic-chain wall medians rise about 7%,
while its short sample ranges overlap and driver medians fall from 4.98–5.08 to
4.64–4.65 ms. Four timed samples do not identify the cause of that wall increase
or establish that it is noise. The two current queens N=11 leg medians differ
by about 11%; the table retains that variation.

SEND's reported grounding falls from 18.34–18.82 to 2.44–2.46 ms, while its
output phase stays near 0.02 ms. Task allocation's output phase falls from
about 259 to 10.0–10.3 ms. Computation and output both contribute to the observed
changes, in different proportions across workloads.

## Metal results and the queens regression

Each leg uses eager grounding with the same worker and batch counts, one
qualification, one warmup, three timed rounds and one separate RSS round per
cell and producer. All 144 scheduled positions pass. Native executions perform
actual Metal work on Apple M4 Pro and complete the selected families: one SEND
answer, 92 queens answers and 1,176 task-allocation optimum ties at cost `[5]`.

Native times below are the A1/A2 and B1/B2 leg medians; clingo ranges span its
four leg medians. RSS retains both native observations.

| Workload | A wall ms | B wall ms | Clingo ms | A RSS MiB | B RSS MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| SEND | 65.708 / 68.262 | 26.703 / 29.230 | 11.597–11.622 | 36.42–36.48 | 27.02–27.08 |
| Queens variant 2, N=8 | 104.666 / 108.402 | 150.958 / 151.001 | 118.223–120.736 | 24.41 | 28.11–28.13 |
| Task allocation v4, larger mix | 599.765 / 606.040 | 275.666 / 266.884 | 158.313–177.337 | 32.00–32.02 | 30.33–30.36 |

Queens variant 2 is slower in both paired comparisons, by 44.23% and 39.30%.
Host candidate-generation medians rise from 71.36–72.44 to 123.50–123.77 ms,
despite the same 185 calls and 92 candidates. Every timed sample retains two
GPU batches, 368 GPU sweeps, 16,004,412 GPU work units, zero CPU residuals and
2,511,856 accounted device bytes. The [device receipt](observations/series-eca5a1a7-metal-device.json)
retains these timed-sample counter ranges and actual adapter identities alongside
the raw-report hashes. This localizes the dominant measured regression
to host candidate generation; it does not identify an allocation, instruction
or synchronization cause. Kernel duration was not measured. The CPU series
contains different queens variants and cannot supply a CPU comparison for this
input.

Task allocation's improvement includes an output-phase reduction from
217.74–217.94 to 8.33–8.74 ms. Its GPU work rises slightly, from about 3.024 to
3.030 billion units, with the same 19 batches and 1,208 candidates. Its wall-time
improvement is therefore not a GPU-work reduction. SEND reduces both reported
grounding and GPU work. The current Metal executable remains slower than fresh
clingo on all three cells. No automatic CPU-to-GPU crossover or Vulkan
performance claim follows from these acquisitions.

## Interpretation and reproduction

Wall samples include process launch, JSON/statistics output, capture and child
reaping. RSS comes from separate fresh helper processes reporting child peak
resident memory; it is not simultaneous process-tree memory or device memory.
Accounted device bytes are logical retained capacities, not measured GPU RSS.
Parallel proposal and membership durations sum worker intervals and may exceed
driver elapsed time. Phase medians are independently summarized and must not be
added into a disjoint wall-time decomposition. Work-charge contracts also change
between revisions, so raw work counts are not interchangeable operation counts.

The maintained runner checks complete reported-display multisets, multiplicity,
optimum ties and full cost vectors against the reference. Hidden interpretations
cannot be reconstructed from `#show`; full-model semantic tests remain a
separate assurance layer. Short runs, one host and selected inputs limit the
performance conclusion. The 56 physical regression tests and line-coverage
population are separate from these timing acquisitions.

Build the named sources separately using the pinned toolchain and ordinary
release profile. For the current source, the measured build selected:

```sh
cargo build --locked --release --all-features \
  -p zetesis-cli -p zetesis-experiments -p zetesis-validation --bins
```

Retain executable hashes and use the current `zetesis-perf` and `zetesis-series`
for both solver revisions. Run A1, B1, B2, A2 sequentially, preserving every
non-pass result and using a new report path each time. With `SOLVER` naming the
chosen executable, `CLINGO` the reference executable and `REPORT` a new path,
the CPU command for each leg is:

```sh
zetesis-perf examples/kr-domains --suite series --profile cpu-auto \
  --workers 4 --completion-workers 4 --clingo-workers 1 --batch-size 64 \
  --warmups 0 --repetitions 2 --memory-runs 1 \
  --timeout-seconds 10 --campaign-seconds 180 \
  --zetesis "$SOLVER" --clingo "$CLINGO" --report "$REPORT"
```

For Metal, use `--suite baseline --profile metal-eager`, `--warmups 1`,
`--repetitions 3`, `--campaign-seconds 90`, and add
`--sample-bytes 33554432 --native-report-bytes 33554432
--capture-bytes 134217728 --report-bytes 268435456`. Other arguments remain as
above. Use an available physical Metal adapter and avoid concurrent builds or
measurements. After each four-leg acquisition:

```sh
zetesis-series --report A1-main=A1-main.json \
  --report B1-candidate=B1-candidate.json \
  --report B2-candidate=B2-candidate.json \
  --report A2-main=A2-main.json \
  --json comparison.json > comparison.md
```

The viewer verifies matching ordered workload identities and profiles. Repeating
the protocol produces a new observation; it does not reproduce identical timing.
