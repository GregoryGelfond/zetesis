# Objective presence and activation

This is a proposed library boundary for admitting a total aggregate producer
followed by scalar filters on a path to a positive objective. It does not remove
the current `ObjectiveAggregateDependency` refusal. Negative, disjunctive and
conditional dependencies, several producers and dynamic priorities remain
outside the initial scope.

## The distinction the representation must retain

Three different facts affect an objective:

1. A source declaration describes an objective template.
2. Grounding retains a numeric contribution and its priority, even when that
   contribution is inactive in a particular answer set.
3. The contribution's condition holds in a verified answer set, so its weight
   participates in that answer set's score.

An empty active contribution set can therefore have a present zero cost vector.
A numeric zero weight also retains a priority. Conversely, a declaration whose
bindings disappear or whose weights are all nonnumeric need not retain one.
Activation is model-relative; the priority layout is fixed for the completed
grounding contract and shared by every returned model.

Current `formula_ground::objective_active` joins against completed possible
support and checks for a numeric weight. `formula_objective_dependencies`
restricts aggregate producers to total observers for which that boundary has
been qualified. An ordered binding plan alone cannot justify removing this
restriction. In particular, exact aggregate tuple coalescing can erase information
that clingo's observable objective layout retains.

## Reference observations

The [recorded originals](../verification/objective-presence-20260908/originals.json)
retain 15 unchanged sources, complete clingo 5.8.2 JSON output, ground ASPIF
output, commands, source hashes and the external executable hash. These small
reference runs contain 26 full model records. They are not a native admission
campaign or a timing benchmark. A missing JSON `Costs` member remains distinct
from a present `[0]` vector.

The reference uses `--opt-mode=enum` without an objective bound to collect every
stable model and its costs. Its retained `OPTIMUM FOUND` footer does not mean
that every enumerated model is optimal. Independent review checked source
hashes, enumeration completion, full model identity and corresponding ASPIF
priority statements; no native objective admission is qualified by this record.

For each fragment below, observe its `n(N)` atoms with
`#minimize {1@7,N:n(N)}.`:

| Producer | External observation |
| --- | --- |
| `n(N):-N=#count{},N>0.` | No numeric objective statement survives; costs are absent. |
| `n(N):-N=#count{},N=0.` | The model contains `n(0)`; priority 7 has cost 1. |
| `{a}. n(N):-N=#count{1:a},N>0.` | Both models retain priority 7; the model without `a` has cost 0. |
| `{a}. n(N):-N=#count{1:a;2:not a},N=0.` | No answer set contains an `n` atom, but both retain cost `[0]`. |
| `{a}. n(N):-N=#count{1:a;1:not a},N=2.` | The actual count cannot reach 2 after whole-tuple coalescing; both models still retain cost `[0]`. |

The ASPIF output independently retains a priority-7 minimize statement in the
last two examples. Thus removing priorities that contribute zero in every
answer set would change this observable behavior. Conversely, adding every
declared fixed priority would change the first example. These observations
establish the distinction without claiming a reconstruction of gringo's
internal simplification order.

The remaining cases cover absent outer domains, nonnumeric-only weights,
numeric zero weights, duplicate keys across weak constraints and `#minimize`,
canceling distinct keys, a surviving lower priority, a priority active only in
a nonoptimal model, repeated conditions and mutually exclusive support.

## Proposed library responsibilities

Keep the existing `ObjectiveProgram`, `ObjectiveTemplate`, contribution identity
and `Score` vocabulary. Add an explicit completed priority-layout boundary
before extending supported observers; its concrete type placement remains an
API review item.

The layout records ordered numeric priorities independently of model-relative
contributions. A completed empty layout represents absence. An unfinished
layout computation returns a typed bounded failure, never an empty layout.
Construction owns source origins and the supporting grounding evidence. Its
fields do not claim semantic completion merely because a caller can fill them.

Each contribution retains the existing global key: normalized weight, priority
and complete objective tuple. Duplicate occurrences coalesce by disjoining
activation and merging origins across statements. Equal displayed terms or
equal numeric weights alone do not identify the same key. Zero and canceling
weights do not erase layout evidence.

A source compiler must establish separately:

- **Carrier coverage:** all admitted outer rows, aggregate proposals and scalar
  descendants needed by the chosen presence contract were considered.
- **Priority presence:** the completed retained priority set matches the
  supported clingo grounding profile. A possible-support approximation or an
  exact stable-model carrier does not prove this obligation.
- **Activation:** each retained key contributes exactly when its original
  condition holds in the verified model. Producer equalities and scalar filters
  remain conditions; a proposed value does not establish either.
- **Bounded completion:** arithmetic, enumeration, retained storage and origin
  accounting complete within explicit limits; failure publishes no layout.

The numeric evaluator then sums each active global key once using checked
integer arithmetic, and presents costs in the completed priority order. Search
uses the same objective ordering and retains all optimum ties. Objective
evaluation supplies no support and does not replace reduct-based acceptance.
Human, JSON and future ASPIF views consume this common layout and score.

## Implementation decision

First characterize the finite presence carrier required by this restricted
source profile, including information before tuple coalescing where necessary.
Retaining an additional carrier is an implementation option, not yet a proven
equivalence to clingo. It must not create semantic atoms or contribute additional
weight solely to imitate a reported priority.

Only then connect a checked layout producer to the existing objective library
and relax the particular admission refusal it discharges. Reuse the recorded
sources for full model/cost and optimal-tie comparisons, add source-order and
duplicate-template controls, and test every exact/one-short resource boundary.
The current implementation and existing objective tests remain unchanged by
this design record.

Existing objective and head-measure Lean laws support ordering and activation
composition. They do not prove the required presence carrier, clingo layout
correspondence, or concrete Rust grounding. Those remain explicit obligations
for the subsequent implementation slice.
