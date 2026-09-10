# Choosing a library boundary

Use the narrowest capability that represents your input and the question you
need to answer. Parsing command-line arguments is not required to use the solver.

The examples in this part use zetesis's public APIs. `zetesis_themelios` is the
solver's own source-admission and preparation crate; `zetesis_cli` exposes its
composed solving sessions. The themelios library's parsing, logical-program
construction and standalone analysis APIs belong to its own manual.

| Input or task | Library and entry points |
| --- | --- |
| Original ASP source | `zetesis_themelios::admit`, `admit_extended`, `prepare_formula`, `admit_formula`, and their bundle APIs |
| Ordinary solve over an admitted owner | `zetesis_cli::{PreparedInput, Session, SolveConfig}` |
| All original answers, streamed or completely collected | `Session::enumerate`, `WorldView::collect`, checked `AnswerSet` |
| Finite relational templates and atoms | `zetesis_core::{Program, Template, Atom, Seed}` |
| Bounded typed column views and ordered equality selection | `zetesis_core::relation::{Relation, Query, Selection}` |
| Explicit complete relational graph | `zetesis_core::GroundProgram::compile` |
| Normal reduct membership | `zetesis_cpu::{check, check_static, BatchOracle}` |
| Finite formula construction and reference membership | `zetesis_ferraris::{Theory, Node, Interpretation, check}` |
| Repeated queries against one candidate's reduct | `zetesis_ferraris::FrozenReduct` |
| Native formula candidate/countermodel search | `zetesis_sat` |
| Bounded device execution | `zetesis_wgpu` |
| Device equality masks over one relation | `zetesis_wgpu::GpuRelationExecutor` |
| Model-relative objective evaluation | `zetesis_objective` |
| Source-domain analysis | `zetesis_domain` |
| Reproducible comparisons and measurements | `zetesis_validation`, `zetesis_experiments` |
| Repository proof records and qualification policy | `zetesis_maintenance` |

Despite its current crate name, `zetesis-cli` exposes a writer-free ordinary
solver session. It is the appropriate existing entry point when an application
wants the solver's composed behavior. The lower libraries remain usable
independently; a caller building a theory need not parse source, and a caller
preparing a program need not search it.

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
The formula owner keeps the theory, atom indexing, objective program and
observations together. Do not build a session by independently pairing a theory
with an atom table from another admission.

`relation::Relation` borrows one immutable atom source and owns a dictionary and
aligned equality-ID columns. A `Query` or `Selection` borrows that exact owner;
equal contents in another relation do not make the objects interchangeable.
`Row::source_index` preserves the original catalog position, while selection
positions are local to the relation. The view currently serves bounded primitive
experiments. It is not an alternative source parser or a complete grounder.

`GpuRelationExecutor::prepare` borrows that relation and the executor, uploading
its equality-ID columns. The resulting prepared view accepts queries from the
same owner and returns query-ordered masks. `RelationGpuMasks::selection`
reconstructs checked local row positions for one query at a time. Complete
pattern matching remains a separate operation. Preparation and each filter have
explicit resource limits; zero rows or zero queries require no compute dispatch.
This is a bounded device primitive, not ordinary source grounding on the GPU.
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

Generate the [local Rust API reference](../../doc/zetesis_cli/index.html) as
described in [Building the documentation](../building.md). Public signatures and
their per-operation cost and error contracts are authoritative.
