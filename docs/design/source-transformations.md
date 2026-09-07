# Semantics-preserving program transformations

zetesis may optimize before grounding by transforming the typed themelios
`Program`. Only transformations with an established semantic preservation
contract belong on the production path. Applicability evidence and cost estimates
are distinct: a heuristic may choose among valid transforms, but cannot supply
their correctness premises.

## Implemented status

There is no general ngo-style `Program`-to-`Program` optimization pipeline yet.
The current pre-grounding `formula_ir::Normalizer` uses themelios's `Rewrite`
to substitute resolved constants and evaluate closed arithmetic while preparing
the finite rule IR. Variable-bearing expressions retain their later binding
semantics. This is source normalization, not the proposed collection of
optimization passes.

The implemented [body factorization](factored-source.md) is related to common
body extraction and join projection, but runs during finite formula lowering,
after possible-positive support construction. It can combine independent local
witness families without enumerating their Cartesian product. It constructs
shared eligibility formula roots rather than a rewritten source `Program` with
new predicates. Structural DAG interning, aggregate element/root caches and
shared threshold construction likewise operate during formula construction.

Predicate inlining, unused-predicate/argument elimination, aggregate-chain
rewrites and a reusable source-pass pipeline remain candidate work. Neither the
design inventory below nor the available themelios rewriting API establishes
that those passes have been implemented or qualified.

## The existing themelios foundation

The read-only themelios design `docs/design/program.md`, sections 2 and 9,
explicitly names an ngo-style non-ground optimizer as a consumer. Section 9
specifies `Visit`, `Rewrite` and `rewrite(Program, ...) -> Program`, substitution,
fresh names and provenance-preserving construction. These are implemented in
the pinned revision `87c11a3f2b72b81a12fd53226941fdf95e7294d3`.

The transformation framework rebuilds shared program values, preserves source
origins, stamps transformed nodes and canonicalizes output. It supplies structural
machinery, not an answer-set preservation certificate. Section 9.3 explicitly
assigns semantic justification to the transformation's author and implementation
checking to downstream differential tests. zetesis should use that boundary.

`Rewrite` is one statement to one statement. Projection, extraction or other
one-to-many passes need bounded construction through shared `Program` ingestion,
retaining program parts and provenance. At this pin, the public `Program::of`
constructor admits statements only to `base`; part-preserving `into_statements`
and `ingest_into` are crate-internal. A downstream one-to-many pass must therefore
require base-only input, or await a public part-aware construction API. Flattening
named parts into `base` is not a valid substitute. The existing public rewriter
does retain parts for its one-to-one transformations.

The separate `unpool` operation can
expand exponentially in output size; it is not a cheap prerequisite to every
analysis. Bound its relevant expansion before invoking it, or work directly over
the structured source. No rendered-source round trip is an integration API.

At the read-only local revision `c4d4045dd0e248ce1922dfa6aa3bfa968b7c6d1d`, the
following files were byte-identical to the dependency pin on inspection:

| themelios file | SHA-256 |
| --- | --- |
| `docs/design/program.md` | `b677ca1e520b872794b9a8fc72b92f904cfa81ef7d397f0be706d5ca5914eab0` |
| `crates/themelios-program/src/transform.rs` | `53ab28f235a5b054e1b6e09e8da3874703f0d041dca46099be673d8279397431` |
| `crates/themelios-program/examples/transformation.rs` | `1239b69b9bcd914b49c25a23927d91bd05b6e0ca2a30893e141b0aaa30f27e16` |
| `crates/themelios-program/tests/transform_laws.rs` | `733e745c0bcce42f9a85b5a6f67679db05d92ef7ca63aa7579f0da6b33661784` |

No sibling source or dependency pin was changed for this inspection.

Public `Rule::new` and `Body::new` create constructed child carriers; their
provenance-preserving `from_nodes` alternatives are crate-internal. A downstream
extraction can retain merged statement evidence with `WithProvenance::new`, but
must distinguish that evidence from exact original child spans. Moreover,
`WithProvenance` equality and ordering ignore origins. Semantic structural
equality does not establish source identity or equality of transformation evidence.

## Preservation contracts

For a fixed original vocabulary, a transformation must preserve complete stable
models and the applicable output, projection, count and objective contracts.
An auxiliary-atom transformation needs an explicit mapping: each original stable
model has an accounted-for extension, and every transformed stable model maps
back correctly. A unique extension gives a particularly useful bijection for
enumeration. Auxiliary alternatives must not multiply original answers.

Eliminating original predicates needs exact reconstruction where full original
models are part of the contract. Equal visible answers alone do not establish
that reconstruction. Objective preservation includes original priorities, global
tuple coalescing, costs and every requested optimal tie. Preserving only a best
cost or one optimum is insufficient for complete optimum enumeration.

For a reusable encoding, preservation must hold for all supplied inputs satisfying
the explicit interface assumptions. A pass valid only for one closed invocation
must be scoped to that invocation. Strong equivalence is useful for rewrites
under arbitrary later rule extensions; a narrower equivalence can be sufficient
under a precisely checked input/context boundary. Neither successful structural
reconstruction nor ordinary classical equivalence alone establishes preservation
for a nontrivial rewrite. Exact equality of the semantic program under the same
context does, of course, preserve it by reflexivity.

For example, `a :- a.` and `a :- not not a.` have classically equivalent rule
formulas, but different stable models. A transform that collapses default double
negation would fail the reduct contract. Likewise, a relationship holding only
in completed stable models cannot automatically simplify every proper-subset
reduct query.

The strongest convenient local check for a same-vocabulary formula transform is
preservation of original truth and frozen-reduct truth for every candidate and
eligible inner interpretation, under explicit premises. For fresh predicates or
eliminated definitions, prove the corresponding model-extension and reduct
mapping instead. The source-to-formula bridge remains a separate obligation.

The checked signed-identity transform is one concrete example of this boundary.
[StrongNegation.lean](../../proofs/Zetesis/StrongNegation.lean) renames complete
signed ground atoms injectively while retaining every connective, and proves
exact frozen-reduct transport and stable-model correspondence. Its coherence
constraints filter co-present opposite identities without supplying support.
Complete pair coverage is an explicit premise; an omitted entry has a checked
counterexample. These formula-level laws do not establish the themelios source
rewrite, Rust interning or generated registry. In particular a source rewrite
must not replace `-p` by `not p`, merge the two signatures, or discard a needed
coherence constraint because one polarity is hidden by `#show`.

## ngo as a source of candidate passes

[ngo](https://github.com/potassco/ngo) is a Python non-ground optimizer. Its
inspected [API](https://github.com/potassco/ngo/blob/main/src/ngo/api.py) takes
input and output predicate lists and composes selectable cleanup, unused-symbol,
duplication, symmetry, aggregate-chain, arithmetic, inline and projection passes.
This is useful reference material for a native Rust design. zetesis does not
depend on ngo or invoke its Python/clingo AST pipeline in production. The
inspection is of published `main` source, not a pinned qualification of ngo.

| Candidate family | Required zetesis evidence | Initial assessment |
| --- | --- | --- |
| Exact duplicate removal and Boolean identities | Preserve scopes, original and reduct truth; retain merged source evidence | Small semantic surface; shared program canonicalization already handles some duplication |
| Common body extraction and join projection | Exact interface variables, local scopes, safe bindings and auxiliary extension mapping | High priority; related to existing bounded body factorization |
| Predicate inlining or definition reconstruction | Complete producer analysis, recursion conditions, exact substitution and fresh names | Useful after a bounded applicability recognizer and mapping proof |
| Domain-guided filtering and redundant conditions | Sound upper/lower evidence for the same immutable source context and every required reduct state | Pair with independent domain analysis; incomplete information cannot justify pruning |
| Aggregate chains and arithmetic rewrites | Recursive aggregate semantics, full tuple identity, signed weights, empty sentinels, exact integer/undefined behavior | Further targeted work; ordinary algebraic identities alone do not admit a pass |
| Unused predicates or argument elimination | Validated input/output contract and reconstruction of all required original models | Output relevance alone is insufficient for the default full-model contract |
| Symmetry-related transforms | Distinguish duplicate binding representations from distinct stable models; preserve or reconstruct all required models | Do not enable a reduction that discards required answers |

ngo's [projection implementation](https://github.com/potassco/ngo/blob/main/src/ngo/projection.py)
already checks binding and scope conditions before splitting a rule. Its
[unused-symbol pass](https://github.com/potassco/ngo/blob/main/src/ngo/unused.py)
considers declared inputs and outputs and describes preserving answer-set counts.
These implementation intentions help identify obligations, but do not certify a
Rust port or establish full original-vocabulary reconstruction. No blanket
correctness or incompatibility verdict about ngo follows from this inventory.

## Implementation and qualification

Each pass should produce a transformed shared program plus located transformation
evidence: its name/version, exact input identity, applicability checks, output
identity, vocabulary mapping, resource outcome and preservation-contract kind.
Existing analyses describe the old program; recompute them for the new program
or transport them through a separately justified law. Preserve the immutable
original theory used for acceptance, or establish a checked semantic bridge for
the transformed oracle before changing that boundary.

Bound traversal, substitution output, new statements, fresh symbols, provenance
growth and fixed-point pass cycles. Stage a pass transactionally: if an optional
optimization cannot complete, retain the original program. An optimizer budget
stop does not reject the original language or imply UNSAT. Rewriting may change
charged work and let a case finish earlier; it cannot bypass source failures by
treating undefined arithmetic or unhandled constructs as known false.

The pinned `rewrite` and `unpool` entry points are infallible and have no
cancellation channel. A caller must establish a safe expansion bound before
entering them; a cancellation check afterward cannot bound work already spent.
Passes needing interruption during construction require a bounded consumer
implementation within the supported public construction surface.

Independent rule analyses can use Rayon after preflight and scope separation.
Fresh-name assignment and deterministic evidence merging need explicit ownership.
Seed fresh-name allocation from the complete immutable context. Independent
copies of that allocator can choose colliding names; parallel passes need a
shared reservation plan or disjoint namespaces with checked collision freedom.
Do not assume whole-program passes are independent: they can change each other's
preconditions. wgpu is appropriate for sufficiently large exact relational
operations; small AST walks may remain cheaper on CPU. Measure analysis and
rewrite overhead together with subsequent construction and solving.

For each admitted pass, add adversarial source tests, small exhaustive or
Proptest comparisons, complete clingo before/after comparisons and regressions
for the original corpus. Test recursion, choices, disjunction, hidden models,
tuple collisions, arithmetic boundaries and interface updates as applicable.
Lean establishes the semantic law under named premises; runtime tests check the
applicability recognizer and transformation implementation. Current Lean proofs
for body factorization and exact DAG sharing do not verify a general ngo-style
source optimizer.

The first implementation should extend a narrowly proved transformation already
useful in profiling, not enable a large pass collection at once. The
[input/output contract](program-interface.md), [domain analysis](domain-analysis.md)
and [observation demand](demand-and-magic-sets.md) can provide its premises while
remaining independently reusable over themelios `Program`.
