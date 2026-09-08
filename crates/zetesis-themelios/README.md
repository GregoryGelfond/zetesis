# zetesis-themelios

The native source boundary uses the pinned themelios parser and owned program model. It preserves original source text, identities, and parsed locations. It never invokes clingo, rewrites source text to hide unsupported constructs, or returns a recovered partial program.

The library keeps three explicit admission contracts:

| API | Result | Supported profile |
|---|---|---|
| `admit` | Relational `Program` | Strict S0 normal rules, constraints, singleton unconditioned choices, scalar/closed structural terms, ordinary positive/default-negated literals, scalar equality/disequality |
| `admit_extended` / `admit_bundle_extended` | Relational `Program` and source evidence | S0 plus acyclic closed constants, checked ground scalar arithmetic, finite fact-head pools/intervals, `#defined`, signature/empty `#show`, and original include graphs for the bundle API |
| `admit_formula` / `admit_bundle_formula` | Ferraris `Theory`, dense original `Atom` identities, lifted objectives, and source evidence | Extended closed-value forms plus evaluated signed disjunctive heads with empty/explicitly true Boolean conditions, finite rule/head pools preserving whole-rule products and local choice groups, finite conditional choices, numeric choice bounds, positive function `#count` heads with positive or default-negated eligibility and checked tuple/atom correspondence, top-level numeric/dependent intervals in positive disjunctive and choice heads, body count/sum/sum+ and complete-value min/max comparisons and independent finite equality assignments, finite comparison-generated bindings, bounded universal body conditionals, and the bounded `#minimize`/`#maximize`/positive weak-constraint profile below |

The CLI chooses an admitted source route automatically. The library APIs remain explicit so callers can require a relational or formula result. `ExpansionFailure::needs_formula_admission()` permits a formula retry only for an unsupported profile or a scalar operation that stopped at a variable. Syntax errors, arithmetic undefinedness/overflow, exceeded budgets, and independent core failures are not profile retries.

## Preparation before grounding

`prepare_formula` and `prepare_bundle_formula` return owned preparation receipts
before possible-support completion and formula materialization. Callers can read
the original source catalog, metadata, bounded logical analysis input and pinned
themelios analysis, then consume the receipt with `ground()` or
`ground_with_observer(...)`. These resume the original budgets. Existing
`admit_formula` functions compose the same two operations.

Preparation is a checked compiler boundary; later grounding can still refuse
undefined arithmetic or exhausted resources. `analysis_basis()` identifies a
`NormalizedProgram` or a `DependencyProjection`. The latter retains every
conditional alternative's predicate, strong sign, arity and default-negation
mode in pool-free syntax, but changes its logical connective for analysis.
Safety and class verdicts describe that projection, not original source
semantics. Neither basis establishes that a lazy implementation exists. The
[grounding-selection contract](../../docs/design/grounding-selection.md) separates
class facts, implementation availability and strategy preference.

## Structural values and finite construction

Bounded closed functions and tuples are logical values, including `f(1,g(2))`,
`-f(1)`, `()`, `(1,)` and `(1,2)`. `f()` normalizes to `f`; a negative constructor
is separate from both numeric minus and the outer predicate sign. Entire values
can be copied through ordinary variables, joins, guards, aggregates, objective
keys and observations. Equality includes every constructor/sign/arity/child.
Closed tuple comparisons use the ASP value order; variable-containing flat tuple
equality retains the existing binding plan and complete guard.

The shared core uses validated flat preorder nodes with immutable shared storage
and cached canonical spelling. The source bridge applies the default per-value
node/depth/payload ceilings in addition to original syntax and cumulative
expansion/formula budgets. Whole-value copies remain finite: neither subterms nor
possible constructor applications are inserted into the source domain. Forward
constant dependencies inside constructors are resolved before use, and nested
constant cycles are refused. Checked i32 arithmetic keeps its existing undefined
and overflow refusals; signed function negation is handled separately.

Formula admission evaluates finite constructors from independently bound inputs,
for example `p(f(X)):-q(X)`. It also extracts components from positive function
and tuple patterns, such as `p(A):-q(f(A),_)` and `p(A):-q((A,_),_)`, retaining
full original atom identities. Named constructors match exact name, sign and
arity; nested mixtures use the same flat matcher.
Matching stages bindings privately and commits them only after the entire pattern
succeeds; mismatches and resource failures cannot leak partial bindings.
Evaluated positions consume independently established inputs: `q(X):-d(X),p(X+1)`
and `q(X):-p(f(X,X+1))` retain the actual supporting `p` atom when the captured
argument equals the evaluated value. Structural positions may bind inputs in
the same atom, independent of traversal and literal order. These equality checks
never infer bindings. `q(X):-p(f(X+1))` without another binder receives
`UnboundArgumentInput`: arithmetic inversion is unsupported by this profile,
which is distinct from declaring the source unsafe in ASP.
Positive structured consequent-only witnesses use this matcher in their own
local alternative scope; evaluated witness positions retain a separate refusal.
Pools/intervals below a constructor remain outside this generation profile.
The [finite-value](../../docs/verification/finite-values-20260907/README.md) and
[tuple-binding](../../docs/verification/structural-bindings-20260907/README.md)
records give exact scope, original-model and frozen-reduct tests. The
[function-pattern record](../../docs/verification/function-patterns-20260907/README.md)
extends the tuple matcher and states the mathematical shape/extraction boundary.
The [positive-argument record](../../docs/verification/positive-arguments-20260907/README.md)
describes the consuming checks, explicit input boundary and scoped proof laws.
Min/max comparisons and independent
assignments accept closed symbols, strings, structures and genuine #inf/#sup
sentinels under the ASP term order; numeric extrema endpoints retain the
[current source guard](../../docs/design/numeric-semantics.md). This internal
refusal is distinct from a themelios rejection and does not establish a KR error.
Original sources, signs, coherence, Ferraris roots and frozen-reduct acceptance
are unchanged by copying a value. The semantic kernels and GPU transports still
operate on complete original atom IDs. Source admission and these Rust consumer
bridges are tested, not claimed to be refined from the existing Lean modules.

## Strong negation and coherence

`p(a)` and `-p(a)` have distinct signed predicate identities; their names and
argument values are unchanged. An ordinary `-p(X)` body occurrence can bind X,
including an independent anonymous slot in `-p(_)`.
`not -p(X)` and `not not -p(X)` retain the original default-negation modes.
Absence of `p(a)` does not imply `-p(a)`, and coherence does not require either
polarity to be present.

The complete S0/extended source boundary adds one positive integrity constraint
`:- p(X1,...,Xn), -p(X1,...,Xn).` for each pair of opposite signatures present in
the original template collection, including across files. It introduces no
missing signature and enumerates no domain product. Original templates retain
their order; generated constraints follow them, retain evidence from both
signs, and consume the existing template, variable, body, work and origin
budgets. The lower-level core `Program` treats signed predicates as identities;
source coherence belongs to this adapter boundary.

The formula boundary instead adds `not (p(t) and -p(t))` roots for opposite
identities in the completed ground atom catalog. It reuses their atom IDs and
charges nodes, roots, work, copied values and origins. These constraints supply
no support. Coherence applies before stable-model acceptance and optimization,
including bounded choices, aggregates and disjunction in their admitted profiles.

Signed `#defined` and signature `#show` distinguish the two polarities. Signed
atoms and directly negated output constructors render with `-`, including
`#show -seen(X) : -p(X).`. Output constructors remain observations; they never
create logical atoms. Arithmetic minus in `p(-1)` remains a numeric value.
Closed signed values such as `p(-a)` and `p(f(1,g(2)))` use the bounded structural
carrier. Formula admission additionally supports the finite construction and
positive structural extraction described above. A function's sign is distinct
from both predicate strong negation and the literal's default-negation mode.

The [Lean `StrongNegation` module](../../proofs/Zetesis/StrongNegation.lean)
proves that injective signed-atom renaming preserves exact reduct syntax and
stable models. Adding a complete registry of coherence constraints filters
incoherent models without supplying support. A sound enclosing atom carrier
and coverage of its opposite pairs are explicit premises; an omitted pair has
a checked counterexample. Whole bounded choice groups are transported intact.
These laws do not verify source admission, concrete Rust identity/registry
construction, objective or observation matching, budgets or GPU execution.

## Relational support before formula construction

Formula admission first validates the complete source profile and variable scopes. It then computes a finite possible-positive relation: ground facts seed the relation; normal heads and positive disjuncts follow positive-body joins; choice heads follow both outer and local positive-condition joins. Static comparison filters remain active. Default-negation gates are ignored only in this support computation, so recursive and negative conditions are not mistaken for extensional facts. Constraints and choice bounds never produce atoms.

The computation must complete a full round with no new atoms. Formula construction then uses complete relational joins over that upper bound, retaining the original negative and recursive conditions in the actual formulas. It does not enumerate all substitutions over a global scalar Cartesian product. Bounded choices combine duplicate grounded heads by disjoining eligibility, count each head once, and express bounds as constraints. Double-negated necessary producer guards prune unsupported classical candidates without restricting reduct subsets of an accepted candidate.

Numeric intervals in choice-head arguments use independent local value slots within the original group: `1 {p(1..2)} 1` has the two singleton models. An empty interval retains its surrounding bounds. Scalar arithmetic and top-level interval endpoints may depend on independently bound outer or element-local variables. Pools, nested intervals and closed nonnumeric endpoints remain refused. The [evaluated-head record](../../docs/verification/evaluated-heads-20260906/README.md) documents the current expansion and binding contract; the earlier [interval record](../../docs/verification/choice-intervals-20260906/README.md) preserves its original model/reduct comparisons, resource boundaries and external timeout.

Each completed support relation retains bounded indexes from argument values to row identifiers. A join selects the smallest indexed list available from a constant or already-bound variable, then checks the complete atom pattern, including other constants and repeated variables. A missing index entry is an empty candidate list; an unbound pattern retains the complete scan. Probe choices survive only while that join depth has the same outer binding. Indexes are populated between synchronous rounds, remain immutable during each join, and charge both an independent retained-entry ceiling and existing work/scalar-key allowances.

Scalar comparison filters may run early once all their referenced slots are actually bound. Generated slots remain unbound during this partial matching, so placeholder values cannot influence pruning. Scalar evaluation errors on an unextendable prefix are deferred to complete-row evaluation; resource failures still stop immediately. A completed nongenerative binding reuses ordinary comparisons only when its final partial check found every comparison bound and successfully true. This transient certificate belongs to the copied binding even though mutable cursor state is undone before that binding is returned; it resets on the next cursor call and each partial pass. Any inconclusive comparison keeps the final checks active. Generated rows always retain final evaluation, and tuple/range filters are never covered by the certificate. Reuse avoids repeated scalar evaluation and its work charges without changing the emitted formulas or accepting an incomplete join. During support discovery only, a normal rule can abandon a remaining body subtree once its fully bound head already belongs to the fixed relation or current round's delta. Final formula construction never uses this shortcut: every alternative body witness must remain represented in its reduct condition.

This is bounded eager formula grounding. The relational admission APIs still produce templates for the separate lazy reduct solver. Source acceptance does not prove the parser, joins, or Rust implementation correct against the Lean model.

## Signed disjunctions and explicitly true conditions

Formula admission accepts `a(X) | b(X) :- d(X).` as one implication from the original body to the disjunction of its grounded heads. Every head variable belongs to the outer rule scope and must be safe there before aggregate-local scopes are considered. Head arguments admit closed values, whole variables and checked scalar arithmetic, plus top-level numeric intervals whose inputs have admitted outer bindings. Each interval expands a Cartesian family of whole rule instances retaining the original disjunction: `p(1..2) | q.` becomes `p(1) | q.` and `p(2) | q.`, not a flat three-atom disjunction. Each disjunct retains its own `not`/`not not` polarity. An empty colon or a conjunction of explicitly true Boolean literals (`#true`, `not #false`, `not not #true`) is erased without changing that rule family: `p(1..2):#true; q:#true.` has exactly the models `{p(1),p(2)}` and `{q}`. The same law covers a singleton conditioned head, retaining its polarity. False conditions, comparisons and atom-dependent head conditions remain explicit refusals. Finite constructor arguments with independently bound inputs use the shared construction plan; pools and intervals below constructors remain outside that plan. The [true-head record](../../docs/verification/true-heads-20260907/README.md) states the bounded slice and its model/reduct evidence. Ordinary singleton `not`/`not not` heads use the same signed implication representation and retain their original frozen semantics; neither supplies positive support. The [singleton-head record](../../docs/verification/singleton-heads-20260907/README.md) gives complete model/reduct and resource tests. Negative choice heads remain refused. `FormulaLimits::max_disjunction_elements` bounds each owned head before IR collection; its default is 1,024, duplicates are already coalesced by the upstream owned representation, and zero allows no disjunctive element. Parsing retains the separate source/syntax ceilings.

The possible-support relation includes only positive disjuncts of each completed body binding. It does not use the single-head support shortcut, because an existing sibling cannot justify skipping another possible head. Final lowering retains one `body → OR(literals)` formula, coalesces identical literal nodes without merging polarities, and records the body as necessary producer evidence only for positive heads. Negative occurrences contribute source provenance. The original rule `a | not a` has both the empty and singleton answer sets; adding `not not a` as a third disjunct removes the singleton, so polarity cannot be replaced by classical equivalence. The separate [negative-head laws](../../proofs/Zetesis/NegativeHeads.lean) justify positive-only necessary support in the stated ground grammar, without proving this compiler. It never shifts a disjunction into normal rules or treats its heads as a choice: `a | b. a :- b. b :- a.` has the stable model `{a,b}`. The current normal-rule component factorization remains disabled for this new head form. The source-to-formula/compiler correspondence is tested, not a new Lean refinement claim.

An objective that can depend directly or indirectly on a disjunctive producer receives `ObjectiveDisjunctionDependency`. This conservative boundary concerns exact objective presence, not reduct stability: clingo removes the objective in `a. a | b. #minimize {1@1:b}.`, while a constraint forbidding `b` may preserve a zero cost slot. A possible-positive upper bound cannot distinguish these grounding simplifications. Disjunctions unrelated to the objective dependency graph remain admissible alongside objectives; costs, priority slots and tuple identity keep their existing contract.

## Checked finite function count heads

Formula admission accepts `1#count{X:p(X):X=1..4}2.` with its four singleton and six pair models. Every function-head element retains the complete tuple key, positive derived atom and eligibility condition. Local conditions may contain finite scalar/range binders, evaluated Boolean/comparison filters and positive ordinary atoms, including admitted structured patterns. Positive atoms join the completed possible-support relation and retain their eligibility formulas; membership in that relation never asserts truth. Tuple/head arguments never supply their own bindings. Default-negated and double-negated conditions consume already established inputs and retain their frozen polarity; they never bind variables or establish support.

Each completely instantiated group must have a one-to-one correspondence between distinct tuple keys and distinct atoms. Identical duplicate pairs coalesce; either one key denoting several atoms or one atom denoting several keys is a located refusal. A complete-group check precedes support derivation and final lowering. Only after it succeeds does the group use the established choice permissions and count constraints. Empty groups retain their bounds, all supported numeric guard relations remain available, and count bounds never supply positive support. Full structural keys remain distinct even when their first terms agree.

Default-negated derived literals, other function-head aggregates and objective-relevant count producers retain explicit refusals. The [count-head record](../../docs/verification/count-heads-20260907/README.md) gives the initial correspondence, resource and frozen-reduct contracts; the [eligibility extension](../../docs/verification/aggregate-bounds-20260907/README.md) records the admitted positive conditions. The [negative-eligibility extension](../../docs/verification/negative-count-eligibility-20260908/README.md) records default/double negation, complete clingo comparisons and the removal of the now-unused `HeadAggregateCondition` error variant from this unreleased Rust API.


## Scalar and interval bindings

Positive equality can bind a whole otherwise-unbound variable from an independently bound scalar expression, in either orientation: `Y=X+1` and `X+1=Y` have the same binding contract. A dependency scheduler starts with the positively bound relational slots and repeatedly schedules ready instructions. Source order does not supply safety. Arithmetic inversion, unanchored equality cycles, and self-dependent intervals remain typed unresolved-binding refusals. The comparison plan below additionally combines separate closed integer bounds and admits closed finite integer chains, retaining every complete guard. Already-bound equalities remain filters, so two proposed definitions must agree.

`X=L..U` and its reversed form stream a separate inclusive integer cursor after both endpoints are bound. A bound `X` uses membership testing; it never binds an endpoint. Descending ranges and evaluated nonnumeric endpoints emit no rows, matching clingo's interval expansion behavior; this applies equally to symbols and real infinity values. Undefined arithmetic while computing an endpoint remains a separate typed refusal. Every cursor has a bounded range width, and every generated value, operation, substitution, support atom, and support round remains under its existing independent ceiling. Backtracking resets dependent cursors before advancing an earlier range or relational row. Values are not drawn from a global source-domain Cartesian product.

Normal-head scalar expressions and intervals lower to fresh internal value slots feeding the original atom. Distinct head intervals produce independent argument products without introducing semantic auxiliary predicates. For example, `end(S+D) :- start(S), duration(D).` can derive `end(4)` from two values of 2 even when 4 never occurs in the source. This generated value participates in later constraints and recursive joins. Evaluated choice and disjunctive heads use the same binding machinery with their distinct group/rule expansion contracts. Already-safe default-negated body arguments can also evaluate scalar expressions and finite constructors. They consume existing bindings rather than supplying new ones. Evaluated positive body arguments compare captured support values with expressions after their inputs are ready; no private capture can establish those inputs. Local choice and aggregate conditions have their own binding plans; generated local variables cannot make an outer head safe.

Undefined or overflowing arithmetic refuses the whole source instead of dropping a substitution. Generative recursion must complete the possible-support fixed point; a default-negated producer can therefore reach a bounded refusal even when clingo's stronger grounding simplification terminates it. Existing aggregate-target dependency restrictions are separate from these scalar instructions. The planner and streamed cursor are tested implementations, not a claimed Lean refinement.

Anonymous arguments under default negation of an **unsigned** predicate have an existential projection contract. For `not q(X,_)`, every named argument must already be safe, while anonymous positions allocate no binding slots. Final formula construction disjoins every matching atom from the completed possible-support relation, then applies the original default or double default negation. It preserves recursive eligibility rather than freezing that disjunction to a truth value. No semantic auxiliary predicate is introduced. Ordinary positive-body anonymous atoms of either classical sign retain their independent binding behavior; already-safe negative arguments can evaluate arithmetic without supplying bindings.

Clingo 5.8.2 treats `not -p(_)` and `not not -p(_)` differently: their anonymous variables are unsafe. The formula and observation source boundaries preserve that refusal, including choice and aggregate conditions. Ordinary positive-body `-p(_)` remains a valid binder. This distinction is a source-safety rule, not a change to the formula reduct or a reason to collapse the two predicate signs. The [strong-negation record](../../docs/verification/strong-negation-20260906/README.md) retains the exact source probes and diagnostics.

## Ground Boolean and comparison guards

Formula rule bodies, choice-element conditions and aggregate-element conditions accept `#true`, `#false` and their default/double negations. Scalar comparisons and flat tuple equality/disequality may be default-negated or chained once every variable has an admitted binding. For example, `d(1..3). p(X):-d(X),not 0<X<3.` derives only `p(3)`. A chain is the conjunction of its adjacent comparisons; `not` complements that entire conjunction and `not not` preserves its truth.

The complete guard remains a truth test. A separate dependency plan may now extract scalar equality bindings from positive or double-negated comparisons/chains. One side must be a whole unbound variable and every variable in the other expression must already be safe. `p(X):-not not X=1.`, `p(X):-not not X=1<2.` and `p(X):-X=1<2.` therefore derive `p(1)`. The plan retains every original comparison and its whole-chain sign; a contradictory neighboring comparison still rejects the row. Equality chains can schedule several dependent scalar bindings, without arithmetic inversion or safety from unanchored cycles.

Positive or double-negated comparisons may also supply a closed finite integer domain for one remaining variable, using a chain or separate lower/upper comparisons. Both endpoints must normalize to integer constants: `p(X):-0<X<3.` and `p(X):-0<X,X<3.` derive `p(1)` and `p(2)`. All applicable closed bounds are intersected, strict endpoint adjustment uses wider arithmetic, and contradictory bounds complete with no rows. The existing range cursor enforces its inclusive width, value, work and substitution ceilings. Every candidate still passes the unchanged complete guard. A limit failure cannot certify an empty domain.

Closed endpoint syntax matters: clingo rejects `d(0,3).p(X):-d(L,U),L<X<U.` as unsafe even though the relation contains numerical endpoints. This implementation retains that refusal. Singly default-negated generators, inversion and unresolved cycles also remain refused. Positive or double-negated flat tuple equality can bind scalar components once its whole opposite tuple is already bound; the complete equality still filters mismatches. One double-negated interval equality likewise proposes a bounded range while retaining its membership guard. Neither tuple component bootstrapping nor local scope leakage supplies safety. Chains with several unresolved integer variables remain a clingo-valid native gap. Local choice, aggregate and universal-condition binding plans reuse the extension; conditional consequents remain nonbinding. Objective and observation conditions keep their separate compilers.

The current comparison-generator fixture retains 137 exact sources: 119 admissions with 138 complete full-model records, five valid-clingo native refusals and 13 unsafe sources. The [finite-binding record](../../docs/verification/finite-bindings-20260906/README.md) records three promotions without changing their original reference models; the earlier campaign retains its historical counts. Portable tests compare native enumeration and an independent exhaustive Ferraris evaluator, compare every original/frozen interpretation for finite-instance specifications, and check exact range/substitution ceilings and original include provenance. External replays use each unchanged source. Binding coverage, source safety and the Rust planner remain separate formal refinement obligations.

Guards evaluate only on complete rows under the existing expression/substitution/work ceilings. Their truth contains no semantic atom and therefore supplies the same constant in the original formula and every frozen reduct. Every adjacent comparison and tuple operand is evaluated; false comparisons, length mismatches and default negation do not hide undefined or overflowing arithmetic. A guarded rule retains its original source and rule origins. Aggregate assignment target-dependency checks include every guard operand, and optional component factorization conservatively declines rules containing the new guard form.

Boolean heads, objective conditions and `#show` term conditions retain their separate admission profiles. The Lean [ground-guard contract](../../proofs/Zetesis/GroundGuards.lean) proves atom-free replacement under explicit totality and fixed-carrier premises, including preservation in arbitrary formula contexts. It does not prove the Rust evaluator, source binding analysis, support completion or arithmetic admission policy.

## Universal body conditionals

Formula admission accepts finite body conditionals such as
`d(1..2). {p(X)}:-d(X). all:-p(X):d(X).`. Each complete condition assignment
retains the original implication from its condition to the disjunction of its
consequent alternatives. The completed condition rows are conjoined. Thus
`p(X):X=1..2` requires both atoms, while `p(X):#true` uses positive `p` rows as
local witnesses and requires at least one. Separate conditionals have separate
local scopes; consequent witnesses neither bind outer names nor generate support.
Condition safety must be established before any consequent witness is admitted.

Root argument pools and pooled argument lists select finite alternatives inside
one condition row. Nested arithmetic intervals use private range slots and the
same flat expression evaluator as ordinary bindings. `p(X-1;2*(X..X+1)-3):X=2`
therefore means `p(1) or p(3)`. Each alternative retains its default negation
before the disjunction: `not p(1..2):#true` means `not p(1) or not p(2)`.
An empty alternative range is false even under default negation; a successfully
exhausted empty condition domain is true. Partial enumeration never certifies
vacuity. Undefined arithmetic and resource exhaustion remain located refusals.

Positive consequent-only whole variables, anonymous arguments and structural
patterns obtain finite witnesses from completed possible-support rows. For example,
`q:-p(f(X,_)):#true.` requires some complete `p(f(...))` atom, retaining its
anonymous values and predicate/constructor signs. Repeated or prebound names must
agree; a failed row cannot leave bindings for the next row. Witness names are
private to each alternative and cannot establish condition or outer-rule safety.
Evaluated negative arguments consume independently bound inputs. Evaluated
positions mixed with local witness extraction, arithmetic inversion, negative
anonymous witnesses and nested pools remain explicit profile boundaries.
Original condition formulas are retained; possible support does not
substitute for their truth. Pooled analysis syntax uses the explicit
`DependencyProjection` basis described above, without changing runtime reduct
or certificate premises.

Aggregate-assignment dependency checks still inspect both the condition and
consequent, including generated value instructions. A conditional producer that
reaches a lifted objective remains an `ObjectiveConditionalDependency` refusal.

The scoped [ConsequentAlternatives](../../proofs/Zetesis/ConsequentAlternatives.lean)
and historical [UniversalConditionals](../../proofs/Zetesis/UniversalConditionals.lean)
laws establish supplied finite formula collections' original and frozen
semantics. They do not prove source join completeness, compiler correspondence
or machine resource accounting. The [current evidence](../../docs/verification/conditional-scope-20260907/README.md)
and [earlier conditional record](../../docs/verification/stocktake-20260906-conditionals-objectives.md)
retain the executable checks and their scope.
The [structured-witness record](../../docs/verification/structured-witnesses-20260907/README.md)
adds complete original-source comparisons, finite frozen checks, inclusive limits
and the scoped [StructuredWitnesses](../../proofs/Zetesis/StructuredWitnesses.lean)
laws. Pattern copies are preflighted; matching reuses the existing transactional
work and storage accounting, including its conservative charges for shared values.

## Finite aggregate comparisons

Formula admission accepts body `#count`, `#sum`, `#sum+` and numeric `#min`/`#max` with complete closed-value tuple keys, positive ordinary binding conditions, scalar comparison and Boolean guards, and frozen default-negated conditions. Both numeric guards, all six comparisons, and default/double default negation of the aggregate are retained. Positive body set literals are cardinality elements. Flat tuple equality/disequality binding filters remain supported; closed tuples also serve as whole predicate argument and comparison values.

Complete tuple identity controls deduplication: repeated eligible tuples combine their condition formulas by disjunction. Equal weights alone never merge different tuples. Recursive or classically tautological eligibility is preserved for the reduct. Nonnegative weights use bounded threshold formulas; signed sums use an exact bounded subset-implication translation. `!=` retains its aggregate reduct and is not replaced with default negation of equality. Choice bounds are constraints over eligible selected heads, so they cannot create support.

Both sum functions ignore empty tuples and tuples with a nonnumeric first component; `#sum+` additionally ignores negative weights. Zero weights are neutral. Filtering is consistent in assignment candidates and final formula construction, and never weakens source variable safety. A count's empty tuple is valid and contributes once. Current typed refusals include cyclic or self-dependent aggregate assignment generators, remaining conditional consumers, tuple intervals and nonpositive body-set base literals. Completed outer values may feed the consumers described below; this does not extend aggregate-element binding or objective-observer contracts.

The possible-support computation conservatively ignores aggregate comparisons, retaining all potential normal/choice heads. This is sufficient for the stable-model upper bound, but does not reproduce objective priority presence for impossible aggregate producers. Ordinary aggregate producer comparisons therefore receive `ObjectiveAggregateDependency` when they can feed an objective. A narrow structural observer contract for total assignments is described below. Aggregate constraints never derive support and remain admissible with objectives.

`FormulaLimits.aggregate` independently bounds tuple count, lowering work, threshold states, signed subsets, and total DAG nodes, within the enclosing formula work/storage ceilings. Errors retain the original rule location; successful roots retain their complete parsed provenance.

## Generated aggregate values and upstream analysis

Several positive function aggregates may bind distinct otherwise unbound named variables through single equality guards (`N = #sum{...}` or the reversed guard). Each target must be absent from its own tuples and conditions. Another assignment may consume it after its producer has completed; the required/produced variable graph must be acyclic and every input must be bound. A target already bound by an ordinary positive atom remains an equality test. Non-binding aggregate element tuples and conditions may use generated targets after the complete row is constructed; this preserves the previously admitted comparison profile. Complete proposals can also supply integer choice/count-head bounds, with their original equality and rule activation retained. Every other global/local variable still needs a valid positive binding. Self-dependencies, cycles, duplicate binding guards and objective-relevant multiple-assignment producers remain explicit refusals.

Across ordinary, choice and checked count-head outer bodies, acyclic aggregate assignments may feed scalar comparisons, tuple comparisons, complete Boolean guards, scalar equalities and constructed or evaluated head values. For example, `{p(1);p(2)}.q(f(Y)):-N=#count{X:p(X)},N>0,Y=N+1.` has four complete answer sets. A private plan records required/produced slots and preserves the prior generator order wherever inputs are ready. Relational bindings precede these instructions; synthetic captures and comparisons never invent inputs. The source body order, aggregate equalities and existing original/frozen guard lowering stay unchanged. Missing and cyclic value inputs have explicit scheduling diagnostics. This does not reject ordinary predicate recursion merely because it is recursive.

Evaluated positive checks can consume independent aggregate proposals while retaining their complete source atom: `p(1).q(N):-N=#count{},p(N+1).` admits `q(0)`. Completed aggregate/scalar values also select default-negated and double-negated ordinary atoms and admitted unsigned anonymous projections in outer bodies. For example, `{p(0)}.q(N):-N=#count{},not p(N).` has answer sets `{p(0)}` and `{q(0)}`. These gates never bind variables or discard possible rows based on support membership; their original formulas remain in the frozen reduct.

Finite outer ranges may consume completed aggregate/scalar endpoints; both generators and membership tests retain their existing contracts. Remaining conditional consumers and objective producers retain their explicit profile boundaries. Aggregate assignment dependencies are scheduled through intermediate scalar and range values; later aggregate tuples and conditions receive the complete predecessor row. Each emitted rule retains all original aggregate equalities, so proposed values do not establish aggregate truth. Previously admitted scopes remain available. Plans charge scans and input/readiness/instruction storage before allocation; ordinary aggregate-free rules retain their previous expansion accounting. Undefined or overflowing consumer arithmetic refuses the whole admission under the existing final-row policy, even when an aggregate proposal would later be unrealizable. No early filter optimization hides that error.

[Completed choice-body consumers](../../docs/verification/choice-consumers-20260908/README.md) use this same plan. For example, `{d}.Y{a;b}Y:-N=#count{1:d},Y=N+1.` retains each proposed count equality and activates one bounded group, yielding `{a}`, `{b}` and `{d,a,b}`. Head-local scopes and objective observers retain their separate admission contracts. Head-local negative eligibility can read completed outer aggregate values: `{b(1);d}.Y#count{1:a:not b(N)}Y:-N=#count{1:d},Y=N+1.` Head-local eligibility and outer-body activation retain their separate scopes even when both consume completed values.

Dependent ranges use the same checked plan and per-row cursor: `{d}.q(K):-N=#count{1:d},K=1..N.` has the empty answer set and `{d,q(1)}`. Endpoints are evaluated only after their complete dependencies; later cursors reset before earlier values advance. A range whose target is already bound is a membership filter, not a new producer. Empty or nonnumeric intervals emit no rows, while endpoint evaluation errors and width/work/value limits retain their typed failures. Generated head intervals follow the same outer row contract. Neither successful range expansion nor a negative gate establishes the original aggregate equality. See the [outer negative gate](../../docs/verification/outer-negative-consumers-20260908/README.md) and [dependent range](../../docs/verification/outer-ranges-20260908/README.md) records for exact source, frozen-reduct and resource checks.

For example, `{p}. pair(N,S) :- N=#count{1:p}, S=#sum{2:p}.` has two answer sets, containing `pair(0,0)` or `p` and `pair(1,2)`. The cursor streams the product of each bounded attainable-value set. Shared atoms can correlate the actual values: every original equality remains in the theory, so reduct checking rejects unrealizable combinations. No global source-domain product or new lowering rule is introduced.

After each complete outer relational join, the candidate cursor computes attainable subset sums over full deduplicated tuple keys and runs ready value consumers; a count produces `0..n`. Final filters inspect complete generated rows. Empty count/sum aggregates produce zero. Numeric min/max assignments propose their possible numeric tuple values plus the actual empty sentinel (`#sup` for min, `#inf` for max), distinct from every integer. This can introduce numbers absent from the source domain. Conditions sharing atoms can make some proposed values unrealizable: the actual aggregate equality remains in every emitted formula, and the reduct oracle determines membership. Checked integer overflow, generated-value, work, substitution, atom, and round ceilings refuse the input; recursive value growth never authorizes a partial support relation.

Final grounding reuses eligibility only over its completed, immutable possible-support relation. A cache key contains the deterministic aggregate IR identity and complete outer binding with the proven-absent assignment target removed. Its values are full-tuple-coalesced condition formulas, never a candidate model's truth values. Independent cache row, element, root, and key-payload ceilings bound retained entries; copies/lookups/reference returns consume the normal work and scalar-payload budgets. Each assignment binding constructs equality roots for all bounded candidate values. Nonnegative count/sum families share one threshold table; signed families retain exact subset implications with cumulative quotas. Numeric extrema use filtered eligibility disjunctions and preserve implication in not-equal formulas. Numeric min/max bounds and first tuple values at i32 endpoints retain an internal admission guard while six clingo 5.8.2 discrepancies are unresolved. The [numeric boundary record](../../docs/design/numeric-semantics.md) keeps this implementation/semantic gap separate from themelios rejection and agreed language exclusions. Every returned equality root is remapped through structural DAG interning and retained under its exact scalar candidate value. Original roots still carry every source occurrence.

`source_analysis()` exposes the pinned `themelios-analysis::Analysis`; `analyzed_program()` exposes its bounded pool-free analysis input qualified by `AnalysisBasis`. Expanded facts are pool-free owned statements retaining original enclosing provenance. An independent upstream visitor rejects residual term or argument-list pools before `Analysis::of`, whose internal unpooling would otherwise expand them. Structural node/edge and logical payload allowances are charged before upstream graph construction; allocator/RSS overhead is not claimed as an exact byte measure. Dependency and head-condition edges come from themelios's established signature/dependency APIs.

The upstream safety result is preserved verbatim. At pin `87c11a3`, its ASP-Core-2 reading treats aggregate guard variables as required, so `n(N) :- N=#sum{}.` is reported unsafe there. The frontend admits that narrow clingo binder extension only after its own explicit target/scope checks. It does not relabel the upstream result as safe or use its finiteness result to remove runtime ceilings. `Unknown` remains unknown, and a `Holds` finiteness value is never evidence by itself when the upstream safety premise does not hold. No sibling repository or dependency pin is changed.

The Lean aggregate-assignment contract proves candidate-value coverage and full-tuple OR-coalescing under explicit carrier coverage assumptions. It does not prove this Rust cursor, parser/scope logic, recursive support completion, caching, or objective-priority behavior.

The scoped [aggregate consumer laws](../../proofs/Zetesis/AggregateConsumers.lean) describe input readiness, retained value association and original/frozen clauses. They do not establish Rust refinement or runtime budgets. [Consumer verification](../../docs/verification/aggregate-consumers-20260907/README.md) records finite-ground comparisons, exact limits and a schedule-bypass negative control.

## Lifted objectives

Formula results expose `objectives()`, `objective_origins()`, and `objective_declarations()`. Objectives remain lifted templates, separate from logical rules. The `zetesis-objective` evaluator joins them against a complete model after the reduct oracle verifies stability; objective conditions never derive atoms or supply support.

The current objective profile accepts:

- `#minimize` or `#maximize` with a scalar integer constant or positively bound variable as its weight;
- a ground integer priority, defaulting to zero;
- closed constant/whole-variable tuple components;
- positive ordinary-atom conditions and scalar equality/disequality filters;
- checked ground scalar normalization and global acyclic constants under the existing expansion limits.

Positive weak constraints such as `:~ p(X). [2@1,X]` normalize into the same
minimization path. Their bodies must already satisfy the positive ordinary
atom and scalar equality/disequality objective profile. Global tuple identity
crosses weak constraints and both optimization directions, including roots and include files.
Original weak declaration and body spans remain in objective evidence and the
normalized analyzed source; synthesized condition carriers have constructed
provenance. Raw weak occurrences count against the same objective-element
ceiling before upstream statement deduplication, and body width is checked before
raising. The strict S0 and extended scalar APIs retain their existing contracts.

Maximize weights normalize by checked negation before constructing a global
`(priority, normalized weight, full tuple)` key. Priority and tuple components
remain unchanged; matching keys across maximize/minimize/weak statements combine
eligibility. Scores and CLI output keep the normalized minimization sign, so
maximizing an eligible weight 2 prints cost -2. The original Ferraris theory and
reduct remain unchanged. Both exact scoring and optional incumbent-bound planning
call the same typed `WeightPolarity` helper.

A maximizing template checks every eligible positive/filter binding in the
completed support relation for `i32::MIN`, under the existing work/substitution
ceilings. An eligible unrepresentable negation receives a located
`NumericOverflow` refusal; absent or filtered endpoint rows do not. This explicit
boundary avoids clingo 5.8.2's wrapped endpoint weight, which can produce either a
solver error or a non-mathematical maximization cost. It does not alter existing
minimize arithmetic. The raw literal `-2147483648` already receives an upstream
raising diagnostic; checked arithmetic such as `(-2147483647-1)` exercises the
eligibility-dependent boundary without broadening that parser contract.
Zero weights and inactive models retain possible priority
slots; the scan never uses constraints to remove them.

All objective variables must be positively bound. Variable arithmetic in weights, dynamic priorities, default-negated objective conditions, aggregate/conditional weak bodies, and other aggregate forms receive typed refusals. Only possible numeric weight bindings establish priority slots; evaluation ignores nonnumeric weights while preserving those slots.

For objective-enabled inputs, the frontend follows the pinned themelios dependency graph from the predicates read by objective conditions. Normal producer bodies and choice conditions within that dependency closure must contain no default negation. Relevant producer bodies refuse general aggregate conditions, with the total-assignment observer exception below. Unrelated producers retain their ordinary formula semantics. Negative and aggregate constraints remain supported. This restriction makes objective priority presence compatible with the possible-positive relation: broader grounder simplification of negative producers can remove priority slots, which a gate-ignoring upper bound alone cannot reproduce.

An objective-enabled aggregate producer may be a pure total count/sum or numeric min/max assignment whose generated target occurs in its normal head. Assignment-generated head argument positions are identified structurally and unioned across every producer of a predicate. An objective-relevant total assignment may consume another generated predicate through a single unfiltered positive aggregate-element condition, with distinct variable bindings at its generated argument positions. Ordinary producer joins over generated outputs and choice producers remain refused; the established upstream dependency graph enforces that boundary. In objective conditions, every generated position must be a fresh variable occurring only once across positive atoms and absent from filters; it may supply the weight or tuple. Non-generated positions may still join ordinary relations. Constants and shared/filtered generated outputs remain typed refusals. This preserves a total assignment's existential priority presence without treating all proposed values as realizable, and recognizes no predicate or domain names.

An objective template with no possible numeric-weight positive/filter binding is omitted. Original declaration locations remain available even when every objective is omitted. Enabled zero weights, cancellation, and priorities inactive in a particular stable model retain their priority slots. The evaluator globally coalesces contributions by `(priority, weight, tuple)`, including repeated bindings and separate source statements. It sums checked costs in descending priority order. Optimization orders verified stable models; it does not replace the reduct membership check. An incumbent is not a proved optimum until search coverage completes.

The optional `objective_bound::ObjectivePlan` compiles the lifted objectives over
the original formula atom catalog without changing its index order. Complete
positive joins produce body conjunctions; all alternatives for an equal
`(priority, weight, tuple)` are OR-coalesced globally. Nonnumeric weights produce
no contribution. `plan.bound(score, limits, control)` builds a separate
same-carrier candidate constraint for lexicographic cost less than or equal to
the supplied score, including every tie and missing zero priority slot. The
caller must establish catalog coverage, verify the incumbent, and apply this
constraint only to the outer candidate search. Its `original()` accessor retains
the semantic theory identity; original and reduct queries are unchanged.

Plan work, bindings, key count/payload, tuple width, local variables, body width,
and nodes have explicit limits. Each bound has cumulative work and bounded
aggregate lowering. Nonnegative levels share threshold DAGs; signed levels use
the same representation when a bounded classical normalization is suitable.
After full-key OR coalescing, each negative contribution `w * I(E)` becomes
`w + (-w) * I(not E)`; the sum of negative weights shifts both scalar guards.
Entries remain distinct after this transformation. A charged preflight checks
the required threshold range, state/node/work capacity, and a conservative
DP-cell versus subset-enumeration estimate. Large-magnitude small families,
`i32::MIN`, and unrepresentable shifted guards retain the exact signed fallback.
Both paths may decline the optional optimization without altering the original
formula or its frozen reduct. The integer identities in
[`SignedObjectiveBounds.lean`](../../proofs/Zetesis/SignedObjectiveBounds.lean)
apply to the candidate constraint; they do not prove this native compiler,
machine arithmetic, cost heuristic, or resource accounting. Typed
errors retain work and template-index evidence, return no partial constraint,
and do not invalidate an already compiled plan. These tests establish runtime
agreement on checked cases, not a source-to-Rust refinement proof.

## Sources, metadata, and limits

`SourceBundle::load_many` accepts ordered original file roots; `load` is its single-root convenience API. The bundle retains each root occurrence and spelling, every unique source, and each include occurrence with its source span and chosen path. Semantic admission combines owned statements through themelios, preserving source identities without concatenating text. Constants are global across roots and includes, and must remain unambiguous and acyclic.

The loader captures the working directory once. An original string `#include` first resolves there; a relative path with a filesystem lookup failure falls back to the directory from which its including source was opened. The captured directory and `IncludeResolution` record make this choice inspectable even if the process directory later changes. This corrects the earlier parent-directory-only lookup: when both locations contain the named file, clingo selects the working-directory file. Absolute includes have no fallback. Once a readable regular file is selected, parse and resource failures never trigger fallback. Nonregular files are explicitly refused before opening; pathname preflight and opened-descriptor checks do not claim immunity to concurrent filesystem replacement.

Unique canonical sources share one catalog. Exact repeated selected paths are included once, whether reached through roots or includes. Different lexical spellings for the same canonical source and include symlink redirections remain conservative, located refusals; the loader never silently merges aliases whose parsing multiplicity could differ in clingo. Root occurrences have their own `max_roots` ceiling, so repeated roots cannot bypass admission bounds. File and byte ceilings apply to the combined unique catalog, include-depth ceilings apply to each root's complete graph, and semantic/expansion budgets are shared across the resulting program.

`#defined` is a declaration. `#show` supplies presentation metadata in these APIs; future task-relative demand analysis is separate. Signature directives union their selections; empty `#show.` activates explicit selection without adding signatures. **Term shows alone leave default atom output enabled.** Full stable-model identities and counts survive equal or empty displays.

The formula APIs additionally compile bounded term/conditional observations such as:

```asp
p(1;2).
#show p/1.
#show placement(X,(X,)) : p(X).
#show p(X) : p(X).
```

`admitted.metadata().observations()` returns an immutable `observation::ObservationProgram`. Its `evaluate(&Model, Limits, &Control)` returns distinct shared themelios `Symbol` values for the **term channel only**. Its `render(&Model, &OutputSelection, Limits, &Control)` buffers one complete display line, combining selected original atoms with the term channel. Equal symbols within the term channel coalesce across all directives/bindings; equal symbols from the atom and term channels remain repeated. Thus `a. #show a.` displays `a a`. Equal displays from different full models remain separate records, including all requested optimal ties.

This evaluator consults only the supplied Model. It neither checks stability nor adds terms to the atom carrier, logical analysis, original Theory, reduct queries or objectives. The caller supplies a separately verified full model. The observer is independent of the chosen oracle; source admission is formula-only. Existing strict/extended APIs preserve term-show refusal, so the CLI's auto route selects the CPU countermodel oracle. Explicit GPU selection uses hybrid formula checking; explicit closure/lazy requests remain refused for this slice.

Accepted templates contain ground shared Symbols, named variables, signed function constructors and tuples, including nested constructors and scalar/string/extremal output. Each directive has its own scope. Every named variable must occur in an ordinary positive body atom; repeated variables enforce equality, while anonymous body arguments are independent wildcards. Conditions support ordinary positive/default-negated/double-negated scalar atoms of either classical sign, admitted anonymous projections and one-step nongenerative scalar comparisons in ASP term order. Arithmetic, unary sign operations beyond folded numeric literals and signed constructors, pools, intervals, binders, aggregates, conditional and Boolean body literals, and anonymous output variables have explicit located refusals. The unsafe `not -p(_)` and `not not -p(_)` forms remain refused; ordinary positive-body `-p(_)` is a valid binder. Clingo drops undefined arithmetic display instances; this initial slice refuses such syntax instead of claiming the existing checked source-arithmetic policy is equivalent.

`FormulaLimits.observation` holds independent `observation::AdmissionLimits` for templates, nodes, depth (capped at 64), text, arity, local variables, body elements and original locations. Runtime `observation::Limits` separately bounds charged work, complete bindings, distinct terms, constructed term-symbol nodes/depth/bytes, retained payload and rendered bytes. Constructed-symbol limits apply to the term channel; selected atom rendering is bounded by work/output bytes and validates public model predicate/symbol spelling. Errors retain typed causes, directive locations when applicable and partial work statistics, without returning a partial term set or line. Original source occurrences remain in metadata even when equivalent templates or output values coalesce. The CLI buffers each complete Answer and objective vector before emitting it; a display failure returns an explicit error and cannot announce exhausted success.

Source bytes, syntax traversal, scalar expansion, variable scopes, possible-support rounds, join work/bindings, formula storage, objective shape, and copied source evidence have independent ceilings. A limit produces a typed located refusal, not UNSAT or a smaller admitted theory. Bundle failures retain the complete catalog so diagnostics can name their original file. Objective evaluation and stable-model search have additional independent runtime budgets.

Parser refusals preserve the original text and every typed themelios diagnostic.
For string admission, `AdmissionFailure::Syntax` carries `SyntaxFailure`, whose
`source()` and `diagnostics()` accessors support caller-owned views. This replaces
the unreleased variant's bare diagnostic vector. Its plain human display resolves
line/column locations and source excerpts under the neutral `<input>` name; bundle
syntax errors retain the original path and source identity. Terminal colour belongs
to the CLI. The [diagnostic record](../../docs/verification/syntax-diagnostics-20260908/README.md)
describes ownership, rendering cost and the syntax-only scope of this refinement.

## Validation

`tests/observations.rs` contains nine portable tests and an optional bounded fresh-clingo replay. Its 63 unchanged reference sources currently include 43 admitted programs with 59 complete displayed model/cost records and 20 explicit typed refusals. The [strong-negation record](../../docs/verification/strong-negation-20260906/README.md) documents those promotions; historical campaign results retain their original counts. Comparisons preserve multiplicities both inside each display and between original models. Additional regressions check every observation ceiling, cancellation/deadlines, public malformed-model names, exact original formula identity, duplicate provenance across bundles, and adjacent logical-source refusal boundaries. The CLI's `tests/observations.rs` adds five portable tests for auto/explicit routing, term/atom duplicates, hidden optimal ties and output failures with no partial Answer or false completion.

`tests/root_bundles.rs` checks seven portable cases for ordered roots, shared includes, global forward constants, original source diagnostics, independent root/file/byte ceilings, aliases and captured-directory lookup. The CLI's `tests/multiple_inputs.rs` records 14 original file sets with 19 complete optimal model/cost records, including equal displays, cross-form objective tuple identity and cwd shadowing. Its optional bounded clingo test refreshes every record using separate original files.

Portable tests load the unchanged vendored include graphs. The five task-allocation variant-01 cases now admit and exhaust native reduct-verified search with complete optimal contracts:

| Case | Full stable models | Optimal cost | Optimal models | Ground atom identities |
|---|---:|---:|---:|---:|
| Basic | 4 | 5 | 1 | 20 |
| Agent reuse | 8 | 6 | 1 | 29 |
| Selective compatibility | 4 | 10 | 1 | 26 |
| No compatible agent | 0 | absent | 0 | 17 |
| Larger mix | 81 | 9 | 1 | 55 |

The source fixtures, exact displayed `assigned_to` and `assigned_cost` atoms, costs, complete model counts, and original objective locations are checked in `tests/objectives.rs`. The larger case uses 55 relevant atom identities; the old unrestricted signature Cartesian products would contain 8,736.

`tests/aggregates.rs` checks tuple identity, recursive bounds, both guards, scopes, typed ceilings, and complete enumeration of the same 92 boards from all six unchanged queens encodings. `tests/aggregate_clingo.rs` records 321 original/reference sources: 313 full native stable-model/optimum comparisons and 8 explicit typed refusals. It independently enumerates Ferraris reduct models; a separate optional test refreshes the clingo references. Two previously refused Boolean conditions are promoted using their unchanged source and expected models; historical reports remain unchanged. `tests/aggregate_assignments_multiple.rs` adds 54 exact-source cases with 97 complete model records, checked against an independent finite reduct evaluator, native search and an optional fresh clingo replay. Limits, source order, generated values, correlated eligibility and retained comparison dependencies have separate regressions.

`tests/ground_guards.rs` checks 124 exact sources with 120 complete full-model records, native enumeration and an independent exhaustive reduct evaluator. It also compares every original/frozen interpretation pair for guard replacement, checks whole-chain sign scope, independent binding and aggregate target refusals, undefined/overflowing operands, resource limits and original bundle provenance. A separate optional test replays every source through external clingo. Objective and observation guards are outside this campaign.

`tests/formula_assignments.rs` checks new-domain values, signed/correlated tuples, recursive assignments, scoped and resource refusals, generic mixed producers, cache isolation, and complete unique optimization of the unchanged layered-DAG source at default limits. Four further unchanged shortest-path include graphs exhaust their full optimal cost/model-count contracts at the existing CLI formula-work ceiling of 1,048,576; the regression exercises shared aggregate families without increasing limits. `tests/formula_analysis.rs` plus the helper's unit tests check the upstream boundary, payload/edge ceilings, pool rejection, original include provenance, and retained safety/finiteness facts. `tests/aggregate_objective_observers.rs` records 92 independently checked observer sources: 72 admissions with 118 complete model/cost records and 20 explicit dependency refusals.

`tests/formula_clingo.rs` preserves an independent recorded campaign of 403 valid and 26 refused source programs. Valid cases compare complete model sets using exhaustive Ferraris minimality. `tests/formula_support.rs` checks support pruning, recursive and negative conditions, variable undo, constraints, statement order, and incomplete-round refusals. Optional clingo tests remain explicit test oracles; they are never runtime dependencies.

`tests/scalar_bindings.rs` checks dependency scheduling, independent interval cursors and bound membership, generated-head products, local scopes, generated-value constraints, and located range/recursive/arithmetic refusals. The independent `tests/scalar_bindings_clingo.rs` campaign records complete scalar-binding model sets and explicitly classifies unsupported or unsafe sources; it never treats an arbitrary admission error as a pass.

`tests/anonymous_projection.rs` checks empty and populated projections, repeated named arguments, independent anonymous positions, exact string identities, and recursive/local aggregate conditions under both forms of default negation.

`tests/indexed_joins.rs` checks complete selective joins within a fixed work ceiling, repeated-variable and constant validation after probing, and the independent inclusive index-entry allowance. The optimization changes candidate row selection, not the supported source language or the final reduct formulas. `tests/comparison_reuse.rs` adds 20 independently recorded sources with 35 complete model records, four deferred-arithmetic refusals, and an inclusive work-ceiling check. Its cases cover sibling-row undo, empty relations, alternative negative witnesses, generated values, fixed component bindings, and residual tuple/range filters; an optional clingo run refreshes the same source records.

Run the portable package tests with `cargo test -p zetesis-themelios`. The repository's `scripts/check.sh oracle` runs the recorded source campaigns again against an independently installed clingo executable.

`tests/weak_objectives.rs` checks 49 recorded sources with 186 complete
model/cost records through independent Ferraris subset enumeration. A bounded
optional test refreshes every record against clingo. Separate tests retain
declaration/body/include provenance, global cross-form tuple identity, raw
occurrence/body ceilings, unsafe-variable refusals and the positive-profile
boundary.

`tests/extrema_source.rs` compares 198 independently recorded numeric min/max sources (654 complete model/cost records) with exhaustive native reduct checking. It covers all six comparisons, default/double default negation, recursion, full-tuple duplicate supports, empty sentinels and sum-to-extremum objective chains. Fresh bounded external execution checks the same records. Numeric-endpoint refusals and independent assignment/cache ceilings remain explicit regressions.

`tests/disjunction.rs` checks 28 admitted hand-written sources with 54 complete stable models against independent formula trees and recorded clingo output, plus every frozen subset on the admitted carrier against manually specified producer guards. A separate full-carrier model comparison detects missing possible atoms. The fixture retains 21 historical refusal entries. Two, arithmetic and interval heads, are now covered by the [evaluated-head suite](../../docs/verification/evaluated-heads-20260906/README.md); the other 19 still receive checked located unsafe/profile refusals, including objective-reachable disjunctions, conditioned heads and unsupported term forms. Inclusive head-element limits and unrelated objective presence/priority/tuple identity are separate checks. The fixture records 62 bounded clingo commands over 49 original tiny sources; unsafe-source reference failures are explicit records, never admitted-equivalence passes. Its ignored replay test refreshes the full records without making clingo a runtime dependency.

`tests/maximize_bounds.rs` checks signed objective guards against complete scores
for every tiny interpretation, global cross-direction tuple keys, optional bounds
on/off over verified stable models, complete ties, unchanged original theory
nodes/roots, inclusive work refusal and typed checked-negation failure. These
runtime checks use the same shared normalization API exercised independently by
`zetesis-objective/tests/polarity.rs`; they do not claim a Rust compiler refinement.

`tests/strong_negation.rs` independently evaluates every finite reduct and proper
subset for 78 admitted sources, matching 124 complete model/display records,
with 16 typed adjacent-profile or unsafe refusals. It also checks a hand-written
coherent signed-choice theory in all frozen worlds, the lazy/extended closure
routes, inclusive coherence ceilings, and cross-file original provenance.
Signed objectives and observation channels keep complete tuple and full-model
identity. The [verification record](../../docs/verification/strong-negation-20260906/README.md)
separates admitted parity, unsafe reference failures and remaining profile gaps.
