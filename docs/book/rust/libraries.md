# Library reference index

Start with [Getting started with the library](getting-started.md) for dependency
setup, then [Programs, answers and queries](agent.md) for the public `zetesis` API.
For direct control of native preparation and execution, the complete
[session example](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/examples/solve.rs)
prepares source through `zetesis_themelios`, consumes checked answers from a
`zetesis_solve::Session`, and inspects its final outcome. From a checkout, run
`cargo run -p zetesis-solve --example solve --no-default-features`.
Use the tables below to find a specific capability.

## Applications and source programs

| Task | Public library entry points | Guide |
| --- | --- | --- |
| Construct canonical ASP and use upstream solve/query vocabulary | `zetesis::{Program, Symbol, program, syntax, analysis, solve, query, prelude}` | [Facade](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis/README.md) |
| Solve and query a canonical knowledge base | `zetesis::{Solver, Config}`, `zetesis::solve::agent::Agent`, `zetesis::query::AgentReading` | [Programs, answers and queries](agent.md) |
| Admit ASP source | `zetesis_themelios::{admit, admit_extended, prepare_formula, admit_formula}` and bundle counterparts | [Source preparation](source.md) |
| Prepare or admit a logical formula program | `zetesis_themelios::{prepare_program_formula, admit_program_formula, ProgramAdmissionOptions, PreparedFormula, ProgramSite}` | [Typed formula preparation](source.md#prepare-a-logical-formula-program) |
| Prepare an owned logical relational program | `zetesis_themelios::{prepare_program_relational, ProgramRelationalOptions, PreparedRelational}` | [Owned relational preparation](source.md#admit-an-existing-logical-program) |
| Admit a logical program under S0 | `zetesis_themelios::{admit_program, ProgramAdmissionOptions, AdmittedProgram}` | [Typed program admission](source.md#borrow-the-strict-relational-input) |
| Solve admitted input | `zetesis_solve::{PreparedInput, Session, SessionBuilder, SolveConfig}` | [Sessions](sessions.md) |
| Collect a complete native family | `zetesis_solve::WorldView::collect`, `SessionBuilder::collect`, `WorldViewLimits` | [Completion and output](outcomes.md) |
| Inspect an answer or interpretation | `AnswerSet`, `zetesis_core::{Interpretation, Model}`, `catalog::{AtomRef, TermRef}` | [Interpretations and atoms](models.md) |
| Evaluate costs and source displays | `zetesis_objective`, admitted-owner observation APIs | [Costs and shown terms](costs-and-output.md) |
| Observe execution without parsing statistics text | `ExecutionObserver`, `SolveMeasurements`, `SemanticOutcome` | [Observations and measurements](measurements.md) |
| Publish answers through a custom view | `zetesis_cli::{publish_prepared, PublicationConfig, AnswerRenderer}` | [Answer presentation](outcomes.md#replace-answer-presentation) |
| Reuse test, benchmark and presentation workflows | `zetesis_validation`, `zetesis_bench`, `zetesis_presentation` | [Command workflows](workflows.md) |

The `zetesis` facade re-exports canonical themelios types and forwards all nine
construction macros through its own runtime, including under a Cargo rename.
Its `Solver` supplies the native CPU backend through `zetesis-engine`. The solve
and query modules expose the upstream contracts and readings. Construction does
not establish native engine support; each grounding route retains its admission
limits.

`zetesis-themelios` admits source text and canonical logical programs. A native
session consumes prepared input and does not parse source or choose another
admission profile after a refusal. The upstream construction and analysis APIs
retain the contracts documented in the themelios manual.

## Logical values, candidates and relations

| Task | Public library entry points | Contract or example |
| --- | --- | --- |
| Construct finite relational programs | `zetesis_core::{Program, Template, Atom, Seed}` | [Parallel and lazy checking](parallel.md) |
| Share admitted template components | `TemplateCatalogBuilder`, `TemplateCatalog`, `TemplateRef`, `PatternRef`, `TemplateTerm` | [Canonical ownership](../architecture/ownership.md) |
| Read or explicitly export canonical values | `catalog::{AtomRef, PredicateRef, TermRef}`, `TermRef::nodes`, `TermRef::write_with`, `AtomRef::to_atom` | [Borrowing and explicit copies](models.md#borrowing-and-explicit-copies) |
| Build typed vocabulary without a program or atom population | `catalog::{VocabularyBuilder, Vocabulary}` | [Scoped term workspaces](models.md#scoped-term-workspaces) |
| Retain scoped bindings and selected roots | `catalog::{CatalogRead, TermRead, TermKey, TermAssignment, AssignmentSlice, TermSet}` | [Scoped term workspaces](models.md#scoped-term-workspaces) |
| Decide each predicate of one vocabulary once | `catalog::{CatalogRead::predicate_mask_with, PredicateMask}` | [Scoped term workspaces](models.md#scoped-term-workspaces) |
| Key values by canonical atom identity per owner | `catalog::AtomIdentityMap` | [Scoped term workspaces](models.md#scoped-term-workspaces) |
| Construct derived terms over immutable input owners | `catalog::{DerivedTerms, DeclaredConstructor}` | [Scoped term workspaces](models.md#scoped-term-workspaces) |
| Locate sparse carrier coordinates | `Program::locate_atom_with`, `CarrierAtom` | [Canonical ownership](../architecture/ownership.md) |
| Borrow candidate membership | `SeedSelection`, `SeedView`, `Candidates::next_selection` | [Parallel and lazy checking](parallel.md) |
| Locate an atom in the original gate carrier | `GateIndex::locate`, `Program::indexed_gate_atoms` | [Candidate identities](parallel.md) |
| Look up complete typed atoms | `Model::lookup`, `AtomIndex`, `AtomLookup`, `AtomPattern::key`, `BindingView` | [Checked lookup example](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/examples/README.md#checked-atom-lookup-probe) |
| Build an append-only atom catalog | `atom_interner::{AtomInterner, CommittedAtoms, AtomAppender}` | [Catalog construction](models.md#building-a-catalog-during-grounding) |
| Select equality matches in one relation | `zetesis_core::relation::{Relation, Query, Selection, Mask}` | [Core relation API](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/relation.rs) |
| Restrict rows and project finite domains | `zetesis_cpu::table::{Table, Domain, Selection}` | [Finite tables](finite-tables.md) |
| Inspect source domains and keyed relations | `zetesis_domain` | [Source preparation](source.md) |

Atoms retain signed predicates, argument order and typed values. Dense positions,
query masks and indexed tokens belong to their exact owner; equal lengths or
printed values do not make them interchangeable. Constructing a seed, finding a
row or locating a gate atom does not establish answer-set membership. See
[ownership and reuse](../architecture/ownership.md) for these boundaries.

## Membership and execution

| Task | Public library entry points | Guide |
| --- | --- | --- |
| Compile a complete relational graph | `zetesis_core::GroundProgram::compile` | [Parallel and lazy checking](parallel.md) |
| Check a normal program's reduct | `zetesis_cpu::{check, check_static, BatchOracle}` | [Parallel and lazy checking](parallel.md) |
| Reuse scalar query preparation | `PreparedQueries`, `PreparationLimits`, `ClosureWorkspace` | [Repeated checks](parallel.md#reuse-preparation-across-scalar-checks) |
| Build finite formulas and check membership | `zetesis_ferraris::{Theory, Node, Interpretation, check}` | [Finite reducts](reducts.md) |
| Reuse original truth or a frozen reduct | `EvaluationWorkspace`, `FormulaEvaluation`, `FrozenReduct` | [Finite reducts](reducts.md) |
| Use checked formula specializations | `zetesis_ferraris::{PositivePlan, TightPlan}` | [Formula plans](sessions.md#formula-membership-plans) |
| Cover and narrow candidate regions | `zetesis_cpu::regions::{Region, Traversal}`, `zetesis_ferraris::{Narrower, producers}` | [Exact execution](../architecture/execution.md) |
| Enumerate formula answers and complete reduct queries | `zetesis_sat::{StableModels, SearchMethod, check_with, PreparedReduct, ReductWorkspace}` | [Sessions](sessions.md) |
| Produce parallel candidate batches | `StableModels::with_region_producers`, `next_batch_with_completion` | [Formula plans](sessions.md#formula-membership-plans) |
| Share device resources | `zetesis_wgpu::GpuContext`, `GpuFormulaProfile`, `ExecutionResources` | [Device ownership](../architecture/ownership.md#device-resource-scope) |
| Select relation masks on a device | `zetesis_wgpu::GpuRelationExecutor` | [Relation selection](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/README.md#immutable-relation-selection) |

`PreparedInput` borrows a coherent admitted owner, native program or complete
ground graph. Keep the theory, atom catalog, objectives and source observations
from the same admission together. A supplied `GroundProgram` is reused; it does
not support a request to replace that graph with lazy grounding.

Prepared resources can share immutable data and reusable storage. Candidate
truth, search coverage and resource budgets remain local to the computation.
Device primitives have explicit capability, limit and failure contracts; enabling
a feature or selecting a device does not establish that work ran there. Every
answer set a session returns is decided by zetesis's own reduct check.

## Validation, measurement and presentation

| Task | Public library entry points |
| --- | --- |
| Compare the pinned corpus with clingo | `zetesis_validation::corpus_comparison::{Request, run_with_invocation}` |
| Check fixed backend examples through a command | `zetesis_validation::backend_check::{Request, run}` |
| Capture a bounded child process | `zetesis_validation::process::{Invocation, Limits, invoke_with_cancellation}` |
| Measure a bounded corpus schedule | `zetesis_validation::performance::{command::run, matrix::Request}` |
| Derive a compact campaign view | `matrix::Report::summary` |
| Compare retained measurements | `zetesis_validation::performance::series::{read_compare, compare}` |
| Render human tables with explicit styling | `zetesis_presentation::{Table, Row, Column, Layout, ColorMode}` |
| Check proof records and coverage policy | `zetesis_maintenance::{proofs, coverage}` |
| Execute and retain a fresh proof check | `zetesis_maintenance::proofs::capture::capture` |

The [workflow guide](workflows.md) explains typed requests, results and views.
Executable comparison workflows intentionally launch solver children; ordinary
embedded solving does not. Validation and maintenance are separate from the
semantic solver libraries. Record checks, physical tests and Lean proofs have
different scopes; none alone certifies the complete Rust/WGSL implementation.
See [validation](../reference/validation.md) and
[proof correspondence](../lean/correspondence.md) before extending those claims.

## API reference

Generate the [local Rust API reference](../../doc/zetesis_solve/index.html) using
[Building the documentation](../building.md). Public signatures document each
operation's ownership, cost, limits and errors. The
[solver source](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/lib.rs)
is also available online.

`zetesis::Solver` implements `zetesis::solve::contract::Backend` through the
CPU adapter in `zetesis-engine`. It connects canonical programs to complete
unscored enumeration and the agent/query APIs. The [adapter guide](agent.md)
records its supported capabilities. A general ASPIF importer and custom theory
propagation remain unsupported; native sessions provide GPU execution,
objectives and direct control of prepared input.

The two world-view types retain different contracts.
`zetesis::query::WorldView` is a nonempty live stream over a consistent result;
it may still be incomplete. Its materialized `Snapshot` is complete and nonempty.
`zetesis_solve::WorldView` retains the complete original family and can be empty
to establish inconsistency. The adapter exports full typed answers and preserves
incomplete-result evidence; query snapshots require a complete, nonempty family.
The native solver remains responsible for reduct checking and execution.
