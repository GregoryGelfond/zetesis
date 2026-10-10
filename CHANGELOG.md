# Changelog

Notable changes by release. Versions follow Semantic Versioning.

## Unreleased

### Added

- Extend the Lean laws for preserving completed constraint scans when
  candidate bounds change.
- Evaluate eligible stratified normal programs directly, checking all original
  constraints before publishing their unique answer.
- Support objectives with lazy formula grounding. Check the original
  constraints before scoring an answer or updating a bound, and preserve
  optimal ties and explicit interruption outcomes.
- Allow GPU checking with hybrid lazy grounding. The selected device checks
  the retained formula core; host checks establish satisfaction of streamed
  constraints before an answer is scored or returned.
- Derive candidate consequences from streamed integrity constraints before
  splitting. Source deductions and formula propagation share a fixed point;
  final answer acceptance still checks original satisfaction and the reduct.

### Changed

- Retain CPU tight-checking storage between candidates, with independent
  workspaces for parallel workers.
- Retain source-checking workspaces between GPU candidate batches and
  serial answer pulls.
- Use smaller propagation counters when the program's bounds permit them.
- Skip propagation rounds that have already reached a shared fixed point.
- Reuse prepared lazy constraint checks between compatible candidates, and
  revisit only affected atom occurrences when complete coverage is established.
  Keep completed checks when positive body atoms only become false.
- Filter structurally impossible or absent positive atoms before constraint
  joins, while preserving complete body checks and arithmetic diagnostics.
- Remove repeated witnesses from constraints over fixed fact relations, and
  construct possible count values directly without changing tuple identity.
- Reduce repeated formula propagation, support scans and split ranking while
  preserving candidate coverage and reduct membership.
- Stop propagation when a region is proved contradictory, without visiting
  the remaining operands or parent formulas.
- Store candidate knowledge together and reuse borrowed terms and cancellation
  flags, reducing allocation and ownership overhead while keeping workers independent.
- Reconstruct terminal answers from authenticated base identities without
  repeating tuple lookup or atom ordering; displayed output remains separate.
- Accumulate constraint-check statistics locally and publish them at preparation
  and check boundaries, preserving resource limits and accepted failure receipts.

### Fixed

- Prepare tight and positive-program certificates within the session's memory
  and search allowances. Fixed internal size limits no longer force otherwise
  eligible programs onto general reduct checking.
- Settle parallel worker statistics before returning a stopped session's
  report, preserving its original interruption reason and incomplete coverage.

### Compatibility

- The internal `zetesis-sat` callback `RegionFilterWorker` now requires `Send`,
  allowing a retained checker to move between executor threads. The `zetesis`
  facade API is unchanged.
- Remove `HybridFeature::Objectives`: lazy formula grounding now admits
  objectives. Rust callers matching this former refusal can remove that arm.
- Remove `SolveError::HybridBackend`, its CLI counterpart and the JSON
  `hybrid_backend` error kind: hybrid sessions now accept GPU backends.
  Device availability and resource failures retain their existing typed errors.
- `ConstraintAllowance::statistics()` now reports settled charges: live readings
  can omit charges from an active check. Shared totals are exact after all
  operations settle, up to counter saturation; local and failure receipts remain
  exact.

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
