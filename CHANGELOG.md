# Changelog

Notable changes by release. Versions follow Semantic Versioning.

## Unreleased

### Added

- `PreparedFormula::ground_lazy` and `PreparedFormulaBundle::ground_lazy` (and
  their `_with_observer` forms): lazy materialization defers certified terminal
  definitions, as adaptive materialization does, over a hybrid base whose
  producer core is instantiated and whose eligible integrity constraints are
  streamed. `TerminalFormula::base_kind` and `TerminalFormula::base` (`BaseKind`,
  `TerminalBase`) say which base an owner has; a session runs a hybrid terminal
  base under automatic or lazy grounding on the CPU, refusing an eager request,
  and checks each core answer before it is reconstructed. `ExecutionObservation::TerminalDefinitions` reports the base
  kind and its streamed constraints (`StreamedConstraints`), and telemetry adds
  `GroundingMode::HybridBaseTerminalDefinitions` with `TerminalBaseMark`.
- `StreamedCore`, reached through `HybridFormula::core`, is the producer core
  with the integrity constraints streamed over it: its theory, atoms, analysis
  and constraint checkers. A hybrid owner's checkers and candidate-region filter
  run over it, so the same core can serve a terminal owner's base.
- `AtomIdentityMap::retain_held` drops the owners that only the map still
  holds, whose atoms can no longer be presented, and `owners` counts them. The
  map now finds an owner by hashing its identity rather than by a linear scan.
- `ClosedCatalog::read` borrows a closed catalog as its writer read it at the
  close: the same atom and vocabulary scopes and every canonical row, so that
  writer's relation memberships bind to it and another writer's are refused.
  `vocabulary_read` is unchanged.
- `zetesis-ferraris` narrowing accepts a `NarrowingQuota` through
  `narrow_known_reserved` and `narrow_frozen_known_reserved`, which reserve
  work permits in batches of at most `NARROWING_BATCH` and refund the unspent
  ones. Work-limit stops and work receipts are those of per-read charging.
  Cancellation and deadlines are now observed at each reservation, so at most
  `NARROWING_BATCH` charged reads apart, where they were observed before every
  read.

### Changed

- `--grounder lazy` defers certified terminal definitions over its hybrid
  formula base, reconstructing them from each accepted answer, as `auto` does
  over an eager base; `zetesis_solve::ground_formula` and `ground_bundle` own the
  grounder-to-materialization mapping the CLI and the `zetesis` facade now use,
  and the facade's `Hybrid` grounder takes the same lazy route. `--stats` and
  JSON report a hybrid terminal base (`hybrid_base_terminal_definitions`, search
  scope `terminal_retained_core`, `terminal_execution.base`), and validation
  reconciles its two receipts.
- `TerminalFormula::base_theory` returns `Option<&Theory>`: `None` for a hybrid
  base, whose core theory's stable models are only proposals until its
  constraints accept them. Match `TerminalFormula::base` instead, and check a
  hybrid base's answers with its core's checker before reconstructing them.
- `ExecutionObservation::TerminalDefinitions` gains `base` and `streamed`;
  `GroundingObserver::terminal_definitions` and
  `StageRecorder::mark_terminal_definitions` take the base's kind; exhaustive
  matches on `GroundingMode` gain a variant.
- Lazy (hybrid) formula admission closes the completed support once it has
  emitted the producer core, keeping the canonical base and only the support
  relations the streamed constraints read; discovery and order indexes and every
  other relation are released. A hybrid owner retains less support memory;
  the close is charged as formula work and its transient peak is checked against
  the support-byte ceiling, so a program within the close's envelope of that
  ceiling can now be refused there. Answers are unchanged.
- Answers reconstructed with deferred terminal definitions each get a
  per-answer allowance of work and substitutions — the headroom grounding left
  under the formula ceilings — instead of drawing on one cumulative allowance,
  so a program with many answers is no longer stopped by the grounding work
  ceiling. A refused answer's work or substitution limit is reported as that
  allowance, with the call's own charge as observed. `ReconstructionStatistics`
  adds `admission`, `allowance`, `latest` and `peak` (`ReconstructionCharges`),
  shown in the `--stats` records line, the `--stats` table (allowance and
  largest answer) and the JSON `terminal_execution.reconstruction` object.
- Formula support keeps posting lists only for the columns a join can bind, by
  a syntactic rule that keeps a superset of what probes and totality domains
  read (tighter for rules with an ordinary head and a flat body, every named
  column for other constructs). A predicate no rule reads keeps none, so `SupportIndexEntries`, support bytes
  and support work fall; a program such as eager Mastermind 6×8, whose unread
  `next_guess/6` alone needed 1.57 million entries, no longer stops there.
  Grounding observations report `unindexed_probes`, which is zero whenever
  every probe found its posting.
- Interning a derived atom, and adding a row to a support relation, compares
  it first with the last entry in order, which each ordered index now keeps:
  atoms and rows that arrive in order, as a rule's derived heads usually do,
  are placed without a typed search. Results, positions and orders are
  unchanged; charged work falls for in-order arrivals and rises by one
  comparison for others, so under a fixed work limit a program whose arrivals
  are rarely in order can stop slightly earlier than before.
- Source-admission refusals located in an input file, such as resource limits
  and unsafe variables, name the file, line and column and show the source
  excerpt, as syntax errors do, instead of a byte range.
- `--json` output of runs whose answers each bring their own atom owner, such
  as answers with reconstructed terminal definitions, no longer slows
  quadratically with the number of answers. The document's atom table copies
  each atom it spells once instead of retaining the answer that spelled it, so
  such runs no longer keep every answer alive until the document ends; its
  identity cache keeps every atom owner something else still holds, pruning the
  rest at amortized constant cost per record, so interleaved closure workers
  keep reusing identities across records whether or not answers are kept.
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
- The regions method builds, and charges, the original theory's narrowing
  index when a region walk first needs it rather than when the enumeration is
  constructed: before the walk's first step, on the scalar, batched, producer
  and parallel routes alike. The charge, one work unit per node, comes first,
  so a refused charge builds nothing and stops the run incomplete at the walk's
  start; a charge admitted before a failed build stays in both search work and
  `regions.work`. Runs decided by a positive certificate build no index and
  report lower search and region work. The producer route's frontier byte
  receipts read before the root's first narrowing exclude the original
  theory's knowledge; from that narrowing on they are unchanged. `--stats`
  attributes the index's time to candidate generation on every route, the
  parallel walk included, whose report now adds the coordinator's measurements
  to the workers' sums: candidate generation counts one more call, and
  candidate setup no longer includes the index. Under a fixed work limit,
  construction no longer fails for want of index work, so a run that stopped
  at construction now stops at the walk's start, and a run a positive
  certificate decides can complete where it stopped before. Certificate
  preparation, which now has the index's work still to spend, can select a
  tight or positive plan that was refused before for lack of work, so such a
  run can take a different, equally exact membership route, with different
  certificate and region receipts. Limits are unchanged.

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
