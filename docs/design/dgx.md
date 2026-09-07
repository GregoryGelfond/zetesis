# NVIDIA DGX execution target

NVIDIA DGX is a prospective execution target. Its model, GPU generation,
device count, memory, interconnect, driver environment and access mechanism are
not yet known. This is a concrete future qualification target alongside the local
Apple M4 Pro Metal target. No DGX execution or performance claim has been made.
Full source and solving compatibility remains the current implementation priority.

## Required execution boundary

The target is GPU acceleration of the complete stable-model search workload:
candidate generation, frozen-reduct construction, and exact support/minimality
checking. Parsing, source validation and orchestration may run on the host. A
GPU microkernel called by a dominant CPU search loop cannot alone demonstrate
that this target has been achieved. Report the host/device split explicitly.

Every backend must implement the same candidate and reduct contracts. Device
truth is exact integer/bitset data. A found proper-subset reduct model rejects a
candidate. A stable-model verdict requires a complete exact check, including
coverage across all rules or formula roots; a timeout or unfinished device queue
is incomplete. Neither approximate proposals nor optimization costs supply support.

Rust and wgpu remain the first portable implementation path. Metal and NVIDIA
backends should share admitted representations, transformer contracts, reference
oracles and conformance fixtures where their capabilities permit. Backend-specific
implementations may be evaluated later when a measured portability limit warrants
them; CUDA is not currently implemented. Existing NVIDIA selection uses a vendor
filter through an available compiled wgpu graphics API.

## Residency and multiple GPUs

Keep immutable theories/templates, relation indexes and reusable transport buffers
resident. Batch candidate truth masks and share source structure across worlds.
Record host-device transfers, synchronization, peak host/device memory, resident
capacity, submission costs and useful work. Increasing candidate batch size must
not require duplicating the complete logical theory for every candidate.

The first multi-GPU decomposition should assign disjoint candidate regions to
devices with a complete shared theory replicated per device when it fits. A global
coverage ledger accounts for every region, retry and incomplete task. Full models
must remain unique, and all devices must exhaust their regions before an UNSAT,
complete enumeration or optimum claim. Device failure cannot erase a region.

Programs larger than one device need a separate distributed-theory design.
Partitioning rules alone does not permit partition-local stable-model decisions.
Closure frontiers, frozen candidate truth, proper-subset witnesses and final
coverage need an explicit global contract. Multi-device ownership and communication
must preserve those obligations before this path can report accepted answer sets.

## Qualification and contribution criteria

The benchmark runner should record actual adapter identities, device count,
usable memory and binding limits, drivers, operating system, compiled/selected
compute APIs and any inter-device transfer capabilities. Avoid assuming a DGX
model from its name or an accessible memory size from advertised hardware.

First establish complete answer-set/count/cost parity on unchanged programs and
small adversarial reduct cases. Then measure end-to-end time and peak host/device
memory against the best available CPU configurations and an external clingo
reference, with identical result contracts and explicit limits. Also break out
source expansion, relational materialization, candidate search, reduct work and
objective evaluation. Warm/cold runs and setup/steady-state costs remain separate.

The key experiment is an unchanged domain whose eager grounding becomes too large
for the baseline while candidate-directed materialization and batched exact reduct
transforms remain tractable. Record incomplete and out-of-memory runs as such;
never interpret them as UNSAT or discard them from a speedup comparison. A strong
result would demonstrate both semantic parity and a material improvement in the
complete workload, with enough source, commands and measurements to reproduce it.
