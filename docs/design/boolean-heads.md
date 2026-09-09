# Boolean operands in formula heads

The finite formula boundary admits `#true` and `#false` as singleton heads and
as disjuncts, with zero, one or two default negations. Atomic siblings retain
their existing sign, binding, pool and interval semantics. Empty conditions and
explicitly true Boolean conditions use the existing conditional-head admission
rule. Arbitrary atom or comparison conditions remain outside this slice.

For example, `#true|a.` has only the empty answer set, while `#false|a.` has
`{a}`. Neither `#true` nor `#false` becomes an atom. In particular,
`#false|not not a.` does not provide positive support for `a`. These are ordinary
rule formulas whose answer sets are checked through the Ferraris reduct; this
extension adds no alternative search semantics.

## Representation and compilation

`DisjunctIr` owns a default-negation sign and a `HeadOperand`, which is either
an `AtomPattern` or a Boolean. A Boolean operand records logical truth without
inventing a predicate, dense atom identifier or sentinel atom. The signed
operand remains explicit until formula construction. `atom()` sees an atomic
operand under any default-negation sign, while `positive_atom()` sees only an
unnegated atomic occurrence. Dependency safety uses the first operation;
possible positive support and necessary-producer guards use the second.

Singleton verum enters the disjunction representation. Existing singleton
falsum keeps its constraint representation. Boolean disjuncts lower to the
builder's existing falsum and verum nodes, with the usual implication-based
default negation applied zero, one or two times. They never call atom allocation,
producer recording or atom-origin recording. The enclosing rule still retains
its source origin.

Compilation visits the complete original rule before any formula simplification.
All atomic siblings are lowered even when an earlier Boolean makes the
disjunction true. Whole-rule interval and pool expansion remains unchanged:
`#false|p(1..2).` emits both required instances; `#false|p(2..1).` emits none.
The empty family does not turn into the constraint `#false.`. Likewise,
`#true|p(X).` remains unsafe and `#true|p(1/0).` retains zetesis's existing
located arithmetic refusal. A tautology is not permission to omit source
validation, binding instructions or their charged work.

The operand representation costs constant additional tag storage per retained
head occurrence. For `H` occurrences, retained head storage remains `O(H)`
plus existing atomic pattern payloads. Lowering visits all `H` occurrences and
uses `O(H log H)` ordered deduplication bookkeeping in the worst case, excluding
the unchanged term evaluation, atom lookup and formula interning costs.
Boolean operands allocate no atom payload. Finite generation can still multiply
whole-rule instances; assignment, head-element, expression-work, substitution,
atom and formula-storage ceilings remain necessary. This change makes no
performance claim and introduces no GPU-specific operation.

## Preserved boundaries

The themelios parser and its dependency pin are unchanged. This is an eager
finite-formula admission extension, not a claim that the lazy source executor
admits formula heads. Existing automatic routing and explicit refusal contracts
remain the responsibility of the consuming library or CLI boundary.

Objective-relevant disjunctive definitions retain their conservative refusal.
For example, `#true|a.#minimize{1@7:a}.` must not be admitted merely by treating
the Boolean as removable syntax: clingo can erase the objective priority after
simplifying the rule. Dependency checks still inspect every atomic sibling,
including default-negated ones. The mechanical accessor changes in objective
dependency analysis do not widen its policy.

Atom-conditioned head alternatives, comparison head literals, signed choice
elements and other head-aggregate extensions are separate language work.

## Semantic evidence

`Zetesis.BooleanHeads` extends the ground signed-head grammar with Boolean
operands. Its constant laws preserve truth in arbitrary original `M` and frozen
`J`, without a subset premise. Its necessary-producer theorem proves that a
stable model's atom must occur positively in the head of an originally active
rule. Boolean and default-negated operands never supply this evidence. The
result justifies double-negated necessary-producer guards without changing
stable models of the original theory.

These laws do not prove Rust admission, source expansion, support joins,
resource accounting, representation refinement or device behavior. Rust tests
separately compare original-source complete models against clingo, evaluate an
independent hand-written formula grammar over arbitrary `M/J`, and exercise
bounded failures and source provenance. The scoped evidence is recorded in
[Boolean-head qualification](../verification/boolean-heads-20260908/README.md).
