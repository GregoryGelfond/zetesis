# Changelog

Notable changes by release. Versions follow Semantic Versioning.

## Unreleased

### Added

- Lean proofs for executable finite reduct checking, normal-program closure
  and packed interpretation construction and word export. These extend the
  mathematical library; they do not yet verify the complete Rust/GPU solver.
- Lean correspondence proofs for checked formula indices, complete binary
  subset counting and normal-reduct acceptance over packed words.
- Lean proofs for packed-word iteration, packed subset generation and streaming
  reduct checking, including correctness of completed decisions under a query limit.
- A composed packed streaming membership proof and finite theory-admission
  checks that establish the evaluator’s structural preconditions.
- Lean refinement of extracted formula evaluation, root checking, atom selection
  and proper-subset search. Completed membership checks agree with the
  answer-set definition under explicit storage and runtime-model assumptions;
  interrupted checks remain distinct. Public allocation and ownership checks,
  grounding, full solver enumeration and GPU execution remain outside this proof.
- Lean refinement of formula admission and theory construction. The generated
  checks preserve the first refusal and establish the evaluator's structural
  preconditions. Constructor proofs use explicit Rust allocation and ownership
  contracts; interpretation construction and complete public membership checking
  remain separate obligations.
- Lean refinement of frozen-reduct construction and its public satisfaction query.
  Completed calls agree with the Ferraris reduct, preserving ownership checks,
  typed stops and separate work budgets under explicit library contracts.

### Changed

- Separate atom selection, subset advancement, reduct queries and proper-subset
  search in the reference checker, preserving their operations and resource checks.
- Separate formula admission's checks from the theory's shared allocation,
  preserving their order, refusals and transfer of the supplied vectors.

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
