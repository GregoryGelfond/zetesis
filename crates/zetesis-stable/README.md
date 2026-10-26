# zetesis-stable

Region search for the stable models of finite general-formula (Ferraris)
theories. Candidates are regions of the theory's atoms narrowed by its readings,
with no clause form, and the reduct's proper-subset query is walked as a second
region tree. This library uses no external SAT engine or clingo at runtime. It
complements the existing exhaustive formula reference and the normal-rule
least-closure checker. Its former clause search is described under
[History](#history).

```rust
use zetesis_ferraris::{AdmissionLimits, FormulaParts, Node, Theory};
use zetesis_stable::{Cancellation, Limits, StableModels};

// a ∨ ¬a: an optional, supported atom.
let theory = Theory::new(
    1,
    FormulaParts::new(
        vec![Node::atom(0), Node::falsum(), Node::implies(0, 1), Node::or_pair([0, 2])],
        vec![],
    )?,
    vec![3],
    AdmissionLimits::default(),
)?;
let mut models = StableModels::new(&theory, Limits::default(), Cancellation::default())?;
for model in models.by_ref() {
    println!("{:?}", model?.atoms().collect::<Vec<_>>());
}
assert!(models.exhausted());
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Public entry points

`check(&Theory, &Interpretation, Limits, &Cancellation)` returns
`Check::Stable`, `Check::NotModel`, `Check::NonMinimal(witness)` or
`Check::Inconclusive(reason)`; a refutation by the support law under a
complete tight certificate is the enumeration's own and never a check's
verdict. `Check::accepted()` is true only for `Stable`.
This standalone call builds the original theory's index for its own region
query.

`StableModels::new(&Theory, Limits, Cancellation)` returns an iterator of
`Result<Interpretation, Incomplete>` whose candidates are the leaves of the
region tree. Interpretations retain the original theory's
immutable instance identity. The iterator is fused: after exhaustion or one
terminal error it returns `None` forever. `exhausted()` becomes true only
after the proposer proves that no classical candidate satisfying all
successful candidate restrictions remains: the region tree is covered.
Stopping after a requested model count does not
establish exhaustion. `statistics()` includes work from incomplete attempts.

`StableModels::restrict_candidates(&Theory)` appends a candidate-only classical
constraint over the same semantic atom count and index meanings. The regions
proposer narrows the regions still to visit by it and continues, since a
visited region was covered under the original theory and a restriction only
removes candidates. A refused restriction installs nothing. The original theory
returned by `theory()` and every reduct check remain unchanged. After any
successful restriction, exhaustion covers only their intersection.

`StableModels::tighten_candidate_bound(&Theory)` keeps one replaceable bound
separately from these permanent restrictions. The caller must ensure that each
new bound, together with the unchanged original theory, classically implies its
predecessor over the same atom meanings; this implication is **not checked**.
Otherwise an earlier pruning may have removed an original model the new bound
permits. The first bound has no predecessor. Under this contract, conjoining all
old bounds is equivalent to retaining the latest one in the presence of the
original theory and every independent restriction.
Original theory and reduct membership are unchanged. Exhaustion covers the
constrained candidate family, not the unrestricted world view.

`StableModels::bound_generation()` counts the bounds installed so far by
`tighten_candidate_bound`: zero before the first, one more after each successful
installation. A refused or failed tightening, a permanent `restrict_candidates`
addition and every batch operation leave it unchanged; it never decreases. It is
the generation the region proposer assigns to the current bound. A scheduling
observation only: membership, exhaustion and optimality never depend on it.

Region modes replace the bound's immutable index after successful preparation
and charging. Per-region bound knowledge has a checked, non-wrapping generation
and is reset before using a different bound DAG. Permanent restrictions added
later receive their own knowledge without reassigning the bound's knowledge.
Queued regions retain no bound DAG owner; active worker snapshots may keep the
old index until their step finishes. Already active or pending candidates remain
eligible for original membership checking, so an optimizer must score every
returned model against its current incumbent. Failed preparation leaves the
active bound and generation unchanged, while admitted work remains charged.

The cumulative `candidate_restrictions` counter includes permanent additions and
bound updates; it is not a count of live indexes. With `r` permanent restrictions,
region propagation visits those `r` conditions and one current bound, rather than
all historical bounds. Each inactive region retains knowledge for at most one
bound; a worker can temporarily retain its prior generation's immutable index.
Existing infallible knowledge cloning and shared-owner allocation contracts
remain; this replacement operation does not make all search allocation fallible.

## Proposing candidates by regions

No clause form of the theory is built. The candidate space is the coverage tree
of `Search.lean` over the theory's atoms, walked by `zetesis_cpu::regions`: the
root leaves every atom open, each region is narrowed by
`zetesis_ferraris::Narrower::narrow_known_reserved`, from its parent's
knowledge, to the fixed point of the theory's readings, with the theory's
producers for the support cut, and by every candidate-only restriction without
producers, since a restriction supports nothing. A region the readings refute is
dropped with its whole subtree; a region with an open atom is split on the atom
its narrowing prefers, held branch first; a region with every atom decided is a
leaf, and a leaf is a classical model of the theory and the restrictions,
because at a full decision every root is sure or impossible and an impossible
root refutes.

Without active candidate restrictions, complete original producer coverage can
prepare one shared `GeneratorOrder`. It prioritizes atomic choice heads and
ordinary disjunctive heads that have no ordinary singleton producer, then the
earliest ground dependency component and lowest atom identity within that
component. Both signs of body formula operands contribute to dependency order;
this does not shift disjunctive rules or certify minimality. Once no open
generator remains, the same component order selects among all open atoms.
An active bound or latest permanent restriction instead ranks atomic choices
first, then uses its unknown-parent count, with the original theory breaking
ties. If generator
preparation is inapplicable or exceeds its independently applied byte allowance,
open atomic choices rank first and unknown-parent counts determine the remaining
preference. Every route preserves both split children and the original reduct.
The order is shared across scalar, parallel and joined device candidate workers;
its preparation receipt is included in candidate reading work.

The leaf is the proposal, and the reduct decides it: by a complete class
certificate when one applies, else by the proper-subset query, which under
this method is a second region tree. Its root cuts every atom outside the
candidate and leaves the candidate's atoms open; its regions are narrowed
by the knowledge of the frozen reduct, read as the original DAG under the
candidate's truth mask (`FerrarisMask`), with no support cut, since a model
of the reduct need not be supported; a leaf other than the candidate is a
proper-subset model and the validated countermodel, and a covered tree
proves stability (`ReductRegions.stable_iff_no_countermodel`). No reduct
encoding is prepared. No exclusion is needed: each leaf is proposed once,
since only the leaf's emission resolves its region.

The narrowing is driven by a worklist over an index of the theory, built
once, when the walk first needs it (a run a positive certificate decides
builds none), and charged then, one work unit per node: a node or atom that learns something is revisited once, and only its
parents, operands and dependent producers are read, as unit propagation
over watched clauses touches only what moved. What a narrowing knows about
a region travels with the region: a split copies the knowledge into one
child and moves it into the other, so a child's narrowing starts from its
parent's and learns only what the split decided (`FormulaBounds.known_mono`),
and the regions still share nothing. The worklists belong to the walker
instead: each walk, worker and producer reuses one `NarrowingScratch` for its
candidate regions, each reduct workspace one for its proper-subset queries, and
each proposer one for its positive checks. The reduct query carries its knowledge the same way. Node visits and producer
checks acquire search-work permits before execution and each split is charged
as a decision, against the same cumulative `SearchLimits`. The original and
frozen narrowing receipts retain admitted work even on a control or quota
failure; parallel live counters publish that prefix before reporting the stop.
Started reduct queries also remain counted when their first narrowing fails.
`Statistics::regions` reports regions visited, refuted and reached as leaves,
propagations (a node learned and its parents revisited, a chain learning by
one counter or witness step, or an atom's support rechecked), atoms held and cut,
whether the support cut applied, and the reading work, and
`Statistics::reduct.regions` the same for the reduct
queries. Laws:
`FormulaBounds.lean` for the readings, the closure's rules and the leaf
(`decided_leaf_models`), `Search.lean` for the tree. See [the proposer](src/ferraris/regions.rs) and
[its propositions](tests/integration/regions.rs).
An optimizer must separately prove that excluded stable candidates are dominated
by an already verified incumbent, and use a non-strict bound to preserve ties.

`enable_certified_checking(TightPlanLimits)` retains the tight-only library door.
`enable_class_checking(CertificateLimits, CertificateOrder)` tries ranked support
and positive least consequences in the requested order. Each plan checks the
complete original theory; a source class hint can choose order but cannot bypass
applicability. Positive producers allow cycles and monotone conjunction/disjunction
bodies. The primitive evaluates every original constraint on the completed least
interpretation; a failed constraint excludes answer sets, without claiming that
there are no classical models. Choices and unsupported producer bodies decline
this specialization.

A successful positive or stratified plan determines its least interpretation:
the stream proposes that candidate alone, walking no region, and proposes none
when it violates an original constraint. This preserves all answer sets and
avoids an exponential walk through other classical models. Positive checking
independently authenticates the original theory and compares the candidate with
the least consequences. A larger original model is refuted by those least
consequences as a proper-subset reduct model, even when they violate an original
constraint: every constraint satisfied by the candidate has a tautological
reduct. Only the least interpretation satisfying the original theory is emitted.
Complete positive execution therefore needs no reduct query.
The [constrained-positive laws](../../proofs/guide/constrained-positive.md)
separate positive producer closure from arbitrary original constraint filtering;
their premises do not establish the Rust or resource refinement.

The certificate statistics API now names the selected algorithm explicitly.
Consumers of the former tight-only `plan`, `refusal` and
`Incomplete::Certificate` payloads match `CertificatePlanStatistics` and
`CertificateError`; the tight-only enabling method retains its original policy.

`CertifiedStatistics::plan` distinguishes `Tight` and `Positive`. Actual refused
attempts remain visible even when a later plan succeeds. Construction and
candidate-check work are disjoint subsets of cumulative search work; neither
receives a fresh run budget. Optional construction refusal leaves fallback
available. Cancellation, deadline, allocation and cumulative work failures
preserve the original stop and cannot establish exhaustion. Repeated
configuration retains its first attempt, and configuration after candidate
generation is refused.

The positive construction receipt reports named retained and peak capacity,
including failed preparation. `positive_check_peak_bytes` separately reports the
actual retained plan plus local evaluation capacity; it is absent when that route
was not entered. Neither number is RSS. Source/refinement and physical execution
remain separate from the semantic least-model preservation argument. No elapsed
performance improvement follows from these route and full-family controls.

`StableModels::next_batch(BatchLimits, checker)` separates complete original-model
proposals from membership checking. The checker receives the immutable original
theory and an ordered, bounded slice, and returns one `BatchVerdict` per candidate.
`NoProperSubset` is a trusted completion-preserving checker's stability result;
`Residual` invokes exact native reduct search; `Refuted` is a rejection the
checker established without a query, as the complete tight certificate does for
a candidate with an unsupported present atom by the support law. A `NotModel`
result for these independently validated original-model proposals is an
invalid-witness error.
The callback must preserve ordering and semantic meaning; cardinality is checked.

Scalar batch proposals and joined producer slates share one original-validation
`EvaluationWorkspace` per batch owner. Only capacity survives: every candidate's
truth is recomputed against the exact original theory before the external checker
runs. Numeric measures are shared within that evaluation, never across candidates.
`max_pending_bytes` includes this workspace's header and actual retained capacities
alongside pending/result storage. Validation storage was previously temporary and
uncharged. A smaller allowance releases oversized optional scratch; validated
pending candidates remain retryable without reevaluation.

A retained pending batch accounts for its candidates until the whole batch
commits. Checker, shape and pending-capacity failures keep that batch retryable;
retries recheck the supplied limits. Search failures cannot establish
exhaustion. Successful calls return ordered verified
models; a consumer still owns scoring, output and any queued result accounting.
New objective restrictions apply only to future candidate generation and never
modify the original reduct theory or discard pending proposals.

The joint protocol also has a two-phase form, for a caller that checks a slate
while the stream produces the next. `propose_slate` hands out the oldest pending
slate with a `SlateTicket`; `slate` reads its identity and candidates; and
`commit_slate` commits it with one ordered verdict per candidate, through the
closure form's own certification and completion. `propose_lookahead` proposes
the next slate while the pending one is checked. Each slate's producers still
join and settle before that slate is checked, and a slate proposed ahead is
handed out, as `SlateProposal::Ahead`, only after its predecessor commits. At
most two slates are pending; any ticket but the oldest pending slate's is
refused with `BatchError::SlateOrder`. A lookahead is declined for a stream
that expects candidate bounds, for a slate handed out for the independent
check, when one is already proposed ahead, when the pending slate's own
production holds a stop, and when its rows do not fit the pending-byte limit
beside the pending slate's or the allocator refuses them; the proposal after
the commit then reserves as the serial schedule's does. Every failure after
its identity is issued, a production stop among them, is held and reported
after the pending slate commits, so `propose_lookahead` fails only on a fused
stream; its empty proposal establishes exhaustion only after that commit. The
pending receipt counts both slates. `joint_batch_capacity` stays the envelope's
whole count, and is zero unless both slates' rows fit; `lookahead_capacity`
sizes the slate proposed ahead. The closure forms run the
same hand-out, checker and commit, with unchanged receipts.

`batch_statistics()` reports checker calls, committed candidates, propagated
decisions, exact residual completions and pending proposals. It also reports
original-validation work, separately from search/reduct work, and the validation
workspace's current retained bytes, including capacity from a failed attempt.
These counters describe batch calls; the general candidate count can also
include scalar iterator calls if a library caller alternates modes after draining each batch.
Switching to scalar iteration with pending work is refused. The CLI uses one
mode throughout a session. The [batch accounting laws](../../proofs/Zetesis/BatchAccounting.lean)
prove semantic coverage and occurrence conservation under explicit producer and
checker assumptions; they do not prove this implementation refines the ledger.

`CompletionExecutor::new(workers)` builds one reusable Rayon pool when more than
one worker is requested. `next_batch_with_completion(limits, &mut executor,
checker)` uses it for independent exact residual checks. Proposal generation,
the supplied checker, objective feedback and publication remain on the calling
thread. `next_batch` uses a bounded single-worker executor. With native parallel
region search, workers decide their own leaves and the CLI pulls their answers
individually. Otherwise CPU CLI execution uses its scalar cursor; the shared
resource policy keeps residual completion at one worker. Library callers can
set `SolveConfig::completion_workers` above one to select reusable batched
completion. GPU formula batches use this same executor for exact residual checks.

Each worker receives only the immutable original theory, its matching candidate,
and limits. A query leases at most 64 work permits from the enumeration's shared
allowance, then consumes them locally before charged operations. When a grant is
fully consumed, renewal commits it and attempts to acquire its replacement under
one ledger lock. It creates no new permits and preserves the charged operation
sequence.
Settlement records consumed permits and returns unused ones on completion or
failure. Returning unused permits or establishing terminal exhaustion wakes
waiting queries; an outstanding grant still prevents an exhaustion claim.
Decisions retain individual atomic reservations. No worker gets a fresh full-run
quota. A private statically selected quota policy keeps scalar
queries free of shared-pointer checks while reusing the same query operations
for parallel completion. Every job joins before counters are merged and before
any result is committed. Failure retains the entire pending batch, even if
other workers already proved some members stable. For complete matching runs within configured limits, model order and
deterministic work counts match scalar completion. Resource-limited
attempts can differ with thread scheduling; no deterministic failure prefix or
parallel speedup is promised.

`executor.last_statistics()` describes its last entered completion attempt,
including completed-but-uncommitted slots and failures. When timing is enabled,
coordinator elapsed time and summed worker intervals are separate. Parallel
worker intervals are not added to the scalar `SearchPhaseTimings` recorder.
`CompletionExecutor::with_scratch_limit(workers, bytes)` sets a cumulative live
completion-scratch ceiling; `new` and `default` use 256 MiB. Each residual is
decided by the proper-subset query over the enumeration's one original index,
which completion borrows and never builds; each query's retained workspace is
bounded by `Limits::max_reduct_bytes` (64 MiB by default). All-certified
execution enters no query.

Completion admits the ordered result storage before allocating any query
workspace. No query storage is estimated in advance: each worker's workspace is
created and measured before candidate work, and a ceiling the measured
workspaces exceed refuses the batch with `BatchError::Limits` carrying
`Incomplete::CompletionScratch` rather than running fewer workers. The original
proposals remain retryable using an executor with a sufficient allowance.
Certificate-only batches need result storage but no query workspace.
Work/decision or semantic failures still follow the existing terminal-search
contract. `last_statistics` distinguishes requested workers, preflight
concurrency, the requested result storage, the measured peak, and residuals
entered, completed locally or failed. A refusal can therefore report a peak
above the limit with zero entered candidates. Local success is not batch
publication.

Worker scratch is released between batches. The scalar enumerator retains one
reduct workspace until it is dropped. Every query recomputes original truth into
its workspace and reuses only its capacity. The caller owns returned models
after publication.

The accounting uses the ordered result storage and the workers' retained
workspace capacities. Hash-table bucket/control overhead, allocator metadata,
Rayon scheduling storage, thread stacks, immutable original theory and the
separate proposal/candidate cursor are excluded. It does not bound RSS or GPU
memory. `max_pending_bytes` separately bounds pending interpretations and their
original-validation workspace. No benchmark or device speedup is established by
these portable completion tests.

`StableModels::enable_phase_timing()` optionally records coarse host wall intervals
in `statistics().phase_timings`. Under several workers the intervals are
the workers' own narrowing and leaf decisions summed over the workers, which
may exceed the wall time; the coordinator's wait for their models is not a
phase. Enabling is idempotent. Separate measurements cover candidate proposal
by region traversal, including the original index's construction; independent
original-model validation; and the proper-subset queries, with positive closure
and witness validation, in scalar membership and in completion alike. Candidate
restrictions are
excluded for the caller to measure alongside objective feedback. Callback/device
execution is also excluded. The CLI enables this only for `--stats`.

Timing counts are attempted calls, including errors, and may differ from query
or model counts. Zero calls means unentered. Counter/duration overflow marks the
measurement incomplete and retains its earlier prefix without interrupting the
solver. Disabled timing reads no clock and allocates no timing-specific heap
storage. These measurements are independent of deterministic work/decision
ceilings and do not establish semantic completion or refine the Lean model.

## Walking the tree with several workers

`StableModels::with_region_workers(&Theory, workers, Limits, Cancellation)` walks
the region tree with up to that many workers at once, starting them once the
walk has run, in wall time since it began, longer than their start costs;
`with_region_workers_engaging` takes an `Engagement` that selects another delay.
The walk begins at the first pull on the calling thread, whose coordinator steps
the region tree in the scalar walk's depth-first order and returns the models it
verifies; a walk that ends within its delay starts no thread. Once the delay has
passed, the coordinator starts every worker, and its remaining frontier becomes
worker 0's deque. Each worker owns a work-stealing deque of regions with their
knowledge, a lease on the enumeration's shared allowance that serves its
consecutive regions, and a mutable reduct evaluation workspace. Workers share
the authenticated original-theory index, certificate, restrictions, cumulative
limits and live counters. Candidate and reduct knowledge remain separate. A
worker narrows a region, drops it when refuted, splits it and pushes both
children onto its deque, steals from a peer when its own deque is empty, and at
a leaf decides membership as the scalar walk does, by the class certificate
when one applies and else by the proper-subset query, sending a stable model to
the enumeration. The regions partition the candidate space exactly
(`Search.split_partition`, `split_disjoint`), so every stable model arrives
once and none is missed, whatever the interleaving; the models arrive in the
scalar walk's order until the workers start and in the schedule's order after,
which differs between runs and is not a property of the result.
A restriction added while the walk runs narrows the regions not yet
visited; models already on their way are returned as they are, as the
scalar contract already allows. The work, decision and candidate ceilings
are shared: the first worker to exhaust one raises the stop and the others
stop at their next charge or their next region; the models the workers
verified before they stopped are delivered first and the stop after them,
so a leaf admitted under the candidate ceiling is never lost to a worker
the ceiling refused. The region counts and reading work, the candidates, the
countermodel counts and the phase timings are live counters, current while
the walk runs; the coordinator's certificate and reduct receipts are merged
after each of its pulls and the workers' when they have finished, so a
snapshot taken while workers run lacks theirs. The region statistics report
the workers configured, the delay in force, and whether the walk engaged: not
begun, unengaged (on the calling thread) or engaged. One worker is the scalar
regions walk. This particular operation does not use the batched protocol,
since its workers decide their leaves themselves.

Each deque has its own mutex: taking a worker's own region locks only that
deque, a split publication locks it and then, after releasing it and only while
a worker is idle, the idle gate to wake one, and a thief skips a busy peer. A
publication reads the number of idle workers after a sequentially consistent
fence, without the gate, so busy workers never contend for it. An atomic count of
unresolved regions carries termination: a split raises it before publishing
the children, a resolved region lowers it once, and an idle worker stops only
once it has reached zero. An atomic closed flag, set on the first stop or when
the enumeration stops listening, halts the others at their next region. An
idle worker records an observed cancellation or deadline before closing the
walk, so the enumeration cannot mistake the resulting disconnection for
exhaustion.

A worker's depth-first walk keeps at most one older sibling per ancestor and
the two newest children, and it steals only with an empty deque, so each deque
holds at most `atom_count + 1` regions. Deque storage grows with the observed
depth and is released after joining. This does not bound total process memory.

`StableModels::with_region_producers` instead retains its region preparation
and an owned Rayon pool across slates, the candidates the stream checks and
commits together, filling each in one bounded run that reads the enumeration's
one original index. It uses the same readings and disjoint splits, returning
classical candidates for a separate membership executor.
`next_batch_with_completion` joins each slate's producers, independently
validates original satisfaction, then invokes its checker and finishes residuals.
Work allowances remain cumulative across those stages. No answer-set membership
is performed in a producer, so a device checker receives new candidates rather
than CPU-completed answers. Scalar iterator consumption is also supported.

The producers' deques and the regions they hold own disjoint unfinished
portions of the candidate space. Each producer keeps a deque, retained between
slates, and shares the region walker's frontier protocols: it continues depth
first on the newer child of each split, publishes the older child for idle peers
to steal, and steals a peer's oldest region only with an empty deque. Its deque
therefore holds at most one sibling per atom split along its current path, so at
most `atom_count` regions, besides the region it returns at a slate's end. A
producer holds a region only in one of the slate's hands, as many as the slate
admits candidates and never more than there are producers; a split keeps the
hand for the continued child, and a refutation or an emission keeps it only
while the producer finds another region at once. A step reserves an output slot
only when it reaches a classical leaf, so in a slate of at least as many
candidates as producers every producer keeps working until the slate's last
candidate is found. Once every output slot is reserved the output is spoken
for: no producer that has read it so takes a region or begins a step. A producer
that has not yet read it holds one of the slate's other hands, and its step ends
as a step under way does. A leaf that finds the output spoken for returns its
region unresolved for a later slate. A return refused entry capacity records an allocation stop. Each producer holds one work
lease per slate. Every slate's producers join before device execution begins. A
cancellation or deadline observed once the slate is full is left to the next
slate, so no polled stop follows the emission that fills it. A checker failure
keeps admitted candidates retryable; a producer or validation stop retains the
completed admitted prefix and leaves enumeration incomplete. Output order is
schedule-dependent. The batch payload limit excludes frontier knowledge and
worker thread stacks; it is not an RSS bound. This does not introduce a new
candidate search algorithm or alter the original frozen reduct.

Separate deques suit an enumeration, which visits every region whatever the
order. A caller that tightens candidate bounds as it commits answers declares
so with `StableModels::expect_candidate_bounds` before the stream's first
proposal, and the solver does for every optimizing search whose bound generation is enabled and whose
objective plan was prepared; a started stream
refuses the declaration, so one order governs every slate. Its producers then
share one deque: each publishes there and takes the region any producer
published most recently, so a slate fills from the area where the latest splits
are, and older, shallower regions wait for the tighter bounds that refute them
near the root. Kept apart, producers would each descend a large subtree under
the bound in force when they took it, or under none before the first, exploring
far more regions. The shared deque gives up the per-deque bound: a schedule in
which producers keep leaving their paths for a newer region can queue regions
bounded only by the tree, as with the single stack the producers used before
they kept deques. Entry capacity still grows fallibly, and the frontier
receipts record its exact peak. `frontier_order` in the region statistics
reports the stream's order.

Each region's lease settles before its worker waits for work or sends a model,
returning unused permits to the other workers. The shared allowance includes
certificate configuration performed before the workers start. A certificate
check reserves its finite work bound, capped by the per-verification ceiling,
before execution; returned attempts charge actual work and refund unused
permits, including after a typed refusal. A short shared allowance bounds the
check and yields a work stop without publication. Certificate configuration is refused after candidate
generation begins. Dropping the iterator closes the region pool and drops its
model receiver before joining the workers, releasing blocked sends as well as
idle workers. An unwinding checker conservatively consumes its reservation;
the worker closes the pool and reports `Incomplete::WorkerPanicked`, so this
failure cannot establish exhaustive coverage.

## Search and limits

`SearchLimits` bounds charged work and region splits, cumulatively across a run.
Work covers region narrowing and traversal, the original index's construction,
proper-subset queries, candidate restrictions and bounds, and optional
certificate work; every split, in the candidate tree and in every proper-subset
query's tree, is a decision. Cancellation and deadlines are polled at charged
work boundaries.

For a `StableModels` run these counters are cumulative across the candidate walk
and every reduct query. `max_candidates` bounds classical candidates checked;
exhaustion after exactly that many candidates is still established.
`max_verification_work` separately bounds each independent original/reduct
evaluator call; evaluator work is not included in search statistics.
`Limits::max_reduct_bytes` bounds each proper-subset query's retained workspace:
actual vector capacity, including capacity from earlier calls. Lowering it below
retained capacity refuses before new work. `Limits::max_native_narrowing_bytes`
bounds the native-extrema index each narrowing owner retains. Exhausting any
bound produces an explicit incomplete outcome, never a logical rejection,
stability decision or coverage proof.

`Statistics::reduct` retains the actual original node/root work of entered
queries, the frozen-positive attempts and their work, the queries' region
counts, and the maximum single-worker retained capacity observed on return.
Original evaluation and returned-witness verification have independent per-call
verification ceilings; proposal validation is a separate operation. Parallel
aggregation sums actual query work and takes the maximum single-worker capacity,
while completion reports aggregate live storage.

## Validation and current scope

Portable tests compare membership for every tiny interpretation, and complete
unique model sets, against the exhaustive reference, including 256 generated DAG
cases per run, native thresholds and ordinal cuts, and exercise exact work,
decision, candidate and storage ceilings and cancellation. Named cases cover
disjunction, choices, double negation, positive cycles, constraints, unused
atoms and zero atoms. The Rust code is tested; the Lean development states the
coverage tree, the readings and the reduct's region query, and does not prove
the Rust narrowing's refinement.

The region search has no clause learning, learned countermodel generalization,
objective evaluation or parallel search within a single query. It accepts
candidate-only constraints from an external optimizer. The normal-rule bridge in
`zetesis-ferraris` can add justified double-negated supportedness guards;
arbitrary theories do not gain this assumption. Horn minimality specialization
remains separate.

The crate consumes an already finite `Theory`. Source grounding, richer choice
and aggregate translations, tuple-set semantics, output projection and
optimization belong to separate boundaries. Passing this kernel's tests is
not acceptance of a correctness example or full clingo compatibility.

## History

This crate also held a clause search until its removal: a Tseitin encoding of
the theory, iterative chronological DPLL with two watched literals, an exact
projection history of the candidates visited, an optional necessary-support
restriction, and a prepared parametric encoding of the frozen reduct with its
own clause query. A session selected it with `SearchMethod::Clauses` (`--search
clauses`); the region search was the default. Membership was the reduct's under
either method. Records observed before the removal, among them the manual's
archived observations, can name the clause search; the
[changelog](../../CHANGELOG.md) records its removal.
