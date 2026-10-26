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

## Restrict candidates by possible support

Possible-foundedness narrows candidate regions using complete original producer
coverage. A body relaxation overestimates which producers can fire and retains
every permitted head of each possible producer, including every disjunctive
head. Every compatible answer set lies inside any set closed under that
relaxation. Atoms outside a completed possible set can therefore be excluded;
this does not establish answer-set membership for the remaining candidates.

Repeated passes reuse the same prepared body projection. The first pass reads
original links and node kinds. On reuse, preparation retains the parent links
and cyclic heads that can affect this closure, preserving their order and
multiplicity. It also records each selected node's fixed initial role; current
false masks still apply on every pass. Links to unrelated constraints and opaque
inputs cannot change this closure. These prepared columns belong to the exact
original theory and become available together. Candidate restrictions and objective
bounds use separate knowledge; sharing worker scratch with them preserves the
original support preparation. Returning to original support authenticates its
owner before processing new evidence. Preparation work and retained storage
remain accounted for, and a theory used once pays no such preparation cost.

A completed pass can also supply a conditional deduction: whenever a signed
atom condition `B` holds, no atom in an unsupported set `U` belongs to an answer
set. The explanation must block every original producer whose head meets `U`,
including atomic choices. Removing `U` from any matching answer set would then
leave a model of its reduct, so minimality makes `U` absent. For example,
`a :- b. b :- a. a :- x. {x}.` permits the deduction that `x` absent implies
both `a` and `b` absent.

Native aggregate bodies can also supply these explanations. A count or
nonnegative sum is false when the contributions still possible cannot reach its
bound. Every excluded contribution needs a false-reduct witness; an unexplained
one keeps its full weight. An interior min/max cut needs all qualifying
contributions false. Empty selections and endpoint bounds retain their ordinary
meaning. These deductions use the existing numeric rows and signed atom premises;
a cached aggregate value alone is not an explanation.

The region narrower keeps these deductions as learned rows. A row pairs signed
atom conditions with signed outputs, each an atom required present or absent,
and belongs to a consequence class: every member of the class that satisfies
the conditions satisfies the outputs. The one class today is the answer sets of
the exact immutable original theory, and the one source is the conditional
unsupported sets above, whose outputs are absences. A row of the original theory
also holds for the answer sets a candidate restriction, objective bound or
region filter admits, since each only narrows the class.

Rows belong to an owner, an explicit value naming the class and its allowances.
A walk's candidate subject is the owner's carrier: the one subject that matches
and captures rows. Restriction, bound, filter and frozen-reduct queries are never
carriers and leave the rows untouched. Each worker's scratch holds one tier of
the owner's rows. Each completed pass attempts at most one capture before
publishing its cuts, preferring a held unsupported atom. Otherwise it may explain
an open atom that the pass is about to exclude. Neither case permits omitting any
producer from the explanation.

When all signed conditions are known, a row gives each open output its sign, or
refutes the region when an output's opposite is known. It also uses a row with
exactly one unknown condition when every other condition holds and some output's
opposite is known: the unknown condition must fail. For the example above,
holding `a` while `x` is open forces `x` to hold. A condition already known to
have the opposite value disables the row. Two or more unknown conditions, or one
unknown condition without a contradicted output, make no deduction.

Fresh sibling knowledge may reuse a row. Both deductions enter the existing
propagation queues. A row retains scalar literal lists for dispersed identities,
or compact word groups when those occupy less space. It has one authoritative
representation. Word intersections implement the same signed-condition checks,
and outputs retain their ascending order. Conversion shares the optional capture
allowance; matching remains charged to ordinary narrowing work. A new row is
redundant when an existing row requires no additional conditions and gives every
proposed output. The comparison uses the same optional capture allowance and
keeps the existing row. A full tier evicts the row with the fewest recent hits,
the refutations and deductions it made, halved each time the tier has retained as
many rows as it holds; among equals it evicts the oldest. It never evicts a
founded row while it holds no more founded rows than their reserved floor.
Deleting a row loses pruning only. The original theory and membership check stay
unchanged, and no row enters a membership certificate. Optional capture limits
lose only reuse; a global resource or cancellation failure still stops the
operation.

A solve's candidate streams share learned rows through one store, which holds
rows by class. A walk seeds a tier from the store before the tier's first
narrowing, and delivers the tier to it at the walk's boundaries: at its end,
when a parallel walk's coordinator hands over to its workers, at each join of the
GPU route's producers, and each time the walk has visited the owner's
`publish_regions` regions since the tier last delivered (4,096 by default). None
of these is inside a narrowing. A tier delivers only rows of at most 8
conditions, or rows with at least two recent hits; a row it keeps to itself still
prunes its own walk. Seeding and delivery are retention,
so a row the receiving side holds or subsumes is not added twice, and a full
store evicts the row with the fewest recent hits. No public operation adds,
seeds, edits or reads a row: a caller chooses which streams share a store
(`StableModels::set_learned_store`), never what is learned. A solve shares by
default; `SolveConfig::share_learned_rows` turns sharing off for measurement and
has no command-line flag. Rows prune candidate regions only, so the answer sets
are the same with or without sharing.
The [correspondence discussion](../lean/correspondence.md) separates the semantic
law from the collector and runtime obligations.

## Evaluate stratified normal programs directly

When positive dependencies stay within or below a component and every negative
dependency reaches an earlier component, normal producers determine one
interpretation. Complete each positive fixed point before consulting its absence
in a later component:

```text
completed := empty interpretation
for component in dependency order:
    freeze negative literals using completed predecessors
    completed := completed union least consequences of this component
check every original constraint in completed
```

This is a specialization of the reduct definition. Positive cycles are allowed;
negative cycles are not. Constraints may exclude the computed interpretation but
cannot create another answer. Tightness alone does not justify this algorithm:
`a :- not b. b :- not a.` is tight and has two answer sets.

`StratifiedPlan` checks the complete original formula theory. It admits atomic
facts and atomic-head rules with conjunctions of atoms, falsum, truth and single
default-negated atoms. It rejects choices and more general producer formulas.
Signed dependencies and queued positive propagation use the original atom IDs
and formula graph; the result retains its original theory owner. The
[stratified evaluation proof](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/StratifiedEvaluation.lean)
establishes the mathematical reduct argument. Recognition of the Rust grammar
and correspondence of its component schedule remain implementation obligations.

Automatic CPU preparation can apply the stronger case earlier: when every
positive and negative dependency is strictly earlier than its producer, one
dependency-ordered source pass suffices. `AcyclicSource` retains the original
program, canonical atom catalog and checked result. Complete support and
original-family arithmetic checks precede that pass; all constraints and
opposite classical signs are checked before an answer is accepted. No formula
theory or candidate search is needed for this class. Choices, objectives,
projection and recursive producers keep the ordinary route. Explicit eager,
lazy, device and oracle requests keep their requested execution contracts.

The [acyclic evaluation law](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AcyclicEvaluation.lean)
shows that the ordered consequence scan equals stratified reduct evaluation.
It does not yet verify source classification, binding enumeration or Rust storage.

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
the family containing one empty answer is consistent. A world-view question the
session decides by folding answer sets, such as whether a literal is known or
the cautious consequences, is a fact about this family, not a world view. Its
value is decided from some members only when the fold reaches an absorbing
value, and is otherwise complete under the same exhaustion; an empty family
yields inconsistency, never the fold's identity. See
[deciding questions over the world view](../rust/sessions.md#deciding-questions-over-the-world-view).
The Rust boundary is
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

`#project` supplies a different observation: a fixed source-prepared atom domain
defines an equivalence relation between full answers. Enumeration may retain one
representative per key after membership and objective selection. `#show` still
controls only display. A projected family can cover every key while omitting
original answers, so it is not a `WorldView`. See the
[projected session contract](../rust/sessions.md#projected-enumeration).

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
