# Parallel and lazy checking

There are two different opportunities to share work: execute independent checks
concurrently, or share source traversal while preserving independent worlds.
Choose a library boundary that makes the distinction explicit.

## Independent candidate checks

`zetesis_cpu::BatchOracle::new` takes nonzero worker and maximum-batch counts and
creates an owned Rayon pool. `check_batch` takes one `Program`, a slice of its
`Seed` values, per-candidate limits and shared control. It returns results in
input order. Duplicate seeds are separate submitted occurrences.

The result has two error layers. An outer `BatchError` can refuse an oversized
submission or a concurrent caller before work starts. Inside an admitted batch,
each candidate has its own `Result<Check, Stop>`. Do not convert failed members
into rejected candidates. There is no unbounded waiting queue: a busy pool
refuses another simultaneous batch. The static variant accepts an already
compiled graph and does not compile one implicitly.

The scalar `check_view` and `check_static_view` operations borrow either an owned
`Seed` or a `SeedSelection`. Their batch counterparts, `check_batch_views` and
`check_static_batch_views`, accept indexed Rayon iterators such as
`selections.par_iter().map(SeedSelection::view)`. The oracle runs them on its
owned pool and collects in input order. These doors share the existing checker
and budgets; they create neither a seed tree nor a temporary view vector.
The owned-seed methods delegate through views. Derived closures retain their
existing output allocation and ownership contracts.

`Candidates::next_selection` retains opaque gate atoms minted in canonical
carrier order. The complete graph's gate-ID list has that same order, so a
selected token resolves by one checked array lookup. Its program, position and
payload cannot be changed separately. The candidate stream checks its carrier
bound and reserves handle storage before sharing each newly discovered token.
Selections sort and deduplicate these positions with integer comparisons;
this still costs `O(n log n)` comparisons for `n` selected handles. Manually
supplied `Arc<Atom>` selections retain typed atom sorting and explicit symbolic
lookup. Both use one entry representation and the same checker, with one work
charge and control poll before each static resolution.
Each discovered token additionally retains a program handle and one position;
the selected-entry variant also occupies more metadata than a bare Arc handle.
The change avoids repeated payload copies and lookup, without promising fewer
bytes for every small candidate.

`GroundProgram::seed_words_into` writes a view directly into exact-width
caller storage. Identity and width errors leave that storage unchanged; successful
packing clears every complement and tail bit. It performs no implicit mapping
registration or cache allocation. Graphs compiled from the same immutable
program share canonical positions; independently admitted equal source remains
foreign. The finite ordering law is
[`GatePositions.retained_position_exact`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/GatePositions.lean),
with Rust's carrier construction, token ownership and machine bounds remaining
explicit correspondence obligations.

Calling `Check::into_stable_interpretation` on an accepted native check transfers
its closure into an instance-bound receipt without copying atoms. Rejection
returns the original check intact. The receipt's `into_interpretation` explicitly discards its program
association. Ordinary CPU session execution uses these consuming operations to
deliver the owned closure after acceptance.

Ordinary formula sessions have a separate bounded exact-completion executor.
Its worker setting is distinct from the relational closure pool. Scalar outer
candidate search does not become parallel simply because residual membership
queries use Rayon.

`SolveConfig::gpu_formula_work` and `gpu_formula_rounds` bound device propagation
per candidate, independently of CPU work and residual-search quotas. Their
defaults match `FormulaLimits`: 100,000,000 charged units and 64 sweeps. A work
limit below mandatory setup refuses before submission; zero sweeps still checks
original roots and returns unresolved candidates for exact completion.
`FormulaExecutionStatistics::gpu_limits` records the effective values when a
device executor exists. Submitted batch/candidate counters survive readback
failure, while existing decoded-result counters include only returned batches.
`GpuFormulaOracle::last_submission_candidates` provides the underlying receipt
after queue submission. It says nothing about completion; an unreturned batch's
shader work and sweeps remain unknown.

The formula device graph keeps original node indices for child references and
frozen truth, but uses dense domain positions only for non-Atom nodes. Atom
leaves resolve to their semantic atom; repeated leaves alias the same position.
The non-Atom positions are disjoint from semantic atoms and from one another.
Initialization visits each original node and writes only those auxiliary
positions; root constraints and enabled gates resolve through the same output
mapping. An original-false gate still disables its connective and constrains its
output to false. Removing unused leaf slots cannot re-enable that connective.

This uses the variable-independent relation contract in
[`Propagation.narrow_models_iff`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Propagation.lean)
and the alias-aware finite transfer in
[`GateProjection`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/GateProjection.lean).
The dense mapping's coverage, injectivity outside leaf aliases, initialized
address range and checked world offsets are Rust/WGSL correspondence obligations;
those abstract laws do not certify the packing or physical execution.

## One source stream, independent world truth

`zetesis_cpu::lazy::check_with` owns the bounded round protocol and accepts an
injected chunk evaluator. Its callback must implement exactly the consequence
relation of `lazy::evaluate`: frozen gates, per-world positive truth, new head
deltas and constraint violations. Matching output length or valid atom IDs
alone cannot establish that semantic contract.

`check_with_source` additionally selects `SourceSelection::Union` or `Worlds`.
`Union` offers joins over the union of derived snapshots. `Worlds` intersects
membership masks to skip prefixes enabled in no current world. Every evaluator
still checks each world's positives and gates. Membership is rebuilt for each
immutable round; neither policy changes candidate-carrier discovery.

`check_with_views`, `check_with_source_views` and
`BatchOracle::check_shared_views` accept exact-size, cloneable iterators of
borrowed seeds. Each clone must preserve occurrence order, length and program
identity. This lets owned seeds and shared selections use the same source and
world evaluator without materializing input trees. The batch interns each
distinct demanded atom once. Canonically ordered IDs select borrowed rows from
the committed catalog while callbacks append to a disjoint tail. No per-round
Union Model or Worlds atom vector duplicates that payload. The visitor still
rebuilds its borrowed relation grouping and copies each bounded source instance.

The catalog grows on demand. Its IDs, frozen seeds, snapshots and pending deltas
survive changes in packed-word width; catalog presence does not establish world
truth. Successful source scans and evaluation commit all newly demanded
identities, including underived heads in a final round with no new consequence.
Finalization transfers the atom vector into one shared Model catalog; each world
retains its own selected positions. Sparse results retain false catalog atoms too.

`max_source_work` now charges seed initialization, identity comparisons and index
maintenance, canonical row selection and catalog commits as well as source joins
and instance copying. One work owner spans the batch; these additional explicit
charges can change resource stopping points without changing reduct semantics.
Packed transport remains batch-owned and the injected evaluator keeps its own
execution accounting.

`max_host_bytes` admits actual catalog/AVL/path and ordered-ID capacities plus
their named growth overlap, nested payload measures, and requested truth, seed,
delta, chunk, result, instance scratch and source membership/workspace storage.
Final model positions have a requested-slot allowance; additional capacity from
their geometrically growing Vec is excluded. Caller inputs, Arc envelopes,
allocator overhead and rounding outside the measured catalog/ordered-ID
capacities, and backend-private transport are also outside that envelope.
The backend must bound its own transport; this is not a process RSS ceiling.

Failure returns `Failure<E>` with its source, protocol or injected execution
cause and accumulated progress. No completed checks are published from a batch
whose required scan or evaluation is incomplete. Failure during identity commit
retains discovered catalog counts and charged work but establishes no completed
round. `Progress::mask_words` includes union-row selection probes under both
policies and is part of source work, not extra work to add again. Optional masks may reduce offered
instances while adding mask work and storage; no universal speedup follows.

Completed lazy checks expose `closure()` for borrowing and `into_closure()` for
ownership transfer. The latter also works on rejected checks: it discards the
program association and verdict, returning a raw interpretation. Leastness and
membership still depend on the injected evaluator's exactness contract. Shared
CPU and lazy GPU session adapters test acceptance before transferring the closure,
preserving input occurrence order and leaving incomplete results on their typed
error paths. Consuming a closure creates no stronger membership evidence.

`zetesis_wgpu::GpuLazyOracle` supplies a device-backed evaluator using the same
protocol. The required adapter and resource validation remain runtime concerns.
Its explicit source-selection wrapper does not change ordinary CLI defaults.
Within one GPU batch, frozen seed uploads are reused while layout and buffer
ownership agree. Positive snapshots are reused only within their owning round.
Catalog-width growth or buffer replacement invalidates the corresponding
receipt. Source records and output resets still occur for each chunk; completion
remains synchronous before the coordinator reuses its chunk storage.
The [execution chapter](../architecture/execution.md) explains why per-world
isolation and whole-round commits are semantic requirements.

## Borrowed GPU candidate inputs

`GpuOracle::check_batch_views` accepts a cloneable exact-size iterator of `SeedView`
values for an already compiled `GroundProgram`. An owned batch maps `Seed::view`;
a shared batch maps `SeedSelection::view`. The existing `check_batch` method
uses this same implementation. Views retain the original program identity and
complete true set without copying atom payloads or allocating a view vector.
Packing writes directly into each candidate's slice of one checked host mask
buffer, preserving occurrence order and zero tail bits. The batch byte allowance
includes that buffer; it no longer needs a separate per-candidate word scratch.

`GpuLazyOracle::check_batch_views` and `check_batch_with_source_views` supply the same views
to the CPU library's lazy round coordinator and use the existing device chunk
evaluator. They preserve the source-selection, final coverage, cancellation and
failure contracts of the owned-seed methods. View construction does not trigger
GPU discovery or static grounding. Lazy batch atom IDs remain independent of
any compiled graph's dense IDs.

Lazy device upload reuse follows the coordinator's immutable inputs.
`Chunk::round_index()` is local to its invoking batch; seeds are frozen for that
batch and snapshots for that round. The private batch-owned transport combines
this index with word width, world count and actual buffer retention. Growth or
replacement forces fresh writes. No pointer, global index or copied host mask
serves as a cache key. The upload receipt is installed after queue submission;
an interrupted read discards that transport. Unknown queue completion or failed
mapping invalidates the context; a late cancellation/deadline after established
completion, successful mapping and released access preserves a healthy context
only after scopes drain and device health is checked. It still returns the
original stop without successful output. Each
new public batch starts fresh, even when its round numbers and layout match.

Actual upload-byte statistics omit those reused prefixes; the byte ceiling
continues to include the conservative full active-input host allowance and all
retained device capacity. Logical source work and device instance counts are
unchanged. The immutable-round law remains the correctness premise; host metadata
and GPU buffer ownership/queue order must implement it, and physical behavior
requires qualification. Dispatch/readback remains synchronous at this boundary.
