# Rust theory extensions beneath themelios

2026-09-06. Architecture investigation and proposed implementation contract.
No theory syntax, propagator registration, CP solver, or theory-aware result API
is implemented by this report. Existing theory refusals remain correct.

**Recommendation:** make a custom theory extension a versioned semantic component
with an exact witness relation, then add sound propagation as an optional
acceleration of that component. themelios owns its public Rust vocabulary;
zetesis supplies independent candidate construction, exact reduct membership,
joint answer accounting, and CPU/GPU execution. The first concrete satellite
should implement a bounded clingcon-style integer profile. A difference-logic
satellite should validate that the interface is general before it is frozen.

This advances the goal of a massively parallel, GPU-hosted ASP engine. It does
not require adopting clingo's search algorithm or moving arbitrary Rust callbacks
onto a GPU. A theory's numerical domains can remain compact during construction;
Boolean proposals and independent numerical states can be processed in batches.
Useful whole-solve acceleration remains a measurement obligation.

There are two future entry routes through the **same** extension boundary:
direct zetesis admission of clingcon/clingo-dl theory syntax using themelios's
parser and registered theory language, and an engine adapter beneath the
themelios Rust solve API. Direct CLI support need not wait for the facade's final
ergonomics, but must use the same semantic catalogs, witnesses, and capability
checks. Neither route is being implemented in this tranche.

## 1. What themelios already establishes

The inspected checkout is
`c4d4045dd0e248ce1922dfa6aa3bfa968b7c6d1d`; zetesis still depends on
`87c11a3f2b72b81a12fd53226941fdf95e7294d3`. The inspected solve design,
program design, specification, theory parser, owned theory algebra, and rewriter
are byte-identical at these revisions. The inspected analysis safety file differs
in recursive arithmetic-finiteness handling; this report does not transfer a
current-checkout analysis guarantee to the dependency pin. Exact hashes are in
the companion `reviewed-inputs.json`.

`docs/design/solve.md` is explicitly pre-implementation. Only base, syntax,
program, and analysis crates exist in the inspected themelios workspace. The
design specifies `Backend`, `Capabilities`, `Fault`, `Determination`,
`Conclusion`, typed theory assignments, and shared extension registration. Its
propagator surface has `init`, `propagate`, `undo`, and `check`, with a `Send`
state owned by each worker. Its final signatures are deliberately unsettled.
The DL/CP/LP litmus requires difference logic, full CP global constraints, and
linear/real arithmetic to be pleasant to implement. Theories are separate Rust
satellites; the foundation owns their integration contract.

The existing syntax/program layers do represent theory declarations, atoms,
guards, and element conditions. `TheoryTerm` is a distinct peer of ordinary
`Term`: operator sequences need interpretation under the applicable theory
definition. The owned `TheoryAtom` retains typed arguments, elements, guard, and
provenance. Parsing this structure does not establish a supported theory, its
operator semantics, safe grounding, or any constraint-solver capability.

The future bridge must consume typed AST/`Program` values directly and retain
`Origin`. Grounded theory records belong beside the Boolean formula, through the
companion theory sink envisaged by themelios. They should not be rendered and
reparsed or disguised as ordinary displayed atoms. A complete ground observer
is a separate capability; requesting it can force work a lazy solve did not need.

Two design issues should be fed back when themelios's API is implemented:

- `Box<dyn Propagator>` in the draft cannot erase the unspecified associated
  `State` by itself. Use a generic public registration method and an internal
  object-safe factory producing erased worker objects, or an equivalent safe
  type-erasure adapter. Do not expose `Any` downcasts to theory authors.
- The draft streams an `AnswerSet` while also requiring a typed theory component.
  The final model envelope must bind both components to the **same** answer and
  version. A detached mutable "last theory assignment" cannot represent a queued
  batch or two answers with equal ASP atoms and different numerical witnesses.

## 2. The semantic choice comes before callbacks

The primary hybrid-ASP account distinguishes two independent choices: whether a
theory atom requires rule support, and whether its absence imposes the opposite
constraint. Its principal profiles are external/strict and founded/non-strict;
its clingcon account uses external atoms, while the difference-logic account
also uses founded atoms. Numerical assignments are witnesses, which need not be
unique for one Boolean model. These are semantic distinctions, not callback
scheduling policies. [Cabalar, Fandinno, Schaub and Wanko, 2023, §§2–5](https://www.cs.uni-potsdam.de/wv/publications/DBLP_journals/algorithms/CabalarFSW23.pdf)

For zetesis, the following is a **proposed, explicitly bounded realization** of
the external/strict profile. Its source correspondence must be proved and tested
for each admitted language construct; the paper is not an implementation proof.

The paper's source profile also matters: its rules have a single head atom or
falsum, and founded theory atoms are excluded from rule bodies (§2). Its results
therefore cannot be transferred wholesale to arbitrary Ferraris formulas,
recursive founded theory occurrences, or every construct zetesis already admits.
Any broader profile below is an engine design proposal needing its own source
correspondence proof and admission rules.

Let `A` be regular Boolean atoms, `Q` the finite registered theory-atom catalog,
and `E ⊆ Q` the atoms declared external by the selected theory language. Let `B`
be the faithful Boolean lowering of the source with this catalog. Establish one
immutable original theory:

```
B_E = B ∪ { e ∨ not e | e ∈ E }
```

Each `e` receives input choice support. Founded atoms receive none. The language
adapter must classify atoms explicitly; `#external`, an ASP external directive,
is a separate concept and must not silently select this theory policy.

Let `v` be a valuation of typed constraint variables and `C(M, v)` the declared
theory relation at the **outer candidate** `M`. For fixed ground constraints,
the initial relation requires every true theory atom's constraint; each false
strict atom requires its exact complement; a false non-strict atom adds no
constraint. Any variable-domain and activation obligations are included in `C`.

The first hybrid answer contract is:

```
hybrid_answer(M, v) ⇔
    models(M, B_E)
    ∧ ¬∃ J ⊂ M, models(J, reduct(B_E, M))
    ∧ C(M, v)
```

The existing Ferraris oracle remains authoritative for the Boolean component.
For a fixed `M`, a selected external atom is forced by its choice reduct; it is
not removed merely because `J` is smaller. Numerical values are not ordinary
atoms minimized by set inclusion. The constraint witness is checked separately
with its declared numerical semantics. Check ordering may change for efficiency
because acceptance is conjunction, but all required results must be exact before
publication.

This is more precise than "solve ordinary ASP, then run the propagator": the
**original lowering has changed intentionally** to express external support.
For example, with `a :- e.`, external `e` denoting `x=1`, and domain `{1}`, plain
ASP has no support for `e`. Filtering plain ASP's empty answer loses the required
hybrid answer `{e,a}`. At the opposite boundary, adding a choice for founded `f`
in `f :- f.` invents a supported `{f}` even when its numerical constraint happens
to be satisfiable. This founded-cycle example is a **generic extension-boundary
counterexample outside the paper's admitted source profile**, because founded
`f` occurs in the body. It is not asserted to be valid clingcon source, whose
theory atoms have the external policy described above. Keep such internal
semantic tests distinct from valid-source clingo/clingcon conformance cases.

An atom's false strict interpretation must denote the logical complement over
the **same declared domain**. The complement of an integer inequality needs
checked arithmetic. The complement of `alldifferent` is an existential collision,
not universal equality; of equality, a disequality, which can require splitting.
Unknown or unsupported complement handling cannot be interpreted as success.

### Profiles that need a different exact checker

Keep three distinct capabilities, rather than a flag claiming universal theory
support:

| Semantic profile | Required acceptance machinery |
| --- | --- |
| Fixed propositional theory plus witness relation | Exact `B_E` reduct membership and exact `C(M,v)`; recommended first implementation |
| Faithful finite compilation of the entire extension | Original Ferraris oracle over a proved encoding, with auxiliary projection and witness reconstruction laws |
| Founded/ordered numerical or interpretation-dependent extension | A specified extended reduct/equilibrium order and an exact checker for it; reject until available |

Finite compilation is a valuable small reference, but eagerly encoding every
integer value into ordinary atoms can recreate the grounding explosion. It
must not be the mandatory production route for large domains.

The 2026 preliminary account of bound-founded difference logic explicitly
distinguishes unconstrained numerical witnesses, undefined values, and canonical
ordered witnesses across clingcon, flingo, and clingo[DL]. Its correspondence to
the current clingo[DL] implementation is itself qualified in the paper. It is
useful design input, not evidence that zetesis already supports those semantics.
[Cabalar et al., July 2026, §§1, 5–6](https://arxiv.org/html/2607.21201v1)

Consequently, a DL extension may start with a clearly named fixed-witness
feasibility profile, but matching **clingo-dl's output convention** additionally
needs the appropriate variable-definedness/canonicalization contract. It cannot
claim compatibility by returning an arbitrary satisfying potential vector.
More ambitious numerical foundedness should introduce a separate typed oracle
input, as the original architecture already separates Horn closure and general
Ferraris theories.

## 3. Propagation restricts proposals; it does not establish stability

For a partial Boolean assignment `α`, numerical domains `D`, and an immutable
version `V`, define the extension's possible completed answers as
`Solutions(V, α, D)`. A propagation step may narrow `α` or `D` only while
preserving that set. A conflict requires proof that it is empty. Failure to
find a completion is inconclusive unless the required search was exhaustive.

The interface should express consequences as data:

- a Boolean implication with its premises;
- a domain narrowing with its premises and exact variable identity;
- a conflict with its premises;
- an optional branching suggestion, which provides no pruning authority.

Every consequence has an explanation scope: version-global candidate region,
assumption-scoped region, objective-incumbent region, or one particular frozen
reduct query. A checked theory implication can be represented as a clause or a
formula restriction; that representation does not require CDNL or nonchronological
backjumping. The scheduler can apply it to a chronological frontier or intersect
it with a batch's live states.

**Default: theory consequences are candidate-only.** They never enter the
immutable original `B_E` and never constrain an inner `J` query. A lemma
licensed only by final theory satisfiability does not automatically hold of
every model of an arbitrary formula's frozen reduct. Similarly, an incumbent
bound may prune future improving answers but cannot provide atom support.

A useful adversarial test for a generic extension API is `a :- a.` with an
additional candidate filter requiring `a`. The filter leaves no stable answers.
If its restriction is inserted as an original fact, `{a}` becomes stable. If
the same filter is used to discard the inner empty countermodel, `{a}` can also
be accepted incorrectly. This deliberately generic filter is not presented as
the initial clingcon profile; it exposes why a reusable extension interface must
keep semantic roles separate.

Numerical entailment alone also cannot derive a founded Boolean atom. Even if
the current domains force the constraint represented by founded `f` to be true,
`f` still needs the rule support required by its policy. For an external/strict
atom, the corresponding reification implication can be valid. The explanation
checker therefore needs the atom policy as well as the numerical payload.

An optional **reduct propagator** needs the stronger law that its consequences
preserve every possible `J ⊂ M` satisfying the specified frozen original reduct.
Its context must expose the fixed outer `M` separately from the current inner
partial assignment. It cannot call the ordinary total-model theory check on `J`
and assume equivalence. Initially, arbitrary custom callbacks should have no
inner-query capability. Valid specialized device operators can gain one after
their own refinement argument.

Explanations narrow the trust boundary only when checked. Safe Rust, a
`Sound` enum name, and a plugin's claimed certificate do not prove an implication.
The first trusted Rust satellite is part of the semantic trusted base. A smaller
independent checker should verify numerical witnesses and, where available,
conflict certificates. Final witness checking catches wrong accepted assignments;
it cannot recover answers already pruned by a false conflict. Completeness
therefore also depends on sound explanations or a trusted propagation kernel.

## 4. Fit the current proposal/check/commit boundary

The reviewed `zetesis_ferraris::Theory` is immutable and instance-identified;
`Interpretation` belongs to exactly one such theory. `StableModels` separates
candidate restrictions from the original theory, and batches retain pending
candidates across failed checking. The current GPU result
`BatchVerdict::NoProperSubset` means Boolean membership for that exact original
theory; it is not a theory witness. The upcoming owned completion boundary is
the right seam for extension work, without adding callbacks inside formula nodes.

Proposed flow:

```
typed Program + registered theory languages + context version
  → bounded source admission and theory normalization
  → immutable (B_E, theory catalog, domains, objective/observation metadata)
  → proposals, optionally narrowed by validated theory consequences
  → independently owned Boolean membership and theory-witness obligations
  → joint verified answers
  → scoring, observation, publication, coverage commit
```

The Boolean checker and theory checker may run concurrently for a candidate.
An exact rejection from either can cancel its remaining work. Late results need
candidate/version identities so they are neither committed twice nor credited
to a reused slot. Rejecting a numerical branch does not reject the whole Boolean
candidate unless every relevant numerical branch has been covered.

There are two distinct completion ledgers:

1. Boolean candidate coverage and exact reduct membership.
2. Theory witness coverage for each retained Boolean candidate, including all
   requested assignments or proof that none exists.

One found witness proves existence only. For all-answer enumeration, blocking a
Boolean candidate after its first witness would lose further joint answers.
The Boolean generator may block it once ownership has transferred to a durable
witness-enumeration cursor, but the run cannot be exhausted while that cursor
has pending work. Caching one completed Boolean stability decision across many
witnesses is sound under the first profile because `B_E` and `M` are unchanged.

The result envelope should own regular atoms, theory assignments, costs,
semantic-profile/version identity, and completion provenance. Regular `#show`
projection and theory-value projection are views; neither defines full joint
identity. A model-suppression option must state which quotient it enumerates.

## 5. A concrete clingcon route

The installed reference is clingcon **5.2.1**, established by its installed
header and package metadata, inspected read-only. Its public header includes
`clingo.h`; registration/preparation require `clingo_control_t`, rewriting uses
`clingo_ast_t`, and model notification/extraction uses clingo model and per-thread
assignment interfaces. The wrapper follows register → rewrite → ground → prepare
→ solve/model callbacks. The official shared theory API documents the same
dependency. [clingo 5.8 theory API](https://potassco.org/clingo/python-api/5.8/clingo/theory.html)

This does **not** provide a binary plugin ABI for zetesis. Emulating those opaque
objects would mean implementing substantial clingo machinery and would conflict
with the chosen foundation boundary. Reusing numerical algorithms after a
bounded source audit is a separate possible port; no complete source audit or
claim of source-level reuse has been completed here. Some upstream GitHub source
fetches were unavailable; the installed header and wrapper, rather than guessed
internal implementation details, establish the concrete dependency finding.

The intended production route is a Rust finite-domain CP satellite behind the
shared themelios propagator contract, with separate clingo and zetesis adapters.
The upstream binary remains an independent differential oracle. **Language and
result compatibility** is the target, not the fiction that the existing binary
runs inside the native backend.

The official clingcon project identifies finite-domain integer constraints as
its purpose; the maintained release notes also record fixes for multi-shot
solving and optimal-model enumeration. These are separate compatibility
dimensions requiring their own campaigns. The older project landing page and
vendored examples must not be treated as a current exhaustive feature matrix.
[clingcon repository](https://github.com/potassco/clingcon),
[release notes](https://github.com/potassco/clingcon/blob/master/CHANGES.md)

| Requirement | Proposed native mechanism | Essential acceptance evidence |
| --- | --- | --- |
| `&dom` and integer variables | Typed finite interval-set domains, checked bounds, explicit activation | Empty/disjoint/conditional domains; zero and negative endpoints; no silent default-domain substitution |
| `&sum` comparisons | Sparse affine records plus exact checked evaluation and bound propagation | Positive/mixed coefficients, duplicate terms, empty sums, strict complements, all relation operators |
| `&distinct` / global CP | Dedicated constraint object; simple pairwise reference and stronger propagator | Duplicate expressions, equal values, empty/singleton constraints, reified polarity, witness parity |
| Boolean/theory interaction | Declared atom support/reification policy and ordinary selector formulas | Head/body/default-negated occurrences, cycles, inactive constraints, shared variables |
| Structured variable names | Typed keys such as `digit(L)`, tuple keys, namespace and origin | Preserve identity separately from an ordinary same-named ASP predicate |
| `&show` and assignments | Typed assignment projection from the joint model envelope | Full witness identity retained when display is empty or hides variables |
| ASP `#minimize` | Existing ASP scoring plus theory witnesses for each feasible Boolean answer | Full optimal assignment ties, not just one witness per Boolean optimum |
| Theory `&minimize` | Separate exact numerical objective and bounded witness optimizer | Improving trajectory vs proven optimum; theory-valued ties; unsupported combinations refused |

Conditional theory elements need a language-specific contract for activation,
scoping and duplicate coalescing. Existing ASP aggregate tuple semantics cannot
simply be reused because two constructs look alike. Start with unconditional
elements or conditions decided by a completed, proven static relation. Admit
dynamic theory-element conditions only after their exact lowering is recorded.

An explicit `&dom` declaration can be part of the theory formula, rather than an
unconditional engine domain restriction: a conditional declaration must not
constrain an inactive branch unless that is its specified meaning. Likewise,
ordinary ASP variables used to create a theory variable name are distinct from
the numerical variable represented by that name. Domain analysis should keep
both roles visible.

For combined ASP/theory optimization, define the ordered cost vector in the
shared API. Do not infer how priorities or defaults compose from spelling.
Initially refuse ambiguous combinations and differential-test admitted ones
against the selected clingcon version. An ASP-optimal Boolean answer may have
many numerical witnesses, and a numerical objective may rank those witnesses
differently. Proven optimality requires coverage of both remaining Boolean
regions and relevant numerical regions. Any bound using a verified incumbent
must retain ties when all optimal answers were requested.

The read-only themelios corpus contains **38 clingcon scenario files, three
standalone cases, and six reusable encodings**. These 41 scenario/standalone
entry files are prospective targets, not passed zetesis cases. In particular:

- `standalone/send-money/send-money-clingcon.lp` combines explicit digit domains,
  a global distinct constraint, one affine equation, and an empty regular display.
  Its assignment witness is essential to parity.
- `standalone/task-scheduling/task-scheduling-clingcon.lp` requires two co-optimal
  joint models and binds an order atom to the corresponding start-time values.
  Matching an order from one answer with times from another must fail.
- Time-windowed TSP represents arrival times as numerical variables. It can
  avoid materializing every time-valued ASP atom, making it a direct scalability
  target for this architecture rather than a domain-specific optimization.

These are corpus contracts to replay, not newly measured performance claims.

## 6. Rust API and ownership

The companion [API sketch](theory-propagator-api.md) is a proposal for themelios's
unsettled contract and zetesis's private adapter. It is not a second public
session framework and has not been compiled. Its important boundaries are:

- Immutable prepared catalog and constraint data, safely shareable between workers.
- One owned mutable state and rollback trail per active theory task; shared budget
  leases and cancellation are borrowed explicitly.
- Bounded propagation effects returned as data, admitted deterministically by the
  coordinator. No arbitrary reference to another worker or the whole solver.
- Exact witness search distinct from propagation and from independent validation.
- A separate optional device plan for an admitted declarative constraint family.

Use different newtypes for semantic atoms, theory atoms, theory variables,
constraint IDs, explanation IDs, and local Boolean auxiliaries. Include an
unforgeable in-process owner/version token; serialized cache records additionally
need canonical content identity. A `u32` slot from a previous version must not be
accepted because its numeric range happens to fit the new catalog. Rust
`TypeId` is useful for in-process dispatch, not persistent semantic identity.

Preserve themelios `Symbol`/`Origin` at the interface. Numerical values use the
solve tier's wider typed assignments; no narrowing into `Symbol::Number(i32)`
or rounded float conversion. A rational/real theory needs exact feasibility
certificates or explicitly limited capabilities. Approximate LP output alone
cannot authorize an exact answer, an UNSAT result, or an optimality claim.

Initial extensions should be statically linked Rust crates. Registration binds
implementation/version, configuration, numerical semantics, and any external
context snapshot. Dynamic loading, ABI stability, plugin isolation, and mutable
remote services require additional contracts. `&self` and `Send + Sync` do not
establish determinism or logical soundness.

## 7. Parallel and device execution

Parallelize across independent candidates and numerical domain branches first.
The immutable formula and theory catalog can be resident once; each lane owns
its Boolean assignment, bounds/domain state, trail, and unresolved obligations.
Avoid one shared lock around numerical propagation. Rayon workers share admitted
read-only records; nested plugin parallelism draws from the same bounded
execution pool rather than creating an unaccounted pool per candidate.

An arbitrary Rust propagator is a **host** operator. Schedule a batch of requests,
return compact restrictions/results, then resume resident device states. A
callback for every literal transition can eliminate the GPU advantage. Record
batch counts, host theory time, device theory time where actually measured,
transfers, suspended lanes, and numerical residual completion explicitly.

For device execution, accept a small declarative IR with a verified meaning:
finite-domain intersections, sparse affine bounds, reified comparisons, and
selected global-constraint operators. A CPU interpreter is the semantic
reference; WGSL lowering must preserve arithmetic, domain holes, activation,
explanation scope and termination status. Closed declarative operations provide
opportunities for scan/reduce/segmented work and batched fixed points.

Monotone domain narrowing is useful but **insufficient**: removing the wrong
value is also monotone. Each operator must preserve all solutions, and the
scheduler must either reach an appropriate fixed point or retain a residual.
An inconclusive device propagator hands the exact state to a host completer;
failed device execution is not a theory conflict. A fixed point without a
witness or exhaustive search is not satisfiability.

Use explicit bounded quanta and a frontier ledger. No busy-waiting global device
barrier is assumed. Theoretical convergence over finite domains does not give an
acceptable watchdog latency. Graph relaxations for a DL satellite, affine bounds
for CP, and a simple distinct propagator provide different workloads to test
the IR before adding elaborate device kernels. Any wider-than-supported numeric
operation takes a declared host path or refuses; it never wraps silently.

The current general GPU oracle checks the Boolean reduct. A future GPU-hosted
theory-aware solve needs both the Boolean frontier and the numerical frontier
to remain resident where the capability permits. If an opaque host plugin is
selected, report a hybrid route honestly. This applies equally to Metal and
NVIDIA; adapter discovery alone does not qualify a theory kernel.

## 8. Rollback, cancellation, sessions, and failure

Within one theory branch, narrowing may be destructive only with an owned trail
or an equivalent persistent-state representation. Forking a branch requires a
complete state snapshot or replay contract. `undo(mark)` must restore domains,
pending events, explanation generations, local auxiliaries, and objective state,
not just Boolean truth values. A token is valid only for its branch/version.

Retain the draft's familiar `init/propagate/undo/check` concepts if useful, but
do not require clingo's particular callback order. A clingo adapter can map
events onto that lifecycle; zetesis can reset or fork state for batched search.
The clingo API distinguishes program/solver literals, total/fixpoint checking,
and clause lifetimes. Those mechanisms inform adapter tests, while shared
semantic IDs and scoped effects protect engine independence.
[clingo 5.8 propagator API](https://potassco.org/clingo/c-api/5.8/group__Propagator.html)

Limits cover catalog size, variable-domain storage, all active worker scratch,
trail entries, effect/certificate bytes, frontier size, witness queues, total
work, and decisions. Limits charged independently per worker are not a shared
run ceiling. Lease disjoint allowances before parallel work and return unused
allowances explicitly; cancellation does not forgive already performed work.

Cooperative host callbacks must poll at bounded units. Checking a deadline only
before and after a callback does not bound an opaque callback's duration or
allocation. Hard preemption/isolation is a separate capability. Catching Rust
unwinding cannot contain `panic=abort`, undefined behavior in dependencies, or
process termination. Native callbacks need a declared failure boundary rather
than a universal "safe plugin" claim.

After a fault, preserve verified answers, pending Boolean candidates, unresolved
numerical branches, queued assignments, committed publications, and the original
cause. A failed output write is not exhaustion. A worker panic or cancellation
cannot be turned into an empty successful result. The current tranche's typed
partial-report work is therefore a prerequisite for a trustworthy extension
prototype, not documentation polish.

Multi-shot operations publish a new immutable combined version. Its identity
includes source, theory registration/configuration, domain definitions,
externals/assumptions, and relevant context. Previous incumbents need rechecking;
clauses and cached witnesses need explicit validity scope. Lazily adding a
previously omitted theory record during one solve must preserve that solve's
logical version and coverage proof. An arbitrary semantic update cannot be
smuggled in as lazy discovery. Pending records keep their original epoch or are
explicitly interrupted and retried.

## 9. Smallest useful implementation and later checkpoints

This investigation should land with the current three implementation tracks.
It should not expand the current language-parity claim to theory constructs.
The extension implementation can then proceed through reviewable checkpoints:

| Checkpoint | Deliverable | Exit evidence |
| --- | --- | --- |
| T0: semantic seam | Typed internal prepared extension and joint-result envelope; specified external/strict and founded/non-strict policy; no callbacks required | Tiny exact model/witness enumerator; external-support and founded-cycle examples; no effect on ordinary ASP |
| T1: finite integer reference | Rust host satellite for explicit finite domains and affine constraints, simple distinct reference, bounded witness enumeration | Complete tiny-domain differential/property campaign, fault/limit cases, both Boolean and witness coverage |
| T2: themelios portability | Adapt the actual shared registration contract once it exists; clingo and zetesis consume the same Rust satellite | No engine control objects in satellite code; macro/programmatic parity; typed origins and joint assignments |
| T3: practical CP | Incremental bounds and domain propagation with checked explanations; structured variable names, selected theory element conditions and objective support | Original applicable clingcon corpus, SEND and scheduling witnesses, all optimal assignments; profiling against T1 and pinned clingcon |
| T4: GPU theory execution | First bounded declarative theory IR and exact device/host completion protocol | Physical Metal tests on exact binary, matched scalar/Rayon/hybrid full-answer runs, actual theory branch work |
| T5: second semantic family | DL engine with explicit witness selection, then rational linear feasibility | Prove which semantics are supported; compare only with matching reference modes; refuse numerical foundedness not implemented |

T0/T1 should initially be internal or unpublished. Do not freeze a competing
public API before themelios-solve exists. T2 may require separate integration changes in themelios. A backend that
lacks the required profile returns a typed capability refusal before solving.

The smallest performance experiment should vary domain widths while holding
Boolean structure fixed. Measure admitted Boolean atoms, theory variables,
constraint records, peak resident bytes, proposals, reduct checks, numerical
branches, first joint answer, all answers/optimal ties, and complete wall time.
Compare exact reference, scalar propagated, Rayon, and device routes. This tests
whether compact numerical state actually avoids grounding growth; a fast theory
kernel alone is not the desired outcome.

## 10. Required tests and formal contracts

All test cases must name their semantic profile, source/version identity,
complete Boolean model, typed witness, objective, and requested projection.
Use clingo plus the shared Rust extension to test backend portability; use
independent clingcon to test numerical/language compatibility. Agreement with
the same buggy satellite on two backends does not establish theory correctness.
Use an independent exhaustive tiny-domain evaluator as a third oracle.

Required adversarial cases include external atoms without producers; founded
cycles; true/false strict reification; conditional domain declarations; mutually
inconsistent constraints; duplicate theory elements; no-witness vs interrupted
witness search; hidden assignment multiplicity; exact complements; overflow;
domain holes; stale IDs/epochs; rollback after conflict; malformed explanations;
allocation/cancellation during a batch; and failure after a verified joint answer.
Include multiple independently registered theories: either their variable spaces
are disjoint or a declared combination contract proves that local witnesses
agree. Separate satisfiability of overlapping theories is insufficient.

Proposed Lean work should establish, without claiming Rust refinement:

1. Input-choice support and its frozen-reduct behavior; correspondence to the
   selected finite hybrid semantic profile.
2. Joint acceptance iff Boolean stability and the exact witness relation under
   that profile; the limits of reusing this law for founded numerical semantics.
3. Candidate-region restriction preservation from a universally valid explanation;
   distinct stronger premises for a frozen-reduct restriction.
4. Domain narrowing and branch-split coverage, including preservation under
   composition, rollback, and disjoint frontier ownership.
5. Two-ledger completion: no exhausted joint enumeration while any Boolean
   candidate or witness cursor remains unresolved.
6. Exact witness checking and optimal-tie retention; caching Boolean membership
   across witnesses of the same `M` and immutable `B_E`.
7. Device-IR interpreter equivalence and bounded residual handoff for each admitted
   operator, with machine arithmetic refinement a separate task.

The 527 existing theorem checks and current portable coverage do not discharge
these obligations. New runtime code must meet the existing formatting, pedantic
Clippy, independent coverage, source-parity and resource/failure gates. A custom
theory platform extends the semantic trusted base; its component conformance,
versioned evidence and readable mathematical boundaries must be reviewed as
carefully as the core reduct oracle.
