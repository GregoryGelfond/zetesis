# zetesis — Lean semantic specification

This package is also a reusable mathematical proof library, independent of the
Rust solver. Its [reading guide](guide/README.md) starts from logic-programming
questions; the [structured proof convention](STYLE.md) and
[worked membership proof](guide/certified-membership.md) make the main argument
readable before its Lean details. The convention has one deliberately bounded
pilot so far; it is not a claim of a library-wide rewrite.


This package accompanies the [zetesis v0.2 specification](../docs/design/zetesis.md). It contains **783 kernel-checked theorems** across 67 semantic modules, using **Lean 4.33.1** and its standard library. There are no external package dependencies, project axioms, proof holes, or native-evaluation proof shortcuts.

The formalization connects normalized stable-model semantics to candidate seeds, compositional reduct execution, lazy completion, consequence bounds, and completed search certificates. It supplies mathematical contracts for the Rust, wgpu, Rayon, and neuromorphic implementation work. It does not verify those implementations or make Lean a runtime dependency.

## Reproduce

With the pinned toolchain installed, run in this directory:

```sh
lake build
lake env lean -DautoImplicit=false -DwarningAsError=true Audit.lean
```

`lean-toolchain` pins `leanprover/lean4:v4.33.1`. The package has no remote dependencies, so the build itself needs no network once that toolchain exists. `lake build` compiles the umbrella module and all 67 semantic modules. `Audit.lean` requests the transitive axiom dependencies of every project theorem.

The [verification report](./verification.json) records the checked source hashes and commands. The [axiom audit](./axiom-audit.txt) contains only standard Lean logical axioms where needed: `propext`, `Quot.sound`, and `Classical.choice`. In particular, no theorem depends on `sorryAx` or `Lean.ofReduceBool`.

`python3 scripts/proof_record.py`, run from the repository root, checks the
recorded source and artifact hashes, theorem index and source locations, audit
names and allowed axioms, counts, and links to the current verification logs.
The `proofs` and `full` check modes run this read-only consistency gate after
Lean. It does not replace kernel checking or prove implementation refinement.
The [2026-09-06 record correction](verification/record-correction-20260906/README.md)
preserves an earlier inconsistent manifest and explains its repair.

## Proof map

| Module | What is proved |
|---|---|
| [TightEvaluation](./Zetesis/TightEvaluation.lean) | Topological Boolean evaluation computes unfolded original truth; indexed producer OR reduction computes support; syntactic producer links, complete carrier and positive ranks compose with exact residual checking |
| [AggregateDependencies](./Zetesis/AggregateDependencies.lean) | Complete dependent aggregate families retain each predecessor association and equality in original and frozen clause rows; complete carriers remain premises |
| [BinaryWatch](./Zetesis/BinaryWatch.lean) | Two distinct binary watch positions exhaust every possible replacement position, independently of literal availability and inspection order |
| [OuterNegativeConsumers](./Zetesis/OuterNegativeConsumers.lean) | Default/double-negative outer gates retain aggregate equality and activation in the frozen reduct; projected gates quantify over the complete supplied witness family |
| [OuterRanges](./Zetesis/OuterRanges.lean) | Complete outer carriers retain their own inclusive integer ranges, occurrence-preserving concatenation and original/frozen clause associations under total endpoint functions |
| [JoinFrames](./Zetesis/JoinFrames.lean) | Root reset and complete child overwrites make positive-prefix membership independent of retained frame contents; an unreset root can erase current truth |
| [ChoiceConsumers](./Zetesis/ChoiceConsumers.lean) | Completed scalar filters retain aggregate equality, whole-group activation and natural bounds under explicit proposal coverage and evaluation assumptions |
| [NegativeEligibility](./Zetesis/NegativeEligibility.lean) | Negated and double-negated eligibility read the frozen candidate; replacing double negation by positive support has a counterexample |
| [OptionalIndex](./Zetesis/OptionalIndex.lean) | Representable positive-successor identities preserve optional index round trips, absence and link replacement |
| [WorldMasks](./Zetesis/WorldMasks.lean) | Immutable positive-prefix membership and exhausted masked scans preserve per-world consequence and constraint coverage; stale snapshots cannot authorize later omissions |
| [StructuralBindings](./Zetesis/StructuralBindings.lean) | Consistent finite named-slot matching preserves prebound values, rolls back on refusal and selects exactly compatible rows under complete support coverage; retained source atoms preserve contextual original/frozen truth and stability |
| [FiniteValues](./Zetesis/FiniteValues.lean) | Declared-input agreement preserves deterministic partial evaluation; single-slot extension and complete constructor identity preserve resolved literal meaning, including every frozen reduct |
| [ConsequentAlternatives](./Zetesis/ConsequentAlternatives.lean) | Universal condition rows retain existential signed alternatives; completed empty domains have distinct meanings, and complete formula collection equality preserves contextual stability |
| [Outcomes](./Zetesis/Outcomes.lean) | Complete regional UNSAT depends on verified membership, not publication; delivery retains membership validity and empty delivery can hide a model |
| [BatchAccounting](./Zetesis/BatchAccounting.lean) | Proposal, pending classification and exact prefix commitment preserve finite coverage and occurrence counts; interrupted or delayed failures cannot establish exhaustion |
| [CertifiedExecution](./Zetesis/CertifiedExecution.lean) | Ranked support soundly precedes exact residual completion over the unchanged original theory; interruption cannot accept and fallback quotas accumulate |
| [TrueHeads](./Zetesis/TrueHeads.lean) | Atom-free true-condition erasure preserves whole-rule families, every frozen reduct and contextual stability; positive producer necessity is retained |
| [CountHeads](./Zetesis/CountHeads.lean) | Complete tuple/atom correspondence gives duplicate-free representatives, original/frozen tuple activity equivalence and contextual count-group stability; bounds provide no reduct support |
| [ValueExtrema](./Zetesis/ValueExtrema.lean) | Complete-value extrema candidate coverage and ordered-selection witness laws, with exact original/frozen guard connectives under an explicit comparator law |
| [FinitePools](./Zetesis/FinitePools.lean) | Independent occurrence products cover complete formula rows and preserve original/frozen models and contextual stability; disjunction flattening has a counterexample |
| [FiniteBindings](./Zetesis/FiniteBindings.lean) | Executable closed-bound intersection with retained total guards preserves finite instantiations, original/frozen formulas and stability in context; source safety and runtime refinement remain separate |
| [NegativeHeads](./Zetesis/NegativeHeads.lean) | Default-negated disjuncts retain frozen truth; stable models have positive producers; double-negated necessary-support guards preserve stability for the ground grammar |
| [TightPlans](./Zetesis/TightPlans.lean) | Checked complete normal/choice roots and positive ranks make original satisfaction plus support equivalent to reduct stability |
| [GroundGuards](./Zetesis/GroundGuards.lean) | Total atom-free guard evaluation preserves original truth, every frozen M/J pair and stability in arbitrary formula context; whole-chain negation scope and atom-dependent counterexample |
| [Core](./Zetesis/Core.lean) | Symbolic predicate sets and elementary inclusion/equality laws |
| [Transformers](./Zetesis/Transformers.lean) | Least closure, sound/closed exactness, composition, fusion, independent world batching, sound finite prefixes |
| [Semantics](./Zetesis/Semantics.lean) | Positive reduct minimality, frozen gates, constraints, accepted-seed/stable-model equivalence and uniqueness |
| [Iteration](./Zetesis/Iteration.lean) | Finite derivation stages and finite convergence of normalized programs |
| [Lifted](./Zetesis/Lifted.lean) | Bind/filter/gate/project composition, safe gate pushdown, sparse materialization, constraints, and snapshot invalidation |
| [LiftedBridge](./Zetesis/LiftedBridge.lean) | Conceptual-grounding equivalence and stable-model acceptance after sound lazy stages with final-snapshot coverage |
| [Search](./Zetesis/Search.lean) | Exact region partitions, complete certificate outputs, uniqueness, regional UNSAT, and sampling counterexamples |
| [Bounds](./Zetesis/Bounds.lean) | Must/may enclosure, acceptance-preserving narrowing, and concrete constraint/bound refutation |
| [Events](./Zetesis/Events.lean) | Legal atom-publication traces, soundness, idempotent publication, and equality of completed legal schedules |
| [Ferraris](./Zetesis/Ferraris.lean) | General formula reduct, subset-minimal stability, frozen choices/negation, and a reduct with no least model; aggregate/source translation remains unproved |
| [FerrarisMask](./Zetesis/FerrarisMask.lean) | Correct frozen-mask evaluation equals the explicit formula reduct, extends to theories, and preserves the subset-minimal stability criterion |
| [FerrarisGuards](./Zetesis/FerrarisGuards.lean) | Double-negated arbitrary formulas freeze candidate truth; adding such guards filters stable models without creating new ones, and preserves all stable models when they entail the guards |
| [ObjectiveDirections](./Zetesis/ObjectiveDirections.lean) | Executable mixed minimize/maximize normalization with global complete-key grouping and OR eligibility preserves independent raw integer costs, fixed-priority vectors, lexicographic comparisons, all original-theory optima and exact tie lists |
| [Optimization](./Zetesis/Optimization.lean) | A finite integer-cost minimum reducer and tie filter return exactly the global optima given sound and complete stable-model coverage; all optima retain original frozen-reduct minimality |
| [QueryCompaction](./Zetesis/QueryCompaction.lean) | Executable signed-wire and recursive classical-query compaction; full single-gate Tseitin truth and unique output, canonical signed AND/commuted-input sharing; compaction after the original frozen mask preserves countermodels and stability; arbitrary-variable branches partition assignments |
| [DagSharing](./Zetesis/DagSharing.lean) | Executable structural node interning and index remapping preserve exact unfolded theories, original truth, arbitrary frozen reducts and stability; exact duplicates reuse existing nodes |
| [Thresholds](./Zetesis/Thresholds.lean) | Executable nonnegative weighted threshold recurrence has exact Boolean truth, is antitone in its bound, and preserves satisfaction when the evaluated sum grows |
| [AggregateAssignment](./Zetesis/AggregateAssignment.lean) | Executable signed subset-sum candidate generation covers actual whole-tuple sums under explicit carrier coverage; OR-coalescing preserves sums; generated values need not be realizable |
| [CandidateCursor](./Zetesis/CandidateCursor.lean) | Executable fuel-bounded forest traversal, exact remaining-leaf accounting, finite completion, duplicate-free semantic projection coverage and separate original-Ferraris stable-output filtering |
| [ExtremumCandidates](./Zetesis/ExtremumCandidates.lean) | Numeric min/max candidate coverage under an explicit complete value carrier, distinct empty extrema, selected-value membership and finite candidate length |
| [RuleFactorization](./Zetesis/RuleFactorization.lean) | All pairwise body implications equal a factored existential body for original truth and every frozen reduct; stability is preserved in arbitrary unchanged theory context |
| [UniversalConditionals](./Zetesis/UniversalConditionals.lean) | Finite universal conditionals preserve each implication in original/frozen semantics; false-in-candidate removal and per-candidate specialization preserve enclosing-rule stability, while static removal requires uniform domain falsity |
| [ChoiceIntervals](./Zetesis/ChoiceIntervals.lean) | Executable closed integer ranges and independent argument products implement relational bindings; same-group expansion with OR-coalesced head eligibility preserves classical truth, every frozen reduct and contextual stability; duplicate heads count once and empty reversed groups retain their constraints |
| [StrongNegation](./Zetesis/StrongNegation.lean) | Injective signed-atom renaming preserves exact reduct syntax and stable models; finite coherence constraints exclude opposite complete atoms without supplying support; bounded groups are transported intact and missing coherence coverage has a checked counterexample |
| [ClauseValidation](./Zetesis/ClauseValidation.lean) | Executable first-true clause scanning equals existential literal acceptance; whole-CNF validation requires every clause; inspected work is bounded by literal count, with exact false-prefix/first-true savings and empty-clause rejection |
| [Propagation](./Zetesis/Propagation.lean) | Complete Boolean gate relations, safe local projection, finite sequential/parallel narrowing, delayed-snapshot safety, semantic strict-subset domains and conflict certificates under explicit encoding coverage; quiescent nonempty domains need not have a model |
| [SignedObjectiveBounds](./Zetesis/SignedObjectiveBounds.lean) | Candidate-only signed weights become nonnegative complemented terms plus fixed offsets after whole-key coalescing; shifted scalar and lexicographic bounds preserve candidate regions, original-theory optima and exact ties |
| [ObjectiveBounds](./Zetesis/ObjectiveBounds.lean) | Executable scalar incumbent filtering, dominance, preservation of all global optimum ties under completed bounded coverage, and unchanged original reduct minimality |
| [IndexedCandidates](./Zetesis/IndexedCandidates.lean) | Executable exact Boolean trie insertion/lookup equals all semantic blocking clauses; certified failed-literal forcing preserves classical query regions |
| [Feedback](./Zetesis/Feedback.lean) | Fixed-witness residual formulas exactly characterize original reduct countermodels; one or several candidate feedback filters preserve all stable models; Unit choice defeats unqualified antichains; cone exclusion requires explicit classical minimality |
| [SingletonHeads](./Zetesis/SingletonHeads.lean) | Original and frozen singleton implications preserve signed literals; default-negated heads supply no positive support |
| [ConstructorPatterns](./Zetesis/ConstructorPatterns.lean) | Exact constructor shape, whole-subtree extraction and transactional binding over explicit paths |
| [ScalarArithmetic](./Zetesis/ScalarArithmetic.lean) | Checked range/error policies and operation substitution over finite plans preserve first failures |
| [GateProjection](./Zetesis/GateProjection.lean) | Finite bit-mask gate projection agrees with the Boolean relation under explicit physical-slot aliases |
| [PositiveArguments](./Zetesis/PositiveArguments.lean) | Independently supplied inputs determine captured-value checks; finite filtering retains complete supporting rows and exact atoms in original/frozen contexts |
| [AggregateConsumers](./Zetesis/AggregateConsumers.lean) | Ready schedules use only initial or preceding outputs; covered proposals retain their downstream value association and original aggregate clause |
| [StructuredWitnesses](./Zetesis/StructuredWitnesses.lean) | Compatible positive witnesses retain complete source atoms and completed condition bindings; identical complete row collections preserve contextual stability |
| [OrderedProbes](./Zetesis/OrderedProbes.lean) | A monotone finite boundary search partitions rows exactly; certified windows retain every full match under an explicit key implication |
| [AggregateBounds](./Zetesis/AggregateBounds.lean) | Aggregate proposals retain their equality and activation; active natural count intervals reject outside candidates while inactive instances impose no frozen obligation |
| [CountEligibility](./Zetesis/CountEligibility.lean) | Complete tuple/atom correspondence preserves original eligibility, indexed witness lists and selected activity in original/frozen thresholds |
| [EvaluationPrefix](./Zetesis/EvaluationPrefix.lean) | A finite partial-operation plan reads only its initialized prefix; storage reuse preserves values and first errors, and reset makes old cells irrelevant |
| [LazyRounds](./Zetesis/LazyRounds.lean) | Complete union scans cover individual worlds; independent per-world antecedent and gate checks preserve exact consequences, constraints and fixed-snapshot chunk composition |
| [Examples](./Zetesis/Examples.lean) | Choices break Gamma antimonotonicity; omitting a constraint gate can lose a stable model |

[theorems.json](./theorems.json) indexes every theorem by qualified name, source file, and line. All theorem assumptions remain explicit in the Lean declarations.

The central operational result is `Zetesis.LiftedBridge.completed_lazy_stage_accept_sound`. Earlier materialization stages may omit bindings; they still add only justified consequences. Final headed-rule coverage, closedness, constraint coverage/no violation, complete gate-carrier coverage, and seed agreement then establish stability.

## Assurance boundary

- `Stable` independently specifies minimal-model semantics of the **normalized gated reduct**. themelios parsing, source admission, source choice normalization, and domain safety still require preservation proofs.
- `ConceptualGrounding` requires the full source registry to correspond exactly to a finite normalized rule list. The runtime need not enumerate that list, but this package does not prove the adapter constructs the correspondence.
- Abstract bindings do not enforce source variable safety. In particular, the singleton empty substitution for an empty source body must be established by normalization.
- Finite convergence is a mathematical existence theorem. No numeric round bound, executable stopping test, search traversal, fairness, or resource guarantee is extracted.
- A coverage tree verifies a supplied completed certificate. It does not construct arbitrary refutation evidence, prove a search scheduler terminates, or verify concurrent model delivery.
- Event proofs cover justified atom insertion. They do not verify rule-edge counters, packet delivery, reset/configuration, or hardware quiescence.
- Frozen-mask equivalence assumes the mask gives the outer candidate's classical truth for each formula. It does not verify Rust DAG indices, stored mask construction, packed membership, resource accounting, or exhaustive subset enumeration.
- Guard preservation is generic formula semantics. The proof does not establish that a Rust graph compiler constructs necessary supportedness formulas, or verify the Tseitin encoding, watched-literal SAT solver, decision trail, blocking clauses, or their resource accounting.
- Optimization ranks models using one abstract unbounded integer cost. The proof does not establish source objective presence, lifted joins, tuple coalescing, multiple-priority comparison, machine arithmetic, or refinement of the Rust evaluator and incumbent manager. Global optimality requires complete stable-model coverage; a partial enumeration does not supply it.
- Objective direction normalization uses finite already-ground entries, an arbitrary scalar type with exact decidable equality, unbounded integer weights and one fixed priority list. Its independent reference charges each active direction-aware full-key class once; the compiler normalizes signs before global grouping and retains OR eligibility. It preserves vector values and all optimal ties over the unchanged Ferraris theory. It does not establish source grounding or slot/presence discovery, tuple eligibility in actual source models, finite-width negation or overflow behavior, resource completion, source admission, search exhaustion or Rust refinement.
- Query compaction acts after the original candidate truth mask is frozen. Its classical identities do not justify rewriting an original Ferraris formula before that mask is computed. Tseitin extension is proved for one gate's free Boolean output, not the entire Rust CNF allocation, fresh-index registry, shared DAG, SAT traversal or blocking implementation. Branch partition permits any selected variable but does not verify the occurrence-count heuristic, its sorting, DPLL termination or resource handling.
- Structural sharing is an executable abstract list-table scan with syntactic topological admission. It proves original-to-shared root bounds and exact unfolded formula equality, so it may share the original theory before freezing a candidate mask. It performs no classical rewrite. The theorem does not verify the Rust hash map, packed evaluator, resource limits, source binding independence or cache key. Reusing a complete tuple-eligibility graph across changing assignment values still requires proving that its tuple keys and conditions are unchanged over the same final support universe.
- The threshold recurrence uses natural weights and arbitrary classical condition queries. It establishes the arithmetic query recurrence, not aggregate-to-Ferraris strong equivalence, source tuple grouping, signed-weight preprocessing of original aggregate formulas, finite machine arithmetic or the Rust dynamic program's shared DAG representation. Monotonicity in the evaluated sum does not assume arbitrary negated conditions are monotone in atom membership.
- Aggregate assignment uses unbounded integers and an explicit covering tuple carrier. Its theorem does not silently discard missing actual tuples. OR-coalescing is proved for supplied Boolean eligibility values; deriving those values from source bindings or frozen condition reducts remains separate. Runtime limits, cancellation, checked scalar conversion, recursive support completion, source safety, objective priority presence and Rust generator refinement are not established. Initial or partial candidate lists cannot be treated as complete, and candidate values can be unrealizable because conditions are correlated.
- Retained candidate traversal uses an executable finite forest and explicit fuel. Exact semantic blocks remove every auxiliary extension of a recorded candidate. Completed traces cover all original unblocked projections exactly once; interruption preserves the remaining candidates. A complete trace exists with sufficient mathematical fuel. These results do not identify the forest with a Rust CNF/watch/trail state, prove chronological backtracking or machine resource accounting, or supply external reduct-check evidence. The stable-output theorem filters by the original Ferraris stability definition; it does not refine a SAT implementation.
- Numeric extrema select an input integer when nonempty and use distinct empty minimum/maximum sentinels. Candidate coverage explicitly requires every actual value to occur in the completed possible-value list. It does not prove source tuple eligibility, recursive-support completion, aggregate-to-formula strong equivalence, machine scalar arithmetic, cache invalidation or Rust refinement.
- Choice intervals use mathematical integers and an independently stated finite binding/table correspondence. Cardinality counts distinct active head/eligibility pairs, not all true head atoms or raw local witnesses. Both bounds are candidate constraints and cannot supply reduct support: the lower threshold is double-negated, and the upper threshold is negated. The recursive singleton `1 {a:a} 1` is proved to have no stable model. A common duplicate-free complete head carrier and unchanged outer body/bounds/context remain explicit. Conditions are arbitrary Ferraris formulas; every frozen M/J pair is covered, without assuming J is a subset of M. Repeated interval occurrences are independent slots. A reversed product yields no rows, but a positive lower bound remains a constraint; splitting a bounded group changes its models. These are denotational expansion and formula laws, not themelios recognition/raising, source safety, generated-slot allocation, support completion, machine-width interval arithmetic, resource bounds, objective presence, Rust cursor or cardinality/DAG refinement proofs.
- Strong negation is a distinct signed identity over a complete ground base atom (predicate and argument tuple). Injective renaming preserves original truth, exact syntax-tree reducts, and stable models; stable target interpretations contain no unencoded atoms. Default negation remains implication to bottom. Finite coherence constraints are candidate filters and cannot support either polarity. Global coherence requires explicit coverage of co-present opposite pairs; a supplied sound possible-atom enclosure and complete pair registry suffice. A one-polarity carrier need not introduce an unused opposite atom. The module proves conflicting signed facts UNSAT, an explicit negative fact stable, and independent polarity choices coherent; omitting a required registry entry permits an incoherent model. Bounded choice groups retain their existing constraint-only bounds and arbitrary surrounding theory. Source recognition/raising, signed predicate and tuple interning, complete registry construction, possible-carrier enclosure, generated Rust constraints, source/output spelling, objective/observation matching, budgets, and all backend implementations remain unverified.
- Clause validation assumes a fixed complete interpretation and total Boolean literal truth. It proves existential clause acceptance, universal CNF acceptance, literal-inspection upper bounds, exact all-false and first-true prefix work, and empty-clause rejection. The count excludes assignment construction, allocation and other solver operations. It does not prove Rust completeness/index checks, counters, budgets/cancellation, SAT state, witness handoff or Ferraris reduct construction. Interrupted validation supplies no completed acceptance theorem; the original frozen reduct remains authoritative.
- Boolean-domain propagation uses complete finite local relations and an immutable query. Projection retains every satisfying completion through finite sequential sweeps, parallel intersections and delayed projections from larger domain snapshots. Aliased local positions may admit inconsistent support rows, which weakens propagation but does not discard a global completion. A false frozen gate fixes only its output to false; it does not impose the disabled connective on its inputs. Semantic subset domains and strict removal exclude auxiliary variables, and the empty candidate has no strict subset. Stability from an empty propagated domain requires an original classical model and explicit coverage of every true reduct countermodel by an encoded completion. The auxiliary-variable bridge states that coverage premise independently of the source/compiler/backend. Candidate objective restrictions are absent from the inner query. A checked quiescent equality/disequality network retains every value but has no model, and a classically satisfied self-implication still has a strict reduct countermodel. These laws do not prove complete constraint encoding, DAG allocation, mask storage, Rust/WGSL projection, atomic updates, scheduling/convergence, budgets/cancellation, search coverage or device execution. Quiescence is residual information; an interrupted or exhausted propagation budget supplies no stability result.
- Rule factorization assumes an explicit Cartesian product of two finite formula families with one fixed head. It proves original and arbitrary frozen-reduct equivalence, then stability in an unchanged theory context. It does not prove that source variable dependencies, fixed head bindings, component joins or complete support establish that product; nor does it refine the Rust multi-component compiler, scalar-totality check, supportedness-guard rebuilding or resource accounting. No restriction is imposed on the condition formulas themselves.
- Incumbent-bound proofs use one unbounded integer cost and a verified original stable model. Completed constrained coverage suffices for global optimum ties because excluded stable candidates are strictly worse. This does not prove complete priority-vector bounded-search coverage, objective tuple compilation, transactional SAT restriction/restart behavior or Rust refinement. Candidate-only signed bound normalization is covered separately below.
- Exact projection indexing uses a recursive Boolean trie and complete finite keys. It proves equality to a linear reference of exact blocking clauses, including the empty key. It does not refine the flat Rust arena or fixed-width admission. Failed-literal forcing requires an actual refutation of the chosen classical branch; it does not establish Rust unit propagation, trial undo, watch correctness or interruption handling.
- Countermodel feedback fixes both the original theory and the atom carrier. Its residual syntax-tree transform is denotational over membership predicates; the proof does not implement a finite proper-subset circuit, shared DAG construction, witness verification, SAT restriction/restart, storage/work ceilings or program-version invalidation. Feedback filters candidates while original Ferraris stability remains the acceptance condition. The Unit choice counterexample has comparable stable models; cone pruning is proved only under an explicit premise that every stable model is a minimal classical model of the original theory. No source-fragment recognizer or splitting theorem is proved.
- Predicate semantics proves permission for sparse evaluation and independent batching. Concrete indexes, cursors, packed bits, synchronization, and performance remain implementation obligations.

The accompanying [Rust reference](../validation/reference/README.md) provides independent executable differential checks. There is currently no formal Lean-to-Rust refinement proof.

The [Ferraris/clingo extension](../docs/design/ferraris.md) connects this formula-level foundation to the full non-clingcon corpus target. The 13 Ferraris foundation theorems, three frozen-mask theorems and nine candidate-guard theorems supplement the 144 original S0/abstract-execution theorems. They do not establish aggregate translation, full-source compatibility, graph-supportedness entailment, or refinement of the Rust formula DAG evaluator or SAT search.

## Batched candidate accounting

`BatchAccounting.lean` adds 17 checked laws for finite queued, pending, accepted
and rejected candidate stores. Proposal and exact prefix commitment preserve
coverage and occurrence counts. Sound classification plus complete producer
coverage yields exactly the required accepted candidates. Pending values and
delayed failures each prevent exhaustion; an empty pending queue alone is not
completion. A hybrid-completion law keeps the original Ferraris theory fixed
and makes partial-checker soundness and exact native residual checking explicit.

This ledger does not refine Rust semantic-block insertion, unique candidate
identity, callback ordering/cardinality, allocation, cancellation or GPU
transport. Accepted entries are membership results, not proof of successful
external output emission. Producer coverage and objective-safe pruning remain
separate obligations. The fresh build and complete axiom audit are retained in
`verification/batch-accounting-20260906/`.

## Fully evaluated ground guards

`GroundGuards.lean` adds 12 checked laws for expressions over already evaluated,
total Boolean results. Its executable evaluator is independent of both the
candidate and tested reduct interpretation. Replacing selected guards by their
computed truth constants preserves every original and frozen formula context,
and stable-model identity with unchanged other roots. This includes guard
positions within a fixed finite choice or aggregate formula.

Comparison chains conjoin adjacent results; default negation applies to the
whole chain, and double negation preserves that conjunction. A concrete mixed
true/false chain distinguishes this from conjoining individually negated
comparisons. An atom-dependent `p or not p` formula is classically tautological
but fails the required frozen-pair equivalence with truth.

The inputs already contain total evaluated results. Source arithmetic, undefined
operations, overflow, comparison ordering, safety and variable binding, complete
finite substitutions, aggregate/choice construction and Rust refinement remain
separate obligations. In particular, undefined arithmetic cannot be treated as
false and then negated. The source parser's nonempty-chain convention is not
proved by the mathematical empty-chain case. Fresh proof commands and the prior
record are retained in `verification/ground-guards-20260906/`.

## Signed objective candidate bounds

`SignedObjectiveBounds.lean` adds 17 checked laws. On an already direction-normalized
and globally coalesced list of complete objective keys, a negative weight `w`
on eligibility `E` becomes `-w` on Boolean `not E`, with the fixed offset `w`.
Nonnegative terms retain their eligibility. The grouped list remains intact;
there is no second deduplication of transformed magnitudes or conditions.
The resulting nonnegative sum plus its offset equals the original signed sum.
Each priority shifts its own bound by its own offset, preserving strict, equal
and non-strict comparisons and complete lexicographic ordering.

These laws preserve the candidate bound region, all original-theory optimum
predicates and ordered tie lists. Every optimum survives a non-strict bound
from a verified incumbent. The existing completed scalar bounded-region theorem
is reused with an explicit normalized-region coverage premise; this neither
asserts actual search completion nor proves complete vector-region enumeration.

The transformation acts only on classical candidate queries. It is not a
strong-equivalence rewrite of signed aggregates in the original Ferraris
theory, whose reduct remains authoritative. Mathematical integers do not prove
Rust's checked offset/shift arithmetic, finite-width magnitude representability,
fallback behavior, graph compilation, restriction transaction or runtime resource
accounting. Eligibility is a supplied total Boolean query; source grounding,
priority/presence discovery and source-to-query refinement remain separate.
The [retained proof record](verification/signed-objective-bounds-20260906/README.md)
preserves the prior 496-theorem manifest and the fresh build/audit logs.

## Finite universal body conditionals

`UniversalConditionals.lean` adds 14 laws over a supplied finite list of condition
and consequent formulas. It reuses the existing conjunction and implication
semantics and proves the full original/frozen meaning of `AND (C → H)`, including
vacuity for an empty completed list. Conditions and consequents remain arbitrary
formulas; the enclosing rule's other body conjunct, head and surrounding theory
remain unchanged.

For one fixed candidate `M`, an instance with `C` false in `M` has a false reduct
antecedent in every tested `J`. Removing such instances preserves original truth
at `M`, every frozen reduct test and stability of `M` in the enclosing rule
context. This gives a precise permission for per-candidate lazy specialization.
Every surviving implication must retain `C`; `J` may disagree with `M` about it.
Static reuse of a filtered list requires discarded conditions to be false
uniformly over the supplied candidate domain. Checked counterexamples distinguish
cross-candidate reuse and erasing an antecedent found true in `M`.

The mathematical list is supplied. Source variable safety, complete substitutions,
possible-positive support coverage, source joins and filtering, generated support
guards, interrupted enumeration, graph construction and Rust refinement are not
proved. An empty incomplete prefix does not establish source-level vacuity.
The [retained proof record](verification/universal-conditionals-20260906/README.md)
contains fresh commands and the prior 513-theorem records.

## Integrated language and certificate contracts

The [7 September integration record](verification/language-tranche-20260907/README.md)
adds 57 laws across five modules to the previous 556-theorem inventory. The fresh
pinned build and strict audit cover all 613 declarations in all 41 modules. The
prior inventory, audit, umbrella, README and manifest are preserved byte for byte.

`CertifiedExecution.lean` adds six laws connecting a supplied sound ranked-support
verdict to exact residual completion. Both paths judge the unchanged original
Ferraris theory. An interrupted attempt returns no membership result, and a
failed optional attempt still consumes a cumulative natural-number quota before
fallback. Complete producer coverage, actual work accounting, Rust extraction,
scheduling, allocation and external output delivery remain separate obligations.

`TrueHeads.lean` adds fourteen laws for conditions that are atom-free and true in
original truth and every frozen reduct. Erasure preserves each complete rule in
the supplied interval product and stability in unchanged context. Empty products
remain empty rule families; negative occurrences retain their polarity, and
positive-producer necessity is retained. The module does not prove source parser
recognition, safety, bindings, complete support, generated ranges or Rust lowering.
It does not justify erasure of arbitrary classical tautologies or dynamic guards.

`CountHeads.lean` adds eleven laws for a completed static-eligibility table of full
tuple keys and positive derived atoms. The executable correspondence check rejects
both alias directions. A supplied complete representative table has distinct
atoms and covers all rows. Tuple activity equals its representative atom in
original truth and every frozen reduct; the interval choice/constraint group
therefore preserves stability in unchanged context. Bounds inspect the candidate
and provide no reduct support. Complete finite eligibility and representative
coverage are explicit premises. Source safety, the Rust map/cursor/checking and
resource implementation, dynamic conditions, general aliases, negative derived
literals, other head functions and objective interactions remain unproved.

`ValueExtrema.lean` adds eighteen laws over an arbitrary complete value carrier.
Selected values belong to the supplied list; completed carrier coverage includes
every actual extreme and a separately supplied empty sentinel. Ordered bound
witnesses require an explicit selection/comparator law. Full-key condition OR
and guard connectives preserve original truth and every frozen M/J test with
arbitrary eligibility formulas. The Rust ASP order, finite-width endpoint policy,
source safety, complete support, key/cache identity and budgets are not verified.

`FinitePools.lean` adds eight laws for independent finite occurrence products and
an independently specified relational binding relation. Complete row coverage
preserves original models, all frozen M/J reducts and stability in unchanged
context. Duplicate alternatives remain in enumeration, and an empty occurrence
empties the product. Each product row denotes a whole formula; flattening a
family of disjunctions changes its truth. Source recognition, cursor/provenance,
choice-group construction, generated bindings, support completion and resources
remain adapter obligations.

## Structural binding and finite consequent alternatives

The [grounder tranche record](verification/grounder-tranche-20260907/README.md)
extends the prior 616-declaration inventory by 30 laws in three modules. That
checkpoint had 646 theorems in 45 semantic modules. Four
previously private structural extension/single-binding helpers are public laws
with explicit contracts; their statements and proof bodies are unchanged.

`StructuralBindings.lean` gives 14 laws over partial bindings and finite extracted
constraints. Successful matching preserves prebound values and makes every named
occurrence agree; a consistent target witnesses success. Failed row transactions
publish the incoming binding unchanged. Complete supplied support rows cover
exactly the compatible matches. Captured source atoms remain logical leaves, so
their identity preserves original and arbitrary frozen truth and stability in
unchanged theory context. Tuple traversal, extraction, source scope/safety,
support completion, machine resource accounting and Rust refinement are separate.

`FiniteValues.lean` gives eight laws over fixed partial operations and declared
source reads. Agreement on those reads preserves results and failures; a failed
step cannot publish a shortened success. Single-slot extension preserves every
other slot, and constructor identity keeps the name, sign and ordered children
while distinguishing tuples. Literal transport assumes that the resolved source
atom is unchanged and preserves each negation depth in original and frozen
contexts. Machine operations, allocation/depth preflight, source safety and
actual compiler correctness are not proved by these abstract contracts.

`ConsequentAlternatives.lean` gives eight laws for complete finite condition rows
with a disjunction of signed consequent formulas inside each implication.
Universal condition rows and existential alternatives have separate scope.
A completed empty row list is vacuous; a row without alternatives requires its
condition to be false. Polarity belongs to each alternative, and moving negation
outside their disjunction has a concrete counterexample. Complete coverage of
identical consequent formulas permits collection replacement in unchanged theory
context. Local witness coverage, source joins and scope, projection analysis,
resource completion and Rust refinement remain separate obligations.

## Execution laws

The execution tranche adds 40 checked laws in five independently importable
modules: signed singleton heads, constructor patterns, checked scalar plans,
finite Boolean gate projection and consuming positive arguments. The complete
execution checkpoint had 686 theorems in 50 semantic modules. These are scoped mathematical
contracts; source compilation,
Rust execution, memory/resource accounting and physical shader refinement remain
separate obligations. The [execution audit](verification/positive-arguments-20260907/README.md)
identifies the exact proof sources and commands and preserves the prior
677-theorem record. Positive argument checks assume independently established
inputs and complete support rows; they do not prove source binding inference,
Rust joins, arithmetic inversion or resource refinement.

## Dependency and ordered-probe laws

The [dependency audit](verification/dependency-tranche-20260907/README.md) added
sixteen laws in three modules. That checkpoint had 702 theorems in 53 semantic
modules. It preserves the previous 686-theorem record and all fifty
prior semantic source files unchanged.

`AggregateConsumers.lean` gives eight laws over an already chosen finite schedule
and an explicitly covering aggregate proposal carrier. A ready instruction reads
only initial slots or earlier outputs. An accepted actual proposal keeps its
computed consumer value, and each clause retains the original aggregate formula;
proposal membership itself asserts neither that formula nor the head. The frozen
identity law assumes equal computed values. These laws do not prove the Rust
sorter, source readiness, actual aggregate coverage, machine evaluation, cursor
backtracking or resource completion.

`StructuredWitnesses.lean` gives six laws for supplied constructor-shape tests and
named-slot constraints over finite positive witness rows. Matching preserves the
completed condition binding or rolls back on refusal. Original and frozen truth
require the retained complete source atom, and the condition remains the logical
antecedent. Complete row-collection identity preserves stability in unchanged
context. Faithful shape/extraction, actual support coverage, source scope and
Rust traversal are separate obligations; negative quantifiers and arithmetic
inversion are outside these laws.

`OrderedProbes.lean` gives two laws for an immutable Boolean predicate whose true
positions form an initial segment. Width-decreasing binary search establishes a
complete finite partition. A pair of certified boundaries retains every full
match when full matching implies the key conditions. Relating Rust's storage
order and parent binding to those predicates remains an explicit representation
obligation. The laws do not establish ASP value order, source-carrier completion,
matcher rollback, Rust resource/cancellation behavior or physical execution.


## Aggregate bounds, evaluation storage and lazy rounds

The [preceding audit](verification/aggregate-bounds-lazy-metal-20260907/README.md)
extends the library by 21 laws in four modules to **723 theorems in 57 semantic
modules**. All 702 prior declaration locations, axiom sets and 53 prior semantic
source files are unchanged. These modules develop fundamentals needed by the
solver; they do not broaden this release into a general ASP literature project.
The original theory and its frozen reduct remain the acceptance foundation.

`AggregateBounds.lean` gives four laws about natural count intervals guarded by
an aggregate equality and rule activation. Active instances reject counts outside
the interval; inactive instances impose no obligation in the frozen reduct. A
proposed value alone cannot activate the group. Complete carriers, actual bound
evaluation, distinct head identities and source safety remain explicit premises.
Signed machine bounds, generated Rust constraints and stronger partial-search
pruning are not verified by these laws.

`CountEligibility.lean` gives seven laws extending complete tuple/atom
correspondence to arbitrary retained eligibility formulas. Key and atom indices
select the same ordered witnesses, including duplicates. Tuple activity equals
the representative head conjoined with its coalesced eligibility, in original
truth and every frozen M/J pair; finite thresholds inherit that equivalence.
Possible-support traversal, complete distinct representative tables, source
scope, Rust maps/cursors and resources remain separate obligations. Weighted or
extremal heads, general aliases and negative head literals are outside this module.

`EvaluationPrefix.lean` gives three laws over finite plans of supplied pure
partial operations. Appending initializes exactly the next cell; evaluation over
an explicit live prefix agrees with evaluation over live values alone, including
the first error. Resetting the live prefix excludes old values. Operation validity,
operand indices, scalar arithmetic, logical charging, Rust allocation, `Drop` and
unwinding remain unproved implementation correspondences.

`LazyRounds.lean` gives seven laws about shared source instances and separate
world snapshots. A completed union scan covers each world's enabled bindings,
but union membership does not establish truth within that world. Independent
positive-antecedent and frozen-gate checks yield exact consequences and constraint
detection. Chunk composition keeps the same immutable snapshot and seed; a
completed round has the corresponding closed/exact interpretation only under
fresh complete coverage. Previous-snapshot exhaustion does not certify a later
snapshot. Rust traversal, atom-ID encoding, budgets, WGSL, readback and publication
remain separate obligations. These laws provide no physical Metal qualification.

## Completed consumers and compact execution

The [preceding clean audit](verification/consumers-memory-20260908/README.md)
adds 25 laws in four modules, reaching **748 theorems in 61 modules**. All 723
previous declaration locations, axiom sets and 57 semantic module sources remain
unchanged. This is a mathematical library supporting solver verification; the
concrete Rust and WGSL implementations are not thereby verified.

`ChoiceConsumers` preserves the original aggregate equality and whole-group
activation around completed scalar filters and natural choice bounds. Complete
proposal carriers and total evaluation are hypotheses. `NegativeEligibility`
instantiates retained eligibility with negation and double negation, and gives a
counterexample to replacing double negation with positive support. Neither module
proves source scheduling, signed machine arithmetic, safety or tuple correspondence.

`OptionalIndex` gives successor bounds, exact optional-index round trips and a
commuting link replacement. Representability is explicit; Rust's layout, allocator,
watch relocation and search charging remain outside the proof. `WorldMasks`
relates positive-prefix intersections to per-world source coverage and exact
consequences/constraints under an exhausted scan. It includes cross-world and stale
snapshot counterexamples. Packed bits, concrete traversal, byte/work accounting
and device execution remain unproved correspondences. These semantic foundations
preserve the original theory and its reduct as the acceptance criterion.

## Outer values and reusable positive-join frames

The [preceding clean audit](verification/consumer-execution-20260908/README.md)
adds 15 laws in three modules, reaching **763 theorems in 64 modules**. All 748
prior declaration locations and axiom sets and all 61 prior semantic source
modules are unchanged. The declined compact-trail experiment contributes no
module or claim to this checkpoint.

`OuterNegativeConsumers` preserves original aggregate equality and activation
while ordinary and projected negative gates read the frozen candidate. Supplied
witness families must be complete; possible support cannot substitute for truth.
The laws do not prove Rust source admission, variable safety, projection coverage,
cursor readiness, argument evaluation or resource completion.

`OuterRanges` expands each complete outer value into its own inclusive integer
range. Concatenating outer rows preserves their full occurrence expansions,
including repeated values, and each clause retains the corresponding original
or frozen row association. Complete carriers and total mathematical endpoint
functions are premises; fixed-width arithmetic, nonnumeric endpoint behavior,
source scheduling, cursor reset and actual support coverage remain unproved.

`JoinFrames` formalizes a finite positive-prefix schedule over world predicates.
Root reset and child overwrite retain exactly current membership regardless of
old frame contents. A stale-root example demonstrates why storage reuse requires
that schedule. Packed bits, tail masks, array bounds, Rust visitor scheduling,
allocation lifetime/budgets and WGSL correspondence remain separate obligations.
These additions strengthen the reduct-based foundation without claiming formal
verification of the concrete solver.

## Dependent aggregate producers and binary replacement

The [current clean audit](verification/dependencies-measurement-20260908/README.md)
adds nine laws in two modules, reaching **772 theorems in 66 modules**. All 763
prior declaration names, source locations and transitive axiom sets and all 64
prior semantic source modules are unchanged.

`AggregateDependencies` composes supplied complete aggregate candidate families
indexed by their completed predecessor rows. Equal child values do not erase
parent association. A signed-sum specialization retains complete full-tuple
coverage, and emitted clauses retain both predecessor activation/equalities and
the dependent equality in original truth and every frozen M/J pair. Total
mathematical value functions and complete carriers remain premises. Rust safety,
input analysis, topological scheduling, local joins, caches, cursor resets,
fixed-width arithmetic and resource completion are not proved.

`BinaryWatch` shows that two distinct valid positions exhaust a binary clause,
so an unwatched replacement search returns none under any availability predicate
and inspection order. Rust must still establish the watch invariant and preserve
unit/conflict decisions. Registry maintenance, candidate order, work charging,
CNF encoding, reduct construction and executable refinement are not proved.
Neither addition weakens the original-theory and frozen-reduct acceptance
foundation or claims that the concrete solver is formally verified.

## Boolean evaluation for ranked support

`TightEvaluation` adds 11 laws connecting the actual topological Boolean fold to
the existing unfolded formula representation. Root and indexed-body checks
therefore compute original truth; producer-head OR reduction and complete
present-atom scanning compute supportedness. Exact syntactic producer links,
original producer membership and a positive rank connect those operations to
ranked-support stability. Residuals still require exact completion against the
original theory. The [worked reading](guide/tight-evaluation.md) separates the
computed correspondence from the supplied compiler/certificate obligations.

This reaches **783 theorems in 67 modules**, with all 772 prior theorem names,
locations and axiom sets preserved and all 66 prior semantic module sources
byte-identical. The [dated record](verification/tight-metal-20260908/README.md)
contains the clean pinned build and complete axiom audit. Mathematical default
values for invalid references do not establish runtime admission. Rust/WGSL
lowering, atom representation, atomic operations, barriers, ordered witnesses,
readback, resource limits and device execution remain unverified.
