# Complete tuple activity in count heads

An aggregate head has two independent carriers: atoms that the rule may permit,
and complete tuples whose selected activity determines the aggregate value.
For example, `1#count{1:a;1:b}1.` permits either atom or both; the tuple contributes
once when either atom is selected. In `2#count{1:a;2:a}2.`, selecting `a` activates
two distinct tuples. Counting distinct atoms would change both programs.

The frontend admits both alias directions for count heads. Numeric sum, sum+,
min and max heads retain their existing tuple/atom bijection and weight profiles.
This slice does not change negative derived-head support, objective dependencies,
source integer semantics, lazy eligibility or backend selection.

## Semantic objects and compilation

A completed local row is `(tuple, head, eligibility)`. The tuple contains every
source term, including secondary values; a numeric weight alone is not a key.
Eligibility remains its original formula, including recursive and default-negated
conditions. Possible-support membership supplies an enumeration domain, never
truth. Tuple expressions and derived head atoms do not supply bindings.

For each atom `a`, permission uses the disjunction of the eligibility formulas
of all rows headed by `a`. With rule body `B`, the permission formula is

```text
(B ∧ eligible(a)) → (a ∨ ¬a).
```

For each complete key `k`, activity uses the disjunction of
`head(row) ∧ eligibility(row)` over rows with key `k`. A distinct key contributes
one to a count regardless of how many rows activate it. A selected atom can
activate several keys. Rows sharing a key may have different heads and conditions;
neither a last-row replacement nor OR-coalescing the conditions independently of
their heads preserves that relationship.

The original guard compiler receives these coalesced activities. The resulting
bound is imposed as `¬(B ∧ ¬within)`. Its frozen reduct tests the original candidate;
the bound supplies no support to a smaller interpretation. Atom permission keeps
the eligibility reduct. Tautological original eligibility is therefore not a
license to replace its formula with truth.

`formula_ground::HeadGroup` keeps separate maps for permission and tuple activity.
Ordinary choices use atom identities as their implicit keys. Explicit count heads
use the entire grounded tuple. The existing support-producing permission roots,
source origins and necessary-support guards remain part of ordinary compilation.
The latter guards inspect the candidate and remain distinct from reduct support.
No synthetic atoms or external solving service are introduced.

## Stronger premises for optional count plans

Complete preflight validation returns an optional tuple/atom bijection certificate.
A nonbijective count group is admitted with no such certificate. The optional
CountPlan collector then skips that group; ordinary source grounding and its exact
original theory remain usable. Other applicable groups in the same source can
still yield a plan. The plan continues to require canonical-true eligibility and
its existing complete disjoint atom-partition premises.

This distinction prevents a tuple count from being misrepresented as a count of
distinct atoms during candidate restriction. Planning remains an optional
candidate-generation aid; its restriction is never substituted for the original
reduct subject. Requesting a plan does not change original atom, formula or origin
bytes. An emitted plan shares the exact admitted original theory instance.

## Bounds and costs

Every possible completed local row is validated before support is derived or an
admitted formula is published. Validation and final replay charge their own work.
Distinct complete tuples obey `AggregateElements`; permitted atoms and formula
nodes retain their separate admission limits. Two atoms sharing one tuple do not
consume two aggregate elements. Two tuples sharing one atom do.

The second activity map introduces bounded temporary storage. Copied complete
keys charge scalar payload and term work before storage. With `R` completed rows,
`H` distinct heads and `T` complete keys, indexing performs at most
`O(R(log H + log T))` map operations, in addition to key comparisons, local joins
and bounded formula construction. Retained group payload is `O(H + T)` entries
plus full keys and referenced formula storage. Formula construction may preserve
more witness conjunctions than the previous bijective factorization. No speed or
RSS improvement is claimed for this language extension.

Resource exhaustion returns a located failure rather than a partial admitted
theory or an UNSAT result. The regression suite checks inclusive work, substitution,
tuple-count and scalar-storage boundaries for both alias directions.

## Evidence and proof boundary

`tests/count_head_activity.rs` compares original sources with clingo, compares every
original/frozen interpretation pair on finite examples, and checks 128 generated
programs against an independent direct permission/activity interpretation. Its
fixtures include recursive/default-negated eligibility, complete tuple identity,
all count guard comparisons, strong negation and outer activation. A wrong-key
mutation tests whether these assertions distinguish tuple activity from atom
activity. CountPlan tests retain the original theory and independently check
candidate consequences when aliased and bijective groups coexist.

`proofs/Zetesis/CountHeadActivity.lean` characterizes original/frozen tuple activity
and atom eligibility without a bijection premise. It establishes row-preservation,
threshold, frozen-bound and contextual-stability laws using the existing
`CountEligibility` and `HeadMeasures` definitions. A complete distinct tuple carrier
is an explicit premise. Older bijective laws remain useful for specialized
optimizations. These laws do not constitute a proof of the Rust maps, source joins,
support completion, resource accounting or executable compiler correspondence.

The representation also exposes a later optimization connection: native reductions
already consume coalesced tuple activities. Sharing a proved complete-tuple carrier
may eventually reduce duplicated construction, but ordinary solving still uses
the existing exact formula/reduct path in this slice.
