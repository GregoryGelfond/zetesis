# Admitted language

zetesis targets clingo-compatible source and answer-set semantics within an
explicit scope. Parsing a construct does not establish that its current
lowering, grounding or execution profile supports it. The tables below describe
implemented forms and their boundaries; they are not a whole-language parity
claim or a promise that every combination of individually supported forms works.

The ordinary source driver selects a supported profile automatically. Explicit
library admission doors are narrower: `admit` accepts the strict relational
profile, `admit_extended` adds its scalar expansions, and `admit_formula` builds
the broader finite Ferraris representation. Automatic formula admission can
defer eligible terminal positive definitions and reconstruct them from verified
base answers; the remaining rules are materialized eagerly.
Explicit lazy CPU execution can retain a producer core and stream
eligible ordinary constraints. Objectives are scored after complete original
constraint checks and disable terminal deferral. This hybrid profile requires
indexed joins. Complete possible support and source arithmetic
admission remain required. See the [grounding profiles](../architecture/grounding.md#eager-and-lazy-execution).
Selecting a GPU does not expand the accepted source language.

## Source loading and finite admission limits

Original file roots and supported `#include` directives form one bounded source
bundle. An include targeting a file on the active include chain is refused as a
cycle. Reusing a file after its earlier traversal has completed is permitted and
includes that file once, as with a diamond-shaped include graph. clingo 5.8.2
warns and skips an already included file even on a cycle; zetesis requires an
acyclic include chain. This is a source-loading contract, separate from the
answer-set semantics of the resulting program. See
[`SourceBundle`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/bundle.rs)
for canonical identity, path and resource requirements.

Ordinary execution derives source, formula, support and output capacities from
one memory allowance and a thread count. Formula atoms, nodes, roots and child
occurrences have separate named storage capacities; repeated children still
count separately. Domain values and generated values are sets: retained payload
is counted once per distinct value, regardless of how often it is encountered.
The allowance is not a process resident-memory cap.

There is no user-selected work, substitution or support-round ceiling. Those
operations remain counted by checked counters for statistics. Cancellation and
an optional deadline can stop execution. Finite representation and recursive
input-depth checks remain, as do internal bounds on optional analyses and GPU
dispatches. Exhaustion or refusal cannot establish completed support,
unsatisfiability or optimality.

Library callers can select explicit bounds through
[`FormulaLimits`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula.rs)
and the corresponding relational limit types. For the same policy as the CLI,
use `zetesis_solve::Resources` to derive admission, grounding and execution
settings together. Zero memory is a real limit, not a request for unlimited
storage. See [resource controls](commands.md#choose-execution-and-limits).

## Rules, terms, and bindings

| Form | Implemented scope | Remaining boundary |
| --- | --- | --- |
| Normal rules and constraints | Safe finite relational rules, default and double negation, singleton unbounded choices | More general constructs use the formula profile |
| Strong negation | Signed atom identities and coherence constraints; signed source and observation forms | Remaining constructor and condition profiles still apply |
| Disjunction | Finite signed/evaluated disjunctive heads and conditional disjuncts with independent local bindings; direct formula compilation without shifting | Unresolved local bindings and unsupported inverse generators |
| Boolean literals | Signed `#true`/`#false` in rule bodies and choice/aggregate conditions; Boolean singleton heads and disjuncts with finite local conditions; signed Boolean choice and function-head elements | Separate objective/observation condition profiles |
| Logical values | Closed signed functions, tuples, strings and extremal terms; complete variable copying, finite construction from bound inputs, nested pools/intervals and structural comparisons | Unbound expression inputs and unbounded recursive construction |
| Positive witnesses | Constructor/tuple patterns preserve sign, name, arity and complete supporting atoms; evaluated positions consume bound inputs | Arithmetic inversion; a negative atom cannot supply a missing binding |
| Comparisons | Equality/disequality, structural ordering, admitted flat-tuple equality and complete comparison chains, including default/double negation; directed finite integer-affine chains can bind several unresolved variables | Nonlinear inverse binders and simultaneous systems without directed finite bounds |
| Finite generators | Scalar equality, admitted flat-tuple equality, closed integer bounds, dependent intervals and scoped finite pools in rules, local elements and objectives | Unresolved or circular binding dependencies and unsupported arithmetic inversion |
| Universal body conditionals | Complete local implication families with signed atom and comparison alternatives, scoped pools/intervals, positive local witnesses and finite anonymous projections under `not` or `not not` | Unbound local inputs and unsupported arithmetic inversion |

A conditional disjunct `H:C` contributes `(C → H) ∧ not not C` at each
completed local binding. Those instances join the same head disjunction;
an empty family contributes false. Each local condition starts with the same
outer bindings, and its private variables cannot bind the enclosing rule.
The condition determines eligibility, while its original implication remains
in the frozen reduct. A positive head atom obtains support only from its own
condition together with the rule body; default and double negation retain their
original form. See the [conditional-head laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/conditional-heads.md)
and [complete-family controls](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/conditional_heads.rs).

For example, `q(X) :- d(X), p(X+1).` checks the complete supporting `p` atom
after `d(X)` binds `X`. By contrast, `q(X) :- p(X+1).` requires arithmetic
inversion and receives the typed `UnboundArgumentInput` refusal.

Ordinary singleton comparison heads, such as `X=Y :- selected(X), selected(Y).`,
require the comparison at every completed body binding. Equality in a head never
binds a variable. These heads use the existing nonbinding comparison profile,
including default/double negation and complete chain evaluation, and retain
source arithmetic diagnostics, including failures and warnings in nested body
conditions even when the head is true. Comparison heads in conditional/disjunctive,
choice and function-head elements remain unsupported.

Comparisons use ASP term order across admitted value types: `5 < a` and
`"foo" > a` are true, while `5 > a` is false. Thus `X > Y` does not imply
numeric operands. Arithmetic is evaluated before comparison; defined `X + Y`
requires numbers and remains subject to the
[arithmetic refusal policy](#numeric-boundaries-and-refusal-meaning).

The current flat-tuple comparison profile requires tuple operands on both sides
when a tuple still contains variables after normalization. It cannot bind that
whole tuple to a variable, even when a positive atom supplies its component bindings:

```asp
d(1;2).p(Y):-d(X),Y=(-f(X),(X,))=(-f(2),(2,)).
```

Parsing and raising accept this program, but formula admission returns the
located `ProfileFeature::Term` refusal. The same boundary applies when the
comparison chain is preceded by `not not`. clingo 5.8.2 completes both forms
with the single answer `{d(1), d(2), p((-f(2),(2,)))}`. This is an existing
admission limitation, not a disagreement about answer-set semantics. The
[finite-value controls](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/finite_values.rs)
check the exact refusal separately from admitted constructor comparisons.

A generating comparison chain may bound several variables before their values
are known:

```asp
{{#include ../../../crates/zetesis-cli/tests/fixtures/finite-carriers/affine.lp}}
```

Its single answer is `{p(1,1),p(1,2),p(2,2)}`. The formula binding planner derives
finite integer intervals, enumerates their values, then checks the original
whole comparison. Intervals cover possible substitutions; they do not discard
the correlation between `X` and `Y`.

The fallback analysis accepts integer-affine addition, subtraction, negation
and multiplication by a closed integer. Closed means syntactically free of
variables after normalization; an already bound variable is still a variable
for this analysis. An endpoint can be inferred only when
the required endpoints of every other term are available. This covers directed
chains, shared variables and finite local scopes. Single default negation does
not supply a generator; double negation retains the comparison's binding
profile. Nonlinear inversion and simultaneous equation solving are outside this
profile. Independently bound expressions still use the ordinary scalar evaluator.
Positive relational values alone do not supply closed endpoints: `d(Y),
0<X+Y<3` cannot use `Y` to generate `X`. Independent source equalities or ranges
can supply the required anchors.

The analysis uses checked 64-bit coefficients and 128-bit intermediate bounds;
exhausting either width gives a located analysis limit. Source integer values
remain 32-bit. Evaluation failures follow the
[arithmetic family policy](#numeric-boundaries-and-refusal-meaning); coefficient
and bound capacity refusals do not become skippable instances.
Expansion work and retained storage are charged before enumeration. See the
[finite-chain contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/finite_chains.rs)
for scoped formulas, complete answer families and resource boundaries.

A positive conditional consequent may bind a local variable from one part of
the same complete witness and then check a dependent expression:

```asp
{p(1,2); p(2,4)}.
q :- p(X,X+1) : #true.
```

The witness plan can extract `X` before evaluating `X+1`; it does not solve an
equation for an otherwise unbound variable. Local witness variables cannot
establish outer-rule or condition safety. Empty completed universal families
are true, while recursive conditions retain their original implications.
The maintained [witness tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/evaluated_witnesses.rs)
exercise these distinctions.

Under `not` or `not not`, anonymous positions instead denote an existential
projection of complete matching atoms. Negation applies to that projection,
before source alternatives are disjoined and condition rows are conjoined:

```asp
{{#include ../../../crates/zetesis-cli/tests/fixtures/projected-conditionals/absent.lp}}
```

Its answer sets are `{q}`, `{p(1)}`, `{p(2)}` and `{p(1),p(2)}`. Here `q` requires
the absence of every matching `p` atom. Replacing `not` with `not not` tests for
the presence of a witness in the candidate; it does not produce a `p` atom.
The same projection contract covers multiple anonymous positions and anonymous
descendants of positive constructors and tuples. Other inputs must already be
bound; arithmetic is evaluated before witness matching. Admitted finite interval
and pool alternatives retain separate projections.

An empty witness family, an empty source-alternative family and an empty
condition-row family have different meanings. Only completed enumeration can
establish any of them:

| Completed empty family | Meaning |
| --- | --- |
| Witnesses of one source alternative | That projection is false; `not` makes it true and `not not` leaves it false |
| Source alternatives for one condition row | That row has a false consequent, so its implication requires a false condition |
| Condition rows | The entire universal conditional is true |

Anonymous inputs inside arithmetic or unary wrappers,
classical-negative anonymous predicates and genuinely unbound named inputs
remain refused. Expanding a pool does not let one alternative supply another
alternative's missing input. See the
[conditional contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/consequent_alternatives.rs)
and the finite-carrier laws in
[`ProjectedConditionals`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ProjectedConditionals.lean).

Finite pools retain source occurrence boundaries. In an ordinary rule, each
head, body or guard occurrence chooses independently and the product emits
complete rules. Thus `q :- p(1;2).` supplies the two rules `q :- p(1).` and
`q :- p(2).`; it does not require both body atoms. Pool alternatives inside a
choice, aggregate or optimization element remain in that same group. Complete
atom or tuple keys still coalesce by the group's established semantics. Each
pool-expanded Boolean choice occurrence instead receives a distinct key, even
when two alternatives have equal values.

A local conditional condition has its own environment for every pool selection.
Universal body conditions contribute separate conjuncts; conditional head
instances remain alternatives in one disjunction. A bound variable shared within
one occurrence keeps its identity, while identical pool spellings in two
occurrences do not force equal selections.

Comparison consequents use the same inner alternative order. For example,
`q :- X=(1;2) : d(X).` requires each completed `d(X)` condition to have at least
one matching value alternative. Comparison alternatives cannot bind a missing
source name. A range in the middle of a comparison chain chooses one value for
both adjacent tests. All reached alternatives are checked, including those
after a true result. A numeric zero divisor contributes to the complete family's
definedness check; a fatal arithmetic error cannot disappear through a true
alternative.

Nested pools distribute through constructors and checked expressions before
scope compilation. Nested intervals use fresh data slots in that scope; their
values are combined with the other constructor arguments only after the required
inputs are bound. Every interval occurrence is independent, including repeated
spellings. An empty range can yield no instances, but cannot hide an unsafe
source name or a required arithmetic failure. Source expansion, value storage,
substitution and work limits still refuse the whole operation. The
[finite occurrence controls](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/finite_pools.rs)
compare complete answers and every original/frozen pair with explicit expansions.
Their mathematical coverage premises are described in the
[finite occurrence guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/finite-occurrences.md).

## Choices and aggregates

Choices admit complete logical bounds, including symbols, strings, signed
constructors, tuples, `#inf` and `#sup`, as well as integers. Scoped variables,
duplicate eligibility, recursive conditions, evaluated arguments and admitted
top-level numeric intervals retain their existing meanings. Complete outer scalar
or dependency-ordered aggregate-assignment values can supply bounds; their original
equalities and whole-group activation remain in the resulting formulas.
Unsupported local generators remain restricted.

For example:

```asp
{{#include ../../../crates/zetesis-cli/tests/fixtures/bounds-priorities/upper.lp}}
```

Its answer sets are `{}`, `{a}`, `{b}` and `{a,b}`. The upper bound `word` is
greater than every integer count. Using it as a lower bound instead gives no
answer set; `#inf {a;b} #sup.` admits all four. This is logical term ordering,
not conversion of a symbol to a numeric threshold.

The same comparison contract applies to all six relations in admitted body and
head `#count`, `#sum` and `#sum+` contexts. Every finite numeric measure is above
`#inf` and below every other nonnumeric logical value. Its comparison with such
a bound therefore has one truth value across every tuple selection, including
the empty selection. The canonical aggregate can be replaced by that constant
in both the original formula and every frozen reduct. Head permissions and signed
activity remain separate: a true bound cannot supply support for an atom.
Source bindings, eligibility, measured values and reached arithmetic still undergo
their required validation before a comparison is folded. A missing extremum-head
value is neutral under the explicit rule below; it does not bypass validation.

The [logical-bound contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/logical_bounds.rs)
cover declared answer families, all comparison relations, arbitrary frozen
`M/J` queries, source comparisons and resource boundaries. Their preservation
law is described under [implementation correspondence](../lean/correspondence.md).

Signed `#true` and `#false` may occupy ordinary choice elements and elements
of `#count`, `#sum`, `#sum+`, `#min` and `#max` heads. A Boolean operand affects
activity through its truth and its own condition. It introduces no atom and
supplies no support for atoms in that condition. Thus `1{#true;a}1.` has only
the empty answer set, while `1{#false;a}1.` has only `{a}`.

For ordinary Boolean choices, each pool-expanded element has its own key within
an activated outer group. All eligible grounding witnesses of that expanded
element share its key. Equal pool alternatives still produce distinct keys.
For example, `2{#true;#true}2.` has the empty answer set, but
`d(1..2).2{#true:d(X)}2.` has no answer set: the two `d(X)` witnesses activate
one occurrence. Separate written rules retain separate groups. In particular,
`1{#true}1.1{#true;#true}1.` is inconsistent. Ordinary atomic choices identify
their contributions by default-negation sign and complete atom; explicit
function heads use complete tuple identity.

Both atomic and Boolean operands admit `not` and `not not`. These forms evaluate
their operand truth in the candidate and supply no positive producer support.
Eligibility conditions retain their own reduct. Thus `{not not a}.` does not
support `a`, while `{a}.` does. Ordinary `a` and `not not a` are distinct
contributions: `1{a;not a;not not a}1.` has only the empty answer set, and
`2{a;not not a}2.` has only `{a}`. Repeated occurrences of the same signed atom
coalesce; repeated Boolean elements retain their separate occurrence keys.

The Boolean element grammar extends ASP-Core-2. Its reduct laws and the
remaining source-to-implementation correspondence are separated in the
[Lean proof boundary](../lean/correspondence.md). Pool counting follows the
expanded occurrences: `{p(1);p(2)}.1{#true:p(1;2)}1.` admits exactly `{p(1)}`
and `{p(2)}`, as does the head written with `#true:p(1);#true:p(2)`.
The both-atom interpretation activates two keys and violates the upper bound.
Likewise, `{ #true : p(1;1) } = 2. p(1).` has answer set `{p(1)}` because its
two equal pool alternatives produce two active occurrences. This differs from
multiple variable bindings of one expanded element, which share its key.

| Aggregate form | Implemented scope | Remaining boundary |
| --- | --- | --- |
| Body `#count`, `#sum`, `#sum+` | Finite comparisons against complete logical bounds, recursive eligibility, complete-tuple coalescing and fresh-target assignments with completed finite support | Unresolved dependencies within a binding scope and unsupported local generators |
| Body `#min`, `#max` | Comparisons over complete logical values; empty extrema; dependency-ordered assignments with completed finite support | Missing tuple values, the numeric endpoint guard below and unsupported local consumers |
| Assignment consumers | Dependency-ordered scalar/tuple filters and equalities, evaluated arguments/heads, outer negative atoms and ranges, logical choice bounds and nonbinding aggregate guards; complete outer proposals can feed finite local choice, aggregate-element, conditional-head and universal-conditional scopes while retaining the original equality | Unresolved dependencies within a binding scope and local generators outside the admitted finite profiles |
| `#count` heads | Positive atomic permission coalesced by head atom and signed activity coalesced by complete tuple, including both alias directions, Boolean operands and objective dependencies | Unsupported local eligibility scopes |
| `#sum`, `#sum+` heads | Signed numeric `#sum` and positive-only numeric `#sum+`; missing/nonnumeric measures contribute zero; positive atomic permission is independent of numeric contribution, including nonpositive `#sum+` weights and objective dependencies | Unsupported local eligibility scopes |
| `#min`, `#max` heads | Complete first values use ASP term order; missing values are neutral under the declared extension; positive atomic permission coalesces by head atom and signed activity by complete tuple, including both alias directions, Boolean operands and objective dependencies | The numeric endpoint guard and unsupported local eligibility scopes |

A tuple becomes active when any occurrence has a true head operand and satisfies
its own condition. An unsigned atomic operand is true when that atom is selected;
`#true` is true and `#false` is false. A sum counts each active tuple's weight
once. Thus `1#sum{1:a;1:b}1.` admits `{a}`, `{b}` and `{a,b}`. Distinct
complete tuples remain distinct contributions even if they share an atom:
`3#sum{1:a;2:a}3.` admits `{a}`, as does `2#sum{1,k:a;1,l:a}2.`. Conditions
retain their model-relative truth and reduct implications. Atom permission is
independent of contribution, so `0#sum+{0:a;0:b}0.` retains all four choices.
Missing or nonnumeric sum measures also contribute zero without removing
permission. Thus `0#sum{:a;word:b}0.` has those same four answer sets. For
`#sum+`, a negative numeric weight contributes zero: `0#sum+{-2:a;2:b}0.`
admits `{}` and `{a}`. A head with no bound retains its permissions without an
aggregate constraint; no extremum result is required in that case.
Boolean and atomic occurrences may share one explicit tuple: in
`{a}.1#count{1:#true:a;1:b}1.`, either a selected `a` or a selected `b`
activates tuple `1`, giving `{a}`, `{b}` and `{a,b}`. Repeating that tuple
does not add a contribution: `2#sum{1:#true;1:#true}2.` is inconsistent.

Extrema use the same tuple-selection rule. `1#min{1:a;1:b}1.` admits
`{a}`, `{b}` and `{a,b}`: either selected atom activates the shared tuple.
Conversely, `1#max{0:a;1:a}1.` admits `{a}`, whose selection activates both
complete tuples. An extremum bound supplies no support for an otherwise
unsupported atom. The optional count specialization still requires its stronger
tuple/atom bijection certificate; admitting aliases does not grant that certificate.
Default-negated or Boolean operands also prevent a group from supplying that
atom-only certificate. Any nonnumeric logical bound excludes its whole group
from this numeric certificate, even when another bound is numeric. Other independently
qualified numeric groups remain eligible for the specialization. Model-relative
operand or body truth does not bypass required source validation. A nonnumeric sum measure is
valid and neutral. An evaluated numeric zero divisor follows the arithmetic
family policy; it is never replaced by a zero contribution. Other arithmetic
failures remain errors.

An extremum measure retains the complete first value of its tuple. For example:

```asp
{{#include ../../../crates/zetesis-cli/tests/fixtures/logical-extrema/min-symbols.lp}}
```

This program has answer sets `{a}` and `{a,b}`: the minimum selected value must
be `m`, while selecting the tuple beginning with `n` remains optional. Numbers,
symbols, strings, signed constructors and tuples use the same logical term order
as body extrema. Empty minima and maxima remain `#sup` and `#inf`; a selected
tuple whose value is that extremum still retains its own activity and permission.
The [ordered-head laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/OrderedHeadActivity.lean)
separate these concerns and state the required order and carrier assumptions.

Within each rule, aggregate assignments may depend on earlier assignments
through scalar and range values. The binding plan needs a finite dependency
order. Recursive dependencies between producer rules are admitted when their
possible-support construction completes within its bounds. A proposed value
remains guarded by the original aggregate equality; support membership cannot
replace model-relative truth.

For a finite selected tuple set `T`, let `V(T)` contain the first values of its
nonempty tuples. An extremum head uses the least/greatest value in `V(T)`, or
`#sup`/`#inf` respectively when `V(T)` is empty. Empty tuples have no measure
contribution and retain their independent head permission. A first value that
is itself `()` is present, as are symbols, strings and both infinite values.
This conserves ordinary extrema on empty aggregates and complete nonempty
tuples. It is a declared extension at the all-missing boundary, not a rule
uniquely determined by the published nonempty-set definition.

For example:

```asp
{{#include ../../../crates/zetesis-themelios/tests/fixtures/head-neutral-extrema.lp}}
```

This checked source has answer sets `{b}` and `{a,b}`. Its head
translation is `(a ∨ ¬a) ∧ (b ∨ ¬b) ∧ ¬¬b`: the bound checks the candidate,
while the choices provide permission. At those two candidates, the reducts
reduce to `b` and `a ∧ b`. Erasing the neutral `a` permission would change the
second reduct and lose `{a,b}`. clingo 5.8.2 returns only `{b}` for this exact
source. The [full rationale](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/head-contributions.md#missing-extrema-the-declared-extension)
states the finite rule, conservation domain and source correspondence boundary.
The extension applies to heads; body, assignment and observation extrema keep
their existing nonempty-tuple admission profile.

The head permissions and sum contributions follow [Abstract Gringo,
§§2.1 and 3](https://arxiv.org/pdf/1507.06576v2); missing extrema use the declared
extension above. Some source forms produce
different answer-set families in clingo 5.8.2. The [contribution contract and
comparison table](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/head-contributions.md) give the original
programs, both results and the remaining source-correspondence limits. Those
differences are tested explicitly and do not count as clingo parity.

### Interpreting source analysis

For ordinary Boolean choices, `analysis_basis()` reports
`AnalysisBasis::DependencyProjection`. The returned `analyzed_program()` is a
signature/polarity projection; `source_analysis()` reports structural facts
about that projection. Its safety and class verdicts are not conclusions about
the original source semantics. The formula compiler retains counted choice
entries and their enclosing groups, assigns Boolean counting identities after
pool expansion, and preserves statement provenance for diagnostics.

Boolean operands in explicit tuple-keyed aggregate heads do not by themselves
require that projection. Always inspect `analysis_basis()` before interpreting
analysis results; other admitted constructs can also require a projection.

## Objectives and observations

`#minimize`, `#maximize` and weak constraints admit finite safely bound priority
expressions; an omitted priority is zero. Priority inputs come from the element's
admitted outer bindings. Weight, priority and tuple values must
belong to the same completed binding. Grounding retains the model-relative
conditions in fixed-priority objective templates; it does not add producer support
or change the original answer-set theory.

```asp
{{#include ../../../crates/zetesis-cli/tests/fixtures/bounds-priorities/correlated.lp}}
```

Both answer sets contain the two `row` facts. The answer containing `a` and
`selected(1)` has costs `[0,2]` at priorities `[2,1]`; the answer containing `b`
and `selected(2)` has costs `[1,0]`. The first is optimal because the higher
priority is compared first, even though its total numeric cost is larger.
Evaluating weight and priority from independent row projections would lose
this correlation.

A resolved nonnumeric weight or priority contributes neither cost nor a priority
slot after required structure and evaluation checks. Weight, priority and tuple
fields participate jointly in the arithmetic family policy, including when the
weight is nonnumeric. Pooled alternatives from one original element share its
family; a defined element cannot rescue a different entirely undefined element.
For numeric weight and priority pairs, contributions coalesce globally by normalized weight,
priority and the complete explicit tuple. Maximization negates weights before
this coalescing; checked overflow remains an error. A numeric zero weight retains
its numeric priority, as does a numeric contribution whose condition is false in
a particular answer. A completed empty binding family contributes no priority.

Search preserves all optimal ties within its resource and delivery limits.
Objectives admit signed atomic and Boolean conditions, default and double
negation, scalar comparisons and checked expressions in their weight, priority
and complete tuple. Producer dependencies can include disjunctions, aggregate
heads and recursive aggregate assignments or universal conditionals whose
possible-support construction completes within its bounds. Weak-constraint bodies can directly use the admitted finite
aggregate guards, assignments and universal conditionals. Scalar binders,
intervals and structural positive patterns also supply outer bindings in weak
constraints and optimization elements where their grammar permits them. A local
aggregate or conditional variable cannot bind an outer weight, priority or tuple.
Pooled conditional consequents in weak bodies retain their local alternatives:
`p(X;X+1):d(X)` requires, for each completed `d(X)` binding, at least one of
`p(X)` and `p(X+1)`. Negation applies to each alternative before their disjunction.
The analyzed program uses the same bounded signature/polarity projection as rule
bodies and reports `AnalysisBasis::DependencyProjection`; this bounded syntax
projection preserves signed dependencies and arities, not the objective's truth
condition. Weight, priority, complete tuple and runtime query remain attached to
the original weak constraint.

Pools in ordinary weak-body atoms and guards form independent observations.
Pools in local conditions or aggregate tuples remain local to their element;
weight, priority and complete objective-tuple pools form independent complete
keys. Repeated complete keys still contribute once if any corresponding
condition is true. Nested interval fields use the same scoped data generators.
The [objective pool contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/objective_pools.rs)
compare the complete scored families with explicit observations. An alternative
must establish its own required bindings: `d(X;X+1)` cannot use the first
alternative to make `X` safe in the second. Unresolved binding dependencies and
unsupported arithmetic inversion remain refused.

For example:

```asp
{{#include ../../../crates/zetesis-solve/tests/fixtures/language-consumers/scoped-objectives.lp}}
```

The full answers are `{a}`, `{b}` and `{a,b}`, with costs `[1,1]`, `[1,0]`
and `[2,1]` at priorities `[2,1]`. The count supplies the higher-priority cost;
the conditional `a : b` supplies the lower-priority cost precisely when `b`
implies `a` in that answer. Only `{b}` is optimal. The shown count is a separate
query; it does not supply support or change either cost.

Scoped body compilation uses a separate value domain and transient formula
storage. Each completed eligible row undergoes required body validation before
activity exclusion or numeric selection. Only retained numeric rows allocate
closed objective query nodes. `FormulaLimits::max_objective_formula_atoms`,
`max_objective_formula_nodes` and `max_objective_formula_operands` bound each
transient body independently of the original theory and retained `objective.max_condition_nodes`; cumulative source
work and value budgets still apply. The [scoped objective contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/objective_scopes.rs)
check full costs, scope, required errors and independent limits.

Transient rich producer and projection-condition validation instead apply
`theory.max_atoms`, `theory.max_nodes` and `theory.max_operands` independently
to each temporary builder. They do not add atoms or roots to the original theory.
`max_objective_presence_entries` bounds shared source-activity planning slots,
including predicate and scope traversal and simultaneous old/new activity
tables, as well as objective-presence carriers. These limits count their stated
formula or planning populations. They are not allocator or process-memory caps;
activity folding does not consume retained `objective.max_condition_nodes`.

Each objective has its own source-eligibility plan. The shared source-activity
fold reads the existing lowered bodies, including aggregate assignments, guards
and universal conditionals. It can establish absent or required atoms, or leave
them optional. Recursive dependencies start from completed possible support and
refine only after aggregating every producer for a whole round. An optional atom
and its negation remain optional; the analysis does not decide their joint
realizability. Heads outside this refinement profile retain the conservative
carrier. An independently absent producer supplies no row or redundant zero-cost
priority slot, while original-model queries determine the actual costs of all
retained rows. Removing an always-zero slot preserves costs aligned by priority
and every optimum tie, although the raw vector becomes shorter.

A completed support round adds no new positive head atom after all its joins and
binding proposals finish. Resource exhaustion returns a typed failure, not a
program with incomplete objective coverage. Recursive value generation need not
terminate; finite storage in an intermediate round supplies no completion result.
The [cyclic producer contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/objective_rich_cycles.rs)
cover complete scored families, source order, independent precision and resource
exits. Their [coverage argument](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/source-support.md)
separates observed closure from the source-to-reduct projection premise.

Applicable source-carrier refinements can exclude impossible generated values.
Their errors remain errors; lack of an applicable refinement does not invalidate
a complete conservative carrier. A flat unary observer over closed facts and
unbounded choices supplies one such refinement. Let R be its required
complete tuple keys and P its possible keys. The carrier is
{measure(S) | R ⊆ S ⊆ P}. It includes invariant results as singleton carriers.
Unique unary renamings preserve the carrier. Original aggregate equalities
remain in the theory; carrier membership filters objective proposal rows before
priority evaluation.

The base predicate and each forwarding edge qualify independently. A filtered
or multiply produced descendant uses its own source-support carrier; observing
it cannot discard an established base or sibling refinement. The
[composition contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/objective_carrier_composition.rs)
check this independence under source order, additional observers and competing
producers, including the shared work and storage limits.

```asp
{{#include ../../../crates/zetesis-cli/tests/fixtures/finite-carriers/count.lp}}
```

Both count keys depend on `a`, so the answer sets contain either `n(0)` or
`a,n(2)`. The source carrier is {0,1,2}; priority 1 remains as a zero slot.
The respective cost vectors at priorities [2,1,0] are [0,0,1] and [1,0,0].
A source carrier describes possible key selections, not realizable answers.
Several admitted observers use the existing complete binding join; their
original equalities retain correlations in each answer.

The [invariant-priority tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/objective_priority_certificates.rs)
include empty endpoints, complete tuple identities and correlated weight/priority
inputs. The [source-carrier tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cli/tests/integration/finite_carrier_sessions.rs)
check changing counts, sums, extrema and forwarding through complete scored
answers. Carrier construction has explicit value, work and retained-entry
limits; numeric subset construction may require exponential work and space.
Filtered and multiple observers, mixed extrema, negative dependencies and
conditional producers use their complete finite source relations when the
flat refinement does not apply. The [producer contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/objective_rich_producers.rs)
check full scored families and unchanged original formulas. The [priority contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/objective_priorities.rs)
check complete scored answers, same-binding numeric presence, empty and zero
cases, evaluation failures and inclusive specialization limits.

Finite source eligibility does not promise clingo's exact retained priority
layout. An extra slot may be zero across all answers even when clingo omits it.
These [versioned reporting cases](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/objective_priority_reporting.rs)
preserve explicit raw differences while comparing full answer identities,
costs at named priorities, pairwise ordering and every optimum tie. They are
distinct from the aggregate-head answer-family differences documented above.

`#defined` and `#show` retain signed signature metadata. Ground and admitted
conditional term observations operate over full models; they do not change
answer-set identity. Shown terms can use checked arithmetic, constructors,
tuples, finite intervals and pools. Conditions admit signed atoms, Boolean
operands, scalar comparisons and directed equalities, aggregates and universal
conditionals. Positive patterns bind structural components from actual model
atoms. Arithmetic reads established inputs or uses the inverse profile for one
unresolved occurrence described below. Nested structural
pool alternatives preserve their own captures, and only captures common to
every alternative can supply another body element without an independent binder.

Positive equality edges can bind from an already finite operand within a whole
comparison chain. A middle pool or interval chooses one value shared by both
adjacent comparisons; the original guards remain. Positive constructor and tuple
equalities can capture ordered components from a finite evaluated operand in
either direction, preserving sign, constructor identity and repeated-variable
agreement. Arithmetic positions can read structural captures from the same
pattern or supply one admitted inverse candidate. Strict comparisons and
default-negated chains remain consumers.

Local scopes retain their own variables. Aggregate keys remain complete tuples;
positive cardinality elements use the complete matched atom. Aggregate result
assignments can supply other guards and later consumers, but element inputs
must be independently established. Constructor-valued `#min` and `#max` equality
guards use the actual supplied-model measure for the same structural capture and
retain every original guard. Empty extrema remain `#sup` and `#inf`. Numeric
aggregate comparisons retain their widened arithmetic; structural capture does
not narrow a `#count` or `#sum` measure to an ordinary scalar value. A failed query
returns a typed error rather than partial shown output. Post-solve observations
retain strict arithmetic errors, including division or remainder by zero; source
admission's policy of omission with warnings does not apply to them. These
operations neither add values to the grounding domain nor create atom support.

Anonymous atom matching projects over the supplied model before applying
default negation. zetesis applies this rule uniformly to both strong signs.
For example, `#show seen:not -p(_).` shows `seen` precisely when no strongly
negative `p/1` atom occurs in that model. This is an explicit observation
extension: clingo 5.8.2 rejects that form as unsafe. Separate
[diagnostic fixtures](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/fixtures/observation-strong-anonymous.jsonl)
record the difference; it is not a parity pass or an answer-set semantics change.

Finite observation binding includes the following forms:

| Observation context | Exact source witness |
| --- | --- |
| Arithmetic inversion in an atom | `p(2). #show. #show X:p(X+1).` |
| Arithmetic inversion in an equality | `#show. #show X:X+1=2.` |
| Pools on the capturing side of an equality | `#show. #show 1:f((X;2))=f(1).` and `#show. #show X:(f(X);g(X))=f(1).` |
| Repeated unary sign around a capturing constructor | `#show. #show X: -(-f(X))=f(1).` |
| Anonymous keys under negated cardinality elements | `#show. #show N:N={not p(_)}.` and `p(1). #show. #show N:N={not not p(_)}.` |
| Structural equality against a numeric aggregate | `#show. #show X:f(X)=#count{}.` and `#show. #show X:f(X)=#sum{2147483647,a;1,b}.` |

One unresolved occurrence can be recovered through unary minus, addition,
subtraction or multiplication by already bound operands. The evaluator checks
the complete authored expression after deriving the candidate. An out-of-range
or nonintegral inverse supplies no scalar match; reached undefined authored
arithmetic still returns an error. A zero multiplier is refused as an unsafe
binder because it cannot provide a unique inverse.
Division, powers, bit operations, multiple unresolved occurrences and
simultaneous equations are not inverse binders in this profile.

Capturing pool alternatives retain independent bindings. Fully bound alternatives
can supply finite values after earlier binders run. Anonymous cardinality keys
preserve the complete signed atom pattern and default-negation kind. A pattern
contributes once when its existential condition holds, regardless of how many
atoms witness it; equal concrete branches coalesce before counting. The numeric
aggregate examples above have an impossible constructor match and show no value. They neither coerce
numbers into constructors nor narrow widened aggregate arithmetic.

The [binding contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/observation_bindings.rs),
[inverse contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/observation_inverse.rs)
and [expression contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/observation_expressions.rs)
retain original source witnesses. Cyclic or unseeded local bindings and missing
measures in authored extremum elements retain their separate explicit boundaries;
an empty extremum itself remains valid. Source-template limits bound admitted
alternatives; canonical metadata vocabulary and component capacities have a
separate `MetadataStorageLimits` allowance. Each observation operation has
independent work, substitution, symbol construction, logical local payload and
output limits. `max_term_storage_bytes` bounds its derived arena and retained
typed wildcard-key graph, excluding borrowed inputs. Transient ID frames use
that ceiling independently; their combined capacities and process RSS are not
that measure. The [output API guide](../rust/costs-and-output.md) states these
ownership and accounting boundaries.

### Projected enumeration

`#project p/1.` selects all original possible atoms with that signed predicate.
`#project p(X):body.` selects the finite instances retained by source activity.
All declarations contribute to one fixed domain. They do not produce logical
heads, change the original theory or choose which atoms `#show` displays.

The CLI returns one full answer-set representative for each distinct restriction
to that domain. An explicit empty domain has one class for a nonempty selected
family. Objective selection happens before representatives are chosen. A model
count limits representatives; full-model verification and optimization counts
remain separate. Programs without `#project` retain full answer identity.

Source activity is an independent-literal abstraction computed during grounding,
not a query evaluated on each answer. For example, `{p;q}. #project p:q,not q.`
still selects `p`: both literals remain optional source possibilities. In
`q. {p}. #project p:not q.`, the required fact makes the declaration inactive.
Complete producer refinement also recognizes facts in dependency cycles and
constant aggregate conditions. It does not solve correlations between optional
literals. Declaration atoms absent from original possible support are omitted.

The library requests this behavior explicitly through
[`SessionBuilder::projected`](../rust/sessions.md#projected-enumeration).
`WorldView` always collects the complete original answer family. The current
projection source profile prepares a complete finite projection domain for both
eager and hybrid formula owners; work and retained-domain
limits can refuse preparation, and separate history limits can stop enumeration
without claiming complete projected coverage.

Ordered input bundles support includes and global constants with bounded
source traversal and retained resolution evidence. Parameter-free `#program base`
sections are admitted. Named or parameterized program parts, `#external`,
assumptions and incremental solving do not yet have a public executable contract.
Mixed stdin/file input and broader include alias or redirection behavior remain
separate input-boundary limitations.
Rust ground-time `@` functions are intended as a first-class extension, but the
current source bridge refuses external calls. Neither libclingo ABI compatibility
nor general ASPIF import/export is implemented.

Theory atoms/terms, embedded Python/Lua scripting, `#heuristic` and `#edge` are
deliberate exclusions from the current target. Their refusal is not an outstanding
ordinary-language implementation obligation. Future theory integration would
need its own semantics and interface; current solving does not ignore such atoms.

## Numeric boundaries and refusal meaning

An interval contributes integers only when its evaluated endpoints are numeric
and ascending. Defined nonnumeric or descending endpoints give an empty range
in both facts and generated bindings: `p(a..b).` contributes no fact. Endpoint
errors remain errors, so `p(a..(1/0)).` refuses admission. An empty range also
does not hide an unbound variable or invalid sibling argument.

A rule is grounded over candidate substitutions for its relationally bound
variables, and a comparison between terms over those variables that is
defined and false excludes a substitution: nothing in an excluded
substitution is reached, neither the arithmetic beside the comparison nor the
head, a gate, a guard or an aggregate element under it. `d(0..2).
p(X) :- d(X), X != 0, 1/X = 1.` therefore answers, with `p(1)`: the
substitution `X = 0` is excluded by `X != 0`, so `1/0` is never evaluated.
The verdict is taken over the complete substitution, whatever order the
comparisons are met in, so a failure met before the relation that decides an
exclusion is still no refusal.

The remaining substitutions are checked as a complete source family. Only an
evaluated numeric division or remainder with a zero divisor may omit an instance:

| Complete family | Result |
| --- | --- |
| Empty positive join | Admit silently; there is no instance to evaluate |
| Defined instances only | Admit silently |
| Both defined and zero-divisor instances | Admit the defined instances and retain a typed warning |
| Zero-divisor instances with no defined instance | Refuse with a typed arithmetic error |
| Any fatal arithmetic failure | Refuse, even if other instances are defined |

Defined does not mean true. For example,
`d(0;2). p(X,Y) :- d(X), 1/X=1, Y=X+1.` is admitted with a warning and only the
two `d` facts. The `X=2` substitution is defined but fails the comparison; it
still establishes that the family is not entirely undefined. With `d(0..3)`
instead, the answer also contains `p(1,2)`. With only `d(0)`, admission fails.
Adding `X != 0` explicitly excludes the bad instance and removes the warning.
The warning reports omitted source instances and suggests guarding the divisor.
Successful formula owners retain typed warnings, deduplicated by `ProgramSite`
(original statement identity and any real source coordinate) and bounded by
`FormulaLimits::max_warnings`. Constructed programs retain the statement identity
without inventing a span; warning `location()` and `diagnostic()` then return
`None`. For source input, the CLI renders warnings once on its diagnostic stream
before solving; JSON answer output remains separate. The same distinction between
logical identity and optional source evidence applies to arithmetic failures.

Local choice and aggregate elements have separate families for each fixed outer
binding. In `d(0;1). e(0,0). e(1,1).
{p(K,X):e(K,X),1/X=1} :- d(K).`, the entirely undefined `K=0` family is refused;
the defined `K=1` family does not rescue it. If no `e(0,X)` fact is present, that
local family is empty and has no arithmetic error. An absent fact is not an
undefined arithmetic value.

Overflow, arithmetic on nonnumeric values and invalid exponents remain hard
errors. They cannot be hidden by a defined instance elsewhere. Within a reached
evaluation phase, a zero divisor does not stop checks of independent branches: an
overflow in another branch still refuses admission. An operation depending on
an undefined operand is not evaluated. The same rule applies to independent
fields of one source instance; a definedness witness requires them to be jointly
defined, not successful values drawn from different instances. Every arithmetic
intermediate must fit the signed 32-bit carrier; values do not wrap. This differs
deliberately from clingo 5.8.2's wrapping integer arithmetic. The zero-divisor
policy also differs from dropping every undefined instance unconditionally:
an entirely undefined family is treated as a modeling error.

A binder, interval check, tuple comparison, aggregate guard or comparison over
a generated value does not provide the relational exclusion described above.
Negative gates remain formulas, so `d(0). p(1/X) :- d(X), not d(X).` is refused.
Closed-term arithmetic is validated as written during source preparation,
before any substitution exists: `p :- 1 = 2, 1/0 = 1.` is refused. The family
policy does not weaken this closed-term check or post-solve observation errors.
An incomplete positive join with no complete extension creates no obligation.

Evaluation retains its body-before-head and condition-before-consequent stages.
A body or condition omitted for zero arithmetic does not start the later head,
consequent or objective-field phase; defined false body selection likewise does
not evaluate a head or consequent. Independent-error checking does not cross
that boundary. Row-dependent head atom arguments are a later stage: `d(0;1).
p((X+1)**31):-d(X),X=0.` evaluates the head only for `X=0`. The same
distinction applies to local choice elements. Negative gates and aggregate
truth remain formulas and do not prune that stage. Aggregate-head tuple
contribution checks retain their own scopes.
The [arithmetic validation contracts](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/arithmetic_validation.rs)
check defined complete families, incomplete joins, required failures and explicit
clingo differences.

There is also a conservative **zetesis extrema guard**: numeric values
`-2147483648` and `2147483647` are refused when reached as `#min`/`#max` bounds or
the first component of a possible aggregate tuple, including assignment values.
It does not ban these integers as ordinary data, later tuple-key components or
integers nested inside a structured value. The guard applies independently of
the comparison operator or recursion.

The typed refusal is `AdmissionFailure::ExtremumEndpoint`, with the evaluated
value and its source location. Its diagnostic code is `zetesis::extremum-endpoint`;
the message names the numeric endpoint and the zetesis guard.

This is an internal source-admission guard, not a themelios parser rejection.
Separately, the pinned themelios program raiser cannot represent the unsigned
magnitude in the literal `-2147483648`; `(-2147483647-1)` can reach that value by
evaluation and then encounter zetesis's guard. The former is a raising
limitation and the latter a zetesis refusal. Neither is a syntax error.

The guard protects unresolved correspondence between source extrema semantics
and the native formula interpretation. It is not proof that every guarded input
is undefined or a modeling error. The lower-level finite formula library can
represent endpoint formulas. The
[admission predicate](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_assignment.rs)
and [source regressions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/integration/extrema_source.rs)
locate the current boundary.

`#inf` and `#sup` are distinct extremal terms. Empty minimum and maximum results
use `#sup` and `#inf`, respectively. Substituting one of those terms, a neighboring
integer or a finite endpoint for another is not a generally semantics-preserving
repair.

| Failure origin | What to conclude |
| --- | --- |
| themelios parsing | Input was rejected by the pinned grammar |
| themelios raising/evaluation | That representation or operation could not handle the construct |
| zetesis admission/lowering | Current native implementation or resource contract refused it |
| Explicit exclusion | The feature lies outside the declared target |

None of these failures establishes UNSAT. A correct refusal regression can pass
without establishing answer-set parity for its source. Keep genuine source
errors, representation limits, unresolved semantics and missing implementation
mechanisms distinct when interpreting a diagnostic.
