# Objective presence and activation

Total aggregate producers can feed positive objectives through acyclic ordinary
predicate renamings and argument permutations. The broader completed-layout
boundary below remains a proposal for admitting scalar filters and other
consumers. Negative, disjunctive and conditional dependencies, more general
producer combinations and dynamic priorities retain their separate gaps.

## Implemented total relation renamings

`n(N):-N=#count{}.p(X):-n(X).#minimize{X@7:p(X)}.` is admitted with `n(0)` and
`p(0)` and a present priority-7 zero cost. Each forwarded predicate has one
ordinary rule, one positive body atom, and a bijection between the body's distinct
argument variables and the head's argument variables. This includes renamed
variables, argument permutations and finite chains. The relevant component of
the themelios dependency graph must be acyclic. Constants, repeated variables,
projections, joins, filters, alternative producers and constructed arguments do
not acquire this certificate.

The certificate transports aggregate-generated argument positions before checking
the final observer. An objective cannot evade the original generated-value
restrictions by adding a renaming predicate. The source rule, original aggregate
equality, formula theory, objective tuple, priority and direction remain intact.
No optimization objective supplies support or changes reduct-based acceptance.

The predicate walk and retained position sets are bounded by the existing
structural-analysis allowances. Argument matching uses finite variable sets;
transport costs at most the product of head arity and generated-position count
for each certified rule. This is source classification, not an added execution
representation. The library still exposes grounding independently of solving.

## Mixed-extrema presence boundary

The new adversarial campaign exposed a pre-existing direct-observer defect:

```asp
b. {a}.
n(N) :- N = #max {2:a; foo:b}.
#minimize {X@7:n(X)}.
```

Every answer set contains `n(foo)`. clingo reports no objective cost vector, but
the old possible-support test saw the unrealizable numeric proposal `n(2)` and
reported a present zero slot. Correct full models do not excuse this observable
cost difference. The [recorded counterexample](../verification/objective-forwarding-20260908/direct-extremum-counterexample.json)
retains both installed pre-change and external outputs with executable hashes.

Direct and forwarded observers now return a located
`ObjectiveAggregateDependency` refusal for the unqualified mixed carrier. A
generated argument used as a variable objective weight seeds predicate dependency
reachability. After support completes, each relevant extrema assignment's raw
tuple carrier is inspected separately for each complete outer binding. Both
numeric and nonnumeric first values in that carrier trigger the refusal. Whole
tuple duplicates have the same first-value class, so this inspection needs no
alternate coalescing rule. The implicit empty endpoint is not a tuple; it does
not cause all optional numeric extrema to be refused.

Numeric-only and nonnumeric-only carriers, constant-weight observers, numeric
zero weights, fixed dominating numeric extrema with numeric-only tuples and
different value classes in disjoint outer bindings remain admitted. A mixed
carrier whose numeric value dominates is conservatively refused too. Dependency
reachability also crosses intervening numeric reductions: such a reduction may
remove the concern, but no value-class certificate currently discharges it.
These are internal zetesis implementation gaps, not themelios rejections or
claims that the source is a modeling error.

Preparation records only bounded aggregate IDs; successful preparation does not
claim completed objective presence. The check runs during objective activation
before an objective program is published. It uses the existing complete joins,
work/substitution counters and scalar-copy budgets, with original producer
locations on refusal. It retains two class flags per inspected carrier and no
second tuple store. Repeated assignment proposals currently repeat this bounded
inspection, so its work is visible and can exhaust the caller's allowance.
Separating one outer binding from its proposal enumeration is a future shared
grounding improvement, not a claimed optimization in this slice.

The [forwarding qualification](../verification/objective-forwarding-20260908/README.md)
distinguishes admitted parity, conservative refusal controls and the historical
defect. `ObjectiveTransport` proves finite presence and contribution transport
laws under explicit carrier/activation correspondence. It does not establish
clingo's presence carrier, the Rust recognizer or the broader layout proposal.

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
support and checks for a numeric weight after the structural and mixed-extrema
boundaries above. An ordered binding plan alone cannot justify removing those
restrictions. In particular, exact aggregate tuple coalescing can erase information
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
The total-renaming slice does not implement this broader completed-layout
proposal. Its separate tests and conservative presence boundary are described
above.

Existing objective and head-measure Lean laws support ordering and activation
composition. They do not prove the required presence carrier, clingo layout
correspondence, or concrete Rust grounding. Those remain explicit obligations
for the subsequent implementation slice.
