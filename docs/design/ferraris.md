# Ferraris/clingo semantic extension

This is the forward semantic target for full support of the original non-clingcon
`kr-domains` programs. It supplements the [S0 design](zetesis.md); it does not
claim that the current Rust frontend accepts the richer source language.
The [corpus compatibility contract](../verification/kr-domains-compatibility.md)
defines the required original cases. Source details and semantic qualifications
are collected in [Ferraris/clingo sources](ferraris-sources.md).

## Two exact oracle profiles

The current S0 profile normalizes rules into frozen gates and Horn consequences.
Its positive reduct has a least model, so an exact least-closure computation plus
constraints and seed-projection equality decides stability. This remains a useful
specialization with a straightforward GPU and event-counter implementation.

The broader profile works with a finite propositional theory T obtained through
a source translation matching clingo's semantics. Its candidate M is an
interpretation of the translated theory, including any internal atoms. The
Ferraris reduct T^M replaces maximal subformulas false in M with false. Stability
requires M to be a subset-minimal model of that frozen reduct:

```
stable(T, M) iff satisfies(M, T)
                 and no J strictly contained in M satisfies T^M
```

The oracle must distinguish these profiles in its input type. A future
`FormulaTheory` must not be passed through the current `Program`/`Gamma` interface
merely because its operators have numerical implementations. A compiler may
select the Horn closure specialization only when its translation establishes
the required least-model property and faithful source-model correspondence.

## Formula and reduct primitives

The finite formula vocabulary consists of atoms, false, conjunction, disjunction
and implication. Negation is implication to false. The implementation boundary
has three distinct operations:

1. `Evaluate(M, F)` computes classical formula truth under the candidate.
2. `FreezeReduct(M, F)` constructs or streams the residual formula, replacing a
   maximal M-false subformula with false and recursively retaining satisfied
   structure. Truth-cache identities include the entire candidate epoch.
3. `EvaluateSubset(J, F^M)` checks a possible minimality counterexample. It uses
   the already frozen outer candidate M; it does not take a new reduct under J.

`not not a` therefore becomes a frozen truth condition from M. A choice formula
`a or not a` reduces to a requirement for `a` when M contains it, and imposes no
such requirement when M omits it. These facts explain the S0 singleton-choice
normalization without making a choice head its own positive support.

An arbitrary residual formula need not have a least model. For example, `a or b`
has incomparable minimal models `{a}` and `{b}`. Their intersection is not a model.
An intersection or one monotone closure cannot serve as the general oracle.
This mathematical boundary is independent of which hardware evaluates formulas.

## Signed atoms and coherence

Classically positive and negative occurrences of the same predicate and tuple
are distinct atoms: `p(t)` and `-p(t)`. Default negation remains a connective over
that identity, so `not -p(t)` does not become `p(t)`. The implemented source
boundary supplies coherence constraints `not (p(t) and -p(t))` for opposite
identities in its complete ground catalog. Relational S0 admission supplies the
corresponding lifted integrity templates. Coherence removes inconsistent
candidates before scoring or observation; it neither derives a polarity nor
requires one to be present. The core atom representation alone imposes no
source-level coherence policy.

[StrongNegation.lean](../../proofs/Zetesis/StrongNegation.lean) defines an
executable connective-preserving renaming and proves exact reduct transport.
An injective encoding preserves stable models, including the converse that a
stable target interpretation contains no unencoded atoms. The finite coherence
transform preserves precisely the coherent original stable models, provided its
registry covers every co-present opposite pair. A supplied sound upper carrier
and complete pair registry establish that premise; arbitrary partial grounding
does not. Whole bounded groups retain their constraint-only bounds and original
context. A checked omitted-pair example exhibits the failure without coverage.

These laws operate on complete ground base atoms and arbitrary Ferraris
formulas. They do not verify parsing, signed tuple interning, support completion,
Rust constraint generation, objective/display matching, numerical limits or
hardware execution. The [source tests](../verification/strong-negation-20260906/README.md)
record those executable boundaries separately, including the clingo 5.8.2 unsafe
signed anonymous default-negation forms.

## Aggregate translation is a semantic boundary

The source translator must implement the chosen clingo-compatible translation
before applying the formula reduct. It must preserve the distinction between
aggregate elements, tuples, their conditions, and scoped local variables.
Repeated witnesses for the same aggregate tuple do not create extra tuple weight.
Conditions supporting one tuple form alternatives, while the tuple key governs
coalescing. The same distinction matters for optimization statements.

Required cases include `#count`, `#sum` and `#max`, aggregate assignments, guarded
and default-negated cardinalities, and conditional bounded choice heads. Aggregate
assignment can introduce values into heads and later domains. Negative or mixed
weights, empty aggregates, ties, local anonymous variables and recursion require
explicit rules and tests. The exact scope of the supported translation must be
recorded; classical aggregate truth equivalence alone does not establish stable
model equivalence of a rewrite.

It is particularly unsafe to replace an aggregate in the reduct by “evaluate the
original aggregate again under J.” That defines a different checking procedure
unless a translation theorem justifies the substitution. Ferraris-style and FLP
semantics must not be conflated. The general oracle consumes the translated
formula reduct, while specialized aggregate transformers require a refinement
argument for their admitted source class.

The initial implementation sequence should preserve a direct finite formula
translation as a reference. Integer reductions and compressed aggregate
representations may then be compared with it before entering the trusted oracle.
The Rust kernel implements finite count/sum formulas and a numeric min/max
translation. Source admission supports finite count/sum/sum+ and numeric min/max comparisons
and scoped equality assignments within its explicit scalar and resource profile. Threshold recurrence and finite assignment coverage
have Lean proofs; aggregate strong equivalence and the source/compiler
refinement remain unproved. Kernel tests record six clingo numeric-endpoint
discrepancies separately from successful equivalence comparisons.

There is a useful route to a compact transformer. Ferraris's 2005 Proposition
7(b) characterizes truth of an aggregate reduct using the frozen reducts of its
element conditions: first require that M satisfies the aggregate, then evaluate
the numeric guard on the weights whose condition reducts hold in J. This avoids
materializing its potentially exponential propositional expansion. It is not
the unsafe operation of re-evaluating the original conditions under J. The
proposition concerns that paper's aggregate representation; transferring it to
clingo's whole-tuple set semantics requires a separate coalescing and translation
argument. See the [primary-source analysis](ferraris-sources.md).

## General minimality checking

The general oracle first rejects M if source-theory truth fails. Otherwise it
searches for a proper subset J of M that satisfies the frozen reduct. A found J
is a compact rejection witness that can be checked independently. Failure to
find J is an acceptance certificate only after exact coverage has completed.
An interrupted subset search yields `Unknown`, even when M itself is a model.

The inner search evaluates formula satisfaction, rather than recursively asking
whether J is a stable model. This avoids a circular solver definition. A simple
finite exhaustive subset search supplies a correctness reference; a practical
implementation can use sound propagation, conflict learning and hardware batches.
All pruning must preserve possible satisfying subsets. The interface separates:

- source-model rejection;
- a verified proper-subset counterexample;
- completed minimality verification;
- interruption, resource refusal or device failure.

The outer candidate generator can still be lazy and heuristic. Full-model
candidates may be represented by compressed supported consequences plus decisions
when a reconstruction contract proves this sufficient. The existing S0 seed
carrier is not automatically a sufficient decision space for every aggregate
theory. Newly discovered atoms, aggregate tuples and source instances must be
included in final coverage before either truth or minimality can be certified.

## Transformer placement

The broader primitive family is `Bind`, `EvaluateTerm`, `Filter`, `CoalesceTuple`,
`ReduceAggregate`, `EvaluateFormula`, `FreezeReduct`, and `SearchSubset`, together
with exact publication, projection and completion operations. These are semantic
contracts; fusion and batching are implementation choices.

GPU execution can batch outer candidates and inner subsets, with distinct outer
candidate and subset identities. Frozen M truth must never be overwritten by J
truth. Formula structure, tuple grouping and proven propagation can be shared;
mutable truth and rejection witnesses are world-local. Integer overflow,
reduction order, transport completion and memory limits remain explicit.

Neuromorphic support counters remain valid for Horn residual regions. General
formula checking needs additional truth-state and implication/minimality
machinery. An event fabric's quiescence alone certifies neither source coverage
nor absence of a proper-subset witness. Native profiles must qualify this wider
contract before claiming full-domain support.

## Optimization and observable results

Optimization operates over verified stable models. Its objective tuples use the
source's identity and set semantics, with lexicographic priority ordering. An
incumbent is distinguished from a proved optimum; a timeout cannot turn the
best model seen into an optimal result. Enumeration of all optimal models and
cautious consequences over optimal models requires its own completed coverage.

`#show` is an observation layer applied after semantic checking. Full internal
models, shown witnesses and model counts are distinct quantities; identical
shown projections can arise from distinct full models. The corpus contract
defines which counts and witnesses are compared with clingo. Includes, constants
and `#defined` must retain their source behavior and origins rather than being
silently discarded at admission.

## Verification sequence

The Lean package establishes the S0 least-closure contracts and formula-level
Ferraris reduct laws. Its denotational extensions cover bounded choice groups,
signed identity/coherence, finite assignment coverage, objective normalization,
and completed classical witness validation. These results preserve their
explicit carrier, binding, interpretation and cost assumptions. They do not
verify source translation of those constructs, source arithmetic or includes,
or the Rust implementation. The [proof scope](../../proofs/README.md) records
each module's premises and remaining representation obligations.

Next proof obligations are source-to-formula correctness on the admitted finite
fragment; scoped tuple/coalescing and aggregate translation; faithful auxiliary
atom projection; minimality-search coverage; and the correctness of specialized
Horn/aggregate execution plans. Complete corpus conformance is a separate
executable acceptance requirement, using clingo as an external oracle.
