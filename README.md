# zetesis

[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
![Rust 1.97+](https://img.shields.io/badge/rust-1.97%2B-orange?style=flat-square)

ζήτησις, *inquiry/search* — candidate-directed answer-set solving through the reduct.

zetesis is an independent Rust answer-set solver exploring lazy materialization,
parallel CPU execution and GPU computation. A generator proposes an
interpretation; an exact oracle checks the original program and its frozen
reduct. Only an interpretation that meets the stable-model criterion is returned
as an answer set. Clingo is an external test oracle, never the runtime grounder
or solver.

**The reduct is the architectural foundation.** For safe normal rules, checking
specializes to least closure, constraints and candidate agreement. Richer
programs use finite Ferraris formulas and proper-subset countermodel checking.
A checked certificate can justify a cheaper exact test for an eligible program
class. These are semantic specializations of the same acceptance criterion.

The aim is a massively parallel ASP system with the assurance required by
mission-critical applications. The current implementation is experimental and
hybrid: general formula grounding and candidate search run on the host, with
optional GPU propagation and exact CPU completion. Full language parity, general
lazy formula construction, a demonstrated full-solve GPU advantage and deployment
qualification remain open.

## Restore a route

A clinic is cut off because both approach bridges need repair. A crew can repair
one bridge. Which repair plans restore a route from the depot to the clinic?

```clingo
road(depot,north).
road(depot,south).

{ repair_north }.
{ repair_south }.
:- repair_north, repair_south.

open(X,Y) :- road(X,Y).
open(north,clinic) :- repair_north.
open(south,clinic) :- repair_south.

reachable(depot).
reachable(Y) :- reachable(X), open(X,Y).
:- not reachable(clinic).
```

```sh
zetesis examples/network-repair.lp --models 0
```

The [complete example](examples/network-repair.lp) has two answer sets: one
repairs the northern bridge, the other the southern bridge. Each includes its
facts and recursive consequences. The solver rejects unsupported reachability
and a plan requiring both repairs. The regression suite compares complete models
with clingo, rather than just their displayed repair choices.

An unchanged [kr-domains task-allocation scenario](validation/corpus/kr-domains/scenarios/task-allocation/variant-01/01-basic.lp)
adds an objective:

```clingo
1{ assigned_to(A,T) : agent(A), compatible_with(A,T) }1 :- task(T).
#minimize{ C,A,T : assigned_to(A,T), cost_of(A,T,C) }.
```

```sh
zetesis validation/corpus/kr-domains/scenarios/task-allocation/variant-01/01-basic.lp
```

Its optimum assigns `a1` to `t1` and `a2` to `t2`, with cost **5**. Objectives
score verified stable models; they supply no atom support. Exact bounds may
prune candidates that cannot improve or tie an incumbent. `OPTIMUM FOUND`
requires complete search, and `--models 0` displays every tied optimum.
The [eight-queens example](validation/corpus/kr-domains/standalone/n-queens/variant-01.lp)
likewise preserves all **92 answer sets**.

## Install and run

From a checkout, install the pinned Rust 1.97.1 toolchain and native release
commands once:

```sh
rustup toolchain install 1.97.1 --profile minimal --component rustfmt,clippy
./scripts/install.sh
zetesis --help
zetesis --version
zetesis examples/network-repair.lp --models 0 --stats
zetesis examples/network-repair.lp --models 0 --json
```

The private repository and pinned themelios dependency require GitHub read
access for the initial checkout/build. No sibling estate checkout is needed.
The installer defaults to `~/.local/bin`; add that directory to `PATH` if the
installer reports it missing. Pass a different binary directory as its argument.
Installed commands need neither Cargo nor a Rust toolchain at runtime. GPU
support is included; normal solving initializes its own device and pipelines.

Give multiple files in order, or use `-` for standard input. Without objectives,
the default requests one answer set; `--models 0` requests exhaustive enumeration.
Human output is the default. `--json` exposes a versioned streaming view retaining
full models separately from displayed symbols, objective costs, completion and
partial failure evidence. A resource stop is **incomplete**, never an UNSAT proof.
The [CLI guide](crates/zetesis-cli/README.md) describes stream contracts and exit
codes.

`-h` and `--help` show everyday solving options. Use `--help-all` for the full
oracle, worker, batch and resource controls; these options remain available in
ordinary invocations.

On a capable terminal, human answer headings use cyan with a bold `Answer:` label;
optimization metadata uses italic green. Solve metadata on stderr uses blue
labels and italic gray values. Colors come from your terminal palette.
`--color auto|always|never` controls styling. Automatic mode resolves stdout and
stderr independently, respects a nonempty `NO_COLOR` and `TERM=dumb`, and leaves
each redirected stream plain. JSON never contains styling.

`--stats` adds host timings and available work counters to stderr. It separates
source preparation, eager grounding, solving and output. Formula grounding has
additional attribution for support completion, rule instantiation and other
materialization phases. Lazy joins are explicitly interleaved with solving;
unentered or unmeasured work is not reported as zero. Timings exclude source
loading, include instrumentation overhead, and are not GPU kernel measurements.
Statistics are also available as typed Rust values and in the JSON view.

## Select grounding and execution

```sh
zetesis examples/network-repair.lp --backend cpu --grounder lazy --models 0
zetesis examples/network-repair.lp --backend metal --grounder eager --models 0
zetesis devices
```

`zetesis devices` is an optional inventory command, not a setup step.

| Control | Current behavior |
|---|---|
| `--grounder auto\|lazy\|eager` | Normal-rule CPU execution can join source templates lazily or lower eagerly. Formula execution currently requires eager materialization. Unsupported explicit combinations are refused. |
| `--backend auto` | The closure route starts on lazy CPU; later eligible batches may use an accessible GPU. Formula solving selects CPU automatically. The closure batch threshold is provisional, not a measured crossover. |
| Explicit backend | `cpu`, `metal`, `vulkan`, `dx12`, `gl`, `nvidia` or `gpu`. An unavailable required device is an error. NVIDIA uses a supported wgpu API, not CUDA. |
| `--completion-workers N` | Bounded Rayon completion of independent formula-reduct queries, including GPU residuals. Defaults to one worker. |
| `--batch-size N` | Bounds candidate batches. Logical completion scratch has its own explicit ceiling. |
| `--oracle auto` | Selects an exact supported check, including certified support checking where applicable. `closure` and `countermodel` make the procedure explicit. |

Metal has been physically qualified on an Apple M4 Pro for the recorded builds.
The static closure oracle uses a bounded ground graph; general formula execution
batches GPU propagation and exact CPU residual completion. Source loading,
parsing, materialization, candidate generation and objective work still run on
the host. GPU/lazy execution, CUDA, multi-GPU execution and neuromorphic backends
are future work. See [execution boundaries](docs/implementation.md) and the
[hardware evidence](docs/verification/metal-requalification-20260907/README.md).

## Status

The language target is clingo source and answer-set compatibility **excluding
theory terms and Python/Lua scripting**. Passing a selected corpus is not a
percentage of language compatibility. The source boundary uses the unchanged
themelios parser and owned program model; admission failures distinguish upstream
syntax, raising and evaluation errors from zetesis implementation refusals.

| Area | Implemented scope |
|---|---|
| Normal rules | Safe finite rules, constraints, default/double negation, strong negation with coherence, and relational lazy checking in the admitted normal-rule profile. |
| Formula rules | Bounded choices, signed disjunctions, finite rule/head pools, evaluated heads, scalar/range bindings, comparisons and admitted universal body conditionals. |
| Aggregates | Body count/sum/sum+ and complete-value min/max comparisons, scoped independent assignments, and a restricted finite function-count head profile. |
| Logical values | Closed signed functions and tuples; finite constructor generation from bound inputs; positive tuple-pattern extraction; evaluated already-safe negative arguments. Positive named-function patterns remain a gap. |
| Objectives and observations | Admitted minimize/maximize/weak constraints; complete tuple keys and optimal ties; signature, term and conditional `#show`; `#defined`; original include bundles and constants. |
| Refusal boundaries | Broader conditioned heads, some binding/aggregate combinations, objective-dependent disjunction/conditionals, broader directives and exact clingo undefined-arithmetic behavior remain incomplete. |

These rows summarize profiles; they are not a grammar specification. The
[source API guide](crates/zetesis-themelios/README.md) describes composition,
scope and resource limits. The [compatibility matrix](docs/verification/clingo-compatibility.md)
records the broader target. [Numeric semantics](docs/design/numeric-semantics.md)
explains endpoint guards and why an internal refusal does not by itself establish
a modeling error. Undefined or overflowing admitted arithmetic currently produces
an explicit refusal rather than reproducing all of clingo's simplifications.

The current combined checkpoint passes **94 unchanged non-clingcon
kr-domains cases**, checking answer contracts, costs, counts and optimum ties.
All **24 selected upstream clingo assertions** pass with 73 complete full-model
occurrences, retaining original bytes and provenance. Local line coverage is
**91.4857% workspace / 91.9220% CPU-only CLI**, and the Lean library checks
**646 theorems in 45 modules**. The
[checkpoint](docs/verification/grounder-tranche-20260907/README.md) states exact
source/binary scopes and limitations; the
[verification record](docs/verification/status.md) indexes historical results.

Earlier CPU measurements demonstrate useful specialized improvements, while
SEND + MORE = MONEY remains slower than clingo and spends most of its measured
driver time grounding. GPU measurements establish correctness and mixed
microbenchmark results, not a general full-solve speedup. The
[performance record](docs/verification/api-hardening-20260907/README.md)
and [SEND attribution pilot](docs/verification/grounder-performance-20260907/send-attribution.md)
keep these distinct. The new attribution API locates work to guide general
improvements with matched, complete-result measurements.

## Libraries and mathematical specification

The project is library-first. Source programs, ground interpretations, checked
answer sets, objectives, completion and publication are distinct objects. Human
and JSON output are views of those objects. Public limits and typed failures are
part of each capability, not just command-line behavior.

| Start here | Responsibility |
|---|---|
| [zetesis-themelios](crates/zetesis-themelios/README.md) | Bounded source preparation, themelios analysis, relational admission and eager formula grounding, preserving origins. |
| [zetesis-domain](crates/zetesis-domain/README.md) | Conservative domain analysis over a borrowed themelios program, independent of solving. |
| [zetesis-core](crates/zetesis-core/src/lib.rs) / [zetesis-ferraris](crates/zetesis-ferraris/README.md) | Relational and formula semantics, interpretations, reducts and validated representations. |
| [zetesis-cpu](crates/zetesis-cpu/src/lib.rs) / [zetesis-wgpu](crates/zetesis-wgpu/README.md) | Closure execution and GPU primitives. |
| [zetesis-sat](crates/zetesis-sat/README.md) / [zetesis-objective](crates/zetesis-objective/README.md) | Native candidate/countermodel search and exact objective work. Boolean queries are internal machinery; stable acceptance belongs to the reduct composition. |
| [zetesis-cli](crates/zetesis-cli/README.md) | Prepared-input sessions, solve configuration, typed outcomes and output views, plus the process adapter. Extracting orchestration into a dedicated package remains planned. |
| [zetesis-validation](crates/zetesis-validation/README.md) / [zetesis-experiments](crates/zetesis-experiments/README.md) | External-oracle qualification, curated fixtures and bounded measurements, separate from production acceptance. |

`prepare_formula` and `prepare_bundle_formula` expose analysis before ground
materialization. Consuming their receipts with `ground()` resumes the original
resource budget. Preparation is not a guarantee of successful grounding or
proof that a lazy strategy supports that source. Standalone ground/solve commands,
ASPIF interchange and Rust `@` functions remain design work.

The [design specification](docs/design/zetesis.md) gives the architecture.
The in-repository [Lean library](proofs/README.md) gives reusable mathematical
laws, with a [reading guide](proofs/guide/README.md), structured proof convention,
pinned toolchain and axiom/source audit. These are checked semantic contracts;
they do **not** certify the Rust compiler, search implementation or shaders.
See the [theory-extension design](docs/design/theory-propagators/README.md)
and [neuromorphic feasibility assessment](docs/design/neuromorphic-feasibility-20260906.md)
for future integrations.

## Build and check

New collaborators should start with [Contributing](CONTRIBUTING.md) and the
[development guide](docs/development.md): repository map, first edit, test
selection, qualification prerequisites and documentation responsibilities.

```sh
cargo build --locked -p zetesis-cli
cargo test --locked --workspace --all-features
./scripts/check.sh portable
```

The portable gate includes rustfmt, pedantic Clippy, strict rustdoc, both normal
and CPU-only CLI tests, Criterion correctness smokes and the remaining legacy
tooling tests. Rust property tests use proptest. Optional checks have explicit
prerequisites:

```sh
./scripts/check.sh oracle    # external clingo 5.8.2 on PATH
./scripts/check.sh proofs    # pinned Lean 4.33.1 through Elan
./scripts/check.sh coverage  # cargo-llvm-cov and llvm-tools
```

Workspace and CPU-only CLI coverage each retain an independent **91% line
floor**. Coverage does not measure assertion strength, Lean correspondence or
physical shader execution. GPU qualification is a separate test group on an
accessible device. [CI](.github/workflows/checks.yml) runs portable gates on
Linux/macOS, proofs and Linux coverage; a hosted runner does not establish NVIDIA
or Metal hardware qualification.

The intended authored implementation is Rust, Lean and WGSL. Repository cleanup
is in progress: existing Python qualification tools and historical upstream C++
provenance remain until Rust replacements and curated evidence are verified.
The [organization plan](docs/design/repository-organization.md) records the
migration without weakening gates or discarding evidence. Tooling follows the
same coding and review standards as the solver.

## Built on

[themelios](https://github.com/GregoryGelfond/themelios) supplies source identity,
parsing, programs and analysis at the estate's reviewed `87c11a3` pin.
[Rayon](https://github.com/rayon-rs/rayon) supplies CPU parallel execution;
[wgpu](https://github.com/gfx-rs/wgpu) supplies the GPU boundary.

zetesis is a separate experiment from apokrisis, and a member of the same estate
as themelios, keryx and morphe. The [standards alignment](docs/design/estate-alignment-20260907.md)
and [assurance audit](docs/verification/audit-contract.md) describe the quality
floor and the work still needed to reach the intended deployment standard.

## License

[MIT](LICENSE). Copyright © 2026 Gregory Gelfond.
