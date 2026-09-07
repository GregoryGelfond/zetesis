# zetesis-wgpu

Exact GPU reduct primitives for zetesis. The original static profile compiles a
`GroundProgram` under separate grounding limits before checking candidates.
`check_batch` performs no source grounding or lazy tuple discovery.

One 64-invocation workgroup owns one frozen candidate and its 4096-atom maximum
closure. Worlds share immutable rules, antecedent lists, and gate-carrier bits.
Each world starts from the empty positive interpretation. Integer seed gates
enable rules; positive antecedents consult atom latches. Every pass which
continues adds at least one previously absent atom. A full pass with no additions
establishes closure. The kernel then checks all enabled constraints and compares
the closure's gate projection with the seed. Duplicate derivations are idempotent.

Workgroup barriers separate reset, derivation, completion observation, and final
constraint checking. `workgroupUniformLoad` provides uniform loop exit. There is
no cross-workgroup communication, floating-point truth representation, or
fixed-depth approximation. These are reviewed implementation invariants; the
Lean abstract schedule proofs do not mechanically verify Rust packing or WGSL.

The public API is `GpuOracle::new(GpuOptions)`,
`GpuOracle::new_metal(GpuOptions)`, or
`GpuOracle::new_selected(GpuOptions, GpuSelection)`, followed by
`check_batch(&GroundProgram, &[Seed], GpuLimits)`. Results preserve candidate
order and expose exact rejection reasons and validated dense closure words.
`GroundProgram::model_from_words` reconstructs symbolic models when needed.
The Metal constructor enables only Metal, checks the returned backend identity,
and refuses targets without compiled Metal support. It does not fall back to
another graphics API or apply environment overrides to its backend selection.

`GpuSelection` combines `GpuBackendPreference::{Auto, Metal, Vulkan, Dx12, Gl}`
with an optional exact `vendor_id`. Explicit APIs and vendor IDs are hard filters;
unavailable requests produce typed errors. `NVIDIA_VENDOR_ID` is `0x10de`, matching
[NVIDIA's published driver source](https://raw.githubusercontent.com/NVIDIA/open-gpu-kernel-modules/main/kernel-open/nvidia/nv-pci-table.c).
NVIDIA devices execute this WGSL profile through an available wgpu API, such as
Vulkan or DX12. This crate supplies no CUDA backend and never matches vendors
using potentially misleading device names.

`compiled_backends()` lists native APIs built for the current target without
touching hardware. `discover_adapters()` enumerates those APIs and returns
reported name, API, category, vendor/device IDs, PCI bus ID when available,
driver details, and static-oracle capability diagnostics. Software and virtual
adapters remain visible and are marked nonphysical. An empty inventory means no
adapter was exposed to this process. A device can appear separately through
multiple APIs. Advertised compute support and limits are a preflight: device
creation, shader compilation, execution, and workload-specific admission remain
separate checks. Browser WebGPU and the nonexecuting Noop API are excluded.

Auto first filters capabilities and the caller's hardware/vendor policy. Among
eligible adapters it ranks physical devices ahead of nonphysical ones, then
prefers Metal on Apple, DX12 on Windows, and Vulkan elsewhere; GL follows the
primary native APIs. Within an API it prefers discrete over integrated GPUs,
then orders reported vendor/device IDs, PCI bus, name, and driver metadata.
This is a reproducible selection policy, not a performance prediction. Exact
metadata ties retain driver enumeration order; it does not promise a persistent
identity for otherwise indistinguishable physical GPUs. A selected device's
initialization failure is returned rather than silently selecting another API.

No compiled API or no exposed adapter yields `AdapterUnavailable`; an exposed
inventory rejected by backend/vendor/hardware filters yields `AdapterRefused`;
matching adapters lacking required advertised capabilities yield `Capacity`.
`discover_adapters()` itself returns an empty vector for no exposed adapters.
CPU reference fallback remains a caller policy, with its reason available from
the returned error. Setting `require_gpu: false` only admits nonphysical wgpu
adapters; it does not invoke the CPU reference solver.

The first nonempty batch packs and uploads immutable rules, antecedents, and
gate-carrier bits. Those three buffers and their validated dimensions remain
resident for the same immutable `Program` instance. Cloning or recompiling that
Program preserves residency; admitting an independently created Program replaces
it, even when its source is identical. The cache retains a shared Program handle
for identity, not a cloned `GroundProgram` or retained host graph packing.

Subsequent batches of the same size reuse the parameter, seed, result, and
readback buffers and their bind group. Each call packs and writes fresh seed
bits, dispatches fresh candidate latches, and fully reads/unmaps its result before
reuse. A changed batch size replaces only transport, with exactly sized buffers;
there is no unbounded high-water capacity cache. Changing Program drops graph
and transport handles before new packing/allocation. One oracle retains at most
one Program and one transport shape. Empty checks preserve residency and clear
last-batch statistics; they submit no work. `clear_residency()` explicitly drops
the retained handles without repairing an invalidated device or cancelling work.

`last_batch_stats()` reports whether the successful nonempty call uploaded a
graph or allocated transport, plus resident and accounted bytes. A repeated
equal-sized batch should report both flags false. These diagnostics distinguish
setup from steady-state benchmarks; they are not GPU timestamps or speedup claims.

The default adapter policy accepts only devices wgpu identifies as integrated
or discrete GPUs. Adapter name, backend, and category remain observable.
There is no automatic CPU fallback. Capacity, allocation, invalid seed,
validation, timeout, device, and readback errors are separate from logical
candidate rejection. An execution failure invalidates the instance and returns
no partial batch. The host waits at most the configured GPU polling interval;
this does not provide preemption of running GPU commands or a wall-time bound
on driver allocation/pipeline compilation. Native execution is the implemented
host profile; browser event-loop integration is not supplied.

Before allocation, packing checks dense IDs, u32 addresses, the adapter's granted
buffer/dispatch limits, the 4096-atom shader capacity, and the batch budget.
Every nonempty batch's budget counts its resident graph and transport even when
reused, host seed/parameter packing, temporary per-seed words, returned closure
arrays, and result metadata. A new graph additionally counts its host packing;
resident batches avoid that cost. Old transport handles are released before a
changed-size allocation, so a prior large batch cannot silently remain in the
authored working set under a smaller budget. The sum conservatively includes
some allocations with nonoverlapping lifetimes.

The budget excludes caller-owned source/seed objects and earlier results,
symbolic models reconstructed by a caller, allocator rounding, wgpu/driver upload
staging, internal objects, and deferred retirement of dropped handles. It is not
a process-RSS or physical-device-memory cap. Counts are checked arithmetic, and
host transport vectors reserve fallibly. The public atom/workgroup constants
are checked against the parsed shader in a portable test.

Portable tests cover packing of gates and constraints, resident Program identity
including clones and recompilation, cold/hot accounting and lowered budgets,
transport/dispatch/binding limits, malformed readback, zero-atom result records,
hard API/vendor filters, software refusal, platform ranking, advertised compute
capabilities, deterministic reported-identity ordering, and full Naga validation
of the WGSL without optional shader capabilities. Selection tests use synthetic
adapter reports and do not imply hardware execution on those platforms.
The explicit hardware qualification is:

```sh
cargo test -p zetesis-wgpu --test hardware -- --ignored --nocapture
```

It compares actual device output with independent ordered-set CPU closures,
including zero-atom programs and constraints, default-negation cycles, choices,
self-support, multiword closure, reversed candidate epochs, and the 4096-atom
boundary. It additionally checks resident reuse, smaller transport replacement,
explicit cache release, and a Metal-only constructor. This separately ignored
hardware-test binary has not yet run on the physical device.

A user-run [Metal qualification](../../docs/verification/metal/20260905T214253Z/README.md)
on the Apple M4 Pro did execute the example and 108 static GPU batches, covering
11,556 candidate checks with exact CPU parity and warm residency checks. Raw
binary hashes and timings identify that measured run. The best CPU median was
faster in all 18 measured cases; there is no established advantage over that
baseline, full-domain result, or energy measurement. The sandboxed execution
environment still reports no adapters. Explicit hardware requests there fail
rather than substitute CPU computation.

## General finite formulas

`GpuFormulaOracle::new_selected` and `new_metal` create a separate resident
Ferraris propagation profile. `propagate_batch(&Theory, &[Interpretation],
FormulaLimits)` evaluates the original DAG, freezes each candidate's truth mask
and narrows its strict proper-subset query. Atom, false, conjunction, disjunction
and implication retain their original structure. A false-masked connective
constrains only its output to false, leaving its children unconstrained by it.

`FormulaVerdict` distinguishes original nonmodels, refuted proper-subset queries
and explicit residuals. Only a refuted query establishes stability. Quiescence,
round limits and work limits require exact residual search. This API performs
no automatic CPU completion and supplies no full candidate search or source
grounding. The CLI's explicit GPU formula route composes it with bounded native
candidate generation and exact native completion of residual queries.

The graph is resident by immutable Theory instance; transport is reused for the
same exact candidate count. Checked storage limits replace the static profile's
4,096-atom ceiling. Candidate epochs, readback identity, charged setup/sweeps and
all authored allocations are checked. `FormulaBatchStats` makes residency and
accounting observable. Device errors invalidate the instance and return no
partial result. The same explicit hardware and failure policies apply.

Portable tests validate WGSL and packing/decoder boundaries and compare a
separate propagation model with independent exhaustive reduct completions.
They do not execute the shader. The physical tests are separate:

```sh
cargo test -p zetesis-wgpu --test hardware_formula -- --ignored --nocapture
zetesis-bench formula --backend metal --atoms 64,256 --batches 1,64,256
```

The benchmark includes exact CPU completion of residuals and reports it
separately. General formula synthetic hybrid membership is now qualified by the
[M4 Pro run](../../docs/verification/metal-formula/20260906T140835Z/README.md).
The subsequent [ordinary-solver campaign](../../docs/verification/metal-batched-formula/20260906-corpus/README.md)
passes all 94 original cases, and four focused CLI/formula device tests pass.
The matched scalar/Rayon/Metal benchmark finds no shape where hybrid Metal has
the lowest warm median; automatic formula selection therefore retains CPU.
The older static Metal record does not qualify this kernel. See the
[design and completion obligations](../../docs/design/gpu-formula-propagation.md)
for semantic premises, resource boundaries and the path to full acceleration.
