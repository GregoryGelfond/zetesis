# Choosing a library boundary

Use the narrowest capability that represents your input and the question you
need to answer. Parsing command-line arguments is not required to use the solver.

The examples in this part use zetesis's public APIs. `zetesis_themelios` is the
solver's own source-admission and preparation crate; `zetesis_solve` exposes its
composed solving sessions. The themelios library's parsing, logical-program
construction and standalone analysis APIs belong to its own manual.

| Input or task | Library and entry points |
| --- | --- |
| Original ASP source | `zetesis_themelios::admit`, `admit_extended`, `prepare_formula`, `admit_formula`, and their bundle APIs |
| Ordinary solve over an admitted owner | `zetesis_solve::{PreparedInput, Session, SolveConfig}` |
| Compose selection, observations and reusable device resources | `Session::builder`, `SessionBuilder`, `ExecutionResources` |
| Share optional host instrumentation across admission and solving | `SolveMeasurements` |
| All original answers, streamed or completely collected | `Session::enumerate`, `WorldView::collect`, checked `AnswerSet` |
| Finite relational templates and atoms | `zetesis_core::{Program, Template, Atom, Seed}` |
| Bounded typed column views and equality selection | `zetesis_core::relation::{Relation, Query, Selection, Mask}` |
| Explicit complete relational graph | `zetesis_core::GroundProgram::compile` |
| Normal reduct membership | `zetesis_cpu::{check, check_static, BatchOracle}` |
| Finite formula construction and reference membership | `zetesis_ferraris::{Theory, Node, Interpretation, check}` |
| Repeated queries against one candidate's reduct | `zetesis_ferraris::FrozenReduct` |
| Native formula candidate/countermodel search | `zetesis_sat` |
| Bounded device execution | `zetesis_wgpu` |
| Several device primitives on one selected device | `GpuContext` and each primitive's `from_context` constructor |
| Reuse a compiled formula primitive across independent sessions | `GpuFormulaProfile`, `ExecutionResources::with_formula_profile` |
| Device equality masks over one relation | `zetesis_wgpu::GpuRelationExecutor` |
| Model-relative objective evaluation | `zetesis_objective` |
| Source-domain analysis | `zetesis_domain` |
| Reproducible comparisons and measurements | `zetesis_validation`, `zetesis_experiments` |
| Repository proof records and qualification policy | `zetesis_maintenance` |

`zetesis-solve` composes ordinary solving without argument parsing, source-file
loading or output writers. `zetesis-cli` maps arguments, admits sources and
publishes the results of that public session API. Its compatibility exports
refer to the same solver types. The lower libraries remain usable independently;
a caller building a theory need not parse source, and a caller preparing a
program need not search it.

For a native application, depend on `zetesis-solve` and the libraries producing
its chosen input. A source consumer normally also uses `zetesis-themelios` for
admission and `zetesis-cpu::Control` for cooperative cancellation. The packages
are currently consumed from this repository, not a published crates.io release.
Use a pinned Git revision or paths into a checked-out workspace. With a path
dependency, for example:

```toml
[dependencies]
zetesis-solve = { path = "../zetesis/crates/zetesis-solve" }
zetesis-themelios = { path = "../zetesis/crates/zetesis-themelios" }
zetesis-cpu = { path = "../zetesis/crates/zetesis-cpu" }
```

The `gpu` feature is enabled by default. Set `default-features = false` on
`zetesis-solve` for a CPU-only consumer; another dependency enabling that feature
can still activate it through Cargo feature unification. Enabling GPU support
does not select hardware by itself: `SolveConfig` governs the requested route.
The [session chapter](sessions.md) supplies a complete checked example, and the
[measurement chapter](measurements.md) covers injected observations and timing.

Validation and repository maintenance are distinct development capabilities.
`zetesis_validation` checks captured answer sets and comparison contracts. Its
`corpus_comparison::run` accepts a typed `Request` and a progress observer,
returning a typed `Report`; command-line parsing and report publication remain
separate consumers. `Report::to_json` is a fallible presentation view, not the
source of a comparison decision. The `performance` API similarly separates a
measurement request and runner from its report views;
`zetesis_maintenance` checks recorded proof evidence and qualification policy.
Neither is a production solver dependency. A consistent proof record does not
establish that Lean ran or that a Rust implementation satisfies its theorems.

Start with the [runnable tour](../architecture/tour.md) for a complete source-to-answer
path. The [source preparation example](source.md) separates analysis from eager
materialization and shows when analysis describes only a dependency projection.

Some source and analysis values exposed by zetesis use themelios types. Their
meaning matters when inspecting a prepared input, especially its analysis basis
and source identities. A formula atom index or dense relational ID is local to
its admitted owner; it is not a transferable identity across admissions.

## Owners preserve coherence

`PreparedInput::admitted` borrows an `Admitted` owner; `PreparedInput::formula`
borrows an `AdmittedFormula`. Bundle variants retain multi-file provenance.
`PreparedInput::program` accepts an already admitted native `Program` directly,
without source metadata or eager compilation.
The formula owner keeps the theory, atom indexing, objective program and
observations together. Do not build a session by independently pairing a theory
with an atom table from another admission.

`relation::Relation` borrows one immutable atom source and owns a dictionary and
aligned equality-ID columns. A `Query`, `Selection` or `Mask` borrows that exact owner;
equal contents in another relation do not make the objects interchangeable.
`Row::source_index` preserves the original catalog position, while selection
positions are local to the relation. Eager formula support uses this view for
typed lookup and row access between catalog-growth rounds. Bounded primitive
experiments use the same representation. It is not an alternative source parser
or a complete grounder.

`Relation::select` returns ordered positions; `Relation::select_mask` applies
the same equality predicate directly into packed original-row membership.
Neither needs to convert through the other's representation. Mask production
charges word initialization even when no row can match. Dense masks can occupy
more storage than positions for sparse results. `selection_from_mask` checks
shape and reconstructs ordered positions; arbitrary supplied bits do not carry
a query-completeness certificate.

`GpuRelationExecutor::prepare` borrows that relation and the executor, uploading
its equality-ID columns. The resulting prepared view accepts queries from the
same owner and returns query-ordered masks. `RelationGpuMasks::selection`
reconstructs checked local row positions for one query at a time. Complete
pattern matching remains a separate operation. Preparation and each filter have
explicit resource limits; zero rows or zero queries require no compute dispatch.
This is a bounded device primitive, not ordinary source grounding on the GPU.
Other primitives constructed from the same `GpuContext` can execute while this
prepared relation remains live. Operations take a nonblocking context lease;
overlap is a typed busy refusal, and device failure invalidates every dependent
primitive. Read the [ownership contract](../architecture/ownership.md#device-resource-scope)
before sharing contexts or accounting for several prepared views.
The [relation measurement profile](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-experiments/README.md#retained-relation-selection)
uses these library operations with matched typed inputs, packed outputs and
shared reconstruction across scalar CPU, Rayon and physical GPU execution.

`PreparedInput::ground` borrows an `Arc<GroundProgram>` that retains its original
program identity. It reuses the supplied graph; an explicit request for lazy
grounding or a formula oracle is incompatible with this input.

Current APIs do not promise a themelios-solve backend implementation, a general
ASPIF interchange boundary or a custom theory-propagator interface. Those
capabilities require additional contracts. The existing admitted-owner and
session boundaries are useful without pretending those interfaces are present.

Generate the [local Rust API reference](../../doc/zetesis_solve/index.html) as
described in [Building the documentation](../building.md). Public signatures and
their per-operation cost and error contracts are authoritative.
