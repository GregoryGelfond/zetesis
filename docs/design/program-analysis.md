# Structural analysis and reduct execution

zetesis consumes the pinned themelios analysis tier for facts about source
programs. The solver owns the decision to use a fact. This preserves the estate's
foundation boundary: themelios parses, represents and analyzes the program;
zetesis constructs candidates and checks stable-model semantics through the reduct.

The typed themelios `Program` is the common source representation before either
grounding or solving. Analysis must preserve its identity, parts and provenance.
The separate [domain-analysis crate](domain-analysis.md) follows that boundary
without depending on the solver; the public typed admission bridge remains
tracked in [themelios-solve integration](themelios-solve-integration.md).
The complementary [demand plan](demand-and-magic-sets.md) treats `#show` as an
implicit observation query when no explicit query is supplied, with separate
whole-program completion obligations.
Pre-grounding [source transformations](source-transformations.md) reuse
themelios's existing `Program -> Program` machinery and require separate
semantic preservation evidence.

## Upstream interface

At revision `87c11a3f2b72b81a12fd53226941fdf95e7294d3`,
`themelios-program` exposes structural queries including a rule's variables,
groundness, head signatures and body signatures. Body dependencies distinguish
positive, negative and aggregate occurrences, including dependencies in head
element conditions. The companion `themelios-analysis` crate assembles these
facts with `Analysis::of(&Program)` into:

- Construct inventory and witnesses.
- Predicate dependency graphs, projections and strongly connected components.
- Safety and grounding-finiteness information.
- Program-class verdicts, including their witnesses and uncertainty.

These are source-level facts. They are not a ground program, a grounding-size
prediction, a countermodel certificate or a stable-model witness. In particular,
failure to prove a property does not establish its negation.

The [grounding-selection plan](grounding-selection.md) maps these exact pinned
APIs to grounder eligibility, implementation availability and cost preference.
There is no upstream lazy/eager class. In particular, source normality, tightness
or stratification alone does not establish an implemented materialization route.
The plan also records the required public preparation boundary before grounding.

The pinned safety implementation explicitly describes its binding rules as
ASP-Core-2 safety. Its `collect_guards`/`require_guard` routines place aggregate
guard variables in the required set; they do not make those variables aggregate
assignment binders. For example, `n(N) :- N = #sum {}.` is a clingo assignment,
but the upstream analysis reports an unbound variable. zetesis must expose that
verdict without relabeling it and validate its admitted clingo assignment scope
separately. An upstream safety refusal is therefore not an unconditional solver
admission gate for this extension. This difference is covered by an integration
regression; it is not a change to themelios or evidence of broader source support.

## Bounded integration

The assembled upstream analysis calls `unpool` internally. A source-byte ceiling
alone does not adequately bound a Cartesian pool expansion. The zetesis adapter
therefore analyzes a retained, normalized program only after bounded fact
expansion and an explicit structural check that the remaining representation is
pool-free. The adapter preflights visited nodes and the potential head/body
dependency-edge product before constructing the assembled analysis. Signature
and edge-endpoint name payloads consume the scalar-byte allowance; this is
conservative logical accounting, not an allocator or process-RSS measurement.

Original carriers and provenance remain available. The public analysis accessor
must identify the exact normalized program analyzed; it must not imply that the
result describes a different input, a partial carrier, or source constructs
removed without justification. Includes and constants must be resolved under
the existing original-source bundle contract before analysis.

The source byte, syntax, scalar expansion, formula work, generated-value,
relation, and support-round limits remain independent. A favorable finiteness
verdict does not disable them. Resource exhaustion refuses admission or leaves
search incomplete; it is never a semantic answer.

An isolated upstream finiteness `Holds` is a certificate only together with
upstream `is_safe()`. Locally admitted clingo binders reported unsafe obtain no
finiteness certificate from that verdict. Their actual finite construction must
still complete under zetesis's explicit coverage and resource obligations.

## First consumer: aggregate-value dependencies

Aggregate assignment generates possible values from eligible full tuples.
Ignoring correlations can add values to this upper bound. That is safe for
candidate coverage only when every emitted assignment still carries its exact
aggregate formula and the completed carrier covers all actual values.

Objective priority presence has an additional observable contract. An arbitrary
objective observer can distinguish extra generated values even when they never
occur in an answer set. A restricted observer policy can instead recognize a
total count/sum or numeric min/max assignment structurally and permit fresh
generated outputs used as objective weights or tuple members. A total assignment
can also consume another generated predicate through a single unfiltered
positive aggregate-element condition with distinct variables in generated
positions. Ordinary producer joins and value-filtered or shared output positions
require stronger analysis. The adapter consults upstream dependency edges rather than
maintaining a second graph with potentially different head-condition semantics.

The policy follows dependency edges from every predicate read by an objective.
Restrictions apply throughout that closure, including head-element conditions;
unrelated logical producers retain their ordinary exact formula semantics.
Constraints still constrain accepted stable models and cannot create objective
slots. Only possible bindings with numeric objective weights establish a slot.
During evaluation, nonnumeric weights contribute nothing while the established
priority vector remains fixed. This also handles an empty min/max result without
coercing its sentinel to an integer.

The rule is about argument positions, bindings and dependencies. Predicate names,
file names and domain labels have no role. Safety classification remains separate
from the policy's supported fragment and from the grounding coverage obligation.

## Further execution decisions

Performance planning separates semantic eligibility from estimated cost. A
themelios verdict or zetesis structural check can establish that a transform is
permitted; observed relation sizes, selectivity, frontier density and device
limits then determine whether it is likely to help. A poor cost estimate may
slow execution, but must not change the accepted answer sets. `Unknown` may
participate in a heuristic estimate; it cannot satisfy a semantic precondition.

| Evidence | Candidate execution choice | Required boundary |
| --- | --- | --- |
| Predicate components and signed/aggregate dependency edges from themelios | Schedule nonrecursive components once, iterate recursive components, batch independent work | Completed input interfaces and an exact component composition contract; an SCC is not automatically an independent solve |
| Normality/Horn classification, plus supported source-to-rule lowering | Use least-closure membership for the admitted reduct fragment | Exact normal-rule reduct correspondence; source class membership alone does not admit unsupported constructs |
| Positive acyclicity/tightness verdict | Consider a cheaper foundedness/minimality check | A theorem for the precise supported choice/aggregate fragment; do not apply normal-program results indiscriminately |
| Scoped variables and checked head/body bindings | Index selective joins and factor independent existential components | Preserve local scope, complete possible support and original/frozen-reduct truth; zetesis currently performs these additional structural checks |
| Successful scalar comparison checks on the exact same complete binding | Reuse those checks when testing the completed row | Expressions and binding are unchanged; deferred checks remain required, and other checks retain their order |
| Completed objective key eligibility and a verified incumbent | Restrict candidate cost while retaining every optimal tie | Exact global tuple coalescing, lexicographic comparison and dominance coverage; original reduct remains unchanged |
| Lowered shape plus measured frontier density and batch size | Select sparse CPU traversal or dense GPU transforms | Equal exact operator results, device capability checks and explicit transport/synchronization costs |

The table is a planning contract, not a claim that every row is implemented.
The current source adapter consumes upstream dependencies for objective
admissibility and implements its own scoped join/factorization checks. Candidate
objective bounds are implemented separately. SCC scheduling, class-directed
minimality shortcuts and general Ferraris GPU execution remain further work.
Benchmark records should identify the selected plan, its applicability evidence
and the phase costs; a favorable class verdict is not a measured speedup.

Dependency components can guide source scheduling and identify work to batch.
They do not make components independent: consequences must cross component
boundaries, and negative or aggregate dependencies retain their candidate-frozen
semantics. A future component execution plan must establish the corresponding
composition theorem before using local completion as global acceptance.

A proven class property can justify considering a specialized reduct procedure.
It cannot alone establish that the implementation supports every construct in
that class. A closure plan still needs the normal-program translation contract;
general formulas still need absence of a proper-subset reduct model. `Unknown`
selects no semantic shortcut. Hardware eligibility additionally depends on the
lowered representation and actual device limits, not just a source class.

Each semantic optimization has two obligations:

1. The analyzed source and lowered representation satisfy the transformation's
   hypotheses, with a recorded witness or explicit check.
2. Under those hypotheses, the transformed reduct procedure accepts exactly the
   same stable models, including complete enumeration and objective contracts.

Lean can express and prove the second obligation and the abstract facts needed
for the first. Existing proofs do not establish Rust, themelios analysis, source
lowering or shader refinement. Differential and property tests supply separate
implementation evidence. This integration also exercises themelios as a real
consumer without changing its revision or modifying its repository.

## Reusing a completed binding's comparisons

The denotational obligation is small and exact. For one immutable complete
binding `b`, let `C` contain only checks actually evaluated successfully to true.
Under `∀ i ∈ C, Check_i(b)`, testing every check is equivalent to testing every
check outside `C`. Merely knowing that an expression's variables are ready is
insufficient. Undefined or deferred evaluation supplies no reusable certificate.
The binding and expression context must be the same; a successful check on a
different relational row or before a generated slot changes supplies no evidence.

This preserves the admitted binding relation. If the subsequent compiler emits
the same rule instances and roots, original and frozen-reduct truth are unchanged.
The Lean `FilterT`/`Lifted.Filter` definitions and filter/composition congruence
laws provide the existing denotational setting. They do not verify a concrete
Rust validity bit, iterator undo, scalar error handling or resource accounting.

Skipping only known successful evaluations must retain the order of the remaining
checks and their located failures. Reuse also must not bypass the surrounding
work, storage or cancellation boundaries. It can spend less work and therefore
complete an input that previously reached a budget; it cannot turn an unresolved
check into a logical success. Tests must exercise row changes, backtracking,
generated bindings, empty relations, component placeholders and deferred errors
in addition to comparing completed formula identities.
