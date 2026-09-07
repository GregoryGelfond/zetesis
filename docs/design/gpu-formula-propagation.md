# Resident Ferraris propagation

The first general GPU primitive consumes an immutable, original finite Ferraris
theory and a batch of frozen candidates. It returns original-model rejection,
proof that no proper reduct subset exists, or an explicit residual query.
It supports all five admitted DAG operators: atom, false, conjunction,
disjunction and implication. It neither enumerates answer sets nor replaces the
source adapter or exact residual search.

This extends the separate normal-rule least-closure GPU specialization. Ordinary
CLI formula solving now uses this primitive for explicit GPU selection, including
`zetesis --oracle countermodel --backend metal`. Candidate generation, unresolved
reduct search, objective scoring and output remain on the host. Automatic formula
selection retains CPU execution pending a measured crossover.

## Semantics and execution

For candidate M, original truth is computed once in topological order and frozen
for the entire dispatch. An original-false node becomes false in the reduct:
its original connective is disabled, so it imposes no relation on its children.
An original-true node retains its connective. Rewriting classical formulas before
freezing this mask could change the reduct and is outside this contract.

A Boolean domain stores the allowed false/true values of each semantic atom and
formula output. Roots are asserted true; atoms outside M are false; a separate
strictness relation requires at least one true atom of M to be absent from J.
Auxiliary formula outputs do not count toward strict subset membership. Shared
atom occurrences refer to the same semantic atom domain.

Each candidate has a workgroup. Cooperative sweeps project the finite Boolean
truth-table relation of each enabled gate onto its current domains and intersect
the supports with those domains. Narrowing preserves every satisfying completion.
Stale observations may delay deductions but cannot remove a valid completion.
An empty domain refutes the proper-subset query. A no-change sweep is only a
residual: local consistency need not imply global satisfiability or minimality.

The graph remains resident while the same immutable `Theory` instance is used.
Transport is reused only for a compatible exact batch shape; every candidate
mask and work domain is initialized for its new epoch. Public batch statistics
report graph upload, transport allocation and authored resident bytes. This API
does not promise a driver-level RSS bound or zero hidden allocations.

## Bounds and failures

`FormulaLimits` bounds candidate count, accounted authored memory, charged work,
complete sweeps and host wait time. Checked packing rejects integer/buffer limits
before submission. Mandatory setup must fit the work budget; a later work or
round ceiling produces a residual. A device failure, timeout, malformed result or
transport error is an error and never a logical verdict. A host wait timeout is
not GPU preemption. A caller must preserve the unresolved work for exact checking.

The two positive outcomes have separate meanings:

| Result | Established fact |
| --- | --- |
| `NotModel` | M violates an original theory root. |
| `NoProperSubset` | M models the original theory and propagation refutes all strict reduct subsets; M is stable. |
| `Residual(reason)` | M models the original theory; the proper-subset query still needs exact search. |

Objective incumbent bounds belong to candidate selection. They never restrict
the inner J query: a reduct countermodel can reject M regardless of its objective
cost. This primitive does not yet evaluate or reduce objective values on GPU.

## Qualification and measurements

After installation, the independent experiment is runnable directly:

```sh
zetesis-bench formula --backend cpu --atoms 64,256 --batches 1,64,256
zetesis-bench formula --backend metal --atoms 64,256 --batches 1,64,256
./scripts/qualify-metal-formula.sh /absolute/path/to/new-results
```

The five synthetic families cover choices, positive cycles, conjunction,
disjunction and masked implication, exercising all five DAG operators.
Complete native membership is the reference. GPU residuals
run exact native checking inside the measured hybrid interval; final vector
parity comparison happens outside it. A residual/native-nonmodel consistency
guard runs inside, and residual time includes completion-vector allocation and
classification bookkeeping. The current fixed order is scalar CPU, an explicitly
owned Rayon pool, then hybrid. `--cpu-workers` defaults to 4 (range 1–64), and pool
initialization is separately timed. The scalar reference and hybrid residual
checks remain serial. The dated M4 Pro campaign predates this Rayon addition and
retains its original scalar/hybrid measurements. Candidate patterns repeat above 256 and shift each iteration; these
controlled probes are not a representative ASP workload distribution.

TSV rows distinguish the initial case and warm repetitions. `dispatch_host_ns`
includes packing, submission, wait and readback; it is not shader timestamp
measurement. Residual CPU time and count are reported separately. Every warm
sample requires graph and transport reuse. Device initialization and fixture
construction have separate fields. A failed run retains diagnostics and lacks a
success marker. Separately recorded source and binary hashes identify both
artifacts; they alone do not attest that the binary was built from that source.
Explicit Metal requires a physical adapter and has no automatic
fallback. Portable tests and WGSL validation do not qualify physical execution.

Source parsing, grounding, outer candidate search, displayed model enumeration,
optimization and full solve time are excluded from this experiment. Its results
cannot establish end-to-end corpus parity, full GPU acceleration or speedups for
explosive grounding. The older static Metal qualification remains a separate
dated result for a different kernel and binary.

## Completion path

| Step | Required completion evidence |
| --- | --- |
| Resident general propagation | Independent completion-preservation tests, strict host gates, physical kernel parity and measured residual rate. |
| Exact GPU residual search | Disjoint region partitioning, bounded work queues, a complete accounting of pending regions, checked witnesses and CPU differential tests. Empty queues alone do not prove exhaustion. |
| GPU outer candidate generation | Complete candidate-region accounting, exact objective bounds and tie retention; batch cancellation or capacity refusal leaves work pending. |
| Source-to-oracle integration | Original admitted corpus model/count/cost parity, including paths with disjunction and aggregates, with explicit per-phase backend diagnostics. |
| GPU relational construction | Persistent indexes and semi-naive deltas, bounded count/scan/materialization, exact tuple deduplication and sound completion detection before accepting a candidate. |
| Performance qualification | Matched CPU/Metal/NVIDIA builds; phase and end-to-end time, memory, traffic, residuals, limits and all failures retained. |

The [GPU Datalog review](gpu-datalog-20260906.md) develops the relational step;
positive closure specialization does not discharge general reduct minimality.
The [neuromorphic assessment](neuromorphic-feasibility-20260906.md) uses the same
frozen candidate, exact narrowing and completion contracts with a different
execution protocol. Parsing, source ownership and I/O remain on the Rust host,
with the pinned, unmodified themelios front end.

## Formal boundary

[`Propagation.lean`](../../proofs/Zetesis/Propagation.lean) formalizes completion
preservation for relation projection, sequential/parallel/delayed narrowing,
false-mask gates, strict semantic subsets and the conflict-to-stability bridge.
The bridge explicitly requires every real reduct countermodel to extend to an
encoded completion. These semantic laws do not prove the Rust packer, WGSL kernel,
atomics, barriers, buffer layouts, compiler or device implementation correct.
Those remain refinement and qualification obligations.

## Runtime integration contract

Device selection, queue creation and pipeline initialization belong to the normal
solver invocation, either at startup for an explicitly selected GPU or on first
eligible use in automatic mode. Installation may prepare artifacts but cannot
attest to every later process's device access. No external setup command,
qualification run or persisted pass marker is a prerequisite for solving.
Help/version and CPU-only use do not need to initialize a GPU.

Both the static CLI route and explicit GPU formula route follow this lifecycle. The
[physical M4 Pro campaign](../verification/metal-formula/20260906T140835Z/README.md)
qualifies 180 propagation batches and exact native residual completion. It does
not qualify the later general-formula CLI integration or Rayon baseline.

The ordinary formula route uses bounded proposal/check/commit batches. Exact
candidate blocks cannot erase unresolved membership work: pending proposals stay
retained after callback failures, and verified stable models remain queued until
the driver processes them. Objective bounds affect only future proposals; the
original theory used by every reduct check stays immutable. CPU residuals use
the same exact native search as scalar solving. The subsequent ordinary Metal
campaign passes all 94 original cases, with 105 batches and 2,358 candidates;
four focused CLI/formula hardware tests also pass. The matched three-backend
membership benchmark is complete, while full-solve phase measurements and
deployment stress/fault qualification remain separate obligations.
