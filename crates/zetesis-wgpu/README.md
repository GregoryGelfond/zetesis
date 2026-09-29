# zetesis-wgpu

Bounded GPU primitives for grounding and answer-set checking. CPU and GPU
operations share the same reduct obligations; device scheduling does not establish a different
semantics. The [execution chapter](../../docs/book/architecture/execution.md)
explains their place in ordinary solving, and the
[parallel library guide](../../docs/book/rust/parallel.md) describes composition.

## Choose an operation

| Operation | Subject and result | Obligation outside this operation |
| --- | --- | --- |
| `GpuOracle` | Complete `GroundProgram` and candidate seeds; least closures, constraint failures and gate agreement | Complete candidate enumeration and source admission |
| `GpuLazyOracle` | Relational source instances and frozen per-candidate worlds; synchronized closure rounds | Complete source coverage and final acceptance accounting |
| `GpuFormulaOracle` | Original formula DAG and ordered candidate interpretations; rejection, completed refutation or a residual query | Exact completion of every residual before answer-set acceptance |
| `GpuTightOracle` | A checked `TightPlan` and interpretations; original satisfaction and ranked support | Plan applicability, candidate enumeration and complete result accounting |
| `GpuAggregateOracle` | An `AggregateGpuPlan` and Group-bound eligibility occurrences; count, sum, sum-plus, minimum and maximum | Source completeness, head permission and reduct minimality |
| `GpuRelationExecutor` | One immutable typed relation and equality queries; ordered row masks | Complete pattern matching, source coverage and answer-set checking |

The tight, native aggregate and relation operations are explicit library
operations; ordinary solver dispatch does not currently select them.

Construct an oracle with `GpuOptions` and an explicit `GpuSelection` where
reproducibility requires a particular backend. Selection distinguishes physical
adapters from fallback/software devices and retains adapter metadata. Consult
[public API](src/lib.rs) for exact constructors, limits and result types.

Selection checks each eligible adapter against the requested primitive's
advertised limits before applying the deterministic ranking, then checks the
created device's granted limits. The lazy shader requires no workgroup scratch;
its admission is independent of the static shader's scratch requirement.
Discovery's `supports_static_oracle` report continues to describe only the static
profile. A bare `GpuContext` requires compute support and the chosen identity
policy; a primitive built on that context checks its own granted limits.

`GpuContext` lets distinct primitives retain prepared subjects on one device.
Existing constructors create independent contexts; `from_context` shares device
resources and their failure boundary. Overlapping operations return `Busy`
without waiting. Primitive byte ceilings retain their local scope. See
[execution ownership](../../docs/book/architecture/ownership.md) and the
[ordinary session example](../../docs/book/rust/sessions.md#share-execution-resources).

## Exact completion and failure

A formula propagation fixed point is not itself an answer-set certificate.
Formula domain storage contains one slot per semantic atom and one additional
slot per non-Atom node, including false. Atom leaves alias their semantic slot;
duplicate leaves do not allocate or initialize extra domains. Non-Atom outputs
occupy dense positions while child references retain original DAG indices.
Frozen truth still has one value per original node. This removes four requested
domain bytes per leaf per candidate (subject to the required empty-buffer
padding), without changing original satisfaction or the frozen query.

All 64 lanes scan distinct semantic-atom positions for the strict-subset
condition. Their counts and last eligible positions are merged before a zero
count refutes the query or a unique eligible atom is required to be false.
Auxiliary nodes never witness strict removal. This uses 24 bytes of workgroup
storage; a sweep reserves `9*nodes+atoms+65` policy work units, including 64
summary merges and one application. These units are not GPU instruction counts.
Original truth is evaluated before propagation. A cold graph groups original
node IDs by dependency level; nodes in a level run in parallel and a barrier
precedes dependent reads. Pure chains retain serial evaluation because they
contain no independent node work. Other narrow levels may still pay more in
synchronization than they gain in parallel work. Setup charges
`2*nodes+atoms+roots+levels` policy units, with zero level units on the serial path.

Preparation uses O(nodes + roots + levels) host work. Temporary depth words in
the node upload buffer become dense output positions before any upload; one
root buffer contains original roots, level offsets and ordered node IDs. Actual
retained capacities of both staging vectors are admitted together with device
copies, then released after upload. Minimum-budget and input refusals preserve
residency; later cold preparation failure leaves a healthy context requiring a
fresh upload. No incomplete graph, new epoch or submission is published. Driver
retirement, allocator bookkeeping and the existing requested seed/result host
allowance are separate from these measured staging capacities.

`FormulaVerdict::Residual` requires the exact host completion path. Candidate
ordering, original theory identity and the frozen candidate are preserved across
that boundary. Device results cannot silently change the subject being checked.

The static operation receives a complete ground program; `check_batch` performs
no lazy tuple discovery. Lazy execution instead admits bounded source instances
against immutable world snapshots. Source rounds and their coverage barriers
remain part of correctness, even when instances are shared across candidates.

Static readback contains one complete record per submitted seed, in input order.
Each record carries the checked nonzero submission epoch, world ordinal, verdict
and nonzero completion marker before its closure words. The host rejects missing,
stale, duplicate, reordered or malformed records and independently checks that
the gate-projection verdict agrees with the closure and original seed. Any error
refuses the entire batch. Framing identifies a completed record; rule closure and
constraint evaluation still rely on the shader and device execution contract.
An empty batch checks caller control, context health and graph admission, then returns without
transport allocation, dispatch limits or an epoch increment. Clearing residency
does not reset epochs; exhausting their 32-bit sequence requires a new oracle.

Lazy plans also receive a checked nonzero submission epoch when constructed.
Their 16-byte uniform contains only word count, rule count, world count and
epoch; the source catalog's address range is checked on the host. Every returned
world record must match the plan's epoch and ordinal. Retained buffers do not
retain an earlier chunk's output or reset the oracle's submission sequence.
The backend independently requires both snapshot and seed slices to contain
exactly `worlds * words` entries. Every lane finishes its rule partition before
a uniform storage barrier allows the world/epoch receipt to be written. The
receipt still requires successful queue completion and complete host validation;
it is not a certificate against an arbitrary faulty driver.

Every primitive requires a positive wait timeout before dispatch. A zero timeout
is a Capacity refusal before transport allocation or submission, preserving
healthy residency. Existing empty operations perform no wait and keep their
cancellation/health checks; relation preparation uploads columns without dispatch
and does not consume a wait allowance. Cancellation and device-health precedence are
unchanged. This policy does not turn a positive timeout into hard preemption.

Within one lazy batch, retained seed buffers need another upload only when their
layout or buffer changes. Snapshot buffers also need a write when the immutable
source round changes. The source coordinator provides a batch-local round index;
it is not a globally unique cache key. Every public batch owns a fresh transport,
and failed readback discards it. Offsets, records and the epoch uniform are still
written each dispatch; every active output is cleared. Statistics count actual
requested upload bytes, while admission retains the conservative allowance for
all active host inputs. This does not measure physical bus traffic or wall time.

Adapter, allocation, limit, cancellation, timeout, validation and device failures
remain failures or incomplete work. They are never converted into UNSAT results.
Resident plans and transport buffers retain explicit identity and capacity
contracts. Static and formula batch statistics describe the last successfully
decoded nonempty batch. They do not measure operations or rounds performed by an
interrupted submission whose result was not returned; those quantities are
unavailable. Payload accounting is not a measurement of process RSS or physical bus
traffic. Read each operation's rustdoc before reusing residency after a failure.
`GpuFormulaOracle::last_submission_candidates()` separately records the candidate
count only after an actual queue submission. It survives an interrupted read,
but each new call or explicit residency clear resets it. It is not completion
evidence. Formula propagation work and round limits are per candidate; ordinary
solving exposes both directly, independently of CPU quotas.

Static `check_batch_with_cancellation` / `check_batch_views_with_cancellation`
and formula `propagate_batch_with_cancellation` accept the same cooperative
`Cancellation` used by CPU execution. Existing calls delegate with
`Cancellation::default()`. A busy context takes precedence; otherwise cancellation
is checked before admission, including empty calls, after host packing and
between device waits of at most 50 milliseconds. Driver
calls and shader execution are not preempted: this is not a hard deadline.
`GpuErrorKind::Interrupted` and `GpuError::interruption()` expose the exact stop
without interpreting display text. Consumers with exhaustive error-kind matches
must handle this variant. Ordinary solving retains interrupted candidates as
uncommitted and does not retry a stopped operation on another backend.

Relation preparation releases mapped host access without submitting queue work.
Cancellation or deadline expiry during that copy returns no prepared view but
leaves the context reusable after successful scope and device-health settlement.
Scope/device failures retain priority and invalidate it. A submitted cancellation
or deadline also preserves context reuse when the runtime observed queue
completion and successful mapping, released every mapped view and unmapped the
buffer, then drained scopes and checked device health. A late stop still returns
its original interruption and discards output; it is never a successful result.
Unknown completion after an interrupted wait, failed mapping/decoding, device
faults and other errors retain conservative invalidation. The private completion
envelope carries these effects through every primitive's common readback path;
the error kind alone cannot justify reuse. Resident buffers may still be dropped
by their primitive after a failed call, independently of shared context health.

## Native numeric aggregates

Native aggregate admission requires already OR-coalesced complete tuple keys
and refuses duplicates. `AggregateGpuPlan` accepts an owned native `Group` or a
`GroupRef` bound to caller-owned canonical storage; both prepare the same numeric
wire representation without copying term payload. Original and frozen
eligibility remain separate and retain the exact aggregate occurrence identity. Empty extrema carry presence explicitly;
no ordinary integer stands for an empty minimum or maximum. The numeric GPU
profile requires representable measured values and guards. Unsupported numeric
plans return a capability failure rather than changing the source semantics.

One 64-lane workgroup reduces each occurrence with strided folds and a shared
addition tree. Signed sums require separately safe complete positive and negative
carriers; cancellation in the final sum cannot justify an overflowing
intermediate. Integer execution uses neither floating point nor optional subgroup
operations. Actual packing, synchronization and readback remain executable
refinement obligations; the Lean arithmetic laws alone do not verify WGSL.

## Immutable relation selection

`GpuRelationExecutor::prepare` uploads the equality-ID columns of a borrowed
`zetesis_core::relation::Relation`. Its prepared view exclusively borrows the
executor and retains the exact relation owner. A filter accepts queries from
that owner and produces one packed row mask per query occurrence. Empty
conjunctions retain every row; missing dictionary values retain none.

The kernel evaluates 64 consecutive row positions per workgroup. Unique writers
pack the flags into ordered mask words; reconstruction preserves local row
identity and the relation's original catalog mapping. Equality IDs provide no
numeric or ASP term ordering. The complete tuple matcher remains responsible for
patterns and binding. Preparation, filtering and reconstruction each have
explicit limits; the enclosing caller accounts for simultaneously retained views.
These operations do not replace ordinary source grounding.

The COL2 completion/identity protocol returns a receipt from every row tile.
For `T=ceil(rows/64)` and `W=ceil(rows/32)`, each query occupies `5*T + W` words:
one five-word marker/epoch/query/tile/work receipt per tile, followed by its mask.
All lanes, including padded final-tile lanes, reach a storage barrier after mask
writes and before lane zero writes that tile's receipt. The host validates every
tile and the mask tail before returning a complete batch. Missing, stale or
misplaced receipts refuse the batch; they never publish a decoded prefix.
The previous COL1 receipt identified tile zero's dispatch only.

The full result and readback payloads each contain `4*Q*(5*T+W)` bytes for `Q`
queries. Scheduled work is `Q*(64*T+32*W+T) + rows*E`, where `E` is the total
number of compiled query equalities. The last `T` charges one fixed receipt unit
per tile; these are policy units, not measured device instructions. Filtering
still uses exactly `[T,Q,1]` workgroups. Fresh transport is allocated per invocation;
the prepared column owner survives successful calls. An unwritten zeroed receipt
cannot match COL2 and the nonzero invocation epoch. These checks are a
completion/identity protocol, not a proof of semantic mask correctness or a
guarantee against arbitrary driver faults.

Filtering first checks the minimum payload, then fallibly reserves host query,
equality and complete output-mask vectors before any invocation device effects.
All retained element capacity, including allocator-provided spare slots, is
charged together with resident columns, device transport and host parameters.
The reported `accounted_bytes` is that admitted peak; readback fills the existing
output storage without growth. Allocation and byte-ceiling refusals remain
distinct and publish no partial masks. This payload bound excludes allocator
metadata and driver-private storage; it is not an RSS guarantee.

The core `Relation::select_mask` producer returns the same low-bit-first row
layout for CPU consumers, directly from the shared equality predicate. Device
qualification compares both producers and an independent typed-row reference.
Layout and owner checks alone do not establish correct device membership.

## Validate a physical backend

Portable checks exercise planning and host failure boundaries. Physical Metal
qualification is selected explicitly:

```sh
cargo test --locked -p zetesis-wgpu --all-features --test integration hardware_formula::metal -- --ignored --nocapture
cargo test --locked -p zetesis-wgpu --all-features --test integration hardware_aggregate::metal -- --ignored --nocapture
cargo test --locked -p zetesis-wgpu --all-features --lib metal_aggregate -- --ignored --nocapture
cargo test --locked -p zetesis-wgpu --all-features --test integration -- --ignored --nocapture --test-threads=1 --exact hardware_relation::metal_relation_masks_match_typed_rows hardware_relation::metal_relation_refusals_preserve_prepared_view
```

The [test sources](tests) contain the separate static, lazy, tight, formula,
aggregate and relation controls. Vulkan tests use their explicit Vulkan filters;
a Metal pass does not qualify Vulkan. The repository's `scripts/check.sh coverage --metal`
checks the selected physical groups with the matching instrumented binaries and
keeps CPU-only CLI coverage separate. See [Contributing](../../CONTRIBUTING.md)
for the complete gate discipline.
