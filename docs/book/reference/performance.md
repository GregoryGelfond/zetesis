# Execution performance

These measurements compare two release builds on an Apple M4 Pro running
macOS 26.6.2, on 12 September 2026. They cover complete enumeration or, for optimized
programs, all optimum ties. Earlier measurements remain in the
[validation reference](validation.md#performance-evidence).

The newer build improves ordinary CPU time on SEND, task allocation and
queens02, and lowers measured peak RSS on all nine selected examples. The
Metal task-allocation path also improves substantially. These results concern
specific programs and execution routes; they do not establish a general solver
ranking.

## Versions and inputs

Both builds use Rust 1.97.1 release settings. “Previous” means source
[`39b79489`](https://github.com/GregoryGelfond/zetesis/tree/39b79489f23998a3faded2c459cea67c162c062b);
“current” means source
[`15e0f77b`](https://github.com/GregoryGelfond/zetesis/tree/15e0f77b2c7b7a1ab0608857265cebf33ec747b7).
The same previous-build `zetesis-perf` executable acquires both versions.
Clingo 5.8.2 supplies the independent reference.

| Artifact | SHA-256 |
| --- | --- |
| Previous zetesis | `4ca0e226c28a918e7e42977c1541668e7a721beaa2a1e326b1625a0d8e354394` |
| Current zetesis | `0cc8194e7687c472eb57b09aa8c8ca7f73d0866ddea4e1a55d17b40f876c196f` |
| Fixed zetesis-perf | `2fd427ec77ec91faa038fbeddf10e686f2f539cc6c2222c16635a8084a637469` |
| clingo 5.8.2 | `31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015` |
| [Corpus manifest](../../../examples/kr-domains/manifest.json) | `b43df1adf17ae0c035f1e310a5c15345c26cbcad8b59596932627c46fd1c6958` |

All six queens encodings use **N=8**. “Task allocation” is
[variant04/scenario05](../../../examples/kr-domains/scenarios/task-allocation/variant-04/05-larger-mix.lp);
“shortest path” is
[variant01/scenario06](../../../examples/kr-domains/scenarios/shortest-path/variant-01/06-layered-dag.lp).
The manifest supplies their companion encodings and records the source bytes.

Each repeated comparison uses previous/current/current/previous blocks. Tables
show the range of block medians: two blocks per zetesis version and four for
clingo. These ranges describe the observations, not confidence intervals.
Process startup and output are included; no cold-cache condition is claimed.

## Ordinary CPU time and memory

Ordinary runs request eager grounding, CPU, `--oracle auto`, one closure worker
and one completion worker. Each block includes one qualification pair, one
warmup pair, five timed pairs, two separate memory pairs and one separate native
statistics run per case. All four blocks pass, with 684 observations total.
Timed zetesis output is human-readable without statistics; clingo produces JSON.

| Case | Previous, ms | Current, ms | clingo, ms |
| --- | ---: | ---: | ---: |
| Queens 1 | 14.066–14.105 | 14.032–14.065 | 5.195–6.515 |
| Queens 2 | 99.307–100.672 | 89.498–90.552 | 121.406–123.025 |
| Queens 3 | 14.117–14.149 | 14.090–15.243 | 6.493–6.539 |
| Queens 4 | 7.790–7.809 | 7.807–7.808 | 6.446–6.459 |
| Queens 5 | 10.291–10.293 | 10.209–10.295 | 6.316–6.463 |
| Queens 6 | 11.544–11.552 | 10.278–10.279 | 6.452–6.499 |
| SEND + MORE = MONEY | 40.383–40.428 | 31.669–32.805 | 12.630–12.830 |
| Task allocation | 122.104–129.812 | 102.159–103.329 | 181.214–191.037 |
| Shortest path | 7.910–7.914 | 6.617–6.653 | 6.447–6.522 |

Zetesis is faster than clingo on queens02 and this task-allocation case under
this output contract. Clingo is faster on the other selected examples. Small
changes in the shortest cases are not evidence of a general speed improvement.

Memory rounds use a fresh helper to measure the solver child's peak resident
set size, excluding the helper. They are separate from the timed population.
MiB means 1,048,576 bytes; these observations are neither simultaneous
process-tree memory nor GPU allocations.

| Case | Previous, MiB | Current, MiB | clingo, MiB |
| --- | ---: | ---: | ---: |
| Queens 1 | 12.211–12.242 | 11.961 | 5.422 |
| Queens 2 | 13.008–13.156 | 12.508–12.555 | 8.344–9.102 |
| Queens 3 | 12.383–12.398 | 12.102–12.133 | 5.438–5.508 |
| Queens 4 | 12.234–12.242 | 12.102–12.148 | 5.469 |
| Queens 5 | 12.945–13.031 | 12.633–12.641 | 5.531 |
| Queens 6 | 13.180–13.273 | 12.688–12.711 | 5.563 |
| SEND + MORE = MONEY | 26.148–26.305 | 23.992–24.055 | 8.281–8.555 |
| Task allocation | 40.367–40.383 | 35.727–35.781 | 20.266–20.445 |
| Shortest path | 13.031–13.047 | 12.852–12.984 | 5.641 |

Peak RSS block medians decrease on all nine cases: roughly 4.6 MiB for task
allocation and 2.1–2.3 MiB for SEND. Clingo remains smaller on each case. These
measurements describe the combined implementation changes, rather than isolating
one representation's contribution.

## Instrumented CPU and Metal comparison

The instrumented matrix requests four closure/completion workers, batch size
64, `--oracle auto`, and explicit eager and lazy profiles. Each block has one
qualification, one warmup and three timed repetitions per case. Native output
contains full JSON model records and statistics; clingo emits its JSON results.
The CPU and Metal campaigns ran in separate windows.

The following eager wall times are in milliseconds. Queens rows use the
`queens` suite; SEND and task allocation use `baseline`. That suite also repeats
queens02, whose observations are retained separately rather than pooled here.
Shortest path was measured only in the ordinary repeated campaign and the
single-sample corpus survey.

| Case | Previous Metal | Current Metal | Current CPU, JSON/stats | clingo in Metal campaign |
| --- | ---: | ---: | ---: | ---: |
| Queens 1 | 36.899–38.009 | 36.875–38.032 | 17.890–18.238 | 6.323–6.537 |
| Queens 2 | 127.431–131.005 | 117.227–122.225 | 95.403–99.288 | 123.260–129.625 |
| Queens 3 | 36.830–39.288 | 38.098–39.272 | 18.474–19.060 | 6.538–6.549 |
| Queens 4 | 28.035–30.514 | 27.899–29.243 | 10.366–10.935 | 6.283–6.553 |
| Queens 5 | 69.756–69.882 | 68.437–68.604 | 47.145–54.547 | 6.523–6.580 |
| Queens 6 | 72.292–73.512 | 70.889–71.004 | 46.920–55.906 | 6.431–6.618 |
| SEND + MORE = MONEY | 84.721–86.976 | 79.470–85.706 | 33.654–34.274 | 11.601–12.874 |
| Task allocation | 1343.650–1880.153 | 616.789–627.071 | 331.736–374.177 | 185.970–206.472 |
| Shortest path | — | — | — | — |

Task allocation falls from 1.344–1.880 seconds to 0.617–0.627 seconds. The old
blocks vary substantially, so a single speedup factor would conceal useful
information. Queens02 also improves; its current Metal medians are below the
corresponding clingo medians. The matched CPU route remains faster than Metal
on every measured matrix case.

Execution records explain the comparison. Every admitted Metal run uses the
Apple M4 Pro device and the general formula countermodel procedure, with
positive submitted batch and work counts. Repeated CPU matrix cases use the
certified tight-support procedure and have no residual queries. Thus the
backend comparison also includes a difference in the selected membership
procedure.

- SEND sends one candidate in one device batch. Each queens case sends 92
  candidates in two batches. Metal decides all of these candidates; no CPU
  residual queries run despite four requested workers.
- Task allocation sends 1,208 candidates in 19 batches. Current timed runs
  decide 13 on the device and complete 1,195 exactly on CPU, with four admitted
  completion workers. All candidates finish without a pending or
  failed check. This is hybrid execution, not a kernel-only measurement.
- Task allocation publishes all 1,176 optimum ties at cost 5. Its current
  observation/output stage takes about 208 ms; queens05/06 spend about 35 ms
  there. The larger native output helps explain why these matrix timings differ
  from the ordinary human-output table.

### Where time changes

The complete exclusive driver-stage records separate grounding, solving and
observation/output. For Metal task allocation, solving falls from
1,117.941–1,643.204 to 390.496–399.054 ms. Its host GPU-call interval falls from
342.456–356.625 to 199.602–208.427 ms, including packing, submission, waits and
readback. These captures do not isolate kernel time or time every CPU residual
phase, so they cannot assign the improvement to a single primitive.

SEND's Metal grounding decreases from 18.739–19.928 to 15.463–15.464 ms, although
whole-process Metal ranges overlap. Queens01–03 grounding is higher in the new
build. In the repeated CPU matrix, queens03 wall time increases from
16.692–17.931 to 18.474–19.060 ms; its grounding rises while solving falls.
This remains an observed regression to investigate. Task allocation's CPU JSON
matrix also has variable output cost and is not uniformly faster.

Exclusive stages partition the driver when their records are complete. Source
loading, final statistics and the JSON envelope lie outside that partition but
remain in process wall time. Finer phase intervals may overlap and should not
be summed as another wall-time partition. Matrix mode does not measure RSS.

## Result agreement and applicability

The complete selected displays, symbol/model multiplicities, optimum ties and
costs agree with clingo for every admitted comparison. This protocol cannot
observe clingo's hidden interpretations. Separately, canonical full native
model records and costs agree between versions for all 94 corpus cases and
across the 180 admitted Metal observations. This comparison sorts signed,
typed atom records and model records, preserves multiplicity and compares
SHA-256 fingerprints. Its scope is all answer sets for nonoptimized cases and
all published optimum ties for optimized cases, under the hash-collision
assumption; it does not compare unpublished nonoptimal interpretations.

The eager CPU corpus survey admits all 94 cases. Its one timed observation per
case is exploratory and is not used to claim corpus-wide speedups. The repeated
Metal matrices cover eight distinct inputs, not all 94.

Both versions refuse the explicit lazy profile for these matrix inputs with
`unsupported_oracle`: automatic admission reaches the formula route, which
requires eager execution here. Each eight-report CPU or Metal campaign accounts
for 540 scheduled positions: 360 passing executions, 36 refusals and 144 later
positions blocked by those refusals. There are no capture faults or unresolved
children. These are retained applicability outcomes, not lazy timing samples
and not a claim that supported relational lazy execution is unavailable.

## Reproduce the comparisons

Use the [checkout installation instructions](https://github.com/GregoryGelfond/zetesis/blob/main/README.md#install-and-run)
to prepare the solver and maintained `zetesis-perf` command. Keep each release
binary separate and use one fixed measurement-tool build for both. Rebuilding
from the named revisions reproduces source selection; executable hashes may
differ with the build environment. Record the hashes of the binaries you run.

Run from the checkout root with clingo 5.8.2 available. Assign absolute paths to
your retained executables, then create a new results directory:

```sh
perf_command=/absolute/path/to/zetesis-perf
previous_solver=/absolute/path/to/previous/zetesis
current_solver=/absolute/path/to/current/zetesis
clingo_command=/absolute/path/to/clingo
results_dir=$(mktemp -d "${TMPDIR:-/tmp}/zetesis-perf.XXXXXX")
```

The ordinary command below reproduces the nine-case timing and RSS schedule.
The function arguments name the block and select its solver. Each call saves
its real command exit status beside the report and continues to the next block.
Reports are new files; the tool refuses to overwrite existing evidence.

```sh
ordinary() {
    if "$perf_command" examples/kr-domains \
        --zetesis "$2" --clingo "$clingo_command" \
        --report "$results_dir/ordinary-$1.json" \
        --case standalone/n-queens/variant-01.lp \
        --case standalone/n-queens/variant-02.lp \
        --case standalone/n-queens/variant-03.lp \
        --case standalone/n-queens/variant-04.lp \
        --case standalone/n-queens/variant-05.lp \
        --case standalone/n-queens/variant-06.lp \
        --case standalone/send-money/send-money.lp \
        --case scenarios/task-allocation/variant-04/05-larger-mix.lp \
        --case scenarios/shortest-path/variant-01/06-layered-dag.lp \
        --warmups 1 --repetitions 5 --memory-runs 2 \
        --timeout-seconds 10 --campaign-seconds 90 \
        --sample-bytes 4194304 --capture-bytes 134217728 \
        --report-bytes 536870912; then
        comparison_exit=0
    else
        comparison_exit=$?
    fi
    printf '%s\n' "$comparison_exit" > "$results_dir/ordinary-$1.exit"
}
ordinary previous-1 "$previous_solver"
ordinary current-1 "$current_solver"
ordinary current-2 "$current_solver"
ordinary previous-2 "$previous_solver"
```

Run the instrumented CPU and Metal campaigns separately. Metal requires an
accessible physical adapter; ordinary installation needs no qualification
script. Use `comparison_backend=cpu` first, then `comparison_backend=metal` and
repeat the eight calls after the function definition.

```sh
comparison_backend=cpu
matrix() {
    if "$perf_command" examples/kr-domains --suite "$1" \
        --zetesis "$3" --clingo "$clingo_command" \
        --report "$results_dir/$comparison_backend-$1-$2.json" \
        --profile "${comparison_backend}-eager" \
        --profile "${comparison_backend}-lazy" \
        --workers 4 --completion-workers 4 --clingo-workers 1 --batch-size 64 \
        --warmups 1 --repetitions 3 \
        --timeout-seconds 10 --campaign-seconds 60 \
        --sample-bytes 33554432 --native-report-bytes 33554432 \
        --capture-bytes 536870912 --report-bytes 1073741824; then
        comparison_exit=0
    else
        comparison_exit=$?
    fi
    printf '%s\n' "$comparison_exit" \
        > "$results_dir/$comparison_backend-$1-$2.exit"
}
matrix baseline previous-1 "$previous_solver"
matrix queens previous-1 "$previous_solver"
matrix baseline current-1 "$current_solver"
matrix queens current-1 "$current_solver"
matrix baseline current-2 "$current_solver"
matrix queens current-2 "$current_solver"
matrix baseline previous-2 "$previous_solver"
matrix queens previous-2 "$previous_solver"
```

For the exploratory full-corpus CPU survey, run the following once with each
solver, previous then current, choosing a distinct report name each time:

```sh
"$perf_command" examples/kr-domains --suite corpus \
    --zetesis "$current_solver" --clingo "$clingo_command" \
    --report "$results_dir/corpus-current.json" \
    --profile cpu-eager --profile cpu-lazy \
    --workers 4 --completion-workers 4 --clingo-workers 1 --batch-size 64 \
    --warmups 0 --repetitions 1 \
    --timeout-seconds 5 --campaign-seconds 180 \
    --sample-bytes 33554432 --native-report-bytes 33554432 \
    --capture-bytes 536870912 --report-bytes 1073741824
```

Exit 0 means the report passes; exit 1 means it contains nonpassing evidence,
which may include refusals or operational failures. Inspect `accounted`, sample
dispositions, faults and unresolved children before drawing a conclusion.
Setup/publication failures exit 2. Child and campaign time limits are polling
budgets; setup, cleanup and report publication have their own scope. A refused,
interrupted or uncaptured cell is not a completed solve time.

The source corpus and measurement tools are in the repository. The original
raw captures for these tables are retained privately and are not currently
shipped with the manual. The commands above generate new self-contained JSON
reports containing the captured output, identities, options and result checks;
no public download of the original reports is claimed.
