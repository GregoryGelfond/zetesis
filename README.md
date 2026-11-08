# zetesis

ζήτησις, *inquiry* — an answer-set solver in Rust.

[![CI](https://github.com/GregoryGelfond/zetesis/actions/workflows/checks.yml/badge.svg?branch=main)](https://github.com/GregoryGelfond/zetesis/actions/workflows/checks.yml)
[![Source release: v0.7.0](https://img.shields.io/badge/source-v0.7.0-blue?style=flat-square)](https://github.com/GregoryGelfond/zetesis/releases/tag/v0.7.0)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
![Rust 1.97+](https://img.shields.io/badge/rust-1.97%2B-orange?style=flat-square)
[![Line coverage: 92.64% (portable)](https://img.shields.io/badge/coverage-92.64%25%20%28portable%29-brightgreen?style=flat-square)](docs/book/reference/coverage-0.7.0.md)

**[Read the zetesis Book](https://gregorygelfond.github.io/zetesis/book/)**

zetesis is an experimental answer-set solver written in Rust. It finds solutions
to logic programs and checks them against the program's reduct. It supports
lazy grounding, parallel CPU execution and optional GPU computation.

zetesis uses [themelios](https://github.com/GregoryGelfond/themelios) for parsing,
program representation and analysis. Its construction, solve and query APIs are
available through the [zetesis Rust library](https://gregorygelfond.github.io/zetesis/book/rust/agent.html).

## Install and run

From a checkout:

```sh
rustup toolchain install 1.97.1 --profile minimal
./scripts/install.sh
export PATH="$HOME/.local/bin:$PATH"
zetesis --help
```

The installer builds release executables and places them in `~/.local/bin`, with
the GPU-capable build of zetesis in a `zetesis-libexec` directory beside them;
the `zetesis` you run starts CPU runs without the GPU stack and hands GPU
requests to it. Pass a directory to `scripts/install.sh` to choose another
location. The initial
build needs GitHub read access for the pinned themelios dependency; no sibling
checkout is required. Installed commands do not need Rust or Cargo at runtime.
[INSTALL.md](INSTALL.md) describes the installed tools, the CPU-only build,
installing with Cargo and checking a build.

## First program

Find a cheapest path through a graph. This is a self-contained form of the
[basic shortest-path example](examples/correctness/scenarios/shortest-path/variant-01/01-basic.lp)
from the correctness corpus, using its [shared encoding](examples/correctness/encodings/shortest-path/variant-01.lp).
Save it as `shortest-path.lp`:

```clingo
vertex(a; b; c).
edge(a,b,1; b,c,1; a,c,3).
start(a). end(c).

{ included(Src,Dst,C) : edge(Src,Dst,C) }.
:- vertex(V), #count{ Dst,C : included(V,Dst,C) } > 1.
:- vertex(V), #count{ Src,C : included(Src,V,C) } > 1.
:- included(_,V,_), start(V).
:- included(V,_,_), end(V).

reachable(Src) :- start(Src).
reachable(Dst) :- reachable(Src), included(Src,Dst,_), vertex(Src), vertex(Dst).
:- end(Src), not reachable(Src).
:- included(Src,_,_), not reachable(Src).

cost(X) :- X = #sum{ C,Src,Dst : included(Src,Dst,C) }.
#minimize{ C@2 : cost(C) }.
#minimize{ 1@1,Src,Dst,C : included(Src,Dst,C) }.

#show included/3.
#show start/1.
#show end/1.
```

```sh
zetesis solve shortest-path.lp --all
```

The only optimal displayed answer is
`end(c) included(a,b,1) included(b,c,1) start(a)`; atom order may differ.
The route costs 2 and uses two edges. The direct edge costs 3, so fewer edges
do not outweigh the primary cost objective. With objectives, `--all` returns
all tied optima; without it, the default displays one optimum.

## Common options

Everyday options select answers, output and resources. Use `--help` for the
common controls and `--help-all` for advanced execution options.

| Option | Purpose |
| --- | --- |
| `--all` | Find every answer, or every tied optimum for a program with objectives. |
| `--count` | Print only the complete number of answer sets, or all optimal ties. |
| `--answers N` | Select up to N answers. |
| `--backend cpu\|gpu` | Choose CPU (the default) or GPU execution. |
| `--threads N` | Set the host thread count; the default uses available parallelism. |
| `--grounder eager\|lazy` | Request a grounding mode. |
| `--json` | Write machine-readable results to stdout. |
| `--stats` | Write statistics to stderr. |
| `--time-limit 60s` | Set a cooperative deadline. |
| `--memory 4GiB` | Set the allowance for named storage capacities. |

`zetesis solve program.lp --count` prints one integer after exhaustive search.
It counts full answer sets independently of `#show` and `#project`; interrupted
search publishes no partial total. It cannot be combined with `--answers`,
`--all`, `--models` or `--json`; `--stats` still writes to stderr.

Execution defaults to CPU and uses lazy grounding where supported. For formula
programs, automatic grounding can defer eligible definitions until their answers
are known; other rules are grounded eagerly. Explicit lazy execution can stream
eligible constraints while checking their formula core on the selected CPU or
GPU. Source joins and streamed constraint checks run on the host. See the
[grounding profiles](https://gregorygelfond.github.io/zetesis/book/architecture/grounding.html#eager-and-lazy-execution)
for the current limits. Use `zetesis devices` to list GPU devices and
`--backend gpu` to run on the GPU: Metal on macOS, Vulkan on Linux.

A stopped search is incomplete; it does not prove unsatisfiability or optimality.
The time and memory options are not hard process-time or RSS caps.
Human output shows the answers, result, model count and basic grounding/solving
times. `--stats` adds detailed tables; diagnostics go to stderr.
See `zetesis help solve`, or `zetesis solve --help-all` for all execution and
output options.

## Performance at a glance

The 0.7.0 candidate at `878f08509` used **14 CPU workers** on an Apple M4 Pro. Clingo 5.8.2 used **one worker**. All **299 distinct inputs** have a recorded disposition. **286** yielded complete paired timings. Zetesis had the lower median on **200**, clingo on **86** and the medians were equal on **0**. With the 3% band, that gives **200 zetesis wins, 11 close and 75 clingo wins**. The remaining **13** have incomplete comparisons.

The regular 95-case corpus had 87 zetesis wins, seven close and one clingo win. Original wide Mastermind returned 45,832 answers in 1.13 s for zetesis and 22.60 s for clingo. Clingo led on integrated Family Crossing, and zetesis led on vanilla B. Both programs returned all eight optimal ties.

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

Across the 51 arithmetic inputs, both solvers completed qualification on **38**, only zetesis on **13**, only clingo on **0**, and neither on **0**. Zetesis proved UNSAT on **13** inputs where the other solver did not finish qualification. On `tseitin-p101-odd`, zetesis proved UNSAT in 23.07 ms in its qualification run. Clingo reached the 600 s limit.

### Selected complete cases

| Selected input | Complete answers | zetesis (ms) | clingo (ms) |
| --- | ---: | ---: | ---: |
| [Equality/generalized TSP, basic](examples/correctness/scenarios/equality-generalized-tsp/01-basic.lp) | 1 optimum | 6.41 | 9.42 |
| [Shortest path, basic](examples/correctness/scenarios/shortest-path/variant-01/01-basic.lp) | 1 optimum | 4.69 | 7.69 |
| [Task allocation, larger mix](examples/correctness/scenarios/task-allocation/variant-04/05-larger-mix.lp) | 1,176 optima | 27.97 | 200.87 |
| [Traveling salesman, windows and revisits](examples/correctness/scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp) | 1 optimum | 7.73 | 7.70 |
| [Eight queens, variant 2](examples/correctness/standalone/n-queens/variant-02.lp) | 92 | 26.67 | 125.35 |
| [SEND + MORE = MONEY](examples/correctness/standalone/send-money/send-money.lp) | 1 | 12.39 | 15.25 |
| [Original wide Mastermind](examples/wide-mastermind/68/mm-68-examples.lp) | 45,832 | 1,126.39 | 22,601.73 |
| [Scalability Mastermind, 8 colors](examples/scalability/mastermind.lp) | 45,832 | 256.90 | 199.10 |
| [Draw Poker](examples/scalability/poker/draw_poker_tests.lp) | 36 | 468.43 | 107.25 |
| USA SP1 | 32 optima | 1,726.26 | 496.23 |
| [Integrated Family Crossing](examples/family-crossing/integrated.lp) | 8 optima | 5,065.80 | 894.05 |
| Family Crossing, vanilla B | 8 optima | 743.91 | 11,414.40 |
| Family Crossing, observation A | 1 | 195.56 | 15.38 |
| [Quantified parity, size 28](examples/scalability/two-qbf-parity/two_qbf_parity.lp) | 1 | 658.25 | 215.87 |
| [Quantified pigeonhole, 8 holes](examples/scalability/two-qbf-pigeonhole/two_qbf_pigeonhole.lp) | 1 | 1,059.40 | 81.61 |
| [Prime sieve, 300](examples/scalability/prime-sieve.lp) | 1 | 7.74 | 9.21 |
| [Subset sum, 30](examples/scalability/subset-sum.lp) | 296 | 6.24 | 7.72 |
| Reported `s_crossing.lp` | 4 | 621.41 | 26.84 |
| Reported `sudoku_simple.lp` | 1 | 88.47 | 1,874.19 |
| Reported `elias_pr_simp.lp` | 1 | 199.01 | 79.61 |
| Tseitin parity, p47 | 0 | 13.05 | 27,428.05 |
| Parity chain, p47 | 0 | 12.49 | 55,512.35 |
| Parity chain, p59 | 0 | 14.04 | 358,759.61 |
| Pigeonhole clauses, 11 holes | 0 | 6.44 | 482,292.78 |
| Mixed parity and clauses, p41/c190 | 12 | 364,423.83 | 42.63 |

Times include startup, grounding, solving and captured output. The [CPU comparison](docs/book/reference/cpu-0.7.0.md) describes the three-run process medians, clingo qualification and workload identities. The [0.6.0 one- and fourteen-worker timings](docs/book/reference/cpu-corpus-final-20261014.md) and [earlier Metal measurements](docs/book/reference/worker-scaling.md) identify their own sources.

### All 299 inputs

Open a panel for every distinct input and its process medians in milliseconds. Incomplete rows give the qualification outcome for each solver. The [per-case table](docs/book/reference/observations/cpu-0.7.0-process.tsv) also retains repeated selections, recovery records and nanosecond timings.

<details>
<summary>Regular corpus (95 inputs)</summary>

| Input | Answers | Cost | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| `scenarios/equality-generalized-tsp/01-basic.lp` | 1 | [3] | 6.41 | 9.42 | Zetesis |
| `scenarios/equality-generalized-tsp/02-larger.lp` | 1 | [4] | 6.27 | 7.70 | Zetesis |
| `scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp` | 0 | — | 4.76 | 7.73 | Zetesis |
| `scenarios/shortest-path/variant-01/01-basic.lp` | 1 | [2, 2] | 4.69 | 7.69 | Zetesis |
| `scenarios/shortest-path/variant-01/02-start-equals-end.lp` | 1 | [0, 0] | 4.81 | 7.71 | Zetesis |
| `scenarios/shortest-path/variant-01/03-zero-cost-detour.lp` | 1 | [2, 1] | 4.71 | 7.72 | Zetesis |
| `scenarios/shortest-path/variant-01/04-no-path.lp` | 0 | — | 4.79 | 7.70 | Zetesis |
| `scenarios/shortest-path/variant-01/05-multi-path.lp` | 1 | [3, 3] | 4.68 | 7.70 | Zetesis |
| `scenarios/shortest-path/variant-01/06-layered-dag.lp` | 1 | [4, 4] | 7.87 | 7.72 | close |
| `scenarios/shortest-path/variant-01/07-cycles.lp` | 1 | [3, 3] | 6.24 | 7.72 | Zetesis |
| `scenarios/shortest-path/variant-01/08-negative-weights.lp` | 1 | [2, 2] | 4.78 | 7.72 | Zetesis |
| `scenarios/shortest-path/variant-02/01-basic.lp` | 1 | [2, 2] | 6.20 | 7.74 | Zetesis |
| `scenarios/shortest-path/variant-02/02-start-equals-end.lp` | 1 | [0, 0] | 4.82 | 7.71 | Zetesis |
| `scenarios/shortest-path/variant-02/03-before-forces-detour.lp` | 1 | [4, 3] | 4.81 | 7.72 | Zetesis |
| `scenarios/shortest-path/variant-02/04-after-forces-extension.lp` | 1 | [3, 3] | 4.83 | 7.72 | Zetesis |
| `scenarios/shortest-path/variant-02/05-ordering-unsat.lp` | 0 | — | 4.81 | 7.73 | Zetesis |
| `scenarios/shortest-path/variant-02/06-layered-dag-before.lp` | 2 | [7, 4] | 7.84 | 7.71 | close |
| `scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp` | 1 | [6, 4] | 7.78 | 7.68 | close |
| `scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp` | 1 | [3, 2] | 4.79 | 7.68 | Zetesis |
| `scenarios/shortest-path/variant-02/09-negative-weights.lp` | 1 | [2, 2] | 4.71 | 7.70 | Zetesis |
| `scenarios/shortest-path/variant-03/01-basic.lp` | 1 | [2, 2] | 4.81 | 7.70 | Zetesis |
| `scenarios/shortest-path/variant-03/02-start-equals-end.lp` | 1 | [0, 0] | 4.72 | 7.72 | Zetesis |
| `scenarios/shortest-path/variant-03/03-budget-forces-detour.lp` | 1 | [4, 2] | 4.80 | 7.75 | Zetesis |
| `scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp` | 1 | [2, 2] | 4.76 | 7.71 | Zetesis |
| `scenarios/shortest-path/variant-03/05-budget-unsat.lp` | 0 | — | 4.82 | 7.71 | Zetesis |
| `scenarios/shortest-path/variant-03/06-layered-dag-cap.lp` | 2 | [7, 4] | 7.76 | 9.25 | Zetesis |
| `scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp` | 4 | [12, 4] | 7.80 | 9.28 | Zetesis |
| `scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp` | 1 | [3, 2] | 4.72 | 7.72 | Zetesis |
| `scenarios/shortest-path/variant-03/09-negative-weights.lp` | 1 | [2, 2] | 4.82 | 7.72 | Zetesis |
| `scenarios/shortest-path/variant-04/01-basic.lp` | 1 | [2, 2] | 4.70 | 7.71 | Zetesis |
| `scenarios/shortest-path/variant-04/02-start-equals-end.lp` | 1 | [0, 0] | 4.80 | 7.74 | Zetesis |
| `scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp` | 1 | [4, 3] | 4.70 | 7.72 | Zetesis |
| `scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp` | 0 | — | 6.34 | 7.73 | Zetesis |
| `scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp` | 1 | [6, 2] | 6.22 | 7.70 | Zetesis |
| `scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp` | 2 | [8, 4] | 7.84 | 9.24 | Zetesis |
| `scenarios/shortest-path/variant-04/07-layered-dag-combined.lp` | 1 | [10, 4] | 7.75 | 9.21 | Zetesis |
| `scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp` | 1 | [4, 2] | 6.36 | 7.71 | Zetesis |
| `scenarios/shortest-path/variant-04/09-negative-weights.lp` | 1 | [2, 2] | 4.68 | 7.73 | Zetesis |
| `scenarios/task-allocation/variant-01/01-basic.lp` | 1 | [5] | 4.74 | 6.20 | Zetesis |
| `scenarios/task-allocation/variant-01/02-agent-reuse.lp` | 1 | [6] | 4.75 | 6.17 | Zetesis |
| `scenarios/task-allocation/variant-01/03-selective-compatibility.lp` | 1 | [10] | 4.79 | 6.17 | Zetesis |
| `scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp` | 0 | — | 4.72 | 6.16 | Zetesis |
| `scenarios/task-allocation/variant-01/05-larger-mix.lp` | 1 | [9] | 4.75 | 6.17 | Zetesis |
| `scenarios/task-allocation/variant-02/01-basic.lp` | 1 | [5, 2] | 4.68 | 7.68 | Zetesis |
| `scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp` | 3 | [3, 5] | 4.76 | 7.68 | Zetesis |
| `scenarios/task-allocation/variant-02/03-cost-dominates.lp` | 1 | [3, 15] | 4.70 | 7.71 | Zetesis |
| `scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp` | 0 | — | 4.70 | 6.20 | Zetesis |
| `scenarios/task-allocation/variant-02/05-larger-mix.lp` | 1 | [9, 7] | 6.28 | 7.68 | Zetesis |
| `scenarios/task-allocation/variant-03/01-basic.lp` | 2 | [6] | 4.79 | 6.17 | Zetesis |
| `scenarios/task-allocation/variant-03/02-multiple-groups.lp` | 4 | [12] | 4.74 | 7.70 | Zetesis |
| `scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp` | 2 | [8] | 4.75 | 6.20 | Zetesis |
| `scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp` | 0 | — | 4.71 | 6.17 | Zetesis |
| `scenarios/task-allocation/variant-03/05-larger-mix.lp` | 1 | [6] | 4.71 | 6.16 | Zetesis |
| `scenarios/task-allocation/variant-04/01-basic.lp` | 81 | [2] | 7.77 | 25.80 | Zetesis |
| `scenarios/task-allocation/variant-04/02-precedence.lp` | 3 | [2] | 6.34 | 9.21 | Zetesis |
| `scenarios/task-allocation/variant-04/03-agent-serialization.lp` | 30 | [2] | 7.74 | 27.30 | Zetesis |
| `scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp` | 0 | — | 6.26 | 7.72 | Zetesis |
| `scenarios/task-allocation/variant-04/05-larger-mix.lp` | 1176 | [5] | 27.97 | 200.87 | Zetesis |
| `scenarios/traveling-salesman/variant-01/01-basic.lp` | 1 | [3] | 4.85 | 7.78 | Zetesis |
| `scenarios/traveling-salesman/variant-01/02-multiple-tours.lp` | 2 | [8] | 6.25 | 7.72 | Zetesis |
| `scenarios/traveling-salesman/variant-01/03-asymmetric.lp` | 1 | [4] | 6.32 | 7.72 | Zetesis |
| `scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp` | 0 | — | 4.71 | 6.21 | Zetesis |
| `scenarios/traveling-salesman/variant-01/05-ring.lp` | 2 | [5] | 6.25 | 7.70 | Zetesis |
| `scenarios/traveling-salesman/variant-02/01-basic.lp` | 2 | [5] | 6.24 | 7.69 | Zetesis |
| `scenarios/traveling-salesman/variant-02/02-single-salesman.lp` | 1 | [3] | 4.77 | 7.69 | Zetesis |
| `scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp` | 0 | — | 4.72 | 7.70 | Zetesis |
| `scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp` | 2 | [4] | 4.79 | 7.74 | Zetesis |
| `scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp` | 2 | [7] | 4.71 | 7.71 | Zetesis |
| `scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp` | 2 | [6] | 6.29 | 7.70 | Zetesis |
| `scenarios/traveling-salesman/variant-03/01-basic.lp` | 1 | [4] | 4.69 | 7.74 | Zetesis |
| `scenarios/traveling-salesman/variant-03/02-single-salesman.lp` | 1 | [3] | 4.73 | 7.68 | Zetesis |
| `scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp` | 0 | — | 4.70 | 7.69 | Zetesis |
| `scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp` | 2 | [4] | 4.83 | 7.74 | Zetesis |
| `scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp` | 1 | [9] | 6.22 | 7.71 | Zetesis |
| `scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp` | 1 | [6] | 4.76 | 7.72 | Zetesis |
| `scenarios/traveling-salesman/variant-04/01-basic.lp` | 1 | [4] | 6.23 | 7.70 | Zetesis |
| `scenarios/traveling-salesman/variant-04/02-single-salesman.lp` | 1 | [4] | 6.29 | 7.72 | Zetesis |
| `scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp` | 0 | — | 6.21 | 7.72 | Zetesis |
| `scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp` | 0 | — | 4.80 | 7.74 | Zetesis |
| `scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp` | 1 | [6] | 7.73 | 7.71 | close |
| `scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp` | 1 | [6] | 7.74 | 7.72 | close |
| `scenarios/traveling-salesman/variant-05/01-basic.lp` | 1 | [4] | 6.23 | 7.70 | Zetesis |
| `scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp` | 1 | [3] | 6.26 | 7.71 | Zetesis |
| `scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp` | 0 | — | 6.21 | 7.70 | Zetesis |
| `scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp` | 1 | [3] | 6.35 | 7.75 | Zetesis |
| `scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp` | 1 | [6] | 7.73 | 7.70 | close |
| `scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp` | 1 | [6] | 6.32 | 7.70 | Zetesis |
| `standalone/n-queens/variant-01.lp` | 92 | — | 6.47 | 7.73 | Zetesis |
| `standalone/n-queens/variant-02.lp` | 92 | — | 26.67 | 125.35 | Zetesis |
| `standalone/n-queens/variant-03.lp` | 92 | — | 7.83 | 9.26 | Zetesis |
| `standalone/n-queens/variant-04.lp` | 92 | — | 6.32 | 9.23 | Zetesis |
| `standalone/n-queens/variant-05.lp` | 92 | — | 7.80 | 9.26 | Zetesis |
| `standalone/n-queens/variant-06.lp` | 92 | — | 9.44 | 9.20 | close |
| `standalone/send-money/send-money.lp` | 1 | — | 12.39 | 15.25 | Zetesis |
| `family-crossing/integrated.lp` | 8 | [29] | 5,065.80 | 894.05 | clingo |

</details>

<details>
<summary>Larger quantified programs (5 inputs)</summary>

| Input | Answers | Cost | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| `parity20.lp` | 1 | — | 313.64 | 28.94 | clingo |
| `parity24.lp` | 1 | — | 461.02 | 50.15 | clingo |
| `parity28.lp` | 1 | — | 658.25 | 215.87 | clingo |
| `pigeonhole7.lp` | 1 | — | 438.88 | 25.97 | clingo |
| `pigeonhole8.lp` | 1 | — | 1,059.40 | 81.61 | clingo |

</details>

<details>
<summary>Public examples (12 inputs)</summary>

| Input | Answers | Cost | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| `scalability/parity-chain/parity_chain.lp` | 0 | — | 6.40 | 7.88 | Zetesis |
| `scalability/parity-mixed/parity_mixed.lp` | 21 | — | 7.77 | 7.73 | close |
| `scalability/parity-tseitin/parity_tseitin.lp` | 0 | — | 6.27 | 7.73 | Zetesis |
| `scalability/pigeonhole-clauses/pigeonhole_clauses.lp` | 0 | — | 4.74 | 6.18 | Zetesis |
| `scalability/poker/draw_poker_tests.lp` | 36 | — | 468.43 | 107.25 | clingo |
| `scalability/shifted-subset-sum/subset_sum.lp` | 0 | — | 4.94 | 7.76 | Zetesis |
| `scalability/strategic-companies/strategic_companies.lp` | 29 | — | 13.95 | 7.70 | clingo |
| `scalability/two-qbf/two_qbf.lp` | 2 | — | 46.43 | 10.88 | clingo |
| `scalability/two-qbf-parity/two_qbf_parity.lp` | 1 | — | 32.08 | 9.28 | clingo |
| `scalability/two-qbf-pigeonhole/two_qbf_pigeonhole.lp` | 1 | — | 27.78 | 9.34 | clingo |
| `scalability/two-qbf-tseitin/two_qbf_tseitin.lp` | 1 | — | 49.09 | 9.25 | clingo |
| `wide-mastermind/68/mm-68-examples.lp` | 45832 | — | 1,126.39 | 22,601.73 | Zetesis |

</details>

<details>
<summary>USA and queens n12 (2 inputs)</summary>

| Input | Answers | Cost | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| `usa/usaSP1.lp` | 32 | [4] | 1,726.26 | 496.23 | clingo |
| `queens-n12.lp` | 14200 | — | 137.55 | 1,653.66 | Zetesis |

</details>

<details>
<summary>Family Crossing variants (2 inputs)</summary>

| Input | Answers | Cost | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| `family_crossing_vanilla_mode_b.lp` | 8 | [7, 29] | 743.91 | 11,414.40 | Zetesis |
| `family_crossing_mode_a_observation.lp` | 1 | — | 195.56 | 15.38 | clingo |

</details>

<details>
<summary>Family Crossing components (2 inputs)</summary>

| Input | Answers | Cost | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| `family_crossing_complete_domain.lp` | 0 | — | 81.74 | 12.23 | clingo |
| `eec_htp_integration_v5.lp` | 1 | — | 7.86 | 9.23 | Zetesis |

</details>

<details>
<summary>Scalability (18 inputs)</summary>

| Input | Answers | Cost | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| `scalability/n-queens.lp` | 92 | — | 6.55 | 7.72 | Zetesis |
| `scalability/n-queens.lp` (8→9) | 352 | — | 7.93 | 10.75 | Zetesis |
| `scalability/n-queens.lp` (8→10) | 724 | — | 12.76 | 30.33 | Zetesis |
| `scalability/pigeonhole.lp` (7→5) | 0 | — | 4.83 | 7.71 | Zetesis |
| `scalability/pigeonhole.lp` (7→6) | 0 | — | 4.75 | 7.73 | Zetesis |
| `scalability/pigeonhole.lp` | 0 | — | 4.72 | 19.73 | Zetesis |
| `scalability/mastermind.lp` (6→5) | 976 | — | 12.40 | 10.84 | clingo |
| `scalability/mastermind.lp` | 6080 | — | 39.90 | 34.05 | clingo |
| `scalability/mastermind.lp` (6→8) | 45832 | — | 256.90 | 199.10 | clingo |
| `scalability/mastermind-nested.lp` | 1 | — | 18.53 | 9.23 | clingo |
| `sudoku.lp` | 1 | — | 41.16 | 15.29 | clingo |
| `scalability/prime-sieve.lp` | 1 | — | 4.87 | 6.20 | Zetesis |
| `scalability/prime-sieve.lp` (30→100) | 1 | — | 4.71 | 6.17 | Zetesis |
| `scalability/prime-sieve.lp` (30→300) | 1 | — | 7.74 | 9.21 | Zetesis |
| `scalability/subset-sum.lp` | 15 | — | 4.72 | 6.17 | Zetesis |
| `scalability/subset-sum.lp` (12→20) | 64 | — | 4.78 | 6.19 | Zetesis |
| `scalability/subset-sum.lp` (12→30) | 296 | — | 6.24 | 7.72 | Zetesis |
| `einstein-riddle.lp` | 1 | — | 17.07 | 37.87 | Zetesis |

</details>

<details>
<summary>Reported programs (3 inputs)</summary>

| Input | Answers | Cost | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| `s_crossing.lp` | 4 | — | 621.41 | 26.84 | clingo |
| `sudoku_simple.lp` | 1 | — | 88.47 | 1,874.19 | Zetesis |
| `elias_pr_simp.lp` | 1 | — | 199.01 | 79.61 | clingo |

</details>

<details>
<summary>Root examples (6 inputs)</summary>

| Input | Answers | Cost | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| `alternatives.lp` | 2 | — | 4.00 | 6.43 | Zetesis |
| `choices.lp` | 3 | — | 3.95 | 6.43 | Zetesis |
| `lazy-reachability.lp` | 2 | — | 4.00 | 6.43 | Zetesis |
| `network-repair.lp` | 2 | — | 3.95 | 6.45 | Zetesis |
| `reachability.lp` | 1 | — | 4.02 | 6.44 | Zetesis |
| `unsatisfiable.lp` | 0 | — | 3.95 | 6.44 | Zetesis |

</details>

<details>
<summary>Series (20 inputs)</summary>

| Input | Answers | Cost | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| `generated/independent-choice-12.lp` | 377 | — | 6.69 | 10.48 | Zetesis |
| `generated/independent-choice-16.lp` | 2584 | — | 19.39 | 33.94 | Zetesis |
| `generated/independent-negation-8.lp` | 55 | — | 5.33 | 6.45 | Zetesis |
| `generated/independent-negation-10.lp` | 144 | — | 5.23 | 7.72 | Zetesis |
| `generated/independent-negation-aggregate-16.lp` | 2584 | — | 21.98 | 38.97 | Zetesis |
| `generated/disjunction-12.lp` | 4096 | — | 23.22 | 29.91 | Zetesis |
| `generated/ties-50.lp` | 1225 | [2] | 19.58 | 22.05 | Zetesis |
| `generated/transitive-path-100.lp` | 1 | — | 54.58 | 10.41 | clingo |
| `generated/transitive-path-200.lp` | 1 | — | 264.79 | 20.46 | clingo |
| `generated/transitive-dense-40.lp` | 1 | — | 30.67 | 9.08 | clingo |
| `generated/chain-1000.lp` | 1 | — | 25.51 | 8.96 | clingo |
| `generated/chain-2000.lp` | 1 | — | 49.47 | 12.90 | clingo |
| `generated/chain-arithmetic-1000.lp` | 1 | — | 15.45 | 6.42 | clingo |
| `generated/stratified-16.lp` | 1 | — | 5.29 | 6.45 | Zetesis |
| `generated/producer-chain-700.lp` | 1 | — | 30.42 | 11.44 | clingo |
| `generated/latin-square-5.lp` | 1344 | — | 14.34 | 19.48 | Zetesis |
| `generated/planning-14.lp` | 3432 | — | 29.57 | 17.85 | clingo |
| `standalone/n-queens/variant-01.lp` (8→10) | 724 | — | 14.45 | 31.69 | Zetesis |
| `standalone/n-queens/variant-01.lp` (8→11) | 2680 | — | 33.97 | 190.05 | Zetesis |
| `standalone/n-queens/variant-04.lp` (8→11) | 2680 | — | 31.83 | 185.06 | Zetesis |

</details>

<details>
<summary>Quantified programs and their cores (18 inputs)</summary>

| Input | Answers | Cost | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| `programs/xorchain-14.lp` | 1 | — | 15.48 | 11.70 | clingo |
| `programs/xorchain-16.lp` | 1 | — | 16.62 | 14.04 | clingo |
| `programs/xorchain-18.lp` | 1 | — | 19.17 | 20.30 | Zetesis |
| `programs/xorchain-20.lp` | 1 | — | 20.63 | 24.12 | Zetesis |
| `programs/tseitin-24.lp` | 1 | — | 14.18 | 12.84 | clingo |
| `programs/tseitin-28.lp` | 1 | — | 15.40 | 12.78 | clingo |
| `programs/tseitin-30.lp` | 1 | — | 16.74 | 12.74 | clingo |
| `programs/php-7.lp` | 1 | — | 68.60 | 21.44 | clingo |
| `programs/php-8.lp` | 1 | — | 595.80 | 76.87 | clingo |
| `cores/xorchain-14.core.lp` | 0 | — | 7.92 | 8.99 | Zetesis |
| `cores/xorchain-16.core.lp` | 0 | — | 6.53 | 10.19 | Zetesis |
| `cores/xorchain-18.core.lp` | 0 | — | 7.73 | 10.23 | Zetesis |
| `cores/xorchain-20.core.lp` | 0 | — | 7.73 | 10.22 | Zetesis |
| `cores/tseitin-24.core.lp` | 0 | — | 6.55 | 7.70 | Zetesis |
| `cores/tseitin-28.core.lp` | 0 | — | 6.55 | 8.94 | Zetesis |
| `cores/tseitin-30.core.lp` | 0 | — | 6.45 | 8.98 | Zetesis |
| `cores/php-7.core.lp` | 0 | — | 56.16 | 17.75 | clingo |
| `cores/php-8.core.lp` | 0 | — | 388.60 | 116.88 | clingo |

</details>

<details>
<summary>Amended and generated inputs (65 inputs)</summary>

| Input | Answers | Cost | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| `subset_sum.lp` (17→19) | 1 | — | 4.03 | 6.45 | Zetesis |
| `subset_sum.lp` (8→30, 17→61) | 0 | — | 5.19 | 6.42 | Zetesis |
| `subset_sum.lp` (8→30, 17→63) | 1 | — | 5.29 | 6.46 | Zetesis |
| `two_qbf.lp` (68→52) | 0 | — | 36.34 | 10.31 | clingo |
| `two_qbf.lp` (1→2) | 4 | — | 47.30 | 8.96 | clingo |
| `two_qbf.lp` (3→2, 68→40) | 0 | — | 27.87 | 9.02 | clingo |
| `two_qbf.lp` (8→10, 68→85) | 2 | — | 71.34 | 11.46 | clingo |
| `two_qbf_parity.lp` (1→0) | 0 | — | 33.18 | 9.04 | clingo |
| `two_qbf_parity.lp` (1→2) | 2 | — | 31.76 | 8.95 | clingo |
| `two_qbf_parity.lp` (2→3, 1→3) | 3 | — | 37.85 | 10.32 | clingo |
| `two_qbf_parity.lp` (1→4, 6→5, 1→2) | 4 | — | 22.88 | 8.97 | clingo |
| `two_qbf_tseitin.lp` (1→0) | 0 | — | 49.74 | 10.31 | clingo |
| `two_qbf_tseitin.lp` (1→2) | 2 | — | 48.46 | 8.99 | clingo |
| `two_qbf_tseitin.lp` (2→3, 1→3) | 3 | — | 55.91 | 10.30 | clingo |
| `two_qbf_tseitin.lp` (1→4) | 4 | — | 45.66 | 10.19 | clingo |
| `two_qbf_pigeonhole.lp` (1→0) | 0 | — | 29.29 | 9.04 | clingo |
| `two_qbf_pigeonhole.lp` (1→2) | 2 | — | 28.18 | 8.94 | clingo |
| `two_qbf_pigeonhole.lp` (2→3, 1→3) | 3 | — | 33.94 | 10.03 | clingo |
| `two_qbf_pigeonhole.lp` (1→4, 4→3) | 4 | — | 12.78 | 9.01 | clingo |
| `two_qbf_parity.lp` (6→10) | 1 | — | 78.12 | 11.57 | clingo |
| `two_qbf_parity.lp` (6→14) | 1 | — | 151.16 | 16.51 | clingo |
| `two_qbf_parity.lp` (6→16) | 1 | — | 194.81 | 20.39 | clingo |
| `two_qbf_tseitin.lp` (12→18) | 1 | — | 81.59 | 12.79 | clingo |
| `two_qbf_tseitin.lp` (12→24) | 1 | — | 125.82 | 12.86 | clingo |
| `two_qbf_tseitin.lp` (12→30) | 1 | — | 181.37 | 15.22 | clingo |
| `two_qbf_pigeonhole.lp` (4→5) | 1 | — | 69.70 | 11.60 | clingo |
| `two_qbf_pigeonhole.lp` (4→6) | 1 | — | 182.70 | 14.04 | clingo |
| `strategic_companies.lp` (1→2) | 76 | — | 15.55 | 9.10 | clingo |
| `strategic_companies.lp` (12→8, 24→16, 1→2) | 13 | — | 10.48 | 7.75 | clingo |
| `strategic_companies.lp` (12→10, 24→20, 1→3) | 24 | — | 12.94 | 9.09 | clingo |
| `strategic_companies.lp` (12→16, 24→32) | 269 | — | 29.99 | 10.28 | clingo |
| `strategic_companies.lp` (12→20, 24→40) | 697 | — | 64.12 | 12.93 | clingo |
| `parity_tseitin.lp` (5→2) | 0 | — | 5.35 | 7.72 | Zetesis |
| `parity_tseitin.lp` (5→3, 1→0) | 16 | — | 5.24 | 7.73 | Zetesis |
| `parity_tseitin.lp` (1→2) | 0 | — | 5.26 | 6.41 | Zetesis |
| `parity_chain.lp` (5→2) | 0 | — | 5.21 | 6.41 | Zetesis |
| `parity_chain.lp` (5→3, 1→0) | 8 | — | 5.26 | 6.42 | Zetesis |
| `parity_chain.lp` (1→2) | 0 | — | 5.21 | 6.44 | Zetesis |
| `pigeonhole_clauses.lp` (4→3) | 0 | — | 3.99 | 6.43 | Zetesis |
| `pigeonhole_clauses.lp` (4→3, 1→0) | 6 | — | 3.94 | 6.43 | Zetesis |
| `pigeonhole_clauses.lp` (4→2) | 0 | — | 4.02 | 6.44 | Zetesis |
| `parity_mixed.lp` (8→0) | 64 | — | 6.48 | 7.69 | Zetesis |
| `parity_mixed.lp` (1→2) | 26 | — | 7.84 | 7.72 | close |
| `parity_mixed.lp` (5→3, 8→4) | 11 | — | 6.55 | 7.73 | Zetesis |
| `parity_mixed.lp` (8→20) | 8 | — | 7.77 | 7.72 | close |
| `standalone/n-queens/variant-01.lp` (8→4) | 2 | — | 5.24 | 6.42 | Zetesis |
| `standalone/n-queens/variant-01.lp` (8→5) | 10 | — | 5.31 | 6.43 | Zetesis |
| `standalone/n-queens/variant-01.lp` (8→6) | 4 | — | 5.32 | 6.45 | Zetesis |
| `standalone/n-queens/variant-03.lp` (8→4) | 2 | — | 5.35 | 6.44 | Zetesis |
| `standalone/n-queens/variant-03.lp` (8→5) | 10 | — | 5.26 | 6.43 | Zetesis |
| `standalone/n-queens/variant-03.lp` (8→6) | 4 | — | 5.31 | 7.70 | Zetesis |
| `standalone/n-queens/variant-02.lp` (8→4) | 2 | — | 5.24 | 6.42 | Zetesis |
| `standalone/n-queens/variant-02.lp` (8→5) | 10 | — | 5.23 | 7.75 | Zetesis |
| `standalone/n-queens/variant-04.lp` (8→4) | 2 | — | 5.32 | 6.45 | Zetesis |
| `standalone/n-queens/variant-04.lp` (8→5) | 10 | — | 5.26 | 6.43 | Zetesis |
| `standalone/n-queens/variant-05.lp` (8→4) | 2 | — | 5.27 | 6.43 | Zetesis |
| `standalone/n-queens/variant-05.lp` (8→5) | 10 | — | 5.26 | 7.31 | Zetesis |
| `standalone/n-queens/variant-06.lp` (8→4) | 2 | — | 5.31 | 6.46 | Zetesis |
| `standalone/n-queens/variant-06.lp` (8→5) | 10 | — | 5.30 | 7.71 | Zetesis |
| `generated/monotone-choices-6.lp` | 7 | — | 5.26 | 6.43 | Zetesis |
| `generated/monotone-choices-8.lp` | 9 | — | 5.16 | 6.42 | Zetesis |
| `generated/monotone-choices-10.lp` | 11 | — | 5.21 | 6.43 | Zetesis |
| `generated/redundant-transitivity-8.lp` | 2 | — | 6.21 | 6.44 | Zetesis |
| `generated/redundant-transitivity-12.lp` | 2 | — | 10.33 | 7.75 | clingo |
| `generated/redundant-transitivity-16.lp` | 2 | — | 15.37 | 6.44 | clingo |

</details>

<details>
<summary>Arithmetic controls (51 inputs)</summary>

| Input | Answers | Cost | zetesis (ms) | clingo (ms) | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| `tseitin-p53-odd` | — | — | — | — | zetesis proved UNSAT, clingo deadline · solver deadline |
| `chain-p53-odd` | — | — | — | — | zetesis proved UNSAT, clingo deadline · solver deadline |
| `tseitin-p59-odd` | — | — | — | — | zetesis proved UNSAT, clingo deadline · solver deadline |
| `tseitin-p71-odd` | — | — | — | — | zetesis proved UNSAT, clingo deadline · solver deadline |
| `chain-p71-odd` | — | — | — | — | zetesis proved UNSAT, clingo deadline · solver deadline |
| `tseitin-p83-odd` | — | — | — | — | zetesis proved UNSAT, clingo deadline · solver deadline |
| `chain-p83-odd` | — | — | — | — | zetesis proved UNSAT, clingo deadline · solver deadline |
| `tseitin-p89-odd` | — | — | — | — | zetesis proved UNSAT, clingo deadline · solver deadline |
| `chain-p89-odd` | — | — | — | — | zetesis proved UNSAT, clingo deadline · solver deadline |
| `tseitin-p101-odd` | — | — | — | — | zetesis proved UNSAT, clingo deadline · solver deadline |
| `chain-p101-odd` | — | — | — | — | zetesis proved UNSAT, clingo deadline · solver deadline |
| `tseitin-p17-odd` | 0 | — | 7.83 | 9.20 | Zetesis |
| `chain-p17-odd` | 0 | — | 6.25 | 9.21 | Zetesis |
| `tseitin-p29-odd` | 0 | — | 9.46 | 190.22 | Zetesis |
| `chain-p29-odd` | 0 | — | 9.37 | 83.19 | Zetesis |
| `tseitin-p41-odd` | 0 | — | 11.78 | 1,780.79 | Zetesis |
| `chain-p41-odd` | 0 | — | 11.72 | 3,506.06 | Zetesis |
| `tseitin-p47-odd` | 0 | — | 13.05 | 27,428.05 | Zetesis |
| `chain-p47-odd` | 0 | — | 12.49 | 55,512.35 | Zetesis |
| `chain-p59-odd` | 0 | — | 14.04 | 358,759.61 | Zetesis |
| `pigeon-clauses-h8-extra1` | 0 | — | 4.72 | 85.94 | Zetesis |
| `pigeon-clauses-h9-extra1` | 0 | — | 6.47 | 966.63 | Zetesis |
| `pigeon-clauses-h10-extra1` | 0 | — | 6.51 | 15,645.24 | Zetesis |
| `pigeon-clauses-h11-extra1` | 0 | — | 6.44 | 482,292.78 | Zetesis |
| `pigeon-clauses-h12-extra1` | — | — | — | — | zetesis proved UNSAT, clingo deadline · solver deadline |
| `pigeon-choice-h8` | 0 | — | 4.75 | 135.70 | Zetesis |
| `pigeon-choice-h9` | 0 | — | 4.80 | 682.05 | Zetesis |
| `pigeon-choice-h10` | 0 | — | 6.53 | 9,495.71 | Zetesis |
| `pigeon-choice-h11` | 0 | — | 6.48 | 260,562.36 | Zetesis |
| `pigeon-choice-h12` | — | — | — | — | zetesis proved UNSAT, clingo deadline · solver deadline |
| `mixed-p11-c40` | 19 | — | 10.79 | 7.69 | clingo |
| `mixed-p17-c70` | 86 | — | 27.57 | 9.33 | clingo |
| `mixed-p29-c130` | 24 | — | 1,152.15 | 13.86 | clingo |
| `mixed-p41-c190` | 12 | — | 364,423.83 | 42.63 | clingo |
| `tseitin-p5-even` | 64 | — | 6.27 | 7.72 | Zetesis |
| `chain-p5-even` | 32 | — | 6.21 | 6.15 | close |
| `pigeon-clauses-h4-extra0` | 24 | — | 4.73 | 6.17 | Zetesis |
| `pigeon-clauses-h5-extra0` | 120 | — | 4.71 | 6.16 | Zetesis |
| `pigeon-clauses-h6-extra0` | 720 | — | 14.16 | 9.09 | clingo |
| `ground-door/tseitin-20.lp` | 0 | — | 6.22 | 7.67 | Zetesis |
| `ground-door/xorchain-12.lp` | 0 | — | 6.29 | 7.71 | Zetesis |
| `ground-door/door-sizes/tseitin-30.lp` | 0 | — | 6.22 | 7.71 | Zetesis |
| `ground-door/door-sizes/tseitin-40.lp` | 0 | — | 7.74 | 9.20 | Zetesis |
| `ground-door/door-sizes/tseitin-50.lp` | 0 | — | 9.29 | 19.76 | Zetesis |
| `ground-door/door-sizes/tseitin-60.lp` | 0 | — | 9.26 | 55.87 | Zetesis |
| `ground-door/door-sizes/tseitin-80.lp` | 0 | — | 12.53 | 1,070.53 | Zetesis |
| `ground-door/door-sizes/xorchain-16.lp` | 0 | — | 6.26 | 9.21 | Zetesis |
| `ground-door/door-sizes/xorchain-20.lp` | 0 | — | 7.77 | 12.25 | Zetesis |
| `ground-door/door-sizes/xorchain-24.lp` | 0 | — | 9.36 | 34.79 | Zetesis |
| `ground-door/door-sizes/xorchain-32.lp` | 0 | — | 9.34 | 64.96 | Zetesis |
| `ground-door/door-sizes/xorchain-40.lp` | 0 | — | 12.54 | 4,574.60 | Zetesis |

</details>

## Use from Rust

Run the complete [Rust example](crates/zetesis-solve/examples/solve.rs) from the
checkout root:

```sh
cargo run --locked -p zetesis-solve --example solve --no-default-features
```

It prepares a bounded task-choice program, streams typed `AnswerSet` values,
distinguishes full interpretations from `#show`, and checks complete search.
The [library quickstart](https://gregorygelfond.github.io/zetesis/book/rust/getting-started.html) includes the full
program and dependency setup for your own application.
The `zetesis` facade provides CPU enumeration; the native session API also
supports GPU checking and optimization.

## Documentation

- [Command guide](https://gregorygelfond.github.io/zetesis/book/reference/commands.html): inputs, objectives, output,
  limits, testing and benchmarking.
- [Language reference](https://gregorygelfond.github.io/zetesis/book/reference/language.html): supported ASP constructs
  and arithmetic rules.
- [Rust quick start](https://gregorygelfond.github.io/zetesis/book/rust/getting-started.html): embed a solve in an application.
- [Solver architecture](https://gregorygelfond.github.io/zetesis/book/architecture/tour.html): grounding, candidate
  search and reduct checking.
- [Performance results](https://gregorygelfond.github.io/zetesis/book/reference/performance.html): measurements and their limits.
- [The zetesis Book](https://gregorygelfond.github.io/zetesis/book/): the complete manual, including the
  Lean proof library.

See [Contributing](CONTRIBUTING.md) for development and verification, and
[building the book](https://gregorygelfond.github.io/zetesis/book/building.html) for a local copy of the manual.

## Status and license

zetesis remains experimental. Unsupported language constructs and execution
combinations are reported explicitly; consult the language reference before
using it for a new workload. The Lean library proves stated semantic laws, not
end-to-end correctness of the implementation.

MIT. Copyright Gregory Gelfond. See [LICENSE](LICENSE).
