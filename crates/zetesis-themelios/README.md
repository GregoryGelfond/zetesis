# zetesis-themelios

This crate admits original ASP source into zetesis through the pinned themelios
parser and owned program representation. It preserves source text, identities
and locations, and returns typed logical objects for other libraries to use.
Admission does not invoke clingo or establish that an answer set exists.

Start with the [library guide](../../docs/book/rust/libraries.md) and
[grounding chapter](../../docs/book/architecture/grounding.md). The
[implementation map](../../docs/book/architecture/source-pipeline.md) locates
source lowering, binding plans, support completion and formula emission. The
[public exports](src/lib.rs) and their rustdoc specify ownership, limits and
errors; build the reference with:

```sh
cargo doc --locked -p zetesis-themelios --no-deps --open
```

## Choose an admission boundary

| Entry point | Result and intended use |
|---|---|
| `ParsedSource` | One owned parse with consuming admission attempts; eligible retries retain the original source and parse. |
| `admit` | Relational templates for the strict normal-rule profile. |
| `admit_extended`, `admit_bundle_extended` | Relational templates with bounded scalar expansion and source metadata, with the expansion charges each admission accepted under its `ExpansionLimits`. Suitable input for lazy relational solving. |
| `prepare_formula`, `prepare_bundle_formula` | An owned preparation that separates source preparation from eager formula grounding. |
| `admit_formula`, `admit_bundle_formula` | A complete finite Ferraris theory, dense original-atom mapping, lifted objectives and source metadata. Composes preparation and grounding. Expansion is charged under the same `ExpansionLimits`; the usage is not reported. |

The strict profile contains normal rules, constraints, singleton unconditioned
choices, closed logical values, positive/default-negated atoms and
equality/disequality. Extended admission adds acyclic constants, checked ground
arithmetic, finite fact pools/intervals, signature or empty `#show`, `#defined`
and original source bundles. Formula admission provides the broader profile below.

A prepared formula exposes source evidence and analysis before consuming
`ground()` or `ground_with_observer(...)`. Materialization resumes the same
budgets; preparation can succeed before grounding encounters an unsupported
dependency, arithmetic failure or resource limit. See [preparation](src/formula.rs)
and [observer contracts](src/grounding_observer.rs).

These APIs deliberately remain distinct. A caller may use
`ExpansionFailure::needs_formula_admission()` to recognize an eligible retry
through the richer boundary. Syntax errors, undefined arithmetic, overflow and
exhausted limits are failures, not reasons to retry with weaker checks.
The command-line adapter makes this selection automatically.
For an owned retry without copying or reparsing the original input, use
`ParsedSource` as shown in the [source-admission example](../../docs/book/rust/source.md).

The pre-1.0 Rust API removes the retired `ProfileFeature` variants
`StrongNegation`, `ObjectiveNegativeDependency`, `ObjectiveDisjunctionDependency`,
`ObjectiveConditionalDependency` and `ObjectiveAggregateDependency`. Admission
no longer produced these values; callers that constructed or matched them must
remove those references. This removal changes the source API, with no change to
admitted source behavior or the remaining typed refusals.

## Finite formula language

Formula admission validates source scopes, completes a bounded possible-positive
relation, then emits a finite theory from complete joins. Positive support is an
upper bound on possible atoms; it never substitutes for truth in an answer set.
A final unchanged support round is required. Default negation, recursive
eligibility and aggregate equalities remain in the original formulas and reduct.

This is eager formula grounding. It is distinct from the candidate-specific
source joins used by relational lazy execution.

Possible atoms have one authoritative catalog per predicate. Catalogs and
bound-column postings persist across growth rounds. Immutable snapshots borrow
them, retaining typed column equality and stable row identities. The complete
tuple matcher still checks every offered row. Snapshot preparation is per
predicate; new tuples extend the catalog and postings after the snapshot drops.
Certified positive producers use disjoint joins containing newly derived rows;
other producers retain complete round traversal. Final formula emission still
validates every authored body instance.

Whole normalized positive-flat programs prepare a borrowed occurrence plan
using the existing signed dependency graph and SCC order. The plan preserves
every original IR occurrence and caches its positive inputs. Bootstrap handles
zero-input producers; subsequent rounds select affected producers through signed
reverse postings and a packed active set, retaining their original order.
Completed publication precedes every new round. Optional domain guards share the
same source applicability check. This is support preparation, not an answer-set
membership or unique-model certificate; richer source retains its existing
traversal and final authored-error validation.

`FormulaLimits::max_support_bytes` bounds retained catalog, Atom vector cells,
postings, producer-plan and wake-set, snapshot and query capacity, including construction scratch. Nested
atom/value payloads, allocator/tree overhead and unrelated grounding state have
separate bounds. Construction and lookup consume grounding work; this ceiling
is not a process-memory measurement.

Prepared formula and bundle values can additionally request
`with_domain_analysis(Some(DomainLimits { .. }))`, disabled by default. The
analysis requires the exact normalized whole positive flat program;
unsupported profiles, Unknown and Stopped retain complete fallback. Where it
prepares candidates, a rule's variable is narrowed to the meet of its
argument domains less the values a comparison over that variable alone
excludes, decided before any row is read, and the guards apply in every
support-completion round and in the final instantiation, so a row a guard
rejects has no continuation. It does not touch dependency projections, and
preserves authored-error validation. Whether or not the analysis runs, a
comparison over relationally bound variables that is defined and false
excludes its substitution, the join criterion places bound occurrences and
decided comparisons before generators, and a constraint over a keyed value
is asked as the one atom its key admits, counted by `keyed_constraints` on
the admitted value; a key with an anonymous position is left as written.
Analyzer logical populations and bounded standard allocations are separate from
named support/guard byte accounting; actual analysis and guard work consume the
original cumulative formula budget. No new cancellation, allocator or RSS cap
is implied. `DomainObservation` and grounding counters report the actual attempt.
See [optional domain guards](../../docs/book/rust/finite-tables.md#optional-argument-domain-guards)
for applicability, accounting, observation and proof boundaries.

### Values and bindings

Logical values include integers, symbols, strings, closed functions, tuples and
the extremal terms `#inf` and `#sup`, including ordinary atom arguments. Extrema
retain their own types and ASP term order; they are neither integers nor the
strings `"#inf"` and `"#sup"`. They supply no numeric objective weight or priority.
Predicate strong negation, signed function constructors and numeric negation
remain separate. Functions such as `f(1,g(2))`, signed constructors such as
`-f(1)`, and tuples such as `()`, `(1,)` and `(1,2)` retain structural identity.
Finite constructed values can be produced from bound inputs; the grounder does
not generate a universal set of constructors or subterms.

Positive atom patterns can bind components, including repeated variables and
anonymous positions. Evaluated positions consume independently established
inputs: `q(X):-p(X,X+1).` can check a captured `X`; it does not invert arithmetic.
Unbound evaluated inputs receive a distinct `UnboundArgumentInput` refusal.
A failed pattern cannot leak partial bindings into another row.

Dependency-ordered scalar equalities, finite interval cursors and admitted
closed integer comparison bounds can generate values. Source order does not
establish safety. Already-bound equalities remain filters. Positive and
double-negated affine chains can bind several variables through directed finite
endpoints, including `0<X<Y<3`, `0<X=Y<3`, `0<2*X<3*Y<7`, and
`0<X+Y<3,0<Y<3`. They use integer addition/subtraction and multiplication by
closed integer expressions. The complete original guard filters each proposed
binding, so a finite envelope never grants independence to correlated variables.

This analysis runs only after established generators stall and reuses their
scoped range cursor. Single negation, unanchored systems and nonlinear or
division expressions supply no new domains; independently bound expressions
keep their existing scalar checks. Positive relational bindings alone do not
make comparison endpoints closed. Normalized coefficients and constants must
fit a signed 64-bit integer; endpoint accumulation must fit a signed 128-bit
integer. An affine refinement exceeding either analysis capacity may be skipped
when another sound finite envelope supplies the required values. The complete
original guard still runs on those values, including its checked arithmetic.
If generation requires the unavailable refinement, admission returns a located
`FormulaFailure::Limit`, distinct from source arithmetic failure.
The [finite-chain contracts](tests/finite_chains.rs) check
correlation, local scopes, source order, frozen reducts and resource boundaries.
Finite pools distribute through constructors and checked expressions. Ordinary
head/body/guard occurrences produce complete rule products; choice, aggregate
and objective selections remain within their original local group. Nested
intervals use scoped data slots and the same checked expression evaluator.
Selections cannot supply another alternative's missing binding. The
[finite occurrence guide](../../proofs/guide/finite-occurrences.md) records these
separate composition laws and their bounded resource contracts.

Arithmetic uses checked `i32` operations. Undefined or overflowing evaluation
refuses admission instead of silently dropping a substitution. Descending or
nonnumeric interval endpoints yield no rows in facts and generated bindings.
For example, `p(a..b).` contributes no fact; `p(a..(1/0)).` remains an
evaluation failure. Both endpoints are checked before an empty range is
selected. Numeric `i32::MIN`/`i32::MAX` extrema tuple values
and bounds retain an internal guard. That guard is a zetesis limitation, separate
from themelios syntax/raising diagnostics and from intentional language exclusions.

### Rules, choices and conditionals

Normal and disjunctive heads retain their original implication semantics; no
disjunction shifting is used. Singleton and disjunctive heads admit positive,
default-negated and double-negated atoms and Boolean constants. Boolean constants
create no atoms or support. Complete sibling and body validation precedes
simplification, so a true head cannot hide unsafe source or exhausted limits.

Conditional disjuncts admit finite local conditions and independent local
bindings. Each completed `H:C` contributes `(C → H) ∧ not not C` to the head
disjunction; an empty family contributes false. The implication remains in the
frozen reduct. Positive head support requires both the outer body and that
occurrence's condition. Default-negated, double-negated and Boolean heads retain
their own truth without acquiring positive producer support. The [conditional-head guide](../../proofs/guide/conditional-heads.md)
states the original and frozen laws and the finite-scope premises.

Ordinary choices and all five function heads admit atomic and Boolean operands
with `not` and `not not`.
Default-negated operands retain candidate-frozen truth and supply no positive
producer support. Ordinary atomic contributions use the sign and complete atom
as their key; Boolean contributions retain their written source occurrences.
Choice-head intervals expand within one group; disjunctive intervals expand
whole rule instances.

Finite conditional choices retain each head's eligibility, with bounds acting
as constraints rather than support. Universal body conditionals retain
condition-to-consequent implications, conjoined over complete condition rows.
Consequent alternatives may use admitted pools, intervals and local positive
structural/evaluated witnesses. These witnesses cannot bind outer variables or
establish condition safety. Only exhaustive enumeration can establish vacuity.
Negative anonymous consequents project complete matching atoms before applying
`not` or `not not`. Multiple anonymous positions and descendants of positive
constructors and tuples share that contract; arithmetic consumes independently
bound inputs before matching. Each admitted source alternative retains its own
projection, separate from universal condition rows. Comparison consequents also
disjoin their finite value alternatives before the universal condition family;
they cannot bind a missing source name. Every reached alternative is evaluated,
even after a true result. Anonymous inputs inside arithmetic or unary wrappers
remain unsafe.

Strong-negated predicates have distinct identities and coherence constraints:
`not p` never implies `-p`. Under default negation, anonymous projections of
unsigned predicates have an existential matching contract. The clingo-unsafe
`not -p(_)` and `not not -p(_)` forms remain refused.

### Aggregates

Body `#count`, `#sum`, `#sum+`, `#min` and `#max` support finite comparisons
and admitted assignments. Complete tuple keys are deduplicated by OR-coalescing
eligibility; matching first tuple values alone do not identify a tuple.
Body sums ignore empty/nonnumeric-weight tuples; `#sum+` also ignores negative
weights. Source variable safety is still checked.

Ordinary choices and finite count/sum/sum+ comparisons accept complete logical
bounds from the existing closed-expression or completed outer-binding profile.
Bounds use ASP term order without coercion: `#inf` is below every integer;
symbols, strings, constructors, tuples and `#sup` are above every integer.
Thus `{a}word.` permits both the empty answer and `{a}`, while `word{a}.` has
no answer set. Each comparison keeps its own default-negation scope. A bound
cannot provide atom support or remove element validation, including weight,
safety and resource checks. Existing head-weight refusals remain unchanged.

The logical comparison is constant over every finite active-tuple subset, so
its canonical aggregate formula has constant original and frozen truth. It
charges formula work and uses no aggregate state/subset enumeration; ordinary
numeric thresholds retain their existing translation. Optional count planning
omits any group with a nonnumeric bound from its numeric premise certificates.
The [logical-bound contracts](tests/logical_bounds.rs) check these boundaries
against independently declared answers and canonical frozen formulas.

Aggregate assignments with acyclic dependencies within each rule may supply later scalar/range instructions,
nonbinding aggregate guards, normal/choice/function-head values and admitted
body conditionals. Generated proposals retain every original aggregate equality;
an attainable proposal is not evidence that a particular model realizes it.
Self-dependent or mutually dependent value slots within a rule and unsupported
local consumers remain typed refusals. Predicate recursion is admitted when the
possible-support iteration completes within its resource limits. Body extrema use ASP term order, including real `#inf`/`#sup` values
and structural terms, subject to the numeric endpoint guard above.

Count heads separate permission to select an unsigned head atom from
complete-tuple activity. A tuple is active if any row has a true signed head
operand and satisfies its own eligibility. Consequently:

```asp
1#count{1:a;1:b}1.  % {a}, {b}, {a,b}
2#count{1:a;2:a}2.  % {a}
1#count{1:a;2:a}1.  % no answer set
```

Each line is an independent program. One head may activate several tuples, and
several heads may activate one tuple. Local positive binders and admitted
positive/default/double-negated eligibility retain their own scopes.

Weighted heads use the same separate atom permission and complete-tuple activity.
`#sum` measures signed numeric weights; head `#sum+` measures only strictly
positive numeric weights while retaining permission for every numeric weight. A
selected complete tuple contributes once, even when several of its head operands
are true. Distinct tuples sharing a head still contribute
separately: `3#sum{1:a;2:a}3.` admits `{a}`. A zero weight still permits an
unsigned atomic head. Extrema use the same complete-tuple activity and retain
the first tuple value in full, including symbols, strings, constructors and
logical extrema. Values are compared in ASP term order. Atoms may occur under
several tuples and several operands may share one tuple.
The optional count specialization requires a stronger tuple/atom bijection and
wholly unsigned atomic groups. Missing first values and nonnumeric sum weights
have weight zero while retaining independent head permission. Unbounded heads
have no measure constraint; their source terms and bindings are still validated.
A bounded extremum projects first values of nonempty tuples; if none remain,
its measure is the ordinary empty extremum. Empty tuples retain their independent
head permission. This declared extension conserves complete-value extrema and
differs from clingo on some original sources. Undefined arithmetic remains a
typed evaluation failure.
The [head contribution contract](../../proofs/guide/head-contributions.md) records
the formal source basis and explicit clingo comparison differences.
Empty minima and maxima are `#sup` and `#inf`; bounds never create support.
Objective-relevant count, sum, sum+ and extremum heads use their independent
positive atom permissions as possible producers. Their measure and bound decide
original-model truth; they do not remove possible support.

### Objectives

Formula results expose lifted objectives, origins and declarations separately
from the logical theory. `zetesis-objective` scores supplied verified models;
objectives do not derive atoms or replace reduct acceptance.

The source profile admits `#minimize`, `#maximize` and weak constraints
with finite safely bound weight, priority and tuple expressions, positive
ordinary conditions and scalar
equality/disequality filters. Maximize weights undergo checked negation before
global `(priority, normalized weight, full tuple)` deduplication. Costs use the
normalized minimization sign. Weight, priority and tuple are resolved from the
same eligible binding; specialization retains the original model query.
Runtime objective templates always have fixed integer priorities.

Resolved nonnumeric weights or priorities, including `#inf`/`#sup`,
contribute neither cost nor priority. Their source syntax, tuple shape, variable scope, filter and resource checks
still run. Resolved tuple expressions are evaluated for numeric contributing
rows; source exclusion retains its defined field-evaluation order. A numeric zero retains a priority; an omitted weight
does not. Eligible unrepresentable maximize negation is a located failure;
an excluded nonnumeric priority needs no weight normalization. Undefined priority
arithmetic remains a located source-evaluation failure. Weight and tuple
expressions reuse that same scalar evaluator and are specialized after source
eligibility selects a complete binding. Arithmetic, structural constructors and
logical extrema retain typed value identity. Simple fields retain lifted joins;
resolved fields feed the same global key, score and candidate-bound operations.
Optimization directives share finite scalar binders and structural patterns
where their grammar permits them. Weak bodies additionally use the admitted
finite aggregate and conditional scopes,
including aggregate equality proposals, independently bound local variables,
scalar binders and structural positive patterns. Routing selects the required
scope capability before compilation; an unrelated aggregate does not change
whether a scalar binder is admitted. The positive relational case retains its
lifted path. Unsafe outer fields cannot acquire safety from a local condition.

Scoped objectives reuse the rule-body compiler in an isolated value domain and
the existing aggregate/conditional formula operations over completed support.
Only original-model truth is translated into the objective query: formula
implication becomes Boolean disjunction with a negated antecedent. This is not
a valid general Ferraris reduct rewrite and never changes the original theory.
Scratch formula IDs map through their own typed atom catalog; objective-only
atoms and values cannot enter the original universe. Backward references let the
translator retain only the chosen root's ancestors in topological order.

`max_objective_formula_atoms` and `max_objective_formula_nodes` bound each
transient body formula independently of final theory storage and retained
`objective.max_condition_nodes`. Existing per-aggregate operation limits and
cumulative source work, substitution and scalar allowances also apply; aggregate
failures retain their typed partial accounting. Required scoped data evaluation
runs on each completed outer row before activity exclusion and numeric selection, even for ignored rows. Only a retained numeric
row creates Condition nodes. Source shape checks are shared with objective
admission; scope and assignment safety remain with the source compiler.
Scoped pools share one bounded dependency projection for analysis. It retains
all signed predicate dependencies and all selected weight, priority and tuple
fields; `AnalysisBasis::DependencyProjection` identifies this limited purpose.
The runtime conditional still disjoins source alternatives inside each local
condition row. Neither that analysis projection nor a synthetic producer enters
the objective query. Ordinary weak-body and guard pools produce complete body
products; local aggregate/conditional scopes and objective fields expand inside
their own collection. Nested interval fields reuse scoped data generation.
Complete weight/priority/tuple keys coalesce by the objective's existing rule.
The [objective pool contracts](tests/objective_pools.rs) retain located safety and
resource refusals; the independent pool-free analysis preflight prevents
unbounded upstream unpooling.

Objective conditions admit default negation, double negation, Boolean truth and
bounded scalar comparisons on independently bound values. They lower into closed
queries over original typed model atoms and do not add theory roots or support.

Source eligibility selects an explicit precision plan per objective and uses
the same activity analysis for projection declarations. Acyclic producers are
evaluated in dependency order. Rich bodies reuse the existing scoped aggregate,
conditional and projection lowering; a three-state fold computes absent,
optional or required activity from their original Boolean operations. It does
not introduce a second aggregate evaluator or solve correlations between
optional literals. Positive aggregate heads supply possible eligibility
independently of their measured contribution.

Cyclic producer cones and their unresolved dependants start from completed
possible support: covered atoms are optional and atoms outside the carrier are
absent. Each complete round aggregates every producer before resolving optional
entries, and cannot retract established information. This finite refinement can
recognize facts in cycles and constant rich bodies. Heads outside its profile
retain the conservative carrier. Independently absent producers contribute no
redundant zero-cost priority slot; retained objective queries still evaluate the
original model and can produce zero costs. Eligibility accepts only an immutable
view owned by a successfully completed support construction. This owner is
created after an entire round
adds no atom, never after a round, work, value or storage limit is reached.
A source can generate new values indefinitely; exceeding a limit returns a typed
failure rather than a program with partial objective coverage. For example,
`n(0).n(N):-N=#count{X:n(X)}.` keeps generating new count values.
A weak constraint's own query uses original-model formula truth, independently
of the producer coverage certificate.

The coverage argument depends on complete typed rule generation, not simply on
observing an empty delta. Positive joins retain full bindings; nonbinding
aggregate and conditional truth never filters possible producers. Assignment
proposals include every aggregate value from the complete eligible tuple-key
carrier, including logical extrema and the empty-set value. Original aggregate
equalities remain in the theory. Under these source-profile premises,
intersecting a model with a producer-closed carrier preserves its frozen reduct;
answer-set minimality therefore excludes atoms outside that carrier.
[SourceSupport](../../proofs/Zetesis/SourceSupport.lean) makes that projection
premise explicit and proves the minimality consequence. It does not prove Rust
binding-plan refinement, resource completion, or termination for every source.

The completed activity is a source grounding abstraction: absent, optional or
required. Optional means retained as a grounding possibility, not simultaneously
satisfiable or realized in an answer. Thus `{a}.#minimize{1:a,not a}.` retains
its zero-valued priority even though the complete conjunctive model query is
always false. In contrast, `a.#minimize{1:not a}.` has no retained objective row.
Choice bounds and constraints never turn optional source atoms into required
ones. This coverage contract can retain additional always-zero priority slots;
it does not promise exact reproduction of another grounder's simplification
metadata. For example, `a.a|b.#minimize{1:b}.` retains priority zero with cost zero,
where clingo 5.8.2 omits the objective vector. Both return the same sole answer
`{a}`. Missing slots are zero in cost comparison, so additional identically-zero
slots preserve every ordering and optimal tie. Derived-fact and repeated-disjunct
counterparts have separately recorded raw reference outcomes; they are not
silently normalized in comparisons. Source eligibility visits finite dependency
cones, charges all ground joins and bounds retained activity entries independently of model
evaluation. Producer traversal computes activity directly; only an eligible
objective row constructs a closed model query and consumes its per-template
condition-node ceiling. Dependency storage is checked before each new predicate
is retained, including coexisting precision certificates. Independent objectives
compose their own selected plans; completed activity and aggregate carriers share the
retained-presence ceiling when they coexist. Neither certificate searches for
answer sets or replaces original rules.

Existing structural and flat certificates remain useful precision refinements.
The flat profile has an unconditional unary assignment, closed complete tuples,
and empty or one positive closed fact/unbounded-choice condition per tuple.
Unique unary forwarding shares the refinement. Qualification belongs to the
base producer and each forwarding edge independently: observing a filtered or
multiply produced descendant cannot erase an established base or sibling
carrier. Such descendants retain their separately covered possible support. Required symbolic extrema can
exclude numeric weight proposals without changing support or the original
aggregate equality. Generated fields can use the measures of all complete key
sets `S` with `R ⊆ S ⊆ P`, where `R` is required and `P` is possible; full tuple
identity coalesces required/optional aliases first.

A nonapplicable refinement leaves the complete possible carrier eligible; it
does not certify exact priority metadata or refuse otherwise covered finite
solving. Resource or evaluation failure during an applied refinement remains an
error. Original model queries decide which retained rows contribute. Correlated
source possibilities can remain unrealized and retain zero slots. Required
arithmetic evaluation still fails explicitly, including on such source rows.

Within that completed flat-carrier profile, objective conditions may select a
generated value by a literal, equality/disequality filter, or a repeated variable
shared with another positive condition. Selection applies to whole bindings
before priority evaluation, including fixed-priority objectives. An unrealized
value in the source carrier can retain an inactive priority slot; a proposal
outside the completed carrier cannot. The original aggregate equalities and
model-relative objective conditions remain intact. Selected fixed-priority rows
use the same bounded specialization path as dynamic priorities.
`FormulaLimits::max_objective_presence_entries` conservatively bounds
logical presence-planning and shared source-activity slots, including completed
carrier values, predicate traversal, borrowed scope-frame capacity and
simultaneous old/new activity tables. Projection declarations use this same
existing limit. These are planning slots rather than allocator bytes.
Each transient rich producer or projection-condition validation builder uses
`theory.max_atoms` and `theory.max_nodes` independently, without adding atoms or
roots to the original theory. Actual objective-body scratch instead uses
`max_objective_formula_atoms` and `max_objective_formula_nodes`. Activity folding
does not consume retained `objective.max_condition_nodes`.
Transient numeric subset construction separately
uses `max_assignment_values` and the grounding work bound; it can be exponential.
See [dependency checks](src/formula_objective_dependencies.rs) and
[presence classification](src/formula_objective_dependencies/presence.rs), plus
the shared [source activity](src/formula_source_activity.rs) and separate
[original-model queries](src/formula_source_activity/model_query.rs).

## Sources, analysis and output views

`SourceBundle::load_many` loads ordered original roots with global constants and
retained include occurrences. Includes first use the captured working directory,
then the including file's directory after a relative filesystem lookup failure.
Repeated selected paths are included once after their traversal completes;
encountering a file still on the active include chain is a cycle refusal. A
diamond-shaped graph is therefore admitted, while a cyclic graph is refused.
clingo 5.8.2 instead warns and skips an already included file on a cycle.
Lexical aliases of one canonical
source, include symlink redirections, cycles and unsupported library includes
receive explicit refusals. File, root, byte and depth limits apply to the combined
bundle. Loading is not a filesystem snapshot guarantee.

`source_analysis()` exposes pinned themelios analysis.
`analyzed_program()` is qualified by `AnalysisBasis::NormalizedProgram` or
`DependencyProjection`. Projection preserves dependency information but changes
connectives; it is not a semantic replacement for the original source.
Upstream safety findings remain unchanged, including their narrower treatment
of aggregate assignment binders. Neither an analysis result nor admission alone
authorizes an unsupported execution route.

`#defined` is a declaration. `#show` is a presentation selection, not a demand
query or change to full model identity. Signature selections union; an empty
selection hides ordinary atoms. Term-only shows leave default atom output enabled.
Parameterless `#program base` delimiters are accepted; this is not multi-shot
program-part support.

The formula boundary admits finite term observations over a caller-supplied full
model. Output terms include structural values, checked scalar arithmetic, pools
and integer intervals. Conditions include Boolean values, signed atoms,
comparisons, directed scalar equalities, scoped aggregate guards and assignments,
and universal conditional literals. Positive patterns bind explicit constructor
or tuple positions, including finite structural pools; arithmetic arguments can
read captures from the same matched atom. An alternative exports only captures
available in every branch unless another binder supplies the name.

Observation truth always uses the supplied model. It does not modify the logical
atom catalog, formula theory, grounding domain, objective priorities or stability
claim. Local aggregate and conditional names do not escape their scope. Aggregate
keys retain complete tuple identity; positive cardinality keys come from exact
matched atoms, including sign and arguments. Sums compare checked widened totals;
constructing an assigned numeric value still requires the pinned i32 scalar
range. Empty `#min{}` and `#max{}` return the genuine `#sup` and `#inf` endpoints;
a nonempty extremum element with no measure is refused.

`evaluate`, `render` and `view` return complete results or a located typed error.
Undefined/overflowing expressions, missing bindings, exceeded limits and
cancellation do not return a partial observation. Terms deduplicate only within
the term channel; an equal selected atom and shown term both remain visible.
Distinct original answers retain their hidden identity even when displays agree.
`#project` remains separate from observation. Native observation also permits
anonymous projection of either strong sign under default negation, such as
`#show absent:not -p(_).`; clingo 5.8.2 reports these strongly signed anonymous
forms unsafe. This is an explicit supplied-model observation extension, recorded
separately from compatible comparisons. Widened aggregate comparisons can also
differ from clingo's reported integer wrapping. Neither difference changes the
original theory or claims an upstream implementation defect.

Each completed outer or local substitution is charged before testing its final
conditions. Relational joins may examine the Cartesian product of their finite
model relations and source alternatives; generated value products and nested
queries have independent finite work and storage ceilings. Source structural
pool products are checked before materialization, and their compiled nodes and
text remain charged. Runtime generated alternatives, owned bindings and retained
aggregate keys share `max_local_bytes` (default 8 MiB), measured as 16 bytes per
semantic node plus UTF-8 text. Borrowed model values, container capacity and
allocator overhead are excluded. This semantic budget is not RSS; transient
scope ownership is released separately from retained output payload. See the
[observation API limits](src/observation.rs) and
[observation proof guide](../../proofs/guide/observations.md).

Equality chains (`#show X:X=Y=1.`) and structural capture
(`#show X:f(X)=f(1).`) preserve one complete value across the chain. A single
unresolved occurrence can be recovered through unary minus, addition,
subtraction or multiplication by established operands, as in
`p(2). #show X:p(X+1).`. The complete authored expression is checked after the
candidate is recovered. Nonintegral or out-of-range candidates give no match;
reached arithmetic errors remain errors. A zero multiplier cannot supply a
unique binding. Division, powers, bit operations and multiple unresolved
occurrences are outside this inverse profile.

Anonymous negative cardinality keys are admitted:
`#show N:N={not p(_)}.` counts the existential pattern once when no witness
exists, while `p(1). #show N:N={not not p(_)}.` counts it once when a witness
exists. Multiple witnesses do not multiply that contribution, and equal
concrete alternatives coalesce before counting. External calls, theory
expressions, anonymous constructed values, unsafe free variables and circular
aggregate result dependencies remain outside the admitted profile. The
[binding](tests/observation_bindings.rs), [inverse](tests/observation_inverse.rs)
and [expression](tests/observation_expressions.rs) contracts retain source
witnesses, complete results and located failures.

`SourceMetadata::project_selection()` records authored projection declarations.
Formula grounding completes `AdmittedFormula::projection()` as one immutable,
sorted typed atom domain. Signatures select complete possible-support rows;
conditional declarations select finite instances using the shared source
activity contract. A declaration never adds a logical producer or changes
display selection. Resource failure publishes no partial domain.

Projected enumeration is an explicit library operation over that prepared
domain. It selects full answer-set representatives after objective selection;
ordinary enumeration and `WorldView` retain full answer identity. The CLI
requests projected enumeration when the source contains `#project`. See the
[projected session API and checked example](../../docs/book/rust/sessions.md#projected-enumeration)
for key-history limits, cancellation and the separate completion receipt.

Theory atoms, scripting and Rust `@`-functions are not implemented. `#heuristic`
and `#edge` are excluded from the intended language. Other unsupported directives
and combinations fail explicitly rather than being ignored.

## Optional candidate restrictions

`ground_with_count_plan` requests a separate CountPlan over the original atom
catalog. `NotRequested`, `NoPlan`, `Ready` and `Incomplete` distinguish its
outcomes. Ordinary count-head admission allows aliases; the optional planner
still requires a tuple/atom bijection and canonical-true eligibility, with
restricted activation matching. It safely declines groups that do not qualify.
A planning stop does not invalidate the already admitted theory or prove UNSAT.

`objective_bound::ObjectivePlan` similarly builds candidate-only incumbent
constraints over the original carrier. Callers must establish catalog coverage
and verify the incumbent. Bounds preserve ties; the original theory and reduct
remain the acceptance subject. These plans have independent work/storage limits.
See [count planning](src/formula_count_plan.rs) and
[objective bounds](src/objective_bound.rs).

## Failures and regression tests

Source bytes, syntax traversal, values, variables, support rounds, substitutions,
formula storage, aggregate lowering, objective presence and observations have
separate ceilings. Zero is an actual zero allowance. A failure never returns a
truncated admitted theory or an UNSAT claim. Logical payload budgets do not
promise an exact process-memory bound.

Syntax failures retain original source and typed themelios diagnostics.
`SyntaxFailure::source()` and `diagnostics()` support caller-owned views;
bundle failures retain the source catalog. Located observation errors expose
`Error::diagnostic()` independently of rendering. `Error::retain_source()` can
attach the matching original source and name, copying only that file's bytes;
`diagnostic_source()` and `diagnostic_source_name()` expose that context.
The human view then resolves the retained span without rereading files; typed
cause and evaluation accounting stay unchanged. This diagnostic storage is
separate from evaluation budgets and bounded by the caller's input limits.
Terminal styling belongs to the CLI.

Maintained regressions include [preparation](tests/formula_preparation.rs),
[bindings](tests/structural_bindings.rs),
[Boolean heads](tests/boolean_heads.rs),
[count activity](tests/count_head_activity.rs),
[aggregate consumers](tests/aggregate_consumers.rs),
[objective presence](tests/objective_extrema_presence.rs),
[observations](tests/observations.rs) and
[refusal contracts](tests/refusal_contracts.rs).
Original-source clingo comparisons and independent finite original/frozen checks
are in these test suites. Consult the [contributing guide](../../CONTRIBUTING.md#verification-and-review)
for portable and external-oracle gates.

The [Lean correspondence chapter](../../docs/book/lean/correspondence.md)
separates proved semantic laws from remaining parser, Rust, shader and
resource-accounting refinement obligations.

## Inspect posting selectivity

A test-only [posting diagnostic](src/formula_support/postings.rs) compares a
streaming intersection of bound-column row IDs with independent full-row equality.
It observes the production shortest-posting probe without replacing its result.
The fixed corpus selection includes all six queens encodings, SEND and selected
shortest-path/task-allocation sources:

```sh
cargo test --locked -p zetesis-themelios --lib \
  formula_support::postings::tests::corpus_postings_preserve_full_row_equalities \
  -- --ignored --exact --nocapture --test-threads=1
```

Each JSON record names its source and diagnostic ceilings. `complete` describes
observation coverage; it does not establish answer-set enumeration. Grounding
still uses the ordinary finite formula limits. A diagnostic limit stops recording
without changing admission and makes this test fail after printing the partial
record. Missing relations are excluded from observed probes and remain included
in the existing grounding observer's actual probe count.

Shortest-posting and intersection row counts describe eligible lists at each
probe opening. A join can stop before visiting all of a list, so these counts
are separate from `actual_join_rows`. Repeated queries are scoped to one support
build, predicate, append-only relation size and exact bound values. Unbound
repeated probes do not establish repeated index lookup cost. Stored key bytes
are requested logical payload, excluding allocator overhead and temporary cursor
storage; they are not process RSS. These observations measure eager formula
selectivity, not elapsed time or GPU/lazy performance. Production libraries and
CLI builds contain neither this instrument nor an additional option.


## Formula atom ownership

Emitted formula atoms use the shared core appendable interner. The AVL index
retains only dense positions and metadata; one payload sequence assigns each
distinct atom its first-insertion position. A committed prefix supplies exact contiguous count
capture, and finalization transfers the sequence into the existing immutable
`AtomCatalog`. Possible positive support remains a separate population; catalog
presence never establishes truth, source coverage or answer-set membership.

Lookup, copied scalar descriptors/text, index construction and commit operations
now consume the cumulative formula work ceiling. Exact work cutoffs can therefore
change. `Limits::for_atoms` derives a finite index-capacity envelope from the
applicable atom ceiling; `FormulaResource::AtomStorageBytes` reports that named
storage refusal. Nested payload remains under the existing scalar-byte budget;
neither budget is a process RSS ceiling.

This pre-1.0 source API change removes the old `AtomAllocation` enum and its
hash-table-specific error variant. `FormulaFailure::AtomAllocation` now retains
the original `std::collections::TryReserveError` directly, with the source
location. Configured limits and exhausted work remain distinct typed refusals.
