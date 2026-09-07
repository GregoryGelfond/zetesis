# Factored source formulas

This contract governs the implemented finite source-factorization transform.
The transform reduces repeated construction of the same consequences while
retaining the exact Ferraris acceptance criterion. It recognizes variable
relationships, not domain or predicate names.

## Fixed head arguments separate local work

Consider this rule:

```clingo
linked(X,Y) :- left(X,A), tail(A,Y), right(X,B), cap(B,Y).
```

For fixed `X,Y`, local variable `A` and local variable `B` belong to separate
body components. Define their eligibility formulas:

```text
L(X,Y) = OR over A of (left(X,A) AND tail(A,Y))
R(X,Y) = OR over B of (right(X,B) AND cap(B,Y))
```

The rule instances can then be represented by one implication:

```text
(L(X,Y) AND R(X,Y)) -> linked(X,Y)
```

These names describe formula roots; they are not new semantic atoms. Combining
independent existential components avoids the Cartesian product of their local
witnesses. If there are `a` and `b` bindings for the two components, construction
visits `a+b` local bindings instead of `a*b` complete combinations. This is a
statement about construction work, not an end-to-end speedup guarantee.

## Planning and coverage

The planner fixes normal-head variables, then groups body elements connected by
remaining variable occurrences. Constraints have no fixed head variables. Every occurrence in atoms, scalar expressions,
negative projections and other supported conditions must participate in that
dependency calculation. A condition sharing a local variable joins its components;
separation cannot be inferred from predicate names or textual adjacency.

Final formula construction enumerates matching normal-head tuples from the
completed possible-positive relation. This requires its existing coverage
contract: every true head of any answer set is present. A tuple introduced by a
different producer is harmless only when all body and supportedness formulas are
still retained. A partial relation cannot justify this enumeration.

Each local component joins the same immutable relation, constructs the original
eligibility formulas, and reduces them with OR. The component roots combine with
AND before implication to the original head. An empty local result is false;
independence does not create a missing witness. Original source origins remain
attached to the resulting rule and support formulas.

The current planner admits atoms, anonymous negative projections, and scalar/flat
tuple comparisons whose terms are constants or variables. Generators, aggregates
and arithmetic-bearing comparisons use the ordinary finite lowering path. Unsupported dependency shapes never authorize approximate
factorization. Limits on planning, components, row visits, scalar payloads and
formula nodes remain explicit.

## Two separate optimizations

Possible-positive support discovery only needs one complete positive/filter
witness to add a normal head tuple. When a partial join already fixes that head
and the tuple exists in the current relation or round delta, the remaining local
search can be skipped. Generated head arguments must actually be bound before
this check. The relation remains unchanged during each synchronous round; a
current delta is observed only for the duration of that join operation.

Final formula construction has a stronger obligation. It must preserve every
eligible source witness, including alternative and recursive supports. It cannot
reuse the support-only duplicate-head shortcut. Component OR formulas retain
those alternatives compactly.

## Reduct preservation

Classical truth equivalence alone does not justify a source-theory rewrite. The
factorization must preserve the original candidate model test and the reduct
model test for every candidate and every proper subset considered by the oracle.
Finite distributivity and implication over disjunction supply the logical basis;
the concrete source planner must establish the variable-independence hypotheses.

Default negation stays inside each original eligibility formula. It is not
reevaluated under a subset after freezing the candidate. A candidate is accepted
only when no proper subset models its reduct. Objectives continue to evaluate
verified answer sets and cannot provide support.

Fourteen `RuleFactorization` theorems establish the two-family original-truth,
frozen-reduct and surrounding-theory stability laws. They assume the Cartesian
family shape and fixed head. The proof registry records their precise scope. Abstract
factorization laws do not by themselves verify source scopes, Rust indices,
allocation limits, hashing, shaders or device execution.

## Hardware direction

The resulting operations are bounded selection, joins, local Boolean reductions,
DAG composition and frozen-reduct checks. This representation exposes independent
rows and candidate batches for later device execution. It does not itself execute
on a GPU; the existing Metal backend still implements its separately qualified
static normal-program specialization. Device-resident formula evaluation and
coverage-preserving distribution remain distinct implementation obligations.
