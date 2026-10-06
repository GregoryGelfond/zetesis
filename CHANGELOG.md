# Changelog

Notable changes by release. Versions follow Semantic Versioning.

## Unreleased

### Added

- `AtomCatalog::same_atom_owner` says whether two catalogs' atoms share one
  atom owner, as one writer's published selections do; `same_owner` remains
  the catalog-allocation identity.
- `zetesis-ferraris` narrowing accepts a `NarrowingQuota` through
  `narrow_known_reserved` and `narrow_frozen_known_reserved`, which reserve
  work permits in batches of at most `NARROWING_BATCH` and refund the unspent
  ones. Work-limit stops and work receipts are those of per-read charging.
  Cancellation and deadlines are now observed at each reservation, so at most
  `NARROWING_BATCH` charged reads apart, where they were observed before every
  read.

### Changed

- `--json` output of runs whose answers each bring their own atom owner, such
  as answers with reconstructed terminal definitions, no longer slows
  quadratically with the number of answers.
- `AtomTable::index` is a structural lookup and no longer records atom
  identities; the record encoder's own lookup keeps the identity cache.
- Region narrowing queues an atom's support recheck at most once until it
  runs, since its supporters only fall as knowledge grows and one recheck with
  the later knowledge derives what the dropped ones would have. Narrowing
  results are unchanged; the propagation count and charged work in the
  narrowing receipt, `regions.work` and the search work are lower, so under a
  fixed work limit some runs complete where 0.3.0 stopped.
- Region narrowing keeps its worklists in a `NarrowingScratch` that each
  walker passes to every narrowing and reuses, instead of in each region's
  `Knowledge`; every `zetesis-ferraris` narrowing entry point takes it, and
  takes what it reads as an `OriginalSubject` or a `FrozenSubject` in place of
  the separate theory and producer or truth arguments. A knowledge no longer
  carries worklist headers or capacity: the producer route's frontier
  `retained_bytes` and `peak_retained_bytes` fall by 72 bytes per knowledge
  slot on 64-bit hosts, plus the worklist capacity a moved knowledge carried.
  Each reduct workspace's retained bytes now include its scratch.
- Region narrowing knowledge chooses its counter width once per value and its
  header is 16 bytes smaller on 64-bit hosts. The producer route's frontier
  `retained_bytes` and `peak_retained_bytes` in `--stats` count each queued
  region's knowledge vector by slot capacity, so they fall by 16 bytes per slot;
  counter payloads are unchanged.
- Both `--stats` views, records (`--json --stats`) and human (`--stats`), are
  rendered whole and written to standard error at once, instead of one write
  per fragment. The report's text and its position among other diagnostics are
  unchanged.

### Removed

- `zetesis-ferraris` `Narrower::narrow_known_metered` and
  `narrow_frozen_known_metered`, the per-read charge entry points, which no
  caller uses since narrowing reserves work through `NarrowingQuota`. A per-read
  charge is a quota granting one permit per reservation.

### Security

- On macOS, the host-memory reading behind the default resource allowance runs
  `/usr/sbin/sysctl` by its absolute path instead of resolving `sysctl` through
  `PATH`, so a program earlier on `PATH` can neither run nor change the reading.

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
