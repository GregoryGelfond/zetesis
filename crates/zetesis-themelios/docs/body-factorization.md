# Factoring independent body variables

For a normal rule, completed possible support supplies candidate rows for its
head predicate. Unifying each row with the head fixes all head variables, checks
constants and repeated variables, and never enumerates the global value domain.
Rows supplied only by other producers are harmless: this rule must still have
an eligible body. Constraints have one empty head binding.

After fixing those separator variables, the compiler builds connected components
of the remaining variables co-occurring in each body literal. Every comparison
and projected gate contributes its complete named-variable scope. Head-only
literals form another fixed component. Components with unbound local variables
without a positive binder are rejected by the optimization plan; the original
complete-join route remains available.

Each component joins independently over the same immutable completed support.
All its successful body formulas are disjoined. Their component roots are then
conjoined and imply the original head, with the existing root provenance and
producer registration. No semantic auxiliary atom is introduced. Duplicate
local witnesses can share formula nodes, but their conditions are never dropped
merely because the head was already seen. The separate support-only optimization
needs only one positive witness per head and must not be applied here.

The finite law is:

    AND(i,j) ((A_i AND B_j) -> H)
        equivalent to
    ((OR(i) A_i) AND (OR(j) B_j)) -> H

It preserves every original and frozen-reduct interpretation, including recursive
and default-negated conditions; it is stronger than classical truth equivalence.
Repeated application permits additional independent components. Necessary
supportedness guards receive the factored producer body. Its original truth is
unchanged, and double negation keeps that guard frozen for reduct checking.

The current source plan accepts ordinary positive/default-negated atoms,
projected negative atoms, and total scalar/tuple comparisons. Comparison
expressions must contain only constants and already bound variables. Arithmetic,
aggregate, assignment and interval literals use the original path. This avoids
raising an undefined arithmetic error in one component when another component
has no complete witness. Richer component profiles need an explicit totality or
error-preservation contract.

Local names are not renamed or conflated. The component cursor receives the
exact fixed head slots, its used-variable set, and placeholders only in slots
that no component literal reads. Every used local slot still requires an actual
positive binding. Work is charged for planning, copying, component joining,
formula construction and head unification. Existing scalar-copy, node, root,
origin and join ceilings remain in force; no default is increased.

[RuleFactorization.lean](../../../proofs/Zetesis/RuleFactorization.lean) proves the
finite two-family formula law and stability in an arbitrary unchanged context.
It does not establish the Rust dependency planner, head-support coverage,
multi-component iteration, supportedness rebuilding or resource accounting.
Portable tests compare every M/J assignment of small factored sources with
explicit Cartesian ground rules, including negation, recursion, constraints,
empty components, repeated head arguments, other head producers, projections,
undefined-arithmetic fallback and original provenance. The full corpus result
is reported independently by the validation runner.
