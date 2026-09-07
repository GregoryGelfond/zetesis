# Aggregate dependencies, witnesses and bounded execution

The starting checkpoint is `8ce3191`, with all four
[hosted checks green](https://github.com/GregoryGelfond/zetesis/actions/runs/34156153285).
The [combined local checkpoint](../verification/dependency-tranche-20260907/README.md)
now qualifies the integrated dependency, witness, CPU and validation slices.
The selected-upstream Rust campaign remains unfinished. Hosted checks qualify
the published revision separately; the starting record retains its historical scope.

Exact original-program satisfaction and frozen-reduct stability remain the
acceptance foundation. The themelios pin, admitted numeric semantics and public
resource-stop distinctions remain unchanged.

## Independently reviewable slices

| Capability | First implementation | Preservation obligation |
| --- | --- | --- |
| Aggregate dependencies | Explicit required/produced slots and stable execution order for scalar/tuple checks and scalar/constructor bindings consuming independent aggregate assignments. | Attainable values propose bindings; the original aggregate equalities and guards remain in the semantic formula. Dependency failure is a native execution limitation, not a blanket source-unsafety conclusion. |
| Conditional witnesses | Positive structured local witnesses through the existing transactional pattern matcher. | Preserve the full consequent, scope, constructor identity, signs and repeated/prebound agreement, with failed-row rollback and charged retained copies. |
| Validation library | Typed bounded subprocess capture and answer normalization, then a curated selected-upstream campaign composed from those capabilities. | Distinguish process completion, semantic completion, display multiplicity and full-model identity. Preserve output/deadline limits, partial evidence, cleanup failure and protected report publication. |
| Lazy CPU joins | Investigate leading-bound-prefix windows over already ordered relation rows. | Skip only rows proved unable to match; retain row order, complete binding checks, guards, frozen seed and original reduct closure. Charge lookup work and retain explicit stops. |

The aggregate first slice excludes cross-aggregate dependencies/cycles and new
range, choice, conditional or objective scheduling. A positive source atom that
consumes an aggregate result needs its own scheduled join; ordinary source
relations must not appear to bind that result in advance. The witness slice does
not solve inverse arithmetic or negative anonymous binding.

The validation migration replaces consumers before retiring legacy tooling or
copied upstream C++ sources. Historical evidence needs verified preservation;
changing GitHub language attributes would not complete that cleanup.

## Optimization and measurements

The prior quiet end-to-end baseline records SEND at 40.883416 ms versus clingo at
12.771 ms; two selected corpus cases are faster than clingo under that protocol.
These are measured cases, not a general ranking. See the
[complete comparison](../verification/execution-performance-20260907/README.md).

The [final dependency-build refresh](../verification/dependency-performance-20260907/README.md)
records SEND at 41.021 ms versus clingo at 12.779 ms, queens02 at 105.307 versus
120.946 ms, and task04 at 131.262 versus 194.286 ms. Its eager routes do not
exercise the lazy-window optimization. Separate phase observations identify
SEND rule instantiation and solving as substantial costs; queens and task
allocation are dominated by solving. This current comparison is not an
old-versus-new ablation.

Measure changed expensive operations before attributing benefits. Sparse and
dense lazy-join controls distinguish saved probes from lookup overhead. Compare
complete outputs and work at one and several workers under controlled conditions. Aggregate scheduling may expose later reuse opportunities, but
this tranche does not introduce a cache without snapshot/completion identity.
GPU follow-ons must retain exact residual accounting and physical qualification;
CPU or shader validation alone does not establish device behavior.
The required [full corpus performance matrix](corpus-performance.md) covers all
original non-clingcon cases with eager/lazy and CPU/Metal requests plus clingo.
Its explicit unsupported cells, retained samples and separate phase observations
prevent selective timing claims. Use a qualified build and controlled measurement
conditions.

## Qualification criteria

Each slice requires focused semantic, adversarial, property, resource and
external-oracle checks. Keep the independent workspace/CPU coverage floors at
91%, strict Clippy, formatting, documentation and Lean audit gates. Retain all
94 corpus contracts and 24 selected upstream comparisons, and add unchanged
source cases for newly admitted contexts. Update the README, implementation scope
and contributor instructions before publication. Review public API ownership,
failure semantics and cost independently of passing tests.

The [selectable aggregate-semantics proposal](aggregate-semantics.md) is explicitly
post-1.0. The current tranche retains clingo/Ferraris semantics and records the
assumptions needed to keep future semantic alternatives possible.

The separately accepted [brave/cautious consequence capability](consequences.md)
targets version 1.1. Its streaming and targeted-search plans preserve the original
reduct oracle and explicit complete/partial evidence.

## Required before version 1.0: lazy Metal execution

True lazy grounding with physical Metal execution is a version 1.0 requirement.
The current blanket refusal is an implementation gap. A supported ordinary solve
must retain lazy source generation, perform useful device-side inference or reduct
checking, and preserve complete candidate/source accounting, exact acceptance,
resource stops and honest route reporting. Switching to complete static lowering
is a distinct eager route. Device availability and bounded resource limits remain
legitimate explicit failure conditions.

The next integration analysis must specify a library boundary between bounded
source/binding batches and GPU per-candidate work, including immutable snapshot
identity, atom identity, exhaustion and exact residual completion. Qualify this
route against CPU and an applicable independent oracle, with physical work
evidence. Its supported language profile must be documented; any remaining lazy
refusal needs an identified semantic or implementation reason. Merely removing
the CLI combination check does not meet this criterion.

## Stocktake and next tranche

The [revised coming tranche](aggregate-bounds-lazy-metal-20260907.md) records the
concrete language, CPU, GPU and tooling sequence after this checkpoint, including
the aggregate-bound and head-aggregate priorities and directive exclusions.

After this tranche is integrated and qualified, review language admission,
measured performance, lazy/Metal execution, public APIs, code clarity and the
remaining version 1.0 requirements before selecting new implementation slices.
The following criteria guide technical priorities:

| Area | Next-slice selection criterion |
| --- | --- |
| Clingo language support | A documented native admission gap with ready shared foundations, semantic preservation checks and independent clingo regressions. |
| CPU hot paths | Measured grounding, candidate, propagation or reduct work whose cost can be reduced generally, including justified Rayon scheduling. |
| GPU hot paths | Useful improvements to batching, residency, transfer or device computation, including the required true lazy-Metal route. |
| Code and documentation clarity | Inspectable semantic contracts, reusable library boundaries, decomposed control flow and accurate collaborator-facing documentation. |

The next language priorities include general function head aggregates and broader
choice bounds, alongside the ready aggregate-argument consumer slice. Head
aggregate work must extend the current restricted count-head profile with exact
tuple identity, eligibility and head-support semantics; other aggregate functions
must not be treated as simple atom cardinalities.

Choice bounds are also an optimization candidate because sound propagation can
exclude candidate extensions before complete model construction. Measure the
bounds already exploited by the current lowering/search path before attributing
new benefits: admitted integer bounds already become guarded count constraints
and threshold formulas used in candidate propagation. Independent aggregate
assignments supplying integer choice bounds are one possible bounded language
extension. Any stronger propagation must preserve conditional eligibility,
partial-assignment completion coverage, complete model sets and optimal ties.
Bounds can rule out candidates; they do not provide support for selected atoms
or replace the original-program and frozen-reduct acceptance checks. Record
candidate/work reductions and whole-solve CPU/GPU costs separately from language
admission gains. These are priorities and proof obligations, not new implemented
capabilities or measured speedups.

`#heuristic` and `#edge` are deliberate project exclusions. Earlier optional or
deferred implementation proposals are superseded; inputs using either directive
remain explicitly refused. See the current
[directive scope](../verification/directive-admission.md).

Shared source and ground representations need coherent contracts before dependent
consumers can use them. Every slice includes relevant proof obligations, resource handling,
semantic tests and qualification. None may substitute a different acceptance
algorithm for the original reduct contract without an explicit preservation law.

The structure does not exclude cross-cutting refinements. Program analysis,
semantics-preserving transformations, execution representations and reusable
validation may justify work when evidence identifies a concrete limitation or
architectural simplification. These priorities do not imply that current coverage,
CI or physical-device evidence automatically qualifies future changes.

Formal verification is a continuing responsibility across every track. Keep
Lean statements, assumptions, source correspondences and limitations current with
each semantic change; require the pinned build, complete axiom audit and sealed
proof record at each checkpoint. Preserve previous evidence instead of silently
reattributing it to changed code. Harden executable representations and their
refinement obligations where feasible, challenge missing premises with
counterexamples, and record which Rust/shader correspondences remain unproved.
Theorem counts summarize an inventory; they do not measure implementation
correctness or replace semantic, resource and physical-backend qualification.
