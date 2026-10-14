# Changelog

Notable changes by release. Versions follow Semantic Versioning.

## Unreleased

### Added

- The default install (`scripts/install.sh`) builds zetesis twice. The
  `zetesis` you run is built without the GPU stack, so CPU runs start without
  loading it, and it hands each request that needs a GPU (a GPU backend, or
  `zetesis devices`) to the GPU-capable build installed beside it in
  `zetesis-libexec`, which takes over the same process. A front that cannot run
  that build fails with the new kind `gpu_executable_unavailable`; JSON
  statistics and the record view now name the executable that did the work
  (`executable`). The installer finishes every build before replacing anything,
  replaces each executable by rename, with the `zetesis` you run last, and
  reports an old GPU build it cannot remove instead of failing, in a default or
  a CPU-only install.
- Backend checks, benchmark campaigns and the selected clingo comparison seal
  the executable their runs name as having done the work when it is not the
  one they invoked, as after a hand-off, and record it beside the invoked one
  (`executed`, `executed_after`); a run naming a third executable, or unable to
  name its own, is refused.
- Prime-sieve and subset-sum examples in the maintained correctness and scaling
  comparisons, with complete-answer checks against clingo.
- `Cancellation::metered` runs a metered computation with a `Meter`, which holds
  the computation's local work limit, and settles its outcome where it returns.
  With `ChargeError`, the `Refusal` classification and `POLL_INTERVAL`, it gives
  metered computations one rule for charging work, observing control and letting
  a requested stop take precedence over resource refusals. A nested metered
  computation charges the enclosing meter; `Meter::remaining` and `Meter::absorb`
  count the reported work of a nested operation that meters itself another way,
  up to the limit when it overran its allowance.
  The error type of each metered operation implements `Refusal`.

### Changed

- Each execution-detail record (metadata, information and execution
  observations, and the `test` workflow's lines) is rendered whole and handed to
  the diagnostics writer in one `write_all` call, where it took one write per
  formatted fragment; on unbuffered standard error that was one system call per
  fragment. A styled source diagnostic takes one call for its heading, one per
  rendered line and one for its closing newline. These units, and a plain
  source diagnostic's final newline, reach the writer's `write_all` rather than
  its `write_fmt`; a plain source diagnostic otherwise writes as before, and the
  statistics report is still one call. The bytes are unchanged, and a writer
  that fails at a byte position fails at the same byte. A writer that fails by
  the content of a call, such as `zetesis_test_support::io::FailAt`, now fails
  at whole units: a whole record, or a whole styled heading, line or closing
  newline. A record or statistics report whose formatting fails while the
  writer did not returns an `Other` error, and writes nothing of the unit,
  where the standard library panicked.
- The formula route holds facts as columnar fact products instead of building a
  rule with an empty body for each fact. Each argument alternative of a fact
  statement is stored as one product, its predicate admitted once, each column
  value admitted once and the statement's evidence stored once; consecutive
  one-row statements of a predicate share a run. Support completion interns the
  rows in its first round where their rules stood, exact activity stages them
  required, and emission asserts each with its own root, producer and origins,
  without a join or any join-path attempt. Answers, theories, atom order and
  output are unchanged. Receipts measure the work performed:
  `expansion_usage()` reports each fact's origins once, where the fact's rule
  charged them a second time for its own vector; grounding work falls by the
  facts' support visits, joins and attempts, and
  `GroundingWork.support_producer_visits` no longer counts facts; support bytes
  fall by the facts' pattern rows and rise by the fact storage. A work, origin or
  support-storage ceiling can therefore be reached later than before. Grounding
  observers see one rule-instantiation phase per fact product or run, at its
  first statement, instead of one per fact; `TerminalFormula::deferred_templates()`
  still counts each deferred fact.
- Formula preparation grows the record of each analyzed statement's original
  statement geometrically, where it reserved exactly the next statement's slots,
  so a program of n statements reallocates it a logarithmic number of times
  instead of n times. Expansion receipts count the slots reserved: their scalar
  bytes can exceed the slots in use by up to eight bytes per statement, so the
  scalar-byte ceiling can be reached that much earlier.
- Comparisons of integer arithmetic are decided with machine integers. The
  compiler compiles each comparison operand of at most sixteen nodes once, with
  the prepared program, into an integer comparison form; on a row whose operand
  values are numbers, the form compares two machine integers, constructing no
  term and reading no projection, and every other row falls back to the
  expression evaluator. Verdicts and answers are unchanged. Preparation's
  expansion receipts rise by the forms' steps (`TermWork`) and their eight bytes
  per step (`ScalarBytes`); every compiled expression is 24 bytes larger, which
  the charges that count the size of compiled literals reflect. Grounding work
  falls for comparisons a form decides, except where a unary projection used to
  answer; a row that falls back is charged the form's constant as well as the
  evaluator's work; support bytes fall where a number would have been newly
  constructed. A work or expansion ceiling can therefore be reached earlier or
  later than before. `GroundingWork` gains `integer_comparisons` and
  `integer_comparison_declines`, the rows decided and declined, which the
  statistics' grounding attribution reports. `ComparisonForms`, from
  `comparison_forms()` on the prepared and admitted formulas, counts the
  comparison literals with a form and without one by reason, and the statistics
  records print it.
- An aggregate assignment now proposes its carrier: the measure of every
  selection that contains its required tuples, those from an element whose
  condition has no atom of either sign, and lies within its possible ones.
  Admission's arithmetic family follows the carrier, in the assignment's own
  rule and in every rule that reads its heads. Programs that were refused only
  for a proposal outside the carrier are now admitted, such as
  `p(M+1) :- M = #min{1; 5}.`, which answers `p(2)`;
  warnings that came only from proposals outside the carrier are gone; and a
  family whose every carrier value divides by zero is now refused, such as
  `q(Y) :- N = #count{1}, Y = 1/(N-1).` The carrier can still hold values no
  answer set realizes: `{p}. q(Y) :- N = #count{1 : p; 1 : not p}, Y = 1/(N-1).`
  substitutes `N = 0` and keeps its warning. A sum is refused exactly when one of
  its carrier values leaves the 32-bit range, whatever the order of its weights.
  Answer sets are unchanged. On the maintained nested
  Mastermind workload, grounding keeps 654 possible atoms instead of 5,162.
- The refusal of a `#min` or `#max` endpoint value names the guard as the manual
  and the code do: "is excluded by the numeric endpoint guard", where it said
  "the zetesis endpoint guard". Its diagnostic code,
  `zetesis::extremum-endpoint`, is unchanged.
- Read the host's memory on macOS with one `sysctlbyname` call for `hw.memsize`
  instead of running `/usr/sbin/sysctl`, so no invocation starts a child
  process. It is the one foreign call in zetesis's own source, named in the
  authored-lints audit.
- The default memory allowance is half of the memory available to the process:
  physical memory, or on Linux the memory limit of its control group when that
  is lower, as inside a container. It no longer rises to two gibibytes on a host
  or container with less than four; two gibibytes remains the default when the
  host does not report its memory. The record view of `--stats` (with `--json`,
  or from a legacy invocation) reports the memory available to the process, and
  `--memory` is listed under `--help-all`, where its default is stated as this
  rule.
- The everyday help offers `--backend cpu` or `gpu`, where `gpu` takes the
  platform's native API; the full help (`--help-all`) lists every value with its
  meaning, `metal` and `vulkan` included. Every spelling is still accepted.
- Reuse aggregate bounds when one native contribution row extends another,
  preserving answer sets while narrowing candidates before reduct checking.
- Share native min/max rows across comparisons and retain deductions across shared
  Boolean conditions, with corresponding Lean laws.
- Reuse numeric row evaluations and checked domain bounds, and stop propagation
  scans and notifications once their result is settled.
- Use divisibility to narrow eligible numeric joins and reject unreachable
  aggregate sums, retaining checked arithmetic and complete answer sets.
- Evaluate eligible acyclic normal programs during automatic CPU preparation;
  repeated sessions reuse the result without constructing a formula theory.
- Stream range facts into admission and reuse borrowed term comparisons in
  grounding filters, reducing temporary storage and repeated lookup.
- Skip the search for native aggregate rows from Boolean conditions that cannot
  reach one, reducing candidate narrowing work without changing its deductions.
- Keep reads of atoms, implications and two-operand connectives inline in formula
  evaluation and narrowing, restoring the speed of complete candidate checks.
- Observe cancellation and deadlines once per 256 charged operations, instead of
  at every node, in formula evaluation, membership certificates, support and
  partition construction and static closure checks. A requested stop still takes
  precedence over every resource refusal, which each operation settles at its
  exit; coarse operations still poll each step.
- A CPU tight certificate check updates the worker's retained evaluation of its
  previous candidate, re-evaluating only the formula nodes whose inputs changed,
  instead of evaluating the whole theory for every candidate; a large change,
  another theory, or a check after one refused or stopped during its evaluation
  still evaluates completely.
  Verdicts and their witnesses equal a complete evaluation's, and a Lean law
  proves the node-table update rule. `TightPlan::check_work_bound` bounds every
  check on either route; a reused `TightWorkspace`'s work and logical bytes now
  depend on the candidate it checked before. A certified run's checking and
  search work therefore depend on the order in which its workers check
  candidates: they differ between worker counts and, with several workers,
  between runs, and under a binding cumulative work ceiling whether such a run
  completes can differ between runs. `TightPlan::prepare_updates` builds
  the reverse index the update needs, only when its work and byte allowances
  cover the build, and the plan's construction work, construction bytes and
  resident bytes include it.
- Split candidate regions on atoms generated by choice rules before atoms they
  derive, in every theory. Programs whose answers come from choices decide them
  first and settle the rest by propagation, which can sharply reduce the regions
  an optimization visits; answers and optima are unchanged.
- `Knowledge::preferred_atom_reserved` takes the `ChoiceHeads` that rank first;
  pass an empty `ChoiceHeads` for the previous ranking. Producer extraction
  reports the heads of every theory's atomic choice roots in
  `Extraction::choice_heads`, and `OriginalSubject::with_choices` gives a
  self-selecting narrowing the same ranking; `OriginalSubject::theory` and
  `OriginalSubject::choices` read the subject's parts.

### Compatibility

- Exhaustive Rust matches on `zetesis_cli::RunError` must handle the new
  `GpuExecutableUnavailable` variant, and those on
  `zetesis_validation::performance::matrix::Decision` its new
  `GpuExecutableUnavailable` disposition.
- `zetesis_validation::curated::Error` and `zetesis_validation::examples::Error`
  refuse a file under the root they read that is not a regular file with the
  new `Irregular` variant, naming its resolved path, where they reported it
  under `Path` as a path not confined to that root. Exhaustive matches must
  handle the variant.
- Removed public items that nothing in the workspace called:
  `GpuOracle::new_metal`, `GpuTightOracle::new_metal`,
  `GpuFormulaOracle::new_metal` and `GpuFormulaOracle::new_metal_with_projection`
  in `zetesis-wgpu` (`new_selected`, or `new_selected_with_projection` for the
  projected constructor, with `GpuSelection { api: GpuApi::Metal }`, does the
  same); `CatalogRead::constructor_of_predicate` in `zetesis-core`;
  and `FormulaParts::extremum_row` and
  `Narrower::extremum_witness_preparation_work` in `zetesis-ferraris`.
- Made crate-private, since only their own crate used them:
  `RunFailure::into_cause` (`zetesis-cli`); `AtomKey::compare_ref`,
  `AtomRef::compare_key_with`, `Model::selection_payload_bytes` and
  `Predicate::shares_name` (`zetesis-core`); `CONSTRAINT_HEAD` and
  `InstanceAtomIter` (`zetesis-cpu`); `OutputLimits::for_resources`
  (`zetesis-engine`); `FormulaView::try_to_parts` (`zetesis-ferraris`);
  `is_record` (`zetesis-maintenance`); `CompletionExecutor::DEFAULT_SCRATCH_BYTES`
  and `ReductPreparationLimits::DEFAULT_BYTES` (`zetesis-sat`);
  `SourceBundle::working_directory`, `Encoded::into_text`,
  `ProgramSite::with_location` and `ProgramSite::with_statement`
  (`zetesis-themelios`); `Contract::with_witnesses`, `Contract::optimal_family`,
  `Capture::stderr_text` and `process::invoke_supervised_with_cancellation`
  (`zetesis-validation`); and `GpuLazyOracle::check_batch_with_source` and
  `GpuTightOracle::from_context_with_support` (`zetesis-wgpu`).

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
