# zetesis-experiments

Reproducible measurements of exact static reduct oracles. `zetesis-bench`
defaults to a physical Metal device and fails if one cannot be used. Select
`--backend cpu` explicitly for CPU measurements alone.

```sh
zetesis-bench --atoms 64,256 --batches 1,64,256 --repetitions 5 > metal.tsv
zetesis-bench --backend cpu --atoms 64,256 > cpu.tsv
```

Three deterministic rule families exercise forward chains, reversed chains,
and wide dependencies. Eight gate atoms provide 256 frozen interpretations;
larger batches repeat those patterns. Constraints mix accepted and rejected
candidates. Every measured result is compared with the exact dense CPU oracle,
including closure bits, constraint rejection, and seed mismatch. The dense CPU
oracle itself has independent exhaustive conformance tests against the lazy
oracle and subset-minimal semantics.

TSV output separates graph construction and static compilation, device and
worker-pool initialization, the first dispatch for each batch shape, and warm
dispatches. Warm GPU calls must reuse graph and transport buffers. A first call
for a new shape may retain graph storage from the preceding shape; the residency
record states which allocations occurred. The phase is named `initial-case`:
repeated dimensions may reuse both graph and transport. Timings include host packing,
transfers, execution and readback. Comparison runs outside timed regions.

These are microbenchmarks of the same explicitly compiled static program.
They exclude source parsing and complete candidate search. Scalar CPU, Rayon,
and Metal run in that order; thermal and ordering effects have not been
controlled. CPU allocations are included per call whereas Metal can reuse its
resident buffers. These limitations must accompany any reported measurements.
Use release builds. No speedup, energy benefit, or full-domain result follows
from portable tests or the CPU-only mode.

## General Ferraris formulas

```sh
zetesis-bench formula --backend cpu --cpu-workers 4 --atoms 64,256 --batches 1,64,256
zetesis-bench formula --backend metal --cpu-workers 4 --atoms 64,256 --batches 1,64,256
```

The separate `formula` profile measures original finite DAGs with choices,
cycles, conjunctions, disjunctions and masked implications. Resident GPU propagation returns
original-model rejection, proper-subset refutation or an explicit residual.
Every residual receives exact CPU membership checking inside the hybrid timing.
Native CPU results provide the reference for all candidates. Warm samples must
reuse both the immutable graph and the batch transport.

`--gpu-max-rounds` and `--gpu-max-work` bound propagation; a sweep limit leaves
an unresolved query. `--max-work` bounds exact CPU completion; exhaustion fails
the experiment. `dispatch_host_ns` includes packing, submission and readback,
not just shader execution. Each case runs the scalar native reference, an ordered
Rayon membership batch, then the hybrid GPU batch. `--cpu-workers` defaults to 4
and accepts 1–64; one explicitly owned pool is reused throughout the run. Pool
initialization is reported separately, with requested and actual workers.
The Rayon interval includes scheduling, native checking and result collection;
all workers join before an error is returned. Each query keeps its native limits
and shared cancellation control. GPU residual queries remain serial inside the
hybrid interval, so this is not a Rayon-completed hybrid measurement.
Candidate patterns repeat above 256. Source grounding, outer search and
objectives are excluded. The [design and qualification contract](../../docs/design/gpu-formula-propagation.md)
records remaining full-GPU work. Ordinary CLI formula solving can explicitly
select the same GPU primitive; this experiment alone does not qualify that
end-to-end integration. The dated M4 Pro record predates the Rayon baseline
and retains its original two-backend measurements.
