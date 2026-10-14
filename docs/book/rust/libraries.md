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
| Choose extended or formula admission once per input door | `ParsedSource::admit_automatic_with_cancellation`, `admit_bundle_automatic_with_cancellation`, `prepare_program_automatic`, `AutomaticAdmission`, `AutomaticAdmissionFailure` | [Parsed-source transfer](source.md#reuse-original-input-across-profiles) and [logical programs](source.md#admit-an-existing-logical-program) |
| Admit a logical program under S0 | `zetesis_themelios::{admit_program, ProgramAdmissionOptions, AdmittedProgram}` | [Typed program admission](source.md#borrow-the-strict-relational-input) |
| Configure ordinary resource policy | `zetesis_solve::Resources` | [Sessions](sessions.md) |
| Solve admitted input | `zetesis_solve::{PreparedInput, Session, SessionBuilder, SolveConfig}` | [Sessions](sessions.md) |
| Count full answer sets or optimal ties | `zetesis_solve::{SessionBuilder::counting, SemanticOutcome::complete_answer_count}` | [Counting](outcomes.md#replace-answer-presentation) |
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
| Construct finite relational programs | `zetesis_core::{Program, Template, FactTables, FactSegment, Atom, Seed}` | [Parallel and lazy checking](parallel.md) |
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
| Look up complete typed atoms | `Model::lookup`, `AtomIndex`, `CatalogIndex`, `AtomLookup`, `AtomPattern::key`, `BindingView` | [Checked lookup example](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/examples/README.md#checked-atom-lookup-probe) |
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
| Build finite formulas and check membership | `zetesis_ferraris::{Theory, FormulaParts, NodeView, Interpretation, check}` | [Finite reducts](reducts.md) |
| Reuse original truth or a frozen reduct | `EvaluationWorkspace`, `FormulaEvaluation`, `FrozenReduct` | [Finite reducts](reducts.md) |
| Use checked formula specializations | `zetesis_ferraris::{PositivePlan, StratifiedPlan, TightPlan, TightWorkspace}` | [Formula plans](sessions.md#formula-membership-plans) |
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

## Migrating from 0.5.0

The changes below affect direct library callers. Source syntax is unchanged;
aggregate admission and execution/resource behavior are described in the
[language reference](../reference/language.md#numeric-boundaries-and-refusal-meaning)
and [resource contracts](outcomes.md#resource-contracts).

| Previous use | Migration |
| --- | --- |
| `zetesis_core::Program::new(templates, limits)` | Pass `FactTables::default()` as the second argument, or supply explicit facts. Use `rules()` for all original rule occurrences and `templates()` for the separate template sequence. See [explicit facts](parallel.md#construct-templates-and-explicit-facts). |
| Matching `zetesis_core::AdmissionError` | Replace `Canonical { error, template }` with `Canonical { error, origin }` and handle `FactLimitExceeded`. `template_index()` and `fact_index()` resolve their separate input sequences; see [explicit facts](parallel.md#construct-templates-and-explicit-facts). |
| Matching `DomainObservation::Inapplicable` or reading its former `Analyzed(&Analysis)` payload | Remove the `Inapplicable` arm. `Analyzed` carries `NumericAnalysis`: use `analysis()` for bounds/statistics, `catalog()` for canonical keys, and `arguments()` for predicate coordinates. Finite-guard applicability is separate; see [domain observation](source.md#optional-numeric-analysis-and-domain-guards). |
| Exhaustive `zetesis_ferraris::NodeView` matches | Handle `Threshold` and `ExtremumCut`. Formula storage now includes shared numeric arenas; preserve them when copying. See [finite formulas](reducts.md#numeric-formula-storage). |
| Exhaustive `zetesis_ferraris::AdmissionError` matches | Handle `ThresholdShape`, `ThresholdMeasure`, `NumericShape`, `NumericDomain`, `OrdinalRank` and `OrdinalCut`. These distinguish numeric shape/measure refusals from allocation and configured limits. |
| Calling `zetesis_ferraris::Knowledge::preferred_atom_reserved` | Pass `&ChoiceHeads` before cancellation. `ChoiceHeads::default()` preserves the previous ranking; `Extraction::choice_heads` supplies the extracted choice heads. `OriginalSubject::with_choices` selects the same ranking for narrowing that chooses its own split. |
| Constructing or destructuring `zetesis_sat::RegionSearchStatistics` | It now has `workers` and is non-exhaustive. Construct with `Default::default()` and assign fields; destructure with `..`. The new `RegionWorkerStatistics` is also non-exhaustive. |
| Selecting or matching worker engagement | `StableModels::with_region_workers_engaging` accepts `Engagement`; `Immediate` starts workers at the first pull. `Engagement` and the reported `RegionWalkEngagement` are non-exhaustive, so external matches need a fallback arm. See [parallel walks](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-sat/README.md#walking-the-tree-with-several-workers). |
| Exhaustive `zetesis_cli::RunError` or `zetesis_validation::performance::matrix::Decision` matches | Handle `GpuExecutableUnavailable`, meaning the installed CPU front could not run its GPU executable. This remains separate from unavailable compiled backend support. |
| Exhaustive `zetesis_validation::curated::Error` or `zetesis_validation::examples::Error` matches | Handle `Irregular(PathBuf)` for a resolved input that is not a regular file. These cases previously used `Path`; that variant still reports paths outside the selected root. |
| Injecting a diagnostics writer whose behavior depends on write-call boundaries | Records and styled diagnostic lines now reach `write_all` as complete units. Bytes are unchanged; see [writer boundaries](outcomes.md#diagnostic-writer-boundaries). |

`GroundingObserver::domain_diagnostics_enabled()` defaults to true, preserving
existing domain callbacks. An observer may return false to omit the callback,
integer-certificate requests and census work; this does not disable the shared
analysis or finite-domain guards. Delegating observers can delegate that request
alongside the callback. See [domain observation](source.md#optional-numeric-analysis-and-domain-guards).

### Removed and private helpers

Replace `zetesis_wgpu::{GpuOracle, GpuTightOracle, GpuFormulaOracle}::new_metal`
with `new_selected(options, GpuSelection { api: GpuApi::Metal })`.
For `GpuFormulaOracle::new_metal_with_projection`, use
`new_selected_with_projection` with the same selection and projection.
`zetesis_core::catalog::CatalogRead::constructor_of_predicate` is removed.

The following helpers were public in 0.5.0 and are now crate-private:

| Crate | Helpers |
| --- | --- |
| `zetesis-cli` | `RunFailure::into_cause` |
| `zetesis-core` | `AtomKey::compare_ref`, `AtomRef::compare_key_with`, `Model::selection_payload_bytes`, `Predicate::shares_name` |
| `zetesis-cpu` | `CONSTRAINT_HEAD`, `InstanceAtomIter` |
| `zetesis-engine` | `OutputLimits::for_resources` |
| `zetesis-maintenance` | `is_record` |
| `zetesis-sat` | `CompletionExecutor::DEFAULT_SCRATCH_BYTES`, `ReductPreparationLimits::DEFAULT_BYTES` |
| `zetesis-themelios` | `SourceBundle::working_directory`, `Encoded::into_text`, `ProgramSite::with_location`, `ProgramSite::with_statement` |
| `zetesis-validation` | `Contract::with_witnesses`, `Contract::optimal_family`, `Capture::stderr_text`, `process::invoke_supervised_with_cancellation` |
| `zetesis-wgpu` | `GpuLazyOracle::check_batch_with_source`, `GpuTightOracle::from_context_with_support` |

## Migrating from 0.4.0

Lazy formula grounding now accepts objectives and GPU backends. The `zetesis`
facade remains CPU enumeration of unscored, unprojected answer sets; these
additions belong to the native session API and CLI.

| Previous use | Migration |
| --- | --- |
| `zetesis_sat::RegionFilterWorker` with thread-local scratch | The worker must implement `Send`: retained checkers can move between executor threads. Each checker remains exclusive to its enumeration. This callback is not part of the `zetesis` facade. |
| Matching `HybridFeature::Objectives` | Remove that arm; lazy formula admission now supports objectives. The `TableJoins` refusal remains. |
| Matching `SolveError::HybridBackend`, its CLI counterpart or JSON `hybrid_backend` | Remove that obsolete refusal. Hybrid sessions accept GPU backends; device availability and resource failures remain typed errors. |
| Reading `ConstraintAllowance::statistics()` during an active check | Treat each field as settled charges, which may omit ongoing work. Fields are read independently. Totals are exact after operations settle or workers join, up to saturation; check-local and failure statistics remain exact. |

## Migrating from 0.3.0

Version 0.4.0 changes resource configuration and several direct library interfaces.
The [command guide](../reference/commands.md#existing-scripts) covers retained CLI
aliases; removed per-stage controls have no hidden replacement flags.

For ordinary execution, construct `zetesis_solve::Resources` from memory and
workers, then derive source, solve and observation settings together. The
[`Resources` example](sessions.md#embedding-an-ordinary-solve) shows the common
preparation/session path. Work remains checked statistics, and named memory
capacities are not an aggregate process-memory guarantee. Direct primitive
limits remain available for an explicitly bounded operation.

| Previous use | Migration |
| --- | --- |
| `zetesis::Config::{admission, expansion, formula, output}` | Configure `grounder`, `workers` and `memory`; the adapter derives preparation, solve and output settings together. For individual bounds, use the underlying preparation/session APIs. |
| `zetesis::Grounder::Hybrid` | Use `Grounder::Lazy`. Relational admission still takes its source-join route; unsupported relational constructs can take the hybrid formula route. Other refusals are returned. |
| `Theory::new` over `Vec<Node>` and matching binary `Node` variants | Supply `FormulaParts` containing nodes and their operand arena. Inspect `theory.view().node(index)` as `NodeView`; `And` and `Or` expose complete slices. Raw pairs use `Node::and_pair`/`or_pair`; wide rows use checked arena spans. |
| Aggregate append functions over a node vector | Use the paired `FormulaNodes` owner. Transactions commit, roll back or detach both suffixes together. `into_parts` discards topology evidence; `prepare_admission` consumes it for checked final admission. |
| Complete literals of formula/aggregate or expansion limits | Include the new operand ceilings and `ExpansionLimits::max_family_bytes`. Exhaustive failures must cover operand-span/arity and materialized-family refusals. Node count alone no longer describes graph size. |
| `TerminalFormula::base_theory() -> &Theory` | It returns `Option<&Theory>`. Match `base()` for eager versus streamed bases; a hybrid core's proposals must pass its constraints before reconstruction. Terminal observations add `base` and `streamed`, and grounding-mode matches include the hybrid terminal route. |
| Narrowing with separate theory/producer/truth arguments and per-read callbacks | Pass `OriginalSubject` or `FrozenSubject` and reusable `NarrowingScratch`. Quota variants use `NarrowingQuota`; `narrow_known_metered` and `narrow_frozen_known_metered` are removed. |
| `AtomTable::index` as an identity-recording operation | It performs structural lookup; the record encoder owns identity recording. `AtomIdentityMap::retain_held` releases unneeded owners. |
| Native benchmark execution fields for workers, completion, batch and work caps | Use `NativeExecution::{threads, memory_bytes, time_limit_seconds}` with algorithm choices. New records identify the requested ordinary policy; historical report readers retain earlier fields. |

Formula work includes logical operand occurrences, even inline pairs. Reused
validation omits only scans actually avoided; raw input still receives complete
admission. See [finite reducts](reducts.md) and the
[paired aggregate contract](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-ferraris/README.md#shared-comparison-families).
Narrowing's quota is consulted at most `NARROWING_BATCH` charged reads apart
(currently 256); exact work refusal and refunds remain part of its contract.

Constraint and terminal-reconstruction allowances now bound each check or answer,
while their statistics accumulate. `SolveConfig::max_model_work` likewise bounds
each verified model's construction, with a separate allowance of the same size
for semantic-order preparation. Code interpreting these receipts must preserve
the distinction between a single operation and the session total.

The shared themelios revision is
`4c163d0d07cf67180354d9b605e19df2d355529e`. Align direct dependencies with it or use
`zetesis`'s reexports; separate revisions provide distinct Rust types. Parsed and
constructed inputs retain their logical/source identity contracts. These API and
representation changes do not extend the proof boundary to all source grounding,
parallel enumeration or GPU execution.

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
