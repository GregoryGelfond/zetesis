# Parallel and lazy checking

There are two different opportunities to share work: execute independent checks
concurrently, or share source traversal while preserving independent worlds.
Choose a library boundary that makes the distinction explicit.

## Reuse preparation across scalar checks

`PreparedQueries::new` inspects one admitted relational `Program` and retains
its shared immutable owner plus the dimensions needed by the closure joins.
It does not enumerate the program's ground carrier or hold candidate truth.
Each `check_view` still computes the least closure of its own frozen seed's
reduct and checks the constraints and gate agreement.

This program has the two answer sets `{d(1), left(1)}` and `{d(1), right(1)}`.
The seeds contain only the selected gate atoms. One workspace serves both
checks; the first returned closure remains valid after the second check.
The example compares both complete results with the one-shot checker and the
explicit expected models:

```rust
# extern crate zetesis_core;
# extern crate zetesis_cpu;
{{#include ../examples/prepared-queries.rs:example}}
```

`PreparationLimits` bounds the initial dimension inspections and named retained
preparation bytes; `prepared.statistics()` returns their
`PreparationStatistics` receipt. Each check has its own `Limits` and work
receipt. The one-shot `check` charges preparation and evaluation together, so
equal completed results do not imply identical resource cutoffs. These separate
per-call limits do not bound an entire sequence of checks.

`ClosureWorkspace` retains a canonical tuple authority over the Program's frozen
vocabulary, plus relation and reference-free cursor/undo capacity. A completed
`Check` retains a selection over its immutable prefix. The workspace clears
relation membership, frontiers and pending marks before reuse; retained identity
supplies no previous candidate truth. Failed evaluation discards dirty workspace
state. A different `PreparedQueries` owner retires the old workspace, even
for the same Program, because its inferred dense layouts can differ.
Its `retained_bytes()` includes the authority's shared frozen vocabulary and
workspace metadata. Other source/preparation storage, separately retained results
and documented container/allocator overhead remain outside that receipt;
it is not RSS. Retained capacity is admitted again under each check's limits.
Independent workers need separate mutable workspaces and may share the immutable
prepared owner. This example executes serially and makes no speed claim.

The [CPU API reference](../../doc/zetesis_cpu/index.html) and
[prepared-query implementation](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cpu/src/oracle/prepared.rs)
give the preparation, identity and failure contracts. Run the example directly
with `cargo run --locked -p zetesis-cpu --example book-prepared-queries`.

## Independent candidate checks

`zetesis_cpu::BatchOracle::new` takes nonzero worker and maximum-batch counts and
creates an owned Rayon pool. `check_batch` takes one `Program`, a slice of its
`Seed` values, per-candidate limits and shared control. It returns results in
input order. Duplicate seeds are separate submitted occurrences.

The result has two error layers. An outer `BatchError` can refuse an oversized
submission or a concurrent caller before work starts. Inside an admitted batch,
each candidate has its own `Result<Check, Stop>`. Do not convert failed members
into rejected candidates. `BatchError::Preparation` separately reports a stop
while preparing the shared queries; no candidate result follows from it.
There is no unbounded waiting queue: a busy pool
refuses another simultaneous batch. The static variant accepts an already
compiled graph and does not compile one implicitly.

The scalar `check_view` and `check_static_view` operations borrow either an owned
`Seed` or a `SeedSelection`. Their batch counterparts, `check_batch_views` and
`check_static_batch_views`, accept indexed Rayon iterators such as
`selections.par_iter().map(SeedSelection::view)`. The oracle runs them on its
owned pool and collects in input order. These doors share the existing checker;
they create neither a seed tree nor a temporary view vector.
The owned-seed methods delegate through views. Derived closures retain their
existing output allocation and ownership contracts.

Relational batches retain query preparation for the exact program and use
`with_preparation_limits` to bound it separately from candidate work.
`query_statistics()` reports completed preparation and workspace assignment;
reused slots are not a count of successfully checked candidates. At most one
contiguous range per configured worker owns a reusable workspace. Candidates
inside a range run sequentially, so uneven costs can balance differently from
per-candidate work stealing. Static and shared-round checks retain their separate
preparation and resource contracts.

`Candidates::next_selection` retains opaque gate atoms minted in canonical
carrier order. The complete graph's gate-ID list has that same order, so a
selected token resolves by one checked array lookup. Its Program, tuple
coordinates and position cannot be changed separately. The candidate stream checks its carrier
bound and reserves handle storage before sharing each newly discovered token.
Selections sort and deduplicate these positions with integer comparisons;
this still costs `O(n log n)` comparisons for `n` selected handles. Manually
supplied `Arc<Atom>` descriptions are located in the symbolic carrier during
admission and then released. Their selections retain Program-bound coordinates,
not the ingress payload. Both use one entry representation and the same checker,
with checked static resolution. A `CarrierAtom` can also enter a selection directly
without needing a representable full-carrier ordinal. Each token retains a Program
handle and O(arity) coordinate words; indexed tokens additionally retain a position.
Sharing payload therefore does not promise fewer bytes for every small candidate.

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

The formula CPU route with several region workers performs candidate generation
and membership checking in each worker. Each worker removes the newest region
from its own deque; an idle worker tries to steal the oldest region from a peer.
Each deque has its own mutex. A thief skips a busy deque, and narrowing, payload
cloning, reduct checks and model sends hold no deque lock. Slot growth uses
fallible reservation before either split child is published; refusal leaves
enumeration incomplete. Region and knowledge payload cloning remains infallible,
so this boundary is not an end-to-end guarantee against allocation failure.

The unresolved-region counter gains one when two children replace a parent and
loses one when a region is resolved. Idle workers finish only at zero or an
explicit stop. The resolution that reaches zero, a stop and a close each wake
idle workers at once; otherwise an idle worker looks for a region again and
polls cancellation about once a millisecond. Queued models are delivered before
a worker's stop is reported.
Cancellation observed by the coordinator can end a pull immediately; explicit
`stop` or dropping the enumerator joins any remaining workers.
Local depth-first order bounds each deque's live entries by the atom count plus
one; capacity grows with the observed frontier instead of reserving that worst
case for every worker. Joining releases abandoned entries and queue capacity,
including after a partially successful worker start.

The device route uses a different execution boundary: an owned Rayon pool
produces a bounded candidate batch, joins, and then submits that batch for device
membership checking. CPU residual completion has its own worker setting.
The routes share semantic region readings and split laws, but their scheduling
costs differ. A scaling comparison must record the producer and completion worker
counts, batch size and observed membership route separately; increasing host
workers does not change the number of GPU execution units.

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

### The gate transfer

Each enabled connective narrows its three positions through a finite gate
transfer. `GateProjection::Enumerated`, the default, enumerates the relation's
rows; `GateProjection::Bitwise` is an explicit optional implementation. The
[device constructors](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/src/formula/device.rs) accept
that selection, and the bitwise path assembles the unchanged shader scaffold
with the [bitwise gate transfer](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/src/formula/bitwise.wgsl).

A domain uses two bits: 0 is empty, 1 permits false, 2 permits true and 3
permits either value. A gate returns the supported values at each position; the
caller intersects them with the current physical domain slots. Left, right and
output can alias, and five equality partitions cover the possible physical
identities. The finite space has 960 transfers: three connectives, five
partitions and 64 domain triples. It includes different observed masks at
aliased positions, because separate atomic loads can observe intervening
monotone narrowing; a coherent snapshot is not assumed.

The bitwise transfer intersects eight-bit row sets and projects position
supports, with no table lookup or per-row loop. The row index is `x + 2*y + 4*z`.
And, Or and Implies masks are `0x87`, `0xe1` and `0xd2`. False/true position
masks are `(0x55,0xaa)`, `(0x33,0xcc)` and `(0x0f,0xf0)`. Equality masks for
left=right, left=output and right=output are `0x99`, `0xa5` and `0xc3`; all
equal uses `0x81`. These encode relation rows, not candidate bit planes or the
physical domain-storage representation. An independent reference enumerates the
eight Boolean assignments, checks the connective, masks and aliases, and then
projects support, without the hexadecimal masks.

zetesis-wgpu's [projection tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/tests/integration/formula/projection.rs)
compare all 960 transfers with that reference, and check that intersecting a
stale snapshot's supports into smaller current domains keeps every current
satisfying completion. Its [contract tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/tests/integration/formula/gate_transfer/contract.rs)
check contraction, idempotence, the 10,935 ordered subset-domain pairs for
monotonicity, each position's exact support by a separate nested Boolean
enumeration, the alias partition and the domain encoding.

In `GateProjection`, `bitwise_support_exact` proves equality with independent
Boolean enumeration for all 960 transfers, and `aliased_intersection_exact`
preserves intersections at shared physical slots. These laws do not prove shader
compilation, memory-model behavior, convergence or physical execution. The
production [projection tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/src/formula/projection/tests.rs)
assemble both variants through their actual selector, verify that substitution
changes only the gate-transfer region, validate both modules with pinned Naga and
compare their device interfaces. The
[formula interface contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/tests/integration/formula_interface.rs)
check the host buffer bindings, uniform layout and compute entry point. These
portable contracts make no physical-device claim.

A gate substitution must preserve the three atomic loads and intersections in
order, M-false suppression, original truth evaluation, barriers, strict
proper-subset constraints, epochs, result decoding, charged sweep work and round
limits. It must not turn quiescence into acceptance or a residual into a
completed membership decision. Interleaving safety requires monotonically
shrinking domains for one immutable query, and a new candidate needs a fresh
epoch and reset; exact local transfer alone does not establish the surrounding
propagation or reduct-completion protocol. Physical qualification must exercise
the actual selected implementation, including aliases, frozen masks, limits and
faults. Performance comparison must keep candidates and limits matched, preserve
scalar and Rayon baselines and separate setup from resident calls: enumeration
may already be unrolled by a compiler, and extra bit operations need not be
faster.

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
Union Model or Worlds atom vector duplicates that payload. The visitor rebuilds
borrowed relation grouping and lends one bounded instance of checked keys to its
callback, without copying Atom/Value payload.

The catalog grows on demand. Its IDs, frozen seeds, snapshots and pending deltas
survive changes in packed-word width; catalog presence does not establish world
truth. Successful source scans and evaluation commit all newly demanded
identities, including underived heads in a final round with no new consequence.
Finalization transfers the discovery map and shares its immutable canonical
prefix with one Model catalog; each world retains its own selected positions. Sparse results retain false catalog atoms too.

`max_source_work` now charges seed initialization, identity comparisons and index
maintenance, canonical row selection and catalog commits as well as source joins
and borrowed-instance admission. One work owner spans the batch; these additional explicit
charges can change resource stopping points without changing reduct semantics.
Packed transport remains batch-owned and the injected evaluator keeps its own
execution accounting.

`max_host_bytes` admits actual catalog/AVL/path and ordered-ID capacities plus
their named growth overlap and canonical payload, plus requested truth, seed,
delta, chunk, result, instance scratch and source membership/workspace storage.
Final model positions have a requested-slot allowance; additional capacity from
their geometrically growing Vec is excluded. Caller inputs, Arc envelopes,
allocator overhead and rounding outside the measured catalog/ordered-ID
capacities, and backend-private transport are also outside that envelope.
The backend must bound its own transport; this is not a process RSS ceiling.

Source scans also have an independent `max_scan_bytes` allowance, 128 MiB by
default in both `source::ScanLimits` and lazy `Limits`. It admits actual borrowed
row-directory and row-buffer capacities, assignment/cursor/undo scratch, and one
offered instance's key buffer, including buffer-growth overlap. The Program and
catalog payload, world masks and callback-owned chunks remain under their own
owners and bounds. The simultaneous host envelope therefore composes these
independent allowances; `max_scan_bytes` is not taken from `max_instance_bytes`.
That per-instance limit still measures referenced typed identity and key metadata,
including repeated occurrences. A failed workspace admission offers no instance
and establishes no completed source scan.

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
