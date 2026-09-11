# Composing exact execution

The semantic layer asks about satisfaction, a frozen reduct and minimality. The
execution layer represents the required work as joins, masks, Boolean evaluations,
reductions and independent candidate queries. These primitives are exact
operations; they are not learned approximations or neural attention layers.

The [alignment chapter](alignment.md) connects the two vocabularies with a
diagram, primitive map and pseudocode for closure, frozen satisfaction and
batched membership.

## Two exact membership paths

For the normal profile, a seed fixes the reduct's gates. Positive inference
starts empty, derives consequences, checks constraints, then compares the
result's gate projection with the seed. The dense graph and source-driven
oracles implement this same acceptance question.

For finite general formulas, the candidate must first satisfy the original
theory. A frozen truth mask then permits repeated evaluations of its reduct
without constructing a separate formula tree. Finding a proper-subset model
rejects the candidate. Proving that none exists accepts it. The exhaustive
`zetesis-ferraris` checker is an exact small-instance reference; ordinary formula
search uses the native candidate and countermodel machinery in `zetesis-sat`.
Its Boolean search representation does not redefine ASP as classical
satisfiability: auxiliary encoding variables do not participate in answer-set
identity or minimality.

Local search operations can specialize while preserving the candidate sequence
of completed search. In a three-literal clause, two distinct watched positions
leave one possible replacement position: `3 - first - second`, with indices from zero to
two. The [replacement operation](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-sat/src/search.rs)
tests that occurrence once, after polling control and charging work. It returns
the same replacement as the general scan. Omitting watched positions can reduce
charged work, so a fixed work limit can permit more progress. The
[ternary law](../lean/correspondence.md) states the position invariant; original
satisfaction and reduct membership retain their existing obligations.

A sufficient class certificate can avoid a full countermodel query. For example,
ranked support under a complete original-producer representation justifies a
tight-program specialization. A failed certificate requests exact residual
checking; it is not automatically a rejection. These are different procedures
for the same reduct-based membership contract.

The device [ranked-support checker](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/src/tight/check.wgsl)
stores support as one bit per semantic atom in each candidate's row. By default it clears
the row, then each enabled original producer atomically sets its head bit.
After the storage barrier, a bit is set exactly when that head has an enabled
producer. Atomic OR preserves updates from different heads sharing a word and
from repeated producers. The final atom scan retains the least unsupported
atom as its witness. This representation reduces the support buffer to
`4 * max(worlds * ceil(atoms / 32), 1)` bytes; it does not describe total device
memory or establish a speedup. Shared-word contention remains a measurement
question. This checker is a reusable device primitive; its availability does
not imply that ordinary solves select it.

The library also exposes `TightSupport::Grouped` through
`GpuTightOracle::new_with_support`. Packing places each original producer in the
group for its head's support word, retaining duplicate occurrences. One
invocation reduces that complete group into a local bit mask and overwrites the
word once. Empty groups write zero. This replaces contended producer ORs with
word ownership; original truth, root precedence, barriers and the least-atom
witness scan retain the same contract. The graph adds `ceil(atoms / 32) + 1`
32-bit offsets; fresh packing also needs one temporary 32-bit cursor per word.
Both payloads are charged before allocation. Few or uneven groups can limit
parallelism, so grouped construction is an explicit alternative, not the default.
The result marker identifies the branch that constructed support. A mismatched
policy marker is a readback failure even if the reported verdict agrees; this
validates the protocol, without proving the shader or device implementation.

The four physical
[tight-oracle tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/tests/hardware_tight.rs)
exercise both policies on Apple M4 Pro Metal. They cover original-root precedence,
least unsupported atoms, duplicate and skewed producers, packed-word boundaries,
batch isolation, resource refusals and retained-theory identity. The
[performance evidence](../reference/validation.md#performance-evidence) separates
these correctness checks from latency and occupancy measurements.

For 256 atoms and 128 candidates, the packed support buffer occupies 4,096 bytes
instead of the earlier per-atom buffer's 131,072 bytes. This saves 126,976 bytes
of logical device buffer storage. Bit packing alone leaves uploaded and
downloaded payloads unchanged; grouped construction additionally uploads its
immutable offsets on a fresh theory.

## CPU and GPU responsibilities

| Capability | Execution boundary |
| --- | --- |
| Source loading, parsing, admission and output | Host libraries |
| Independent relational membership | Scalar CPU or owned Rayon pool |
| Shared lazy relational rounds | Host source stream with CPU or GPU chunk evaluation |
| Ordinary GPU formula membership | Device propagation plus exact native CPU completion of residuals |
| Native aggregate reductions | Bounded reusable CPU/device primitive; availability alone does not imply use by every solve |

wgpu supplies portable device access, including Metal on supported macOS
adapters. Device availability is discovered at runtime. Requesting a backend is
not evidence that it ran; adapter identity, submitted work, completed work and
residual work are separate observations. GPU support does not mean the complete
solver is device-resident.

For admitted relational programs, automatic materialization selects lazy source
grounding on both CPU and GPU. Hardware changes do not require a complete ground
rule store. Explicit eager grounding retains its compiled graph, including when
automatic device execution falls back to CPU. General formulas still require
eager grounding.

The relational scheduler starts on CPU and may discover a GPU when a later
candidate batch contains at least 32 candidates. This is a provisional scheduling
heuristic, not a measured crossover. Explicit lazy grounding permits the same
automatic discovery. An explicit GPU backend initializes its requested device
immediately; explicit shared CPU source batching remains a CPU policy.

If automatic device execution fails before publishing a batch, the engine can
retry those same seeds on CPU. It retains the failed lazy device attempt's work
record. A source limit or cancellation remains an incomplete result rather than
a reason to exceed that bound. Device statistics count device attempts; they do
not include seeds checked only by the CPU.

## Immutable rounds and commit boundaries

Each lazy batch has independent frozen seeds and positive snapshots. All chunks
in one round read those same snapshots. Deltas become the next snapshots only
after the source scan and every required evaluation complete. Catalog growth
preserves atom identity and each world's stride; it cannot turn another world's
truth into a local fact.

An incomplete scan or failed evaluation returns progress without completed
checks from that batch. A formula batch similarly separates pending proposals,
completed local checks and committed results. Cancellation or a device error
must not turn an unfinished operation into acceptance, rejection or exhaustion.

Performance work measures the actual boundary: admission, source work,
candidate generation, oracle calls, residual completion, scoring and output.
Host elapsed time around a GPU call includes transport and waiting; it is not
kernel time. Logical storage budgets are not process RSS or total device memory.
The [outcome contracts](../rust/outcomes.md) preserve these distinctions for
library consumers.

The [neuromorphic appendix](../appendices/neuromorphic.md) describes a proposed
mapping of the same semantic operations to Loihi 2 and SpiNNaker2. Its event
protocol and qualification steps are design obligations; these targets are not
implemented backends.
