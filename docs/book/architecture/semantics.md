# Programs, answer sets, and the reduct

Fix a program `P` and its atom vocabulary. An **interpretation** `M` selects
which atoms hold. Satisfaction, written `M ⊨ P`, says that every rule or asserted
formula holds under `M`. Satisfaction alone permits unsupported interpretations;
answer-set semantics adds a minimality requirement on the reduct.

## Begin with positive programs

For a basic positive normal program, each headed rule has one atom in its head
and a conjunction of positive atoms in its body. An empty body is a fact;
a headless rule is a constraint. Its immediate-consequence operator
derives every head whose body already holds. Iteration from the empty set, adding
these consequences, reaches the least closed interpretation when the finite
iteration completes. Positive constraints must also hold. If this least closure
violates a positive constraint, every larger closed interpretation violates it,
so there is no model. Otherwise the least closure is the unique answer set.
Positive cycles do not invalidate this argument: `a :- b. b :- a.` has the
empty least closure, while adding the fact `a.` derives both atoms.

The same reasoning applies to atomic-head formula rules with monotone bodies.
The admitted producer body grammar is atoms, falsum, conjunction, disjunction
and exactly the truth constant `False → False`. Every producer root must be an
atomic fact or such a body implying one atom. For
example, `(a ∨ b) → c` requires `c` whenever either body alternative holds.
Choice heads, default negation and other implications in producer bodies are
outside this class;
classical truth alone does not admit a different formula as its truth constant.

These producers can coexist with arbitrary original constraints `F → False`,
including falsum as an unconditional constraint. A constraint satisfied by a
candidate has a reduct true in every interpretation, so it can filter answer
sets but supplies no support. Compute the producers' least consequences, then
evaluate the original constraints there: if all hold, that is the unique answer
set; otherwise there is no answer set. Unlike positive constraints, arbitrary
constraints can still permit larger classical models. For `:- not a.` with no
producer for `a`, `{a}` is a classical model but neither interpretation is stable.

The library primitive `PositivePlan` checks this complete grammar against its
immutable original `Theory`, computes the least consequences, and then checks
all original constraints. Its result distinguishes least producer closure from
a model of the full theory. The
[constrained-positive argument](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/constrained-positive.md)
proves this formula-level characterization. The primitive's CSR propagation,
resource admission and connection to ordinary candidate enumeration require
their own implementation correspondence; an analysis label cannot substitute
for its complete-root check.

A positive disjunctive program has a different shape. `a | b.` has two minimal
models, `{a}` and `{b}`, and no least model: their intersection fails the rule.
Its answer sets are its subset-minimal models. An atomic-head least-consequence
specialization must decline this theory and retain the general reduct check.
“Compute the positive answer
sets” therefore does not mean “compute a least closure” for every language.

## Freeze the candidate, then check its reduct

For normal rules, the Gelfond–Lifschitz reduct removes rules whose default-negated
body is false in `M`, and removes default negation from the remaining rules.
`M` is an answer set exactly when it is the resulting positive program's least
model and satisfies its constraints.

The general formula path uses the Ferraris reduct. Formulas use atoms, falsum,
conjunction, disjunction and implication. Default negation is implication to
falsum. To form `Fᴹ`, replace every maximal subformula false in `M` by falsum;
continue recursively through candidate-true subformulas. For a theory `T`:

```text
M is an answer set of T
    iff M satisfies T
    and there is no J ⊊ M satisfying Tᴹ.
```

The candidate `M` stays fixed while different `J` are tested. Recomputing the
reduct at `J` asks another question. A reduct-satisfaction operation can test any
interpretation `J`; the membership algorithm is responsible for restricting
its countermodel search to proper subsets.

The general path supports finite formulas encoding admitted choices and
aggregates. These constructs require their own source-to-formula preservation
arguments. Classical equivalence of two aggregate expressions is insufficient:
the replacement must preserve the required frozen-reduct behavior as well.

## Membership, enumeration, and optimization

Membership answers whether one candidate is an answer set. Enumeration also
requires complete candidate coverage. A verified answer can be returned before
that coverage is complete; an inconsistency conclusion cannot.

A **world view** here is the complete answer-set family of a specified original
program. It is not an epistemic extension of the language. A family restricted
by assumptions or selected by an objective must be identified as such. In
particular, a set of optimal ties is not the unrestricted world view.

The writer-free `Session::enumerate` returns checked `AnswerSet` values as they
are found. `WorldView::collect` additionally requires complete unrestricted
coverage and retention of every answer. Its empty family means inconsistency;
the family containing one empty answer is consistent. The Rust boundary is
backed by native runtime checks and enumeration contracts, not an executable
refinement proof from the Lean definitions.

`#show` defines observations of answers. It does not erase hidden atoms from
membership or justify ignoring rules irrelevant to a displayed predicate.
Two full answers can have identical displays. Objectives rank verified answers;
candidate-only objective restrictions do not replace the original theory used
for reduct checking.

## Compose consumers without changing membership

Source preparation establishes finite bindings and preserves the original
conditions. It does not decide which of those bindings hold in an answer set.
The prepared program has three consumers with different contracts:

```text
source program
    │ finite bindings, typed values, scoped conditions
    ├─ theory ──────── satisfaction + frozen-reduct checking ── AnswerSet M
    ├─ objectives ──── original-model conditions + keyed sums ── score(M)
    └─ observations ── original-model queries + value construction ── shown(M)
```

All three use the same logical atom and value identities. Their owned execution
representations serve different questions: a theory needs support and reduct
semantics; a closed objective query needs only truth in the supplied model; an
observation can bind local variables while reading that model. Sharing scalar
evaluation or tuple operations does not merge those contracts.

For an objective, **source eligibility** gives a finite set of potential
contributions. **Condition truth** selects the contributions active in `M`.
**Contribution identity** coalesces equal complete keys before summation. An
eligible contribution may be false in every answer; its retained priority can
therefore be an always-zero slot. Removing a potentially active contribution
would instead change the optimization problem. The preparation certificate
must establish coverage, independently of any particular answer.

Eligibility evidence belongs to the objective whose producer dependencies it
describes. Adding an unrelated objective must not impose a new semantic
restriction on an existing one. Program-wide storage limits still account for
their combined retained data. Closed query storage is charged only when the
numeric contribution is retained; source evaluation and its work limits remain
separate obligations.

Scoped objective bodies reuse the formula operations through an isolated
temporary builder. The compiler borrows completed producer support, validates
the body for one outer binding, and then translates reachable formula nodes
into a closed query if the numeric contribution is retained. The temporary
builder cannot publish program roots or enlarge the original atom universe.
Its storage limits are separate from the retained query limit; both operations
consume the source preparation budget.

This reuse depends on the consumer's semantics. Replacing `A → B` by
`not A ∨ B` preserves truth in a supplied original interpretation, so it is
valid inside this objective query. It does not generally preserve Ferraris
reducts and is not a program transformation. The
[`formula_query_truth` law and reduct counterexample](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ObjectiveConditions.lean)
state both sides of this boundary.

In aggregate heads, positive head permission and aggregate measure also have
distinct roles. A positive head occurrence can permit an atom even when its
measure contributes zero. Complete tuple identity determines aggregate
coalescing; atom identity determines positive permission. Neither may stand in
for the other. The [language reference](../reference/language.md#choices-and-aggregates)
states the contribution and empty-measure boundaries.

The [costs and shown terms example](../rust/costs-and-output.md) demonstrates
these consumers through the library. The [implementation correspondence](../lean/correspondence.md)
separates the mathematical preservation laws from unproved source, Rust and
device refinements.

The formal definitions are developed in
[Part III](../lean/foundations.md). The current Rust names for these objects are
listed in the [vocabulary](../vocabulary.md).
