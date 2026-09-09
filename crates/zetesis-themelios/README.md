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
establish safety. Already-bound equalities remain filters. Arithmetic inversion,
unanchored cycles, chains with several unresolved integer variables and pools or
intervals nested under constructors remain outside this profile.

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

Head conditions are limited to empty or explicitly true Boolean conditions.
General conditional disjunction remains unsupported. Boolean choice elements
and negative choice heads remain unsupported. Choice-head intervals expand
within one group; disjunctive intervals expand whole rule instances.

Finite conditional choices retain each head's eligibility, with bounds acting
as constraints rather than support. Universal body conditionals retain
condition-to-consequent implications, conjoined over complete condition rows.
Consequent alternatives may use admitted pools, intervals and local positive
structural/evaluated witnesses. These witnesses cannot bind outer variables or
establish condition safety. Only exhaustive enumeration can establish vacuity.

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

Acyclic aggregate assignments may supply later scalar/range instructions,
nonbinding aggregate guards, normal/choice/function-head values and admitted
body conditionals. Generated proposals retain every original aggregate equality;
an attainable proposal is not evidence that a particular model realizes it.
Self-dependent/cyclic generators and unsupported local consumers remain typed
refusals. Body extrema use ASP term order, including real `#inf`/`#sup` values
and structural terms, subject to the numeric endpoint guard above.

Count heads separate permission to select a head atom from complete-tuple
activity. A tuple is active if any row has both its head selected and its own
eligibility satisfied. Consequently:

```asp
1#count{1:a;1:b}1.  % {a}, {b}, {a,b}
2#count{1:a;2:a}2.  % {a}
1#count{1:a;2:a}1.  % no answer set
```

Each line is an independent program. One head may activate several tuples, and
several heads may activate one tuple. Local positive binders and admitted
positive/default/double-negated eligibility retain their own scopes.

Weighted and extrema heads retain a stricter complete-tuple/atom bijection.
`#sum` accepts signed numeric weights; head `#sum+` accepts nonnegative numeric
weights. Nonnumeric head weights, negative head `#sum+` weights and nonnumeric
extrema heads remain unsupported. A zero weight still permits its head.
Empty minima and maxima are `#sup` and `#inf`; bounds never create support.
Objective-relevant function-head producers remain refused, including count heads.

### Objectives

Formula results expose lifted objectives, origins and declarations separately
from the logical theory. `zetesis-objective` scores supplied verified models;
objectives do not derive atoms or replace reduct acceptance.

The source profile admits `#minimize`, `#maximize` and positive weak constraints
with scalar literal or positively bound variable weights, constant integer
priorities, closed/whole-variable tuples, positive ordinary conditions and scalar
equality/disequality filters. Maximize weights undergo checked negation before
global `(priority, normalized weight, full tuple)` deduplication. Costs use the
normalized minimization sign.

Resolved nonnumeric literal weights, including weight-position `#inf`/`#sup`,
contribute neither cost nor priority. Their tuple, priority, binding, filter and
resource checks still run. A numeric zero retains a priority; an omitted weight
does not. Eligible unrepresentable maximize negation is a located failure.
Dynamic priorities, variable weight arithmetic, negative objective conditions
and richer weak bodies remain unsupported.

Objective dependencies have additional boundaries because a possible-support
upper bound alone cannot determine clingo-compatible priority presence.
Relevant negative, disjunctive, conditional and general aggregate producers are
refused. A structural certificate admits specified total assignments and acyclic
bijective predicate forwarding. It does not admit arbitrary joins, filters,
alternative producers or constructed forwarding arguments.

For variable objective weights, a mixed numeric/nonnumeric extrema carrier needs
an additional completed-presence certificate. Its admitted flat profile has an
unconditional unary assignment, closed tuples, and tuple conditions that are
empty or one positive closed fact/unbounded-choice atom. Relevant forwarding is
a unique unary positive renaming. Exact signed facts are mandatory; optional
choices remain possible even when constraints correlate them. Outer joins,
filtered/alternative producers and nested reductions do not gain this certificate.
Preparation can succeed before materialization detects the unsupported case.
`FormulaLimits::max_objective_presence_entries` separately bounds simultaneously
retained logical presence-planning slots, rather than allocator bytes.
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
