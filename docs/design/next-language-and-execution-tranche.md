# Language and execution tranche: plan and progress

The [completed checkpoint](../verification/language-execution-tranche-20260908/README.md)
at `1dc167beda8549681957cdc34c1cfae8e35043c4` records the baseline,
with reviewed evidence and artifact identities. This plan adds native
aggregate primitives and a foundation-wide research exploration alongside the
existing language, CPU, GPU and Vulkan work. It adds no performance evidence.

Implementation and [integrated qualification](../verification/aggregate-primitives-tranche-20260908/README.md)
are complete for this checkpoint. Combined gates, independent
coverage floors, frozen release comparisons and 25 physical Metal tests pass.
Controlled GPU measurements remain incomplete. The following
[language-closure tranche](language-closure-tranche.md) prioritizes the remaining
language obligations before a dedicated optimization and Lean-hardening tranche.

| Tranche item | Current scope |
| --- | --- |
| [Nonbinding guards](../verification/nonbinding-guards-20260908/README.md) and [numeric min/max heads](../verification/extrema-heads-20260908/README.md) | Complete outer values remain consumers, and head permission stays separate from measured bounds under the existing tuple/head bijection. Full ordered-value head extrema remain outside this numeric slice. |
| [Native CPU aggregates](../verification/native-aggregates-20260908/README.md) and [numeric GPU aggregates](../verification/native-aggregate-gpu-20260908/README.md) | Standalone complete-group original/frozen operations pass portable and frozen Metal controls; ordinary source/solver aggregate lowering remains Boolean. Actual eligibility acquisition is host work; Vulkan remains unqualified. |
| [Source CountPlan](../verification/source-count-plans-20260908/README.md) | Optional closed source-to-partition bridge with a real pre-proposal consumer; original theory identity and reduct acceptance are preserved. Greedy discovery is incomplete and does not recognize queens02. |
| [Numeric-prefix experiment](../verification/numeric-prefix-20260908/measurements/README.md) | Declined after controlled comparisons showed mixed effects and regressions. Evidence is retained; the specialization is not a retained optimization. Source arithmetic remains checked i32. |
| [Selective lazy retention](../verification/lazy-transport-retention-20260908/README.md) and [Vulkan interfaces](../verification/vulkan-interfaces-20260908/README.md) | Portable ownership/resource and selection controls plus passing changed-path Metal tests. Linux hardware qualification and whole-solve Vulkan matrix profiles remain open. |

The [current Lean audit](../../proofs/verification/aggregate-ranges-20260908/README.md)
checks 819 laws across 72 modules. It establishes scoped mathematical aggregate,
partition and signed-range results, not Rust/WGSL or source-compiler refinement.
The acceptance criteria below still govern each deliverable; this progress note
is not a release qualification or new performance evidence.

Accepted-language gaps take priority. CPU and GPU work proceed concurrently as
bounded, general operations whose applicability and costs are explicit. The
original program and its frozen reduct remain the acceptance foundation;
candidate restrictions, domain information and device results never substitute
for complete semantic checking.

## Scope and dependencies

| Capability | Primary deliverable | Dependency boundary |
| --- | --- | --- |
| Source language | Completed-value nonbinding aggregate guards, then scoped min/max heads | Shared frontend IR and lowering precede dependent consumers. Each slice needs separate qualification. |
| CPU execution | One general candidate-cardinality or domain operation, plus the native aggregate reference and CPU execution slice | Candidate/domain operations and aggregate execution need separate measurements. Aggregate execution depends on the shared representation and semantic contract. |
| GPU execution | Transport replacement tracing and justified retention, Vulkan qualification interfaces, plus native aggregate kernels | Shared backend types and aggregate representation precede kernels; preserve explicit Metal identities. |
| Shared contracts | Public API clarity, Lean registry, qualification, objective presence and the shared aggregate contract | Aggregate IR depends on the source-language contract. Aggregate and objective contracts remain separate; neither silently relaxes admission. |
| Foundational research | A bounded survey of additional foundational operations | Distinguish existing features, measured bottlenecks and untested hypotheses. Proposals require independent implementation review. |

Independent changes retain separate proof and measurement scopes.

## 1. Close the next two language gaps

**Completed-value nonbinding aggregate guards** are integrated as the first
continuation of the assignment planner. For example,
`q(M):-N=#count{},M=#sum{N},M<=#count{}.` reached the explicit
`AggregateAssignment` refusal at the starting checkpoint. The new consumer reads the completed row;
it must not become a second producer of `M`. Preserve every original equality,
comparison, default-negation sign and rule activation. Mark the consumer in the
shared plan so existing objective restrictions remain effective.

Acceptance requires source-order permutations, scalar descendants, several
consuming guards, unrealized producer values, empty and decreasing bounds,
cyclic/unsafe controls, original source ownership and exact/one-short limits.
Compare original clingo results and independently authored original/frozen
formulas for arbitrary `M/J`, not just accepted stable models. Existing
AggregateConsumers and AggregateDependencies supply composition lemmas; complete
carrier enumeration, scheduler/cursor correspondence and Rust refinement remain
separate obligations.

**Numeric min/max heads under the existing tuple/head bijection** are integrated
as the second language slice. Retain all
head permissions independently of the bound, reuse complete-value extremum
compilation, and define empty groups and `#inf`/`#sup` explicitly. The qualified
slice accepts numeric first tuple values with ASP-term guards and the existing
endpoint limits; it is not general ordered-value head support.
HeadMeasures provides permission/bound composition under explicit numeric
agreement, not a proof of concrete aggregate compilation.

Keep both alias directions, unsupported derived-head polarity and objective
dependencies refused until their own arguments exist. Broader aliases need
separate per-head permission and per-whole-tuple activity representations;
removing the current bijection check is unsound. Empty extrema, repeated
eligibility, multiple conditions for the same tuple, original/frozen comparisons
and typed resource failures are required controls.

## 2. Establish objective presence separately

The current total-observer policy refuses objective-relevant filters and new
consumers because a proposed value does not establish that a tuple or priority
survives grounding simplification. Removing `plan.consumers` rejection alone
would not resolve that obligation.

Define a small library-first presence/activation contract for a
single total aggregate producer followed by scalar filters. Specify absent
bindings, false filters, zero and canceling weights, duplicate global keys,
priority disappearance and all optimal ties. Keep negative/disjunctive/conditional
dependencies, several producers and dynamic priorities outside the initial
contract. Implementation becomes a subsequent reviewed slice once its semantic
and representation obligations are concrete. No CLI-only workaround or extra
fixed priority may conceal missing presence information.

The [presence contract](objective-presence.md) now records that boundary and 15
original clingo/ASPIF references. It identifies an additional requirement:
impossible aggregate proposals can retain a reported zero priority slot.
Neither exact tuple coalescing nor accepted-model inspection alone determines
that layout. The existing admission refusal remains in force.

## 3. Use CPU evidence to choose one general operation

The [queens analysis](../verification/language-execution-tranche-20260908/analysis/queens.md)
identifies candidate generation as the main variant-02 diagnostic cost, with all
six inputs already using certified tight support and no residual queries. Local
cardinality groups and grouped diagonal eligibility expose different work than
one broad total and pairwise exclusions. The current CPU timing population has
recorded preparation interference, so exact causal speedup claims are not a
basis for selecting a change.

First inspect the retained cardinality/partition representation. A sound finite
partition certificate may derive smaller candidate bounds from a fixed total
and group exclusions. Keep the original theory intact, and prove that every
answer set remains in the candidate family. Use arbitrary partitions, overlapping
groups, missing elements and contradictory bounds as controls; never trigger on
a queens predicate or filename.

The [domain analysis](../verification/language-execution-tranche-20260908/analysis/domains.md)
also motivates interval/domain narrowing and expression-key joins for time
windows and scheduling. Reuse the themelios program/analysis vocabulary and the
independent `zetesis-domain` boundary. Missing facts, nonfunctional relations,
negative values and checked arithmetic prevent assuming the dependencies of one
fixture hold generally. Choose this alternative if profiling identifies it as
the stronger bounded operation; do not start both as an unmeasured rewrite.

Measure candidate decisions, join/filter rows, arithmetic visits and retained
representation sizes alongside time. Collect qualified peak RSS before claiming
memory reduction. Companion diagnostics must remain separate from uninstrumented
fresh-process timing. Full source/model/cost identity, negative controls and
baseline/candidate executable seals are required. Missing inner general-reduct
time remains unmeasured until explicitly instrumented; do not infer it by
subtracting enclosing phases.

The integrated [source CountPlan](../verification/source-count-plans-20260908/README.md)
now closes one deliberately narrow source-to-consequence path. Complete ordinary
choice or bijective count-head groups supply integer lower/upper premises.
Coalesced element eligibility must be the canonical true formula; a positive
domain atom remains ineligible even if the program entails it. Partition groups
must be unconditional or share the exact global activation, which every emitted
consequence retains. Discovery greedily follows retained grounding order;
`NoPlan` means that policy emitted nothing stronger, not that every valid cover
was ruled out. Optional failure preserves successful original admission.

This path consumes actual source evidence without depending on the candidate
solver. Its separate restriction can be installed before proposal; neither
source defaults nor reduct acceptance change. The earlier
[manual queens experiment](partition-consequences.md) supplies its own premises.
Its variant-02 work reduction does not demonstrate automatic recognition: that
encoding expresses a body count and pairwise exclusions, outside CountPlan's
initial profile. Time, restriction-construction cost and total memory still need
matched end-to-end measurement.

The [numeric-prefix experiment](../verification/numeric-prefix-20260908/measurements/README.md)
completed semantic/resource qualification and a controlled old/new comparison.
It evaluated short expressions in stack integers and resumed general evaluation
without replaying an operation or binding lookup. Source checked `i32`
arithmetic, failure order and authored work charges were preserved. The measured
mix of small benefits and regressions did not justify retaining the production
specialization; the negative result and complete evidence remain available.

Arithmetic work remains a priority. A next bounded experiment should remove
actual repeated work or batch binding evaluation, rather than assume that a
smaller temporary representation improves runtime. Batched numeric execution,
prefix-key reuse and domain narrowing still need separate semantic, resource
and complete-population measurement obligations. This progress update establishes no further arithmetic implementation.

## 4. Trace GPU input replacement before expanding retention

The [lazy Metal ABBA evidence](../verification/language-execution-tranche-20260908/timing/metal/review/README.md)
records real reuse but mixed old/new timing,
and the strongest observed CPU route is faster in all twelve cases. It does not
justify changing ordinary defaults or claiming a general GPU speedup. The next
question is which input/result-shape/budget condition causes each whole transport
replacement, not how to pool only favorable observations.

The integrated slices add bounded typed replacement reasons, per-buffer
allocation/reuse observations and retained/active payload accounting to library
results and measurement views. Keep logical
payload distinct from driver allocation and RSS. Trace the same sparse/dense,
growth and shrinking-chunk controls before proposing selective input reuse or
bounded geometric capacity.

The trace justified the bounded selective-retention slice: individually fitting
**input** buffers now survive other binding replacements. Preserve exact-capacity admission when slack would exceed a limit, checked
growth before allocation, obsolete bind-group ownership, fresh active truth and
every output clear. Keep output/readback exact-sized initially: larger buffers
need a separate active-range readback and decoder contract. Failed attempts must
retain submitted-work evidence, return no fabricated result and preserve the
existing poisoning/cleanup lifecycle. No cache crosses independent calls.

Require portable structural/limit controls, ignored hard-failing physical Metal
tests, ordered complete reference results and retained AB/BA pairs including
regressions. Source masks, candidate occurrences and actual work must match when
claiming a transport comparison. Neither Lean source-round laws nor fewer
allocations prove concrete device correctness or a runtime benefit.

## 5. Make Vulkan qualification truthful and usable

The [Fedora/Vulkan plan](linux-vulkan-qualification.md) identifies measurement and
test-selection gaps; ordinary explicit Vulkan construction already exists.
Reusable primitive qualification APIs and their CLI views now expose explicit
Vulkan selection; the repeated whole-solve matrix remains a separate extension. Preserve requested and actual API/device metadata, typed failures and
backend-specific route identities; never publish Vulkan observations under a
Metal label or substitute a software adapter.

Sequence the changes: shared typed selection/report fields, portable parser and
accounting controls, explicit physical fixture selection, then native Linux
execution. Retain Metal regression coverage. A native Linux build of the same
frozen source must record binary/toolchain/driver identities, exits, complete
results and actual Vulkan work. A successful inventory is not device qualification. Keep
Linux/Metal and Linux/clingo timing populations distinct and preserve refused
lazy-formula cells. No hosted NVIDIA or Vulkan result is presumed.

## 6. Preserve aggregates as native execution operations

Treat `#count`, `#sum`, `#sum+`, `#min` and `#max` as explicit, typed aggregate
operations with one semantic definition and separate CPU/GPU lowerings. This is
an additional architecture deliverable. Retaining tuple groups, eligibility,
weights or ordered values, and guards may avoid expanding and traversing large
Boolean graphs. Reduced compilation work, storage, transfer and evaluation time
are hypotheses to measure, not benefits established by this plan.

The current [formula representation](../../crates/zetesis-ferraris/src/theory.rs)
contains only atoms, falsum and Boolean connectives. Aggregate lowering uses
[threshold or subset-implication graphs](../../crates/zetesis-ferraris/src/aggregate/lower.rs)
and [extremum witnesses](../../crates/zetesis-ferraris/src/aggregate/value_extremum.rs).
Ordinary [GPU formula packing](../../crates/zetesis-wgpu/src/formula/packing.rs)
therefore still receives no native aggregate operation. The new
[retained Group API](native-aggregates.md) and dedicated GPU aggregate wire
provide a separate library route, without changing `Theory::Node` or substituting
shared Boolean roots. Preserve the original lowering as a reference and fallback;
source-to-native mixed evaluation still needs its own coherent consumer and
coverage/identity contract. CountPlan retains private source premises for a
different purpose and does not replace aggregate evaluation.

| Operation | CPU execution candidates | GPU execution candidates |
| --- | --- | --- |
| Count | Population count over packed tuple-eligibility bits; batching across groups/candidates | Segmented population counts and integer reductions |
| Sum / sum+ | Masked exact integer accumulation; vectorization and Rayon batching where profitable | Segmented weighted integer reductions with a proved accumulator range |
| Min / max | Ordered masked reduction; comparison-only witnesses when a value is unnecessary | Segmented extrema or guard witnesses; presence and a value or order-preserving rank |

Several small groups can be batched; one large group can be reduced in parallel.
Group size, candidate count, sparsity, transfer and synchronization determine the
useful layout. Device subgroup operations require capability checks; retain a
portable workgroup route. A floating-point dot product does not implement exact
ASP integer aggregation. Work distribution and early threshold termination need
their own accounting and soundness arguments.

### Shared semantic contract

The finite carrier consists of distinct **whole tuples**. Alternative eligibility
conditions for one tuple are OR-coalesced; equal weights or equal conditions do
not identify equal tuples. Preserve exact sum/sum+ source profiles, zero-weight
head permissions, comparison polarity and the source term order. Empty minimum
is `#sup`; empty maximum is `#inf`. A machine integer endpoint cannot stand in for
either logical value. Ranked extrema need an order-preserving dictionary with
stable identity across an invocation, not atom IDs or insertion order.

Start with complete finite groups, fixed values and an evaluated guard for each
ground instance. A future lazy group must establish completeness before an exact
reduction verdict; partial-source bounds require a separate sound approximation
law. Neither a fast reduction nor missing currently known tuples supplies that
coverage proof.

The semantic basis is an existing result: Ferraris's [*Answer Sets for
Propositional Theories*, Proposition 7(b)](https://www.cs.utexas.edu/~ai-lab/pubs/proptheories.pdf)
gives direct reduct evaluation for finite numeric aggregates without expanding
their canonical subset formulas. The scoped result has now been formalized in Lean. For the intended
abstract interface `A_P(F)`, let `P` be the exact guard predicate on selected
tuple indices, `a_i` eligibility truth in candidate `M`, and `b_i` truth of the
recursively frozen eligibility in interpretation `J`. The target contract is:

```text
J satisfies (A_P(F))^M  iff  P(a) and P(b).
```

The canonical finite-mask result is now encoded in
[`AggregateReduct`](../../proofs/Zetesis/AggregateReduct.lean), under an arbitrary
guard predicate over distinct tuple positions. Source-tuple coalescing,
ordered-value and arithmetic implementations, correspondence with existing
lowering, and concrete Rust/WGSL refinement remain separate obligations. Re-evaluating original
eligibility directly in `J` would change the contract. Head permission remains
separate from the numeric or ordered guard. Comparison complement (`!=`) also
remains distinct from outer default negation; neither substitutes for the other.

### Incremental delivery and acceptance

1. The retained `Group`/`Eligibility` library boundary is integrated. It validates
   shape, whole-tuple uniqueness, guards and original theory identity. The caller
   still supplies complete OR-coalesced tuples and the source relation; shape
   admission does not prove that the original program entails an aggregate.
2. CPU count/sum/sum+/ordered-extremum reductions are integrated for original and
   frozen formula observations, using `P(A) ∧ P(B)` for the reduct. Numeric results
   use checked `i128`; source terms and arithmetic remain checked `i32`. A future
   `i64` source profile requires coordinated value/evaluation/interchange changes,
   not merely a wider accumulator or lazy grounding.
3. The integrated GPU primitive accepts actual Group-bound eligibility records.
   Its numeric profile checks complete positive and negative totals separately
   against `i32` bounds before reassociation; nonnumeric extrema and unsupported
   guards return typed capability failures. CPU acquisition, wire preparation,
   reduction and readback retain separate limits/accounting. Portable controls
   pass, and the [integrated qualification](../verification/aggregate-primitives-tranche-20260908/README.md)
   passes the physical Metal tests. Vulkan execution and matched performance
   measurements remain pending.
4. The separate opt-in CountPlan provides a source-derived candidate consequence
   consumer. Ordinary aggregate model/reduct evaluation is still Boolean. A mixed
   native source/solver route must retain original head permissions, exact
   coalescing/coverage, activation and theory identity. Constant/shared/previous
   root IDs cannot be replaced by assuming one root uniquely names one group.
   `HeadMeasures` supplies a scoped composition law, not that compiler bridge.
5. Future complete-group lazy acquisition must establish tuple coverage before an
   exact aggregate verdict. Partial-group bounds need their own approximation
   laws. Wider device representations and negative sum+ head admission remain
   separate obligations; no primitive capability may silently narrow source
   acceptance or change ordinary defaults.

Use independently authored finite aggregate semantics, original clingo programs,
arbitrary `M/J` checks and generated tuple/weight/eligibility cases. Include empty
and duplicate groups, equal values with different tuples, nested and double
negation, nonconvex guards, signed cancellation, infinity/term-order cases,
irregular batches, exact/one-short limits and stop/failure paths. Check complete
model/cost parity and source ownership after integration.

Measure compilation, primitive evaluation and complete solving separately on the
same qualified source and candidates. Record formula/group storage, retained
payload, transfers, decisions and exact residual work, alongside time. A count
kernel benchmark does not establish a solver speedup; a smaller representation
does not establish lower RSS. Physical Metal qualification is required, with
Vulkan qualification following the explicit platform work in section 5.

## 7. Explore further foundational opportunities

Run a bounded research survey across source/domain analysis, joins and grounding,
candidate generation, frozen reduct checking and CPU/GPU execution. Look for
operations whose logical structure could survive lowering and admit better
parallel implementations or semantics-preserving transformations. Inspect actual
implementation and prior experiments before proposing a feature as new.

The report should rank six to eight concrete opportunities. Each entry identifies
current code and evidence, the proposed operation, its expected time or memory
mechanism, a semantic/Lean obligation, an experiment with negative controls and
its dependencies. Distinguish measured bottlenecks from plausible hypotheses;
use primary research or official documentation for external algorithm claims.
Keep general source coverage, reduct preservation, exact arithmetic and complete
candidate accounting explicit wherever affected.

The [first survey](foundation-optimization-opportunities.md) records seven ranked
opportunities and their current implementation boundaries. The deliverable is a
research report and recommended bounded experiments, not
automatic adoption of every idea. Preserve the current language and execution
tasks while evaluating its recommendations. Native aggregates are an organizing
example, not the entire survey. No domain-specific dispatch, CDNL replacement,
unqualified hardware speedup or novelty claim follows from the exploration.

## Checkpoint and limits

Completion requires reviewed library APIs and coherent body/head/observation
boundaries, current source documentation and scoped Lean records, original
clingo plus independent frozen-semantics tests, strict rustfmt/pedantic
Clippy/Rustdoc, and both unchanged 91% coverage floors. Changed device paths need
physical qualification; final release corpus checks must use frozen artifacts.
Publish time and memory evidence at its actual scope before changing defaults.

General formula lazy grounding remains distinct from source admission. Preserve
its complete-source and frozen-candidate obligations in the existing
[bounded-choice direction](lazy-bounded-choices.md); the two language slices
above do not close the corpus's lazy execution gap. `#project`, broader program
parts/externals, further construction profiles and Rust `@` functions retain
separate plans. `#heuristic`, `#edge`, current theory runtime support and
Python/Lua are outside the agreed present profile. Keep themelios frontend
numeric boundaries distinct from zetesis implementation refusals, including
nonnumeric sum **head** weights and the unresolved negative sum+ head profile.
There is no whole-language percentage or fully verified solver claim.
