# Developing zetesis

This guide takes a collaborator from a checkout to a reviewed change. Read the
[contribution contract](../CONTRIBUTING.md) for the estate's naming, API and testing
standards. The [implementation map](implementation.md) describes current code;
the broader [specification](design/zetesis.md) also includes future capabilities.

## First checkout and first run

Obtain read access to zetesis and its pinned themelios Git dependency, then clone
using your normal GitHub credentials. Work from the repository root. The lockfile
and `rust-toolchain.toml` select the tested dependencies and Rust 1.97.1. Sibling
estate checkouts are read-only references, not build dependencies.

```sh
rustup toolchain install 1.97.1 --profile minimal --component rustfmt,clippy
cargo build --locked -p zetesis-cli
./target/debug/zetesis examples/network-repair.lp --models 0 --backend cpu
./target/debug/zetesis examples/network-repair.lp --models 0 --json --stats
./target/debug/zetesis --help
./target/debug/zetesis --help-all
```

The example has two complete answer sets. Answers go to stdout; human statistics
go to stderr. `--models 0` requests exhaustion. A resource interruption must remain
incomplete. If `CARGO_TARGET_DIR` is set, use that directory's `debug/` executable.
Short help contains everyday solve options; full help also describes oracle,
worker, batch and resource controls. Both views use the same parser/defaults.
Human metadata styling resolves stdout and stderr separately; redirected streams
and generic Auto library writers stay plain. JSON remains plain even with
`--color always`. The [CLI guide](../crates/zetesis-cli/README.md) describes the
injected-writer and failure contracts.

For regular use, `./scripts/install.sh` installs release commands in `~/.local/bin`
or a supplied binary directory. Add it to `PATH`; normal solving needs no Cargo
invocation or qualification step. A CPU-only development build uses
`cargo build --locked -p zetesis-cli --no-default-features`.

The installed commands are `zetesis`, `zetesis-bench`, `zetesis-validate`,
`zetesis-corpus` and `zetesis-perf`. Only `zetesis` is needed to solve programs.
The others expose development qualification and measurement libraries.

## Reproduce the public corpus

`examples/kr-domains` contains all 94 non-clingcon cases and their 14 shared
encodings. Elenctic annotations have been removed from the source comments;
their typed test contracts and exact deletion provenance remain in the sealed
manifest. Solver execution never interprets those annotations.

```sh
zetesis-corpus verify-examples examples/kr-domains \
  --originals validation/corpus/kr-domains
zetesis-validate --repo . --zetesis /absolute/path/to/zetesis \
  --clingo /absolute/path/to/clingo --report target/new-corpus-comparison.json
zetesis-perf examples/kr-domains --zetesis /absolute/path/to/zetesis \
  --clingo /absolute/path/to/clingo --report target/new-cpu-comparison.json
```

The first command checks integrity without running a solver. The second checks
all 94 answer contracts; the third measures the three established CPU cases
using complete paired comparisons. Keep the corpus unchanged during the 94-case
run. Performance and selected-upstream campaigns also use private source copies
and before/after seals. Run timing campaigns after builds, tests and competing
solves stop. See the [validation guide](../crates/zetesis-validation/README.md)
for resource limits, output interpretation and library entry points.

## Find the relevant boundary

| Location | Responsibility |
|---|---|
| `crates/zetesis-themelios` | Source admission, bounded preparation/grounding, binding scopes, values and provenance. Reuse the pinned themelios parser. |
| `crates/zetesis-domain` | Conservative analysis of themelios programs, independent of solving. |
| `crates/zetesis-core`, `zetesis-ferraris` | Relational/formula representations, interpretations and reduct semantics. |
| `crates/zetesis-cpu`, `zetesis-sat`, `zetesis-objective` | Candidate generation, exact checking, CPU schedules and objective work. Candidate restrictions never replace the original theory. |
| `crates/zetesis-wgpu` | Validated device inputs, WGSL primitives, bounded transport and exact residual boundaries. |
| `crates/zetesis-cli`, `zetesis-telemetry` | Reusable orchestration, typed outcomes/measurements, output views and process adaptation. |
| `crates/zetesis-validation`, `zetesis-experiments` | Independent qualification and bounded measurements, held to the same authored-code standard. |
| `proofs/` | Reusable Lean definitions/laws, explanations and checked proof records. |
| `validation/`, `examples/` | Curated source contracts, provenance/licenses and runnable examples. |
| `docs/design/`, `docs/verification/` | Architecture/decisions and dated evidence with explicit source/execution scopes. |
| `target/` or a separate ignored directory | New build products, raw campaign logs and coverage captures. |

Cleanup is incremental. The [organization plan](design/repository-organization.md)
identifies remaining Python tools, C++ provenance and historical generated data.
Do not introduce new foreign-language scaffolding or remove an active gate before
its Rust replacement is verified. Minimal shell bootstrap commands remain useful;
reusable orchestration belongs in Rust.

## Make one semantic change

Start with the proposition: admitted inputs, resulting behavior, failure cases
and preservation obligation. A language feature needs original sources, scope and
binding rules, reduct meaning, resource ceilings and unsupported neighbors. An
optimization needs justified applicability and semantic preservation before a
performance claim.

Put reusable operations in capability libraries with typed configurations and
errors. Separate semantic work from clocks, I/O and device submission. Document
ownership, cumulative limits and what success establishes: a prepared formula
can expose analysis yet still fail grounding.

Use the relevant focused tests while implementing, for example:

```sh
cargo test --locked -p zetesis-themelios --test formula_preparation
cargo test --locked -p zetesis-themelios --test structural_bindings
cargo test --locked -p zetesis-themelios --test negative_heads --test function_patterns --test positive_arguments
cargo test --locked -p zetesis-themelios --test scalar_evaluation
cargo test --locked -p zetesis-cli --test grounding_statistics
cargo test --locked -p zetesis-cli --test help --test metadata_style
```

Test names state one proposition. Complete parity checks inspect model identity
and exhaustion; displayed symbols can hide distinct answer sets. Property tests
and independent small-world reduct evaluators complement exact source regressions.
Failures of budgets, arithmetic and writers must remain failures. Avoid tests
that mirror implementation solely to increase line coverage.

Current language work is recorded by slice: [singleton heads](verification/singleton-heads-20260907/README.md),
[function patterns](verification/function-patterns-20260907/README.md), and
[independently bound positive arguments](verification/positive-arguments-20260907/README.md).
The [scalar evaluator](verification/scalar-evaluation-20260907/README.md) preserves
the pinned arithmetic contract. Their focused evidence remains distinct from the
[combined qualification](verification/execution-tranche-20260907/README.md).

## Required checks and prerequisites

The portable gate requires Rust and Python 3 for the remaining legacy tooling
tests; it needs neither clingo nor a physical GPU:

```sh
./scripts/check.sh portable
```

It runs all-feature workspace tests, CPU-only CLI tests, rustfmt, pedantic Clippy,
strict rustdoc and Criterion correctness smokes. Ignored external/device tests
are not passes. CI repeats portable checks on Linux and macOS.

Install clingo 5.8.2 independently and put `clingo` on `PATH` for external checks:

```sh
clingo --version
./scripts/check.sh oracle
zetesis-validate --repo . --zetesis zetesis --clingo clingo \
  --report target/full-compatibility.json
```

The last command checks the installed solver. Use `--zetesis` to select the exact
new executable being qualified and retain its identity with the report. The
target is all 94 unchanged non-clingcon kr-domains cases. `--reference-only`
cannot establish native compatibility. Selected upstream assertions have a
separate [comparison contract](../validation/upstream/clingo-5.8.2/README.md).

Coverage requires the pinned instrumentation tools:

```sh
rustup component add llvm-tools --toolchain 1.97.1
cargo install cargo-llvm-cov --version 0.8.7 --locked
./scripts/check.sh coverage
```

Workspace and CPU-only CLI each meet an independent 91% line floor. Inspect
uncovered behavior before adding tests; retain per-crate and feature scopes.
Concurrent coverage runs must not share a report directory. See
[coverage instructions](verification/coverage.md).

For proof changes, install Elan and the pinned Lean toolchain. Follow the
[proof instructions](../proofs/README.md) to update the declaration, source and
axiom records as well as checking the build:

```sh
elan toolchain install leanprover/lean4:v4.33.1
./scripts/check.sh proofs
```

New substantial proofs follow the [structured convention](../proofs/STYLE.md).
Mathematical laws, Rust refinement and device evidence are separate claims.

Physical device tests run separately on an accessible adapter:

```sh
zetesis devices
cargo test --locked -p zetesis-wgpu --test hardware -- --ignored --nocapture
cargo test --locked -p zetesis-cli -p zetesis-wgpu --all-features \
  --test formula_gpu --test hardware_formula -- --ignored --nocapture
cargo test --locked -p zetesis-cli -p zetesis-wgpu --all-features \
  --test lazy_gpu --test hardware_lazy -- --ignored --nocapture
```

These named groups include Metal qualification. Shader compilation, CPU fallback
and hosted CI do not qualify a physical GPU. Retain the actual adapter, binary
identity and observed device work; do not relabel old results for a new build.

## Measure and document the result

Build before timing and pause concurrent builds. Rotate compared configurations
and retain every sample, including failures. A complete comparison preserves
models, costs, ties and exhaustion. Describe source loading, preparation,
grounding, solving, output, transfer and residual work according to the actual
measured scope. `--stats` has instrumentation overhead; summed worker time and
wall time are different quantities.
The [corpus performance protocol](design/corpus-performance.md) requires every
non-clingcon kr-domains case across eager/lazy and CPU/Metal configurations, with
matched clingo runs and explicit unsupported/incomplete cells. This full matrix
has not yet been collected; existing reports retain their narrower scope.

The [scalar ablation](verification/scalar-evaluation-20260907/ablation.md) is an
example of a controlled admission measurement with matched ordered subjects and
complete models. It does not measure ordinary solve latency. The paired GPU gate
command likewise measures synthetic membership with per-candidate CPU limits;
the enumerated projection remains the default. Relational-lazy bound-column
indexes, ordinary shared-budget scheduling characterization and migration of
remaining campaigns into Rust are separate open work.

Before pushing, review the human entry points against integrated code:

- README: runnable examples, current scope and remaining limitations.
- Crate guides and rustdoc: API contracts, ownership, effects and failures.
- Implementation map: component responsibilities and actual execution routes.
- Verification index: latest qualified source and newer unqualified work.
- Design/proof notes: implemented decisions, assumptions and unproved bridges.

Run changed examples and check local links. Preserve dated records as historical
evidence instead of rewriting them for a newer binary. A collaborator familiar
with ASP and the reduct should not need the development conversation to follow
the design or reproduce its checks.
