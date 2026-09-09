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

`zetesis_wgpu::GpuLazyOracle` supplies a device-backed evaluator using the same
protocol. The required adapter and resource validation remain runtime concerns.
Its explicit source-selection wrapper does not change ordinary CLI defaults.
The [execution chapter](../architecture/execution.md) explains why per-world
isolation and whole-round commits are semantic requirements.
