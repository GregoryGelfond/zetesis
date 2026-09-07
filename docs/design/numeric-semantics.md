# Numeric boundaries and admission

The intended language contract and the currently implemented source profile are
separate. A zetesis refusal can expose unfinished implementation or unresolved
semantic correspondence; it does not establish a knowledge-representation error
or automatically justify excluding the input from the compatibility target.
The reduct remains the permanent criterion for accepting answer sets.

## Preserve the reason for rejection

| Boundary | What the failure establishes | What it does not establish |
| --- | --- | --- |
| themelios syntax parsing | The input could not be parsed under the pinned grammar. | That every downstream refusal is a syntax error. |
| themelios program raising or evaluation | The parsed construct could not be represented or evaluated under that operation's contract. | A general proof that the model is invalid; the reason may be a representation limit. |
| zetesis admission or lowering | The requested source operation could not be admitted by the current implementation. | A themelios failure, UNSAT or a modeling error. |
| Explicit project exclusion | A separately agreed feature is outside the intended target, such as embedded Python/Lua scripting. | Permission to reclassify every unimplemented feature as excluded. |

Report the actual failed operation and its reason. If an intended construct is
valid and themelios can represent it, closing zetesis's implementation gap remains
an obligation. Genuine source errors, undefined operations, numeric representation
limits and missing solver mechanisms must not be conflated. A refusal can prompt
a review of modeling intent, but its origin and cause determine that advice.

## Current extrema guard

The zetesis source bridge refuses reached numeric values `-2147483648` and
`2147483647` when they are evaluated as a `#min` or `#max` comparison bound or as
the first component of a possible aggregate tuple. This includes possible values
used to generate aggregate assignments. The predicate is independent of the
comparison operator or whether the aggregate is recursive.

This is an internal admission guard in **zetesis-themelios**, distinct from
parsing with the unchanged themelios parser. It is not a blanket ban on these
integers as ordinary data. Later tuple components identify elements; the guard
checks the first component, which supplies the extremum value. An endpoint nested
inside a structured value does not match this numeric predicate.

There is an earlier, separate boundary: the pinned themelios program raiser cannot
represent the unsigned numeral magnitude in the literal `-2147483648`. The
expression `(-2147483647-1)` can produce that value during evaluation and reach
zetesis's extrema guard. The former is a raising limitation; the latter is a
zetesis refusal. Neither is a syntax-parser rejection. No themelios modification
is part of documenting these boundaries.

The current guard protects a known uncertainty: six retained recursive examples
produce different model sets in clingo 5.8.2 and the native mathematical extrema
oracle. Other endpoint cases agree, but the guard conservatively covers both.
That protection is useful while the discrepancy is investigated; it does not
establish a permanent language exclusion or prove that the rejected programs
have undefined source semantics.

For example, an existing source regression refuses:

```asp
p :- #min {2147483647,k:not p} != 2147483647.
```

The refusal is neither UNSAT nor an empty answer set. The current extrema guard
produces a located aggregate-profile diagnostic; it does not yet provide a
dedicated endpoint code with the value and a remediation hint.

## Modeling intent and implementation work

A machine integer limit may have been used to mean infinity, which would merit
review of the encoding. It can also be legitimate finite domain data. The solver
cannot infer either intent merely from encountering the number.

`#inf` and `#sup` are the least and greatest terms in ASP's term order, distinct
from finite integer limits. Empty `#min` and `#max` assignments retain actual
`#sup` and `#inf` values respectively. The
[Potassco language guide](https://github.com/potassco/guide/blob/master/language.tex)
describes those terms and the empty aggregate values. Replacing a finite endpoint
with an extremal term or a neighboring integer is not a semantics-preserving
repair in general. zetesis must not silently make such substitutions.

Closing the endpoint gap requires establishing the intended source and reduct
semantics, explaining the recorded divergence and implementing checked handling
with complete original/frozen-model regressions. Preserve separate tests for
evaluated endpoints, adjacent integers, comparison operators, recursion, empty
aggregates and assignments. A changed upstream result requires investigation;
it does not by itself prove native correctness or justify removing the guard.
If an intentional semantic divergence is ultimately warranted, record that as
an explicit decision with evidence rather than inferring it from current refusal.

## Evidence and accounting

- [Source refusal regressions](../../crates/zetesis-themelios/tests/extrema_source.rs)
  retain original examples, located failures and empty-aggregate assignments.
  Their literal minimum-integer example exercises the earlier raising boundary;
  it does not isolate the evaluated minimum-endpoint guard.
- [The admission predicate](../../crates/zetesis-themelios/src/formula_assignment.rs)
  checks evaluated numeric endpoints; grounding also applies it to comparison
  bounds and first tuple values.
- [Independent oracle characterization](../../crates/zetesis-ferraris/tests/extrema_clingo.rs)
  preserves all six discrepancies separately from equivalence comparisons. The
  lower-level mathematical oracle can represent endpoint formulas; the source
  admission guard is not a restriction on that library.

Keep original sources and observed model sets intact. A test can pass its refusal
contract without passing answer-set parity. Report internal implementation gaps,
agreed exclusions and unassessed inputs separately; none should disappear from
the evidence merely because a safer failure replaced an incorrect answer.
