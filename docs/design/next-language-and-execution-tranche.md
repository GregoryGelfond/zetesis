# Proposed next language and execution tranche

This proposal adds no performance evidence.
The [current checkpoint](../verification/language-execution-tranche-20260908/README.md)
and its source-specific records remain the baseline.

Accepted-language gaps take priority. CPU and GPU work proceed concurrently as
bounded, general operations whose applicability and costs are explicit. The
original program and its frozen reduct remain the acceptance foundation;
candidate restrictions, domain information and device results never substitute
for complete semantic checking.

## Scope and dependencies

| Capability | Primary deliverable | Dependency boundary |
| --- | --- | --- |
| Source language | Completed-value nonbinding aggregate guards, then scoped min/max heads | Shared frontend IR and lowering precede dependent consumers. Each slice needs separate qualification. |
| CPU execution | One general candidate-cardinality or domain operation, selected from measured work | Establish applicability and a matched baseline before implementation; do not combine unrelated heuristics. |
| GPU execution | Per-input transport replacement evidence, then one justified retention change; Vulkan qualification interfaces | Trace before changing retention. Use shared backend/report types and preserve explicit Metal identities. |
| Shared contracts | Public API clarity, Lean registry, independent qualification and objective-presence foundation | Shared types precede consumers. The objective investigation produces a separate reviewed contract, not an incidental relaxation of admission. |

Independent changes retain separate proof and measurement scopes.

## 1. Close the next two language gaps

**Completed-value nonbinding aggregate guards** are the smallest continuation of
the current assignment planner. For example,
`q(M):-N=#count{},M=#sum{N},M<=#count{}.` still reaches the explicit
`AggregateAssignment` refusal. The new consumer would read the completed row;
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

**Min/max heads under the existing tuple/head bijection** follow next. Retain all
head permissions independently of the bound, reuse complete-value extremum
compilation, and define empty groups and `#inf`/`#sup` explicitly. Begin with a
reviewed numeric profile if a complete ordered-value source contract cannot be
qualified in one slice; do not label that profile general min/max support.
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

## 4. Trace GPU input replacement before expanding retention

The [lazy Metal ABBA evidence](../verification/language-execution-tranche-20260908/timing/metal/review/README.md)
records real reuse but mixed old/new timing,
and the strongest observed CPU route is faster in all twelve cases. It does not
justify changing ordinary defaults or claiming a general GPU speedup. The next
question is which input/result-shape/budget condition causes each whole transport
replacement, not how to pool only favorable observations.

Add bounded typed per-buffer replacement reasons and retained/active payload
accounting to the existing library result and measurement views. Keep logical
payload distinct from driver allocation and RSS. Trace the same sparse/dense,
growth and shrinking-chunk controls before proposing selective input reuse or
bounded geometric capacity.

If the trace justifies it, retain unaffected **input** buffers in one subsequent
slice. Preserve exact-capacity admission when slack would exceed a limit, checked
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
Parameterize backend selection in reusable qualification APIs before adapting
the CLI. Preserve requested and actual API/device metadata, typed failures and
backend-specific route identities; never publish Vulkan observations under a
Metal label or substitute a software adapter.

Sequence the changes: shared typed selection/report fields, portable parser and
accounting controls, explicit physical fixture selection, then native Linux
execution. Retain Metal regression coverage. A native Linux build of the same
frozen source must record binary/toolchain/driver identities, exits, complete
results and actual Vulkan work. A successful inventory is not device qualification. Keep
Linux/Metal and Linux/clingo timing populations distinct and preserve refused
lazy-formula cells. No hosted NVIDIA or Vulkan result is presumed.

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
