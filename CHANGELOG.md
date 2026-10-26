# Changelog

Notable changes by release. Versions follow Semantic Versioning.

## Unreleased

### Added

- Hybrid statistics report how lazy grounding divided the eligible constraints:
  the streamed templates and instances and those grounded in the core, as
  `HybridExecutionStatistics::streamed` (a `StreamedConstraints`, which adds
  `retained_templates` and `retained_instances`), in the `streamed` object of
  the JSON statistics' `hybrid_execution`, and as four rows of the human
  statistics. `HybridFormula` and `StreamedCore` add `retained_templates` and
  `retained_instances`. The recorded execution reads "no streamed constraints"
  when nothing was streamed.
- An ordering between tuples that still contain variables, such as
  `(M,L) < (K,J)`, is admitted wherever a comparison can stand, including
  aggregate element conditions and guards. It compares complete values in ASP
  term order, arity first and then components, and binds nothing. Such orderings
  were refused as an unsupported term; equality and disequality keep binding
  their components.
- GPU formula and tight sessions that enumerate answers can check a slate while
  the stream produces the next one, through a two-phase slate protocol:
  `StableModels::propose_slate`, `propose_lookahead`, `slate` and `commit_slate`,
  with `SlateTicket`, `SlateProposal`, `Lookahead`, `LookaheadDeclined` and
  `BatchError::SlateOrder` in zetesis-stable, and `lookahead_capacity` to size the
  slate proposed ahead. zetesis-ferraris adds `SlateId` and `SlateRef`, the
  identity that binds a slate's results to it. The general and tight GPU oracles
  add `prepare`, returning a `PreparedFormulaSubmission` or
  `PreparedTightSubmission` that holds its oracle until checked, with a
  synchronous `check` and a `check_overlapping` that runs a closure while the
  submission is in flight, both returning a `CheckedSlate`. An overlapping
  check's `CheckedSlate::finish_wait` reports the finish's wait after the
  closure, up to the submission's observed completion.
- `FormulaExecutionStatistics` reports `gpu_lookahead_slates`, the last
  `gpu_lookahead` decision (`GpuLookahead`, `GpuLookaheadDeclined`),
  `gpu_lookahead_device_ns`, `gpu_lookahead_finish_wait_ns`,
  `gpu_lookahead_unmeasured` and `first_answer_ns`, in the JSON and the human
  statistics. The device compute and the finish waits are paired per overlapped
  submission, so their difference is the device time hidden behind production
  over the submissions measured.
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
- A maintained wide Mastermind workload, outside the regular benchmark corpus,
  checks complete enumeration against clingo, with its original sources kept
  byte for byte and a recorded 45,832-answer contract.
- Native sessions decide questions about a program's world view, its set of
  answer sets, by folding answer sets as they arrive instead of collecting the
  family: `SessionBuilder::known` and `possible`, whether a literal holds in
  every or in some answer set; `cautious` and `brave`, the atoms that do; and
  `answer`, a ground literal query's three-valued `QueryAnswer`, the agent's
  reading of complete families. Each returns a `WorldViewDecision` whose
  `WorldViewVerdict` is `Decided` once the fold reaches its absorbing value,
  stopping the session with at most two witness answer sets; `Complete` after
  the unrestricted family is exhausted; or `Inconsistent` when it has no answer
  set. The request is `WorldView::collect`'s with any requested model count
  cleared, so a projection, an objective's selection or `config.models` never
  narrows the folded family, and a session stopped before either is a
  `WorldViewDecisionFailure`, never a value. Brave consequences are decided only
  by exhaustion.
- `zetesis_core::Atom::contrary` gives an atom's strong contrary: the same
  tuple under the opposite classical sign.
- Learned rows generalize the founded collector's ledger. A row pairs signed
  atom conditions with signed outputs, each an atom required present or absent,
  under a consequence class; the Lean module `LearnedRegions` states what such a
  row may decide about a region. zetesis-ferraris adds `LearningOwner`, the
  explicit owner of a class's rows, `LearningLimits` with
  `LearningLimits::for_allowance`, `LearningStore`, `LearningAttempt`,
  `OriginalSubject::carrier`, the one subject of a walk that matches and
  captures rows, and `NarrowingScratch::{seed_learning, deliver_learning,
  learned_rows, learned_evictions}`. A full tier evicts the row with the fewest
  decayed hits; founded rows keep a reserved floor. Above 64 rows matching
  reads an occurrence index over condition and output atoms.
- A solve's candidate walks share learned rows through one store:
  `StableModels::set_learned_store` with `LearnedStore`, the type it takes, in
  zetesis-stable. Its public surface is creating an empty store; no public
  operation adds, seeds, edits or reads a row. Walks seed their tiers from the
  store and deliver to it at their boundaries only: their end, a parallel
  walk's handover to its workers, every join of the GPU route's producers, and
  every `publish_regions` regions visited. `SolveConfig::share_learned_rows`, on
  by default, turns sharing off for measurement; it has no command-line flag.
  Answer sets are unchanged; work counts depend on the schedule under sharing.
- Three 2QBF workloads put a hard unsatisfiable core on the universal side of
  the saturation encoding, so that one candidate's proper-subset query is the
  solve: a parity contradiction, Tseitin's formula on committed 3-regular
  graphs, and the pigeonhole formula. Each records its complete default
  contract, and the oracle gate checks their families against clingo.
- `QueryDivision` in zetesis-stable selects when a parallel region walk divides
  a candidate's proper-subset query among its workers: `WhenIdle`, the default,
  `Immediate`, for tests and measurement, or `Never`.
  `StableModels::set_query_division` sets it before the walk begins, and
  zetesis-solve re-exports it and carries it in `SolveConfig::query_division`;
  the command has no flag for it.
- `RegionWorkerStatistics::query_division` (`QueryDivisionStatistics`) reports
  the division a walk ran with, the queries it published, the published regions
  its workers served and stole, the workers that served any and the most one
  served, in the human statistics' `query division` line and the statistics
  JSON's `search.candidate_regions.workers.query_division` object.
- zetesis-cpu's `Traversal::pending` and `Traversal::surrender` report and hand
  over a traversal's regions still to visit, the oldest first.

### Changed

- `--grounder lazy` and the hybrid formula schedule stream an integrity constraint
  only when its instances do not fit in the formula. Once the producer core and its
  completion are built, each eligible template is grounded into the core, in
  program order, while an upper bound on its instances fits the share
  `FormulaLimits::max_retained_constraint_nodes` grants and the theory ceilings.
  The ordinary resource policy grants the theory's node share, a quarter of the
  memory allowance, so lazy grounding streams only what eager grounding could not
  hold; a smaller `--memory` allowance streams more. Answer sets are unchanged.
  Streaming joined every streamed rule again in every search region, so programs
  whose constraints fit no longer pay that cost. `FormulaLimits::default()`
  grants no share, so a library caller of
  the explicit schedule with default limits streams every eligible constraint as
  before.
- The `--grounder` help no longer reads as though `auto` streams constraints: it
  says that `auto` never does, and that `lazy` streams only what does not fit.
- GPU formula and tight sessions that enumerate answers produce the next slate
  of candidates while the device checks the current slate, with one device
  submission in flight. Answer families, membership checks, failure and
  exhaustion semantics and the release of answers in proposal order are
  unchanged. Two cases can leave the slate under check undelivered, and claim
  nothing for it: a stop that arrives while the next slate is produced, and, in
  a GPU formula session, a work or decision limit that the next slate's
  production reaches or brings within reach of the current slate's completion.
  Optimizing searches still check one slate at a time. The physical
  qualification has 75 exact tests per backend.
- Support completion keeps its integer key certificates and source-column hulls
  from one round to the next and extends them with each round's new rows instead
  of rebuilding them over every snapshot. An ordering query whose key range would
  not halve its source interval reads the interval's own keys. Recursion through
  arithmetic grounds in time linear in the rounds where it was quadratic: about
  9x fewer instructions for a 2,048-step chain under a range comparison and 13x
  for a counter whose rule assigns from the chain. Possible atoms, the emitted
  formula and answer sets are unchanged. Grounding statistics add
  `numeric_index_extensions`. The kept bytes count against `max_support_bytes`
  until completion.
- Numeric grounding joins combine certified divisibility and range filters before
  producing tuples; equal-size sources prefer the bindings needed by those filters.
- Repeated support checks consume initial support seeds directly after resetting
  their counters, avoiding a redundant queue round trip.
- Repeated support checks skip dependency paths already known false and reuse a
  compact list of affected heads. Packed explanations omit outputs already
  excluded from the candidate region.
- Possible-sum checks detect an allowed value while updating the packed sum set,
  avoiding a separate scan.
- Region narrowing keeps its admitted and unspent reads as one count, so
  charging a read within them is a single decrement; the work ceiling is met, or
  the quota consulted, only when they run out. Receipts, quota reservations,
  refunds and refusals are unchanged.
- Split selection walks the region's open atoms a mask word at a time, through
  `Region::mask_words` and `Region::open_word` in zetesis-cpu, instead of
  chained atom iterators. The atoms ranked, their order, the chosen atom and
  the charged reads are unchanged.
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
  programs, and every batch the route cannot run, keep evaluating every
  formula, on a kernel compiled without the route, which binds no retained
  state. Each slot's retained state counts toward the batch's memory
  allowance, so adaptive slates can settle at a smaller device prefix, never
  below the batch size. Answer sets are unchanged.
- In the GPU library, tight checks report the work their route performed, and
  `TightGpuLimits` gains `min_update_candidates`. `TightGpuBatchStats` reports
  routes and the update route's availability, and
  `GpuTightOracle::UPDATE_ENGAGEMENT_STRIDES` names the strides per dependency
  level from which the route engages. The tight kernel requires eight storage
  buffers per shader stage and 48 bytes of workgroup storage.
- GPU tight checking guards its update route while it runs: after a batch
  whose slowest update costs more than evaluating every formula, both measured
  by the busiest invocation's operations under one cost table, the route waits
  for 1, 2, 4 and up to 32 batches, which run the kernel without the update
  route, before it is tried again. An update's cost includes rebuilding its
  slot's support summaries after a change too large to update. Answer sets are
  unchanged. In the GPU library, `TightGpuUpdateUnavailable::GuardDeclined`
  reports a declined batch, `TightGpuCheck::critical_path` an update's path,
  and `TightGpuBatchStats` gains `update_critical_path`, the batch's slowest,
  and `dense_critical_path`. The solver's `TightUpdateAvailability` gains
  `GuardDeclined` (`guard_declined`) and `TightUpdateStatistics` the count
  `guard_declined_batches`, which the statistics JSON and both views report.
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
- Lazy grounding moves a pairwise constraint into the retained core when every
  component of its pairs is complete, two-atom components included: each
  two-atom component becomes its pair constraint, once, and each larger
  component still becomes one at-most-one condition. A single two-atom
  component, such as a corner diagonal in n-queens, previously kept the whole
  constraint streamed; a constraint with an incomplete component still streams.
  Answer sets are unchanged.
- JSON answer output recognizes the atoms that reconstructed answers inherit from
  their shared base by identity instead of by structure, from the third answer
  on: the first is written whole, and the second records the identities.
  Documents are unchanged.
- GPU tight support checks may account a sixteenth of the memory allowance per
  batch, rather than the thirty-second other device batches use, so that their
  update route engages under smaller allowances. On a theory of about 790,000
  nodes and 262,000 atoms, the default allowance engages it on hosts with about
  9.3 GiB of memory, rather than about 18.7 GiB. Requested statistics report the
  tight batch bytes beside the batch bytes. Answer sets are unchanged.
- An optimizing search for one answer, the command line's default, bounds the
  candidates strictly: each better answer admits only candidates cheaper than
  itself, where it admitted those as cheap, so the search no longer walks the
  ties of incumbents it has beaten. Its exhaustion proves the optimum as before.
  On integrated Family Crossing, with one thread, it walks 115,389 candidate
  regions instead of 661,303, and returns the same answer. A request for more
  answers, `--all` included, and `--count` keep the inclusive bound, which
  still meets every optimal tie in one pass.
- Such a one-answer search counts only the optimal answers it met, not every
  tie of the optimum. `--stats` prints `full ties=unavailable` for it, its JSON
  `outcome.optimization.tied_models` is `null`, and the library reports
  `Optimization::ties_pruned`, under which
  `SemanticOutcome::complete_answer_count` gives no total. Requests for more
  answers count every tie, as before.
- The library schedule `OptimumSearch::StrictWithTiePass` bounds every request
  strictly and, when more than one answer is requested, enumerates the proved
  optimum's ties in a second pass over the same theory, bounded to cost at most
  the optimum. It returns the same optimal answers, each once, and is kept for
  measurement, as `OptimumSearch::Inclusive` keeps the earlier schedule: for all
  eight optimal answers of Family Crossing it walks 2,946,650 candidate regions,
  2,831,261 of them in the second pass, where the inclusive schedule walks
  661,303. A stop in the second pass keeps the optimum the first proved
  (`SemanticOutcome::optimum_proved`) and reports the family incomplete.
  `SolveConfig::optimum_search` selects the schedule; no command-line option
  does.
- With several CPU region workers, a candidate's proper-subset query that runs,
  from its first narrowing, for the cost of engaging one worker while another
  waits idle publishes its pending regions for any worker to serve, and the
  candidate is decided by the regions' servers: refuted by a validated
  countermodel, or proved stable once every region is resolved without one. The
  calling thread publishes its query only once its engagement deadline has
  passed, and then starts the workers. Answer sets are unchanged; a stop or a
  countermodel whose check stopped still ends the enumeration incomplete. A
  refuted query's regions, divided, may be narrowed beyond those the undivided
  query visits, so the reduct statistics' region counts of such queries depend
  on the schedule.

### Fixed

- A default install no longer replaces the GPU-capable `zetesis` in Cargo's
  build output with the CPU front, so running that copy serves GPU requests. A
  front copied away from its install now explains that it is not an installed
  copy and points to the installed one, instead of advising a reinstall that
  cannot repair it.
- With phase timing enabled, batched completion's region queries are timed in
  the reduct phase, as scalar membership's are: in
  `SearchPhaseTimings::reduct` and the workers' `worker_reduct`, and so in the
  exact reduct membership phase of `--stats`. They were missing there.

### Compatibility

- `ExecutionObservation::HybridGrounding` carries one `streamed:
  StreamedConstraints` in place of `streamed_templates` and `streamed_instances`.
  `StreamedConstraints` adds `retained_templates` and `retained_instances`,
  `HybridExecutionStatistics` adds `streamed`, and `FormulaLimits` adds
  `max_retained_constraint_nodes`; a struct literal naming every field must add
  them, and one completed from a default needs no change.
- Fields added to exhaustive public structs: `zetesis_stable::Limits::learning`
  and `SolveConfig::share_learned_rows`. A struct literal naming every field must
  add them; one completed from a default needs no change.
- `Incomplete` adds `LateLearnedStore` and `LearnedStoreAlreadySet`, refusing a
  learned store for a started stream and a replacement. It is exhaustive, so a
  match without a wildcard must add them. The JSON output's interruption `code`
  names them `late_learned_store` and `learned_store_already_set`.
- In zetesis-ferraris, only a subject built by `OriginalSubject::carrier`
  matches and captures learned rows. An original subject built by
  `OriginalSubject::new` no longer does; its narrowing is otherwise unchanged.
- The clause search is removed, with no alias and no deprecation period: the
  region search, already the default, is the only formula search. Membership,
  answer sets and the region search's work are unchanged. The command line
  refuses `--search`, and `zetesis-bench run` refuses its `--search`.
  zetesis-sat removes `SearchMethod`, `StableModels::with_method`, `check_with`,
  the clause kernel (`Cnf`, `Clause`, `Clauses`, `Literal`, `Assignment`,
  `AdmissionError`, `AdmissionLimits`, `Resource`, `Solve`, `solve`,
  `solve_with_statistics`), the prepared reduct (`PreparedReduct`,
  `ReductPreparationAttempt`, `ReductPreparationLimits`,
  `ReductPreparationStatistics`, `ReductQueryStatistics`, `ReductWorkspace`),
  the projection history (`ProjectionLimits`, `ProjectionResource`,
  `ProjectionStatistics`), `SupportStatistics`, `SupportStatus` and
  `CompletionScratch`; the fields `Limits::{admission, reduct_admission,
  projections}`, `Statistics::{candidate_queries, projections, support}`,
  `ReductStatistics::{preparation, parameter_work}`,
  `SearchStatistics::{propagations, conflicts}`,
  `CertifiedStatistics::{restriction_refusal, restriction_work,
  restriction_clauses}` and `SearchPhaseTimings::reduct_preparation`; and the
  variants `Incomplete::{Admission, ProjectionLimit, RegionFilterUnsupported}`
  and `CertificateError::Restriction`. `Incomplete::ReductStorage` now reads
  "reduct query storage".
- zetesis-solve removes `SearchMethod`, `SolveConfig::search`,
  `SolveConfig::max_projection_nodes`, `SolvePhase::ReductPreparation` and the
  `search` field of `ExecutionObservation::{CpuFormula, DeviceFormula,
  DeviceTight}`. `max_projection_entries` and `max_projection_bytes` remain, for
  an explicit `#project` session's history. zetesis-validation removes
  `selected::SearchMethod`, `NativeExecution::search` and the performance
  request's `search`, and still reads archived reports and statistics that
  record the clause search; zetesis-bench removes `Search`.
- `--stats` no longer prints a search method, the clause search's classical
  query count, its projection history and necessary-support lines, or the class
  attempts' restriction fields. The statistics JSON's `search` object drops
  `candidate_queries`, `projection_history` and `necessary_support`, and the
  interruption codes `admission`, `projection_entries`, `projection_nodes`,
  `projection_bytes` and `region_filter_unsupported` no longer occur. The phase
  timing section, in text and in JSON, drops `reduct_preparation`.
- The closure route is removed, with no alias and no deprecation period: every
  program is prepared as formulas and solved by region search, whose answer
  sets equal the closure route's on normal programs
  (`NormalFerraris.answer_set_iff`). The command line refuses `--oracle closure`
  and `--source-batching`, and `--oracle auto` prepares formulas directly.
  - zetesis-solve removes the closure session and executions
    (`ClosureExecutionStatistics`, `ClosureJoinStatistics`, `ClosureRoute`, and
    the lazy and shared executions), `PreparedInput`'s relational and ground
    constructors, `Oracle::Closure`, `SourceBatching`, the closure-only
    `SolveConfig` fields, `SolveError::{Batch, ClosureReservation,
    UnsupportedSourceBatching}` and `ExecutionObservation::{CpuClosure,
    DeviceClosure}`.
  - zetesis-cpu removes the closure oracles, candidates, batches and lazy
    rounds, keeping cancellation, regions and tables.
  - zetesis-wgpu removes the static and lazy GPU oracles and their shaders.
    `GpuInfo::supports_static_oracle` is now `supports_formula_oracle`, and it
    screens adapters by the formula kernel's limits.
  - The phase timing section is schema 7, without the closure membership phase.
    zetesis-validation reads schema 7 and every earlier schema.
  - Answers arrive in the region walk's order, held branch first.
  - A facts-only program is evaluated completely at admission, so `--models 1`
    reports exhaustion.
  - `--backend gpu` with a zero device batch allowance reports an incomplete run
    instead of a GPU error.
  - An automatic run's parse error has the JSON error kind `formula_admission`
    instead of `expansion`.
- The JSON document is schema 3. Its model records are unchanged from schema
  2; its statistics drop the keys of the removed routes: the clause search's
  `search.candidate_queries`, `search.projection_history` and
  `search.necessary_support`, and the closure route's `closure_execution`,
  `lazy_execution`, `shared_execution`, `candidate_restrictions`,
  `query_execution` and `expansion`. zetesis-validation reads schemas 1 to 3,
  so documents of earlier executables can still be compared.
- zetesis-sat is renamed zetesis-stable, and its library `zetesis_sat` is now
  `zetesis_stable`, with no alias and no deprecation period: depend on
  `zetesis-stable` and import from `zetesis_stable`. With the clause search
  removed, the crate holds region search for the stable models of finite
  general-formula theories, which its new name states. Its API is otherwise
  unchanged by the rename.

- `BatchError` gains `SlateOrder`. `BatchError` is not non-exhaustive, so an
  exhaustive match by a library caller must add the variant. The new public
  enums are non-exhaustive.
- An overlapping check holds the context's lease while the production it runs
  is under way, so another operation on a shared context is refused with
  `Busy` for that span. The synchronous device forms and their `CheckedBatch`
  receipts are unchanged.
- `FormulaBatchStats`, `TightGpuBatchStats` and `FormulaExecutionStatistics`
  report the workgroup size each device batch dispatched. These statistics
  structs are now non-exhaustive: read their fields by name, and build a
  `FormulaExecutionStatistics` from `Default` before assigning its fields.
- GPU formula and tight graphs whose largest address lies within 256 of
  `u32::MAX` are refused with a capacity error, where 0.6.0 refused only those
  within 64, keeping room for the largest workgroup stride. Device buffer limits
  keep such graphs out of reach on the adapters measured.
- `zetesis_stable::Incomplete` gains `LateCandidateBounds`, so an exhaustive match
  on it needs an arm for the new variant.
- `zetesis_core::catalog::AtomIdentityMap` changes from owner-scoped to
  row-scoped identity: it keys a row of a closed catalog by the writer that was
  closed, so a value recorded for that atom through one descendant, or through
  the writer's own snapshot, is found through every other. A caller's value must
  therefore depend on the atom alone, not on the owner that presented it. Rows a
  descendant appends keep their own owner's identity, as before.
- `SolveConfig` has a new public field, `max_tight_batch_bytes`, which bounds
  the accounted bytes of each GPU tight support batch. A struct literal that
  names every field must add it; one completed from `SolveConfig::default()` or
  `SolveConfig::for_allowance` needs no change.
- `SolveConfig::max_batch_bytes` no longer bounds GPU tight support checks;
  `max_tight_batch_bytes` does. Code that sets only `max_batch_bytes` still
  compiles, but its tight checks keep the ceiling the configuration was built
  with: a sixteenth of the allowance from `Resources`, and 128 MiB under
  `SolveConfig::DEFAULT`. A caller who bounds device memory through these fields
  sets both.
- `StableModels::tighten_candidate_bound` is removed, with no alias and no
  deprecation period. `StableModels::improve_candidate_bound` replaces it: it
  takes the identity of the objective plan that compiled the bound and the
  incumbent score it was compiled at, beside the restriction. The first bound of
  a stream is accepted as given; a later one from another plan is refused with
  `Incomplete::CandidateBoundPlan`, and one at a score no better than the active
  bound's with `Incomplete::CandidateBoundNotImproving`, and the active bound
  stays. An exhaustive match on `zetesis_stable::Incomplete` needs arms for both
  variants; the JSON interruption codes name them `candidate_bound_plan` and
  `candidate_bound_not_improving`. zetesis-objective adds `PlanIdentity` and
  `Score::from_costs`, `ObjectivePlan::identity` returns a plan's identity, and
  zetesis-stable now depends on zetesis-objective.
- zetesis-themelios adds `ObjectivePlan::strict_bound`. zetesis-stable adds
  `StableModels::follow`, which reports an earlier stream's receipts before a
  stream's own, and `Limits::remaining_after`, which leaves a later pass of one
  run what the earlier passes did not spend.
- `SolveConfig` has a new public field, `optimum_search`, of the new type
  `OptimumSearch`, which zetesis-cli also exports, and `Optimization` a new
  public field, `ties_pruned`. A struct literal that names every field must add
  them; a `SolveConfig` completed from `SolveConfig::default()` or
  `SolveConfig::for_allowance` needs no change.
- `ExecutionObservation::ObjectiveBound` has a new field, `strict`, and
  `ExecutionObservation` a new variant, `ObjectiveTiePass`. A pattern that names
  every field of `ObjectiveBound` must add `strict`, and an exhaustive match on
  `ExecutionObservation` needs an arm for the variant.
- The JSON `outcome.optimization.tied_models` is `null` when a strict bound
  pruned the optimum's ties, where it was always a number.
- `SolveConfig` has a new public field, `query_division`. A struct literal that
  names every field must add it; one completed from `SolveConfig::default()` or
  `SolveConfig::for_allowance` needs no change.
- The caller-supplied candidate-region filter is removed from zetesis-stable,
  with no alias and no deprecation period: `RegionFilter`, `RegionFilterWorker`,
  `RegionFeasibility`, `RegionPass`, `RegionConsequence` and
  `StableModels::set_region_filter`. A filter could refute regions or decide
  atoms under an obligation nothing checked. The hybrid route's check of an
  admitted program's streamed integrity constraints, the only filter in the
  tree, is now zetesis-stable's own: `StreamedConstraints::new` takes the
  program's `StreamedCore` and a `ConstraintAllowance`, and
  `StableModels::set_streamed_constraints` installs it.
  `Statistics::region_filter` and its `RegionFilterStatistics` remain, as do
  `Incomplete::RegionFilter`, `InvalidRegionConsequence`, `LateRegionFilter`
  and `RegionFilterAlreadySet` and their JSON interruption codes, which now
  report this check and its installer. zetesis-stable now depends on
  zetesis-themelios, whose streamed core holds the checker; every crate that
  used zetesis-stable already depended on it.

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
  [parallel walks](crates/zetesis-stable/README.md#walking-the-tree-with-several-workers).
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
  `FactLimitExceeded`.
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
