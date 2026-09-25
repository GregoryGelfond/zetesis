# zetesis

[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
![Rust 1.97+](https://img.shields.io/badge/rust-1.97%2B-orange?style=flat-square)
[![Line coverage: 94.28% (CPU + Metal)](https://img.shields.io/badge/coverage-94.28%25%20%28CPU%20%2B%20Metal%29-brightgreen?style=flat-square)](docs/book/reference/validation.md#coverage)

zetesis is an experimental answer-set solver written in Rust. It finds solutions
to logic programs and checks them against the program's reduct. It supports
lazy grounding, parallel CPU execution and optional GPU computation.
Solving does not require clingo.

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

## First program

Save this program as `choices.lp`:

```clingo
a :- not b.
b :- not a.
```

Find all its answer sets:

```sh
zetesis solve choices.lp --all
```

The answers are `{a}` and `{b}`, in either order. Without `--all`, the default is
one answer. For a larger example with recursive rules, try
[restoring a route](examples/network-repair.lp):

```sh
zetesis solve examples/network-repair.lp --all
```

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

Execution defaults to CPU and uses lazy grounding where supported. Formula
programs default to eager grounding; explicit lazy CPU execution can stream
eligible constraints while retaining their producer core. See the
[grounding profiles](docs/book/architecture/grounding.md#eager-and-lazy-execution)
for the current limits. Use `zetesis devices` to list devices and
`--device metal` to request Metal explicitly.

A stopped search is incomplete; it does not prove unsatisfiability or optimality.
The time and memory options are not hard process-time or RSS caps.
Human output is the default, and diagnostics go to stderr.
See `zetesis help solve`, or add `--advanced` for resource controls.

## Use from Rust

Run the complete [Rust example](crates/zetesis-solve/examples/solve.rs) from the
checkout root:

```sh
cargo run --locked -p zetesis-solve --example solve --no-default-features
```

It prepares a bounded task-choice program, streams typed `AnswerSet` values,
distinguishes full interpretations from `#show`, and checks complete search.
The [library quickstart](docs/book/rust/getting-started.md) includes the full
program and dependency setup for your own application.

## Documentation

- [Command guide](docs/book/reference/commands.md): inputs, objectives, output,
  limits, testing and benchmarking.
- [Language reference](docs/book/reference/language.md): supported ASP constructs
  and arithmetic rules.
- [Rust quick start](docs/book/rust/getting-started.md): embed a solve in an application.
- [Solver architecture](docs/book/architecture/tour.md): grounding, candidate
  search and reduct checking.
- [Performance results](docs/book/reference/performance.md): measurements and their limits.
- [The zetesis Book](docs/book/index.md): the complete manual, including the
  Lean proof library.

See [Contributing](CONTRIBUTING.md) for development and verification, and
[building the book](docs/book/building.md) for a local copy of the manual.

## Status and license

zetesis remains experimental. Unsupported language constructs and execution
combinations are reported explicitly; consult the language reference before
using it for a new workload. The Lean library proves stated semantic laws, not
end-to-end correctness of the implementation.

MIT. Copyright Gregory Gelfond. See [LICENSE](LICENSE).
