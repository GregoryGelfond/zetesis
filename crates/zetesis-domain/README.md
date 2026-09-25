# zetesis-domain

Conservative argument domains over the exact borrowed
`themelios_program::program::Program`. The crate uses the pinned
`themelios-program` dependency declared in the workspace; it has no solver,
oracle, CLI, GPU or `zetesis-core` dependency. It neither accepts source strings
nor reparses, normalizes, unpools or grounds its input. Callers supply the complete
logical program whose argument domains they want to analyze.

For a program corresponding to:

```prolog
edge(1,2). edge(2,3).
reachable(X,Y) :- edge(X,Y).
reachable(X,Z) :- reachable(X,Y), edge(Y,Z).
```

the completed upper domains for `reachable/2` are `{1,2}` and `{2,3}`. The
analysis does not construct the argument product or claim that every pair is
reachable. The result describes possible values, rather than concrete atoms or
stable answers.

## Terminal positive definitions

`terminal::analyze(&Program, Limits) -> terminal::Analysis<'_>` classifies a
separate structural profile. It returns every original rule carrier for signed
predicates whose producers are all normal positive flat definitions and whose
atoms are never read by any rule, constraint, or local condition. Each head
variable must have a whole-argument positive body binding. Repeated variables,
facts and already closed `Symbol` constants are allowed. Unevaluated arithmetic,
ranges, pools and constructors disqualify a producer; they are not evaluated.
Choices can remain in the base program, without any assumption that their
possible support is functional.

For example, `1 { assigned(K,V):value(V) } 1 :- key(K).` chooses one value
per key in each answer set. If `value(red)` and `value(blue)` are possible,
grounding must retain both alternatives. A separate definition
`receipt(K,V) :- assigned(K,V).` can qualify as terminal when no logical rule
reads `receipt`. Eligibility depends on the complete definitions and
dependencies, not these names, the number of choices, or a particular encoding.

The complete occurrence scan includes choice, disjunction, aggregate and
conditional head/body scopes. A complementary strong-sign occurrence prevents
selection because coherence is an implicit dependency. Every producer of a
selected signature is returned in Program iteration order, retaining its exact
carrier and combined provenance. The result belongs only to the borrowed input
instance. `#show` and `#defined` neither select nor suppress definitions.
Objectives, explicit projection and unresolved source contexts produce
`Unknown`; a logical ceiling produces `Stopped`. Either discards the selection.

This profile uses work, inspected text, signed predicate/argument populations,
submitted occurrence/reference links and per-symbol structural limits. The
finite-domain width, value-entry and fixed-point-round limits are unrelated and
unused. Names, values and rule carriers stay borrowed. Standard collection
allocations are bounded by the admitted logical populations; this is not a
fallible physical-byte allocation contract or a total memory limit.

The classifier does not omit grounding, reconstruct an answer, or establish
answer-set correspondence. A consumer must validate source/IR correspondence,
preserve original diagnostics and reconstruct complete interpretations before
membership evidence, projection or output. Those execution obligations remain
separate from this structural result.

## Keyed relations

`keys(&Program, &mut KeyWork) -> Result<Vec<KeyedRelation<'_>>, Stop>` lists the relations
whose value is a function of their key: for the choice rule
`1 { p(K, V) : c(V) } 1 :- b(K).`, the one producer of `p`, every answer set
holds exactly one `p(k, v)` for every `k` that `b` admits and no other atom
of `p`. A `KeyedRelation` names the signature, the value position, the key variable at
each other position, the value variable, the element's condition and the
body. `atom_signature(&Atom, arity)` is the signed signature an atom's arity
fits, `None` when it does not fit a signature's width. The reading is
syntactic and conservative:
another producer, other bounds, a second element, a value bound outside its
condition, a body binding more than the key, or a negated literal anywhere
yields no key. The consumer proves what it does with the fact; this crate
states it.

## Contract

`analyze(&Program, Limits) -> Analysis<'_>` retains the exact input reference.
`program()` and `belongs_to()` expose that identity; equal cloned content is a
different input instance. Signed `themelios_program::symbol::Signature` values
identify argument positions. Finite sets share borrowed `&Symbol` values with
the original program, including admitted compound symbols. Direct producer
references retain each enclosing `WithProvenance<Statement>` and all its
origins. They are source evidence, not a transitive derivation proof.

The domain lattice is finite sets ordered by inclusion, with `Unknown` above
every finite set. A finite empty set is meaningful. An unregistered signature,
invalid argument index, or global fallback returns `Unknown`, never inferred
emptiness. `Domain::permits` is an upper-bound membership query: it returns true
for every value when the domain is Unknown.

`Status::FixedPoint` means only that this conservative abstract transfer system
converged. It does not establish source safety, finiteness of grounding, precise
argument correlations, or stable-model existence. A global context fallback is
`Status::Unknown(reason)`; exhaustion is `Status::Stopped(Stop)`. Both clear all
finite results, including results discovered before the interruption. No partial
finite approximation is published as a sound completed bound.

The analyzed Program is the whole supplied semantic input. No ambient constant
overrides, future includes, part activations, external assignments or additional
facts are silently assumed. Such changes require a new analysis of their complete
context. The result has no candidate-specific or task-specific demand filter.
`#show` supplies no narrowing of this global domain analysis; its planned role
as an implicit observation query belongs to separate task-relative demand
analysis. Hidden predicates remain accounted for. `#defined` retains its clingo
diagnostic semantics and creates no open-input domain here.

## Initial transfers

- Ordinary positive literal heads contribute direct bounded symbolic terms.
  Numeric arithmetic is not evaluated; it widens its affected head argument.
- A named variable occupying a whole head argument receives the **union** of
  domains at its whole-variable ordinary positive body positions in that rule.
  Union deliberately forgets join equalities; intersection may improve a later
  transfer. Every producer contributes, including recursive producers.
- Negative literals, comparisons, aggregate/conditional bodies and other
  unsupported conditions do not narrow a domain or bind its variables. A
  missing ordinary positive binding widens the output to Unknown. Anonymous
  occurrences never unify as a named variable; an anonymous head widens.
- Choice and disjunction heads widen their affected positions. No variable
  transfer crosses head-element local scopes. Pooled head alternatives are
  inspected structurally and widened, without constructing their product.
- Generative function applications, intervals, pools and external calls in
  head terms widen their positions. Ground constructor values already stored as
  `Symbol` can contribute if their structural bounds pass.
- `#const`, unresolved `#include`, external declarations, script statements,
  theory definitions/heads, and non-base or parameterized parts make the whole
  result Unknown. Unsupported head producers and unknown future statement
  families do likewise. Source names with no `#const` are ordinary symbolic
  constants; the analysis never pretends to substitute unresolved definitions.
- Constraints and display/optimization/diagnostic statements introduce no
  argument values. Ignoring their restrictions is conservative for this upper
  cover; their validity and observable solver behavior are separate concerns.

In-place monotone union passes continue until unchanged. Unsupported producers
and width widening propagate Unknown through the same dependencies. For the
stated ordinary positive profile, every value in its concrete least-model
closure is covered. For broader forms the result deliberately sacrifices
precision rather than inventing a closed finite carrier.

## Bounds and verification

Independent ceilings cover work, inspected text, signatures (including nullary
ones), argument positions, transfers/producer/dependency links, retained value
references and fixed-point passes. Per-argument width overflow widens locally.
Per-symbol node count, depth and string/name payload are checked before a value
enters an ordered set; oversized symbols widen locally without deep comparison.
The bounded traversal checks child counts before extending its explicit stack.
Stored symbol comparisons therefore have bounded structure and payload.

Temporary transfer snapshots copy only references from one bounded finite
argument, charging copy work before allocation. Borrowed symbol payload is not
copied. Statistics count charged logical operations/reservations, including
work before fallback; they do not measure allocator overhead or peak RSS.
Caller-supplied query symbols and the already constructed input Program are
outside the analysis allocation contract.

```sh
cargo test -p zetesis-domain --offline
cargo clippy -p zetesis-domain --all-targets --offline -- -D warnings
cargo doc -p zetesis-domain --no-deps --offline
```

The portable suite checks exact input/symbol/provenance identity, recursive union,
signed positions, local/anonymous scopes, unresolved context, every global
resource category and inclusive work boundaries, local width widening and deep
symbols. A 128-case Proptest campaign compares upper coverage with an independent
concrete positive fixed point over random facts and conjunction rules, including
cycles and deliberate precision widening. Test parsing is confined to development
fixtures; the production API receives Program directly. No Lean refinement or
external-oracle coverage is claimed for this crate.

Different immutable Programs can be analyzed independently. A future synchronous
transfer/reduction implementation could parallelize independent components after
profiling. This first bounded implementation has no threading/device dependency
and makes no hardware speedup claim. The source bridge consumes domain analysis
for eligible grounding guards and terminal-definition reconstruction; those
operations belong to the grounder and solver, not this analysis crate. Abstract
intervals, arithmetic transfer and demand/magic-set rewriting remain future work.
