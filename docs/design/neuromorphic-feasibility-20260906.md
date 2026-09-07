# Tentative neuromorphic feasibility: SpiNNaker2 and Loihi 2

Evidence checked **6 September 2026**. This is an engineering proposal for the
separate zetesis experiment. No neuromorphic hardware was accessed, firmware
compiled, simulator installed, or performance measured for this report.

**SpiNNaker2 is the stronger first target for a Rust implementation of the exact
event machine. Loihi 2 is a plausible second target for specialized integer
circuits.** This ranking is an inference from their programming models and
current tooling, not a performance result. Both should share zetesis's candidate
generator, frozen-reduct semantics, and explicit coverage contract. There is no
reason to rebuild clingo or make CDNL the organizing architecture of this work.

The first useful experiment should measure an exact, resident reduct transform
and the cost of completing its search. A neural system that proposes promising
answers can contribute later, but proposing an answer, verifying one, and
proving that no other answer exists are different operations.

## What the two platforms actually provide

| Question | SpiNNaker2 | Loihi 2 |
| --- | --- | --- |
| Execution substrate | General-purpose Arm M4F processing elements connected by an event-routing fabric; the chip paper describes 152 PEs and 128 KB of code/data SRAM per PE. | Programmable digital neuron microcode, with separate embedded microprocessors; Intel's brief describes up to 128 neuron cores and six C-programmable management cores. |
| Natural exact mapping | Rust event handlers, packed Boolean evaluation, bounded queues, counters, local joins, and search-region control. | Compiled gate/latch/counter circuits and frozen-mask evaluation; irregular allocation and general search control need separately qualified embedded or host placement. |
| Main qualification risk | Board runtime, native-code access, memory layout, interrupt/DMA/router contracts, and reliable completion. | Available SDK and deployment permission, exact microcode/state/channel behavior, synchronization, and capacity. |

The hardware descriptions come from the
[SpiNNaker2 chip authors' July 2026 paper](https://arxiv.org/html/2607.24396v1)
and [Intel's Loihi 2 technical brief](https://download.intel.com/newsroom/2021/new-technologies/neuromorphic-computing-loihi-2-brief.pdf).
The proposed mappings in the table are our inference. A programmable neuron
pipeline is not a conventional CPU on which arbitrary Rust functions can run.

There is a material tooling change since Loihi 2's introduction: Intel archived
the Lava repository on **13 May 2026** and states that all Lava repositories are
archived while a successor Loihi architecture and SDK are being developed.
The reviewed announcement does not establish that the successor is available.
The same repository describes Loihi extensions and research-system access
through the Intel Neuromorphic Research Community. This report has not confirmed
membership, current allocation, a loan, or access to the hardware extensions.
[Lava's current repository notice and access description](https://github.com/lava-nc/lava).

SpiNNaker2's official access page describes institutional access/purchase and
lists individual cloud access as forthcoming. Its software catalog warns that
some repositories require access and lists a low-level OS, Python communication
tools, and a C++ board interface. These are access conditions to confirm with
the platform provider, not evidence that this project can already deploy native
firmware. [Access](https://spinnaker2.gitlab.io/external/get_started/requirements/),
[software catalog](https://spinnaker2.gitlab.io/external/documentation/software/).

## Preserve the same reduct architecture

The current finite source route already lowers admitted rules, choices,
aggregates, and strong-negation coherence into a DAG with five operators:
atom, false, AND, OR, and implication. A neuromorphic backend should consume
that same immutable theory and atom identity. It need not implement a second
source language or move parsing away from themelios.

For a candidate `M`, first compute and freeze the classical truth `C_M[n]` of
every original DAG node. For a tested interpretation `J`, evaluate:

```text
R_M,J[atom(a)] = C_M[atom(a)] AND J[a]
R_M,J[false]   = false
R_M,J[and(x,y)] = C_M[n] AND (R_M,J[x] AND R_M,J[y])
R_M,J[or(x,y)]  = C_M[n] AND (R_M,J[x] OR R_M,J[y])
R_M,J[imp(x,y)] = C_M[n] AND ((NOT R_M,J[x]) OR R_M,J[y])
```

All asserted roots must be true. The candidate is stable precisely when it
models the original theory and **no proper subset `J ⊂ M` models this frozen
reduct**. That last condition is an exact search/coverage obligation. An arbitrary
Ferraris reduct can have incomparable minimal models, so a single least-closure
computation cannot replace it.

This separates three reusable transforms:

1. **Freeze:** original theory plus candidate produces immutable node truth.
2. **Evaluate:** frozen truth plus a tested world produces exact root truth.
3. **Cover:** a controller explores candidate or countermodel regions and records
   which regions have been completed, refuted, or remain unresolved.

The existing `FerrarisMask.lean` establishes the denotational mask equivalence.
It does not prove a hardware scheduler, packed representation, or firmware.
The existing `Events.lean` result concerns legal Horn publication traces with
semantic closure at completion. It must not be advertised as a proof of this
general formula machine. This proposal extends the execution direction in
`docs/design/zetesis.md` sections 11–12 and `docs/design/ferraris.md`; it does not
turn their planned neuromorphic backends into implemented features.

## An exact event implementation

Start with a finite, immutable dependency graph. A node publishes an explicit
Boolean value only after its required inputs for the current evaluation are
known. Maintain separate validity and truth state. **No spike yet means unknown,
not false.** Implication and default negation make this distinction essential.
The first implementation can use complete logical rounds or topological layers,
even on asynchronous hardware. Later change-driven evaluation may avoid
revisiting unaffected nodes after a world changes, but must invalidate every
dependent node and complete that work before freezing or reporting a result.

Proposed logical event identity is:

```text
(theory identity, candidate epoch, tested-world epoch, source node, input slot)
```

This is a logical protocol, not an assertion that either chip exposes a packet
with those fields. A physical mapping may use static routes, reserved channels,
tables, and synchronized reuse instead. Epoch wraparound requires draining or
quarantining old traffic; a small counter must never alias a live epoch.

Use integer truth, exact bounds, checked counters, and immutable semantic
parameters. Keep leakage, stochastic thresholding and learning outside the exact
channel. Prove saturation unreachable within admitted ranges or refuse the plan. A node's false value must be represented by
an explicit message or a proved phase-completion rule. Distinct input slots
remain distinct even when they refer to one shared DAG node; duplicate transport
of the same logical slot must not decrement readiness twice.

For a separately recognized Horn residual, retain the cheaper existing proposal:
deduplicated positive antecedents, an unmet-antecedent counter, a fired bit, and
idempotent atom publication. Counts must be assigned after substitution and
atom deduplication. This optimization requires the Horn premise; it is not the
general implementation of OR, implication, or recursive aggregate conditions.

On SpiNNaker2, place compact subgraphs on PEs, retain their state locally, and
route only cross-PE dependencies. Keep a scheduler and coverage ledger on
qualified device PEs. For dynamic relational work, use bounded keyed stores
and exact joins on the processors. Those stores are a later milestone: the
initial finite graph has already been materialized and does not establish lazy
grounding.

On Loihi 2, test whether fixed routes and separate validity/value channels can
realize the same small circuits. Intel documents bitwise operations, arithmetic,
comparisons, conditional control, and graded spikes. A graded payload participates
in downstream synaptic multiplication; it is not an arbitrary software message
header. Exact state widths and available microcode operations must be checked
against the accessible SDK. [Intel technical brief, pp. 5–6](https://download.intel.com/newsroom/2021/new-technologies/neuromorphic-computing-loihi-2-brief.pdf).

Loihi phase barriers also need explicit treatment. Lava documents a phased
`LoihiProtocol`; its separately named `AsyncProtocol` is documented for Python
CPU processes. Neither the name nor asynchronous physical circuits imply that a
free-running application protocol is available on neurocores.
[Lava synchronization protocols](https://lava-nc.org/lava/lava.magma.core.sync.protocols.html).

## Reliable completion is part of the oracle

A completed evaluation must account for local queues, scheduled handlers,
DMA, interconnect traffic, and delayed delivery. The first protocol should
admit a bounded batch, finish all of its messages/handlers, and acknowledge a
barrier before committing. A later asynchronous protocol can use explicit
credits or acknowledgments, provided it also handles messages generated while
completion is being checked. Reserving control capacity is necessary to avoid
deadlock when data traffic fills a queue.

The SpiNNaker2 paper explicitly describes FIFO modes that drop, overwrite, or
stall, together with routing/link diagnostics. It also describes logged SNN
time-step overruns. These are concrete reasons to qualify the chosen transport
mode and completion scheme rather than infer correctness from an absence of
spikes. [Chip paper, sections 6.2–6.5](https://arxiv.org/html/2607.24396v1).

For exact operation, every loss is prevented or detected. Repeated/stale delivery
is deduplicated or produces a fault. Diagnostic counter width, wraparound,
sampling interval, and coverage must themselves be checked; an unchanged
counter is not sufficient unless the counter observes every relevant failure.
Any fault, missed completion, exhausted allocation, or budget stop returns
`Incomplete`/`Faulted`, never `Stable`, `UNSAT`, or `Exhausted`. Already verified
results may be retained, while the enclosing all-model run remains incomplete.

## Candidate generation, feedback, and complete search

Use a bounded region controller rather than treating a settling network as a
solver. An outer region covers possible candidates. For a fixed candidate,
inner regions cover all its proper subsets. A split replaces one region by
children whose union is the parent. Disjoint ownership or exact semantic
deduplication prevents duplicate full models from being reported twice.

The ledger distinguishes queued, in-flight, completed, and unresolved regions.
One verified proper-subset witness rejects its candidate. Stability requires
completion of every remaining inner region. All-model exhaustion requires
completion of every outer region after any sound exclusions. For the empty
candidate, there are no proper subsets; this boundary must be handled exactly.
Timeouts and quiet networks do not close a region.

Proposal dynamics may choose promising candidates, branch order, placement, or
work budgets. The controller still retains a complete fallback for unexplored
regions. Countermodels provide useful feedback, but a countermodel to one
candidate cannot automatically exclude other candidates: changing `M` changes
the frozen reduct. Stronger region exclusions need their own proof.

Optimization remains outside reduct acceptance. Compute the current normalized
objective tuple semantics only for verified stable models. An incumbent can
justify a non-strict candidate bound that retains all optimal ties. Do not place
this bound in the original theory or in the tested-`J` query. In particular,
signed weights can make a proper-subset countermodel more expensive than `M`.
Old in-flight work must be reconciled with a new bound, not silently removed from
the coverage ledger. Keep strong-positive and strong-negative atoms distinct,
retain coherence constraints, and apply `#show` only after full model identity
and counts are established.

The existing exact native CPU search can initially complete unresolved regions
in an explicitly reported mixed deployment. The eventual device controller can
use finite splitting and exact transforms without adding a clingo dependency or
reproducing CDNL. Its worst case remains exponential; better scheduling does not
remove the underlying complexity.

## Rust remains the implementation center

The host owns themelios parsing, bounded source admission, original provenance,
theory/atom identity, placement compilation, and independent checking. Extract
the device event state machine into a small Rust library with explicit buffers,
bounded arenas, integer state, and no dependency on a vendor's Python object
model. The same logical transitions should run in a deterministic host simulator
and in a qualified firmware adapter.

Rust supports `thumbv7em-none-eabi` and `thumbv7em-none-eabihf` as Tier 2 targets
with `core`/`alloc`, covering Cortex-M4/M4F. This makes SpiNNaker2 `no_std` firmware
plausible; it does not supply a board support package. Choose the ABI to match
the vendor runtime, then qualify startup, linker sections, interrupts, atomics,
memory ownership, DMA, and router access. A narrow C shim is acceptable at that
boundary. [Rust target documentation](https://doc.rust-lang.org/rustc/platform-support/thumbv7em-none-eabi.html).

For Loihi, Rust should compile and verify the circuit description and control
the experiment. A small Python bridge may be necessary for an available Lava
extension, and embedded code may require the documented C integration. Lava's
architecture separates language-specific process implementations and deployment
configuration; it does not document a general Rust neurocore backend.
[Lava architecture](https://lava-nc.org/lava_architecture_overview.html).
Any claim that the controller itself is Rust on a Loihi embedded processor
requires a separately demonstrated target/ABI/toolchain path.

Report placement honestly: **host simulation**, **device transform with host
search**, **mixed placement**, or **device-controlled search**. The last requires
candidate control and exact checking on the device/SoC without a host decision
for each candidate. Conventional on-chip processor work must still be separated
from work performed by the neural/event fabric.

## What software testing can establish before access

| Test route | Useful evidence | Does not establish |
| --- | --- | --- |
| Rust deterministic event simulator with fault injection | Exact state transitions, legal event reorderings, stale/duplicate handling, completion ledger, whole-model parity on bounded exhaustive cases. | Device instruction behavior, real FIFO reliability, cycle time, power, or placement feasibility. |
| Lava CPU process models | Algorithm behavior and phase/interface experiments; selected supplied models have fixed-point or bit-accurate tags. | That a newly authored process exactly matches Loihi microcode, or that simulation predicts device timing. |
| py-spinnaker2 Brian2 backend | Supported built-in network behavior with its documented quantization/recording rules. | Custom Rust firmware, arbitrary custom neuron models, routing faults, or full-chip cycle accuracy. |
| Qualified native firmware/circuit tests | Actual reset, routing, overflow, integer boundaries, and end-to-end results on an identified board/SDK. | A speed/energy advantage until measured against the current optimized CPU and Metal baselines. |

Lava distinguishes bit-accurate and bit-approximate supplied models; a
`fixed_pt` tag by itself is not a universal hardware-equivalence guarantee.
[Dense process model documentation](https://lava-nc.org/lava/lava.proc.dense.html).
SpiNNaker2's Brian2 documentation lists supported models and a timing convention
adjustment for `lif_curr_exp`. Its custom-model tutorial explicitly says newly
registered models require hardware and are unsupported by that backend.
[Brian2 backend](https://spinnaker2.gitlab.io/py-spinnaker2/user_guide/brian2.html),
[custom-model tutorial](https://spinnaker2.gitlab.io/py-spinnaker2/tutorials/deep_dive/09_custom-neuron-models_basics.html).

The reviewed public material did not establish an available, supported
full-system emulator for arbitrary zetesis firmware on either target. That is
an unresolved tooling question, not a claim that no such tool exists. Pin any
archived simulator, SDK, compiler, and circuit/model implementation actually used.

## Proposed milestones and stop conditions

| Milestone | Concrete artifact and go/no-go test |
| --- | --- |
| 0. Access and ISA qualification | Obtain actual deployment rights, SDK/version and native programming documentation. SpiNNaker2: a minimal Rust image boots and exchanges a checked event through the real runtime. Loihi: the SDK can load a tiny exact latch/Boolean circuit. If either is unavailable, continue simulation and label native work blocked. |
| 1. Portable exact transforms | Implement the frozen-mask event evaluator and bounded region ledger in Rust. Compare all candidates and tested worlds for small independent formula trees/DAGs, including worlds outside `M`. Compare complete stable-model sets separately. Add property tests, fault schedules, meaningful mutations, and maintain the project's coverage gate. |
| 2. Transport and reset | On one device, test duplicate delivery, missing delivery, reorderings, backpressure, counter limits, stale epochs, cancellation, and reset with traffic in flight. Every run must give exact results or a typed fault. Any undetectable loss is a no-go for the exact channel. |
| 3. Resident reduct transform | Keep a fixed theory resident across many `M` and `J` epochs. Verify no leakage between epochs, exact non-Horn behavior and coherence. Measure cold setup, reset, mask formation, evaluation, completion and readback separately. Advance only if useful work amortizes those costs. |
| 4. Complete device search | Move region ownership/splitting and exact acceptance onto qualified device processors. Test accepted, rejected, empty, unsatisfiable and exhausted regions, including interrupted work. Require complete original model/cost contracts before calling this a native complete solver. |
| 5. Dynamic relations and useful workloads | Add bounded tuple stores, joins and subscriptions where the target supports them. Test late supports and simultaneous tuple arrivals. Demonstrate coverage without pre-enumerating all bindings; otherwise retain the static-profile label. |
| 6. Comparative performance | Use identical original programs and complete contracts; compare optimized CPU/Rayon, qualified Metal, and device routes. Preserve paired distributions, all failures, compiler/binary hashes, board/SDK and placement identity. Include host, management-core, reset, routing, completion and output costs. Measure full-system energy with a documented boundary. |

Adversarial semantic tests must include `a or not a` versus true under a frozen
candidate, unsupported positive cycles, `1 {a:a} 1`, nested implication,
nonmonotone aggregates, duplicate tuple eligibility, `J=M`, `J` outside `M`,
empty theories, and contradictory strong-negation facts. These cases expose
errors that simple spiking demonstrations or satisfiable examples miss.

Performance remains an open hypothesis. Resident state and sparse communication
could help repeated, localized evaluation, while dense dependencies, limited
local memory, many resets, and difficult inner search could erase the advantage.
A published Loihi 2 runtime study models both computation and communication;
its measurements are not predictions for ASP workloads.
[Timcheck et al., revised February 2026](https://arxiv.org/abs/2601.10035).

If SpiNNaker2 native access proves impractical, original SpiNNaker is a bounded
fallback for transport/controller experiments because its public API exposes
event callbacks, packet sending, DMA, and runtime operations. It is a different
hardware/toolchain target and would need its own qualification.
[Manchester native API](https://spinnakermanchester.github.io/spinnaker_tools/spin1__api_8c.html).
No additional platform currently has a sufficiently established exact-operator
and Rust deployment path in this review to justify dividing the first effort.

The immediate deliverable should therefore be the shared Rust event machine and
its exact completion tests, alongside access qualification for SpiNNaker2 and a
small Loihi circuit. This work remains useful even if either device fails the
hardware or performance gate: it develops the same composable reduct transforms
and explicit search coverage needed by the Metal path.
