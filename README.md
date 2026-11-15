# zetesis

ζήτησις, *inquiry* — an answer-set solver in Rust.

[![CI](https://github.com/GregoryGelfond/zetesis/actions/workflows/checks.yml/badge.svg?branch=main)](https://github.com/GregoryGelfond/zetesis/actions/workflows/checks.yml)
[![Latest source release](https://img.shields.io/github/v/release/GregoryGelfond/zetesis?label=source&style=flat-square)](https://github.com/GregoryGelfond/zetesis/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
![Rust 1.97+](https://img.shields.io/badge/rust-1.97%2B-orange?style=flat-square)
[![Line coverage: 92.80% (portable)](https://img.shields.io/badge/coverage-92.80%25%20%28portable%29-brightgreen?style=flat-square)](docs/book/reference/coverage-0.8.0.md)

**[Read the zetesis Book](https://gregorygelfond.github.io/zetesis/book/)**

zetesis is an experimental answer-set solver written in Rust. It finds solutions
to logic programs and checks them against the program's reduct. It supports
lazy grounding, parallel CPU execution and optional GPU computation.

**Latest CPU scorecard: 214 zetesis wins, 9 close and 76 clingo wins** across
299 complete paired comparisons; 16 further workloads have unavailable
reference timings. The [full 315-workload report](docs/book/reference/cpu-0.8.0.md)
covers 14 zetesis workers against one clingo worker, with every result and
reproduction details.

zetesis uses [themelios](https://github.com/GregoryGelfond/themelios) for parsing,
program representation and analysis. Its construction, solve and query APIs are
available through the [zetesis Rust library](https://gregorygelfond.github.io/zetesis/book/rust/agent.html).

Its [shared execution algebra](docs/book/architecture/tour.md#where-the-machine-enters)
names the reusable operations: `Region` and `Walk` cover interpretations,
`Words` supplies packed operations, `Reduction` combines values, and `Meter`
accounts for resources. Candidate search and each candidate's frozen-reduct
check use separate knowledge. These execution choices preserve the reduct
as the definition of answer-set membership; see the
[architecture](docs/book/architecture/execution.md) and
[library map](docs/book/rust/libraries.md).

## Install and run

Clone the source, then install the pinned toolchain and executables:

```sh
git clone https://github.com/GregoryGelfond/zetesis.git
cd zetesis
rustup toolchain install 1.97.1 --profile minimal
./scripts/install.sh
export PATH="$HOME/.local/bin:$PATH"
zetesis --help
```

The installer builds release executables and places them in `~/.local/bin`, with
the GPU-capable build of zetesis in a `zetesis-libexec` directory beside them;
the `zetesis` you run starts CPU runs without the GPU stack and hands GPU
requests to it. Pass a directory to `scripts/install.sh` to choose another
location. The initial build needs GitHub access for the pinned themelios dependency; no sibling
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

Execution defaults to CPU. For formula
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

The **0.8.0 candidate** was compared with clingo on **315 distinct workloads**
on an Apple M4 Pro: **14 CPU workers for zetesis, one worker for clingo 5.8.2**.
The full selection includes planning, Poker, USA, wide Mastermind, quantified
programs and arithmetic, as well as the regular 95-case suite.

| Complete paired comparisons | Zetesis wins | Close | clingo wins | Unavailable pairs |
| ---: | ---: | ---: | ---: | ---: |
| 299 | **214** | **9** | **76** | **16** |

The score uses the zetesis/clingo elapsed-time ratio: below 0.97 is a zetesis
win, above 1.03 is a clingo win, and both boundaries count as close. These are single timed observations after complete-family
qualification, so close is a descriptive band, not statistical equivalence.
Clingo's qualification timed out on the other 16 arithmetic inputs; zetesis
reported exhaustive UNSAT on each, but those cases receive no paired timing
win. The ordinary 95-case panel alone had 93 wins, one close and one loss.

| Selected complete workload | Zetesis | clingo |
| --- | ---: | ---: |
| [Wide Mastermind](examples/wide-mastermind/68/mm-68-examples.lp), 45,832 answers | 0.958 s | 22.715 s |
| [Queens](examples/scalability/n-queens.lp), n=12, 14,200 answers | 0.125 s | 1.689 s |
| Family Crossing, vanilla B, eight optimal ties | 0.383 s | 11.388 s |
| [Integrated Family Crossing](examples/family-crossing/integrated.lp), eight optimal ties | 1.958 s | 0.889 s |
| [Draw Poker](examples/scalability/poker/draw_poker_tests.lp), 36 answers | 0.355 s | 0.107 s |
| USA SP1, 32 optimal ties | 1.280 s | 0.499 s |
| Reported five-crosser planner, complete optimal enumeration | 22.881 s | 2.680 s |
| [Quantified parity](examples/scalability/two-qbf-parity/two_qbf_parity.lp), size 28 | 125.45 ms | 215.49 ms |
| [Quantified pigeonhole](examples/scalability/two-qbf-pigeonhole/two_qbf_pigeonhole.lp), eight holes | 201.72 ms | 79.95 ms |
| Parity chain, p47, UNSAT | 10.77 ms | 55,249.51 ms |
| Mixed parity and clauses, p41/c190, 12 answers | 29.39 ms | 40.98 ms |
| [Prime sieve](examples/scalability/prime-sieve.lp), n=300 | 7.89 ms | 9.31 ms |
| [Subset sum](examples/scalability/subset-sum.lp), n=30, 296 answers | 6.24 ms | 7.74 ms |

Wide Mastermind enumerates 45,832 answers **23.7× faster**, and the structured
parity-chain contradiction finishes **over 5,000× faster**, in this comparison.
These show promising applications in large answer-set enumeration and
structured arithmetic reasoning. Clingo retains a substantial lead on several
planning and quantified workloads; the table includes those gaps. These are
CPU measurements of particular workloads, not a general GPU or complexity-class
performance claim.

Times include startup, grounding, solving and captured output. Read the
[full 315-workload comparison](docs/book/reference/cpu-0.8.0.md) for every
result, exact input identities, limits and reproduction commands. It preserves
all losses and incomplete comparisons. The [0.7.0 record](docs/book/reference/cpu-0.7.0.md)
used a different population, longer deadlines and three timing repetitions;
its headline counts are not a matched speedup baseline. Earlier
[CPU and Metal measurements](docs/book/reference/worker-scaling.md) retain
their own qualification scope.

To measure your installation with the maintained benchmark tool, run from
this checkout with clingo on `PATH`:

```sh
zetesis-bench run --threads 14 --report cpu.json
zetesis-bench compare before=before.json after=cpu.json
```

The default command selects the 95-case suite; the full scorecard combines
that suite with the other declared panels. The
[benchmark guide](docs/book/reference/benchmarking.md) explains selections,
complete-answer qualification and saved reports.

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
