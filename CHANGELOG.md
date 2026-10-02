# Changelog

Notable changes are recorded here. Releases use Semantic Versioning; unreleased
changes are collected below. Each release receives its version and release date.

## Unreleased

### Added

- Singleton comparison heads, such as `X=Y :- p(X), p(Y).`, with nonbinding
  comparison semantics. Conditional and compound comparison heads remain
  unsupported.
- A Sudoku example and scalability workload. The original 94-program corpus
  is unchanged; the scalability suite contains ten workloads, or eleven with
  `--include-einstein`.
- Library operations for reusable aggregate-prefix validation (`FormulaNodes`),
  prepared objective scoring, replaceable candidate bounds, packed interpretation
  views, fallible copies and allocation-free single-equality resolution.

### Changed

- Share certified computed-domain selection between eager formula grounding and
  hybrid constraint checking. Hybrid checkers retain successful preparation
  across cursors; original arithmetic diagnostics and resource checks remain
  authoritative.
- Reuse join plans, relation resolution, arithmetic results and aggregate
  compilation. Owned formula buffers retain completed prefix validation;
  finite-table selection avoids redundant mask initialization.
- Intersect positive-body argument domains within each producer and combine
  different producers by union, preserving conservative unknown domains.
- Use positive-formula certificates for eligible candidate enumeration, compact
  propagation storage and direct packed membership transfer to GPU checks.
  Answer-set acceptance still requires original satisfaction and reduct membership.
- Reuse objective preparation for scoring and replace obsolete optimizer bounds
  in region search. Objective-work limits now include preparation and refused
  prefixes. `--max-objective-bound-work 0` disables pruning while permitting
  prepared scoring; default limits are unchanged.

### Fixed

- Preserve cumulative objective-work receipts before an incumbent exists and
  after an observer failure.
- Enforce shared hybrid work allowances before table operations and retain
  completed work when a limit or cancellation interrupts a check.

### Compatibility

- Exhaustive Rust matches on `zetesis_solve::Interruption` must handle the new
  `PreparedObjective` variant.
- JSON interruption records can report `prepared_objective`. Statistics include
  `objective_work`, which is `null` when no typed semantic outcome survives.
- Library callers replacing candidate bounds must establish that each new bound
  implies its predecessor; the operation does not check this proof obligation.
