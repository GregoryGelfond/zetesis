# Feedback, decomposition and domain analysis

Candidate generation and exact membership should exchange semantic evidence.
The original program version and reduct remain the acceptance boundary. This
note separates implemented feedback from further transforms suggested during
development; none of the proposed transforms is silently enabled by this design.

| Mechanism | Current status | Evidence required |
| --- | --- | --- |
| Exact semantic candidate blocking | Implemented | The exact original-atom candidate has been accounted for |
| Incumbent cost bounds | Implemented | Verified stable incumbent, exact objective eligibility and a bound retaining ties |
| Failed-literal probing in refined candidate regions | Implemented | Complete branch conflict in the fixed candidate query |
| Possible-support relations, indexed joins and body factorization | Implemented for stated profiles | Completed upper relation and source/transform applicability checks |
| Antichain cone pruning | Proposed | Certified antichain property for the admitted semantic theory and fixed carrier |
| General countermodel feedback formulas | Lean semantic laws checked; no production compiler | Exact fixed-witness residual transform and original-theory identity |
| Splitting and interface caches | Proposed | Applicable splitting/module theorem and completed conditional coverage |
| Abstract type/domain refinement | Proposed beyond existing support/scalar analysis | Sound overapproximation, with explicit scope and transfer functions |

## Accepted answers and antichains

Ordinary normal programs, and ordinary disjunctive programs with positive heads
and flat default-negated bodies, have subset-incomparable stable models. In this
fragment a verified answer `S` permits exclusion of its proper subsets and proper
supersets from subsequent answer enumeration. For a fixed finite carrier `U`,
the two excluded cones can be blocked by requiring respectively
`OR(a : a in U minus S)` and `OR(not a : a in S)`, after `S` itself has been
returned or retained according to the enumeration/optimization contract.

For an outer generator already enumerating classical models of an ordinary
program, proper-subset exclusion is redundant: its stable models are also
subset-minimal classical models. Proper supersets can still be classical models,
so excluding those is the more useful direction for that generator. The size of
a Boolean cone is not a measured search saving; propagation may already avoid
most of it. Benchmark actual decisions, candidate checks, storage and elapsed time.

The property does not hold for arbitrary Ferraris theories. Both `{ a }.` and
`a :- not not a.` have the stable models `{}` and `{a}`. Verifying `S` excludes
proper subsets as models of the reduct **at S**, not as stable models under their
own reducts. Choices, nested negation and aggregates therefore need their own
eligibility theorem; a normal-looking head or the absence of choice syntax alone
is insufficient. Formula minimality is defined relative to its candidate in
[Ferraris's propositional semantics, section 2](https://www.cs.utexas.edu/~ai-lab/pubs/proptheories.pdf).

Cone guards require complete original atom identities, not displayed models or
auxiliary encoding variables. A guard compiled over a partial lazy carrier is
not automatically reusable when the carrier grows. It must be rebuilt or backed
by a coverage theorem for the omitted atoms. A program/context update likewise
requires fresh semantic evidence. `themelios` analysis may propose the fragment;
zetesis must check that its admitted source and lowering satisfy the exact law.

## Feedback from rejected candidates

There is a general feedback rule even without the antichain property. If a
candidate `M` is rejected using a verified proper-subset reduct model `J`, keep
`J` as a witness and characterize the other candidates for which it also works:

```text
reject_J(X) = (J is a proper subset of X) AND (J models the original theory's reduct at X)
allow_J(X)  = NOT reject_J(X)
```

Every stable model satisfies `allow_J`, and the rejected `M` violates it. Merely
rejecting every extension of `J` is unsound in the general language: the reduct
condition must be reevaluated as a function of the new candidate. This is a
candidate restriction, never an additional source rule or a fact supplying support.

The reduct test admits a compositional formula transform. Fix `J`; construct a
formula `W_J(F)` whose classical truth in a future candidate `X` equals the truth
of `F`'s reduct at `X` when tested in `J`:

```text
W_J(a)     = a if a belongs to J, otherwise false
W_J(false) = false
W_J(F op G)= (F op G) AND (W_J(F) op W_J(G))
              for op in {AND, OR, IMPLIES}
```

The original subformula supplies the new candidate's truth mask. The transformed
children supply the fixed witness's reduct truth. Structural induction gives
`X models W_J(F)` iff `J models F^X`. Conjoin transformed original roots, then
combine with the proper-subset test to construct `allow_J`. A DAG implementation
can share original nodes and retain one transformed result per original node;
expanding a shared syntax tree independently would lose that storage bound.
This is an architecture proposal derived from the reduct definition, not a
claim of a new semantic theorem or an implemented production feedback compiler.
The checked [Feedback module](../../proofs/Zetesis/Feedback.lean) proves the
transform equivalence, stable-model preservation, witness-based exclusion and
the choice counterexample. Its cone laws require an explicit classical-minimality
premise; it does not prove a source classifier or a splitting implementation.

Any implementation needs limits on retained witnesses, extra nodes/clauses,
construction work and query-reset cost. Learning may decline when its cost is
too high, retaining exact single-candidate blocking. Feedback must remain scoped
to the immutable original theory and full atom mapping. A failed or interrupted
countermodel search supplies no witness. A valid witness formula preserves all
stable models, including every optimal tie, independently of objective bounds.

## Splitting and conditional extension work

The original [splitting-set theorem](https://www.cs.utexas.edu/~vl/papers/splitting0.pdf)
supports a bottom program that does not depend on predicates defined in its top.
It permits solving the bottom first and then specializing the top for each bottom
answer. Its ordinary disjunctive condition couples every atom of a rule when a
head atom belongs to the splitting set; separating arbitrary graph components
can violate that requirement. Constraints and all dependency polarities must be
assigned consistently with the chosen theorem.

Relevant extensions include
[Ferraris's propositional splitting theorem, proposition 6](https://www.cs.utexas.edu/~ai-lab/pubs/proptheories.pdf)
and the [general module theorem of Babb and Lee](https://arxiv.org/abs/1210.5222),
which treats a broader stable-model language, including choices, count aggregates
and nested expressions. Their precise hypotheses differ. A module/SCC discovery
algorithm must produce the appropriate certificate; source graph membership
alone cannot authorize a semantic split.

The proposed execution unit is a top extension task bound to a bottom answer,
its relevant input interface and an immutable program version. Different tasks
can run in parallel. Top construction may share a compiled shape, indexes and
device buffers when an explicit interface equivalence preserves the residual
program. Equal top interfaces do not erase distinct bottom models, hidden-model
multiplicity or different bottom objective contributions.

Feedback is conditional on completed work. If a complete extension search is
UNSAT for an interface, that interface has no top extension. Broader interface
pruning requires a proof that the same result applies to every matching bottom
answer. A timeout, a missing source instance or a single rejected top candidate
proves no such fact. Final enumeration requires every relevant bottom answer and
each corresponding extension search to be accounted for.

Optimization cannot select only a locally cheapest bottom answer unless a
decomposition theorem justifies it. That answer may have no top extension, and
top costs can depend on its interface. Global objective tuple coalescing and
priorities must remain exact; summing independently optimized modules can double
count a shared tuple. A first implementation should retain global scoring of
complete verified answers and introduce separable cost bounds only with evidence.

## Type and domain information before construction

Type/domain analysis should infer possible values, not impose an undeclared type
system on ASP. Useful abstract facts include scalar categories, bounded finite
sets, integer intervals, congruences, argument equalities and selected relational
dependencies. Begin with cheap domains and widen conservatively when recursive
growth or a resource ceiling prevents a precise result. Unknown information may
lose an optimization; it cannot lose a possible answer.

For every admissible source binding, the abstract relation must contain its
concrete values. Positive joins can intersect compatible domains; arithmetic can
map them through checked transfer functions; every producer contributes to its
head domain. A per-column product forgets correlations, so relational filters and
factorization remain separate mechanisms. Facts for one predicate do not establish
the complete domain of a recursively or otherwise intensional predicate.

Negation needs a completed relation or a sound lower/upper approximation. Absence
from an unfinished upper relation is not evidence of falsity. Aggregate ranges
must preserve whole-tuple deduplication, signed weights, empty extrema and all
scopes; a declared callback result range is useful only under the versioned Rust
function contract. Inferred numeric domains must not erase supported symbol
comparisons or silently drop arithmetic failures from the admitted profile.

Keep three scopes distinct: facts valid for the whole program, facts conditional
on a bottom interface, and facts conditional on the current candidate. Candidate
truth cannot shrink the global possible carrier. Likewise, an atom present in
every stable model may restrict outer candidates without being safe to force
inside every reduct countermodel query. Treating such information as a new source
fact could supply unsupported derivations.

This direction connects to established grounding research rather than assuming
that current grounders instantiate a universal Cartesian product.
[Kaminski and Schaub](https://arxiv.org/abs/2108.04769) describe fixed-point and
well-founded operators that guide instantiation and simplification, including
recursive aggregates. zetesis's experiment concerns the representation,
interaction with exact reduct feedback, deferred construction and hardware
execution of sound operators. Current possible-support joins are one implemented
part of that foundation, not a completed abstract interpreter.

## Qualification and profiling

The existing Rayon membership batches and static Metal dispatches exploit some
independent work. The general Ferraris candidate/countermodel path still leaves
substantial scheduling work serial. Further parallelism needs explicit task
ownership and completion evidence:

Parallel execution is a standing design requirement for new transforms. Each
implementation should identify independent work, bound its mutable state and
choose serial CPU, Rayon or wgpu execution from applicability and measured cost.
The architecture does not require a central CDNL search loop, but this alone
neither makes every dependency parallel nor establishes a performance advantage.
Dependency depth, memory bandwidth, duplicate work and synchronization remain
part of the operator's execution contract.

| Work unit | Condition for safe parallel execution |
| --- | --- |
| Independent source joins | Immutable input relations, preserved local scopes, and an exact union of emitted instances |
| Disjoint candidate regions | Complete region partition, original semantic atom identities and duplicate-free accounting |
| Candidate membership checks | A separate frozen mask and mutable query state for each candidate |
| Top extension tasks | An applicable splitting theorem, completed bottom interface and retained full-model identity |
| GPU batches with a common shape | Exact operators, isolated candidate state and measured transport/synchronization costs |

A stale but verified incumbent for the same immutable objective context may
cost extra work; an incumbent from the wrong version is not valid evidence.
Interrupted or missing workers leave coverage incomplete. Parallel scheduling
cannot convert those regions to exhausted searches. The
[domain-analysis design](domain-analysis.md) similarly separates reusable source
facts from candidate and interface conditions.

Every proposed transform must preserve full candidate coverage and original
reduct membership before a hardware cost model selects it. Record analysis time,
domain sizes before/after, generated bindings, retained formula nodes, learned
regions, extension tasks, candidate/reduct queries, work and memory. Measure CPU,
Rayon and GPU paths with startup, transfers, dispatch/synchronization and output
separated from warm kernel execution. Validate complete answer/cost contracts and
retain incomplete runs explicitly. The
[measured comparison reuse](../verification/comparison-reuse-20260905/findings.md)
and [end-to-end CPU campaign](../verification/cpu-performance-20260905/README.md)
illustrate the current process; they establish no device speedup for these
proposed transforms.
