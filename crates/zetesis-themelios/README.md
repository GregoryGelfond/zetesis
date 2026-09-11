# zetesis-themelios

This crate admits original ASP source into zetesis through the pinned themelios
parser and owned program representation. It preserves source text, identities
and locations, and returns typed logical objects for other libraries to use.
Admission does not invoke clingo or establish that an answer set exists.

Start with the [library guide](../../docs/book/rust/libraries.md) and
[grounding chapter](../../docs/book/architecture/grounding.md). The
[public exports](src/lib.rs) and their rustdoc specify ownership, limits and
errors; build the reference with:

```sh
cargo doc --locked -p zetesis-themelios --no-deps --open
```

## Choose an admission boundary

| Entry point | Result and intended use |
|---|---|
| `admit` | Relational templates for the strict normal-rule profile. |
| `admit_extended`, `admit_bundle_extended` | Relational templates with bounded scalar expansion and source metadata. Suitable input for lazy relational solving. |
| `prepare_formula`, `prepare_bundle_formula` | An owned preparation that separates source preparation from eager formula grounding. |
| `admit_formula`, `admit_bundle_formula` | A complete finite Ferraris theory, dense original-atom mapping, lifted objectives and source metadata. Composes preparation and grounding. |

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

## Finite formula language

Formula admission validates source scopes, completes a bounded possible-positive
relation, then emits a finite theory from complete joins. Positive support is an
upper bound on possible atoms; it never substitutes for truth in an answer set.
A final unchanged support round is required. Default negation, recursive
eligibility and aggregate equalities remain in the original formulas and reduct.

This is eager formula grounding. It is distinct from the candidate-specific
source joins used by relational lazy execution.

Possible atoms have one authoritative catalog. Immutable core relation views
borrow it between growth rounds, retaining typed column equality and original
row order. Bound-column postings use the view's equality IDs; the existing
whole-tuple matcher still checks every offered row. The snapshot drops before
the catalog grows. `FormulaLimits::max_support_bytes` bounds authored snapshot,
membership-index and query capacity, including construction scratch. Source
atoms, allocator/tree overhead and unrelated grounding state have separate
bounds. Snapshot construction and lookup consume grounding work; this byte
ceiling is not a process-memory measurement.

### Values and bindings

Logical values include integers, symbols, strings, closed functions and tuples.
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
integer. Exceeding either fixed
analysis capacity is a located `FormulaFailure::Limit`, distinct from source
arithmetic failure. The [finite-chain contracts](tests/finite_chains.rs) check
correlation, local scopes, source order, frozen reducts and resource boundaries.
Nested pools and broader constructor/interval contexts remain restricted;
admitted consequent alternatives are described below.

Arithmetic uses checked `i32` operations. Undefined or overflowing evaluation
refuses admission instead of silently dropping a substitution. Descending or
nonnumeric interval endpoints yield no rows; an error while evaluating an
endpoint remains a failure. Numeric `i32::MIN`/`i32::MAX` extrema tuple values
and bounds retain an internal guard. That guard is a zetesis limitation, separate
from themelios syntax/raising diagnostics and from intentional language exclusions.

### Rules, choices and conditionals

Normal and disjunctive heads retain their original implication semantics; no
disjunction shifting is used. Singleton and disjunctive heads admit positive,
default-negated and double-negated atoms and Boolean constants. Boolean constants
create no atoms or support. Complete sibling and body validation precedes
simplification, so a true head cannot hide unsafe source or exhausted limits.

Conditional disjuncts are limited to empty or explicitly true Boolean conditions.
General conditional disjunction remains unsupported. Ordinary choices and all
five function heads admit atomic and Boolean operands with `not` and `not not`.
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
projection, separate from universal condition rows. Anonymous inputs inside
arithmetic or unary wrappers remain unsafe; broader nested pools remain
unsupported.

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

Acyclic aggregate assignments may supply later scalar/range instructions,
nonbinding aggregate guards, normal/choice/function-head values and admitted
body conditionals. Generated proposals retain every original aggregate equality;
an attainable proposal is not evidence that a particular model realizes it.
Self-dependent/cyclic generators and unsupported local consumers remain typed
refusals. Body extrema use ASP term order, including real `#inf`/`#sup` values
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
A bounded extremum with a missing first value has no established total measure
and retains the named `ProfileFeature::HeadAggregateMissingValue` refusal.
Undefined arithmetic remains a typed evaluation failure. The obsolete broader
`ProfileFeature::HeadAggregateWeight` refusal has been replaced.
The [head contribution contract](../../proofs/guide/head-contributions.md) records
the formal source basis and explicit clingo comparison differences.
Empty minima and maxima are `#sup` and `#inf`; bounds never create support.
Objective-relevant function-head producers remain refused, including count heads.

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
contribute neither cost nor priority. Their tuple, priority, binding, filter and
resource checks still run. A numeric zero retains a priority; an omitted weight
does not. Eligible unrepresentable maximize negation is a located failure;
an excluded nonnumeric priority needs no weight normalization. Undefined priority
arithmetic remains a located source-evaluation failure. Weight and tuple
expressions reuse that same scalar evaluator and are specialized after source
eligibility selects a complete binding. Arithmetic, structural constructors and
logical extrema retain typed value identity. Simple fields retain lifted joins;
resolved fields feed the same global key, score and candidate-bound operations.
Aggregate/conditional weak bodies remain unsupported.

Objective dependencies have additional boundaries because a possible-support
upper bound alone cannot determine clingo-compatible priority presence.
The additional ordinary source-completion profile admits acyclic producer cones
with normal rules, positive atomic disjunctions, ordinary choices and admitted
aggregate heads. Every positive head supplies possible objective eligibility;
its permission is independent of whether its tuple contributes to the bound.
Bodies may
contain positive/default-negated atoms and admitted scalar guards. Objective
conditions in that profile may use default negation, double negation, Boolean
truth and bounded scalar comparisons, including arithmetic on independently
bound variables. These conditions lower into closed queries over the original
complete model; they do not add atoms or theory roots. Finite ordinary cyclic
cones and their unresolved dependants use completed possible support as an
explicit conservative certificate: every covered atom remains optional, and
atoms outside that carrier are absent. This does not infer realization or run
another solver. Cyclic aggregate generators and richer producer forms outside
these certificates retain a located `ObjectiveSourceEligibility` refusal.

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
silently normalized in comparisons. Source completion visits the finite dependency cone in order, charges
all ground joins and bounds retained activity entries independently of model
evaluation. Producer traversal computes activity directly; only an eligible
objective row constructs a closed model query and consumes its per-template
condition-node ceiling. Dependency storage is checked before each new predicate
is retained, including coexisting legacy certificates. Each objective selects its own certificate. An independent extended
query does not impose this cone's acyclicity requirement on a previously admitted
positive objective; completed activity and legacy aggregate carriers share the
retained-presence ceiling when they coexist. Neither certificate searches for
answer sets or replaces original rules.

Aggregate-dependent cones retain additional producer boundaries. A structural
certificate admits specified total assignments and acyclic
bijective predicate forwarding. It does not admit arbitrary joins, filters,
alternative producers or constructed forwarding arguments.

An ordinary choice containing only default-negated occurrences does not create
a competing positive producer. Such occurrences can compose with certified
forwarding or assignment dependencies; their bounds still constrain the answers.
This does not remove the separate function-head or objective-condition refusals.

For variable objective weights, a mixed numeric/nonnumeric extrema carrier needs
an additional completed-presence certificate. Its admitted flat profile has an
unconditional unary assignment, closed tuples, and tuple conditions that are
empty or one positive closed fact/unbounded-choice atom. Relevant forwarding is
a unique unary positive renaming. Exact signed facts are mandatory; optional
choices remain possible even when constraints correlate them. Outer joins,
filtered/alternative producers and nested reductions do not gain this certificate.
Preparation can succeed before materialization detects the unsupported case.
Generated priority inputs additionally require a completed source measure
carrier. In the same flat unary profile, its values are the measures of all
complete key sets `S` with `R ⊆ S ⊆ P`, where `R` is required and `P` is possible.
Required/optional aliases coalesce by the full tuple. Unique unary renamings
share the certificate. Completed objective rows must belong to their source
carrier before priority evaluation; all fields resolve from the same binding.
Original aggregate equalities remain in the reduct theory and decide which
values realize in each answer. Shared optional conditions can leave source
values unrealized; their numeric priority slots still remain, with zero cost.
Undefined source priority arithmetic remains a located error even for such a
row. Invariant carriers are the singleton case. Broader producer shapes remain
outside this certificate.

Within that completed flat-carrier profile, objective conditions may select a
generated value by a literal, equality/disequality filter, or a repeated variable
shared with another positive condition. Selection applies to whole bindings
before priority evaluation, including fixed-priority objectives. An unrealized
value in the source carrier can retain an inactive priority slot; a proposal
outside the completed carrier cannot. The original aggregate equalities and
model-relative objective conditions remain intact. Selected fixed-priority rows
use the same bounded specialization path as dynamic priorities.
`FormulaLimits::max_objective_presence_entries` conservatively bounds
logical presence-planning slots, including completed carrier values,
rather than allocator bytes. Transient numeric subset construction separately
uses `max_assignment_values` and the grounding work bound; it can be exponential.
See [dependency checks](src/formula_objective_dependencies.rs) and
[presence classification](src/formula_objective_dependencies/presence.rs).

## Sources, analysis and output views

`SourceBundle::load_many` loads ordered original roots with global constants and
retained include occurrences. Includes first use the captured working directory,
then the including file's directory after a relative filesystem lookup failure.
Repeated selected paths are included once. Lexical aliases of one canonical
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

The formula boundary also admits bounded term observations with structural output
values, positive binders, admitted negative atoms and nongenerative scalar
comparisons. Evaluation produces a separate term channel over the supplied full
model; it does not establish stability or add support. Terms deduplicate within
that channel, while equal atom/term spellings both remain visible. Arithmetic or
generative displays and broader condition forms remain refused.
See [observation APIs](src/observation.rs).

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
bundle failures retain the source catalog. Terminal styling belongs to the CLI.

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
