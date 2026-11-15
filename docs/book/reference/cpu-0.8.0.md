# CPU comparison for the 0.8.0 candidate

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

On 15 November 2026, the qualified 0.8.0 candidate was compared with clingo
across **315 distinct workloads**. Of **299 complete paired timings**, the
established 3% band gives **214 zetesis wins, nine close and 76 clingo wins**.
The other **16** retain an unavailable reference; they are not timing wins.
Strictly comparing the recorded times gives 214 zetesis-faster and 85
clingo-faster observations, with no exact equality. All nine close cases
slightly favor clingo.

The [per-case TSV](observations/cpu-0.8.0-process.tsv) gives every selected
workload and exact nanosecond times. The
[receipt](observations/cpu-0.8.0-process.json) records SHA-256 workload identities,
source files and amendments, contracts, profiles, report hashes, duplicate
observations and classification. The full table below is the same 315-workload
population. Historical [0.7.0 measurements](cpu-0.7.0.md) remain unchanged.

## Conditions and interpretation

The measured source was qualified as
`3d395e57f14ca39c8ab436dfc1dbb7439982e4b7`, then incorporated without a source-tree
change into `dev/0.8.0` at `aa0df11fbd76420e557a858c053d29581a19e1d6`.
The runs preceded the release-number and release-documentation changes; the
frozen executable still identified itself as 0.7.0. These are retained
measurements of the implementation, not a fresh repeat after the merge.
Capture ran from 06:30:10 to 07:10:36 UTC on 15 November 2026.

Zetesis used **14 CPU workers** on an Apple M4 Pro, automatic grounding and
checking, and a 24 GiB named-storage allowance. Clingo 5.8.2 used **one worker**.
Each solver had one qualification run and **one timed fresh process**, no
warmup and no separate RSS run. The outer process deadline was **120 seconds**, except for the reported-planner
panel, which allowed 130 seconds around the solver's cooperative 120-second
deadline; the scalability panel requested indexed formula joins.
The receipt preserves these per-panel controls. All native execution was on
CPU; these results do not measure GPU speed.

Elapsed time includes fresh-process startup, grounding, solving and captured
JSON output, with statistics enabled for zetesis. It excludes comparison
hashing. Qualification requires complete selected answer families, including
multiplicity, final optimal ties and costs. Native full models also agree
between qualification and timing; hidden reference interpretations are not
observed. Interrupted or reference-unavailable runs do not receive a timing
vote.

The exact zetesis/clingo ratio is classified below 0.97 as a zetesis win and
above 1.03 as a clingo win; both boundaries count as close. This is the
published 3% reporting convention, **not a statistical equivalence test**.
One sample is a performance screen: small differences and parallel-search
variation need repeated measurement before a causal improvement is claimed.
The canonical benchmark's strict faster/slower verdicts remain in the receipt.

The original schedule has 320 positions. Five extra positions repeat three
workload identities: task allocation and SEND+MORE=MONEY occur in three
panels each, and queens variant 02 in two. Each gets one vote in its original
corpus position, using the SHA-256 identity rather than its display label.
All repeated observations have the same faster/slower and 3% classification.
No timing samples are pooled across panels and no summed process total is
presented as one elapsed corpus run.

On the 16 unavailable arithmetic references, clingo reached the qualification
deadline. Zetesis completed with exhaustive UNSAT and no interruption on each;
clingo supplied no confirming complete family. Paired timing was not attempted.
The 0.7.0 record used a 600-second deadline, three timed samples and 299 inputs,
so neither its 13 incomplete cases nor its headline win count is directly
comparable with this campaign.

## Panel scorecard

| Panel | Distinct | Paired | zetesis wins | Close | clingo wins | Unavailable |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| reported-default-120s | 2 | 2 | 0 | 0 | 2 | 0 |
| usa-plus-queens-w14 | 2 | 2 | 1 | 0 | 1 | 0 |
| corpus95-w14 | 95 | 95 | 93 | 1 | 1 | 0 |
| public-supplement-w14 | 12 | 12 | 5 | 1 | 6 | 0 |
| sigma-larger5-w14 | 5 | 5 | 1 | 0 | 4 | 0 |
| reported-workloads-w14 | 3 | 3 | 1 | 0 | 2 | 0 |
| arithmetic-schedule-w14 | 6 | 6 | 0 | 1 | 5 | 0 |
| family-variants-w14 | 2 | 2 | 1 | 0 | 1 | 0 |
| family-components-w14 | 2 | 2 | 0 | 1 | 1 | 0 |
| inner-heavy18-w14 | 18 | 18 | 12 | 0 | 6 | 0 |
| arithmetic-defaults-w14 | 8 | 8 | 4 | 1 | 3 | 0 |
| typed65-w14 | 65 | 65 | 34 | 2 | 29 | 0 |
| scalability21-w14 | 18 | 18 | 13 | 0 | 5 | 0 |
| root6-w14 | 6 | 6 | 6 | 0 | 0 | 0 |
| series22-w14 | 20 | 20 | 12 | 0 | 8 | 0 |
| ar51-w14 | 51 | 35 | 31 | 2 | 2 | 16 |
| **Total** | **315** | **299** | **214** | **9** | **76** | **16** |

Wide Mastermind took 0.958 s against 22.715 s for clingo, and the parity-chain
p47 contradiction took 10.766 ms against 55.250 s. Integrated Family Crossing,
Poker, USA and the reported planner still favor clingo. Larger quantified
parity at size 28 favors zetesis; the other four cases in that panel favor
clingo. Arithmetic schedule h399/d80 takes 1.399 s against clingo's 0.247 s.
The table records useful strengths and unresolved losses, not a blanket claim
about planning, arithmetic or all problems at a complexity level.

## Reproducing the selections

Use `zetesis-bench`; no separate timing or model-decoding implementation is
needed. From the repository root, build the solver and benchmark tool with the
pinned toolchain, then run a canonical panel:

```sh
cargo build --locked --release --all-features -p zetesis-cli -p zetesis-bench
ROOT=$(pwd -P)
NATIVE="$ROOT/target/release/zetesis"
BENCH="$ROOT/target/release/zetesis-bench"
CLINGO=$(command -v clingo)
OUT=$(mktemp -d)

"$BENCH" run "$ROOT/examples/correctness" --suite corpus \
  --zetesis "$NATIVE" --clingo "$CLINGO" --native-interface solve \
  --backend cpu --grounder auto --oracle auto --threads 14 --clingo-threads 1 \
  --memory 24GiB --warmups 0 --repetitions 1 --memory-runs 0 \
  --timeout-seconds 120 --campaign-seconds 7200 \
  --sample-bytes 134217728 --answer-bytes 134217728 \
  --native-report-bytes 134217728 --native-atoms 4194304 \
  --native-value-nodes 8388608 --capture-bytes 2147483648 \
  --report-bytes 4294967296 --report "$OUT/corpus95.json" --json --color never
```

These commands reproduce the selection and protocol, not the timing of a
particular machine. Compare an old and new saved report with
`zetesis-bench compare before=before.json after=after.json`; the tool requires
matching identities and profiles. Retain the supplied limits and all failed
slots. A fresh comparison should use enough repeated samples for the claim it
is intended to support.

The other in-repository selections use the same command with the following
selection arguments instead of `--suite corpus`, and a separate report name:

| Panel | Selection arguments |
| --- | --- |
| `scalability21-w14` | `--suite scalability --examples "$ROOT/examples" --include-einstein --formula-joins indexed` |
| `series22-w14` | `--suite series` |
| `typed65-w14` | `--workloads "$ROOT/crates/zetesis-validation/tests/fixtures/typed-selection65.json"` |
| `public-supplement-w14` | `--workloads "$ROOT/docs/book/reference/observations/cpu-0.8.0-manifests/public-supplement-w14.json"` |
| `family-components-w14` | `--workloads "$ROOT/docs/book/reference/observations/cpu-0.8.0-manifests/family-components-w14.json"` |
| `root6-w14` | `--workloads "$ROOT/docs/book/reference/observations/cpu-0.8.0-manifests/root6-w14.json"` |

The public supplement includes Poker and the original wide Mastermind source;
scalability includes the differently shaped Mastermind controls. The typed
manifest applies its amendments through the existing benchmark constructors.
No original input needs editing in place.

The remaining panels use declared external bundles. Their manifests below
have root `.`: copy each manifest beside the exact source files it lists, with
its relative names, include spellings and SHA-256 digests, then pass that
manifest to `--workloads` instead of a suite. **The repository does not contain
all externally reported programs**, and a manifest does not supply missing
source contents. The default 95-case command alone does not reproduce the full
315-input scorecard.

- [reported-default-120s](observations/cpu-0.8.0-manifests/reported-default-120s.json).
- [usa-plus-queens-w14](observations/cpu-0.8.0-manifests/usa-plus-queens-w14.json).
- [sigma-larger5-w14](observations/cpu-0.8.0-manifests/sigma-larger5-w14.json).
- [reported-workloads-w14](observations/cpu-0.8.0-manifests/reported-workloads-w14.json).
- [arithmetic-schedule-w14](observations/cpu-0.8.0-manifests/arithmetic-schedule-w14.json).
- [family-variants-w14](observations/cpu-0.8.0-manifests/family-variants-w14.json).
- [inner-heavy18-w14](observations/cpu-0.8.0-manifests/inner-heavy18-w14.json).
- [arithmetic-defaults-w14](observations/cpu-0.8.0-manifests/arithmetic-defaults-w14.json).
- [ar51-w14](observations/cpu-0.8.0-manifests/ar51-w14.json).

The larger quantified files can be recreated from the tracked parity and
pigeonhole generators: change only `n = 6` to 20/24/28 in
`examples/scalability/two-qbf-parity/two_qbf_parity.lp`, or `holes = 4` to 7/8 in
`examples/scalability/two-qbf-pigeonhole/two_qbf_pigeonhole.lp`, and verify the
manifest hashes. USA, the reported planners and other external bundles require
the corresponding original inputs; the receipt preserves their exact identity.
For `reported-default-120s`, use `--time-limit 120 --timeout-seconds 130 --campaign-seconds 900` to retain
its cooperative solver deadline and outer allowances. The benchmark guide describes the distinction
between that option and the outer `--timeout-seconds` deadline.

The receipt identifies the measured executables by SHA-256:

```text
zetesis        4ce7e0c37863c284f499c2293ab306a172752c170159a7a36fbc329c63029edc
zetesis-bench  031285d3437617550687a3960cbfefcdbc3ad8b26023642c5b918d418b774f7f
clingo         36b7424fe275f3070e1533bb1c1ed5be52a84a6fbeea8b5f773a8814b1ec61d1
```

## All 315 workloads

Times are milliseconds, rounded only for display; classifications use exact
nanoseconds. The entry path and amendment label distinguish workloads whose
short display names coincide. An unavailable row has no paired timed sample;
its dashes are not zero-time observations.

### reported-default-120s

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `five_crosser_light_mode_a_verify_v2.lp` — five_crosser_light_mode_a_verify_v2 | 417.266 | 26.049 | clingo |
| `five_crosser_light_mode_b_plan.lp` — five_crosser_light_mode_b_plan | 22,880.583 | 2,679.637 | clingo |

### usa-plus-queens-w14

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `usa/usaSP1.lp` — usa/usaSP1 | 1,279.972 | 498.560 | clingo |
| `queens-n12.lp` — queens-n12 | 125.234 | 1,688.966 | zetesis |

### corpus95-w14

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `scenarios/equality-generalized-tsp/01-basic.lp` — equality-generalized-tsp/01-basic | 6.365 | 9.410 | zetesis |
| `scenarios/equality-generalized-tsp/02-larger.lp` — equality-generalized-tsp/02-larger | 6.222 | 7.718 | zetesis |
| `scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp` — equality-generalized-tsp/03-unreachable-subset-unsat | 4.752 | 6.188 | zetesis |
| `scenarios/shortest-path/variant-01/01-basic.lp` — variant-01/01-basic | 4.696 | 7.676 | zetesis |
| `scenarios/shortest-path/variant-01/02-start-equals-end.lp` — variant-01/02-start-equals-end | 4.826 | 7.715 | zetesis |
| `scenarios/shortest-path/variant-01/03-zero-cost-detour.lp` — variant-01/03-zero-cost-detour | 4.696 | 7.693 | zetesis |
| `scenarios/shortest-path/variant-01/04-no-path.lp` — variant-01/04-no-path | 4.739 | 6.207 | zetesis |
| `scenarios/shortest-path/variant-01/05-multi-path.lp` — variant-01/05-multi-path | 4.685 | 7.683 | zetesis |
| `scenarios/shortest-path/variant-01/06-layered-dag.lp` — variant-01/06-layered-dag | 6.274 | 7.702 | zetesis |
| `scenarios/shortest-path/variant-01/07-cycles.lp` — variant-01/07-cycles | 4.676 | 7.693 | zetesis |
| `scenarios/shortest-path/variant-01/08-negative-weights.lp` — variant-01/08-negative-weights | 4.752 | 6.189 | zetesis |
| `scenarios/shortest-path/variant-02/01-basic.lp` — variant-02/01-basic | 4.700 | 7.675 | zetesis |
| `scenarios/shortest-path/variant-02/02-start-equals-end.lp` — variant-02/02-start-equals-end | 4.789 | 7.707 | zetesis |
| `scenarios/shortest-path/variant-02/03-before-forces-detour.lp` — variant-02/03-before-forces-detour | 4.671 | 7.677 | zetesis |
| `scenarios/shortest-path/variant-02/04-after-forces-extension.lp` — variant-02/04-after-forces-extension | 4.717 | 6.195 | zetesis |
| `scenarios/shortest-path/variant-02/05-ordering-unsat.lp` — variant-02/05-ordering-unsat | 4.700 | 7.675 | zetesis |
| `scenarios/shortest-path/variant-02/06-layered-dag-before.lp` — variant-02/06-layered-dag-before | 6.238 | 7.733 | zetesis |
| `scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp` — variant-02/07-layered-dag-before-after | 6.213 | 7.677 | zetesis |
| `scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp` — variant-02/08-tie-break-under-ordering | 4.819 | 7.678 | zetesis |
| `scenarios/shortest-path/variant-02/09-negative-weights.lp` — variant-02/09-negative-weights | 4.680 | 7.671 | zetesis |
| `scenarios/shortest-path/variant-03/01-basic.lp` — variant-03/01-basic | 4.798 | 7.718 | zetesis |
| `scenarios/shortest-path/variant-03/02-start-equals-end.lp` — variant-03/02-start-equals-end | 4.720 | 7.676 | zetesis |
| `scenarios/shortest-path/variant-03/03-budget-forces-detour.lp` — variant-03/03-budget-forces-detour | 4.819 | 7.737 | zetesis |
| `scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp` — variant-03/04-cost-at-cap-allowed | 4.707 | 7.288 | zetesis |
| `scenarios/shortest-path/variant-03/05-budget-unsat.lp` — variant-03/05-budget-unsat | 4.791 | 7.738 | zetesis |
| `scenarios/shortest-path/variant-03/06-layered-dag-cap.lp` — variant-03/06-layered-dag-cap | 7.736 | 9.206 | zetesis |
| `scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp` — variant-03/07-layered-dag-tight-cap | 7.856 | 9.207 | zetesis |
| `scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp` — variant-03/08-tie-break-under-cap | 4.712 | 7.680 | zetesis |
| `scenarios/shortest-path/variant-03/09-negative-weights.lp` — variant-03/09-negative-weights | 4.782 | 7.690 | zetesis |
| `scenarios/shortest-path/variant-04/01-basic.lp` — variant-04/01-basic | 4.691 | 7.691 | zetesis |
| `scenarios/shortest-path/variant-04/02-start-equals-end.lp` — variant-04/02-start-equals-end | 4.780 | 7.741 | zetesis |
| `scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp` — variant-04/03-before-forces-detour-within-budget | 4.708 | 7.690 | zetesis |
| `scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp` — variant-04/04-ordering-violates-budget | 4.795 | 7.707 | zetesis |
| `scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp` — variant-04/05-after-and-budget-interact | 4.687 | 7.698 | zetesis |
| `scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp` — variant-04/06-layered-dag-ordering-cap | 7.826 | 9.214 | zetesis |
| `scenarios/shortest-path/variant-04/07-layered-dag-combined.lp` — variant-04/07-layered-dag-combined | 7.718 | 9.176 | zetesis |
| `scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp` — variant-04/08-tie-break-under-ordering-and-cap | 6.311 | 7.713 | zetesis |
| `scenarios/shortest-path/variant-04/09-negative-weights.lp` — variant-04/09-negative-weights | 4.689 | 7.686 | zetesis |
| `scenarios/task-allocation/variant-01/01-basic.lp` — variant-01/01-basic | 4.844 | 6.215 | zetesis |
| `scenarios/task-allocation/variant-01/02-agent-reuse.lp` — variant-01/02-agent-reuse | 4.702 | 6.149 | zetesis |
| `scenarios/task-allocation/variant-01/03-selective-compatibility.lp` — variant-01/03-selective-compatibility | 4.748 | 6.180 | zetesis |
| `scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp` — variant-01/04-no-compatible-agent-unsat | 4.677 | 6.178 | zetesis |
| `scenarios/task-allocation/variant-01/05-larger-mix.lp` — variant-01/05-larger-mix | 4.782 | 7.723 | zetesis |
| `scenarios/task-allocation/variant-02/01-basic.lp` — variant-02/01-basic | 4.692 | 6.134 | zetesis |
| `scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp` — variant-02/02-makespan-tiebreak | 4.704 | 6.156 | zetesis |
| `scenarios/task-allocation/variant-02/03-cost-dominates.lp` — variant-02/03-cost-dominates | 4.667 | 7.685 | zetesis |
| `scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp` — variant-02/04-no-compatible-agent-unsat | 4.764 | 6.192 | zetesis |
| `scenarios/task-allocation/variant-02/05-larger-mix.lp` — variant-02/05-larger-mix | 6.229 | 7.686 | zetesis |
| `scenarios/task-allocation/variant-03/01-basic.lp` — variant-03/01-basic | 4.778 | 6.183 | zetesis |
| `scenarios/task-allocation/variant-03/02-multiple-groups.lp` — variant-03/02-multiple-groups | 4.695 | 6.143 | zetesis |
| `scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp` — variant-03/03-mixed-grouped-ungrouped | 4.761 | 6.155 | zetesis |
| `scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp` — variant-03/04-incompatible-group-unsat | 4.706 | 6.150 | zetesis |
| `scenarios/task-allocation/variant-03/05-larger-mix.lp` — variant-03/05-larger-mix | 4.746 | 6.172 | zetesis |
| `scenarios/task-allocation/variant-04/01-basic.lp` — variant-04/01-basic | 7.743 | 25.842 | zetesis |
| `scenarios/task-allocation/variant-04/02-precedence.lp` — variant-04/02-precedence | 6.297 | 7.692 | zetesis |
| `scenarios/task-allocation/variant-04/03-agent-serialization.lp` — variant-04/03-agent-serialization | 7.734 | 27.248 | zetesis |
| `scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp` — variant-04/04-window-too-tight-unsat | 6.382 | 7.701 | zetesis |
| `scenarios/task-allocation/variant-04/05-larger-mix.lp` — variant-04/05-larger-mix | 27.935 | 197.879 | zetesis |
| `scenarios/traveling-salesman/variant-01/01-basic.lp` — variant-01/01-basic | 4.830 | 7.792 | zetesis |
| `scenarios/traveling-salesman/variant-01/02-multiple-tours.lp` — variant-01/02-multiple-tours | 4.685 | 7.687 | zetesis |
| `scenarios/traveling-salesman/variant-01/03-asymmetric.lp` — variant-01/03-asymmetric | 6.269 | 7.738 | zetesis |
| `scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp` — variant-01/04-subtour-unsat | 4.720 | 6.155 | zetesis |
| `scenarios/traveling-salesman/variant-01/05-ring.lp` — variant-01/05-ring | 6.313 | 7.692 | zetesis |
| `scenarios/traveling-salesman/variant-02/01-basic.lp` — variant-02/01-basic | 4.701 | 7.699 | zetesis |
| `scenarios/traveling-salesman/variant-02/02-single-salesman.lp` — variant-02/02-single-salesman | 4.748 | 6.186 | zetesis |
| `scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp` — variant-02/03-too-many-salesmen-unsat | 4.700 | 6.168 | zetesis |
| `scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp` — variant-02/04-equal-cost-split | 4.798 | 7.688 | zetesis |
| `scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp` — variant-02/05-larger-asymmetric | 4.691 | 7.676 | zetesis |
| `scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp` — variant-02/06-unreachable-edge | 6.235 | 7.726 | zetesis |
| `scenarios/traveling-salesman/variant-03/01-basic.lp` — variant-03/01-basic | 4.689 | 7.691 | zetesis |
| `scenarios/traveling-salesman/variant-03/02-single-salesman.lp` — variant-03/02-single-salesman | 4.790 | 7.728 | zetesis |
| `scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp` — variant-03/03-depot-crossing-unsat | 4.674 | 7.692 | zetesis |
| `scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp` — variant-03/04-equal-cost-split | 4.729 | 7.735 | zetesis |
| `scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp` — variant-03/05-larger-three-depots | 6.227 | 7.684 | zetesis |
| `scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp` — variant-03/06-unreachable-edge | 4.706 | 6.193 | zetesis |
| `scenarios/traveling-salesman/variant-04/01-basic.lp` — variant-04/01-basic | 6.193 | 7.691 | zetesis |
| `scenarios/traveling-salesman/variant-04/02-single-salesman.lp` — variant-04/02-single-salesman | 6.304 | 7.713 | zetesis |
| `scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp` — variant-04/03-window-too-tight-unsat | 4.676 | 7.682 | zetesis |
| `scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp` — variant-04/04-depot-window-too-tight-unsat | 4.750 | 7.735 | zetesis |
| `scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp` — variant-04/05-three-depots-asymmetric-times | 6.234 | 7.670 | zetesis |
| `scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp` — variant-04/06-unreachable-edge | 6.381 | 7.724 | zetesis |
| `scenarios/traveling-salesman/variant-05/01-basic.lp` — variant-05/01-basic | 6.230 | 7.709 | zetesis |
| `scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp` — variant-05/02-tight-bound-exact | 6.334 | 7.741 | zetesis |
| `scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp` — variant-05/03-revisit-too-tight-unsat | 4.686 | 7.670 | zetesis |
| `scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp` — variant-05/04-depot-and-vertex-revisits | 6.322 | 7.748 | zetesis |
| `scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp` — variant-05/05-three-depots-mixed | 6.257 | 7.683 | zetesis |
| `scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp` — variant-05/06-unreachable-edge | 6.299 | 7.718 | zetesis |
| `standalone/n-queens/variant-01.lp` — n-queens/variant-01 | 6.217 | 8.862 | zetesis |
| `standalone/n-queens/variant-02.lp` — n-queens/variant-02 | 39.845 | 125.302 | zetesis |
| `standalone/n-queens/variant-03.lp` — n-queens/variant-03 | 6.243 | 7.717 | zetesis |
| `standalone/n-queens/variant-04.lp` — n-queens/variant-04 | 6.315 | 7.702 | zetesis |
| `standalone/n-queens/variant-05.lp` — n-queens/variant-05 | 7.849 | 9.247 | zetesis |
| `standalone/n-queens/variant-06.lp` — n-queens/variant-06 | 7.845 | 7.687 | close |
| `standalone/send-money/send-money.lp` — send-money/send-money | 10.776 | 15.237 | zetesis |
| `family-crossing/integrated.lp` — family-crossing/integrated | 1,958.365 | 889.255 | clingo |

### public-supplement-w14

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `scalability/parity-chain/parity_chain.lp` — parity-chain/parity_chain | 6.327 | 7.913 | zetesis |
| `scalability/parity-mixed/parity_mixed.lp` — parity-mixed/parity_mixed | 7.750 | 7.703 | close |
| `scalability/parity-tseitin/parity_tseitin.lp` — parity-tseitin/parity_tseitin | 4.782 | 7.699 | zetesis |
| `scalability/pigeonhole-clauses/pigeonhole_clauses.lp` — pigeonhole-clauses/pigeonhole_clauses | 4.686 | 6.150 | zetesis |
| `scalability/poker/draw_poker_tests.lp` — poker/draw_poker_tests | 354.527 | 107.219 | clingo |
| `scalability/shifted-subset-sum/subset_sum.lp` — shifted-subset-sum/subset_sum | 6.407 | 7.757 | zetesis |
| `scalability/strategic-companies/strategic_companies.lp` — strategic-companies/strategic_companies | 12.373 | 7.721 | clingo |
| `scalability/two-qbf/two_qbf.lp` — two-qbf/two_qbf | 39.146 | 10.823 | clingo |
| `scalability/two-qbf-parity/two_qbf_parity.lp` — two-qbf-parity/two_qbf_parity | 18.444 | 9.195 | clingo |
| `scalability/two-qbf-pigeonhole/two_qbf_pigeonhole.lp` — two-qbf-pigeonhole/two_qbf_pigeonhole | 16.929 | 9.237 | clingo |
| `scalability/two-qbf-tseitin/two_qbf_tseitin.lp` — two-qbf-tseitin/two_qbf_tseitin | 22.919 | 9.186 | clingo |
| `wide-mastermind/68/mm-68-examples.lp` — 68/mm-68-examples | 958.083 | 22,715.332 | zetesis |

### sigma-larger5-w14

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `parity20.lp` — parity20 | 69.974 | 28.911 | clingo |
| `parity24.lp` — parity24 | 96.913 | 49.981 | clingo |
| `parity28.lp` — parity28 | 125.446 | 215.491 | zetesis |
| `pigeonhole7.lp` — pigeonhole7 | 109.644 | 25.865 | clingo |
| `pigeonhole8.lp` — pigeonhole8 | 201.721 | 79.950 | clingo |

### reported-workloads-w14

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `s_crossing.lp` — s_crossing | 592.658 | 27.549 | clingo |
| `sudoku_simple.lp` — sudoku_simple | 81.676 | 1,881.142 | zetesis |
| `elias_pr_simp.lp` — elias_pr_simp | 148.920 | 80.773 | clingo |

### arithmetic-schedule-w14

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `S3-h20-d4.lp` — S3-h20-d4 | 9.336 | 7.791 | clingo |
| `S3-h19-d4.lp` — S3-h19-d4 | 7.732 | 7.694 | close |
| `S3-h100-d20.lp` — S3-h100-d20 | 99.762 | 19.800 | clingo |
| `S3-h99-d20.lp` — S3-h99-d20 | 87.672 | 18.255 | clingo |
| `S3-h400-d80.lp` — S3-h400-d80 | 1,612.519 | 395.018 | clingo |
| `S3-h399-d80.lp` — S3-h399-d80 | 1,399.354 | 247.407 | clingo |

### family-variants-w14

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `family_crossing_vanilla_mode_b.lp` — family_crossing_vanilla_mode_b | 382.656 | 11,388.304 | zetesis |
| `family_crossing_mode_a_observation.lp` — family_crossing_mode_a_observation | 126.137 | 15.381 | clingo |

### family-components-w14

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `family_crossing_complete_domain.lp` — family_crossing_complete_domain | 53.011 | 12.207 | clingo |
| `eec_htp_integration_v5.lp` — eec_htp_integration_v5 | 9.349 | 9.243 | close |

### inner-heavy18-w14

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `programs/xorchain-14.lp` — programs/xorchain-14 | 13.959 | 12.243 | clingo |
| `programs/xorchain-16.lp` — programs/xorchain-16 | 15.416 | 13.764 | clingo |
| `programs/xorchain-18.lp` — programs/xorchain-18 | 16.999 | 19.843 | zetesis |
| `programs/xorchain-20.lp` — programs/xorchain-20 | 18.422 | 22.766 | zetesis |
| `programs/tseitin-24.lp` — programs/tseitin-24 | 13.998 | 12.211 | clingo |
| `programs/tseitin-28.lp` — programs/tseitin-28 | 13.844 | 12.204 | clingo |
| `programs/tseitin-30.lp` — programs/tseitin-30 | 15.357 | 12.213 | clingo |
| `programs/php-7.lp` — programs/php-7 | 31.976 | 21.290 | clingo |
| `programs/php-8.lp` — programs/php-8 | 65.228 | 75.589 | zetesis |
| `cores/xorchain-14.core.lp` — cores/xorchain-14.core | 6.251 | 7.675 | zetesis |
| `cores/xorchain-16.core.lp` — cores/xorchain-16.core | 6.287 | 9.173 | zetesis |
| `cores/xorchain-18.core.lp` — cores/xorchain-18.core | 7.716 | 9.172 | zetesis |
| `cores/xorchain-20.core.lp` — cores/xorchain-20.core | 7.882 | 10.695 | zetesis |
| `cores/tseitin-24.core.lp` — cores/tseitin-24.core | 6.247 | 7.686 | zetesis |
| `cores/tseitin-28.core.lp` — cores/tseitin-28.core | 6.266 | 7.658 | zetesis |
| `cores/tseitin-30.core.lp` — cores/tseitin-30.core | 6.183 | 7.673 | zetesis |
| `cores/php-7.core.lp` — cores/php-7.core | 14.600 | 16.700 | zetesis |
| `cores/php-8.core.lp` — cores/php-8.core | 43.313 | 114.745 | zetesis |

### arithmetic-defaults-w14

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `S1a-default.lp` — S1a-default | 15.522 | 13.581 | clingo |
| `S1b-default.lp` — S1b-default | 14.057 | 15.579 | zetesis |
| `S2-default.lp` — S2-default | 9.506 | 10.888 | zetesis |
| `S3-default.lp` — S3-default | 23.105 | 18.717 | clingo |
| `S5-default.lp` — S5-default | 13.941 | 7.695 | clingo |
| `S6-default.lp` — S6-default | 8.424 | 12.245 | zetesis |
| `C2-default.lp` — C2-default | 7.881 | 7.686 | close |
| `C2-odd.lp` — C2-odd | 6.281 | 7.724 | zetesis |

### typed65-w14

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `subset_sum.lp` — subset_sum 17→19 | 4.752 | 6.163 | zetesis |
| `subset_sum.lp` — subset_sum 8→30 17→61 | 4.706 | 6.156 | zetesis |
| `subset_sum.lp` — subset_sum 8→30 17→63 | 4.778 | 6.169 | zetesis |
| `two_qbf.lp` — two_qbf 68→52 | 29.020 | 9.272 | clingo |
| `two_qbf.lp` — two_qbf 1→2 | 38.522 | 9.229 | clingo |
| `two_qbf.lp` — two_qbf 3→2 68→40 | 24.449 | 9.242 | clingo |
| `two_qbf.lp` — two_qbf 8→10 68→85 | 56.332 | 10.751 | clingo |
| `two_qbf_parity.lp` — two_qbf_parity 1→0 | 16.995 | 9.206 | clingo |
| `two_qbf_parity.lp` — two_qbf_parity 1→2 | 16.940 | 9.224 | clingo |
| `two_qbf_parity.lp` — two_qbf_parity 2→3 1→3 | 20.007 | 9.218 | clingo |
| `two_qbf_parity.lp` — two_qbf_parity 1→4 6→5 1→2 | 13.912 | 9.196 | clingo |
| `two_qbf_tseitin.lp` — two_qbf_tseitin 1→0 | 21.393 | 10.753 | clingo |
| `two_qbf_tseitin.lp` — two_qbf_tseitin 1→2 | 23.077 | 9.216 | clingo |
| `two_qbf_tseitin.lp` — two_qbf_tseitin 2→3 1→3 | 24.589 | 10.775 | clingo |
| `two_qbf_tseitin.lp` — two_qbf_tseitin 1→4 | 23.066 | 9.255 | clingo |
| `two_qbf_pigeonhole.lp` — two_qbf_pigeonhole 1→0 | 15.369 | 9.241 | clingo |
| `two_qbf_pigeonhole.lp` — two_qbf_pigeonhole 1→2 | 17.154 | 9.269 | clingo |
| `two_qbf_pigeonhole.lp` — two_qbf_pigeonhole 2→3 1→3 | 18.692 | 9.269 | clingo |
| `two_qbf_pigeonhole.lp` — two_qbf_pigeonhole 1→4 4→3 | 9.281 | 7.765 | clingo |
| `two_qbf_parity.lp` — two_qbf_parity 6→10 | 27.726 | 10.758 | clingo |
| `two_qbf_parity.lp` — two_qbf_parity 6→14 | 39.920 | 16.759 | clingo |
| `two_qbf_parity.lp` — two_qbf_parity 6→16 | 50.281 | 19.830 | clingo |
| `two_qbf_tseitin.lp` — two_qbf_tseitin 12→18 | 30.835 | 12.257 | clingo |
| `two_qbf_tseitin.lp` — two_qbf_tseitin 12→24 | 39.090 | 12.259 | clingo |
| `two_qbf_tseitin.lp` — two_qbf_tseitin 12→30 | 52.205 | 15.212 | clingo |
| `two_qbf_pigeonhole.lp` — two_qbf_pigeonhole 4→5 | 29.631 | 10.766 | clingo |
| `two_qbf_pigeonhole.lp` — two_qbf_pigeonhole 4→6 | 52.833 | 13.752 | clingo |
| `strategic_companies.lp` — strategic_companies 1→2 | 14.033 | 7.724 | clingo |
| `strategic_companies.lp` — strategic_companies 12→8 24→16 1→2 | 9.419 | 7.691 | clingo |
| `strategic_companies.lp` — strategic_companies 12→10 24→20 1→3 | 10.789 | 7.735 | clingo |
| `strategic_companies.lp` — strategic_companies 12→16 24→32 | 29.699 | 9.227 | clingo |
| `strategic_companies.lp` — strategic_companies 12→20 24→40 | 68.866 | 12.426 | clingo |
| `parity_tseitin.lp` — parity_tseitin 5→2 | 4.826 | 7.699 | zetesis |
| `parity_tseitin.lp` — parity_tseitin 5→3 1→0 | 4.699 | 7.684 | zetesis |
| `parity_tseitin.lp` — parity_tseitin 1→2 | 6.331 | 7.727 | zetesis |
| `parity_chain.lp` — parity_chain 5→2 | 4.684 | 6.139 | zetesis |
| `parity_chain.lp` — parity_chain 5→3 1→0 | 4.735 | 6.141 | zetesis |
| `parity_chain.lp` — parity_chain 1→2 | 4.686 | 7.688 | zetesis |
| `pigeonhole_clauses.lp` — pigeonhole_clauses 4→3 | 4.834 | 6.235 | zetesis |
| `pigeonhole_clauses.lp` — pigeonhole_clauses 4→3 1→0 | 4.703 | 6.164 | zetesis |
| `pigeonhole_clauses.lp` — pigeonhole_clauses 4→2 | 4.775 | 6.229 | zetesis |
| `parity_mixed.lp` — parity_mixed 8→0 | 6.221 | 7.713 | zetesis |
| `parity_mixed.lp` — parity_mixed 1→2 | 6.278 | 7.700 | zetesis |
| `parity_mixed.lp` — parity_mixed 5→3 8→4 | 6.232 | 7.704 | zetesis |
| `parity_mixed.lp` — parity_mixed 8→20 | 7.849 | 7.705 | close |
| `standalone/n-queens/variant-01.lp` — n-queens/variant-01 8→4 | 4.700 | 6.135 | zetesis |
| `standalone/n-queens/variant-01.lp` — n-queens/variant-01 8→5 | 4.791 | 6.144 | zetesis |
| `standalone/n-queens/variant-01.lp` — n-queens/variant-01 8→6 | 4.688 | 7.684 | zetesis |
| `standalone/n-queens/variant-03.lp` — n-queens/variant-03 8→4 | 4.765 | 6.196 | zetesis |
| `standalone/n-queens/variant-03.lp` — n-queens/variant-03 8→5 | 4.708 | 7.671 | zetesis |
| `standalone/n-queens/variant-03.lp` — n-queens/variant-03 8→6 | 4.708 | 6.186 | zetesis |
| `standalone/n-queens/variant-02.lp` — n-queens/variant-02 8→4 | 4.697 | 6.136 | zetesis |
| `standalone/n-queens/variant-02.lp` — n-queens/variant-02 8→5 | 4.717 | 6.131 | zetesis |
| `standalone/n-queens/variant-04.lp` — n-queens/variant-04 8→4 | 4.694 | 7.668 | zetesis |
| `standalone/n-queens/variant-04.lp` — n-queens/variant-04 8→5 | 4.791 | 6.210 | zetesis |
| `standalone/n-queens/variant-05.lp` — n-queens/variant-05 8→4 | 4.683 | 6.157 | zetesis |
| `standalone/n-queens/variant-05.lp` — n-queens/variant-05 8→5 | 4.723 | 6.133 | zetesis |
| `standalone/n-queens/variant-06.lp` — n-queens/variant-06 8→4 | 4.700 | 6.143 | zetesis |
| `standalone/n-queens/variant-06.lp` — n-queens/variant-06 8→5 | 4.747 | 6.140 | zetesis |
| `generated/monotone-choices-6.lp` — monotone-choices-6 | 4.707 | 6.143 | zetesis |
| `generated/monotone-choices-8.lp` — monotone-choices-8 | 4.812 | 6.175 | zetesis |
| `generated/monotone-choices-10.lp` — monotone-choices-10 | 4.706 | 6.169 | zetesis |
| `generated/redundant-transitivity-8.lp` — redundant-transitivity-8 | 4.754 | 6.183 | zetesis |
| `generated/redundant-transitivity-12.lp` — redundant-transitivity-12 | 4.656 | 6.158 | zetesis |
| `generated/redundant-transitivity-16.lp` — redundant-transitivity-16 | 6.258 | 6.160 | close |

### scalability21-w14

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `scalability/n-queens.lp` — scalability/n-queens | 6.334 | 7.728 | zetesis |
| `scalability/n-queens.lp` — scalability/n-queens 8→9 | 7.877 | 10.770 | zetesis |
| `scalability/n-queens.lp` — scalability/n-queens 8→10 | 12.897 | 30.321 | zetesis |
| `scalability/pigeonhole.lp` — scalability/pigeonhole 7→5 | 4.773 | 7.700 | zetesis |
| `scalability/pigeonhole.lp` — scalability/pigeonhole 7→6 | 4.763 | 7.729 | zetesis |
| `scalability/pigeonhole.lp` — scalability/pigeonhole | 4.704 | 19.745 | zetesis |
| `scalability/mastermind.lp` — scalability/mastermind 6→5 | 12.399 | 10.818 | clingo |
| `scalability/mastermind.lp` — scalability/mastermind | 38.405 | 33.967 | clingo |
| `scalability/mastermind.lp` — scalability/mastermind 6→8 | 253.546 | 198.357 | clingo |
| `scalability/mastermind-nested.lp` — scalability/mastermind-nested | 12.445 | 10.801 | clingo |
| `sudoku.lp` — sudoku | 39.585 | 16.788 | clingo |
| `scalability/prime-sieve.lp` — scalability/prime-sieve | 6.369 | 7.770 | zetesis |
| `scalability/prime-sieve.lp` — scalability/prime-sieve 30→100 | 6.336 | 7.712 | zetesis |
| `scalability/prime-sieve.lp` — scalability/prime-sieve 30→300 | 7.893 | 9.309 | zetesis |
| `scalability/subset-sum.lp` — scalability/subset-sum | 4.731 | 6.161 | zetesis |
| `scalability/subset-sum.lp` — scalability/subset-sum 12→20 | 4.775 | 6.157 | zetesis |
| `scalability/subset-sum.lp` — scalability/subset-sum 12→30 | 6.237 | 7.744 | zetesis |
| `einstein-riddle.lp` — einstein-riddle | 15.460 | 37.855 | zetesis |

### root6-w14

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `alternatives.lp` — alternatives | 3.247 | 6.166 | zetesis |
| `choices.lp` — choices | 3.149 | 6.152 | zetesis |
| `lazy-reachability.lp` — lazy-reachability | 4.795 | 6.199 | zetesis |
| `network-repair.lp` — network-repair | 4.684 | 6.131 | zetesis |
| `reachability.lp` — reachability | 3.232 | 6.186 | zetesis |
| `unsatisfiable.lp` — unsatisfiable | 3.160 | 6.141 | zetesis |

### series22-w14

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `generated/independent-choice-12.lp` — independent-choice-12 | 6.321 | 10.997 | zetesis |
| `generated/independent-choice-16.lp` — independent-choice-16 | 21.791 | 39.058 | zetesis |
| `generated/independent-negation-8.lp` — independent-negation-8 | 4.779 | 7.772 | zetesis |
| `generated/independent-negation-10.lp` — independent-negation-10 | 4.682 | 7.702 | zetesis |
| `generated/independent-negation-aggregate-16.lp` — independent-negation-aggregate-16 | 26.572 | 44.962 | zetesis |
| `generated/disjunction-12.lp` — disjunction-12 | 24.803 | 34.450 | zetesis |
| `generated/ties-50.lp` — ties-50 | 20.372 | 24.817 | zetesis |
| `generated/transitive-path-100.lp` — transitive-path-100 | 39.770 | 10.832 | clingo |
| `generated/transitive-path-200.lp` — transitive-path-200 | 165.441 | 21.481 | clingo |
| `generated/transitive-dense-40.lp` — transitive-dense-40 | 24.668 | 9.261 | clingo |
| `generated/chain-1000.lp` — chain-1000 | 21.390 | 9.235 | clingo |
| `generated/chain-2000.lp` — chain-2000 | 38.135 | 12.345 | clingo |
| `generated/chain-arithmetic-1000.lp` — chain-arithmetic-1000 | 13.922 | 7.781 | clingo |
| `generated/stratified-16.lp` — stratified-16 | 6.297 | 7.710 | zetesis |
| `generated/producer-chain-700.lp` — producer-chain-700 | 28.981 | 12.228 | clingo |
| `generated/latin-square-5.lp` — latin-square-5 | 15.627 | 23.336 | zetesis |
| `generated/planning-14.lp` — planning-14 | 33.884 | 19.922 | clingo |
| `standalone/n-queens/variant-01.lp` — n-queens/variant-01 8→10 | 15.020 | 31.977 | zetesis |
| `standalone/n-queens/variant-01.lp` — n-queens/variant-01 8→11 | 35.444 | 188.744 | zetesis |
| `standalone/n-queens/variant-04.lp` — n-queens/variant-04 8→11 | 28.637 | 182.849 | zetesis |

### ar51-w14

| Input / amendment | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | --- |
| `materialized/tseitin-p17-odd/program.lp` — tseitin-p17-odd/program | 6.276 | 10.777 | zetesis |
| `materialized/chain-p17-odd/program.lp` — chain-p17-odd/program | 6.193 | 9.228 | zetesis |
| `materialized/tseitin-p29-odd/program.lp` — tseitin-p29-odd/program | 9.466 | 191.379 | zetesis |
| `materialized/chain-p29-odd/program.lp` — chain-p29-odd/program | 7.719 | 84.590 | zetesis |
| `materialized/tseitin-p41-odd/program.lp` — tseitin-p41-odd/program | 10.953 | 1,777.607 | zetesis |
| `materialized/chain-p41-odd/program.lp` — chain-p41-odd/program | 10.773 | 3,504.134 | zetesis |
| `materialized/tseitin-p47-odd/program.lp` — tseitin-p47-odd/program | 12.534 | 27,405.090 | zetesis |
| `materialized/chain-p47-odd/program.lp` — chain-p47-odd/program | 10.766 | 55,249.514 | zetesis |
| `materialized/tseitin-p53-odd/program.lp` — tseitin-p53-odd/program | — | — | reference unavailable |
| `materialized/chain-p53-odd/program.lp` — chain-p53-odd/program | — | — | reference unavailable |
| `materialized/tseitin-p59-odd/program.lp` — tseitin-p59-odd/program | — | — | reference unavailable |
| `materialized/chain-p59-odd/program.lp` — chain-p59-odd/program | — | — | reference unavailable |
| `materialized/tseitin-p71-odd/program.lp` — tseitin-p71-odd/program | — | — | reference unavailable |
| `materialized/chain-p71-odd/program.lp` — chain-p71-odd/program | — | — | reference unavailable |
| `materialized/tseitin-p83-odd/program.lp` — tseitin-p83-odd/program | — | — | reference unavailable |
| `materialized/chain-p83-odd/program.lp` — chain-p83-odd/program | — | — | reference unavailable |
| `materialized/tseitin-p89-odd/program.lp` — tseitin-p89-odd/program | — | — | reference unavailable |
| `materialized/chain-p89-odd/program.lp` — chain-p89-odd/program | — | — | reference unavailable |
| `materialized/tseitin-p101-odd/program.lp` — tseitin-p101-odd/program | — | — | reference unavailable |
| `materialized/chain-p101-odd/program.lp` — chain-p101-odd/program | — | — | reference unavailable |
| `materialized/pigeon-clauses-h8-extra1/program.lp` — pigeon-clauses-h8-extra1/program | 4.858 | 89.357 | zetesis |
| `materialized/pigeon-clauses-h9-extra1/program.lp` — pigeon-clauses-h9-extra1/program | 4.681 | 967.791 | zetesis |
| `materialized/pigeon-clauses-h10-extra1/program.lp` — pigeon-clauses-h10-extra1/program | 6.431 | 15,693.459 | zetesis |
| `materialized/pigeon-clauses-h11-extra1/program.lp` — pigeon-clauses-h11-extra1/program | — | — | reference unavailable |
| `materialized/pigeon-clauses-h12-extra1/program.lp` — pigeon-clauses-h12-extra1/program | — | — | reference unavailable |
| `materialized/pigeon-choice-h8/program.lp` — pigeon-choice-h8/program | 4.738 | 137.331 | zetesis |
| `materialized/pigeon-choice-h9/program.lp` — pigeon-choice-h9/program | 6.410 | 686.115 | zetesis |
| `materialized/pigeon-choice-h10/program.lp` — pigeon-choice-h10/program | 4.720 | 9,563.898 | zetesis |
| `materialized/pigeon-choice-h11/program.lp` — pigeon-choice-h11/program | — | — | reference unavailable |
| `materialized/pigeon-choice-h12/program.lp` — pigeon-choice-h12/program | — | — | reference unavailable |
| `materialized/mixed-p11-c40/program.lp` — mixed-p11-c40/program | 9.549 | 9.451 | close |
| `materialized/mixed-p17-c70/program.lp` — mixed-p17-c70/program | 16.992 | 9.306 | clingo |
| `materialized/mixed-p29-c130/program.lp` — mixed-p29-c130/program | 20.838 | 12.197 | clingo |
| `materialized/mixed-p41-c190/program.lp` — mixed-p41-c190/program | 29.393 | 40.983 | zetesis |
| `materialized/tseitin-p5-even/program.lp` — tseitin-p5-even/program | 6.324 | 7.701 | zetesis |
| `materialized/chain-p5-even/program.lp` — chain-p5-even/program | 6.234 | 7.692 | zetesis |
| `materialized/pigeon-clauses-h4-extra0/program.lp` — pigeon-clauses-h4-extra0/program | 4.795 | 6.185 | zetesis |
| `materialized/pigeon-clauses-h5-extra0/program.lp` — pigeon-clauses-h5-extra0/program | 4.690 | 6.156 | zetesis |
| `materialized/pigeon-clauses-h6-extra0/program.lp` — pigeon-clauses-h6-extra0/program | 9.324 | 9.254 | close |
| `ground-door/tseitin-20.lp` — ground-door/tseitin-20 | 6.277 | 7.689 | zetesis |
| `ground-door/xorchain-12.lp` — ground-door/xorchain-12 | 6.325 | 7.719 | zetesis |
| `ground-door/door-sizes/tseitin-30.lp` — door-sizes/tseitin-30 | 6.187 | 9.215 | zetesis |
| `ground-door/door-sizes/tseitin-40.lp` — door-sizes/tseitin-40 | 7.786 | 9.213 | zetesis |
| `ground-door/door-sizes/tseitin-50.lp` — door-sizes/tseitin-50 | 9.250 | 21.276 | zetesis |
| `ground-door/door-sizes/tseitin-60.lp` — door-sizes/tseitin-60 | 9.297 | 57.483 | zetesis |
| `ground-door/door-sizes/tseitin-80.lp` — door-sizes/tseitin-80 | 12.255 | 1,106.194 | zetesis |
| `ground-door/door-sizes/xorchain-16.lp` — door-sizes/xorchain-16 | 7.882 | 10.892 | zetesis |
| `ground-door/door-sizes/xorchain-20.lp` — door-sizes/xorchain-20 | 7.729 | 13.762 | zetesis |
| `ground-door/door-sizes/xorchain-24.lp` — door-sizes/xorchain-24 | 7.840 | 36.385 | zetesis |
| `ground-door/door-sizes/xorchain-32.lp` — door-sizes/xorchain-32 | 10.726 | 67.997 | zetesis |
| `ground-door/door-sizes/xorchain-40.lp` — door-sizes/xorchain-40 | 12.546 | 4,676.604 | zetesis |
