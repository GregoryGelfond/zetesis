# Shared CPU source rounds

The optional shared CPU route checks the same ordered candidate occurrences
through immutable reduct rounds. It reuses the relational source protocol used
by lazy device execution and evaluates its per-world consequences on an owned
Rayon pool. Ordinary solving still defaults to independent candidate joins.
No comparative speed or process-memory improvement is established by adding
this route.

## Library composition

`zetesis_cpu::BatchOracle::check_shared` owns admission and execution. It takes
the original admitted `Program`, ordered `Seed` occurrences,
`lazy::shared::Limits`, existing `SourceSelection::{Union, Worlds}`, and
`Control`. It uses the pool's existing nonblocking admission slot. The original
`check_batch` method and its independent per-candidate stop semantics are
unchanged.

Within each round the source visitor reads a fixed union carrier and immutable
per-world snapshots. Union exhausts the positive source relation. Worlds omits
only prefixes whose antecedents share no current world; every offered rule still
checks all positive antecedents and frozen true/false gates independently in
each world. Completed chunks contribute deltas only after output validation.
The next snapshot is committed only after complete per-world round coverage.

The concrete CPU evaluator and portable scalar reference call the same private
per-world consequence function. Workers write disjoint output slices in input
occurrence order; duplicate seeds retain distinct slices and records. Gate truth
is always read from the frozen seed, rather than a growing closure.

A successful call returns one complete closure/verdict per input occurrence.
If any source operation or world stops, the whole batch is incomplete. A
`shared::Failure` retains source progress and every allocated world record,
but contains no complete checks. The first observed failing world in input
order is identified; this is not a claim about wall-clock failure order. There
is no retry or independent-oracle completion. Admission errors remain distinct
from incomplete source/world execution.

The returned public records can be edited by their caller. Their guarantees
apply to unmodified operation results; they are not proof receipts.

## Limits and work

The source limit owns the collective catalog, chunks, source work, rounds and
requested host payload. It also reserves the retained per-world progress vector
before allocation. Worker outputs use the already-accounted single chunk result
buffer; no private per-worker output copies are created. Original program and
seed ownership, Rayon pool/thread stacks, allocator rounding and tree-node
bookkeeping remain outside the existing source-payload contract.

World work counts one record visit and one operation per antecedent test,
cumulatively across chunks and rounds. A record visit is included in world work,
not an additional charge. It is a different unit from independent CPU join/copy
work and shared source work. Exact work ceilings are inclusive. Complete model
parity does not imply equal resource stopping points across these policies.

Control is polled before source setup, before world allocation/evaluation, at
each world work step and before successful evaluator return. Workers finish
their attempted chunk before the indexed failure is selected. Cancellation can
therefore leave different observed work prefixes; such prefixes establish no
completed closure.

## Ordinary use and reporting

Advanced `--source-batching independent|union|worlds` selects the source policy.
Union and Worlds require a relational closure program, lazy/auto grounding and
CPU/auto backend. An explicit shared policy with auto backend selects CPU
without device discovery; explicit incompatible requests fail as route
capabilities, not source-invalidity claims. Prepared formula and ground inputs
cannot silently ignore the requested policy.

```sh
zetesis example.lp --models 0 --source-batching worlds --stats
zetesis example.lp --models 0 --backend cpu --grounder lazy \
  --source-batching union --max-source-work 10000000 --max-work 10000000
```

`--max-source-work` bounds collective source work per shared CPU batch.
`--max-work` bounds world evaluation. `--max-atoms` bounds its collective
catalog, and `--max-batch-bytes` supplies the full source/world host allowance.
Independent and lazy-device resource settings retain their previous meanings.
The new flags appear in `--help-all`, preserving the short ordinary help.

Writer-free sessions expose `SharedExecutionStatistics`. Ordinary reports and
optional JSON statistics use a separate `shared_execution` record, never
inventing a device adapter, dispatch or transfer. Source and world work remain
separate. Submitted, complete, stopped and queued occurrences distinguish
membership from publication. A failed batch marks all its submitted occurrences
stopped; the terminal session interruption is a control result, not a candidate
rejection. Output failure retains already completed membership evidence.
The legacy `checked` counter counts consumed closure result/control records:
one whole-batch interruption is one stop record. Use the separate shared
submitted/completed/stopped counters for occurrence accounting after interruption.

## Qualification and measurement scope

Portable controls exercise distinct sparse/dense seeds, complete ordered
closures, default-negation gates, mismatches, constraints, duplicate occurrences,
pool occupancy, wrong program identity, source/catalog/host/world limits and
ordinary publication boundaries. Existing `LazyRounds` and `WorldMasks`
Lean laws provide the consequence and coverage statements reused here. This
composition adds no new stable-model semantics. Rayon scheduling, Rust
allocation, cancellation observation and implementation refinement remain
verification boundaries; no new global proof count is claimed.

The existing `lazy_joins` Criterion target now includes the
`shared-source-distinct` group. For domains of 8 and 32 values, each sparse
seed selects a different singleton, and each dense seed omits a different value.
A binary join produces pairs; a fixed constraint supplies complete rejection
controls in the dense case. Each input occurrence has a directly constructed
expected closure. All routes use the same original program, ordered seeds and
owned four-worker pool. Shared Union/Worlds and independent Rayon checks must
match those closures and verdicts before timing and after every observed call.

Timers cover complete check calls, including source state, work accounting and
result construction. Program/pool construction and result validation are outside
each call timer. Qualification prints source/world counters separately; no
process RSS is inferred. Criterion's fixed route order and synthetic workloads
do not establish a general solver ranking. Accepted comparisons require a
coordinated quiet window with retained source/build identities and raw results.
`cargo test -p zetesis-cpu --bench lazy_joins -- --test` is a bounded fixture
qualification, not an accepted timing campaign.
