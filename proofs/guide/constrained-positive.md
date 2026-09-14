# Positive producers and arbitrary constraints

[`ConstrainedPositive`](../Zetesis/ConstrainedPositive.lean) separates positive
atomic-head producers from original constraints `F → False`. Constraint bodies
may contain arbitrary implication or default negation. They filter
interpretations; they never supply positive support.

Fix a candidate `M` satisfying a constraint. Its body `F` is false in `M`, so
that body's entire reduct is falsum. The constraint reduct is therefore
`False → False`, true in every tested interpretation `N`. This argument needs
neither `N ⊆ M` nor monotonicity of `F`.

`stable_append_constraints` uses that fact in both directions. An answer set of
the producer theory remains an answer set after adding constraints it satisfies.
Conversely, any proper-subset reduct model of the producer theory also satisfies
all constraint reducts. Thus a stable model of the combined theory must already
be stable for the producers. This law applies to arbitrary producer theories.

`positive_stable_iff` composes this filtering law with
[`PositiveTheory`](positive-theory.md). Its explicit split is `P ++ bodies.map
Neg`, where every root of `P` has the strict positive atomic-head grammar. The
only possible answer set is `Least P`; it must satisfy the positive constraints
already in `P` and every additional arbitrary constraint. When `P` contains only
facts and atomic-head producers, its positive-constraint condition is vacuous.
No arbitrary constraint contributes to computing that least set.

Failure has a narrower conclusion than the strict positive-constraint law.
For `:- not a.` with no producer for `a`, least consequences are empty and fail
the constraint. `{a}` is a classical model, but neither interpretation is stable:
the constraint cannot supply support for `a`. `failed_constraints_no_stable`
therefore rules out answer sets. The older
`PositiveTheory.failed_constraint_no_model` retains its stronger claim only
under its positive-constraint hypothesis.

Rust `PositivePlan` classifies every original root. Atomic-head producer bodies
keep the positive grammar; any body implying falsum is a constraint. After the
positive queue completes, its incidence/work vectors are released.
`EvaluationWorkspace` computes exact original truth for that same least
Interpretation under the remaining cumulative work and named-byte allowance.
It tests roots through the first false root, or all roots on success. A false
producer is an invariant refusal; only a constraint root can establish the
no-answer conclusion. Stopped evaluation supplies no plan or conclusion.

Concrete obligations include a complete original-root split, formula/atom
identity, producer closure, authenticated original evaluation, and queue/CSR and
resource correctness. The mathematical append presentation does not authorize
omitting or reordering executable validation: Rust retains the first failed
original root. The laws do not verify Rust allocation, indexing, work accounting,
source grounding or ordinary candidate coverage.
