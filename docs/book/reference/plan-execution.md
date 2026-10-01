# Shared plans and CPU/Metal execution

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

Source links use the [revision map](source-revisions.md); labels retain the
recorded revision identifiers.

On Apple M4 Pro, the execution changes in `2e80d065` reduce Metal wall medians
by about 35% for queens variant 2 and 65% for task allocation, while SEND takes
9–14% longer. Accounted device storage falls, but process RSS rises on all three
inputs. The matched CPU-eager route remains faster and uses less RSS throughout.
The CPU observations do not establish a broad speed gain or performance neutrality.

The architectural improvement is shared checked-plan ownership and clearer
separation of candidate production from membership execution. These measurements
qualify particular executables and inputs; they do not establish that a shared
plan makes every workload faster on a device.

## Sources and evidence

The acquisitions ran on 20 September 2026, on arm64 macOS 26.6.2, using the order
A1, B1, B2, A2 and fresh clingo 5.8.2 observations in each leg.

| Role | Compiled source | Executable SHA-256 |
| --- | --- | --- |
| A, baseline | [`eca5a1a7`](https://github.com/GregoryGelfond/zetesis/tree/5385635e782eddc9b1b6b9c1509900f1e6ff67be) | `f1b3adbdaa93d6a7f9adff130c1b68adee216d3eb986f16f617348b0ef4a4aeb` |
| B, candidate | [`2e80d065`](https://github.com/GregoryGelfond/zetesis/tree/7d83909daa36602b0d6f5526d28d7e45ace8c3f3) | `14119b506867127da9b088d943ee5d56241c1af4bbb1338c727027fd7e813d00` |

The baseline executable represents `935f2db7`: runtime crates, Cargo manifests,
lockfile and pinned toolchain are unchanged between that revision and
`eca5a1a7`. Relative to the compiled candidate, qualification source
[`6754a4ff`](https://github.com/GregoryGelfond/zetesis/tree/7c24c607841aea20dc254bfd8df56a3c93cf0831)
changes one device-test fixture and two manual pages. The measured candidate
bytes were retained unchanged. This distinguishes the compiled source from the
later test/documentation source; it is not a claim that a binary was rebuilt at
6754 or that source equality establishes transitive build equivalence.

| Acquisition | Maintained JSON view | Complete tables |
| --- | --- | --- |
| CPU, automatic grounding, 22 cells | [JSON](observations/series-2e80d065-cpu-auto.json) | [all 22 cells](https://github.com/GregoryGelfond/zetesis/blob/main/docs/book/reference/observations/series-2e80d065-cpu-auto-tables.md) |
| CPU, eager grounding, 3 cells | [JSON](observations/series-2e80d065-cpu-eager.json) | [all observations](https://github.com/GregoryGelfond/zetesis/blob/main/docs/book/reference/observations/series-2e80d065-cpu-eager-tables.md) |
| Metal, eager grounding, the same 3 cells | [JSON](observations/series-2e80d065-metal-eager.json) | [all observations](https://github.com/GregoryGelfond/zetesis/blob/main/docs/book/reference/observations/series-2e80d065-metal-eager-tables.md) |

The [provenance](observations/series-2e80d065-provenance.json) records executable,
runner, input and raw-report hashes, acquisition order, timestamps and limits.
The [Metal execution receipt](observations/series-2e80d065-metal-device.json)
retains actual routes, adapters, work counters, host phases and memory samples.
The comparison JSON and tables are maintained `zetesis-series` outputs; raw
reports and captured streams are not published here. The
[earlier execution series](execution-series.md) remains a separate comparison.

All 12 reports pass and account for every scheduled position: 704 CPU-auto,
144 CPU-eager and 144 Metal observations, or 992 total. No position is omitted
because of a refusal, timeout or failed report, and the retained before/after
input and executable seals are unchanged. Passing compares complete selected
display multisets, model multiplicities and final optimum ties and cost vectors
with clingo. Hidden clingo interpretations cannot be reconstructed from `#show`.

## What the execution change aligns

Answer-set membership remains defined by the reduct. A completed tight
certificate authenticates the original theory and its producer coverage before
CPU or device execution can use support checking. The enumeration owns one
certificate and preparation receipt; sharing its immutable plan with an external
executor does not activate CPU membership checking.

The scalar region traversal, native CPU workers and bounded Rayon candidate
producers share the same narrowing, region and leaf-construction operations.
Their schedules remain distinct. Native workers check membership and stream
verified models. Device-oriented producers return bounded batches of classical
candidates, join, and leave original-satisfaction validation, membership, retry
and commit to the batch owner. The implementation adds this schedule rather than
replacing the existing workers or introducing another membership definition.

The distinction is described in [execution alignment](../architecture/alignment.md)
and the [session API](../rust/sessions.md). Backend coverage is not identical:
CPU can also use the positive-consequences certificate; there is no corresponding
device implementation in this selection. Theories without a complete tight
certificate, and explicit countermodel requests, retain general device
propagation with exact CPU residual completion. A positive program may itself be
tight, so positivity alone does not select that general route.

## Matched CPU-eager results

Every leg uses four native workers, four requested completion workers, one
clingo worker, batch 64, eager grounding and the automatic oracle. Each
producer/cell/leg has one qualification, one warmup, three timed observations
and one separate RSS observation. All three CPU cells use tight support in both
revisions. The selected families are one SEND answer, 92 queens answers and
1,176 task-allocation optimum ties at cost `[5]`.

Wall ranges below span the two A or B leg medians, in milliseconds. Clingo spans
its four leg medians. These are not confidence intervals or pooled medians.
Native RSS ranges retain the two separate memory observations, in MiB.

| Workload | A wall ms | B wall ms | Clingo ms | A RSS MiB | B RSS MiB | Clingo RSS MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| SEND | 12.835–12.934 | 12.822–12.847 | 12.881–12.948 | 18.47–18.53 | 18.44–18.48 | 8.27–8.30 |
| Queens variant 2, N=8 | 78.767–79.023 | 78.810–80.702 | 122.177–123.830 | 21.98–22.31 | 21.97–22.14 | 8.62–9.92 |
| Task allocation v4, larger mix | 34.704–35.456 | 34.626–35.944 | 183.008–188.330 | 22.64–22.77 | 22.44–23.16 | 21.70–22.67 |

Old/new median ranges overlap on all three cells. Queens and task allocation
retain substantial observed wall advantages over clingo, usually with higher
RSS. SEND's difference from clingo is a fraction of a millisecond. These results
do not identify a systematic CPU improvement from the execution change.

## Metal results and remaining costs

The Metal acquisition uses the same three original corpus inputs and the same
worker, batch, repetition and memory schedule. All timed native records report
actual Apple M4 Pro Metal execution and complete the same selected families.
The old route uses general formula propagation on all three cells; the new
route uses the checked tight-support plan on the device.

| Workload | A wall ms | B wall ms | Clingo ms | A RSS MiB | B RSS MiB | Clingo RSS MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| SEND | 27.968–29.275 | 31.788–31.820 | 11.593–11.616 | 27.06–27.14 | 28.91–29.42 | 8.48 |
| Queens variant 2, N=8 | 154.796–154.805 | 99.590–100.974 | 123.095–124.575 | 28.06–28.64 | 33.45–33.88 | 8.39–8.97 |
| Task allocation v4, larger mix | 275.861–278.458 | 97.290–98.710 | 168.582–174.851 | 30.42 | 33.41–33.59 | 19.61–19.84 |

Paired B1/A1 and B2/A2 wall changes are +13.77%/+8.59% for SEND,
−35.66%/−34.77% for queens and −64.55%/−64.73% for task allocation. The raw
old/new timed ranges do not overlap within these Metal cells. New Metal beats
its corresponding clingo median on queens and task allocation in both legs;
old Metal beats it on none. Matched CPU-eager remains faster and lower-RSS on
every cell. CPU and Metal were acquired in separate windows: task clingo
medians differ by about 10% between them, so their cross-campaign ratios are
not controlled CPU/device crossover estimates.

The host phases explain different parts of these observations:

- Queens candidate-generation medians fall from 126.0–127.2 to 69.3 ms. Its
  host device-oracle phase increases from 8.4–8.5 to 11.3 ms, despite fewer
  charged device operations. The dominant observed gain is candidate production.
- Task allocation eliminates 1,195 CPU residual queries per timed old run.
  Candidate generation falls from 30.2–30.3 to 14.1–16.0 ms and the host
  device-oracle phase from 157.2–160.3 to 39.3–39.6 ms. New execution examines
  1,215–1,222 candidates versus 1,208 previously; changed ordering and batched
  objective feedback still have costs. The final 1,176 optimum ties are unchanged.
- SEND candidate generation falls from 3.2–3.3 to 2.1 ms, but the host
  device-oracle phase rises from 3.9–6.0 to 8.9–9.1 ms. That increase outweighs
  the proposal savings. Execution setup remains approximately 8 ms in both
  revisions, and new certificate setup costs about 0.1 ms across these cells.

New records show zero propagation sweeps, CPU residuals and countermodel
queries. Their completion executor is inactive: one requested worker and zero
effective workers, since the device's complete tight verdict needs no residual
search. Actual decoded batches remain 1 for SEND, 2 for queens and 19–20 for
task allocation. This is a change in the membership primitive, not a device
request satisfied by a CPU fallback.

Accounted device storage falls from 0.334 to 0.257 MiB for SEND, 2.395 to
1.262 MiB for queens and 2.281 to 1.205 MiB for task allocation. Process RSS
nevertheless rises by roughly 2, 5 and 3 MiB respectively. The
[device receipt](observations/series-2e80d065-metal-device.json) keeps these
populations separate. Its propagation and support-scan work counts measure
different primitives; their ratio is not a kernel speedup or instruction ratio.
Old reason-specific residual counts are unavailable and are not replaced by zero.

## The 22-cell CPU-auto comparison

The [full tables](https://github.com/GregoryGelfond/zetesis/blob/main/docs/book/reference/observations/series-2e80d065-cpu-auto-tables.md) retain every
cell, including less favorable observations. Each leg has two timed observations
per producer/cell, no warmup and one RSS observation. All 704 positions pass.
The actual routes are unchanged: ten lazy CPU closure cells, nine eager
tight-support cells, two eager positive-consequence cells and one eager general
countermodel cell. Old medians beat clingo on 9/22 cells in both legs; new
medians do so on 9/22 and 8/22. That small difference is not a generalized win rate.

`chain-1000` merits a focused follow-up: paired wall medians rise 9.89% and
4.71%, while closure work and rounds are unchanged. Output-phase medians rise
from 0.773/1.099 to 1.835/1.624 ms, with two output calls and approximately
216.8 KB of native stdout per timed sample. This localizes an observed cost;
the small, overlapping sample population does not identify a code-caused
regression. Independent-negation-8 also rises 5.84%/7.05%, and planning-14
1.47%/1.58%. Task allocation rises in CPU-auto but changes direction between the
two CPU-eager pairs. These observations remain part of the result, rather than
being discarded as noise or treated as proof of neutrality.

## Measurement and semantic limits

Wall time includes fresh process launch, JSON/statistics output, capture and
reaping. Host device-oracle phases include host/API activity and waits, not
measured kernel duration. Worker-accumulated phases can exceed driver elapsed
time, and independently summarized phase medians must not be added as a disjoint
wall-time breakdown. RSS comes from a separate fresh helper's child accounting;
it excludes that helper, may include usage propagated from waited descendants,
and is not simultaneous process-tree RSS or device memory. Accounted device
bytes describe authored capacities rather than physical VRAM residency.

The shared plan and candidate schedules preserve the original semantic
obligations: original satisfaction, reduct membership, complete family coverage
and honest interruption. Tests, checked certificates and the
[Lean correspondence boundary](../lean/correspondence.md) provide different
forms of evidence; none of these timings proves Rust or shader correctness.
Physical qualification and coverage are recorded separately from performance.
The changes combine plan selection, candidate scheduling and observability, so
this acquisition does not isolate a single optimization. Three Metal inputs,
one host and a short schedule establish neither an automatic crossover policy
nor Vulkan performance.

## Reproducing the protocol

Build the named revisions in separate checkouts with the pinned toolchain and
ordinary release profile. Build the solver and the two maintained measurement
tools explicitly:

```sh
cargo build --locked --release --all-features \
  -p zetesis-cli -p zetesis-experiments -p zetesis-validation \
  --bin zetesis --bin zetesis-perf --bin zetesis-series
```

Retain binary hashes and use one fixed `zetesis-perf` and `zetesis-series` for
both solver revisions. For each profile, run A1, B1, B2, A2 sequentially with
new report paths, retaining every non-pass result. With `SOLVER` naming the
chosen executable, `CLINGO` the reference and `REPORT` a new output path, the
matched CPU-eager leg is:

```sh
zetesis-perf examples/correctness --suite baseline --profile cpu-eager \
  --workers 4 --completion-workers 4 --clingo-workers 1 --batch-size 64 \
  --warmups 1 --repetitions 3 --memory-runs 1 \
  --timeout-seconds 10 --campaign-seconds 90 \
  --sample-bytes 33554432 --native-report-bytes 33554432 \
  --capture-bytes 134217728 --report-bytes 268435456 \
  --zetesis "$SOLVER" --clingo "$CLINGO" --report "$REPORT"
```

For the matched device leg, change only the profile to `metal-eager` and use a
physical Metal adapter. For the 22-cell CPU acquisition, use `--suite series
--profile cpu-auto --warmups 0 --repetitions 2 --campaign-seconds 180`.
Also omit the four explicit sample, native-report, capture and report byte
ceilings above, retaining the recorded runner defaults for that acquisition.
Avoid concurrent builds and measurements. After each four-leg acquisition:

```sh
zetesis-series --report A1-main=A1-main.json \
  --report B1-candidate=B1-candidate.json \
  --report B2-candidate=B2-candidate.json \
  --report A2-main=A2-main.json \
  --json comparison.json > comparison.md
```

The viewer checks matching ordered workloads and profiles. Repeating this
protocol creates a new observation; it does not promise identical timing or
recreate the recorded executable bytes.
