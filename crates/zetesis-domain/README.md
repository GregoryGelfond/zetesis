# zetesis-domain

Conservative argument domains through one neutral producer engine. The source entry point borrows the exact
`themelios_program::program::Program`; the prepared-IR adapter supplies authenticated catalog keys through a reader.
The crate uses the pinned
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

## Source transfers

- Ordinary positive literal heads contribute direct bounded symbolic terms.
  Numeric arithmetic is not evaluated; it widens its affected head argument.
- A named variable occupying a whole head argument receives the **intersection**
  of domains at its whole-variable ordinary positive body positions within that
  producer. A binding must satisfy every such occurrence. `Unknown` is the
  unrestricted domain: intersecting it with a finite domain retains the finite
  bound, including an empty one. Only all-Unknown inputs widen this transfer.
  Different producers still contribute by **union**, including recursive ones.
  This does not recover tuple correlations: `p(1,2). p(2,3). q(X):-p(X,X).`
  gives `q/1` the upper domain `{2}` although no complete `p` row matches.
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

Both lowerings use `numeric`'s synchronous engine. Each producer reads one frozen
previous state, its positive bindings intersect their argument domains, and all
producer outputs join with the fact seeds. Ascent also joins the previous state,
so no component narrows before convergence. Finite width overflow moves a set to
Unknown; another finite positive binding can still bound a conjunction. Numeric
endpoints use finite outward thresholds. A stable accumulated state W satisfies
F(W) ⊑ W; one complete unwidened image F(W) then narrows soundly. The final local
rule bounds are recomputed from that result. Work and round ceilings bound the
attempt, and no partial fixed point survives a global stop.

For the stated ordinary positive profile, every value in its concrete
least-model closure is covered. For broader forms the result deliberately
sacrifices precision rather than inventing a closed finite carrier. The tighter
bounds do not authorize a consumer to suppress source arithmetic or admission
diagnostics: analysis does not evaluate those expressions, and consumers retain
their original diagnostic boundaries.

## Bounds and verification

Independent ceilings cover work, inspected text, signatures (including nullary
ones), argument positions, transfers/producer/dependency links, retained value
references and fixed-point passes. Per-argument width overflow widens locally.
Per-symbol node count, depth and string/name payload are checked before a value
enters an ordered set; oversized symbols widen locally without deep comparison.
The bounded traversal checks child counts before extending its explicit stack.
Stored symbol comparisons therefore have bounded structure and payload.

Each finite meet scans its smaller set and checks both operands by binary search
over registered constant coordinates. An O(V log(V + 1)) meet retains O(V)
temporary references, where V is `max_values_per_argument`. Key classification
and semantic deduplication happen at registration; transfers copy references,
not symbol payloads. The same live-entry ledger covers seeds, current arguments,
candidate vectors and scratch sets, releasing reservations when their owners
are dropped, including failure paths. `Statistics::value_entries` reports live
occupancy at the receipt boundary, not a peak allocation count.

Lowering, expression and refinement visits, threshold searches, finite equality,
membership and reduction, metadata copies and actual key inspection charge work
before their operations. Logical receipts consequently differ from the previous
source-only engine; the limits remain inclusive and retain original stop context.
No old numeric receipt schedule is simulated. Statistics do not measure allocator
overhead or RSS. The source compatibility wrapper retains its host collection
allocation boundary. Its final `BTreeSet<&Symbol>` materialization is outside the
engine work meter: completed `value_entries` counts those entries, while `work`
counts lowering and the shared engine. Conversion costs at most
O(Σ arguments V log(V + 1)) ordered-set comparisons and O(total finite entries)
references; admitted symbol node/depth/payload limits bound each comparison.
The prepared-IR consumer reads coordinates and authentic keys directly, without
this source conversion. The neutral and prepared-IR path reports fallible metadata
allocation as typed failure rather than inventing a logical limit. Caller-supplied
query values and the already constructed input are outside this allocation bound.

## Neutral numeric analysis

`numeric::analyze(reader, limits, lower)` supplies one checked `Input` to the
lowering closure and returns `numeric::Analysis<K, C>`. The `Values` reader
classifies authentic keys and compares them using a consistent semantic equality;
its order is not used as ASP relational order. Finite `entries(argument)` lend
keys with `ConstantId` coordinates ordered only within that analysis. A consumer
may intersect those coordinates, then read the original key through its owner.

Each argument has a finite component, integrality (`Integer` or `Mixed`), and a
numeric interval (`Bottom`, bounded i32 endpoints, or `Unbounded`). Nonnumeric
values, including infinities, remain possible under `Mixed`; a numeric comparison
does not establish integrality. Every producer must be submitted, using a local
Unknown expression when precision is unavailable. Facts seed columns directly;
compact wide integer segments contribute their hull without row products.

`RuleBounds::domains` covers complete bindings before comparisons, whereas
`produced` includes a single ordered guard pass and is used for head transfer.
Neither establishes prefix coverage, arithmetic totality or binding existence.
Those stronger premises remain the responsibility of an execution consumer.
The engine retains authentic finite constants, copies and intersections; it does
not construct finite arithmetic image sets or new catalog keys. Generated ranges
and arithmetic therefore have Unknown finite components even when small.

The first static transfer scope includes negation, addition/subtraction,
multiplication by a literal, and division/remainder by a nonzero literal.
Unproved operators, including absolute value, bitwise operations, powers and
variable multiplication/division/remainder, yield Integer and Unbounded defined
results. Opaque Unknown summaries are never denotational identities, including
when their coordinates are reused in subtraction or guard/head expressions.
Count and sum assignments remain Integer and Unbounded; min/max remain Mixed and
Unbounded. Tighter aggregate envelopes remain open.

`Rule::integer_queries` requests separate integer-totality checks in that rule's
scope. `RuleBounds::integer` lends a certificate only when every intermediate
is defined over the complete-binding domains. Checked interval transfers cover
variable products, division and remainder, absolute value and a power whose
exponent has one nonnegative value throughout its domain. Zero divisors,
minimum-integer/-1 division or remainder, and possible intermediate overflow
decline; bitwise expressions remain outside the certificate grammar.
A declined request proves neither undefinedness nor impossibility. A successful
one does not establish that a binding exists or cover earlier join prefixes.
The prepared-IR observer uses these certificates for diagnostic reports; no
static pruning or family-admission shortcut is enabled by that report.

Every logical limit becomes `numeric::Status::Stopped` with all queryable results
cleared. Reader/control failures, invalid plans and fallible allocation remain
typed failures with committed statistics and original context. No failure becomes
a finite key, an empty domain or a claim about answer-set existence.

```sh
cargo test -p zetesis-domain --offline
cargo clippy -p zetesis-domain --all-targets --offline -- -D warnings
cargo doc -p zetesis-domain --no-deps --offline
```

The portable suite checks exact input/symbol/provenance identity, producer-local
intersection and producer union, signed and repeated positions, mixed symbols,
recursive source order, local/anonymous scopes, unresolved context, every global
resource category and all intersection work cutoffs, local width widening and
deep symbols. Two 128-case Proptest campaigns share an independent concrete
unary positive fixed point over random facts and conjunction rules including
cycles. One requires equality when no widening is needed; the other requires
upper coverage under deliberate width widening. Test parsing is confined to
development fixtures; the production API receives Program directly. No Lean
refinement or external-oracle coverage is claimed for this crate.

Different immutable Programs can be analyzed independently. The synchronous
transfer/reduction mechanism may allow independent components to run in parallel
after profiling. This bounded implementation has no threading/device dependency
and makes no hardware speedup claim. The source bridge consumes domain analysis
for eligible grounding guards and terminal-definition reconstruction; those
operations belong to the grounder and solver, not this analysis crate. Static
certificates, stronger aggregate precision and demand/magic-set rewriting remain
future work. The numeric tests additionally cover mixed-term guard conservatism,
recursive caps, opaque expression identity, complete-stop publication and actual
work at finite/threshold/refinement inspection boundaries. These tests do not
establish Rust refinement of the Lean laws.
