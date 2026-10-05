# zetesis

ζήτησις, *inquiry* — an answer-set solver in Rust.

[![CI](https://github.com/GregoryGelfond/zetesis/actions/workflows/checks.yml/badge.svg?branch=main)](https://github.com/GregoryGelfond/zetesis/actions/workflows/checks.yml)
[![Source release: v0.3.0](https://img.shields.io/badge/source-v0.3.0-blue?style=flat-square)](https://github.com/GregoryGelfond/zetesis/releases/tag/v0.3.0)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
![Rust 1.97+](https://img.shields.io/badge/rust-1.97%2B-orange?style=flat-square)
[![Line coverage: 92.17% (portable)](https://img.shields.io/badge/coverage-92.17%25%20%28portable%29-brightgreen?style=flat-square)](https://gregorygelfond.github.io/zetesis/book/reference/validation.html#coverage)

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
| `--memory-budget 4GiB` | Set the allowance for supported storage limits. |

Execution defaults to CPU and uses lazy grounding where supported. For formula
programs, automatic grounding can defer eligible definitions until their answers
are known; other rules are grounded eagerly. Explicit lazy CPU execution can stream
eligible constraints while retaining their producer core. See the
[grounding profiles](https://gregorygelfond.github.io/zetesis/book/architecture/grounding.html#eager-and-lazy-execution)
for the current limits. Use `zetesis devices` to list GPU devices and
`--backend gpu` to run on the GPU: Metal on macOS, Vulkan on Linux.

A stopped search is incomplete; it does not prove unsatisfiability or optimality.
The time and memory options are not hard process-time or RSS caps.
Human output shows the answers, result, model count and basic grounding/solving
times. `--stats` adds detailed tables; diagnostics go to stderr.
See `zetesis help solve`, or add `--advanced` for resource controls.

## Performance at a glance

CPU measurements on an Apple M4 Pro, using default execution settings:
zetesis ([measured source](https://github.com/GregoryGelfond/zetesis/commit/120fadfb3744c00760bcefd575ce3641075341dc))
with **14 threads** on this host, and clingo 5.8.2 with **one thread**.
Both enumerate every answer or every tied optimum. Each time averages two
five-run medians of complete command-line runs, including startup, parsing,
grounding, solving and captured output; zetesis statistics are enabled.

| Workload | zetesis | clingo | Comparison |
| --- | ---: | ---: | --- |
| [Task allocation, larger instance](examples/correctness/scenarios/task-allocation/variant-04/05-larger-mix.lp) | 32.10 ms | 188.57 ms | zetesis 5.88× faster |
| [Eight queens, variant 2](examples/correctness/standalone/n-queens/variant-02.lp) | 33.88 ms | 123.64 ms | zetesis 3.65× faster |
| [SEND + MORE = MONEY](examples/correctness/standalone/send-money/send-money.lp) | 15.40 ms | 12.22 ms | clingo 1.26× faster |
| [Eight queens, variant 1](examples/correctness/standalone/n-queens/variant-01.lp) | 12.59 ms | 6.20 ms | clingo 2.03× faster |
| **All 94 programs: sum of paired per-case medians** | **823.37 ms** | **812.49 ms** | **clingo 1.01× faster** |

zetesis is faster on **4 of 94 cases**; clingo is faster on 90. The substantial
wins on a few cases bring the totals close. The total is a sum of individual
medians, not one timed combined run. All cases completed within default solver
budgets and agreed on shown answers and costs, including optimum ties.

These observations depend on the workload
and machine; they do not establish a general speedup. clingo's optional parallel
modes are outside this comparison. See the [complete results](https://gregorygelfond.github.io/zetesis/book/reference/foundation-reuse.html)
for all cases, memory, source identities and reproduction commands.

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
