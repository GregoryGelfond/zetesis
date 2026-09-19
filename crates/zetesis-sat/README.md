# zetesis-sat

Stable-model enumeration for finite Ferraris theories, with a native bounded
Boolean search for the reduct's proper-subset query. Candidates are proposed
either by regions of the theory's atoms narrowed by its readings, with no
clause form, or by the Boolean search over a clause form. This library uses
no external SAT engine or clingo at runtime. It complements the existing
exhaustive formula reference and the normal-rule least-closure checker.

```rust
use zetesis_ferraris::{AdmissionLimits, Node, Theory};
use zetesis_sat::{Control, Limits, StableModels};

// a ∨ ¬a: an optional, supported atom.
let theory = Theory::new(
    1,
    vec![Node::Atom(0), Node::False, Node::Implies(0, 1), Node::Or(0, 2)],
    vec![3],
    AdmissionLimits::default(),
)?;
let mut models = StableModels::new(&theory, Limits::default(), Control::default())?;
for model in models.by_ref() {
    println!("{:?}", model?.atoms().collect::<Vec<_>>());
}
assert!(models.exhausted());
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Public entry points

`Literal::new(variable, positive)` and
`Cnf::new(variable_count, clauses, AdmissionLimits)` admit classical CNF.
`solve(&cnf, SearchLimits, &Control)` returns
`Solve::Sat(Assignment)`, `Solve::Unsat`, or
`Solve::Inconclusive(Incomplete)`. `Assignment::value(index)` returns `None`
outside its declared universe. `solve_with_statistics` additionally returns
work, decision, propagation and conflict counts.

Admitted clauses share one packed literal arena. `Cnf::clauses()` returns an
ordered exact-size iterator of borrowed `Clause` values; `Cnf::clause(index)`
provides checked random access. A clause exposes `len`, `is_empty`, `get` and
literal iteration. These views preserve empty clauses and canonical literal
order without allocating. Public input `Literal` values retain their full
index range until admission; packing introduces no additional admitted bound.

`check(&Theory, &Interpretation, Limits, &Control)` returns
`Check::Stable`, `Check::NotModel`, `Check::NonMinimal(witness)`,
`Check::Unsupported { atom }` under a complete tight certificate, or
`Check::Inconclusive(reason)`. `Check::accepted()` is true only for `Stable`.
This standalone call explicitly constructs a fresh candidate-simplified reduct,
retaining a differential control for the persistent path.

`PreparedReduct::prepare(&Theory, ReductPreparationLimits, &Control)` returns
`ReductPreparationAttempt { result, statistics }`. The result contains the
immutable proper-subset query or its original typed refusal; the receipt retains
actual work and capacity on either outcome. Clones share the completed owner.
This replaces the provisional `new`/combined failure-receipt API: inspect
`attempt.result` for the unchanged typed cause and `attempt.statistics` for its
actual prefix. Failed preparation does not allocate an error wrapper.
`prepared.check(&candidate, &mut ReductWorkspace, Limits, &Control)`
reuses worker allocation capacity and returns the verdict with actual statistics.
Original truth is authenticated by `EvaluationWorkspace`; callers cannot supply
arbitrary truth bits. The prepared owner checks exact theory identity. Each
concurrent query needs its own workspace. Preparation reports separate work,
submitted dimensions, retained capacity and observed construction peak, including
failure prefixes. The fixed CNF can be larger or search more slowly than a fresh
candidate-simplified encoding; reuse does not promise a speedup.


`StableModels::with_method(&Theory, SearchMethod, Limits, Control)`
returns an iterator of `Result<Interpretation, Incomplete>` whose candidates
come from the chosen proposer; `StableModels::new` is the clauses proposer,
which builds the candidate CNF. Interpretations retain the original theory's
immutable instance identity. The iterator is fused: after exhaustion or one
terminal error it returns `None` forever. `exhausted()` becomes true only
after the proposer proves that no classical candidate satisfying all
successful candidate restrictions remains: the region tree is covered, or
the outer query is UNSAT. Stopping after a requested model count does not
establish exhaustion. `statistics()` includes work from incomplete attempts.

`StableModels::restrict_candidates(&Theory)` appends a candidate-only classical
constraint over the same semantic atom count and index meanings. The regions
proposer narrows the regions still to visit by it and continues, since a
visited region was covered under the original theory and a restriction only
removes candidates. The clauses proposer restarts only the outer cursor,
retaining previous restrictions and exact blocks; encoding failure restores
the previous CNF and cursor, and charged work remains. The original theory
returned by `theory()` and every reduct check remain unchanged. After any
successful restriction, exhaustion covers only their intersection.

## Proposing candidates by regions

Under `SearchMethod::Regions`, reachable in the solve session as
`--search regions`, no clause form of the theory is built. The candidate space is the coverage tree of
`Search.lean` over the theory's atoms, walked by `zetesis_cpu::regions`: the
root leaves every atom open, each region is narrowed by
`zetesis_ferraris::narrow` to the fixed point of the theory's readings, with
the theory's producers for the support cut, and by every candidate-only
restriction without producers, since a restriction supports nothing. A region
the readings refute is dropped with its whole subtree; a region with an open
atom is split on the atom its narrowing prefers, the open atom with the most
parents still unknown, as the clause search branches on the variable with
the most unresolved occurrences, cut branch first; a region with every atom
decided is a leaf, and a leaf is a
classical model of the theory and the restrictions, because at a full
decision every root is sure or impossible and an impossible root refutes.
The leaf is the proposal, and the reduct decides it: by a complete class
certificate when one applies, else by the proper-subset query, which under
this method is a second region tree. Its root cuts every atom outside the
candidate and leaves the candidate's atoms open; its regions are narrowed
by the knowledge of the frozen reduct, read as the original DAG under the
candidate's truth mask (`FerrarisMask`), with no support cut, since a model
of the reduct need not be supported; a leaf other than the candidate is a
proper-subset model and the validated countermodel, and a covered tree
proves stability (`ReductRegions.stable_iff_no_countermodel`). No reduct
encoding is prepared. Under `SearchMethod::Clauses` the prepared reduct
encoding and the clause query serve instead, and the positive
certificate's unit restriction, which is clause-only, applies.

The narrowing is driven by a worklist over an index of the theory, built
once: a node or atom that learns something is revisited once, and only its
parents, operands and dependent producers are read, as unit propagation
over watched clauses touches only what moved. What a narrowing knows about
a region travels with the region: a split clones the knowledge into both
children, so a child's narrowing starts from its parent's and learns only
what the split decided (`FormulaBounds.known_mono`), and the regions still
share nothing. The reduct query carries its knowledge the same way. Node visits and producer
checks are charged as search work and each split as a decision, against
the same cumulative `SearchLimits`. Regions are the default method; the
clauses remain a method a session may select.
`Statistics::regions` reports regions visited, refuted and reached as leaves,
propagations (a node learned and its parents revisited, a chain learning by
one counter step, or an atom's support rechecked), atoms held and cut, whether the support cut applied, and
the reading work, and `Statistics::reduct.regions` the same for the reduct
queries; `candidate_queries` and the projection history stay zero,
since no classical query is asked and no exclusion index is kept. Laws:
`FormulaBounds.lean` for the readings, the three rules and the leaf
(`decided_leaf_models`), `Search.lean` for the tree. See [the proposer](src/regions.rs) and
[its propositions](tests/regions.rs).
An optimizer must separately prove that excluded stable candidates are dominated
by an already verified incumbent, and use a non-strict bound to preserve ties.

The initial optional [necessary support filter](docs/candidate-pruning.md#initial-necessary-support)
recognizes complete mixed ordinary-disjunctive and exact atomic-choice producers.
Its status and setup work are recorded separately; original reduct checking
remains authoritative even when necessary support removes proposals.

`enable_certified_checking(TightPlanLimits)` retains the tight-only library door.
`enable_class_checking(CertificateLimits, CertificateOrder)` tries ranked support
and positive least consequences in the requested order. Each plan checks the
complete original theory; a source class hint can choose order but cannot bypass
applicability. Positive producers allow cycles and monotone conjunction/disjunction
bodies. The primitive evaluates every original constraint on the completed least
interpretation; a failed constraint excludes answer sets, without claiming that
there are no classical models. Choices and unsupported producer bodies decline
this specialization.

A successful positive plan adds one unit for every original semantic atom, or
one empty candidate clause when the least interpretation violates a constraint.
The units preserve all answer sets and avoid an exponential walk through other
classical models. They use the existing CNF arena and admission, introduce no
auxiliary atom or copied restriction DAG, and roll back together on failure.
The immutable original theory remains the subject. Each emitted candidate must
match the exact owner and least interpretation and independently satisfy that
original theory. Complete positive execution therefore needs no reduct query;
ranked support may still leave residual candidates for exact completion.
The [constrained-positive laws](../../proofs/guide/constrained-positive.md)
separate positive producer closure from arbitrary original constraint filtering;
their premises do not establish the Rust CNF or resource refinement.

The certificate statistics API now names the selected algorithm explicitly.
Consumers of the former tight-only `plan`, `refusal` and
`Incomplete::Certificate` payloads match `CertificatePlanStatistics` and
`CertificateError`; the tight-only enabling method retains its original policy.

`CertifiedStatistics::plan` distinguishes `Tight` and `Positive`. Actual refused
attempts remain visible even when a later plan succeeds. Construction, exact-unit
restriction and candidate-check work are disjoint subsets of cumulative SAT work;
none receives a fresh run budget. Optional construction or unit-dimension
refusal leaves fallback available. Control, allocation and cumulative work failures
preserve the original stop and cannot establish exhaustion. A partially appended
unit restriction is never installed. Repeated configuration retains its first
attempt, and configuration after candidate generation is refused.

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

Exact semantic blocks are installed as candidates are proposed. A retained
pending batch accounts for those candidates until the whole batch commits.
Checker, shape and pending-capacity failures keep that batch retryable; retries
recheck the supplied limits. Search failures and delayed proposal/blocking
failures cannot establish exhaustion. Successful calls return ordered verified
models; a consumer still owns scoring, output and any queued result accounting.
New objective restrictions apply only to future candidate generation and never
modify the original reduct theory or discard pending proposals.

`batch_statistics()` reports checker calls, committed candidates, propagated
decisions, exact residual completions and pending proposals. These counters
describe batch calls; the general candidate count can also include scalar
iterator calls if a library caller alternates modes after draining each batch.
Switching to scalar iteration with pending work is refused. The CLI uses one
mode throughout a session. The [batch accounting laws](../../proofs/Zetesis/BatchAccounting.lean)
prove semantic coverage and occurrence conservation under explicit producer and
checker assumptions; they do not prove this implementation refines the ledger.

`CompletionExecutor::new(workers)` builds one reusable Rayon pool when more than
one worker is requested. `next_batch_with_completion(limits, &mut executor,
checker)` uses it for independent exact residual checks. Proposal generation,
the supplied checker, objective feedback and publication remain on the calling
thread. `next_batch` uses a bounded single-worker executor. Ordinary CPU CLI
execution retains its scalar cursor by default; `--completion-workers 2` (or
another value above one) opts into reusable batched completion. GPU formula
batches use this same executor for exact residual checks.

Each worker receives only the immutable original theory, its matching candidate,
and limits. A query leases at most 64 work permits from the enumeration's shared
allowance, then consumes them locally before charged operations. Settlement
records consumed permits and returns unused ones on completion or failure.
Waiting workers cannot declare exhaustion while a grant can still return work.
Decisions retain individual atomic reservations. No worker gets a fresh full-run
quota. A private statically selected quota policy keeps scalar
queries free of shared-pointer checks while reusing the same parameter and search
operations for parallel completion. Every job joins before counters are merged and before
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
completion-scratch ceiling; `new` and `default` use 256 MiB. The enumeration
lazily builds one `PreparedReduct` at the first actual residual, using cumulative
SAT work, its `Limits::reduct_admission` dimensions and independent
`Limits::max_reduct_bytes` ceiling (64 MiB by default). `Limits::admission`
bounds the original candidate CNF and submitted restrictions; it does not
bound the distinct reduct owner. The explicit fresh `check` uses
`reduct_admission` too. All-certified execution does not construct it. Scalar checks and joined
residual workers borrow the same immutable owner across candidates, batches and
candidate-only restrictions. Shared original theory remains unchanged.

Completion first admits result-only storage, then prepares if required.
`prepared.scratch_requirements(candidates)` uses actual compiled CNF dimensions
and retained owner capacity. Its `shared_bytes` is charged once; `query_bytes`
contains one worker's truth, parameter, search and witness-transient slots;
`result_bytes` covers the whole ordered batch. The source-only
`CompletionExecutor::scratch_requirements` instead estimates the parametric
worst-case shape, treating every node as a possible implication; it constructs
nothing and does not predict allocator slack. Actual worker capacities are
checked after reservation and before candidate work. A scratch refusal may
retain the newly prepared owner and cold-work receipt for a later retry.

Worker scratch is released between batches. The scalar enumerator retains one
workspace until it is dropped or switches to batched completion; that switch
releases scalar scratch while preserving preparation. Every query recomputes
original truth once, replaces candidate/truth parameters, and resets assignments,
watches and decision state. The CNF, its gate structure and proper-subset
constraints are not rebuilt. The caller owns returned models after publication.

All result slots plus at most the requested number of query envelopes are
admitted together. The scratch ceiling may reduce query concurrency to one.
If results plus one required query cannot fit, `BatchError::Limits` carries
`Incomplete::CompletionScratch`; the original proposals remain retryable using
an executor with a sufficient allowance. Certificate-only batches need result
storage and any already retained shared owner, but no query workspace. Work/decision or semantic failures still follow
the existing terminal-search contract. `last_statistics` distinguishes requested
workers, preflight concurrency, requested minimum bytes, observed capacity plus
transient bytes, and residuals entered, completed locally or failed. Reserved
capacity can exceed the minimum request. Its ceiling is checked before candidate
work begins; a refusal can therefore report a peak above the limit with zero
entered candidates. Local success is not batch publication.

The accounting uses the shared prepared owner and retained worker-vector
capacities, plus a conservative allowance for transient and result storage. Hash-table
bucket/control overhead, allocator metadata, Rayon scheduling storage, thread
stacks, immutable original theory and the separate proposal/candidate cursor are
excluded. It does not bound RSS or GPU memory. `max_pending_bytes` retains its
independent pending-interpretation contract. No benchmark or device speedup is
established by these portable completion tests.

`StableModels::enable_phase_timing()` optionally records coarse host wall intervals
in `statistics().phase_timings`. Under several region workers the intervals are
the workers' own narrowing and leaf decisions summed over the workers, which
may exceed the wall time; the coordinator's wait for their models is not a
phase. Enabling is idempotent and starts after initial
candidate CNF construction. Separate measurements cover candidate queries,
projection and exact blocking; independent original-model validation; and frozen
cold reduct preparation; parameter installation, exact search and witness
validation. Cold preparation has its own interval and is not added to worker
intervals. Candidate restrictions are
excluded for the caller to measure alongside objective feedback. Callback/device
execution is also excluded. The CLI enables this only for `--stats`.

Timing counts are attempted calls, including errors, and may differ from query
or model counts. Zero calls means unentered. Counter/duration overflow marks the
measurement incomplete and retains its earlier prefix without interrupting the
solver. Disabled timing reads no clock and allocates no timing-specific heap
storage. These measurements are independent of deterministic work/decision
ceilings and do not establish semantic completion or refine the Lean model.

## Semantic construction

Original atom indices remain `0..theory.atom_count()`. The encoder maps each
query node to a Boolean constant or signed variable reference. Constant identities,
idempotence and complementary literals compact the classical query. Atom nodes
alias their original variables. Remaining gates use canonical signed-input AND
keys: OR uses negated inputs and a negated output. A query-local map reuses an
existing gate for the same pair of signed inputs in either order. Every distinct
gate receives a fresh auxiliary with full equivalence (three clauses), giving every original
interpretation exactly one auxiliary extension. Only semantic atoms participate
in interpretations, subset tests and model blocking.

Compaction is local to a query expression. It never rewrites the stored Ferraris
theory: the fresh encoder fixes original truth before composing reduct children,
and the persistent network retains explicit original-truth guards as parameters.
For example, `a or not a` can be constant true in the outer query but must reduce
to `a` in the reduct of `{a}`. Classically equivalent source formulas cannot in
general be substituted before establishing their reduct meanings.

For a candidate M, `EvaluationWorkspace` evaluates every original DAG node and
checks original satisfaction, producing a borrow tied to that exact interpretation
and theory. One persistent network reads prospective subset N from the original
atom slots. And/Or compose child reduct values; implication reads
`original_truth_M(implication) AND (NOT reduct(left) OR reduct(right))`.
Under N subset M, a false-in-M atom cannot be in N, and false And/Or nodes
collapse through their child reducts. Thus only implication nodes need explicit
original-truth parameters. Classical equivalences of the original false nodes
are never reused as reduct equivalences.

The CNF asserts every reduct root, N subset M, and at least one M atom absent
from N. With no atoms, strictness is an empty clause. Each query supplies M and
implication truth as level-zero assumptions. Backtracking cannot retract them.
A completed assignment that propagation left without conflict satisfies every
clause, since a clause with both watches false would have propagated or
conflicted; the truth-table tests state that property, a debug build re-checks
each witness against the clauses, and a release build does not rescan them.
A returned witness is still independently checked for properness and by
`zetesis-ferraris::models_reduct`. Only completed UNSAT establishes stability.
No least-model assumption or recursive stability query for N is used.
The [parametric reduct laws](../../proofs/Zetesis/ParametricReduct.lean) prove the
formula-level equivalence under subset and authentic-truth premises; the
[guide](../../proofs/guide/parametric-reduct.md) separates that argument from
Rust DAG/CNF, owner, search and resource obligations.

After checking a candidate, the clauses proposer records the exclusion that
disagrees with that candidate on at least one original atom. It excludes
exactly `M` and does not generalize countermodel evidence. If this blocking
operation fails after a stable model was already proved, the iterator returns
that model and emits the pending failure on its next call; coverage remains
incomplete. The regions proposer needs no exclusion: a leaf is visited once.

## Walking the tree with several workers

`StableModels::with_region_workers(&Theory, workers, Limits, Control)` walks
the region tree with that many workers at once. Each worker owns a stack of
regions with their knowledge, a budget leased from the enumeration's shared
allowance, its own index for the reduct query and its own evaluation
workspace; the workers share a pool of regions still to visit and nothing
else. A worker narrows a region, drops it when refuted, splits it and keeps
both children, offering one to the pool when the pool runs short, and at a
leaf decides membership as the scalar walk does, by the class certificate
when one applies and else by the proper-subset query, sending a stable
model to the enumeration. The regions partition the candidate space exactly
(`Search.split_partition`, `split_disjoint`), so every stable model arrives
once and none is missed, whatever the interleaving; the order of arrival is
the schedule's, differs between runs, and is not a property of the result.
A restriction added while the workers run narrows the regions not yet
visited; models already on their way are returned as they are, as the
scalar contract already allows. The work, decision and candidate ceilings
are shared: the first worker to exhaust one raises the stop and the others
stop at their next charge or their next region; the models the workers
verified before they stopped are delivered first and the stop after them,
so a leaf admitted under the candidate ceiling is never lost to a worker
the ceiling refused. The region counts and reading work, the candidates, the
countermodel counts and the phase timings are the workers' live counters,
current while they run; the certificate and reduct receipts are merged when
the workers finish, so a snapshot taken earlier lacks them. One worker is
the scalar regions walk. The batched protocol is not used with workers,
since the workers decide their leaves themselves.

## Search and limits

The Boolean search serves the reduct's proper-subset query and the outer
candidate query under the clauses method, and nothing under regions. It is
deterministic, iterative chronological DPLL. It uses false-first
branching, a heap trail and decision frames, and two watched literals. After
initial unit propagation it counts unassigned variable occurrences in unresolved
clauses. A bounded mergesort forms a complete permutation, highest count first
with variable-index tie breaking. Backtracking restores the cursor by permutation
rank. The heuristic changes only visitation order: clauses, both branch regions,
complete witness checking and exhaustion obligations remain unchanged. It reads
no domain names, source identifiers or benchmark metadata. Intrusive watch lists allocate exactly two watch
nodes per original clause, avoiding accumulated per-list capacities during watch moves.
No recursion depends on formula or decision depth. A completed SAT assignment
is checked against the clauses independently of the watch structure.

Stable-model enumeration retains the original candidate CNF traversal across
yields. Its watch lists, trail, decision frames and initial branching permutation
are initialized once per fixed candidate region. The initial region skips the
optional probe. After a successful candidate restriction, root failed-literal
probing tries each unassigned semantic variable in both polarities using unit propagation.
A conflict certifies its opposite as a root assignment; successful trials are
fully undone. Only the outer candidate query uses this precheck. Resumption undoes the last complete leaf and explores the
next unvisited region. Exact semantic exclusions are inserted directly into one
persistent binary trie. They are not materialized as blocking clauses or
registered in watch lists, including after a candidate restriction restarts
traversal. The index has the fixed original atom width; no auxiliary variable
can enter a key. A completed assignment must be absent from this index before it
counts as another candidate. This also excludes repeated semantic projections
when a classical encoding admits multiple auxiliary extensions. No such
uniqueness premise is needed for enumeration coverage.

Exhaustion means the original finite traversal has no unvisited base-CNF model
whose semantic projection remains unblocked. Both accepted and rejected
candidates receive the same exact block after their reduct check completes.
An incomplete check terminates the iterator with incomplete coverage. Each inner
reduct query still receives fresh search state and authenticated original-truth
parameters; independent countermodel validation remains unchanged.

`AdmissionLimits` bounds variables, submitted clauses and submitted literal
occurrences, including duplicates and tautologies before canonicalization.
Candidate restrictions consume this actual CNF population. Exact exclusions
instead use `Limits::projections`, independently of each candidate/reduct CNF.
Arithmetic and watch-index representability are checked. Storage reservations
are fallible. These are shape bounds, not a claim that a configured count is a
particular number of bytes or that allocator bookkeeping is measured as work.

The persistent fixed encoding has its own actual submitted CNF population under
`Limits::reduct_admission`, independent of `Limits::admission` for the candidate
CNF. The explicit fresh check also uses `reduct_admission`; a limit fitted to one
simplified reduct need not admit every parameterized query. `Limits::max_reduct_bytes` separately bounds
named construction storage and a retained query worker, each independently.
Query bytes include actual vector capacity, including capacity from earlier calls;
construction also names gate-map entry capacity. Lowering a query ceiling below
retained capacity refuses before new work. Refused proposals do not count as
allocated peaks. Reservation overlap, Arc/allocator/hash-control metadata,
shared original theory and bounded local variables are excluded. Independent
witness/output transients keep their shape bounds and are additionally included
in aggregate completion's transient allowance. These names are not process RSS.

`Statistics::reduct` retains the cold preparation attempt, actual original
node/root work of prepared queries, parameter work (a subset of SAT work), and
maximum single-worker retained capacity observed on return. Original evaluation
and returned-witness verification have independent per-call verification ceilings;
proposal validation is a separate operation. The preparation work is also a
subset of SAT work. Parallel aggregation sums actual query work and takes the
maximum single-worker capacity, while completion reports aggregate live storage.


`ProjectionLimits` independently bounds distinct complete keys, logical trie
nodes and named history capacity. Defaults are 1,000,000 entries, 12,582,913 nodes
and 128 MiB. Duplicate insertion consumes no new entry. The zero-atom universe
has one possible excluded key and needs no nodes. Bytes include the history
header and actual node-vector capacity; growth conservatively admits the old
and new capacities together. The minimum requested capacity is checked before
reservation and actual capacity is checked afterward, before key publication.
A refusal can retain enlarged capacity, but never a partial key. Allocator
metadata, bounded local variables and other SAT owners are excluded: this is
not RSS or an operating-system allocation quota.

`Incomplete::ProjectionLimit` names the refused dimension and its required and
allowed quantities. A scalar answer whose membership was already checked is
returned before a delayed history stop. A batch likewise keeps its proposed
candidates available for checking before surfacing the original history stop.
Neither behavior establishes exhaustion. `Statistics::projections` derives
entry/node counts, current capacity, conservative peak capacity and trie work
from the single history owner; failed attempts retain spent work and capacity,
but do not increment published-key counts. Standalone membership owns no history
and reports none of that work.

`SearchLimits` bounds charged work and fresh decision frames. Work includes
initializing each search-state entry, watched-clause and literal examination,
occurrence counting, merge-sort output cells, branch scans, trail undo and witness
validation, resumption, failed-literal trial propagation and undo, projection
index insertion and lookup. Trie work in the projection receipt is a subset of
the cumulative search work, not another allowance or an amount to add again.
Amortized trie growth is fallible. Formula runs also charge
encoding-node and literal operations, fixed-size gate-key lookups, semantic
projection and exact exclusion. No work is charged for copying nonexistent
blocking literals.

For Rust callers migrating from the earlier joint CNF/history limit, set
`Limits::projections` explicitly when a nondefault history population is needed;
lowering CNF clauses or literals no longer limits the number of excluded keys.
`ProjectionLimits::DEFAULT` is also available in constant configurations.
The history receipt is derived on read, so `StableModels::statistics` is no
longer a `const` method.
The fallibly grown gate map retains at most one entry per fresh auxiliary;
hash-table allocation and collision handling are shape-bounded library operations.
Allocation, sorting inside CNF admission and allocator overhead are governed
by finite shape limits rather than counted as SAT primitive work. Cancellation
and deadlines are polled at charged work boundaries.

For a `StableModels` run these search counters are cumulative across all outer
and inner queries. `max_candidates` bounds classical candidates checked, and
still permits a final UNSAT query after exactly that many candidates.
`max_verification_work` separately bounds each independent original/reduct
evaluator call; evaluator work is not included in SAT statistics. Exhausting
any bound produces an explicit incomplete outcome, never a logical rejection,
stability decision or coverage proof.

The [retained-cursor note](docs/candidate-cursor.md) records its invariant,
resource boundary, bounded measurement and Lean specification. The
[candidate-pruning note](docs/candidate-pruning.md) covers restrictions, probing
and exact projection indexing.

## Validation and current scope

Portable tests compare 5,874 tiny canonical and noncanonical CNFs with truth
tables, exercise exact admission/search ceilings and cancellation, and run
1,024 independent choices on a 64 KiB thread stack. Formula tests compare
membership for every tiny interpretation and complete unique model sets
against the exhaustive reference, including 256 generated DAG cases per run.
Retained-cursor tests additionally enumerate all 522 canonical CNFs through two
variables, including the empty universe, plus a noncanonical CNF. They compare
complete model sets with truth tables, test semantic projection blocking with
free auxiliary variables, late constraints, cancellation between yields and
exact cumulative ceilings. A separate six-variable comparison checks complete
parity and lower charged work against restarting search after each exact block.
Named cases cover disjunction, choices, double negation, positive cycles,
constraints, unused atoms and zero atoms. The Rust code is tested; the Lean
development does not yet prove its Tseitin, watched-literal or trail refinement.

This implementation has no clause learning, support or loop nogoods, learned
countermodel generalization, dynamic propagation of appended clauses, objective
evaluation or parallel search within a single query. It accepts candidate-only constraints from an
external optimizer. Retaining the outer traversal avoids revisiting its
completed decision regions, but late blocks only filter full assignments; this
is not guaranteed to improve every theory. Each reduct is encoded anew. Unused semantic atoms can
still cause exponentially many classical candidates before exhaustion. The normal-rule bridge in `zetesis-ferraris` can add justified double-negated
supportedness guards; arbitrary theories do not gain this assumption. Horn
minimality specialization remains separate.

The crate consumes an already finite `Theory`. Source grounding, richer choice
and aggregate translations, tuple-set semantics, output projection and
optimization belong to separate boundaries. Passing this kernel's tests is
not acceptance of an original kr-domains case or full clingo compatibility.
