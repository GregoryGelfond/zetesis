# Exact event execution on neuromorphic hardware

This appendix specifies a possible neuromorphic execution backend for zetesis.
The semantic interface remains the one introduced in the [guided tour](../architecture/tour.md):
generate interpretations, check membership through the original program's reduct,
and account for the search before asserting completion. The proposed targets are
Intel Loihi 2 and SpiNNaker2. Neither is an implemented or qualified zetesis
backend; no device performance or energy advantage has been established.

The portable opportunity is to keep a dependency graph resident and communicate
changes to the operations that depend on them. Whether sparse communication
outweighs placement, reset and completion costs is an experimental question.
Dense dependencies and small device memories may favor the existing CPU or GPU
routes. A new backend must establish the same semantic result under its stated
resource limits.

## The operation being accelerated

For an original finite theory `T`, candidate `M`, and tested interpretation `J`,
separate three operations:

1. **Freeze:** compute each formula node's original truth under `M`.
2. **Evaluate:** compute truth in the reduct fixed by `M`, using membership in `J`.
3. **Cover:** account for every proper-subset region needed to decide minimality.

The first two correspond to the current
[`FrozenReduct` interface](../rust/reducts.md). Freezing does not assert that
`M` satisfies `T`. A completed evaluation does not assert that `M` is an
answer set. Membership additionally requires original satisfaction and the
absence of any `J ⊂ M` satisfying the reduct. Queries outside `M` are valid
satisfaction queries even though the minimality search only needs proper subsets.

Let `C_M(F)` be classical truth of formula `F` under `M`, and let
`R_M,J(F)` be its reduct truth under `J`. The Boolean equations are:

| Formula | Frozen evaluation |
| --- | --- |
| False | false |
| Atom `a` | `C_M(a) ∧ J(a)` |
| `F ∧ G` | `C_M(F ∧ G) ∧ R_M,J(F) ∧ R_M,J(G)` |
| `F ∨ G` | `C_M(F ∨ G) ∧ (R_M,J(F) ∨ R_M,J(G))` |
| `F → G` | `C_M(F → G) ∧ (¬R_M,J(F) ∨ R_M,J(G))` |

These equations define exact Boolean operations. An implementation may schedule
independent nodes together, share subexpressions, or reuse the frozen mask across
queries while preserving each node's denotation. Integer accumulation used to
realize an operation must have proved bounds; saturation or rounding cannot
silently change its Boolean result.

Consider:

~~~text
a :- not b.
b :- not a.
~~~

For `M = {a}`, the reduct requires `a`; its only proper subset is empty and
fails that requirement. For `M = {a,b}`, both rule bodies are false in the
candidate and the empty interpretation satisfies the reduct, so the candidate
is rejected. The original satisfaction test alone would accept both candidates.

A positive cycle is another necessary test: `a :- a.` has the empty answer
set, and activity persisting at `a` cannot justify accepting `{a}`.
For the [normal-rule class](../lean/normal-rules.md), least closure provides the
specialized membership operation. Unmet-antecedent counters and idempotent atom
publication are plausible event implementations. General disjunction and
Ferraris formulas still require their corresponding minimality argument;
positive disjunction need not have a unique least model.

## An exact event protocol

Each logical message belongs to a theory, a candidate epoch, a tested-world
epoch, a destination node and an input slot. A routing allocation may encode some
of this identity implicitly. This is a protocol requirement, not a claim about
the fields in either chip's packet format.

A node stores validity separately from value. Before an input is delivered its
value is unknown, not false. A first implementation should evaluate the acyclic
formula graph in ordered phases, committing a node once its required inputs are
known. Explicit false messages or a proved phase-completion rule establish
negative information. Shared edges require input-slot accounting: receiving the
same message twice must not satisfy two distinct antecedents.

The transition invariant is: every committed node value equals the frozen
Boolean equation for its theory, candidate and tested world. Reset either drains
old traffic or rejects it by epoch; an old message must never complete a new
query. Counter widths, epoch reuse, queue capacity and memory ownership have
explicit bounds. The exact channel uses deterministic state transitions with
learning and decay disabled unless a separate equivalence argument permits them.

Completion must include queued messages, active handlers, pending transfers and
traffic in flight. A quiet interval alone is not completion evidence. Overflow,
undetectable packet loss, missed barriers or cancellation cannot produce an
answer-set receipt. A backend must prevent such failures or expose a typed fault
or unfinished result. Backpressure must also leave enough capacity for the
protocol's completion messages.

## Candidate generation, lazy grounding and coverage

The initial device experiment can accelerate a resident reduct while the host
owns candidate generation and exact residual search. This is a useful placement
to measure, with host work reported explicitly. Moving the search controller
onto device processors is a separate step.

The inner controller partitions proper subsets of one fixed candidate; its
regions must cover the required search without losing unfinished work. Finding
a countermodel justifies rejection. Accepting a candidate requires complete
inner coverage or an applicable proved specialization. The outer controller
likewise accounts for original candidate regions before a complete world view
can be returned. An objective bound may prune optimization work but cannot
establish unrestricted enumeration of all original answer sets.

A materialized finite formula graph does not demonstrate lazy grounding.
Device-side demand materialization additionally needs bounded tuple stores,
joins, subscriptions to new supports and a coverage argument at the final
positive snapshot. Concurrent tuple arrivals must neither omit a newly enabled
instance nor give an instance another world's supports. Static placement and
dynamic source work should be qualified independently.

## Platform mappings and the Rust boundary

Loihi 2 provides programmable neuron microcode, integer-valued spike payloads,
and bitwise, arithmetic and comparison operations. Those facilities suggest
exact latches, Boolean gates and bounded counters as possible circuit components.
The mapping must establish the actual state widths, instruction behavior and
synchronization of the selected deployment. Neurocores are not general Rust
processors. Rust can own graph construction, validation, placement and host
control; a hardware adapter must use the available programming interface.
[Intel's Loihi 2 technology brief](https://download.intel.com/newsroom/2021/new-technologies/neuromorphic-computing-loihi-2-brief.pdf).

SpiNNaker2 uses ARM processing elements communicating through event packets.
That makes native event handlers a plausible mapping for the same transition
system. Its documented custom-model interface currently uses C on the chip and
Python on the host. A Rust implementation would require a demonstrated runtime
and ABI integration, not just translation of the algorithm.
[SpiNNaker2 architecture](https://spinnaker2.gitlab.io/py-spinnaker2/tutorials/deep_dive/06_hardware_architecture.html),
[custom-model interface](https://spinnaker2.gitlab.io/py-spinnaker2/tutorials/deep_dive/09_custom-neuron-models_basics.html).

Rust supplies bare-metal Cortex-M4/M4F targets, making a `no_std` implementation
worth investigating. Target support does not provide the board's startup code,
linker layout, interrupts, DMA ownership or router access. Those interfaces need
their own qualification against the actual firmware and deployment rights.
[Rust target documentation](https://doc.rust-lang.org/rustc/platform-support/thumbv7em-none-eabi.html).

The proposed reusable Rust boundary is a bounded event state machine with
explicit buffers and integer state. The same transitions should run in a
deterministic host simulator. Vendor deployment tools belong behind an adapter;
their object models should not define the solver's semantic API. Source parsing,
provenance and I/O remain with themelios and host libraries.

SDK availability is an unresolved deployment dependency. As checked in September
2026, Intel has archived Lava and states that it will no longer maintain it.
Its announcement mentions a successor architecture and SDK without establishing
a deployment path for zetesis. Any experiment using archived Lava must pin and
identify the actual toolchain; the event specification should remain independent
of that choice. [Lava project notice](https://github.com/lava-nc/lava).

## Lean correspondence obligations

The proof library already supplies relevant mathematical laws:

| Existing law | What a backend still needs to establish |
| --- | --- |
| `Ferraris.masked_eval_iff_reduct` in `FerrarisMask` | Stored masks, indexed nodes and event transitions implement the formula denotation |
| `Ferraris.stable_iff_masked_minimality` in `FerrarisMask` | Original satisfaction and complete proper-subset checking refer to the same candidate and theory |
| `completed_events_exact` in `Events` | Concrete publication traces satisfy its monotone legality and semantic closedness premises |
| `BatchAccounting.completed_results_exact` | Concrete region ownership, classifications and completion implement the abstract ledger |

The event law concerns normalized positive inference. It does not verify a
general formula machine, hardware delivery, counter behavior, epoch isolation or
a quiescence detector. Those are remaining refinement obligations, alongside
source-to-ground correctness. The [correspondence chapter](../lean/correspondence.md)
explains the distinction between mathematical laws and a verified implementation.

An implementation proof should follow the decomposition: graph denotation,
mask agreement, sound node transitions, transport preservation, completed-query
truth, and complete membership. Each theorem must expose the delivery, fairness
or finite-work assumptions on which it depends. Hardware tests then check the
selected device interfaces; they do not discharge every mathematical premise.

## Feasibility experiments

Begin with bounded Rust simulation: compare every `M,J` pair for small formula
graphs against the existing exact evaluator, and inject reordered, duplicated,
stale and missing events. Include nested implication, shared inputs, unsupported
cycles, nonmonotone aggregate eligibility, empty interpretations, strong-negation
coherence and interrupted work. Compare full answer families separately from
individual satisfaction results.

SpiNNaker2 offers a Brian2 backend without a board, but it supports built-in
neuron models; its documentation explicitly excludes newly registered custom
models. It can inform component experiments, not qualify arbitrary Rust event
firmware or predict full-system timing.
[Brian2 backend](https://spinnaker2.gitlab.io/py-spinnaker2/user_guide/brian2.html),
[custom-model limitation](https://spinnaker2.gitlab.io/py-spinnaker2/tutorials/deep_dive/09_custom-neuron-models_basics.html).

Progress to hardware in independently reviewable steps:

1. Establish SDK access and a minimal exact latch or event exchange.
2. Qualify routing, reset, arithmetic boundaries and completion under faults.
3. Keep one theory resident across many candidate and query epochs.
4. Move candidate-region control only after completed membership is reliable.
5. Add demand-driven source work with its own coverage checks.

For each measurement identify whether execution is host simulation, a device
transform with host search, mixed execution, or device-controlled search. Also
separate conventional on-chip processor work from neurocore work. Measure
placement, freeze, reset, communication, completion, readback and host residual
costs against current CPU/Rayon and Metal routes using the same semantic task.
Peak memory and full-system energy need explicit measurement boundaries. No
simulator result alone establishes native feasibility, speed or energy savings.
