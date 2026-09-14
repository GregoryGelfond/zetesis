# Least consequences of positive atomic-head theories

[`PositiveTheory`](../Zetesis/PositiveTheory.lean) characterizes a complete
original formula theory whose heads are atomic and whose bodies are monotone.
Bodies use atoms, falsum, conjunction, disjunction and exactly the truth constant
`False → False`. Roots are atomic facts, such a body implying an atom, a body
implying falsum, or falsum. Positive cycles are admitted. Choices, default
negation and other body implications are outside this class. The classification
must cover every asserted original root, including constraints.

This is narrower than arbitrary positive formulas. An asserted `a ∨ b` has two
incomparable minimal models; it is not an atomic-head producer. Disjunction in a
body causes no such ambiguity: whenever that body holds, its one head is required.

## Semantic argument

1. `positive_monotone` states that positive body truth survives adding atoms.
   Its proof follows the body grammar: atoms use set containment, conjunction and
   disjunction use their children, and the two constants are independent of truth.
2. `positive_reduct_exact` states that below a frozen candidate, a positive body's
   reduct has the body's original truth. A body false in the candidate is also
   false in every subset by monotonicity; a true compound uses the child laws.
3. `Closed` describes only facts and atomic-head producer implications.
   `Constraints` describes only the remaining restrictions. The complete-root
   law `models_iff_closed_constraints` separates these two obligations exactly.
4. `Least` is the intersection of all producer-closed interpretations. It is
   contained in every closed set by definition. `least_closed` proves it closed:
   a body true in the intersection is true in every closed set, so its atomic
   head lies in every such set and hence in the intersection. No rank or
   acyclicity premise is used.
5. Positive constraints pass to subsets. `models_reduct_exact` combines the
   root/body laws to equate full theory satisfaction with its reduct below an
   original model. These facts establish `stable_iff_least_constraints`: the
   unique possible answer set is `Least`, and it is an answer set exactly when
   it satisfies the constraints. A stable model must contain `Least`; reduct
   minimality forbids it from being larger. Conversely, a proper subset of
   `Least` modeling the reduct would be closed, contradicting leastness.
6. `failed_constraint_no_model` states the stronger failure result: if the least
   consequences violate a positive constraint, there is no classical model.
   Every model contains the least set and would pass its constraints downward.

The model need not equal the least consequences merely because it satisfies the
original producers. Cyclic rules can have larger classical models. The reduct
minimality argument is what excludes them as answer sets.

## Rust correspondence

The ferraris `PositivePlan` constructor uses this positive producer argument and
the [arbitrary-constraint extension](constrained-positive.md). The strict laws
above retain their positive-constraint scope; the executable accepts arbitrary
bodies only at constraint roots. It inspects the immutable original `Theory`,
then computes producer consequences once. It uses atom vertices plus
formula-node vertices and forward incidence. An atom becoming true signals each
of its formula occurrences. Or needs one true child; And needs both occurrences,
including two incidences when its children alias. A true body signals its atomic
head. Facts and canonical truth initialize the queue. Each vertex becomes true
once and its outgoing edges are processed once, so this avoids expanding a body
to disjunctive normal form or rescanning every rule each round.

The executable correspondence still needs these explicit invariants:

- The topological classifier covers every accepted body's descendants and checks
  every original root. Atom IDs belong to the same immutable theory owner.
- Every declared incidence is counted and written once into the same CSR stream;
  binary aliases retain their occurrence multiplicity. No producer is omitted.
- Every activated atom belongs to every producer-closed interpretation. Activated
  positive nodes have sound truth there. Facts, constants and each transition
  preserve this invariant.
- After the finite queue drains, all true-body heads are active. The result is
  closed, while the preceding invariant places it inside `Least`; leastness then
  gives equality. Exact original evaluation checks that completed result through
  its first false root, or all roots on success. A false producer is an invariant
  refusal; a failed constraint retains its original root identity.
- Checked sizes, actual vector-capacity observations and admitted work precede
  publication. Stopped attempts expose accounting, never a partial certificate.

The plan retains only its exact-owner `Interpretation`, first failed constraint
and observations. Forward graph and work vectors are released before exact
evaluation allocates its truth vector under the remaining work/byte limits. Its
`least_consequences()` remains available when constraints fail, but then is not a
model of the full theory. This class certificate does not manufacture the solve
crate's subject-associated `AnswerSet` value or establish source completeness.

The Lean statements are denotational and do not prove finite queue termination,
CSR indexing, Rust allocation or shader behavior. Those obligations are addressed
separately by source review and complete-family/resource regression checks. Named
capacity excludes the shared theory, allocator metadata and other stack
temporaries; it is not process RSS. No performance improvement follows solely
from the semantic laws.
