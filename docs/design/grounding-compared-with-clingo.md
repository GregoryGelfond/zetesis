# Grounding in zetesis and clingo

This comparison is for ASP practitioners and new contributors. It describes
the implementation on 8 September 2026 and separates it from the proposed
[lazy bounded-choice extension](lazy-bounded-choices.md). The clingo reference
is the inspected upstream **v5.8.2**, commit
`a99ffb2a58293c68b28fcc283a1d1c9ccad900fe`.

The distinction concerns **when instances are generated, what is retained, and
how inference is scheduled**. zetesis keeps the original program and its frozen
reduct as the acceptance foundation. For the admitted formula semantics, a
candidate must satisfy the original theory and have no proper-subset model of
its reduct. Choosing eager or lazy execution does not choose a different
stable-model semantics. The [Ferraris specification](ferraris.md) is a permanent
architectural contract, not a description of an earlier experiment.

## Logical grounding and physical materialization

A correctness argument can quantify over a finite ground program without an
implementation storing every rule instance. Conversely, avoiding a ground-rule
vector does not remove the need to account for all relevant instances, possible
candidate decisions, or aggregate elements.

| Route | Representation and timing | What must be complete |
| --- | --- | --- |
| clingo ordinary grounding | Instantiate and simplify selected source parts, supplying ground output to the solving layer before solving those parts. | The grounding of the selected parts, with their parameter and incremental context. |
| zetesis eager formula | Complete possible support, then construct an immutable atom catalog and shared formula DAG before search. | Admitted source lowering, formula roots, objective and observation contracts. |
| zetesis relational lazy | Retain safe templates; generate instances through source joins while checking frozen candidate gates. | Each closure round's source coverage, the final membership check, and separately the candidate enumeration. |

These rows describe execution boundaries, not equal language coverage or memory
requirements. zetesis also has explicit static lowering for its relational
representation; “eager” does not imply that every native route uses a formula DAG.

For example, this normal program has two answer sets:

```asp
edge(1,2). edge(2,3).
start(1) :- not blocked.
blocked :- not start(1).
reach(X) :- start(X).
reach(Y) :- reach(X), edge(X,Y).
```

One answer set contains `blocked` and the two edge facts. The other contains
`start(1)`, all three `reach` atoms and the edge facts. Eager execution prepares
the relevant instances before checking candidates. In a lazy check whose frozen
seed selects `blocked`, no `start` or `reach` consequence is derived, so the
positive reachability joins have no matching rows. The check that selects
`start(1)` discovers those consequences over successive rounds. Both grounders
must still account for all candidate choices; this example demonstrates the
scheduling distinction, not a speedup. Both zetesis routes and clingo 5.8.2 were
checked against these same two complete answer sets.

## What clingo already avoids

Clingo's grounder, gringo, does not blindly enumerate a Cartesian product of
every variable over every constant. Its predicate domains support bound and full
indexes; binders distinguish `NEW`, `OLD` and `ALL` generations. Matching can
therefore use existing bindings and restrict recursive work to newly available
domain entries. The source explicitly maintains generation order in those
indexes. See [the pinned domain/index implementation](https://github.com/potassco/clingo/blob/a99ffb2a58293c68b28fcc283a1d1c9ccad900fe/libgringo/gringo/domain.hh).

Dependency analysis organizes statements into components. Grounding processes
those components through an instantiation queue; propagation updates domains
and advances generations until that queue is exhausted. The instantiator uses
binding dependencies for backjumping within the join. Thus component scheduling,
indexed joins and delta-based iteration are existing clingo techniques, rather
than distinctions that zetesis can claim merely by using relational operations.
The relevant sources are [input dependency construction](https://github.com/potassco/clingo/blob/a99ffb2a58293c68b28fcc283a1d1c9ccad900fe/libgringo/src/input/program.cc),
[component grounding](https://github.com/potassco/clingo/blob/a99ffb2a58293c68b28fcc283a1d1c9ccad900fe/libgringo/src/ground/program.cc)
and [instantiation and queue processing](https://github.com/potassco/clingo/blob/a99ffb2a58293c68b28fcc283a1d1c9ccad900fe/libgringo/src/ground/instantiation.cc).

Clingo also need not ground every future application step at once. Its API
grounds selected, parameterized `#program` parts, then solves the accumulated
ground program. Multi-shot cleanup can use the solver's top-level assignment
to remove false atoms from grounding domains and mark true atoms as facts,
enabling further simplification. This is feedback across grounding/solving
steps; it is distinct from zetesis's candidate-specific source rounds described
below. See the [official grounding, solving and cleanup API](https://potassco.org/clingo/c-api/5.8/group__Control.html)
and its [pinned source contract](https://github.com/potassco/clingo/blob/a99ffb2a58293c68b28fcc283a1d1c9ccad900fe/libclingo/clingo.h).

## What zetesis currently does

**Eager formula grounding.** The themelios adapter prepares bounded source IR,
retains source evidence and analyzes its normalized logical projection.
Possible-support completion then discovers an upper bound on producible atoms,
using indexed relations, binding/value plans and explicit work limits. Possible
support is not truth in an answer set: negative conditions and correlated
aggregate choices cannot be decided by membership in that upper bound.

The grounder instantiates against completed support and builds shared formula
nodes, original roots, support/coherence obligations and objective metadata.
Aggregate eligibility reuse is scoped to completed support and the exact outer
binding. Candidate-dependent conditions remain formulas for original and reduct
evaluation. This is an optimized materializing grounder. Its implementation
seams are [possible support](../../crates/zetesis-themelios/src/formula_support.rs)
and [formula construction](../../crates/zetesis-themelios/src/formula_ground.rs);
the [source API guide](../../crates/zetesis-themelios/README.md) states the admitted
constructs and current dependency restrictions.

**Relational lazy grounding.** The narrower relational route retains normalized
templates. A seed fixes atoms tested by default or double negation; the oracle
derives the least consequence closure under those frozen gates. Positive joins
read immutable closure snapshots. New consequences enable later rounds, and a
complete no-growth round establishes closure. Acceptance additionally requires
constraint satisfaction and agreement between the seed and the resulting
closure on the gate atoms.

The [source scanner](../../crates/zetesis-cpu/src/oracle/source.rs) retains a
relation index, join frames and one emitted instance, rather than a complete
ground-rule store. Exhausting a snapshot proves only that snapshot's coverage.
The [round coordinator](../../crates/zetesis-cpu/src/lazy.rs) keeps this distinction
through chunks, catalog growth, cancellation and failures. In the coordinated
chunk/device path, a union of worlds can offer bindings but cannot establish truth
in any individual world. Optional world masks prune positive prefixes with no
witness in the current worlds; they do not justify pruning future rounds or
candidates. The ordinary CLI currently selects union scanning. Independent Rayon
CPU batches instead check each seed separately; the coordinated portable routes
are also available for library use and matched experiments.

Candidate generation has its own potentially large obligation.
[`Candidates`](../../crates/zetesis-cpu/src/candidates.rs) incrementally enumerates
gate seeds, with an empty first seed that requires no carrier expansion. Complete
enumeration still covers the full gate carrier. Its present predicate/domain
carrier can contain many combinations, and seed enumeration remains exponential
in the number of gate atoms. Demand-driven catalog growth during one check does
not prove that undiscovered candidate choices are irrelevant. A stopped scan,
check or enumeration must retain its incomplete status.

## Hardware placement and library boundaries

The host uses pinned themelios for parsing, source identities/spans and logical
program services. zetesis owns bounded bundle loading and include traversal.
The adapter supplies validated native representations; it does not replace the
parser or invoke clingo. Independent libraries own
formula semantics, relational closure, candidate streams and backend execution.
`PreparedFormula::ground()` exposes eager materialization separately from
preparation; already-constructed native programs and theories have their own
checking APIs. [Grounding selection](grounding-selection.md) describes the
evidence those boundaries retain.

Rayon supports independent CPU work. The current relational
[`GpuLazyOracle`](../../crates/zetesis-wgpu/src/lazy.rs) uses host source joins and
bounded uploads, with Metal computing per-world consequences and frozen-gate
tests. It uploads neither a completed CPU closure nor a complete ground graph.
Formula Metal execution instead consumes the eager theory, accelerates general
reduct propagation, and preserves exact CPU completion of residual queries.
The [tight-support Metal primitive](metal-tight-support.md) is a separate
experiment, not yet the ordinary formula execution policy. None of these routes
is full GPU grounding or an entirely GPU-hosted solve.

The intended standalone-grounder and interchangeable-solver uses follow these
library boundaries. A native theory is an execution representation, not a
replacement for the source-level themelios `Program`. ASPIF import/export remains
proposed; the current system must not be advertised as a drop-in gringo or clasp
interchange replacement. Session dependency extraction also remains open, as
recorded in the [library-first audit and follow-up](library-first-20260907.md).

## Demonstrated scope and the next extension

The last complete [94-case physical corpus matrix](../verification/dependencies-measurement-tranche-20260908/physical/README.md)
passes every selected-answer, cost and optimum-tie comparison on eager CPU and
eager Metal. Native full-model families also agree between those routes; clingo's
hidden interpretations are unavailable in that campaign. **Both explicit lazy
profiles refuse all 94 formula inputs.** Relational lazy Metal has separate
device tests; they do not close this corpus gap.

That matrix reports higher Metal process medians than CPU on all 94 cases. It
includes fresh device setup, different CPU specialization, instrumentation and
output costs; native JSON carries full atoms while clingo exposes selected
symbols. It supplies neither an isolated algorithm ranking nor a memory result.
Removing a rule vector can save storage but repeated joins, candidate masks,
catalogs and transfers also cost memory and time. The
[performance protocol](corpus-performance.md) therefore separates eager phase
timings, interleaved lazy work, actual device work and explicitly unavailable RSS.

The proposed first lazy formula slice adds finite integer-bounded normal/choice
groups such as `1 {a;b} 1.`. It composes exact base reduct closure with complete
validation of original group bounds. It must cover every active group, including
groups with no eligible elements, and deduplicate selected heads without losing
their eligibility conditions. Complete gate coverage remains necessary; a full
retained table of all eligibility witnesses need not be. The
[bounded-choice design](lazy-bounded-choices.md) states the semantic argument,
API seams and missing implementation/proof obligations. It is progress toward
broader lazy source coverage, not an implemented general on-demand grounder.
