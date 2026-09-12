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

Calling `Check::into_stable_interpretation` on an accepted native check transfers
its closure into an instance-bound receipt without copying atoms. Rejection
returns the original check intact. The receipt's `into_interpretation` explicitly discards its program
association. Ordinary CPU session execution uses these consuming operations to
deliver the owned closure after acceptance.

Ordinary formula sessions have a separate bounded exact-completion executor.
Its worker setting is distinct from the relational closure pool. Scalar outer
candidate search does not become parallel simply because residual membership
queries use Rayon.

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
world evaluator without materializing input trees. Demanded catalog atoms and
completed closures still own their payloads under the existing host budget.

The catalog grows on demand. Its atom IDs, frozen seeds, snapshots and pending
deltas must survive changes in packed-word width. The protocol charges catalog,
round, chunk, result and optional membership storage against its logical host
budget. Caller inputs, allocator overhead and backend-private transport are
outside that budget; the backend must bound its own transport.

Failure returns `Failure<E>` with its source, protocol or injected execution
cause and accumulated progress. No completed checks are published from a batch
whose required scan or evaluation is incomplete. `Progress::mask_words` is part
of source work, not extra work to add again. Optional masks may reduce offered
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
