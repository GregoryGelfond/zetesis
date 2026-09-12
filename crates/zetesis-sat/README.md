# zetesis-sat

Native, bounded Boolean search for finite Ferraris theories. This library uses
no external SAT engine or clingo at runtime. It complements the existing
exhaustive formula reference and the S0 Horn closure implementation.

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

`check(&Theory, &Interpretation, Limits, &Control)` returns
`Check::Stable`, `Check::NotModel`, `Check::NonMinimal(witness)` or
`Check::Inconclusive(reason)`. `Check::accepted()` is true only for `Stable`.

`StableModels::new(&Theory, Limits, Control)` builds the candidate CNF and
returns an iterator of `Result<Interpretation, Incomplete>`. Interpretations
retain the original theory's immutable instance identity. The iterator is
fused: after exhaustion or one terminal error it returns `None` forever.
`exhausted()` becomes true only after the outer query proves that no unblocked
classical candidate satisfying all successful candidate restrictions remains. Stopping after a requested model count does not
establish exhaustion. `statistics()` includes work from incomplete attempts.

`StableModels::restrict_candidates(&Theory)` appends a candidate-only classical
constraint over the same semantic atom count and index meanings. It restarts
only the outer cursor, retaining previous restrictions and exact blocks. The
original theory returned by `theory()` and every reduct check remain unchanged.
Encoding failure restores the previous CNF and cursor; charged work remains.
After any successful restriction, exhaustion covers only their intersection.
An optimizer must separately prove that excluded stable candidates are dominated
by an already verified incumbent, and use a non-strict bound to preserve ties.

`StableModels::next_batch(BatchLimits, checker)` separates complete original-model
proposals from membership checking. The checker receives the immutable original
theory and an ordered, bounded slice, and returns one `BatchVerdict` per candidate.
`NoProperSubset` is a trusted completion-preserving checker's stability result;
`Residual` invokes exact native reduct search. A `NotModel` result for these
independently validated original-model proposals is an invalid-witness error.
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
and limits. Search work and decisions are reserved atomically against the
enumeration's cumulative allowance before the charged operation. No worker gets
a fresh full-run quota. A private statically selected quota policy keeps scalar
queries free of shared-pointer checks while reusing the same encoding and search
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
logical completion-scratch ceiling; `new` and `default` use 256 MiB. Before any
completion-owned result/query allocation, `scratch_requirements` computes a
conservative envelope from original atom/node/root counts and CNF limits.
It includes all ordered outcome slots, accepted flags, output interpretation
slots, CNF clause/literal capacity, frozen/node buffers, reserved alias-map
entries, search state and ordering arrays, assignments and independent witness
validation. Unused reserved slots remain charged. Bounded encoding reserves its
clause registry and alias-map entry allowance before filling them. Query vectors
are dropped before the next query in a worker; no workspace is retained between
calls. The caller owns returned model storage after a successful batch.

All result slots plus at most the requested number of query envelopes are
admitted together. The scratch ceiling may reduce query concurrency to one.
If results plus one required query cannot fit, `BatchError::Limits` carries
`Incomplete::CompletionScratch`; the original proposals remain retryable using
an executor with a sufficient allowance. Certificate-only batches need result
storage but no query workspace. Work/decision or semantic failures still follow
the existing terminal-search contract. `last_statistics` distinguishes requested
workers, admitted maximum concurrency, peak logical bytes, and residuals entered,
completed locally or failed. Local success is not batch publication.

This is deliberately a **logical authored-storage** contract: requested typed
slots are charged, including retained unused slots, but allocator rounding,
hash-table bucket/control overhead, Rayon scheduling storage, thread stacks,
immutable original theory, and the separate proposal/candidate cursor are not
counted. It does not bound RSS or GPU memory. `max_pending_bytes` retains its
independent pending-interpretation contract. No benchmark or device speedup is
established by these portable completion tests.

`StableModels::enable_phase_timing()` optionally records coarse host wall intervals
in `statistics().phase_timings`. Enabling is idempotent and starts after initial
candidate CNF construction. Separate measurements cover candidate queries,
projection and exact blocking; independent original-model validation; and frozen
reduct encoding, exact search and witness validation. Candidate restrictions are
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

Compaction is local to a classical query. It never rewrites the stored Ferraris
theory before reduct formation: the original candidate truth mask is fixed first.
For example, `a or not a` can be constant true in the outer query but must reduce
to `a` in the reduct of `{a}`. Classically equivalent source formulas cannot in
general be substituted before this masking step.

For a candidate `M`, the independent `zetesis-ferraris::models` evaluator first
checks the original theory. The encoder evaluates every original node once in
`M`. Every `M`-false node becomes falsum; every other node keeps its connective
over the reduct values of its children. This is the same all-node masking
interpretation used by the reference kernel. The frozen mask never changes
while searching an inner interpretation `J`.

The inner CNF asserts the frozen reduct, makes every original atom outside `M`
false, and asserts that at least one original member of `M` is false. Thus its
semantic models are exactly the proper subsets satisfying the reduct. For an
empty candidate the last condition is an empty clause, correctly ruling out a
proper subset. An inner SAT result is checked independently for strict subset
membership and by `zetesis-ferraris::models_reduct`. Only a completed inner
UNSAT query establishes stability. No least-model assumption or recursive
stability query for `J` is used.

After checking a candidate, enumeration adds the clause that disagrees with
that candidate on at least one original atom. It excludes exactly `M` and does
not generalize countermodel evidence. If this blocking operation fails after
a stable model was already proved, the iterator returns that model and emits
the pending failure on its next call; coverage remains incomplete.

## Search and limits

Search is deterministic, iterative chronological DPLL. It uses false-first
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
reduct query still receives fresh search state; its frozen mask and independent
countermodel validation are unchanged.

`AdmissionLimits` bounds variables, submitted clauses and submitted literal
occurrences, including duplicates and tautologies before canonicalization.
An exact exclusion consumes one logical clause unit and its semantic width in
literal units, preserving the joint admission ceiling without retaining those
literal occurrences. These units do not describe allocated CNF storage.
Arithmetic and watch-index representability are checked. Storage reservations
are fallible. These are shape bounds, not a claim that a configured count is a
particular number of bytes or that allocator bookkeeping is measured as work.

`SearchLimits` bounds charged work and fresh decision frames. Work includes
initializing each search-state entry, watched-clause and literal examination,
occurrence counting, merge-sort output cells, branch scans, trail undo and witness
validation, resumption, failed-literal trial propagation and undo, projection
index insertion and lookup. The trie has at most one node per admitted exclusion
bit plus its root; amortized growth is fallible. Formula runs also charge
encoding-node and literal operations, fixed-size gate-key lookups, semantic
projection and exact exclusion. No work is charged for copying nonexistent
blocking literals.
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
