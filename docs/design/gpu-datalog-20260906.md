# GPU Datalog techniques for zetesis

Research and design note, sources checked 2026-09-06. The proposals below are
not implemented capabilities or measured zetesis speedups. They extend the
[performance evaluation contract](zetesis.md#required-cpugpu-evaluation-contract)
and preserve answer-set acceptance through the reduct.

The most promising transfer is a shared, exact relational execution layer for
positive closure and possible-atom construction. GPU Datalog research supplies
useful alternatives to repeated tuple scans. It does not replace the Ferraris
minimality oracle for choice, disjunction, or general aggregate formulas.

## Primary evidence

| Work | Verified mechanism and relevance | Evidence boundary |
| --- | --- | --- |
| GPUlog, ASPLOS 2025, *Optimizing Datalog for the GPU* | Hash-indexed sorted arrays combine dense tuple traversal with join-key ranges and full-row deduplication. Semi-naïve relations distinguish new, delta, and accumulated tuples; temporary multiway materialization trades memory for repeated work. | CUDA implementation using Thrust. Its data-center experiments do not establish Metal performance. [Full paper, §§4–6](https://arshovon.com/publications/ASPLOS_2025.pdf). |
| VFLog, AAAI 2025, *Column-Oriented Datalog on the GPU* | Columns retain tuple IDs, sorted indexes, and value-to-range maps. Joins first determine output sizes, then place results using prefix sums; column projection can be delayed. This closely matches the need to separate joins from tuple construction. | The published PDF uses the spelling FVLOG. Its runtime is CUDA/C++; the [authors' repository](https://github.com/harp-lab/vflog) provides relational operators and query examples, not a Rust/WGSL backend. [Full paper, pp.15179–15182](https://ojs.aaai.org/index.php/AAAI/article/view/33665/35820), [publication record](https://ojs.aaai.org/index.php/AAAI/article/view/33665). |
| MNMGDatalog, ICS 2025, *Multi-Node Multi-GPU Datalog* | Partitions relations, prepares exchange buffers, and performs all-to-all redistribution. It compares sorting versus two-pass buffer preparation and CUDA-aware versus host-buffer MPI. Buffer preparation and communication become substantial costs. | This is evidence for a later distributed plan, not automatic DGX scaling through wgpu. The paper distinguishes communication experiments from complete-query comparisons. [Full paper, §§4–5](https://hpcrl.github.io/ICS2025-webpage/program/Proceedings_ICS25/ics25-71.pdf), [authors' CUDA/MPI repository](https://github.com/harp-lab/MNMGDatalog). |
| SRDatalog, *Scaling Worst-Case Optimal Datalog to GPUs*, arXiv v2, 23 April 2026 | Uses multiway intersections over sorted columns, count/materialize passes, incremental histograms for skew, and a small head buffer before larger merges. Independent rules use CUDA streams. | This is a preprint; its HTML retains placeholder proceedings metadata. Treat architecture and performance results as the authors' reported experiments, not established zetesis results. [Versioned abstract](https://arxiv.org/abs/2604.20073v2), [full text, §§3–7](https://arxiv.org/html/2604.20073v2). |
| Lu and Kumar, GCASR 2025, *Accelerating Iterative Relational Algebra Operations with WebGPU* | Direct WGSL precedent: hash join, counting, sorting, deduplication, difference, and merge inside a delta fixed-point loop. | A workshop poster, not a complete solver qualification. Its reported WebGPU timings are slower than its Soufflé and CUDA GPUJoin baselines; adding radix sort makes the reported results worse. This supports feasibility while showing that a GPU implementation can lose badly. [Institutional record](https://www-new.evl.uic.edu/news/2025/2025-05-08-2927/), [poster PDF](https://www-new.evl.uic.edu/documents/jiaxin.pdf). |

No external implementation was built or run for this note. Repository links
identify inspectable implementations; they are not dependency selections or
reproduced artifacts. No paper's headline speedup is transferred to zetesis.

## Where these techniques fit

The current [CPU lazy oracle](../../crates/zetesis-cpu/src/oracle.rs) rebuilds
predicate relations from the accumulated closure, then visits every template
against those relations each round. It collects a delta but does not restrict
each rule's input to newly derived rows. Its final empty-delta round checks all
templates and constraints before comparing the closure with the frozen gate
seed. The [owned Rayon pool](../../crates/zetesis-cpu/src/batch.rs) parallelizes
independent candidates.

The formula frontend's
[possible-positive support](../../crates/zetesis-themelios/src/formula_support.rs)
already retains predicate relations and per-column row indexes. It chooses a
small matching row range when columns are bound, suppresses some already-known
heads, and revisits rules until no possible atom is added. This is an enclosure
used to compile formulas, not a stable model. Disjunctive alternatives enter this
enclosure independently and must retain their original support formulas.

| Execution role | Proposed acceleration | Semantic obligation |
| --- | --- | --- |
| Exact closure for one frozen gate seed | Delta joins, indexed range probes, projected tuple deduplication, persistent relations | Same least positive closure; unchanged constraint checks and gate-seed agreement |
| Possible-atom construction before formula compilation | Apply the same primitives to the already admitted positive transfer fragment | Complete covering support before absence-based pruning; uncertain or unsupported transfers keep their existing conservative treatment |
| Complete bindings for formula roots or objective/observation preparation | Filter and enumerate bindings, with bounded projection | Preserve tuple identity, all eligibility conditions, origins, arithmetic/refusal behavior, and source grouping |
| General Ferraris membership | These joins may reduce preparation work; candidate and frozen-reduct truth can use separate formula kernels | A completed no-proper-subset result is still required; a Datalog fixed point alone is insufficient |

The typed themelios program and existing signed signatures remain the planning
boundary. [Domain analysis](domain-analysis.md) can eventually supply sound
enclosures and [program analysis](program-analysis.md) can identify dependencies;
neither currently licenses unproved source pruning. Keep positive closure,
possible support, and reduct truth distinct even if they share physical kernels.

## Proposed primitives and invariants

**Persistent relations and deltas.** For one immutable program and frozen seed
`G`, compute `R(next) = R ∪ T_G(R)`. Keep `Delta = R \ R(previous)` separate from
`R`. Bootstrap rules without positive bodies, then schedule every applicable
delta variant of a recursive rule. A body with several recursive occurrences
needs coverage of each possible changed occurrence, including self-joins; using
only its first relation's delta is unsound. Begin with an independent final
closure/constraint pass and remove it only after a complete schedule-to-operator
argument. This is a zetesis design proposal motivated by GPUlog's semi-naïve
pipeline, rather than a direct adoption of its evaluator.

**Exact column and range operations.** Intern admitted scalar values and signed
predicate identities on the host, retaining reversible typed mappings. A tuple
is `(signed signature, complete ordered arguments)`; a hash collision or matching
one column never establishes equality. Keep one immutable row identity while
column orderings vary. Evaluate candidate layouts: sorted full rows with
key-range indexes; columns with row-ID permutations; and small sorted deltas
merged into larger relations. Do not reconstruct tuples by combining unrelated
column memberships. This last condition matters for aggregates and objectives,
whose contribution keys have their own complete-tuple coalescing rules.

**Bounded count, scan, and materialize.** Count eligible outputs against immutable
input snapshots, compute checked offsets, admit the entire destination footprint,
then write disjoint ranges. Count and materialize must see the same program,
bindings, epoch, and predicates. Count raw witnesses as well as distinct tuples:
deduplication does not excuse unbounded temporary output. Split oversized work
into explicit chunks or return a typed incomplete result; never truncate a
relation. A workgroup-local counter wrap, exhausted hash table, or incomplete
scan cannot mean an empty delta.

**Selective multiway intersection.** Add a variable-oriented intersection plan
for cyclic and wider positive joins; retain indexed binary joins where they
perform well. Worst-case optimal joins address unnecessary intermediate work
for full conjunctive queries; they are not instance-optimal and cannot remove
the cost of an actually huge required result. The foundational bound and
algorithm are described by
[Veldhuizen, ICDT 2014](https://openproceedings.org/2014/conf/icdt/Veldhuizen14.pdf).
SRDatalog supplies a recent GPU design to investigate, not a proof that every
ASP body, projection, arithmetic filter, or aggregate satisfies that algorithm's
assumptions. zetesis's existing lazy cursor already avoids retaining a complete
binary intermediate; measure avoided scans as well as allocations.

**Candidate epochs and completion.** Index scratch state by program identity,
candidate identity, and generation. Read-only source relations can be shared;
derived facts, frozen guards, deltas, and constraints belong to their own epoch.
A completed reduction over all scheduled work, with no overflow, failed job,
pending transfer, or omitted recursive variant, is required before declaring
the delta empty. Similar candidates do not justify carrying derived facts into
a new epoch: their enabled reduct rules can differ. Reuse needs a separately
proved incremental contract. Strong-negation coherence stays an ordinary
acceptance constraint; it never supplies support or merges predicate signs.

These contracts are suitable targets for the existing
[Lean transformer and coverage framework](../../proofs/README.md): prove join
enumeration sound and complete for its binding relation, exact full-tuple set
operations, complete delta scheduling, and isolation between epochs. Published
Datalog algorithms do not discharge those Rust/WGSL refinement obligations.

## Rust, Metal, and later NVIDIA

Implement portable operations in Rust plus WGSL, with scalar/Rayon versions of
the same relation contract. Use explicit integer tuple IDs, storage buffers,
bounded dispatches, and staged scans. GPUlog/VFLog depend on CUDA facilities;
SRDatalog's CUDA streams and MNMGDatalog's MPI/device exchange are separate
engineering work. Retain a backend boundary for later native NVIDIA operations
without making CUDA assumptions part of the semantic interface.

WGSL synchronization is scoped; a workgroup barrier is not a global fixed-point
barrier. Do not assume NVIDIA's warp width, implicit global progress, or native
64-bit atomic counters. Validate storage-buffer, dispatch, workgroup-memory,
and optional-feature limits for the actual adapter; use checked host accounting
and bounded portable counters. Timestamp and subgroup paths need capability
checks and an explicit fallback. These constraints follow the
[WGSL specification](https://www.w3.org/TR/WGSL/) and
[wgpu limit interface](https://docs.rs/wgpu/30.0.1/wgpu/struct.Limits.html).

Start with one device and a resident relation store. A future DGX campaign must
measure whether partitioning relation rows or distributing independent candidate
epochs is better for each workload. Shared relations, replicated indexes,
cross-device deduplication, exchange completion, and transfer volume must be
counted. The distributed paper's results do not establish that wgpu alone
provides its CUDA-aware communication path.

## Bounded profiling and qualification plan

Use the existing evaluation contract rather than a second benchmark policy.
First instrument the CPU closure and support builders: time relation/index
construction, range probes, binding visits, guard evaluation, raw output,
deduplication, delta merge, and final validation separately. Record relation
sizes, distinct keys, maximum range length, recursion rounds, repeated scans,
and charged work. Instrumentation results diagnose work; publish uninstrumented
end-to-end measurements separately.

Begin with deterministic generators, fixed seeds, complete source manifests,
and powers-of-two sizes `16..4096`; predeclare per-case wall and byte ceilings
before running. Stop a case on its ceiling and retain that record. Scale further
only in a separately specified campaign. Cross all applicable cases with scalar
CPU, specified Rayon worker counts, and physical Metal; qualify NVIDIA later.

| Family | Variable dimensions and required evidence |
| --- | --- |
| Chain, star, and bounded layered reachability | Depth, fan-out, delta size, and closure size; exposes tiny-frontier overhead and skew |
| Three-relation cyclic join | For example `r(X,0), s(0,Y), t(X,Y)` with diagonal `t`: the `r`–`s` intermediate has `n²` witnesses and the final result has `n` rows. Compare chosen binary orders and intersection; do not claim every binary order loses on this instance |
| Repeated relation and wider body | Self-joins, repeated variables, empty/nullary relations, and two-to-six body occurrences; exercise all delta positions and tuple correlation |
| Duplicate-heavy projection | Many distinct witnesses derive the same complete tuple; measure raw output, unique delta, peak scratch bytes, and exact deduplication |
| Sparse chosen-edge closure | Use the already specified [candidate-directed family](zetesis.md#151-workload-families), vary candidate density independently of source-domain size, and retain nonempty constraints |
| Genuine output growth | Dense closure whose actual result is large, alongside cases with only large avoidable intermediates; distinguish unavoidable output cost from grounding strategy |
| Supported ASP integration | Unchanged kr-domains sources plus signed/choice/aggregate regressions; report full models, hidden multiplicity, objective vectors and all optimal ties, not just relation counts |

For small instances, compare each complete relation against an independently
enumerated binding/set reference, and each ASP result against the existing
reduct oracle and external clingo. Check count/write agreement, collisions,
skew, duplicate elimination, candidate resets, cancellation, and exact
inclusive limits. A device or allocation failure must remain incomplete.

Then ablate one mechanism at a time: persistent indexes, semi-naïve input
selection, layout, count/scan/write, deduplication, multiway intersection, and
candidate batching. Measure cold device/setup cost and declared resident reuse,
kernel time and host wall time, transfers/readback, host RSS, authored device
allocations, and unavailable telemetry explicitly. The WebGPU poster makes
sort and dispatch overhead especially important measurements. Compare the
unchanged sources against one-thread and specified multithread clingo, and
retain every unsupported, slower, timed-out, or memory-limited result.

The first acceptance target is exact parity with fewer repeated binding/index
operations and bounded memory on an identified family. A GPU speedup is a
subsequent measured result. Reduced grounding storage is a separate claim from
reduced scanned work, and neither alone establishes complete ASP speedup.
