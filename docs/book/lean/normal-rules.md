# Normal rules as Ferraris formulas

The normal closure checker and the general formula checker use different
representations. They must agree on the fragment both represent.
[`Zetesis.NormalFerraris`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/NormalFerraris.lean)
proves that agreement between their independently defined answer-set predicates.

## Translate a rule without changing its gates

The translation uses the same atom type on both sides:

| Normalized component | Formula |
| --- | --- |
| Positive body atom `a` | `a` |
| Frozen true gate `a` | `not not a` |
| Frozen false gate `a` | `not a` |
| Empty body | `⊥ → ⊥` |
| Head atom `h` | `h` |
| Constraint head | `⊥` |
| Complete rule | `body → head` |

The body places positive atoms first, followed by true and false gates, and
combines its nonempty list with left-associated conjunction. The double
negation on a true gate matters: it fixes that atom's gate truth at the outer
candidate while positive antecedents remain conditions on the tested subset.

`translate P` includes only rules whose ground filter holds. Because that filter
is an arbitrary Lean proposition, this is a noncomputable denotation. It is not
an implementation of scalar filtering. `Applicable P` states that all retained
filters have been discharged; `translate_applicable` then gives the direct
`P.map rule` representation corresponding to the ground-program boundary.

## Compare models before minimality

`models_translate` equates original formula models with normalized models of
their own reduct. The stronger frozen-subset law, `models_frozen_translate`,
requires `J ⊆ M` and establishes:

```text
J models the Ferraris reduct of translate(P) at M
    iff M models the normalized reduct of P at M
        and J models the normalized reduct of P at M.
```

The first conjunct is essential: a false original implication becomes falsum
in the Ferraris reduct. Omitting original candidate satisfaction would lose
that behavior. Once the candidate is known to be a model,
`frozen_subset_model_iff` removes the already-established conjunct.

`answer_set_iff` transports minimality through these laws:

```lean
Semantics.Stable P M ↔ Ferraris.Stable M (NormalFerraris.translate P)
```

Finally, `ferraris_answer_set_iff_closure` composes this equivalence with
`Semantics.stable_iff_gamma`. For translated normal rules, equality with the least
closure plus constraints is therefore an exact Ferraris answer-set test.
The reduct remains the common foundation; a least-model procedure is justified
for this fragment.

## What the bridge leaves open

The subset condition belongs to this bridge's frozen-model law. It does not
restrict the general `FerrarisMask` theorem or Rust `FrozenReduct` queries,
which can test arbitrary interpretations.

The proof permits unused atoms and does not require every atom to occur in a
rule. It does require one shared atom universe. It does not prove conversion
from source symbols to dense IDs, concrete source coverage and filter discharge,
Rust DAG construction, allocation behavior or WGSL evaluation. The additional
support guards of `from_ground_program_supported` also need their own argument.
The bridge covers normalized single-head rules and constraints, including true
choice gates; it does not translate every source disjunction or aggregate into
that fragment.

