# Execution performance

These measurements compare two release builds on an Apple M4 Pro running
macOS 26.6.2, on 12 September 2026. They cover complete enumeration or, for
optimized programs, all published optimum ties. The
[preceding comparison](https://github.com/GregoryGelfond/zetesis/blob/8f22c257/docs/book/reference/performance.md)
and earlier [validation measurements](validation.md#performance-evidence)
remain separate historical populations.

Current ordinary CPU time improves on task allocation and Queens 6. Task
allocation's measured peak RSS falls by 45.7%. Current Metal times improve on
all eight distinct inputs in the repeated matrix, although the selected CPU
route remains faster on those inputs. Other ordinary changes are mixed;
these observations do not establish a general solver ranking.

The optional finite-table experiment is reported separately. It is not selected
by ordinary grounding and cannot account for the application improvements.

## Versions and inputs

Both builds use Rust 1.97.1 release settings. “Previous” means source
[`15e0f77b`](https://github.com/GregoryGelfond/zetesis/tree/15e0f77b2c7b7a1ab0608857265cebf33ec747b7);
“current” means source
[`6bebb980`](https://github.com/GregoryGelfond/zetesis/tree/6bebb980f9c102dbb7f943076d7cde92374841ce).
One preserved `zetesis-perf` build acquires both versions.
Clingo 5.8.2 supplies the independent reference.

| Artifact | SHA-256 |
| --- | --- |
| Previous zetesis | `0cc8194e7687c472eb57b09aa8c8ca7f73d0866ddea4e1a55d17b40f876c196f` |
| Current zetesis | `35b96c837dd5027853c735044e092f9054d63fc516940ec83d10627ecc2cf5d8` |
| Fixed zetesis-perf | `2fd427ec77ec91faa038fbeddf10e686f2f539cc6c2222c16635a8084a637469` |
| Current zetesis-bench | `b471d6d4812a68011ef14b3986be307c4ab6a092da0a077bf29d0efb981d6fba` |
| clingo 5.8.2 | `31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015` |
| [Corpus manifest](../../../examples/kr-domains/manifest.json) | `b43df1adf17ae0c035f1e310a5c15345c26cbcad8b59596932627c46fd1c6958` |

All six queens encodings use **N=8**. “Task allocation” is
[variant04/scenario05](../../../examples/kr-domains/scenarios/task-allocation/variant-04/05-larger-mix.lp);
“shortest path” is
[variant01/scenario06](../../../examples/kr-domains/scenarios/shortest-path/variant-01/06-layered-dag.lp).
The manifest supplies their companion encodings and records their source bytes.

Repeated comparisons use previous/current/current/previous blocks. Tables show
**pooled medians [minimum, maximum]**, not confidence intervals. Process startup
and output are included; no cold-cache condition is claimed. Timed samples,
memory samples and diagnostic runs remain separate.

## Ordinary CPU time and memory

Ordinary runs request eager grounding, CPU, `--oracle auto`, one closure worker
and one completion worker. Each block includes qualification and warmup pairs,
five timed pairs, two separate memory pairs and one separate native statistics
run per case. All four reports pass, with 684 observations total. Each zetesis
version has ten timed samples per case; clingo has 20.

Timed zetesis output is human-readable without statistics; clingo produces JSON.
Values are milliseconds.

| Case | Previous, ms | Current, ms | clingo, ms |
| --- | ---: | ---: | ---: |
| Queens 1 | 13.868 [13.651, 15.416] | 13.864 [13.830, 15.355] | 6.245 [6.017, 6.433] |
| Queens 2 | 90.997 [89.178, 92.434] | 92.183 [90.637, 93.717] | 123.686 [120.668, 126.430] |
| Queens 3 | 15.379 [15.143, 15.499] | 15.408 [15.147, 15.540] | 6.276 [5.939, 6.353] |
| Queens 4 | 7.843 [7.782, 7.975] | 7.829 [7.591, 7.986] | 6.216 [5.731, 6.304] |
| Queens 5 | 9.805 [9.258, 10.863] | 9.326 [9.195, 9.380] | 6.209 [5.758, 6.258] |
| Queens 6 | 10.814 [10.776, 10.979] | 9.323 [9.257, 10.839] | 6.200 [5.738, 6.264] |
| SEND + MORE = MONEY | 33.344 [31.630, 33.534] | 33.466 [33.315, 34.714] | 12.321 [12.153, 13.901] |
| Task allocation | 104.352 [102.641, 106.031] | 98.209 [96.964, 100.069] | 187.152 [182.488, 197.416] |
| Shortest path | 7.943 [7.814, 7.970] | 7.915 [7.867, 8.112] | 6.291 [6.207, 6.357] |

Task allocation improves 5.9%, with both current block medians below both
previous blocks. Queens 6 improves 13.8%, also consistent across the blocks.
Queens 5's pooled decrease is qualified by previous-build drift: its block
medians move from 9.293 to 10.318 ms, while current medians remain near 9.33 ms.
Queens 2 increases 1.3% with overlapping ranges; SEND increases 0.4%. Small
changes on the remaining cases do not establish reliable speedups.

Under this output contract, current zetesis is faster than clingo on Queens 2
and task allocation. Clingo is faster on the other seven cases.

Memory rounds use a fresh helper to measure the solver child's peak resident
set size, excluding the helper. Waited descendants may contribute to the
reported child usage. Each zetesis version has four samples per case; clingo
has eight. MiB means 1,048,576 bytes. These observations are neither simultaneous
process-tree RSS nor GPU allocations.

| Case | Previous, MiB | Current, MiB | clingo, MiB |
| --- | ---: | ---: | ---: |
| Queens 1 | 11.969 [11.922, 11.969] | 11.969 [11.922, 12.109] | 5.422 [5.422, 5.547] |
| Queens 2 | 12.641 [12.547, 12.813] | 12.461 [12.391, 12.563] | 9.414 [9.031, 9.844] |
| Queens 3 | 12.117 [12.078, 12.250] | 12.016 [12.000, 12.047] | 5.438 [5.438, 5.594] |
| Queens 4 | 12.078 [12.016, 12.094] | 12.039 [11.938, 12.063] | 5.469 [5.469, 5.547] |
| Queens 5 | 12.617 [12.594, 12.641] | 12.586 [12.578, 12.609] | 5.570 [5.531, 5.688] |
| Queens 6 | 12.711 [12.672, 12.781] | 12.656 [12.578, 12.734] | 5.594 [5.563, 5.672] |
| SEND + MORE = MONEY | 24.078 [24.031, 24.953] | 24.117 [23.984, 24.141] | 8.375 [8.281, 8.891] |
| Task allocation | 35.797 [35.719, 35.906] | 19.445 [19.391, 19.453] | 21.852 [19.656, 24.000] |
| Shortest path | 12.906 [12.891, 12.953] | 12.992 [12.953, 13.031] | 5.641 [5.641, 5.672] |

Task allocation's peak RSS median falls from 35.797 to 19.445 MiB, below the
measured clingo median. Other cases show small increases, decreases or no median
change. There is no across-the-board RSS reduction. The measurements concern
the combined implementation, including shared model catalogs and ownership
changes; they do not isolate one representation's contribution.

## Instrumented CPU and Metal comparison

The matrix requests four closure/completion workers, batch size 64,
`--oracle auto`, and explicit eager and lazy profiles. Each block has
qualification, one warmup and three timed repetitions per case: six timed
samples per zetesis version and 12 for clingo. Native output contains full
JSON model records and statistics; clingo emits its JSON results. CPU and
Metal ran in separate windows.

The following eager wall times are milliseconds. Queens rows use the
`queens` suite; SEND and task allocation use `baseline`. The baseline suite
also repeats Queens 2; its observations are retained separately and not pooled
here. Shortest path has no matrix observation.

| Case | Previous Metal | Current Metal | Current CPU, JSON/stats | clingo in Metal window |
| --- | ---: | ---: | ---: | ---: |
| Queens 1 | 38.169 [36.868, 38.380] | 33.687 [32.986, 34.529] | 18.018 [17.950, 18.131] | 6.545 [5.160, 6.605] |
| Queens 2 | 118.541 [117.244, 119.614] | 111.328 [109.660, 112.250] | 95.920 [94.558, 96.870] | 123.295 [123.044, 124.620] |
| Queens 3 | 38.085 [36.835, 39.358] | 34.296 [34.275, 34.369] | 17.989 [17.723, 18.061] | 6.530 [5.202, 6.548] |
| Queens 4 | 29.178 [27.975, 29.400] | 26.772 [26.727, 26.906] | 10.479 [10.446, 10.655] | 6.524 [5.158, 6.557] |
| Queens 5 | 69.450 [68.052, 70.668] | 64.427 [64.086, 64.552] | 47.070 [46.424, 48.028] | 6.519 [5.151, 6.657] |
| Queens 6 | 72.261 [72.015, 72.324] | 65.100 [64.334, 67.140] | 47.706 [47.455, 47.917] | 6.575 [5.166, 6.633] |
| SEND + MORE = MONEY | 79.853 [77.079, 83.515] | 65.836 [64.527, 67.240] | 34.359 [34.222, 35.589] | 11.619 [11.518, 12.828] |
| Task allocation | 628.813 [620.086, 638.016] | 576.636 [565.774, 588.404] | 324.027 [320.635, 325.035] | 173.602 [155.769, 186.196] |

All nine suite/case populations have lower current Metal medians, with each
current sample below the previous sample range. SEND improves 17.6%, task
allocation 8.3%, and queens approximately 5.9–11.7%. The duplicated baseline
Queens 2 population changes from 117.898 to 110.938 ms. Three repetitions per
block support this bounded comparison, not a general crossover prediction.

Every admitted Metal run uses the Apple M4 Pro and the general formula
countermodel procedure, with positive GPU batch and work counts. CPU matrix
runs select the certified tight-support procedure and have no residual queries.
Thus the backend comparison includes a difference in membership procedure.
The matched CPU route is faster on every measured case. Current Metal is
faster than clingo on Queens 2; clingo is faster on the other unique inputs.

- SEND sends one candidate in one device batch. Each queens case sends 92
  candidates in two batches. Metal decides all of them with no CPU residuals,
  despite four requested workers.
- Task allocation sends 1,208 candidates in 19 batches. Current observations
  decide 13–14 on the device and complete 1,194–1,195 exactly on CPU, with four
  effective completion workers. Every candidate finishes without a failed
  or pending check. This is hybrid execution.
- Task allocation publishes 1,176 optimum ties at cost 5. Full output dominates
  parts of this contract: its Metal observation/output median is about
  218.815 ms;
  Queens 5/6 spend roughly 36 ms there. These results cannot be pooled with
  the ordinary human-output timings.

### Where time changes

The exclusive driver stages distinguish grounding, solving and
observation/output. Two separate CPU diagnostic runs per version show task
allocation solving decreasing approximately 88.004 → 83.030 ms, while
observation/output increases 4.549 → 7.285 ms. Its finer objective
scoring/retention phase decreases 5.372 → 4.287 ms. The ordinary improvement
does not imply every stage became faster.

In the six-sample CPU JSON/stats population, task allocation's wall median
decreases 330.311 → 324.027 ms (1.9%). Solving decreases 105.083 → 97.956 ms,
while output increases 210.145 → 215.040 ms. Output costs explain why this
contract shows a smaller end-to-end change.

Metal task allocation solving decreases 394.023 → 342.623 ms; its host GPU-call
interval decreases 211.203 → 165.011 ms. SEND's host GPU-call interval decreases
35.013 → 22.409 ms. Execution setup remains roughly 8.5–8.8 ms in timed runs.
The host GPU interval includes preparation/lowering performed by the call,
uploads, dispatch/wait and result handling; it is not kernel-only time.
No separate CPU-residual wall interval is available here.

Exclusive stages partition the driver when complete. Source loading, final
statistics and the JSON envelope have their own boundaries but remain in
process wall time. Finer phase intervals are nested and must not be summed
as another wall partition. Neither matrix measures RSS.

Named GPU accounting is mixed: SEND's peak increases from 1,657,564 to
1,966,508 bytes; Queens 2 increases from 2,495,592 to 2,511,856 bytes.
The other measured GPU peaks decrease. These are bounded component-accounting
receipts, not physical GPU memory or process RSS; lower elapsed time does not
imply lower storage in every case.

## Result agreement and applicability

Complete selected displays, symbol/model multiplicities, optimum ties and costs
agree with clingo for every admitted comparison. This boundary does not expose
clingo's hidden interpretations. Separately, canonical full typed native models,
costs and output records agree across both versions and both backends for all
360 passing native matrix observations, covering eight unique inputs.
Comparison sorts typed atom records and model records while preserving
multiplicity. Its scope is all answer sets for nonoptimized inputs and all
published optimum ties for optimized inputs; it does not compare unpublished
nonoptimal interpretations.

Both versions refuse explicit lazy execution for these matrix inputs with
`unsupported_oracle`: automatic oracle admission reaches the formula route,
which requires eager execution here. Each eight-report CPU or Metal campaign
accounts for 540 positions: 360 passing executions, 36 refusals and 144 later
positions blocked by those refusals. There are no capture faults or unresolved
children. These are applicability outcomes, not lazy timing samples or a claim
that supported relational lazy execution is unavailable.

This comparison does not include a new 94-case timing survey. The preceding
revision's broader survey remains historical evidence, not observations of
the current executable.

## Optional finite-table experiment

The [finite-table library](../rust/finite-tables.md) prepares typed value-to-row
support bitsets, intersects surviving rows and projects witnessed domains.
The maintained experiment compares complete outputs with a prepared full-row
scan and with independent queries sharing one table through Rayon.

All **2,880 query observations across 90 batches** agree with an independent
whole-row reconstruction, including repeated-variable aliases, duplicate row
occurrences and restored domains. No route refused or failed. There is no GPU
table implementation in this experiment and no ordinary grounder selects it.

Each entry below is a median [minimum, maximum] in **microseconds for 32
queries**, from only three timed batches per route. Preparation, independent
validation and JSON publication are excluded; projection and common-output
conversion are included.

| Fixture | Rows | Prepared scan | Scalar table | Rayon table, 4 workers |
| --- | ---: | ---: | ---: | ---: |
| Correlated | 128 | 82.042 [81.417, 108.208] | 43.250 [43.167, 44.667] | 40.417 [38.417, 47.000] |
| Correlated | 1,024 | 669.500 [644.167, 680.417] | 115.667 [95.250, 117.625] | 67.417 [59.375, 77.125] |
| Independent | 128 | 181.917 [181.792, 182.292] | 45.417 [44.209, 53.292] | 34.333 [33.709, 51.000] |
| Independent | 1,024 | 1,868.500 [1,836.292, 1,906.875] | 128.875 [109.667, 142.250] | 90.708 [75.833, 138.167] |
| Aliased | 128 | 181.875 [180.750, 183.625] | 38.458 [37.417, 45.167] | 30.584 [29.667, 30.917] |
| Aliased | 1,024 | 1,589.667 [1,570.167, 1,629.708] | 122.417 [121.291, 124.458] | 65.000 [61.041, 115.000] |

The scalar table medians are approximately 1.9–14.5 times faster than scanning.
Scan/table/Rayon run in that fixed order; overlapping scalar/Rayon ranges
and three batches do not establish a stable crossover. Worker intervals
overlap under Rayon, so their sum is not parallel wall time.

Preparation must be amortized. For the independent 1,024-row fixture, the
single observed preparation costs are 16.416 µs for scan domains, 546.042 µs
for the typed relation, 274.125 µs for its table index and 35.625 µs for the
Rayon pool. The table borrows the relation. Its index retains 9,080 bytes,
and the common complete output uses 91,168 bytes per batch versus 274,304
for the scan. The index adds retained storage; these are named capacities,
not RSS or total concurrent memory.

The operation's limits are 100,000,000 charged work and 128 MiB of live
table-operation capacity. Caller-owned fixtures and other retained results
have separate limits. This result supports evaluating a real grounding
consumer with a proved-complete finite relation. A surviving table row is a
constraint witness, not ASP producer support or answer-set membership.

## Reproduce the comparisons

Use the [checkout installation instructions](https://github.com/GregoryGelfond/zetesis/blob/main/README.md#install-and-run)
to prepare the solver and maintained `zetesis-perf` and `zetesis-bench`
commands. Keep release binaries separate and use one fixed performance tool
for both solver versions. Rebuilding named source revisions may yield different
executable hashes; record the binaries actually used.

From the checkout root, assign absolute executable paths and create a fresh
output directory:

```sh
perf_command=/absolute/path/to/zetesis-perf
previous_solver=/absolute/path/to/previous/zetesis
current_solver=/absolute/path/to/current/zetesis
clingo_command=/absolute/path/to/clingo
bench_command=/absolute/path/to/current/zetesis-bench
results_dir=$(mktemp -d "${TMPDIR:-/tmp}/zetesis-perf.XXXXXX")
```

The functions below save the real command exit status and return it. Run the
listed block calls one at a time, rather than pasting them as an unattended
sequence. Inspect each nonzero report before issuing the next call. Exit 0 means the report
passes; exit 1 means nonpassing evidence, which can include refusals or operational
failures. Setup/publication failures exit 2. Check `accounted`, dispositions,
faults and unresolved children; a refused or uncaptured cell has no solve time.

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
    printf '%s\n' "$comparison_exit" > "$results_dir/ordinary-$1.exit" || return 2
    return "$comparison_exit"
}
ordinary previous-1 "$previous_solver"
ordinary current-1 "$current_solver"
ordinary current-2 "$current_solver"
ordinary previous-2 "$previous_solver"
```

Run the CPU and Metal matrices in separate quiet windows. Use
`comparison_backend=cpu` first, then `comparison_backend=metal` and repeat the
eight calls. Metal requires an accessible physical adapter; installing the solver
requires no separate qualification procedure.

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
        --timeout-seconds 10 --campaign-seconds 30 \
        --sample-bytes 33554432 --native-report-bytes 33554432 \
        --capture-bytes 536870912 --report-bytes 1073741824; then
        comparison_exit=0
    else
        comparison_exit=$?
    fi
    printf '%s\n' "$comparison_exit" \
        > "$results_dir/$comparison_backend-$1-$2.exit" || return 2
    return "$comparison_exit"
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

Child and campaign time limits are polling budgets; setup, cleanup and report
publication have their own scope. The measured matrix profiles use the tool's
268,435,456-byte completion scratch default. Report records retain that value
and the complete native arguments.

The separate table population uses the following six commands, with explicit
operation limits. This loop attempts all six cases even after a nonzero result
and records each exit status; inspect every report before interpreting the
population. Each JSON-lines stream retains its subject, preparation, batch
results and refusals. These are bounded library experiments, not solver
invocations.

```sh
for table_case in correlated independent aliased; do
    for table_rows in 128 1024; do
        if "$bench_command" table --case "$table_case" --rows "$table_rows" \
            --queries 32 --workers 4 --warmups 1 --repetitions 3 \
            --max-table-work 100000000 --max-table-bytes 134217728 \
            > "$results_dir/table-$table_case-$table_rows.jsonl"; then
            comparison_exit=0
        else
            comparison_exit=$?
        fi
        printf '%s\n' "$comparison_exit" \
            > "$results_dir/table-$table_case-$table_rows.exit"
    done
done
```

The recorded table acquisition additionally imposed a 10-second child deadline
and 64 MiB capture ceiling using the maintained bounded process library.
The direct commands above reproduce the fixture and algorithm schedule;
they do not supply that external deadline.

The corpus and measurement tools are in the repository. Original raw captures
are retained privately and are not shipped with this manual. These commands
generate new self-contained reports containing captured outputs, identities,
options and result checks; no public download of the original records is claimed.
