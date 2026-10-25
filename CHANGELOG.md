# Changelog

Notable changes by release. Versions follow Semantic Versioning.

## Unreleased

### Added

- The regular benchmark corpus includes integrated Family Crossing, checking all
  eight optimal answers alongside the existing 94 cases.
- Requested GPU statistics include compute-pass time when the device supports
  timestamps, separately from host preparation and waiting. Each batch's time is
  measured from its own compute pass; the final batch is read once when the solve
  settles, and a solve that stops, or cannot read it then, reports it unresolved.
- The GPU library exposes workgroup sizing and optional changed-dependency
  propagation for controlled comparisons. Fitted workgroup sizes and dense
  propagation are the defaults.
- Requested GPU statistics report the workgroup size the device checks used.
- The GPU library can begin adapter enumeration ahead of the primitive that uses
  it (`GpuDiscovery`, which creates no device). Each solve-path primitive takes a
  `GpuDeviceSource` (`from_source`): its own selection, a selection over such an
  adapter enumeration, or an existing context. `ExecutionResources::discovering`
  retains an adapter enumeration for a session. Adapter choice, errors and
  timestamp requests are those of ordinary selection at the moment the
  enumeration began; the enumeration is a snapshot of the adapters present then.
- GPU formula sessions size their candidate slates, the candidates checked and
  committed together, adaptively by default. Slates start at the batch size,
  double after a filled slate whose answers leave the objective bound unchanged,
  and return to the batch size after a slate whose answers tighten it. Growth
  stops at the device's admitted prefix, never below the batch size, and at the
  pending-memory allowance. `GpuBatching::Fixed` keeps every slate at the batch
  size for controlled comparisons. Answer sets, optima and ties are unchanged.
- Requested GPU statistics report committed candidate slates, the largest slate
  and the slates that returned to the batch size on objective-bound feedback.
- `StableModels::bound_generation` reports how many objective bounds the
  candidate stream has installed.
- `StableModels::expect_candidate_bounds` declares, before the stream's first
  proposal, a caller that tightens candidate bounds as it commits answers, so
  GPU-route producers share one frontier for the whole stream; a started stream
  refuses the declaration with `Incomplete::LateCandidateBounds`.
  `RegionSearchStatistics::frontier_order` and the statistics JSON's
  `search.candidate_regions.frontier_order` report the stream's order.
- Requested GPU statistics report how tight support checking evaluated each
  candidate (complete, dense reuse or update) and whether the update route was
  available, with its reason when it was not, and whether any batch held the
  reuse authority the route needs.
- A maintained draw-poker workload checks complete enumeration against clingo,
  with portable filenames and a recorded 36-answer contract.

### Changed

- Numeric grounding joins combine certified divisibility and range filters before
  producing tuples; equal-size sources prefer the bindings needed by those filters.
- Repeated support checks consume initial support seeds directly after resetting
  their counters, avoiding a redundant queue round trip.
- Repeated support checks skip dependency paths already known false and reuse a
  compact list of affected heads. Packed explanations omit outputs already
  excluded from the candidate region.
- Possible-sum checks detect an allowed value while updating the packed sum set,
  avoiding a separate scan.
- Weighted aggregate comparisons can determine a condition when its opposite
  value leaves no possible remainder in the required interval. These deductions
  use the existing propagation queues and preserve answer-set semantics.
- GPU formula checking retains shared numeric aggregate rows instead of expanding
  them into Boolean formulas. Original satisfaction and reduct membership keep
  their existing meaning; unresolved device checks still receive exact CPU checks.
- GPU formula and tight checks choose each program's workgroup size from its
  dependency schedule: 128 or 256 invocations evaluate wide levels together,
  while serial and narrow schedules keep 64. Each solve compiles only the size it
  uses, when its first device check needs it. Answer sets are unchanged; bounded
  device checks still receive exact CPU checks.
- GPU tight checking can bring each candidate slot's previous evaluation to its
  next candidate when successive batches share a program and size, evaluating
  again only the formulas the change reaches; larger changes keep evaluating
  every formula. It engages only for programs whose dependency levels are wide
  enough that evaluating every formula takes several passes per level; other
  programs keep evaluating every formula. Each slot's retained state counts
  toward the batch's memory allowance, so adaptive slates can settle at a
  smaller device prefix, never below the batch size. Answer sets are unchanged.
- In the GPU library, tight checks report the work their route performed, and
  `TightGpuLimits` gains `min_update_candidates`. `TightGpuBatchStats` reports
  routes and the update route's availability, and
  `GpuTightOracle::UPDATE_ENGAGEMENT_STRIDES` names the strides per dependency
  level from which the route engages. The tight kernel requires eight storage
  buffers per shader stage.
- Command-line GPU solves begin adapter enumeration once their configuration is
  validated, while the program is read and admitted, and on the formula routes
  grounded, so the device is ready sooner; CPU solves start none.
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
- Candidate propagation reuses existing disjunctions to expose shared necessary
  conditions within rule bodies. Under an active restriction, split-order ties
  use the original program's dependencies. Answer-set semantics are unchanged.
- Candidate checks retain bounded explanations of missing support, allowing later
  regions to reuse deductions without repeating the support computation.
  Partially matched explanations can also determine a remaining atom before
  branching. Inactive ledgers avoid repeated statistics updates.
  Repeated support checks reuse relevant formula links, cyclic heads and fixed
  node roles; single-use theories retain their original traversal. Successful
  support cuts can also supply explanations for later regions.
  Original satisfaction and reduct membership remain unchanged.
- Stored support explanations use compact bit masks when they need less space,
  with unchanged deductions. Native count, sum and min/max conditions can supply
  support explanations. Identical conditions can share their outputs, and stronger
  explanations can replace weaker retained entries.
- Parallel region walks take their shared idle lock after publishing a region
  only when a worker is waiting for one. Answer sets are unchanged.
- GPU-route candidate producers keep a deque each, continue depth first on their
  own regions and steal a peer's oldest region only with an empty deque,
  holding one work lease per slate instead of one per region. A producer
  reserves a candidate's output slot only when it reaches a leaf, so the end of
  a slate no longer leaves one producer to find its last candidates. In a slate
  of more than one candidate a step under way when the slate fills finishes
  after it, so a step's stop, or a returned region's refused entry capacity,
  can then follow the filling emission; the slate's
  candidates are still checked and committed before the stop is reported. A
  leaf the slate cannot take is narrowed again by a later one, so the regions
  count can exceed the tree's regions. An
  optimizing search's producers instead share one deque and take the region
  published most recently, so regions far from the current work wait for
  tighter bounds. Proposal order remains schedule-dependent, and answer sets are
  unchanged.

### Fixed

- A default install no longer replaces the GPU-capable `zetesis` in Cargo's
  build output with the CPU front, so running that copy serves GPU requests. A
  front copied away from its install now explains that it is not an installed
  copy and points to the installed one, instead of advising a reinstall that
  cannot repair it.

### Compatibility

- `FormulaBatchStats`, `TightGpuBatchStats` and `FormulaExecutionStatistics`
  report the workgroup size each device batch dispatched. These statistics
  structs are now non-exhaustive: read their fields by name, and build a
  `FormulaExecutionStatistics` from `Default` before assigning its fields.
- GPU formula and tight graphs whose largest address lies within 256 of
  `u32::MAX` are refused with a capacity error, where 0.6.0 refused only those
  within 64, keeping room for the largest workgroup stride. Device buffer limits
  keep such graphs out of reach on the adapters measured.
- `zetesis_sat::Incomplete` gains `LateCandidateBounds`, so an exhaustive match
  on it needs an arm for the new variant.

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
