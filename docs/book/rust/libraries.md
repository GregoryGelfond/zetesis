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
| Shared candidate ownership and borrowed checking | `zetesis_core::{SeedSelection, SeedView}`, `Candidates::next_selection` |
| Checked borrowed atom identity | `AtomPattern::key`, `BindingView`, `AtomKey` |
| Predicate ranges and exact typed membership | `Model::lookup`, `AtomIndex`, `AtomLookup`, `Value::compare_identity_with` |
| Append-only atom identity during synchronous grounding | `atom_interner::AtomInterner`, `CommittedAtoms`, `AtomAppender` |
| Bounded typed column views and equality selection | `zetesis_core::relation::{Relation, Query, Selection, Mask}` |
| Reusable finite-table row selection and domain projection | `zetesis_cpu::table::{Table, Domain, Selection}` |
| Explicit complete relational graph | `zetesis_core::GroundProgram::compile` |
| Normal reduct membership | `zetesis_cpu::{check, check_static, BatchOracle}` |
| Reuse scalar closure preparation across frozen seeds | `zetesis_cpu::{PreparedQueries, PreparationLimits, PreparationStatistics, ClosureWorkspace}` |
| Finite formula construction and reference membership | `zetesis_ferraris::{Theory, Node, Interpretation, check}` |
| Reusable original satisfaction with subject-bound node truth | `zetesis_ferraris::{EvaluationWorkspace, FormulaEvaluation}` |
| Least consequences for an exact positive atomic-head formula theory | `zetesis_ferraris::{PositivePlan, PositivePlanLimits}` |
| Repeated queries against one candidate's reduct | `zetesis_ferraris::FrozenReduct` |
| Regions of a candidate space and the traversal that covers them | `zetesis_cpu::regions::{Region, Traversal}` |
| Narrow a region of formula candidates, or of a reduct's subsets, by the theory's readings | `zetesis_ferraris::{Narrower, producers}` |
| Reuse one reduct encoding across different candidates | `zetesis_sat::{PreparedReduct, ReductWorkspace}` |
| Formula candidates and the reduct query by regions or by clauses | `zetesis_sat::{StableModels, SearchMethod, check_with}` |
| Bounded device execution | `zetesis_wgpu` |
| Several device primitives on one selected device | `GpuContext` and each primitive's `from_context` constructor |
| Reuse a compiled formula primitive across independent sessions | `GpuFormulaProfile`, `ExecutionResources::with_formula_profile` |
| Device equality masks over one relation | `zetesis_wgpu::GpuRelationExecutor` |
| Model-relative objective evaluation | `zetesis_objective` |
| Source-domain analysis and keyed relations | `zetesis_domain` |
| Reproducible comparisons and measurements | `zetesis_validation`, `zetesis_experiments` |
| Repository proof records and qualification policy | `zetesis_maintenance::proofs::{verify, verify_with_audit}`, `zetesis_maintenance::coverage` |
| Execute and publish current pinned proof evidence | `zetesis_maintenance::proofs::capture::capture` |

`zetesis-solve` composes ordinary solving without argument parsing, source-file
loading or output writers. `zetesis-cli` maps arguments, admits sources and
publishes the results of that public session API. Its compatibility exports
refer to the same solver types. The lower libraries remain usable independently;
a caller building a theory need not parse source, and a caller preparing a
program need not search it.

`Model::lookup` borrows the model's existing canonical selection without
allocation. For an immutable catalog of distinct atoms in arbitrary original
order, `AtomIndex::new_with` prepares exact-key and predicate orders while
preserving original row IDs.
Preparation reserves two integer indices and one temporary merge buffer, each
with the admitted catalog's row count; atoms are borrowed, not copied. Every
comparison and index write passes through the caller's fallible work callback.
`Value::compare_identity_with` uses canonical typed identity rather than ASP
term order. The [lookup implementation](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/atom_lookup.rs)
and [bounded example](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/examples/README.md)
document construction, reuse and capacity accounting.

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
Its separate `proofs::capture::capture` operation runs the pinned proof commands,
retains their outputs and prior records, and publishes only after validation
against unchanged source and tool identities. This operation writes proof
records and build artifacts; it requires explicit tool and evidence owners.
The [capture contract](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-maintenance/README.md#capture-current-proof-evidence)
documents bounded execution and recoverable publication. Neither maintenance
nor validation is a production solver dependency. Record consistency alone
does not establish that Lean ran, and successful proof execution does not
establish that Rust or WGSL satisfies the theorems.

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

`PreparedQueries` shares one exact native `Program` and retains its join
dimensions, argument bounds and dense layouts under independent preparation
limits. A `ClosureWorkspace` reuses
empty catalog metadata, reference-free join buffers and the zeroed pending
rows of the dense layouts across scalar seed
checks; returned closures own their atoms independently. It retains no completed
candidate truth and retires old capacity when used with a different program
instance. The [checked preparation example](parallel.md#reuse-preparation-across-scalar-checks)
shows the same completed semantic results as one-shot checking. Preparation work
is reported separately; per-check limits and work still apply to each seed.

`AtomPattern::key` provides a checked, borrowed atom identity over a
`BindingView`. Complete values, partial owned slots and partial borrowed slots
use the same lookup boundary. Unreferenced variables may remain absent; a missing
referenced variable returns `InstantiationError`. The key preserves signed
predicate identity, argument order and typed structural values. Its storage
ordering and hash operations agree with an owned `Atom`. `AtomKey::get` borrows
an existing member of an ordered atom set, and `Catalog::lookup_key` supplies the
same checked work receipt as owned-atom lookup. Neither operation copies an atom.
`AtomKey::to_atom` materializes a new atom by cloning its predicate and resolved
values. These operations establish identity without certifying support or
answer-set membership.

`SeedSelection` retains shared immutable handles to its true atoms. Construction
validates their gate-carrier membership and canonicalizes the selected handles;
it does not copy atom payloads. `SeedSelection::view` and `Seed::view` supply the
same borrowed true-membership interface. Missing atoms are false. Both owners
retain the original `Program` instance, and neither establishes answer-set
membership. `SeedSelection::to_seed` explicitly clones the atoms into an owned
tree for consumers requiring the existing `Seed::atoms` contract.

`Candidates::next_selection` and its owned-seed iterator share one enumeration
cursor. Mixing their pulls continues that cursor; it cannot restart enumeration.
Returned selections remain valid across later pulls or after the cursor is
dropped. Each discovered atom needs one shared allocation; each selection still
needs its handle vector, identity checks and canonicalization. These are
ownership guarantees, not a total-memory bound.

`Program::indexed_gate_atoms` supplies opaque `GateAtom` values with their
canonical gate-carrier positions. `SeedSelection::from_gate_atoms` retains
shared tokens from that exact program instance. Ordinary candidates use these
tokens: static checking resolves a position through `GroundProgram::gate_atom_ids`
without another symbolic lookup. Manually supplied atom handles use the checked
lookup path. Both routes use `SeedAtom::resolve_in`; an invalid indexed position
is refused. Tokens retain program and position metadata, so eliminating lookup
does not imply every intermediate representation is smaller.

`relation::Relation` borrows one immutable atom source with a dictionary and
aligned equality-ID columns. `from_atoms` and `from_catalog` own that layout.
For growing relations, `relation::Catalog` owns the unique typed atoms and the
same layout, extending both through checked `insert` operations. Its `view`
borrows the existing columns without allocation or reconstruction. Rust prevents
append while a view remains borrowed. Existing row and equality IDs survive
insertion; `into_atoms` transfers the tuples in insertion order.

A `Query`, `Selection` or `Mask` borrows its exact immutable relation object;
equal contents in another relation do not make the objects interchangeable.
`Row::source_index` preserves the original catalog position, while selection
positions are local to the relation. Eager formula support uses this view for
typed lookup and row access between catalog-growth rounds. Bounded primitive
experiments use the same representation. It is not an alternative source parser
or a complete grounder.

`zetesis_cpu::table::Table` borrows a relation and indexes coherent original rows
by variable/value supports. Its `select` operation accepts borrowed unrestricted,
singleton or finite domains and returns an owned mask borrowing only the
relation. Full projection additionally returns witnessed value domains. Both use
the same row restriction operation; neither copies atom payloads or asserts
logical truth. The [finite-table chapter](finite-tables.md) gives the API,
capacity/work contract and row-preservation argument.

`PreparedFormula::with_grounding_options` and its bundle counterpart compose this
primitive into eager formula grounding through explicit `JoinStrategy::Table`.
The default is `Indexed`. Only flat positive patterns over completed possible
support use table masks; structural patterns and support-growth rounds retain
indexed joins. The private query workspace accounts for shared indices and all
live masks under the enclosing source limits. It feeds the existing matcher and
authored-body validation. This scalar grounding strategy is independent of the
later solver's CPU/Rayon/GPU backend.

`Catalog::construction`, `insert` and `lookup` report operation work and actual
retained capacity. Failed operations return `CatalogFailure` with completed work
and any capacity retained before refusal. They publish no new tuple or equality
ID. Nested atom payload storage remains the source caller's separate charge.
A catalog view reports only its own object; callers retaining the catalog must
also account for `Catalog::retained_bytes` once. Appendable catalog membership
and dictionary lookup use checked AVL indexes containing only IDs and links;
insertion does not shift a historical sorted ID sequence. Typed comparisons,
transactional index updates and column/vector growth still have admitted costs.
Canonical row access uses an explicitly prepared view of sorted runs,
invalidated by a successful append and restored by sorting the appended rows
into a new run, with older runs merged only when two are within a factor of
two, rather than by traversing the index again. Standalone immutable `Relation::from_atoms` and `from_catalog`
instead build a sorted dictionary index for binary search; those relations do
not offer append operations.

`Relation::columns()` yields an exact-size iterator of argument-column slices;
`column(i)` borrows one slice. Each slice has the relation's row count. This
replaces the earlier flat packed slice: consumers should iterate columns in
argument order instead of computing offsets into one host allocation. Device
preparation copies these columns directly into one checked mapped GPU buffer,
without a temporary packed host vector.

`Relation::query_attempt` resolves equalities into that same dictionary and
retains admitted work and named peak capacity on success or failure. No partial
query escapes a refusal. Eager support probes use the receipt to charge a failed
lookup prefix to the original cumulative formula budget, while keeping its
original error and source location. `Relation::query` delegates to this operation
when a caller needs only its result. Capacity covers the relation and query
frame/buffer, excluding source payload, caller scratch and the receipt wrapper;
it is not RSS. An initial admission refusal reports zero admitted work/capacity,
while actual allocator slack after reservation remains visible even on failure.

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
The [solver library source](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/lib.rs)
is also available without a local documentation build.
