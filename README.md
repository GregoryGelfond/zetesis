# zetesis

ζήτησις, *inquiry* — an answer-set solver in Rust.

[![CI](https://github.com/GregoryGelfond/zetesis/actions/workflows/checks.yml/badge.svg?branch=main)](https://github.com/GregoryGelfond/zetesis/actions/workflows/checks.yml)
[![Source release: v0.6.0](https://img.shields.io/badge/source-v0.6.0-blue?style=flat-square)](https://github.com/GregoryGelfond/zetesis/releases/tag/v0.6.0)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
![Rust 1.97+](https://img.shields.io/badge/rust-1.97%2B-orange?style=flat-square)
[![Line coverage: 93.18% (portable)](https://img.shields.io/badge/coverage-93.18%25%20%28portable%29-brightgreen?style=flat-square)](https://gregorygelfond.github.io/zetesis/book/reference/coverage-0.6.0.html)

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

Measurements of the 0.6.0 candidate on an Apple M4 Pro compare zetesis with
**14 threads** against clingo 5.8.2 with **one thread**, enumerating all answers
or all tied optima. Automatic grounding was eager on these cases. Times are
medians of three runs, including startup, input, grounding, solving and captured
output; zetesis statistics are enabled.

The examples below cover all six corpus families and all six queens encodings.
“Close” means within 3%; wins and losses use the same descriptive band.

| Workload | zetesis (ms) | clingo (ms) | vs clingo |
| --- | ---: | ---: | --- |
| [Equality/generalized TSP, basic](examples/correctness/scenarios/equality-generalized-tsp/01-basic.lp) | 4.829 | 4.726 | Close |
| [Shortest path, basic](examples/correctness/scenarios/shortest-path/variant-01/01-basic.lp) | 4.779 | 4.675 | Close |
| [Shortest path, layered with order and budget](examples/correctness/scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp) | 6.331 | 6.217 | Close |
| [Task allocation, larger mix](examples/correctness/scenarios/task-allocation/variant-04/05-larger-mix.lp) | 28.263 | 176.552 | zetesis 6.25× faster |
| [Traveling salesman, three depots](examples/correctness/scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp) | 4.724 | 4.682 | Close |
| [Traveling salesman, windows and revisits](examples/correctness/scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp) | 6.293 | 6.179 | Close |
| [Eight queens, variant 1](examples/correctness/standalone/n-queens/variant-01.lp) | 6.247 | 6.209 | Close |
| [Eight queens, variant 2](examples/correctness/standalone/n-queens/variant-02.lp) | 25.926 | 122.510 | zetesis 4.73× faster |
| [Eight queens, variant 3](examples/correctness/standalone/n-queens/variant-03.lp) | 6.415 | 6.255 | Close |
| [Eight queens, variant 4](examples/correctness/standalone/n-queens/variant-04.lp) | 6.377 | 6.219 | Close |
| [Eight queens, variant 5](examples/correctness/standalone/n-queens/variant-05.lp) | 7.870 | 6.237 | clingo 1.26× faster |
| [Eight queens, variant 6](examples/correctness/standalone/n-queens/variant-06.lp) | 7.897 | 6.263 | clingo 1.26× faster |
| [SEND + MORE = MONEY](examples/correctness/standalone/send-money/send-money.lp) | 10.949 | 12.257 | zetesis 1.12× faster |
| **All 94 programs: sum of per-case medians** | **520.523** | **803.673** | **zetesis 1.54× faster on the sum** |

Across all 94 cases, zetesis is faster on **16**, within 3% on **65**, and
slower on **13**. All completed and agreed on displayed answers and costs,
including optimal ties. The summed time is about **33% below** the recorded
0.5.0 result of 777.447 ms; that baseline comes from an earlier campaign, not
a simultaneous comparison. The sum is not one timed combined run.
The [complete final results](https://gregorygelfond.github.io/zetesis/book/reference/cpu-corpus-final-20261014.html)
include every case, one- and fourteen-thread profiles, memory, identities,
reproduction commands and the earlier comparison. GPU execution and clingo's
parallel modes are outside this comparison.

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
