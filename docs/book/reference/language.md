# Admitted language

zetesis targets clingo-compatible source and answer-set semantics within an
explicit scope. Parsing a construct does not establish that its current
lowering, grounding or execution profile supports it. The tables below describe
implemented forms and their boundaries; they are not a whole-language parity
claim or a promise that every combination of individually supported forms works.

The ordinary source driver selects a supported profile automatically. Explicit
library admission doors are narrower: `admit` accepts the strict relational
profile, `admit_extended` adds its scalar expansions, and `admit_formula` builds
the broader finite Ferraris representation. General formula grounding remains
eager. Selecting a GPU does not expand the accepted source language.

## Rules, terms, and bindings

| Form | Implemented scope | Remaining boundary |
| --- | --- | --- |
| Normal rules and constraints | Safe finite relational rules, default and double negation, singleton unbounded choices | More general constructs use the formula profile |
| Strong negation | Signed atom identities and coherence constraints; signed source and observation forms | Remaining constructor and condition profiles still apply |
| Disjunction | Finite signed/evaluated disjunctive heads, including admitted top-level numeric intervals; direct formula compilation without shifting | Nontrivial conditional disjuncts, unsupported nested pools and objective-dependent producers |
| Boolean literals | Signed `#true`/`#false` in rule bodies and choice/aggregate conditions; Boolean singleton heads and Boolean disjuncts with empty or explicitly true conditions | Boolean choice/head-aggregate elements and separate objective/observation condition profiles |
| Logical values | Closed signed functions, tuples, strings and extremal terms; complete variable copying, finite construction from bound inputs and structural comparisons | Nested pools/intervals and broader expression contexts remain restricted |
| Positive witnesses | Constructor/tuple patterns preserve sign, name, arity and complete supporting atoms; evaluated positions consume bound inputs | Arithmetic inversion; a negative atom cannot supply a missing binding |
| Comparisons | Equality/disequality, structural ordering, admitted flat-tuple equality and complete comparison chains, including default/double negation | Several unresolved variables in a generating chain and broader inverse binders |
| Finite generators | Scalar equality, admitted flat-tuple equality, closed integer bounds, dependent intervals and finite scoped rule/head pools | Unsupported body/local pool contexts and broader nested interval construction |
| Universal body conditionals | Complete local implication families with signed consequent alternatives and positive local witnesses | Negative anonymous consequent witnesses, unsupported local generators and objective-dependent conditional producers |

For example, `q(X) :- d(X), p(X+1).` checks the complete supporting `p` atom
after `d(X)` binds `X`. By contrast, `q(X) :- p(X+1).` requires arithmetic
inversion and receives the typed `UnboundArgumentInput` refusal.

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
The maintained [witness tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/evaluated_witnesses.rs)
exercise these distinctions.

## Choices and aggregates

Unbounded and integer-bounded choices support scoped variables, duplicate
eligibility, recursive conditions, evaluated arguments and admitted top-level
numeric intervals. Complete outer scalar or acyclic aggregate-assignment values
can supply bounds; their original equalities and whole-group activation remain
in the resulting formulas. Symbolic bounds and unsupported local generators
remain implementation gaps.

| Aggregate form | Implemented scope | Remaining boundary |
| --- | --- | --- |
| Body `#count`, `#sum`, `#sum+` | Finite comparisons, recursive eligibility, complete-tuple coalescing and acyclic fresh-target assignments | Cyclic/self-dependent assignment generators, unsupported local generators and objective dependencies |
| Body `#min`, `#max` | Comparisons over complete logical values; empty extrema; admitted acyclic assignments | Numeric endpoint guard below and unsupported consumer/observer combinations |
| Assignment consumers | Dependency-ordered scalar/tuple filters and equalities, evaluated positive arguments/heads, admitted outer negative atoms, finite outer ranges, integer choice bounds, nonbinding aggregate guards and universal conditionals | Broader local scopes, objective-relevant new consumers and objective-relevant multiple assignments |
| `#count` heads | Permission coalesced by head atom and activity coalesced by complete tuple, including both alias directions | Unsupported eligibility/observer contexts |
| `#sum`, `#sum+` heads | Signed numeric `#sum` and nonnegative numeric `#sum+`; permission coalesced by head atom and activity coalesced by complete tuple, including both alias directions; zero-weight heads retain permission | Missing/nonnumeric measured values, negative `#sum+` weights, default-negated derived elements, unsupported eligibility contexts and objective-relevant heads remain refused |
| `#min`, `#max` heads | Numeric first tuple values, with a bijection between complete tuple and head atom | Extrema aliases, missing/nonnumeric measured values, the numeric endpoint guard, default-negated derived elements, unsupported eligibility contexts and objective-relevant heads remain refused |

A tuple contributes its weight once when any of its eligible head occurrences
is selected. Thus `1#sum{1:a;1:b}1.` admits `{a}`, `{b}` and `{a,b}`. Distinct
complete tuples remain distinct contributions even if they share an atom:
`3#sum{1:a;2:a}3.` admits `{a}`, as does `2#sum{1,k:a;1,l:a}2.`. Conditions
retain their model-relative truth and reduct implications. Atom permission is
independent of contribution, so `0#sum+{0:a;0:b}0.` retains all four choices.

Acyclic aggregate assignments may depend on earlier assignments through scalar
and range values. A proposed value remains guarded by the original aggregate
equality. Membership in possible support cannot replace model-relative aggregate
truth. The numeric-first-value restriction on extremum **heads** is narrower
than the complete-value extremum **body** profile.

## Objectives and observations

`#minimize`, `#maximize` and weak constraints use complete tuple identity and
priorities. Search preserves all optimal ties within its resource and delivery
limits. Positive lifted objectives and supported total aggregate observers can
pass through unique acyclic relation renamings or argument permutations.
Resolved nonnumeric literal weights contribute neither cost nor priority only
after the element's remaining structure is validated. Flat unary mixed-extrema
observers have a separate presence certificate; arbitrary filtered, multiple,
negative, disjunctive or conditional producer patterns do not inherit it.
Dynamic priorities, additional objective conditions and broader mixed-extrema
profiles remain restricted. In particular, accepting an extremal term as atom
data does not imply accepting it in every objective field.

`#defined` and `#show` retain signed signature metadata. Ground and admitted
conditional term observations operate over full models; they do not change
answer-set identity. Broader observation expressions and conditions remain
separate admission obligations. Projection through `#project` is not implemented
and is not silently treated as `#show`.

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

Reached undefined or overflowing scalar operations produce located refusals.
The formula source API does not emulate clingo's warning-and-drop behavior for
such operations. A syntactically false condition is not permission to skip
required source validation.

There is also a conservative **zetesis extrema guard**: numeric values
`-2147483648` and `2147483647` are refused when reached as `#min`/`#max` bounds or
the first component of a possible aggregate tuple, including assignment values.
It does not ban these integers as ordinary data, later tuple-key components or
integers nested inside a structured value. The guard applies independently of
the comparison operator or recursion.

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
and [source regressions](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/tests/extrema_source.rs)
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
