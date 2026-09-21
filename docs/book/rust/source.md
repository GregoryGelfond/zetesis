# Preparing source and interpreting analysis

Use `zetesis_themelios::prepare_formula` to prepare input for zetesis's finite
formula solver and inspect its retained analysis before materialization.
`zetesis_themelios` is zetesis's source bridge; this workflow does not require
constructing an upstream parser or logical program. The returned
`PreparedFormula` owns source, metadata, checked intermediate representation and
the remaining expansion budget. `ground(self)` consumes that preparation and
produces an `AdmittedFormula`; it computes no answer sets.
`zetesis_solve::PreparedInput::formula` then borrows that owner for a session.

```text
source → prepare_formula → PreparedFormula → ground → AdmittedFormula
                              │                           │
                         inspect analysis            borrow in a session
```

The preparation can succeed while grounding later refuses arithmetic or a
resource ceiling. Splitting the calls does not refresh the source expansion
budget. Keeping the returned owner is therefore part of the contract, not merely
a convenience for avoiding another parse.

## Stream ordinary constraints

`PreparedFormula::ground_hybrid()` and the corresponding bundle method return
one shared `HybridFormula`. It retains the original source, complete possible
support and typed atom catalog. Producers and constraints with aggregates,
projected atoms or conditional scopes remain in `core_theory()`. Ordinary
atom/scalar integrity constraints retain their prepared templates instead of
complete formula DAGs. This first schedule requires indexed joins and refuses
objective programs explicitly. The existing `ground()` methods remain eager.

Use `PreparedInput::hybrid(&owner)` for solving. A core answer is only a proposal:
the session checks the retained constraints before returning an answer of the
original program. A core certificate applies to the core. It does not establish
membership in the original program on its own.

For direct composition, `owner.checker(ConstraintCheckLimits { .. })` creates a
mutable checker borrowing the owner. `check(&model, &cancellation)` requires the
model's exact atom-catalog owner. It returns `Satisfied` after the required scan
completes, or `Violated { location }` when an admitted constraint body is true.
Neither result establishes reduct minimality. Wrong-owner, cancellation,
deadline, allocation and resource failures are typed separately from both
verdicts; failures preserve cumulative statistics. Equal source bytes do not
substitute for owner identity.

Support dictionaries and equality indexes are reused. Ordinary joins lend their
current binding; generators retain their existing owned-row requirements.
Candidate truth uses borrowed typed atom keys, with no temporary formula DAG or
copied atom per instance. Original source-family arithmetic validation completes
before the hybrid owner is returned, independently of later candidate truth.
Omitted zero-divisor instances therefore retain the same located warnings, and
candidate filtering cannot conceal fatal arithmetic.

Hybrid admission preserves its original expansion-budget prefix. It also keeps
prepared constraint plans and completed support that eager grounding can release
after emission. The existing source, scalar and support ceilings still apply;
they are not a single aggregate live-memory or RSS bound. Core atoms, nodes and
roots retain the formula-theory ceilings, including coherence and unsupported
atom guards. `streamed_templates()` counts lowered templates, including pool
alternatives, and `streamed_instances()` counts scalar-selected instances visited
during admission; neither is a retained instance store.

Each checker has separate cumulative work, substitution and copied-scalar-byte
limits across all candidate checks. Snapshot descriptors are prepared once,
without copying support rows; per-operation binding/storage ceilings remain
those of the admitted source. A new checker starts new execution allowances,
not new source admission. Its statistics describe charged work and payload,
not process memory. Streaming can reduce retained constraint storage while
repeating joins and proposing more core answers; it does not remove the complete
finite atom envelope or imply a time improvement.

## Reuse original input across profiles

`ParsedSource` owns original bytes, their successful parse and fixed
`AdmissionOptions`. Its consuming profile operations retain that owner on
failure. This is useful when relational admission identifies a construct that
needs the finite formula profile:

```rust
# extern crate zetesis_themelios;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, ParsedSource};

let source = ParsedSource::new("1{a;b}1.".into(), AdmissionOptions::default())?;
let failure = source.admit_extended(ExpansionLimits::default()).unwrap_err();
assert!(failure.error().needs_formula_admission());
let prepared = failure.into_source()
    .prepare_formula(ExpansionLimits::default(), FormulaLimits::default())?;
let admitted = prepared.ground()?;
assert_eq!(admitted.atoms().len(), 2);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The example's known bounded choice requires formula admission. General callers
must inspect `needs_formula_admission()` before choosing that alternative;
undefined arithmetic, syntax and resource refusals are not permissions to retry.
`SourceFailure::error()` exposes the unchanged typed refusal, and `into_parts()`
recovers both it and the parsed owner. The String-taking convenience functions
preserve their existing result and error types.

The automatic CLI route uses this same ownership transfer. It neither clones
the entire stdin buffer nor reparses it on profile retry. The owner retains
O(source bytes + syntax nodes) storage; a failed attempt boxes that owner once.
Each attempt still checks its profile and performs any required raising and
normalization. Source options remain fixed; supplied expansion and formula
limits apply to the requested attempt. Formula grounding consumes the resulting
preparation's remaining budget as before.

Configure eager support storage through `FormulaLimits::max_support_bytes`
before preparation. The default is 128 MiB of authored catalog, snapshot, index and
query capacity, including construction scratch; nested atom payloads, allocator/tree
overhead and unrelated state have separate bounds. The CLI exposes the same
allowance as `--max-support-bytes` in `--help-all`. This is an admission limit;
`SolveConfig` applies after the formula owner has already been constructed.

Choose positive joins separately through the preparation's
`with_grounding_options(GroundingOptions { joins: JoinStrategy::Table })` method.
The default `Indexed` strategy probes existing value postings. `Table` reuses
support masks only for flat positive patterns over completed eager support;
structural patterns and support-growth rounds keep indexed joins. The bundle
preparation exposes the same method, and the CLI maps `--formula-joins table`
to it. The choice retains the preparation's source identity and remaining budgets.

The private query workspace owns cached indices and accounts for simultaneous
selected masks, scratch and cumulative work. Named vector/object capacities are
distinct from nested atom payloads, allocator/tree/control-runtime overhead and
other grounding state; the limit is not RSS. An exhausted table operation returns
a located failure without silently choosing a different strategy. The existing
matcher and required authored-body validation remain shared. Choosing table joins
does not launch ordinary Rayon or GPU grounding; later answer-set execution has
its own policy. See [finite-table selection](finite-tables.md) for the ownership,
applicability and proof boundaries.

## Optional domains during final instantiation

`PreparedFormula::with_domain_analysis(Some(DomainLimits { .. }))` requests
necessary argument-domain guards; `None` is the library default. Bundle
preparation has the same method. The ordinary command requests the analysis
with its default limits. This is separate from choosing `Indexed` or `Table`
joins, and adds no language construct or CLI flag.

The consumer checks the exact normalized whole program and its original rule
occurrences. The initial profile permits ordinary positive flat rules and
constraints, whole named variables, atomic numbers, strings, positive nullary
symbols, infimum and supremum, and body comparisons. It excludes arithmetic in
atoms, generators, negative body literals, structured terms, anonymous/local
scopes and richer heads. `NormalizedProgram` is necessary; a dependency projection never supplies
narrowing. Inapplicability keeps the existing complete path and does not create
a new source refusal.

Finite argument domains constrain each rule's variable occurrences, in every
possible-support completion round and in final instantiation. Their intersection, less every value a
comparison over that variable alone is defined and false at, can reject a row
that has no complete positive continuation, before copying its new bindings or
opening deeper probes; the candidates are prepared once per rule with the
analysis, and a guard is prepared only where they are fewer than the
argument's domain. The surviving rows keep the existing matcher, original
positions and source origins. Objective and factorized component joins retain
their existing paths. Global Unknown or Stopped analysis supplies
no guards. An individually Unknown argument is unrestricted; other finite
arguments may still contribute restrictions.

This complete example compares admitted atoms, nodes, roots and provenance,
observes an actual typed `Analysis` and positive guard activity, and checks both
local widening and a stopped analysis. It materializes theories without solving
for answer sets or measuring elapsed time.

```rust
# extern crate themelios_base;
# extern crate zetesis_domain;
# extern crate zetesis_themelios;
{{#include ../examples/domain-grounding.rs:example}}
```

Run its maintained Cargo control or the example itself from the checkout root:

```sh
cargo test --locked -p zetesis-themelios --example book-domain-grounding
cargo run --locked -p zetesis-themelios --example book-domain-grounding
```

`DomainObservation` also distinguishes Disabled and Inapplicable. An Analyzed
callback borrows the exact owner's status, context, argument domains and logical
statistics; retaining a result requires copying the specific owned information
the caller needs. Detailed work reports include applicability, analysis and guard
preparation, even on a failed or stopped prefix. A completed analysis-attempt
phase can have used conservative fallback; it does not imply FixedPoint.

Analyzer `DomainLimits` bound logical populations and inspected source/symbol
bytes. Actual work, including stopped work, consumes the original cumulative
formula budget. The analyzer uses bounded standard collections with no
fallible-allocation or caller-control API; its heap is outside
`FormulaLimits::max_support_bytes`. The guard/query lease separately accounts
its named headers, scratch, temporary atomic payload and actual vector capacity
beside support, table indices and live masks. These contracts give neither a
hard allocator/RSS limit nor cancellation/deadline polling. Guard preparation
and membership costs may outweigh avoided probes. See
[finite-domain ownership and proof boundaries](finite-tables.md#optional-argument-domain-guards)
for the exact named-capacity scope and preservation premises.

## Partial arithmetic families

Only an evaluated numeric division or remainder by zero can omit a source
instance. A nonempty original rule family needs at least one complete
substitution whose reached arithmetic is jointly defined; a defined comparison
that is false still supplies that witness. Thus `d(0;2). p(X):-d(X),1/X=1.`
keeps just the facts and reports a warning, whereas the same rule over `d(0)`
is refused. Empty positive joins are valid and silent. An ordinary comparison
over relationally bound variables, such as `X!=0`, explicitly excludes that
substitution and produces no warning for its zero divisor.

Overflow, arithmetic on a nonnumeric value and an invalid exponent remain
errors when reached. After a zero divisor, source classification continues
through independent expression branches and independent components; a later
fatal error takes precedence. An operation depending on an undefined value is
not evaluated. This source-family policy uses the same checked scalar
operations as strict evaluation; it does not change the observation evaluator's
first-failure policy. Authored closed undefined expressions are still refused
during preparation. The existing evaluation phases remain ordered: a rule body
or local condition selects an instance before head or consequent generation.
An omitted or false body does not invoke that later phase. Independent-fatal
validation applies to the branches, components and generators of the reached
phase.

The decisive traversal reads completed support, before formula emission.
It does not use comparison-pruned candidate domains, support delta partitions
or already-produced-head shortcuts as family evidence. Normalized fragments of
one original rule combine their evidence. Each nested choice, aggregate or
conditional local family is judged separately for its fixed outer binding, so
a valid family at another outer binding cannot rescue it. Alternatives of one
authored local element share evidence; distinct authored elements do not. An
undefined conditional consequent omits its whole local implication. Defined
false consequents and empty nonarithmetic witness sets retain their logical
meaning, including under default negation. The additional complete
traversal spends the existing grounding work and substitution ceilings;
exhaustion refuses admission rather than accepting partial evidence.

Successful formula and bundle owners retain typed `FormulaWarning` values and
provide `warnings()` and `warning_view()`. Zero-divisor warnings are deduplicated
by original source span and ordered by source identity and span; they do not
count attempted rows. `FormulaLimits::max_warnings` defaults to 10,000, and a
new distinct warning beyond that bound produces `FormulaResource::Warnings`.
The CLI renders retained warnings once after admission. The
[numeric boundary](../reference/language.md#numeric-boundaries-and-refusal-meaning)
records the deliberate differences from clingo 5.8.2 and their rationale.

## Constraints over keyed values

A choice rule of the form `1 { p(K, V) : c(V) } 1 :- b(K).` holds, in every
answer set, exactly one atom `p(k, v)` for every `k` that `b` admits and no
other atom of `p`, when nothing else produces `p`. `zetesis_domain::keys`
reads these *keyed relations* from the normalized program: the key positions
are the body's variables, the value position the one variable the element's
condition binds. The analysis is syntactic and conservative; another producer,
other bounds, a second element, a value bound outside its condition or a
negated literal anywhere yields no key.

Preparation then asks a constraint over a keyed value as the one atom its key
admits. `:- G, p(k, Y), Y != t.`, with `Y` read nowhere else, `t` free of
`Y` and every key position of `k` naming its value, is prepared as `:- G, b(k), not p(k, t).`: whenever `b(k)` holds, exactly
one `p(k, y)` holds, and the written constraint fires exactly when that `y` is
not `t`, which is exactly when `p(k, t)` is absent. A column with a digit and a
carry, `:- G, p(k, Y), q(j, C), s != Y + 10*C.`, is prepared as two such
constraints demanding `s \ 10` of `p` and `s / 10` of `q`, when the facts of
the conditions binding the digit and the carry admit only digits in `0..9`
and only natural numbers, facts being all that produces them
(`zetesis_domain::facts`); the equation then has one solution, and for a
negative `s` no solution, in which case both forms fire. The product of the
demanded value with every value the key admits is never formed; the
[observation record](https://github.com/GregoryGelfond/zetesis/blob/main/docs/book/reference/observations/README.md#keyed-constraints-the-one-atom-the-key-admits)
measures the send-money puzzle's columns at one hundred instances each in
place of eighteen hundred forbidden combinations.

The rewrite additionally requires checked-evaluation evidence for the whole
written constraint. Bounds from fact-only positive predicates, or fact-only
conditions binding a keyed value, must establish the numeric type of each
arithmetic operand and an `i32` result at **every intermediate operation**.
The dividend `s` must also be numeric. An unknown type or bound leaves the
constraint as written. The proof is conservative: it uses intervals, does not
infer bounds from comparisons, and does not prove variable-exponent powers.
Arithmetic-free constraints need no numeric bound.

Checking the whole constraint preserves its source evaluation obligations.
For example, replacing `Y != 1` by `not p(k, 1)` must not expose `1/X` in a
substitution the written comparison excludes. Likewise, `Y + c*C` cannot be
removed when its multiplication or addition may overflow, even when the
equation is valid over mathematical integers. The original source still
governs errors, warnings and source locations when a rewrite is declined.
The [numeric boundary](../reference/language.md#numeric-boundaries-and-refusal-meaning)
states the deliberate differences from clingo 5.8.2; this optimization does
not relax that boundary or infer safety from the reference solver's behavior.

The written constraint is compiled once and its rules replaced in place by
the asked constraints', so nothing is prepared twice. The key analysis and
its readings of facts run under `FormulaLimits::max_key_work` and the term
work remaining, and their steps are charged to the term work;
`AdmittedFormula::key_analysis` says whether it completed or stopped, and a
stop leaves every constraint not yet asked as written, which changes no
answer set.

The replacement constraints are compiled under the remaining expansion budget.
Every asked statement carries the written constraint's source location and
the transformation's tag, so diagnostics and `formula_origins()` still name
the constraint as written. `keyed_constraints()` on the admitted formula
counts the constraints asked; the answer sets are the same either way, which
the contract tests state against the hand-asked program and against clingo.
A constraint outside the two patterns is left as written, and so is one
with an anonymous variable in a key position: the asked atom stands under
`not`, where `p(_, t)` holds when some key has the value `t`, so
`not p(_, t)` would forbid only that no key has it, while the written
constraint forbids a wrong value at every key.

## Know which program was analyzed

The analysis accessors are part of the prepared solver input's inspection
contract. `source_analysis()` describes exactly the value exposed by
`analyzed_program()`. Read `analysis_basis()` before interpreting its properties:

| Basis | Meaning of the retained program |
| --- | --- |
| `NormalizedProgram` | The bounded pool-free normalization of admitted source |
| `DependencyProjection` | A signature/polarity projection whose structural safety and class verdicts do not certify the original source semantics |

An unknown class verdict is not evidence that the program lies outside that
class. Neither kind of analysis establishes satisfiability or authorizes an
execution profile that the admission API refuses.

The distinction is observable for the adopted Boolean choice extension:
`2{#true;#true}2.` has one empty answer set. Its two written Boolean occurrences
must stay distinct in the formula solver's counting family.
By contrast, `2#count{1:#true;1:#true}2.` has no answer set: both elements name
the same complete tuple. Ordinary Boolean choices therefore retain a separate
source-occurrence family while exposing a dependency projection for analysis.
The explicit tuple aggregate does not require that projection by itself.

The following complete example checks preparation, analysis, materialization,
original source locations and the resulting families. It also inspects a located
resource refusal instead of treating failed preparation as inconsistency. The
last two source cases distinguish an empty program's one empty answer from the
empty family of an inconsistent program containing the constraint `:-.`.

```rust
# extern crate zetesis_solve;
# extern crate zetesis_cpu;
# extern crate zetesis_themelios;
{{#include ../examples/source.rs:example}}
```

Run it from the checkout root with:

```sh
cargo run --locked -p zetesis-solve --no-default-features --example book-source
```

## Preserve evidence through materialization

`formula_origins()` associates each emitted theory root with retained original
source locations. Locations identify source spans; they are not a proof of the
root's meaning. Some generated roots have different evidence needs from written
rules, so consumers must not assume that one root always corresponds to one
written statement. The admitted owner also retains objectives, observations and
the completed projection domain separately from theory roots. Authored
`project_selection()` metadata is a declaration plan; the admitted owner's
`projection()` supplies the fixed typed domain for
[projected enumeration](sessions.md#projected-enumeration).

The example checks that emitted root evidence retains the source identity
supplied in `AdmissionOptions`. It leaves syntax traversal to themelios's API
documentation. For the internal occurrence catalog, tuple activity and their
proof obligations, see [source identity in grounding](../architecture/grounding.md#preserving-source-identity).

The public implementation declarations are
[`PreparedFormula`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula/preparation.rs),
[`AnalysisBasis` and `AdmittedFormula`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula.rs).

## Work and retained space

Preparation traverses and retains bounded source/analysis structure; source
occurrence retention and pool expansion consume cumulative expansion resources.
Boolean choice retention charges each selected rule's source span and syntax
nodes as work, its node bound as Values, and a conservative four locations per
node as Origins before copying. Raising and metadata diagnostics retain their
precedence over these copy limits. The original occurrence stream and the
additional selected-rule copies coexist temporarily; this is not streaming
admission or a zero-copy guarantee.
`max_analysis_nodes` bounds visited structure, while `max_analysis_edges` bounds
head/dependency occurrence products before graph allocation. These are different
from source-byte limits and do not bound process RSS.

Grounding then completes possible support and enumerates admitted outer/local
bindings. Join work may grow as a product of relation sizes, and general
aggregate formula construction may be combinatorial even when the final atom
catalog is small. `FormulaLimits` separately bounds substitutions, construction
work, support rounds, caches, origin locations and final atoms/nodes/roots.
Retained space includes those structures and their logical value payloads;
allocator overhead is outside these logical counters. A successful preparation
therefore promises neither cheap grounding nor a smaller retained representation;
the hybrid schedule still completes this support before streaming constraints.

See [source grounding](../architecture/grounding.md) for binding and coverage
invariants and [completion and resources](outcomes.md) for result accounting.

## Validate values before materializing them

Source normalization, fact sizing and head preflight use a shared checked symbol
walk. A validation consumer checks the value without building an owned core
value or rendering its spelling. A construction consumer creates the flat node
owner and enters the existing independent core constructor.

[`ValueNode::view()`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/structured.rs)
returns a borrowed `ValueNodeRef`. Its text and canonical-spelling measures are
shared by source validation and owned construction. The view owns no payload;
it retains node kind, constructor sign and arity, and borrows exact text. It is
not a certificate that the surrounding preorder sequence forms a valid value.

Validation retains logical node/depth bounds and the byte bound for node cells,
referenced text and canonical spelling. Construction additionally admits its
actual node capacity, validation/render frame capacity and rendered-string
capacity. Validation does not charge storage for an output that is never built;
passing it does not guarantee that later materialization fits its actual storage
budget. NUL strings, invalid arithmetic and overflowing arithmetic remain
refusals even in an inactive rule or unused constant. Complete authored-body
validation still runs after an earlier false filter.

## Diagnostic ownership

Typed refusals retain source identity and original error data. Their `Display`
implementations write to the caller's formatter and propagate its failure.
Constant-cycle messages stream the retained names in dependency order without
first joining them into another owned string. A caller can impose a byte ceiling
through its formatting sink; formatting stops at the first refused write.

The themelios human diagnostic API returns an owned rendering for each diagnostic.
zetesis keeps that canonical view. Source indexing, typed diagnostic messages and
that rendering can require temporary storage before the caller receives text;
the sink's byte ceiling does not bound those upstream allocations. Constructing
an owned diagnostic view remains distinct from streaming an already-retained
constant chain.
