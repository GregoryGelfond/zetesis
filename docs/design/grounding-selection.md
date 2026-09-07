# Analysis and grounding selection

Lazy candidate generation, parallel/GPU execution and reduct-based acceptance
remain zetesis's architectural objective. A source classification should help
establish and expand executable strategies. It must not turn an unfinished lazy
implementation into a theorem that eager grounding is required.

## The existing themelios vocabulary

This map was checked against the dependency pin
`87c11a3f2b72b81a12fd53226941fdf95e7294d3`, not substituted from the sibling
checkout's latest source. `Analysis::of(&Program)` assembles four facets:

| Existing facet or method | Use in zetesis planning | Additional obligation |
| --- | --- | --- |
| `constructs()`; `Constructs::uses`, `first`, `all` | Identify choice, disjunction, aggregate, arithmetic and other occurrences, with statement witnesses. | Inspect the actual construct form and context. A choice flag does not distinguish singleton choices from bounded groups. |
| `classes().normality()`, `horn()`, `uses_choice()`, `uses_disjunction()` | Identify candidate source-to-template translations. | Establish complete supported lowering, binding and gate semantics. Head shape alone does not certify the body. |
| `classes().tightness()`, `head_cycle_free()`, `stratification()`, `confirmed()` | Supply premises for separately justified execution or reduct specializations. | Match the theorem to the precise fragment and representation. These classes do not mandate a materialization strategy. |
| `safety().is_safe()`, `unsafe_statements()`, `finiteness()` | Supply scoped safety and conservative finiteness evidence. | Preserve the verdict's dialect and faithful-raising premises, and all resource bounds. Finite does not mean small. |
| `dependencies()`; `components()`, `component_of()`, `positive()`, `edges_from()` | Identify recursive components and positive, negative and aggregate dependencies. | Prove component composition before treating completion as global; graph components are not automatically independent solves. |

There is no existing `LazyGroundable` class. `Verdict::Unknown { witness }`
means that the property was not established by that analysis. It is not false,
infinite, unavailable analysis, or a requirement for eager execution.

## Why one class flag is insufficient

Normality checks head shape and permits bodies requiring additional support.
Conversely, the source `{p}.` is not normal under that classification, yet its
existing singleton-choice translation can use lazy reduct closure. Positive
recursive reachability can also use lazy closure without a predicate-level
tightness proof; negative cycles can be handled through frozen candidate gates.
Thus neither normality, tightness nor stratification alone classifies the current
lazy engine's capability.

SEND uses bounded choices and evaluated arithmetic beyond that engine's current
template support. These are implementation boundaries, not an impossibility
result for lazy grounding. The preparation regression now executes pinned analysis
on the unchanged corpus source: the normalized projection uses choice, is safe
under the pinned analysis, and has a `Holds` tightness verdict. It obtains these
facts with the support-round ceiling set to zero, then confirms materialization
still refuses that ceiling. Tightness neither supplies a lazy implementation nor
waives resource limits.

The general point also has prior research support: Bomanson, Janhunen and
Weinzierl show aggregate extensions through on-demand normalization in
[Enhancing Lazy Grounding with Lazy Normalization in Answer-Set Programming](https://ojs.aaai.org/index.php/AAAI/article/view/4119).
Aggregate occurrence alone is therefore not a universal eager-grounding
requirement. Their result is not a correctness proof for zetesis's proposed
implementation or a reason to adopt their search algorithm.

## Facts, capability and preference

Use a composition of distinct decisions:

1. Analyze the identified program under bounded preparation.
2. Establish which implemented grounder/oracle combinations preserve its source,
   binding, candidate coverage and reduct obligations.
3. Choose among eligible combinations using relation sizes, measured selectivity,
   estimated materialization, device limits and observed execution costs.

For the existing lazy template route, a schematic eligibility obligation is:
supported finite template lowering, complete positive binding and gate carrier,
and exact correspondence between the translated closure check and source stable
models. This is a description of proof premises, not a new checked Lean theorem
or a claim that `ProgramClass::Normal` supplies them all.

A future library assessment should retain four dimensions separately: established
eligibility with its subject/evidence; implementation availability with located
unsupported features; analysis availability with the actual verdict/witness;
and cost preference among eligible paths. Explicit `--grounder lazy` must not
silently execute eager grounding. Automatic selection should explain the selected
path through structured statistics and its human-readable view. A wrong cost
estimate may hurt performance; it must not alter accepted models.

## Prepare before deciding to materialize

`prepare_formula` and `prepare_bundle_formula` expose the existing bounded
compiler preparation before possible-support completion or theory materialization.
Their owned `PreparedFormula` and `PreparedFormulaBundle` receipts retain source
evidence, metadata, the analyzed program and its upstream analysis. Consuming
`ground()` resumes the original expansion budget and grounding limits; the caller
cannot replace them at that boundary. The existing admission functions compose
preparation and grounding. Optional observation starts only when grounding starts.

This is a formula compiler preparation, not a universal routing API. It can still
refuse forms that the formula compiler cannot lower; success does not prove later
arithmetic is defined, that materialization fits its limits, or that lazy formula
execution is implemented. Relational `Admitted` and `AdmittedBundle` still expose
no equivalent retained analysis. A future planning API must cover those boundaries
without materializing a theory merely to choose a grounder.

The analyzed program is a normalized, pool-free logical/optimization projection:
constants and closed terms are resolved, facts and admitted pools expanded, and
weak constraints normalized. It omits `#const`, `#defined` and `#show`, as well as
the subsequently generated Ferraris support/coherence formulas. Retain original
source/metadata separately and identify which representation each fact describes.
An absent pool flag in this projection says nothing about pools in the source.

`Analysis::of` internally unpools, so reuse the existing node/edge/payload preflight
and bounded projection rather than passing raw pooled inputs around them. The
pinned safety analysis also does not treat clingo aggregate equality guards as
binders: zetesis can admit `n(N) :- N=#sum{}.` while retaining an upstream unsafe
verdict. A favorable finiteness result cannot be detached from its safety premises,
and that dialect difference must not relabel valid admitted bindings as KR errors.
The [analysis integration contract](program-analysis.md) records these boundaries.

## Broader lazy formula milestone

The [next tranche](next-tranche-after-api-20260907.md) builds measured grounding
primitives and capability evidence. The following architecture milestone must
explicitly extend lazy execution to formula constructs, with SEND as one original
qualification input, rather than leaving eager fallback as the permanent answer.

That extension needs complete candidate coverage while instances and possible
atoms are discovered incrementally. Unseen relevant instances cannot be assumed
false or irrelevant. Before acceptance, every required source/reduct obligation
must be discharged, either by complete generation/checking or an exact covering
representation with justified premises. Graph growth must not reuse a receipt
or class certificate for a different theory. Enumeration, objectives and
interruption need their own completion evidence.

Grounding, oracle specialization and hardware placement remain independent
planning dimensions with checked compatibility. Pure value/binding operations,
immutable plans and bounded batches should support Rayon and GPU kernels as
their implementations become available. CPU references and eager paths retain
qualification/fallback roles; they do not replace the lazy/GPU/reduct objective.

Physical Metal execution while retaining lazy grounding is required before
version 1.0. The current explicit lazy/Metal refusal is an implementation gap,
not a semantic requirement. The [current tranche plan](dependency-tranche-20260907.md#required-before-version-10-lazy-metal-execution)
records complete candidate/source accounting, useful device work, bounded
execution and CPU/oracle/device qualification as acceptance obligations.
