# zetesis

[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
![Rust 1.97+](https://img.shields.io/badge/rust-1.97%2B-orange?style=flat-square)
[![Line coverage: 94.00% (CPU + Metal)](https://img.shields.io/badge/coverage-94.00%25%20%28CPU%20%2B%20Metal%29-brightgreen?style=flat-square)](docs/book/reference/validation.md#coverage)

ζήτησις, *inquiry/search* — candidate-directed answer-set solving through the reduct.

zetesis is an independent Rust answer-set solver with lazy grounding, parallel
CPU execution and GPU computation. A generator proposes an interpretation; an
exact oracle checks the original program and its frozen reduct. clingo is an
external test oracle, never the runtime grounder or solver.

**The reduct is the architectural foundation.** For normal rules, checking
specializes to least closure, constraints and candidate agreement. General
Ferraris formulas require minimality: no proper subset of a candidate may satisfy
its frozen reduct. A checked program-class certificate can justify a cheaper
exact test without changing this acceptance criterion.

Start with the [guided tour](docs/book/architecture/tour.md). The
[zetesis Book](docs/book/index.md) develops the solver architecture, teaches the
Rust libraries and presents the Lean proof library. It connects the logical
operations to the joins, masks, fixed points and batches that implement them.
The [alignment chapter](docs/book/architecture/alignment.md) makes that connection
explicit with a diagram, algorithm pseudocode and links to the corresponding
Rust and Lean definitions.
The [ownership chapter](docs/book/architecture/ownership.md) explains how logical
identity, prepared data and invocation state delimit memory reuse and parallel
work.

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
zetesis examples/network-repair.lp --models 0 --stats
```

The [complete example](examples/network-repair.lp) has two answer sets: one
repairs the northern bridge, the other the southern bridge. Each includes its
facts and recursive consequences. Merely assuming that the clinic is reachable
does not supply the support required by the reduct.

A [task-allocation example](examples/kr-domains/scenarios/task-allocation/variant-01/01-basic.lp)
adds bounded choices and an objective:

```clingo
1{ assigned_to(A,T) : agent(A), compatible_with(A,T) }1 :- task(T).
#minimize{ C,A,T : assigned_to(A,T), cost_of(A,T,C) }.
```

```sh
zetesis examples/kr-domains/scenarios/task-allocation/variant-01/01-basic.lp
```

Its optimum assigns `a1` to `t1` and `a2` to `t2`, with cost **5**. Objectives
score verified answer sets; they supply no atom support. `OPTIMUM FOUND` requires
complete search, and `--models 0` displays every tied optimum.

## Install and run

From a checkout, install the pinned toolchain and release commands once:

```sh
rustup toolchain install 1.97.1 --profile minimal --component rustfmt,clippy
./scripts/install.sh
zetesis --help
zetesis examples/network-repair.lp --models 0 --json
```

The repository and pinned themelios dependency require GitHub read access for the
initial checkout/build. No sibling checkout is needed. The installer defaults to
`~/.local/bin`; add it to `PATH` if requested, or pass a different installation
directory. Installed commands need neither Cargo nor a Rust toolchain at runtime.

Human output is the default, with terminal-aware color. `--json` provides a
versioned machine view. `--stats` reports measured phases and work counters;
eager grounding is timed separately, while lazy joins interleave with solving.
A resource stop is **incomplete**, never an UNSAT proof. Parser errors include
themelios diagnostics with source locations and excerpts.

`--help` shows everyday options; `--help-all` adds execution and resource controls.
Without objectives the default requests one answer set. Use `--models 0` for
exhaustive enumeration. The [CLI reference](crates/zetesis-cli/README.md) documents
stream contracts, diagnostics and exit codes.

## Grounding and execution

```sh
zetesis examples/network-repair.lp --backend cpu --grounder lazy --models 0
zetesis examples/network-repair.lp --backend metal --grounder eager --models 0
zetesis devices
```

Automatic selection uses the admitted program profile and available execution
capabilities. Explicit choices remain useful for reproducible comparisons.
Normal relational programs default to lazy source joins, including when
automatic selection moves later batches from CPU to Metal. Explicit eager
grounding remains available on either backend.
General formulas use eager source grounding, with host candidate search,
optional GPU propagation and exact CPU completion of unresolved reduct queries.
For CLI invocations that select GPU execution, adapter discovery and initialization
happen automatically during the solve, including on the first run after installation.
`zetesis devices` is an optional inventory.
Automatic selection may keep small workloads on the CPU; `--backend metal`
requests Metal explicitly.

Metal has physical regression coverage on Apple M4 Pro. Vulkan is implemented
but needs physical qualification on each claimed platform. Full GPU residency,
general lazy formula construction and broad hardware qualification remain open.
The [execution chapter](docs/book/architecture/execution.md) distinguishes
semantic guarantees, scheduling and the work that remains on the host.
The tight GPU library offers atomic-OR and grouped-word support construction;
both have physical Metal checks. Atomic remains its default. These are reusable
membership primitives, and ordinary solving does not automatically select them.
Eager formula grounding uses typed column snapshots over one possible-atom
catalog. The same relation library supplies CPU/Rayon/GPU equality-selection
primitives with checked row reconstruction. It preserves logical values and row
identity. The eager support consumer currently runs on the host; GPU equality
selection is a separate library capability.
The [primitive measurements](crates/zetesis-experiments/README.md) describe the
operation boundaries and reproducible commands.

## Libraries and assurance

The [zetesis-solve library](crates/zetesis-solve/README.md) composes admitted
programs, interpretations, bounded checks, sessions and typed outcomes
independently of terminal rendering. The CLI consumes its public session API.
A reusable
`zetesis_ferraris::FrozenReduct` binds a reduct to the interpretation that defines
it. Independent satisfaction queries can share that immutable reduct.
See the [working Rust examples](docs/book/rust/libraries.md).

The session API returns checked `AnswerSet` values. `Session::enumerate` streams
the original answer family, including nonoptimal answers with their scores.
Optional bounded `WorldView` collection requires complete enumeration; a stopped
prefix and a selected optimum remain distinct results. The
[session manual](docs/book/rust/sessions.md) explains ownership and completion.
`Session::builder` composes answer selection, typed observations, measurements
and reusable `ExecutionResources`. Its `collect` operation requires the complete
original answer family. Sessions supplied the same context share the device;
formula sessions supplied the same compiled profile also reuse that pipeline.
Their subjects, work budgets, candidate state and outcomes remain independent.

The [Lean library](proofs/README.md) develops satisfaction, reducts, minimality,
normal least closure and their preservation laws. Its normalized-rule translation
is proved equivalent to Ferraris answer-set semantics. These mathematical laws
are not yet an end-to-end verification of Rust, source lowering or WGSL. The
[correspondence chapter](docs/book/lean/correspondence.md) states that boundary.
An optional [executable refinement](refinement/membership/README.md) proves
packed membership for an extracted Rust method under an explicit storage
invariant and translation/library-model assumptions.

The implementation remains experimental. Full intended language coverage and
mission-critical deployment assurance are goals, not present certifications.
Theory atoms, Python/Lua scripting, `#heuristic` and `#edge` are outside the current
supported scope; Rust ground-time functions and broader incremental integration
remain planned. Unsupported constructs receive explicit refusals.
The [admitted-language reference](docs/book/reference/language.md) records the
precise boundaries, including shared atoms and tuples in aggregate heads,
complete logical values in extrema heads, and logical bounds on choices and
numeric aggregates. Missing extremum-head values are neutral while retaining
independent head permission; the [formal rule and worked reduct](proofs/guide/head-contributions.md)
explain this declared extension and its exact clingo differences.

Objective priority expressions can use safely bound values; weight, priority and
tuple stay correlated within each binding. Completed finite source relations also admit changing priorities from filtered
and multiple aggregate observers, including mixed extrema and recursive
aggregate or conditional producers. A complete support round supplies the
finite carrier; exceeding a resource limit supplies no partial program. The
[source-coverage argument](proofs/guide/source-support.md) keeps possible support
separate from model truth and retains the original producer theory. Original-model
conditions determine costs, independently of answer-set membership. Remaining
source scopes and explicit clingo reporting differences are documented alongside
the supported contracts.

Finite scoped weak constraints can use aggregate
assignments and universal conditionals, including pooled conditional consequents;
their costs read the original answer without adding rules or support. Broader
weak-body pools and observation binding contexts remain explicit obligations.
Observations can bind positive equality chains and capture constructor or tuple
components from finite values, while preserving each original model beside its
completed display.

Directed finite affine comparison chains can bind several variables while
retaining the original correlations and checked source arithmetic.
Choice and aggregate-head elements preserve `not` and `not not` explicitly.
Default-negated operands contribute activity without positive producer support;
Boolean operands carry truth without creating atoms. Universal body conditionals
apply default and double negation after complete finite anonymous witness
projection. The
[language coverage checklist](docs/book/reference/language-coverage.md) identifies
remaining obligations and unclassified scopes.

The [neuromorphic appendix](docs/book/appendices/neuromorphic.md) specifies a
possible Loihi 2/SpiNNaker2 backend, its exact event protocol and its qualification
obligations. Native execution and performance on those platforms remain unproven.

The self-contained [kr-domains examples](examples/kr-domains/README.md) include
94 non-clingcon cases and 14 shared encodings, with source hashes, licenses and
annotation-removal provenance. Regression comparisons check completed displayed
answer multisets, model counts, objective costs and optimum ties. Reproduce comparisons with the Rust
[validation tools](crates/zetesis-validation/README.md); distinguish matched
end-to-end solves from kernel measurements when comparing performance.
The [validation chapter](docs/book/reference/validation.md) explains which
claims the corpus, proof and physical execution checks can establish.
Its [performance evidence](docs/book/reference/validation.md#performance-evidence)
records nine-case CPU/eager comparisons with clingo, side-by-side child peak-RSS
comparisons, and bounded N=8/N=10 queens comparisons. Results retain failed and
unavailable cases alongside complete native-model agreement. The columnar
integration has not demonstrated a broad latency or RSS reduction. Matched
Metal solves and separate GPU primitive measurements retain their own scopes
and recorded hardware.

See [Contributing](CONTRIBUTING.md) for development and verification requirements,
and [build the book](docs/book/building.md) to read the complete manual locally.

## License

MIT. Copyright Gregory Gelfond.
