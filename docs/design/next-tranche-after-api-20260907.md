# Next tranche: language foundations and grounder performance

The reduct is the permanent semantic foundation of zetesis. This applies to every
tranche and execution backend. Candidate generation proposes interpretations;
acceptance requires stable-model membership under the applicable reduct semantics.
General formulas use the Ferraris/clingo-oriented reduct. Normal-rule closure is
an exact specialization of that same stable-model requirement.

A class certificate can justify a cheaper exact acceptance procedure only while
its stated premises hold for the actual program. Such a procedure implements the
reduct criterion for that class; it does not replace zetesis's semantic foundation.
The general reduct oracle remains the exact path where those premises do not hold.
Heuristics, propagation, arithmetic plans, source transformations and hardware
scheduling cannot independently establish an answer set. Changes must preserve
stable-model correspondence, including any auxiliary-atom projection, and retain
explicit coverage and interruption obligations. No transition to CDNL or to a
SAT solver with ASP syntax is proposed.

Lazy candidate generation, GPU-parallel execution and reduct-based acceptance
remain the combined architectural objective. The present eager formula route
and CPU implementations are qualified reference/fallback paths; optimizing them
must also produce reusable, bounded operations for broader lazy and parallel
execution. The [grounding-selection plan](grounding-selection.md) maps existing
themelios classifications to distinct semantic eligibility, implementation
availability and cost decisions. It keeps broader lazy formula support as an
explicit following milestone, including unchanged SEND as a qualification target.

This document plans the next language work; it declares neither implementation
nor parity. Build on the reusable semantic sessions from the
[API checkpoint](api-hardening-20260907.md) through general binding and value
operations. Grounder performance is an active, dedicated track in this tranche,
with measured improvements as its objective. Dedicated optimization follows
closure of the intended-language gaps. Grounder work must contribute to that
architecture, not merely keep regression numbers unchanged.

## Target and evidence

The retained clingo 5.8.2 catalog has 21 admitted sources, 56 full models and three
explicit refusals. All 94 original non-clingcon kr-domains contracts pass. Neither
set supplies a language-wide denominator. The next concrete target is to close
the three selected refusals through reusable language mechanisms, while preserving
every existing admission, semantic, resource and output contract.

| Track | General mechanism | Original regression target |
| --- | --- | --- |
| Binding foundations | Positive structural pattern matching with transactional binding deltas and exact source-atom preservation. | `lparse/projectionBug/01`: one full model. |
| Finite value evaluation | Declared-input value plans, bounded constructor/arithmetic evaluation, constructed choice heads and already-safe evaluated negative atom arguments. | `aspcomp13/aspcomp2013_05/01`: 12 full models. |
| Conditional scope | Conditional-local positive witnesses and finite consequent alternatives under each universal implication. | `lparse/conjunction/07`: four full models. |
| Grounder performance | Bounded cost attribution, then measured improvements to value execution and relational joins in the applicable grounder. | Unchanged SEND+MORE for formula eager grounding; supported relational inputs for matched eager/lazy comparisons. |

These names identify unchanged upstream cases in
[`validation/upstream/clingo-5.8.2/cases.jsonl`](../../validation/upstream/clingo-5.8.2/cases.jsonl).
The [binding plan](../verification/remaining-binding-20260907/README.md) and
[construction plan](../verification/remaining-construction-20260907/README.md)
retain their original sources, refusals, counterexamples and dependencies.

## Shared interfaces and qualification

The existing flat `Expression.nodes` already supplies an arithmetic operation
plan. Value execution and performance changes must share this evaluator and
expression representation. Shared immutable plans and explicit per-execution
scratch are preferred over hidden mutable caches.

Conditional scope depends on established binding and value contracts. Dependent
consumers must preserve those foundations. Shared analysis projections and exact
original/frozen semantics require parity, resource, coverage and runtime checks.
Consumers of a new primitive depend on its established contract.

The [repository organization plan](repository-organization.md) calls for curated
ASP fixtures, Rust validation tooling and preserved raw evidence. Shared gate
changes require verified replacements. No solver packaging overhaul, ASPIF
implementation or theory extension is part of this tranche. The existing
prepared/session doors should be exercised by the new regressions.

## Semantic obligations

For structural matching, repeated and prebound variables must agree, anonymous
occurrences remain independent, and failed matches restore every binding. The
matched original atom remains in the compiled formula. Lean laws should state
matcher soundness/completeness, binding extension and rollback.

Value plans declare all inputs before evaluation. They preserve constructor
identity, choice grouping, duplicate eligibility through disjunction and original
default-negation depth. Fresh internal slots must not add support or escape their
scope. The laws cover input agreement, constructor denotation, fresh extension
and equivalence in both original and frozen interpretations. General aggregate
scheduling is not a hidden prerequisite for the selected construction target.

Conditionals require `AND rho (C(rho) -> OR nu L(rho,nu))`. Apply the consequent's
polarity before the inner disjunction. Empty consequent alternatives are false;
an exhausted empty condition domain is true. Witnesses provide neither outward
bindings nor atom support. Establish a bounded, complete, pool-free projection
for themelios analysis before admitting residual nested pools. Preserve explicit
refusal when that contract cannot be met.

## Grounder performance work

The retained [ordinary comparison](../verification/api-hardening-20260907/ordinary-comparison.md)
measures 50.636 ms of eager grounding and 15.952 ms of solving in a separate
67.406 ms SEND+MORE host-driver sample. Grounding accounts for about 75.1% of
that interval. An idealized halving of that stage alone would reduce the same
driver interval to about 42.088 ms, a 1.60-fold improvement. This is a sensitivity
calculation, not a prediction or a measured speedup; the 71.023 ms process median
and 12.477 ms clingo median are separate measurements.

The [five-solve diagnostic pilot](../verification/grounder-diagnostics-20260907/pilot-01/README.md)
retains a second SEND stage observation, the explicit lazy refusal and complete
eager/lazy reachability parity on the qualified binary. The attempted
process sampler failed without a trace. No internal hotspot or performance
improvement is established by that pilot; optional library attribution remains
the next measurement step.

Current code has three relevant paths:

| Path | Current work | First performance question |
| --- | --- | --- |
| Formula eager | Complete possible support, instantiate rule bindings and guards, intern formulas and validate the theory. SEND uses this path. | How much time and allocation belong to final joins, checked arithmetic, binding copies and formula construction? |
| Relational eager | Materialize the complete carrier and filter-valid Cartesian substitutions into `GroundProgram`. | What do carrier, substitution and storage costs contribute on inputs also accepted by lazy execution? |
| Relational lazy | Perform candidate-directed joins over derived relations during exact reduct closure. | What do repeated predicate scans, binding allocation and per-round relation construction contribute? |

SEND currently refuses `--grounder lazy`; this is a capability gap, not a lazy
performance result. Arithmetic evaluator improvements in formula admission do
not automatically accelerate relational lazy closure. Measure both paths, share
primitives where the contracts actually coincide, and keep any path-specific
improvement explicit. General lazy formula admission remains separate language
and architecture work; routing alone cannot supply it.

The original SEND encoding already decomposes addition into column constraints.
Those constraints enumerate forbidden assignments. Static counting gives 13,318
violating column tuples, while the admitted theory has 13,836 roots. These counts
motivate inspecting materialization but do not measure its time. Possible-support
construction skips constraints, so their joins are not repeated in every support
round. Formula joins already use column indexes, partial comparison pruning and
successful complete-row comparison reuse.

Proceed in bounded steps:

1. **Attribute the cost.** Preserve the existing outer grounding interval. Add
   optional library observations for support completion, objective preparation,
   rule materialization and finalization, with counters for probes, bindings,
   expression operations, scratch growth, emitted roots and interning. Join,
   arithmetic and emission work can be interleaved; counters are not time shares.
   Avoid clocks per tuple/operator and allocation-heavy event streams. Keep
   ordinary admission free of timing, preserve existing observer implementations,
   and do not nest the current non-reentrant `enter`/`exit` adapter. New detailed
   telemetry needs an additive interface, defined failure/overflow accounting
   and the same separation between library data and CLI rendering.
2. **Run a formula-eager ablation.** If attribution supports it, remove transient
   arithmetic AST construction beneath the existing flat expression plan and
   reuse bounded evaluation scratch. Dependency readiness is a separate ablation,
   coordinated with the binding plan. Preserve exact operation order, deferred
   prefix errors, final-row refusals, scalar payload charges and original formulas.
   Do not claim that allocation is dominant from source inspection alone.
3. **Run a relational-lazy ablation.** Measure stable bound-column probes over
   immutable per-round relations as an alternative to predicate-vector scans.
   This path does not already have the formula join's column indexes. Preserve
   complete matching rows, original deterministic order, frozen seed identity,
   guard timing and rollback. Preflight index storage and account setup cost;
   keep an exact scan fallback where an index is not justified. Do not combine
   this first experiment with a semi-naive closure redesign or an eager carrier
   contract change.
4. **Prepare parallel execution from demonstrated kernels.** Use owned Rayon
   workers with bounded local scratch for independent bindings after support is
   immutable. Deterministic merging retains IDs, provenance, quotas and failure
   order. Existing candidate/completion workers do not parallelize grounding.
   A later wgpu integer-evaluation kernel requires checked value/error parity
   and measurements including packing, dispatch and readback. No GPU grounding
   or speedup is established by the current GPU reduct oracle.

Do not replace forbidden arithmetic tuples with only equation-satisfying rows:
that would erase constraints. A compact arithmetic relation is a larger separate
transformation requiring original-world and frozen-reduct equivalence. Neither
SEND-specific names, decimal digits nor a hand-rewritten corpus should determine
optimization eligibility. The earlier [arithmetic plan](../verification/arithmetic-and-pure-functions-next-20260906.md)
and [lazy/eager protocol](../verification/lazy-eager-memory-20260906/eligibility-plan.md)
retain additional design and workload obligations.

Freeze each control and treatment separately. Compare uninstrumented full solves,
separate structured profiles and peak RSS; include planning, retained scratch and
index storage. Use unchanged SEND, arithmetic-light controls and general bounded
arithmetic/join families. For relational comparisons, explicitly select CPU and
the closure oracle on identical sources accepted by both grounders, including
reachability, sparse joins, selective comparisons and negative/UNSAT controls.
Lazy join time remains interleaved with solving; do not report it as an additive
eager-style interval. Keep refusal/limit cases out of completed-solve ratios.

Land optimizations on measured evidence of useful overall benefit, with small-case
overhead and negative results retained. A smaller microbenchmark time alone is
insufficient. The first evaluator change should preserve ordered atom catalogs,
formula nodes/roots, origins and objectives as well as complete models. Binding
and index laws establish enumeration equivalence; evaluator laws include value
and error behavior. Lean specifications do not automatically constitute Rust or
shader refinement proofs.

## Exit criteria and following decision

- Every newly admitted unchanged upstream case matches all full models. Small
  exhaustive original/frozen interpretation checks cover the new transformation
  laws, including recursive and negative cases.
- Existing cross-feature regressions and all 94 original corpus contracts pass.
  Compare library/session and CLI results, worker configurations, costs and ties.
  Validate provenance, safety, inclusive limits, rollback and recursive growth
  refusal; resource interruption must not manufacture coverage.
- Formatting, pedantic Clippy, strict docs, proof records and both independent
  91% coverage floors pass. Fresh matched ordinary timing and RSS evidence makes
  changes visible without replacing the original tasks or encodings.
- Grounding attribution and at least one controlled optimization experiment are
  retained for each applicable eager/lazy path. Report what improved, what did
  not and which cost remains dominant; a refusal or unsuccessful experiment is
  not a claimed performance win. Backend and worker claims identify the work
  actually executed there.
- Refresh the admitted-feature/refusal inventory against a broader bounded sample
  of the local clingo source and ASP competition inputs. Categorize each gap as
  required, excluded by the intended profile, or not yet assessed. Reaching 24/24
  selected cases is not enough to announce language parity.

Broader aggregate dependencies/cycles, dynamic head eligibility, key aliases,
directive/objective restrictions and exact undefined-arithmetic behavior remain
explicit inventory items. The six retained recursive-extrema endpoint differences
require [semantic investigation and checked implementation](numeric-semantics.md),
with the current guard retained until its replacement is justified. An internal
zetesis refusal does not establish a KR error or remove an intended feature from
the compatibility target. Keep themelios rejections, internal gaps and explicitly
agreed exclusions separate in the inventory. Theory atoms and embedded
Python/Lua scripting remain excluded. First-class Rust functions and future theory
propagators keep their separate integration designs.

After this checkpoint, use the refreshed language inventory and grounding
measurements to select the next work. Once the intended profile is closed, broader
optimization includes less copying,
batched indexed joins, larger lazy-grounding profiles and GPU execution. Preserve
exact class certificates and the general reduct fallback throughout.

The API release is qualified on CPU and portable CI. Its exact binary
still needs renewed physical Metal qualification; the execution context used for
the checkpoint reports no accessible adapter. Earlier hardware evidence keeps
its original identity. This is a device-evidence boundary, not an admission result
or a claim that the hardware lacks Metal.
