# zetesis

ζήτησις, *inquiry* — an answer-set solver in Rust.

[![CI](https://github.com/GregoryGelfond/zetesis/actions/workflows/checks.yml/badge.svg?branch=main)](https://github.com/GregoryGelfond/zetesis/actions/workflows/checks.yml)
[![Source release: v0.4.0](https://img.shields.io/badge/source-v0.4.0-blue?style=flat-square)](https://github.com/GregoryGelfond/zetesis/releases/tag/v0.4.0)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
![Rust 1.97+](https://img.shields.io/badge/rust-1.97%2B-orange?style=flat-square)
[![Line coverage: 92.68% (portable)](https://img.shields.io/badge/coverage-92.68%25%20%28portable%29-brightgreen?style=flat-square)](https://gregorygelfond.github.io/zetesis/book/reference/coverage-0.4.0.html)

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

The installer builds release executables and places them in `~/.local/bin`.
Pass a directory to `scripts/install.sh` to choose another location. The initial
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

| Option | Purpose |
| --- | --- |
| `--all` | Find every answer, or every tied optimum for a program with objectives. |
| `--answers N` | Select up to N answers. |
| `--threads N` | Set the host thread count. |
| `--grounder eager\|lazy` | Request a grounding mode. |
| `--json` | Write machine-readable results to stdout. |
| `--stats` | Write statistics to stderr. |
| `--time-limit 60s` | Set a cooperative deadline. |
| `--memory 4GiB` | Set the allowance for named storage capacities. |

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
See `zetesis help solve`, or add `--advanced` for all execution and output options.

## Performance at a glance

CPU measurements on an Apple M4 Pro compare zetesis
with **14 threads** against clingo 5.8.2 with
**one thread**. Automatic grounding was eager on these cases.
Both enumerate every answer or every tied optimum. Each value
is the median of three complete command-line runs, including startup, parsing,
grounding, solving and captured output; zetesis statistics are enabled.

| Workload | zetesis | clingo | Comparison |
| --- | ---: | ---: | --- |
| [Task allocation, larger instance](examples/correctness/scenarios/task-allocation/variant-04/05-larger-mix.lp) | 30.54 ms | 197.88 ms | zetesis 6.48× faster |
| [Eight queens, variant 2](examples/correctness/standalone/n-queens/variant-02.lp) | 29.42 ms | 128.31 ms | zetesis 4.36× faster |
| [SEND + MORE = MONEY](examples/correctness/standalone/send-money/send-money.lp) | 13.86 ms | 12.21 ms | clingo 1.14× faster |
| [Eight queens, variant 1](examples/correctness/standalone/n-queens/variant-01.lp) | 10.80 ms | 6.21 ms | clingo 1.74× faster |
| **All 94 programs: sum of per-case medians** | **755.324 ms** | **830.639 ms** | **zetesis 1.10× faster on the sum** |

zetesis is faster on **4 of 94 cases**; clingo is faster on 90. A few
substantial wins reduce the total; most cases favor clingo. The total sums
individual medians and is not one timed combined run. All cases completed and
agreed on shown answers and costs, including optimum ties.

These observations depend on the workload and machine; three repetitions do
not establish a general speedup. clingo's parallel modes and GPU performance
are outside this comparison. The [complete results](https://gregorygelfond.github.io/zetesis/book/reference/cpu-corpus-20261010.html)
retain all 94 cases, the measured binary/source identities, limits and
reproduction commands.

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
