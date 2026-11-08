# CPU comparisons for the 0.7.0 candidate

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

On 7 November 2026 we compared the 0.7.0 candidate with clingo on CPU. All **299 distinct inputs** have a recorded disposition. **286** yielded complete paired timings. Zetesis had the lower median on **200**, clingo on **86** and the medians were equal on **0**. With the 3% band, that gives **200 zetesis wins, 11 close and 75 clingo wins**. The remaining **13** have incomplete comparisons.

The original schedule had 304 positions. Five repeat inputs from the corpus occur in the scalability or series panels. Each has one vote in its original corpus position. The evidence retains every scheduled observation, including repeated selections and any later recovery of an arithmetic case interrupted solely by the campaign scheduling deadline. The [per-case table](observations/cpu-0.7.0-process.tsv) gives the measured outcomes by solver. The [receipt](observations/cpu-0.7.0-process.json) records source identities, effective edits, report hashes, timings and memory observations.

| Panel | Distinct | Comparable | zetesis wins | Close | clingo wins | Incomplete |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Regular corpus | 95 | 95 | 87 | 7 | 1 | 0 |
| Larger quantified programs | 5 | 5 | 0 | 0 | 5 | 0 |
| Public examples | 12 | 12 | 5 | 1 | 6 | 0 |
| USA and queens n12 | 2 | 2 | 1 | 0 | 1 | 0 |
| Family Crossing variants | 2 | 2 | 1 | 0 | 1 | 0 |
| Family Crossing components | 2 | 2 | 1 | 0 | 1 | 0 |
| Scalability | 18 | 18 | 13 | 0 | 5 | 0 |
| Reported programs | 3 | 3 | 1 | 0 | 2 | 0 |
| Root examples | 6 | 6 | 6 | 0 | 0 | 0 |
| Series | 20 | 20 | 12 | 0 | 8 | 0 |
| Quantified programs and their cores | 18 | 18 | 9 | 0 | 9 | 0 |
| Amended and generated inputs | 65 | 65 | 32 | 2 | 31 | 0 |
| Arithmetic controls | 51 | 38 | 32 | 1 | 5 | 13 |
| **Total** | 299 | 286 | 200 | 11 | 75 | 13 |

Across the 51 arithmetic inputs, both solvers completed qualification on **38**, only zetesis on **13**, only clingo on **0**, and neither on **0**. Zetesis proved UNSAT on **13** inputs where the other solver did not finish qualification. On `tseitin-p101-odd`, zetesis proved UNSAT in 23.07 ms in its qualification run. Clingo reached the 600 s limit. Zetesis recorded the contradiction in its root parity check.

Both solvers proved the corresponding p47 inputs unsatisfiable. Their three-run medians were 13.05 ms versus 27.43 s on the direct encoding and 12.49 ms versus 55.51 s on the chain encoding. On the 59-input chain the medians were 14.04 ms for zetesis and 358.76 s for clingo. The root cardinality check proved the 11-hole capacity instance inconsistent. Its medians were 6.44 ms versus 482.29 s. On the 82-vertex parity graph with 190 additional three-literal clauses, both solvers enumerated 12 answers. Zetesis took 364.42 s and clingo took 42.63 ms.

The solvers’ families were cross-checked against each other on 38 arithmetic inputs. A one-sided UNSAT result means the solver exited normally, reported an exhaustive search with zero models, and agreed with the input's expected outcome. The other solver supplied no confirming family. A solver that finished qualification in the original window retains that result when a recovery supplies timings. Each incomplete row in the README identifies the solver that hit a deadline or did not run. Timing comparisons require all twelve scheduled observations.

The full schedule includes 40 recovery positions and 4,128 process slots. Only inputs that passed all twelve observations, including paired complete counts and costs, receive a timing vote. For those inputs combined, the sum of case medians was 384.558 s for zetesis and 1,265.999 s for clingo. The sum weights each input by elapsed time. The win column weights each comparable input once. A zetesis/clingo ratio below 0.97 favors zetesis, and a ratio above 1.03 favors clingo. Both boundary values count as close. Scores use unrounded nanoseconds.

## Examples, optima and size

| Input | Answers | Optimal cost | zetesis (ms) | clingo (ms) |
| --- | ---: | ---: | ---: | ---: |
| Mastermind, original wide 68 | 45,832 | — | 1,126.39 | 22,601.73 |
| Mastermind, scalability 8 colors | 45,832 | — | 256.90 | 199.10 |
| Mastermind, scalability 6 colors | 6,080 | — | 39.90 | 34.05 |
| Mastermind, scalability 5 colors | 976 | — | 12.40 | 10.84 |
| Mastermind, nested | 1 | — | 18.53 | 9.23 |
| Poker | 36 | — | 468.43 | 107.25 |
| USA SP1 | 32 | [4] | 1,726.26 | 496.23 |
| Family Crossing, integrated | 8 | [29] | 5,065.80 | 894.05 |
| Family Crossing, vanilla B | 8 | [7, 29] | 743.91 | 11,414.40 |
| Family Crossing, observation A | 1 | — | 195.56 | 15.38 |
| Two-QBF parity 20 | 1 | — | 313.64 | 28.94 |
| Two-QBF parity 24 | 1 | — | 461.02 | 50.15 |
| Two-QBF parity 28 | 1 | — | 658.25 | 215.87 |
| Two-QBF pigeonhole 7 | 1 | — | 438.88 | 25.97 |
| Two-QBF pigeonhole 8 | 1 | — | 1,059.40 | 81.61 |
| Prime sieve 300 | 1 | — | 7.74 | 9.21 |
| Subset sum 30 | 296 | — | 6.24 | 7.72 |
| Reported `s_crossing.lp` | 4 | — | 621.41 | 26.84 |
| Reported `sudoku_simple.lp` | 1 | — | 88.47 | 1,874.19 |
| Reported `elias_pr_simp.lp` | 1 | — | 199.01 | 79.61 |
| Tseitin parity, p47 | 0 | — | 13.05 | 27,428.05 |
| Parity chain, p47 | 0 | — | 12.49 | 55,512.35 |
| Parity chain, p59 | 0 | — | 14.04 | 358,759.61 |
| Pigeonhole clauses, 11 holes | 0 | — | 6.44 | 482,292.78 |
| Mixed parity and clauses, p41/c190 | 12 | — | 364,423.83 | 42.63 |

Clingo took 20.1 times as long on original wide Mastermind and 15.3 times
as long on Family Crossing vanilla B. Clingo led by a factor of 5.7 on
integrated Family Crossing and was faster on all five larger quantified inputs.
In the standalone component panel the domain returned UNSAT, and EEC returned
one answer. They are separate component checks.

## What was measured

The source commit was `878f08509852654a27e3cb6158e746c9d39a1a1d` (tree
`d37a7b6d8cf33c2d13ccdc5513ce6ae80deb31ca`). It used the release profile
with fat LTO and one codegen unit, Rust 1.97.1 on an Apple M4 Pro. The
GPU-capable build of zetesis ran with the CPU backend, 14 workers, automatic
grounding and automatic checking. Clingo 5.8.2 used one worker in every
phase. Admission required AC power and one- and five-minute loads below four.

Each input scheduled a qualification, a warmup, three timed fresh processes
and a separate RSS sample per solver. Process time starts before spawning the
child and ends after reaping it. It includes input, grounding, solving and
captured JSON, with statistics enabled for zetesis. Each process had the same
600-second outer limit. Qualification compared complete selected answer sets,
including multiplicities and all final optimal ties and costs. The native
reports also matched full atoms across repetitions. Some quantified references
show only projected atoms; their cross-solver checks use that displayed family.

The [0.6.0 one- and fourteen-worker corpus timings](cpu-corpus-final-20261014.md)
and [earlier CPU and Metal measurements](worker-scaling.md) identify their own
sources, methods and results.

## Earlier single-worker instruction comparison

A separate study compared zetesis at `878f08509` to the integrated baseline
`440525ee917a2a51b15ba394c35bc105b1c11824` using the original compiler
profile. It measured retired instructions on one CPU worker against the maximum
of five baseline readings per selection. Of the 95 regular inputs, **93**
exceeded that maximum. The median per-case difference was **+5.26%**. Complete
integrated Family Crossing was **+3.36%**, pigeonhole 8's `programs/php-8`
was **+126.28%**, and `programs/xorchain-20` was **−99.73%**. All 113 complete
answer families matched their historical references. The additional one-answer
Family Crossing probe returned a member of the eight proved optima at cost 29.

Gregory accepted the per-case exceptions for 0.7.0 and retained the migration
for its shared algebra. The [114 instruction observations](observations/cpu-0.7.0-instructions.csv)
record both the baseline ranges and candidate readings.

## Reproduce the CPU panels

Start with the measured commit and build the workspace's release binaries in
one invocation. Use an absolute path to clingo 5.8.2.

```sh
cargo build --locked --release --all-features -p zetesis-cli -p zetesis-bench
ROOT=$(pwd -P)
NATIVE="$ROOT/target/release/zetesis"
BENCH="$ROOT/target/release/zetesis-bench"
CLINGO=$(command -v clingo)
OUT=$(mktemp -d)
MANIFESTS="$ROOT/docs/book/reference/observations/cpu-0.7.0-manifests"
run_panel() {
  label=$1
  seconds=$2
  shift 2
  "$BENCH" run "$ROOT/examples/correctness" "$@" \
    --zetesis "$NATIVE" --clingo "$CLINGO" --native-interface solve \
    --backend cpu --grounder auto --oracle auto --threads 14 --clingo-threads 1 \
    --warmups 1 --repetitions 3 --memory-runs 1 --timeout-seconds 600 \
    --campaign-seconds "$seconds" --sample-bytes 134217728 --answer-bytes 134217728 \
    --native-report-bytes 134217728 --native-atoms 4194304 --native-value-nodes 8388608 \
    --capture-bytes 2147483648 --report-bytes 4294967296 \
    --report "$OUT/$label.json" --json --color never > "$OUT/$label.summary.json"
}
run_panel corpus95 7200 --suite corpus
run_panel scalability21 7200 --suite scalability --examples "$ROOT/examples" --include-einstein
```

The measurement used the same 0.7.0 examples and validation trees as the
recorded inputs. The observation manifests first appear with this report.
Point `MANIFESTS` to that directory when running an older checkout. The
[public supplement](observations/cpu-0.7.0-manifests/public-supplement.json)
selects files from the measured examples directory, including Poker's driver
and original Mastermind's includes. The
[standalone components](observations/cpu-0.7.0-manifests/family-components.json)
selects the two tracked Family Crossing components. Their manifests resolve
their input roots relative to the manifest location. Supply the measured
examples tree at those roots.

```sh
run_panel public-supplement 7200 --workloads "$MANIFESTS/public-supplement.json"
run_panel family-components 7200 --workloads "$MANIFESTS/family-components.json"
```

The remaining manifests have root `.`. Copy each manifest beside its input
files, retaining its relative entry names and exact SHA-256 digests.

- [Larger quantified inputs](observations/cpu-0.7.0-manifests/sigma-larger5.json).
  For `parity20.lp`, `parity24.lp` and `parity28.lp`, copy
  `examples/scalability/two-qbf-parity/two_qbf_parity.lp`, changing only
  `#const n = 6.` to the named size. For `pigeonhole7.lp` and
  `pigeonhole8.lp`, copy `examples/scalability/two-qbf-pigeonhole/two_qbf_pigeonhole.lp`,
  changing only `#const holes = 4.` to 7 or 8.
- [USA and queens](observations/cpu-0.7.0-manifests/usa-plus-queens.json).
  This panel requires an external `usa/usaSP1.lp` matching the manifest.
  Copy `examples/scalability/n-queens.lp` to `queens-n12.lp` and change
  `#const n = 8.` to 12.
- [Family Crossing variants](observations/cpu-0.7.0-manifests/family-variants.json).
  These require the external vanilla B and observation A entry files matching
  the manifest. Copy the two included components unchanged from
  `examples/family-crossing/` into the same directory.

For example, with the corresponding input directories in `SIGMA`, `USA`
and `FAMILY`, run the three external selections as follows.

```sh
run_panel sigma-larger5 900 --workloads "$SIGMA/sigma-larger5.json"
run_panel usa-plus-queens 7200 --workloads "$USA/usa-plus-queens.json"
run_panel family-variants 7200 --workloads "$FAMILY/family-variants.json"
```


The [reported programs](observations/cpu-0.7.0-manifests/reported-workloads.json),
[quantified programs and cores](observations/cpu-0.7.0-manifests/inner-heavy18.json) and
[arithmetic programs](observations/cpu-0.7.0-manifests/ar51.json) use external
input files. Copy each manifest beside its listed sources and match their verified
hashes and relative names. The [six root examples](observations/cpu-0.7.0-manifests/root6.json)
are tracked under `examples/`. Use the same `run_panel` settings, with the
respective input directories in `REPORTED`, `INNER` and `AR`.

```sh
run_panel reported-workloads 7200 --workloads "$REPORTED/reported-workloads.json"
run_panel root6 7200 --workloads "$MANIFESTS/root6.json"
run_panel series22 7200 --suite series
run_panel inner-heavy18 7200 --workloads "$INNER/inner-heavy18.json"
run_panel ar51 7200 --workloads "$AR/ar51.json"
```

The 65 amended and generated workloads used the maintained Rust performance
matrix directly. The receipt lists each case's expected contract. Recreate the
ordered inputs with the `Workload` constructors using the listed base file and exact byte edits or the generator family, size
and content digest. Submit the sequence with
`zetesis_validation::performance::matrix::run_workloads_with_invocation`
and the `Solve` invocation. The six generated cases
come from the `monotone-choices` and `redundant-transitivity` families.
Each selection uses the existing validation workload and matrix APIs. The
typed invocation recorded in the receipt identifies the measured local
driver by its source and executable SHA-256. It used the same solver paths, one warmup, three timed runs, one RSS
run, reference policy, byte limits and child deadline as the CLI campaigns.

The arithmetic panel allows one recovery for each input whose original pair
lacked the required measurements solely because the schedule reached its deadline.
An already-comparable result or a genuine solver timeout is ineligible. Copy
the unchanged matching case into its own manifest at the same root and run
the same solver pair, limits and phases. The public receipt retains both
records and identifies which one represents the input in the score.
