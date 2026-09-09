# Programs, answer sets, and the reduct

Fix a program `P` and its atom vocabulary. An **interpretation** `M` selects
which atoms hold. Satisfaction, written `M ⊨ P`, says that every rule or asserted
formula holds under `M`. Satisfaction alone permits unsupported interpretations;
answer-set semantics adds a minimality requirement on the reduct.

## Begin with positive programs

For a positive normal program, each headed rule has one atom in its head and a
conjunction of positive atoms in its body. Its immediate-consequence operator
derives every head whose body already holds. Iteration from the empty set, adding
these consequences, reaches the least closed interpretation when the finite
iteration completes. Positive constraints must also hold. If this least closure
violates a positive constraint, every larger closed interpretation violates it.

A positive disjunctive program has a different shape. `a | b.` has two minimal
models, `{a}` and `{b}`, and no least model: their intersection fails the rule.
Its answer sets are its subset-minimal models. “Compute the positive answer
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

`#show` defines observations of answers. It does not erase hidden atoms from
membership or justify ignoring rules irrelevant to a displayed predicate.
Two full answers can have identical displays. Objectives rank verified answers;
candidate-only objective restrictions do not replace the original theory used
for reduct checking.

The formal definitions are developed in
[Part III](../lean/foundations.md). The current Rust names for these objects are
listed in the [vocabulary](../vocabulary.md).

