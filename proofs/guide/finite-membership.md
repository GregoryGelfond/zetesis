# Deciding membership through the reduct

[FiniteMembership](../Zetesis/FiniteMembership.lean) gives an executable finite
answer-set checker and proves it agrees with the independent Ferraris
definition. It is a reference algorithm in the ASP library, not a replacement
for a solver's optimized search.

## The objects and the claim

The candidate is supplied as a finite list of atoms. Its meaning is set
membership: repeated occurrences do not create different atoms. The ambient
atom type only needs decidable equality; it need not be finite. The theory is
the formula DAG and root list already interpreted by `DagSharing`.

`check_iff_answer_set` proves:

```text
check candidate table roots = true
    exactly when
the interpretation represented by candidate is an answer set
of the theory represented by table and roots.
```

The checker establishes original satisfaction, then searches the candidate's
proper subsets for a model of the frozen reduct. A countermodel rejects the
candidate. Exhausting that finite search without a countermodel accepts it.
The original candidate fixes the reduct throughout.

The algorithm uses ordinary Boolean computations. Its correctness theorem has
no assumption that an external evaluator is correct or that an external
proposal source supplies every subset.

## Why the search is complete

`selections` includes or omits each list occurrence. `selection_subset` proves
that a selection introduces no atom. Conversely, filter the original list by
any semantic subinterpretation. `filter_selected` places that filter in the
enumeration, and `selections_cover` proves that it represents exactly the
subinterpretation. Classical decidability is used to construct this witness in
the proof; the executable enumeration does not consult an arbitrary predicate.

The properness check asks whether some original atom is absent from the selected
interpretation. It compares membership, not list length. This matters when the
candidate list contains duplicates: omitting one occurrence need not remove an
atom. `proper_iff` proves the exact semantic proper-subset condition.

`has_countermodel_iff` composes these two facts with
`ReductEvaluation.roots_true_iff`. A successful search supplies a genuine
proper-subset reduct model. Every such semantic countermodel has a represented
selection and therefore can be found. The final membership theorem combines
this result with original DAG satisfaction and the definition of an answer set.

## Boundaries

An empty candidate has no proper subset; it is accepted precisely when it
satisfies the original theory. The algorithm also applies to disjunctive and
nested formulas. It does not presume a unique least reduct model, as the
normal-rule specialization may.

This finite reference search is exponential. Repeated input occurrences can
repeat work, and each `ReductEvaluation.values` call computes the candidate's
mask anew. The theorem establishes the result, not the efficiency of a cached
or parallel implementation.

The checker has no interruption channel. A bounded implementation must retain
the distinction between an exhausted search and unfinished work. The theorem
does not verify the Rust candidate counter, source grounding, packed indexing,
resource accounting, cancellation, output delivery or device execution. It also
does not enumerate the program's complete answer-set family: it decides
membership of one supplied finite candidate.

As in the underlying DAG model, unavailable references have a total falsum
meaning. The Rust implementation's admission checks and valid-index invariant
remain separate obligations; the mathematical default is not permission to
accept malformed runtime input.
