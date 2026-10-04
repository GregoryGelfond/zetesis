# Changelog

Notable changes by release. Versions follow Semantic Versioning.

## Unreleased

### Added

- Admission of constructed themelios programs in the strict relational profile,
  with bounded traversal and errors that identify the original statement.
- General formula preparation from a canonical themelios program, sharing the
  source compiler and eager, hybrid and adaptive grounding operations. Errors
  retain the original statement even when no source text exists.
- Bounded export of native terms and signed atoms to themelios symbols, without
  rendering and reparsing.
- A Lean law separating pool-expanded Boolean occurrences from their grounding
  witnesses.
- Lean proofs for executable finite reduct checking, normal-program closure
  and packed interpretation construction and word export. These extend the
  mathematical library; they do not yet verify the complete Rust/GPU solver.
- Lean correspondence proofs for checked formula indices, complete binary
  subset counting and normal-reduct acceptance over packed words.
- Lean proofs for packed-word iteration, packed subset generation and streaming
  reduct checking, including correctness of completed decisions under a query limit.
- A composed packed streaming membership proof and finite theory-admission
  checks that establish the evaluator’s structural preconditions.
- Lean refinement of the generated public scalar reference checker. Completed
  verdicts agree with the answer-set definition and retain their actual false-root
  or proper-subset evidence under explicit input and library contracts. Runtime
  proofs cover returning control reads and reservations, trace typed refusals to
  their reached cause, and preserve the candidate through the owned API. The scope
  is one finite ground theory; grounding, solver enumeration, optimized routes
  and GPU execution remain separate.
- Lean correspondence from successful theory and interpretation construction to
  the candidate-bound check and accepted-result conversion. The proof derives
  input invariants and preserves the exact checked candidate under explicit
  library contracts.
- Lean refinement of formula admission and theory construction. The generated
  checks preserve the first refusal and establish the evaluator's structural
  preconditions. Constructor proofs use explicit Rust allocation and ownership
  contracts without assuming successful allocation.
- Lean refinement of frozen-reduct construction and its public satisfaction query.
  Completed calls agree with the Ferraris reduct, preserving ownership checks,
  typed stops and separate work budgets under explicit library contracts.
- Lean refinement of finite interpretation construction: actual insertion keeps
  exact bounded-prefix writes and first-invalid refusal; completed construction
  retains its theory and derives exact packed contents and zero padding under an
  explicit reservation contract. Owned vectors supply the finite-input contract;
  unrestricted iterator termination is not claimed.

### Changed

- Boolean choice elements now count separately after pool expansion, matching
  clingo. `{ #true : p(1;1) } = 2. p(1).` therefore has answer set `{p(1)}`.
  Multiple grounding witnesses of one expanded element still share its key.
- Updated the reviewed themelios dependency and removed the separate
  source-location registry used for Boolean choice identity.
- Separate atom selection, subset advancement, reduct queries and proper-subset
  search in the reference checker, preserving their operations and resource checks.
- Separate formula admission's checks from the theory's shared allocation,
  preserving their order, refusals and transfer of the supplied vectors.
- Separate checked atom insertion from interpretation allocation, preserving
  the public API, input order and first invalid-atom refusal.

### Compatibility

- Formula owners' `source()` now returns `Option<&Source>`. Canonical-program
  input retains `original_program()` without source bytes.
- Formula origins and typed failures separate `ProgramSite` identity from
  optional source coordinates. Callers reading origins or matching failure
  fields must handle sites; warning and count-plan location accessors, and
  warning diagnostics, can be absent for constructed input.

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
