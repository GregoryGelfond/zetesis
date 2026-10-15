# Changelog

Notable changes by release. Versions follow Semantic Versioning.

## Unreleased

### Added

- Requested GPU statistics include compute-pass time when the device supports
  timestamps, separately from host preparation and waiting.
- The GPU library exposes checked workgroup choices and optional changed-dependency
  propagation for controlled comparisons. Dense propagation remains the default.

### Changed

- GPU formula checking retains shared numeric aggregate rows instead of expanding
  them into Boolean formulas. Original satisfaction and reduct membership keep
  their existing meaning; unresolved device checks still receive exact CPU checks.
- GPU tight checking evaluates independent formulas in parallel. General reduct
  propagation alternates directions through the same dependency schedule, carrying
  deductions between parents and operands; bounded checks retain exact CPU completion.
- GPU checks compute each shared aggregate row once when testing the original
  program, and skip backward scans when the inputs already determine the result.
- GPU candidate batches reuse host validation and upload storage, with retained
  capacity included in their existing memory allowances.
- Numeric GPU rows use narrower exact arithmetic when their bounds permit it.
  Rows certified to fit 32 bits also omit the unused high-value column.
  Supported devices can use 64-bit instructions and subgroup reductions; portable
  implementations remain available.
- Retained CPU aggregate checks update sums and counts from changed contributions.
  Shared Boolean evaluation stops reading operands once their result is known.
- Lazy constraint checks avoid revisiting the same binding through several newly
  held atoms or repeated body occurrences.
- Eligible grounding joins use completed numeric-domain analysis to filter bindings
  earlier, while retaining checked arithmetic and complete source validation.
  Certificates also cover bounded multiplication, division, absolute values and
  fixed nonnegative powers when every intermediate operation is defined.

## 0.6.0 — 2026-10-14

### Added

- `zetesis solve --count` prints the complete answer-set count, or the number of
  optimal ties. It ignores `#show` and `#project` and prints no total if the
  search is interrupted. See [answer selection](docs/book/reference/commands.md#select-answers).
- The default installer supplies a CPU executable and a separate GPU executable.
  GPU requests transfer to the latter; statistics identify the executable that
  ran. See [installation](INSTALL.md#what-gets-installed).
- Benchmark campaigns can use declared external source bundles, preserving their
  original files and include graph and checking complete results against clingo.
  See [external bundles](docs/book/reference/benchmarking.md#declared-external-bundles).

### Changed

- Everyday help presents the common CPU/GPU choices; `--help-all` retains the full
  reference, including Metal and Vulkan. The default memory allowance is half the
  memory available to the process, including a lower Linux container limit;
  two gibibytes remains the fallback when that information is unavailable.
- Aggregate assignments use the values their required and possible tuples can
  produce when checking arithmetic admission. This admits programs such as
  `p(M+1) :- M = #min{1; 5}.` and refuses families whose arithmetic is undefined
  for every proposed value. See [numeric boundaries](docs/book/reference/language.md#numeric-boundaries-and-refusal-meaning).
- Ordinary CPU solving uses shared native operations for `#count`, nonnegative
  `#sum`, `#min` and `#max` comparisons, reusing contributions and deductions
  across eligible shared Boolean conditions. See [aggregate representation](docs/book/architecture/alignment.md#ordered-comparisons-and-objective-reductions).
- Grounding reuses fact tables, join plans, body atoms and aggregate bounds, skips
  unchanged producer traversals, and uses checked integer comparisons and indexed
  arithmetic filters. Source identities and arithmetic failures are preserved.
  See [grounding](docs/book/architecture/grounding.md).
- Numeric analysis shares one engine across source and prepared input, with
  argument bounds and an optional diagnostic census. The census does not enable
  further pruning. See [numeric analysis and domain guards](docs/book/rust/source.md#optional-numeric-analysis-and-domain-guards).
- CPU candidate checks reuse prior evaluations, candidate search prefers atoms
  from choice heads, and parallel region walks delay starting their workers.
  Eligible acyclic normal programs can be evaluated during preparation.
  Answer sets and optima are unchanged; search order and bounded work can differ.
  See [membership plans](docs/book/rust/sessions.md#formula-membership-plans) and
  [parallel walks](crates/zetesis-sat/README.md#walking-the-tree-with-several-workers).
- Metered operations check cancellation at entry, exit, resource refusals and
  256-unit work boundaries. Work and storage receipts reflect the new execution
  routes, so resource ceilings can be reached at different points. See
  [resource contracts](docs/book/rust/outcomes.md#resource-contracts).
- The final 94-program CPU check records 521 ms in summed command time at
  fourteen threads, against clingo’s 804 ms. This is about 33% below the earlier
  recorded 0.5.0 result. See the [measurements and method](docs/book/reference/cpu-corpus-final-20261014.md).

### Fixed

- Closed region walks remain closed on later pulls without starting workers.
- Parallel region statistics report retained countermodel workspace peaks.
- The macOS installer preserves the previous installation if inspection of a new
  executable fails.

### Compatibility

- `Program::new` now takes `FactTables`; `rules()` includes facts, while
  `templates()` contains only explicit templates. `AdmissionError::Canonical`
  replaces `template` with `origin: AdmissionOrigin`, and matches must handle
  `FactLimitExceeded`. See [explicit facts](docs/book/rust/parallel.md#construct-templates-and-explicit-facts).
- `DomainObservation::Analyzed` carries `NumericAnalysis`; `Inapplicable` is
  removed. Use `analysis()`, `catalog()` and `arguments()` to read the new view.
  Observers can decline census work with `domain_diagnostics_enabled()`, whose
  default preserves existing callbacks. See [domain observation](docs/book/rust/source.md#optional-numeric-analysis-and-domain-guards).
- `Knowledge::preferred_atom_reserved` takes `ChoiceHeads`; pass an empty value
  for the previous ranking. `RegionSearchStatistics` gains `workers` and is
  non-exhaustive; construct it through `Default`. New worker and engagement
  records are also non-exhaustive.
- Exhaustive matches must handle `RunError::GpuExecutableUnavailable`, the
  corresponding benchmark `Decision`, and `curated::Error::Irregular` and
  `examples::Error::Irregular` in `zetesis-validation`.
- Removed the GPU `new_metal` constructors and
  `GpuFormulaOracle::new_metal_with_projection`. Use `new_selected` or
  `new_selected_with_projection` with `GpuSelection { api: GpuApi::Metal }`.
  Other removed/private helpers and the new numeric `NodeView` and admission
  error variants are listed in the [library migration guide](docs/book/rust/libraries.md#migrating-from-050).
- Diagnostic bytes are unchanged, but injected writers now receive complete
  records and styled diagnostic lines through `write_all`. A record or statistics
  report whose formatting fails returns an I/O error instead of panicking.

## 0.5.0 — 2026-10-10

### Added

- Optimization with lazy grounding, including all optimal ties and explicit
  incomplete results when execution is interrupted.
- GPU execution with hybrid lazy grounding. The device checks the formula core;
  the host checks streamed constraints before returning or scoring an answer.
- Direct evaluation of eligible stratified normal programs.
- Lean laws for retaining completed constraint checks as candidate bounds change,
  with updated implementation correspondence.

### Changed

- Use streamed constraints and required choices to narrow candidate search and
  bound objective costs, preserving original-program satisfaction and reduct
  membership.
- Share objective preparation and retain checking workspaces across candidates,
  GPU batches and successive answers. Reduce repeated relation scans and answer
  reconstruction.
- Compact propagation state and avoid redundant deductions, allocation and
  statistics synchronization while keeping parallel workers independent.
- Use checked arithmetic bounds to enable incremental lazy checks for eligible
  sums and differences, preserving arithmetic diagnostics.
- Update the manual, API migration guidance and reproducible eager/lazy timing
  comparisons with clingo.

### Fixed

- Remove fixed internal size ceilings from tight and positive-program certificate
  preparation; the session's resource allowances govern preparation instead.
- Settle parallel worker statistics before reporting interrupted execution,
  preserving its original reason and incomplete status.

### Compatibility

- The `zetesis` facade API is unchanged: it provides CPU enumeration without
  objective scoring or projection. Optimization and GPU execution remain
  available through native sessions and the CLI.
- `RegionFilterWorker` now requires `Send`.
- Remove the obsolete `HybridFeature::Objectives` and `SolveError::HybridBackend`
  refusals, including the CLI's JSON `hybrid_backend` error kind.
- `ConstraintAllowance::statistics()` reports settled charges. Live readings
  may omit an active check; totals are exact after checks settle, up to counter
  saturation.

## 0.4.0 — 2026-10-07

### Added

- Ordinary and nested Mastermind scalability examples, including six positions
  and eight colours. The maintained scalability suite has fourteen workloads,
  or fifteen with Einstein.
- `zetesis_solve::Resources`, which derives source, solving and output capacities
  together for Rust applications. The CLI and `zetesis` API use the same policy.
- Finite semantic laws for grouped formulas, shared aggregate dependencies and
  support-condition regrouping. Their source and implementation obligations
  remain explicit in the [proof correspondence](docs/book/lean/correspondence.md).

### Changed

- Ordinary solving uses `--threads`, `--memory` and optional `--time-limit`.
  Grounding and search no longer stop at selected cumulative operation counts.
  Work remains visible in statistics. Storage checks, cooperative interruption
  and checked representation limits remain; the memory allowance is not a
  process-wide resident-memory cap.
- Reuse finite grounding work across aggregate assignments with identical
  inputs, structured-term queries and applicable recursive support rounds.
  Derived heads share catalog preparation, and normal rules reuse matched body
  prefixes while preserving every producer condition and source origin.
  Support indexes retain only the columns that joins can read.
- Store conjunctions and disjunctions as native groups, retaining shared
  subformulas. Grounding shares repeated support conditions and completed
  validation. Original satisfaction and the frozen reduct remain the criteria
  for answer-set membership.
- Reduce CPU search memory and repeated propagation through compact region
  state, reusable narrowing worklists and shared immutable indexes. Build the
  original narrowing index only when search needs it. Narrowing reserves work
  in bounded groups while retaining cooperative control and exact work receipts.
- Extend lazy CPU formula grounding to reconstruct eligible terminal definitions
  from each accepted base answer, alongside streamed constraints. These are
  derived predicates the remaining program does not read. Returned answers
  contain the full original atoms, independently of `#show`.
- Share constraint and reconstruction preparation across candidates and answers,
  and release support indexes that a hybrid core no longer needs. Explicitly
  bounded library checks, reconstruction and model construction use per-check
  or per-answer allowances, with cumulative statistics.
- Stream JSON atom identities without retaining complete prior answers merely
  to name their atoms. Statistics include coordinator work and the actual
  retained graph, model and checker storage; text statistics are written in one
  operation.
- Source and eager-program preparation now observe cooperative cancellation.
  Located source-admission errors include the file, line, column and excerpt.
  Update themelios parsing and lowering while preserving original occurrences,
  counted-element identity and provenance.

### Fixed

- Count formula column-demand metadata once, instead of again for every
  published support row.
- Check retained reduct-workspace storage before returning a verdict, including
  scratch grown during the query. Failed queries retain their original cause
  and the resources already used.

### Compatibility

- Remove solver `--max-*` resource flags, `--completion-workers`, `--batch-size`
  and `--gpu-formula-work`/`--gpu-formula-rounds`. Use the ordinary controls above.
  `--workers` and `--memory-budget` remain aliases. Benchmark requests use the
  same controls; historical reports retain their original settings and meaning.
- Rust applications must update the `zetesis` API's resource configuration,
  replace `Grounder::Hybrid` with `Grounder::Lazy`, and migrate direct formula
  construction to paired node/operand storage. Terminal-base and narrowing
  interfaces also change. See [Migrating from 0.3.0](docs/book/rust/libraries.md#migrating-from-030).
  Explicit bounded operations remain available in the underlying libraries.
- Lazy formula execution remains CPU-only and refuses objectives and table
  joins. Formula GPU execution needs an eager theory; host search and exact CPU
  completion remain part of its general route. A requested device is honored or
  refused. The `zetesis` facade remains CPU-only and enumerates unscored,
  unprojected answer sets; native sessions retain optimization and GPU execution.
- Direct themelios dependencies must match the workspace revision, or use
  zetesis's canonical reexports. Work counts, formula shapes and retained-byte
  receipts can change; they are not compatible performance units across releases.

### Security

- Read macOS host memory through `/usr/sbin/sysctl` by its absolute path, so an
  earlier executable on `PATH` cannot run or change the default allowance.

## 0.3.0 — 2026-10-04

### Added

- A `zetesis` Rust API for constructing programs with typed values or ASP macros,
  enumerating answer sets, and querying them. Checked examples show both
  construction styles.
- Brave and cautious consequences, computed by complete enumeration of full
  answer sets, including atoms hidden by `#show`.
- Queries using **Gelfond–Kahl three-valued semantics**: `Yes`, `No` and `Unknown`.
  Incomplete execution and inconsistency produce typed errors, not logical
  `Unknown`.
- Knowledge-base assertion and retraction, reusable answer-set snapshots,
  cooperative time budgets and cancellation. Each new question rebuilds from
  the current program.
- Lean results for finite reduct checking, normal-program closure, formula
  admission and packed interpretations, with implementation correspondence for
  the scalar reference checker under explicit library contracts. Grounding,
  full enumeration and GPU execution remain outside that verification.

### Changed

- Parsed and constructed programs share preparation without rendering and
  reparsing. Errors retain the original statement and available source location.
- Boolean choice elements count separately after pool expansion.
  `{ #true : p(1;1) } = 2. p(1).` now has answer set `{p(1)}`. Multiple
  grounding witnesses of one expanded element still share its key.
- Simplify the reference checker's admission, subset traversal and interpretation
  construction while preserving their resource checks and failure behavior.

### Compatibility

- The new `Solver` uses CPU execution with configurable grounding and workers.
  It enumerates unscored, unprojected answer sets: objectives and `#project` do
  not restrict enumeration; `#show` controls display. Native sessions retain
  optimization and GPU execution.
- `AdmittedFormula::source()` and `PreparedFormula::source()` return
  `Option<&Source>`. Formula origins and errors use `ProgramSite`, with optional
  source coordinates for constructed input. Grounding observers and verdict
  fields follow the same convention; `FormulaWarning::diagnostic()` replaces
  its `ToDiagnostic` implementation.
- `ExpansionFailure` adds `Interrupted`. `FormulaFailure` adds `Program` and
  `Logical` and removes `ChoiceSource`. Exhaustive matches need updating.
- Direct themelios dependencies must use the same revision as zetesis, or use
  the canonical types reexported by `zetesis`.

## 0.2.0 — 2026-10-01

### Added

- Single comparisons in rule heads, such as `X=Y :- p(X), p(Y).`.
  Head equality tests values; it does not bind variables.
  Conditional and compound comparison heads remain unsupported.
- A curated Sudoku example for tests and benchmarks, runnable within default
  resource limits.
- Rust APIs for reusing aggregate and objective preparation, replacing
  optimization bounds, and reading compact interpretations.

### Changed

- Reuse grounding preparation and arithmetic results across eager and hybrid
  execution.
- Infer tighter argument domains from positive rule bodies and reduce repeated
  aggregate compilation.
- Simplify candidate generation for eligible positive formulas. Reduct-based
  answer-set checking is preserved.
- Reduce memory used during search and copying before GPU checks.
- Reuse objective preparation and discard superseded bounds in the default
  search. Count preparation and work completed before interruption toward
  objective-work limits. Default resource limits are unchanged.

### Fixed

- Keep accurate objective-work statistics before the first answer is retained and
  when an output callback fails.
- Enforce shared work limits before grounding-table operations; retain
  completed-work counts when interrupted.

### Compatibility

- Exhaustive Rust matches on `zetesis_solve::Interruption` must handle the new
  `PreparedObjective` variant.
- JSON interruption records can report `prepared_objective`. Statistics include
  `objective_work`, which is `null` when that measurement is unavailable.
- `--max-objective-bound-work 0` disables pruning while allowing objective
  evaluation.
- Library callers replacing an optimization bound must ensure that the new
  bound is at least as restrictive as the old one; the library does not verify
  this requirement.
