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
- Lean refinement of generated formula evaluation, root scanning and private
  queries of a stored reduct under fixed observation tokens, with work bounds
  and typed stops. Completed queries decide reduct satisfaction given a mask
  agreeing with original truth. Constructing that stored reduct, public owner
  checks, allocation, changing runtime observations and subset search remain
  outside the proof.
- Lean proofs connecting atom-selection and packed subset-update primitives to
  the shared mathematical definitions. The containing Rust search loop remains
  unverified; its current translation limit is documented.

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
