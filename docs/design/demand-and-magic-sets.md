# Observation demand and magic sets

In the absence of an explicit query, `#show` is an implicit observation query
from the domain modeler. The proposed demand planner starts from those requested
observations. This gives display metadata an additional planning role without
changing its existing enumeration semantics.
The companion [program interface contract](program-interface.md) records the
modeler's `#defined`/`#show` input/output convention and separates declared intent
from analyzed producer roles and input assumptions.

This is design work. No production magic-set transformation, demand-pruning
option or broader query API is implemented by this note. Compatibility remains
the active milestone; the current solver's exact source and reduct contracts
remain in force.

## Two analyses over the same program

The shared input is the typed themelios `Program`, with its parts, scopes,
statements and provenance. [Domain analysis](domain-analysis.md) computes upper
bounds on values that may occur. Demand analysis identifies the bindings needed
to investigate a specified observation or semantic obligation. The first is
global possible-value information; the second is relative to a task and possibly
a candidate or completed splitting interface.

Magic sets provide an established way to propagate query bindings through rules
and express demand as relations. ASP-specific Dynamic Magic Sets also exploit
relevance during nondeterministic search. Their published results establish
query preservation for particular program classes, including disjunctive programs
with stratified negation, and a broader super-consistent class. The latter means
consistency is retained after adding any set of facts. These theorems do not
establish arbitrary clingo aggregate, choice or optimization transformations.
See [Alviano et al.](https://arxiv.org/abs/1204.6346) and
[Alviano and Faber](https://arxiv.org/abs/1011.4377).

The architecture should support demand propagation, with magic-style rewriting
as one possible implementation. It is not required for correctness. On selective
workloads it may avoid extensive construction; if most of the program matters,
its maintenance cost may outweigh the savings. This is a hypothesis to measure,
not a claim of a zetesis speedup.

## Observation seeds and completion obligations

An explicit future query supplies the observation seed. Otherwise use the
program's display selection:

- Signature `#show` requests seed the corresponding signed predicate and arity.
- Future term/conditional displays also demand their value expressions and
  condition dependencies, with preserved local variable scopes.
- With no display restriction, use the default display policy.
- Empty `#show` has no displayed-value demand; it does not remove satisfiability,
  optimization, hidden-model enumeration or count obligations.

Observation demand is only one source of work. Constraints, original-theory
consistency, objective tuple/priority eligibility and full-model completion
supply additional obligations. A hidden component can be irrelevant to a shown
value while still changing whether answers exist, how many there are, or which
are optimal. Candidate generation and the reduct oracle can introduce further
binding demands as they encounter unresolved obligations.

For default clingo-compatible enumeration, defer a hidden component or solve it
through an applicable decomposition. Do not permanently discard it merely
because its atoms are absent from `#show`. A completed extension must retain
distinct full models, even when several have the same display. Any optimization
plan must also preserve global costs and every requested optimal tie.

For a future explicit brave/cautious query task, the contract can instead be
preservation of those query answers under a proved fragment restriction. Even
preserving each observed atom's brave/cautious truth does not in general preserve
their joint displayed models or multiplicities. Projection and display are
distinct contracts; choosing a query mode must be explicit in the shared API.

## Small semantic checks

These examples were checked with external clingo 5.8.2. They illustrate why the
planner needs both observation seeds and completion obligations; they are not
counterexamples to magic sets under their stated hypotheses.

```asp
q.
{a}.
#show q/0.
```

There are two stable models, `{q}` and `{q,a}`, displayed twice as `q`. Removing
the choice leaves one. The observation value is unchanged, but full enumeration
and the displayed multiset differ.

```asp
q.
a :- not a.
#show q/0.
```

There are no stable models. Removing the disconnected odd cycle would turn the
program into a satisfiable one, so a dependency walk from shown predicates alone
cannot certify whole-program consistency.

```asp
q.
1 {a;b} 1.
#minimize {1@1:a; 1@1:b}.
#show q/0.
```

There are two optimal models at cost 1, both displayed as `q`. A complete `optN`
comparison must retain both after accounting for clingo's initial incumbent
replay. Observations alone cannot determine objective presence, costs or ties.

## Demand as compositional operations

A proposed demand item contains the shared predicate signature, its bound/free
argument pattern, the bound values, its source/task identity and its scope.
Scopes distinguish program-wide observation demand from a fixed candidate or
completed splitting interface. Source and callback context versions participate
in cache identity. This representation need not invent an alternative atom or
symbol vocabulary in the analysis crate.

The execution primitives are bounded producer lookup, binding propagation,
indexed joins or semijoins, scalar filters, projection, exact deduplication and
worklist fixed points. A binding-passing strategy controls the order in which
information flows through a rule. Disjunctive sibling heads, negative occurrences,
aggregate elements and local conditions require their precise semantic rules;
an ordinary positive dependency walk is not sufficient for all of them.

These operations can retain delta frontiers and batch equal-shaped work on CPU
or GPU. A GPU plan needs enough useful work to pay for indexing, transport and
synchronization; a smaller demanded relation can favor a sparse CPU plan. Device
selection follows applicability and measurements. No dense execution is required
merely because the program has a GPU backend.

Keep demand relations as execution metadata initially. They request construction
or prioritize candidate exploration; they do not become source facts supplying
support. If a later implementation rewrites source rules with magic predicates,
it needs the corresponding source-to-reduct and model correspondence proof.
The original reduct remains the acceptance criterion.

## Proof obligations and implementation order

Before pruning, establish the semantic contract for the chosen task:

1. Every relevant source binding and every possible witness that could change
   the result is covered by the demand closure or by a completed residual task.
2. Each proposed full candidate is checked against the original theory's reduct.
   This protects acceptance soundness; it cannot recover a valid answer that an
   unsound demand restriction prevented the generator from proposing.
3. Whole-model enumeration has sound and complete extension accounting, including
   hidden multiplicities and objective contracts. A query-equivalence theorem
   alone is insufficient.
4. Incomplete worklists, failed callbacks, timeouts and device failures leave
   explicit unfinished obligations. A missing demanded row is not proof of
   falsity or exhaustion.

The initial useful implementation can build a bounded demand graph and record
its seeds and binding patterns without pruning. Next, prioritize existing exact
construction while preserving exhaustive completion. A restricted positive
relational transform or a certified split can then supply the first pruning
theorem. General Ferraris source support requires further laws, especially for
choices, aggregates, disjunction and objectives.

Lean should distinguish query preservation, full stable-model correspondence,
residual extension coverage and exact reduct-query preservation. The existing
lifted composition, upper-bound, final-coverage and feedback proofs supply useful
pieces; none currently proves a magic-set source transformation. General
super-consistency recognition is itself computationally expensive, so a planner
should use checked sufficient conditions or fall back conservatively rather
than treat an unknown classification as permission to transform.
[Alviano, Faber and Woltran](https://arxiv.org/abs/1212.5895)

The [feedback and decomposition design](feedback-and-decomposition.md) records
how reduct witnesses and splitting interfaces can interact with this demand.
Keep those mechanisms separate from the reusable global domain analysis.
