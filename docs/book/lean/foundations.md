# Definitions and imports

The `proofs` directory is a Lean package. Import `Zetesis` for the full library,
or a specific module such as `Zetesis.Ferraris` for its declared dependency
closure. Definitions and theorem statements are mathematical APIs. Their
hypotheses matter as much as their conclusions.

```lean
import Zetesis.Ferraris

#check Zetesis.Ferraris.Satisfies
#check Zetesis.Ferraris.Reduct
#check Zetesis.Ferraris.stable_iff_minimal_reduct
```

This is Lean source, not a Rust solver invocation. From the repository, build
the package with `cd proofs && lake build`; `lean-toolchain` selects the package's
Lean version. A small consumer file in that directory can be checked with
`lake env lean Consumer.lean`. Lean declarations are maintained with the semantic
changes they describe, not generated from the prose in this book.

## Interpretations are predicates

[`Zetesis.Core`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Core.lean)
uses `Atoms α := α → Prop`. Thus `M a` means that atom `a` belongs to interpretation
`M`. `Sub M N` is inclusion, and `EqOn S M N` is agreement on carrier `S`.
These definitions do not enumerate atoms or commit to a packed representation.

The finite Rust representations supply additional obligations: finite atom
indexing, valid ownership, bit membership and bounded allocation. A proof over
predicate sets does not establish those properties automatically.

## Positive consequence and normalized rules

[`Zetesis.Transformers`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Transformers.lean)
defines a transformer on interpretations, monotonicity, closed sets and `Least`.
Here `Least` is defined denotationally by membership in every closed set. It is
not an implementation of a while loop. The finite-iteration laws supply a
separate connection when sound iteration reaches a closed stage.

[`Zetesis.Semantics`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Semantics.lean)
defines normalized rules with one optional head, positive antecedents, frozen
true and false gates, and a proposition-valued ground filter.
`Gamma P z` is the least consequence set for seed `z`.
`ReductModel P z X` requires closure and constraints. `Stable P M` independently
requires minimality among models of the reduct frozen at `M`.

`stable_iff_gamma` proves that, for this normal-rule representation, stability
is equivalent to equality with the least closure plus satisfaction of
constraints. Least-closure checking is justified by the representation; it is
not assumed in the general definition of an answer set.

## General formulas

[`Zetesis.Ferraris`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Ferraris.lean)
defines finite syntax trees, `Satisfies`, `Reduct`, `Models`, `MinimalModel` and
`Stable`. `stable_iff_minimal_reduct` connects original satisfaction and absence
of a proper-subset reduct model to minimal-model semantics of the reduct.

The reduct is a noncomputable denotational definition because atom predicates
need not be decidable. Rust evaluates finite indexed data instead. The theorem
does not assert that an arbitrary formula reduct has a least model; the module
contains explicit disjunctive counterexamples.

Lean uses `Stable` to connect with the stable-model literature. The solver-facing
term **answer set** denotes that same semantic property. Naming an unchecked Rust
value `AnswerSet` would not establish the property.

