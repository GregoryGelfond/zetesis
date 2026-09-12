# Composing exact execution

The semantic layer asks about satisfaction, a frozen reduct and minimality. The
execution layer represents the required work as joins, masks, Boolean evaluations,
reductions and independent candidate queries. These primitives are exact
operations; they are not learned approximations or neural attention layers.

The [alignment chapter](alignment.md) connects the two vocabularies with a
diagram, primitive map and pseudocode for closure, frozen satisfaction and
batched membership.
The [ownership chapter](ownership.md) describes prepared data, shared device
contexts, invocation lifetimes and the dependencies that permit parallel work.

## The composed solver boundary

`zetesis-solve` owns the ordinary computation from a coherent `PreparedInput`
to checked `AnswerSet` values and a `SemanticOutcome`. Its configuration contains
execution policy and resource limits. It contains no command-line options,
source-file loading or output writers. `zetesis-cli` admits input and consumes
the same public session interface as an embedding application.

```text
source or native program
    → admission and preparation
    → PreparedInput + SolveConfig + Control
    → SessionBuilder + optional resources and measurements
    → Session: candidate generation → exact membership → checked AnswerSet
    → consumer retention or publication
```

The membership step follows one of the reduct paths below. `SessionBuilder`
composes the effects before execution; it does not add a second semantic engine.
Streaming permits partial consumption. `SessionBuilder::collect` instead owns
unrestricted enumeration and complete retention, producing a `WorldView` only
when both succeed. Output formatting consumes checked values after membership;
an output failure cannot retract checked membership or prove search exhaustion.

This boundary is implemented by
[`Session`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/session.rs),
[`WorldView`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/world_view.rs)
and the CLI's
[publication adapter](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cli/src/publication.rs).
The [library manual](../rust/sessions.md) gives checked examples and operation
contracts; the [proof correspondence](../lean/correspondence.md) distinguishes
the semantic laws from the remaining executable refinement obligations.

## Two exact membership paths

For the normal profile, a seed fixes the reduct's gates. Positive inference
starts empty, derives consequences, checks constraints, then compares the
result's gate projection with the seed. The dense graph and source-driven
oracles implement this same acceptance question.

Ordinary sessions retain shared atom selections across each candidate batch.
CPU and GPU checkers borrow their true atoms through `SeedView`; an owned `Seed`
uses the same checking implementation. Eager device packing writes into the
admitted batch buffer without constructing a temporary word vector for each
seed. These ownership and transport choices do not change the frozen gates or
the least-consequence check. Returned answer sets still own their interpretations.

For finite general formulas, the candidate must first satisfy the original
theory. A frozen truth mask then permits repeated evaluations of its reduct
without constructing a separate formula tree. Finding a proper-subset model
rejects the candidate. Proving that none exists accepts it. The exhaustive
`zetesis-ferraris` checker is an exact small-instance reference; ordinary formula
search uses the native candidate and countermodel machinery in `zetesis-sat`.
Its Boolean search representation does not redefine ASP as classical
satisfiability: auxiliary encoding variables do not participate in answer-set
identity or minimality.

Both paths can restrict candidate generation by necessary conditions. A normal
source constraint supplies a forbidden positive gate conjunction when its
remaining antecedents are witnessed by actual unconditional facts. The binary
seed cursor can skip a whole interval while those gate bits remain true.
Possible support alone cannot supply that witness. Ineligible constraints stay
with the full closure check.

For a theory whose complete asserted-head grammar is ordinary disjunction,
every true atom in an answer set must have an original producer whose body is
true and whose other head atoms are false. Otherwise removing that atom leaves
a proper-subset model of the reduct. The resulting support condition restricts
only outer proposals; independent producers can still support several atoms in
one disjunctive head. Choices and other rich asserted heads decline this
certificate. The original theory and frozen reduct are unchanged.

The [gate restriction laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/GateRestrictions.lean)
and [disjunctive support laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DisjunctiveSupport.lean)
state these necessary conditions. Source binding coverage and the executable
certificate constructors retain separate refinement obligations.

Exact projection exclusions have one owner across candidate restrictions. The
outer cursor retains an index of previously proposed semantic interpretations;
strengthening the candidate query rebuilds its traversal while preserving that
index. Original and restriction clauses alone enter the watch lists. A completed
assignment is independently checked against those clauses and looked up in the
exclusion index before becoming another candidate. The original theory and its
reduct remain separate from both operations. The
[projection contract](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-sat/docs/candidate-pruning.md)
states its transactional insertion and logical admission units.

Admitted classical queries use a single packed literal arena and clause-end
offsets. A literal occupies one machine word; repeated offsets preserve empty
clauses. Public `Literal` values remain lossless inputs until range admission.
`Cnf::clauses()` yields ordered borrowed `Clause` views, and `Cnf::clause(index)`
provides checked access. Decoding allocates nothing and preserves variable,
polarity and canonical order. This is a query representation change, with
[separate arithmetic laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/PackedQueryLiterals.lean);
it does not replace the formula reduct or establish a process-memory bound.

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
adapters. With default resources, device availability is discovered at runtime.
Requesting a backend is
not evidence that it ran; adapter identity, submitted work, completed work and
residual work are separate observations. GPU support does not mean the complete
solver is device-resident.

The static reduct oracle reads one framed closure record per candidate. A checked
nonzero epoch identifies the submission, an ordinal identifies the input seed,
and a nonzero marker follows synchronized closure writes. The host validates
these fields, the exact record population, verdict bits and unused closure bits.
It also checks the gate-carrier projection against the original seed. A missing
or inconsistent record fails the whole batch instead of becoming acceptance.
The 32-byte parameter buffer and four-word per-record header count toward the
ordinary authored payload ceiling. Empty batches validate context health and
graph admission without consuming dispatch resources or an epoch. The epoch
sequence survives residency clearing and refuses overflow.

These checks establish transport identity and representation consistency, not
an independent proof of every rule consequence or constraint verdict. Actual
shader execution still relies on the device API contract and requires physical
qualification for each claimed backend; an observed driver fault is not presumed.

For admitted relational programs, automatic materialization selects lazy source
grounding on both CPU and GPU. Hardware changes do not require a complete ground
rule store. Explicit eager grounding retains its compiled graph. General
formulas still require eager grounding.

Automatic hardware selection retains CPU throughout the solve. Existing
measurements do not establish a device crossover for an automatic policy; batch
size alone is insufficient evidence. Explicit shared CPU source batching also
remains a CPU policy. An explicit GPU backend prepares its executor during
session setup. Default resources discover a device at that boundary;
caller-supplied [ExecutionResources](../rust/sessions.md#share-execution-resources)
reuse their exact context after policy and capability checks, without discovering
a replacement. Supplying a context does not override automatic CPU selection.

An explicit device failure is returned without a hidden CPU retry. A source
limit or cancellation remains an incomplete result. Device statistics count
actual device attempts and never include seeds checked only by the CPU. This
policy leaves device scheduling separate from the reduct membership contract.

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
