# Numeric argument domains: integrality and intervals for native arithmetic

The formula route derives numeric facts locally, at run time, for one consumer at a time. This note designs one static
analysis that gives every predicate argument an integrality and an interval across rules, before support completion
and through recursion, and the consumers that use it to make the grounding of arithmetic cheaper: fewer rows visited,
fewer instances examined and, in Stage 3, fewer possible atoms. Every consumer is an execution choice. The
[grounding chapter](../book/architecture/grounding.md) and the [language reference](../book/reference/language.md)
state the contracts it preserves.

The aim is to remove most of the grounding of arithmetic and to evaluate arithmetic directly where possible, so that a
modeler can keep a natural integer representation without a constraint-ASP extension. Nothing here propagates over
integer variables during search. Stages 1–3 make the grounding of arithmetic cheaper and, except in Stage 3, leave the
ground program unchanged. Admission refinements (Stage 4) and linear decomposition (Stage 5) are the aim's second
half. Acceptance reports how much arithmetic grounding each stage removes, so the distance to the aim stays visible.

Paths are under `crates/zetesis-themelios/src/` unless another root is named. Encodings are under
`examples/correctness/encodings/`.

## Status

| Part | Status |
|---|---|
| Stage 1: shared analysis and fact input | Implemented: one neutral engine, source and prepared-IR lowerings, borrowed finite values, integrality, intervals, requested integer certificates, diagnostic census and (1d) support-round selection; static decisions, aggregate envelopes and measured acceptance remain open |
| (1a) typed integer comparison forms | Deferred, with its return conditions |
| (1c) interval guards | Folded into (2a) |
| Stage 2: (2a) range probes and (2b) expression keys | Flat-body (2a) and (2b) implemented in support rounds and eager emission; broader traversal coverage and acceptance remain open; does not need Stage 1 |
| Stage 3: fixing atoms false | Designed; open; admission route decided |
| Stage 4: admission refinements | Deferred until Stage 2 is measured |
| Stage 5: linear constraints without enumerating combinations | Research |

Stage 1 serves the fact-input target, existing finite-domain consumer and a typed diagnostic census. Ordinary bounds
describe defined values; separately requested integer certificates establish totality of their expression over the
complete-binding domains. Neither establishes complete-binding existence or prefix coverage. The census does not
activate (1b), (1d) or Stage 3. The independent (1d) execution request retains scalar-totality certificates for support
rounds; the remaining design below labels the other consumers and their obligations separately.

Stage 2 implements checked expression-equality and single-column ordering probes for flat normal bodies during
support rounds and eager emission. They share one snapshot-owned index and retain residual comparisons, source row
order and the first-new partition. Arithmetic evidence, direct-source exact activity, retained emission and hybrid
capture/checks keep their previous traversal. The index uses existing checked scalar operations and source-row
identities, with no static analysis premise. The reach and gain claims below are estimates from sources or profiles;
Stage 2 acceptance measurements remain open.

## Problem

### Starting point

Before Stage 1, six mechanisms on the formula route each derived a local numeric fact for one consumer. This list
records the starting point; the shared engine described below now adds argument ranges across rules and recursion.

1. **Finite symbol domains** (`crates/zetesis-domain`). Each argument is a finite set of at most 256 values, or
   `Unknown`. Every arithmetic head term widens to `Unknown` (`Widening::HeadTerm`), and so does every choice,
   disjunctive or negated head (`Widening::HeadForm`). The formula route uses the analysis only when the caller
   requests a domain attempt, and only for programs whose every rule is positive and flat with an atom or falsum head
   (`formula_domains/positive.rs`); every other program is `Inapplicable` (`formula_domains.rs`). Candidates are
   narrowed by comparisons over one variable (`formula_support/queries/domains.rs`).
2. **Checked interval transfers** (`zetesis-domain::Range`, formerly `scalar_arithmetic/range.rs`). `Range` carries i64 endpoints confined to i32 and
   transfers every operator: sums and differences exactly; products and quotients by corners; division and remainder,
   declining when the divisor's range contains zero or `MIN / -1` is possible; fixed nonnegative powers; bitwise
   operations to the full i32 range. Its only dependency is themelios's `BinaryOp` and `UnaryOp`.
3. **Totality hulls** (`formula_support/projections/totality/numeric.rs`). Per rule and per completed support
   snapshot, a variable's hull is the numeric range of its full positive column (`column_range`). An expression is
   total when every intermediate is defined on the hulls. The certificate enables computed keys and prefix pruning on
   completed support only.
4. **Binding-guard envelopes** (`formula_binding_guard/envelope.rs`). Rule-local affine inequalities bound a variable
   generated by comparisons, from closed endpoints only.
5. **Range multiples** (`formula_support/range_selection.rs`, `formula_support/multiples.rs`). One divisibility shape,
   `N \ D = 0` over a closed range.
6. **Integer comparison forms** (`formula_ir/integer.rs`, `formula_support/integer_comparisons.rs`). Compiled once per
   comparison literal. On each row the form reads each variable, checks that it is a number and evaluates with checked
   operations; otherwise it declines to the general path.

The lazy route keeps a seventh mechanism, finite argument bounds over the admitted templates
(`crates/zetesis-cpu/src/oracle/argument_bounds.rs`). It holds sets only, serves the lazy route's dense-relation
choice, and stays separate.

### What grounding arithmetic costs

Measured at one worker against clingo 5.8.2, with integer comparison forms, when the design was written. Later changes
on this branch move these ratios, and each stage measures its gain afresh.

| Case | zetesis / clingo | Where the time goes | Targeted by |
|---|---|---|---|
| n-queens variants 1, 3 | 1.41×, 1.54× | The diagonal constraints join every pair of possible queens (4,096 pairs per constraint at n = 8) and filter by `R1 - C1 = R2 - C2`. Filtering was 31.1% of samples before the comparison forms. | (2b) |
| n-queens variants 5, 6 | 1.38×, 1.48× | No comparison literal: the arithmetic is in heads over `row(R), column(C)` (2n² instances) and the constraints are counting aggregates. | none |
| traveling-salesman variant 05, case 05 | 1.46× | Support completion is 37%. `visit_time(Dst,S,T1+ET)` is an arithmetic head under recursion, capped by `T1+ET <= M` and constrained by time windows. | Stage 3 |
| shortest-path variant 04, case 06 | 1.24× | Support completion is 31%. The arithmetic is two `#sum` assignments and one comparison of their results; nothing attributes the time to arithmetic. | none |
| prime-sieve 30→300 | 1.49× (direct source evaluation, the automatic grounder's choice); 1.995× (eager grounding) | Support completion is 37% on direct source evaluation, and both sub-routes run it. `composite(N) :- candidate(N), candidate(D), D < N, N \ D = 0.` (`examples/scalability/prime-sieve.lp`) joins with complete coverage because of `\`. | (1d) |
| send-money | 1.12× | Digit constraints over choice-bound letters. The natural single-equation encoding grounds 10^8 instances. | Stage 5 |

An approximate census of the maintained programs: 16 compare variables, 13 have arithmetic comparisons, 5 have
arithmetic head terms, 1 has a variable-bounded range (`time(0..M) :- horizon(M)`), 11 have assignment aggregates and
2 divide or take a remainder in a body (sudoku's block constraints and prime-sieve's composite rule). Integer
comparison forms decide comparisons in 46 of the 105 programs.

## Design

### Where it lives and when it runs

- **One engine, two lowerings.** `zetesis_domain::numeric` owns `Input`, the reduced lattice, synchronous engine,
  `Analysis<K, C>` and `RuleBounds`. A `Values` reader classifies borrowed keys and compares them for semantic
  equality; its ordering is not interpreted as ASP comparison order. The source lowering uses borrowed `&Symbol`
  keys and original statement contexts. The prepared-IR lowering uses authenticated catalog term keys and original
  source contexts, matches `LiteralIr` and `HeadIr` exhaustively, and retains every producer even when its transfer
  is imprecise. `Range` is exported from `zetesis-domain`; its scalar users remain in `zetesis-themelios`.
- **The source-level entry point stays.** `zetesis_domain::analyze(&Program, Limits)` retains its public result and
  borrowed provenance contract. `compile.rs` lowers its existing source profile to the same engine; unsupported
  source forms retain their previous local widening or whole-analysis fallback. Preparation calls the IR entry point.
- **The finite guards keep their positive-flat scope**, and `PositiveSource` remains the producer plan's certificate.
- **Public surface.** `DomainObservation::Inapplicable` is removed: its producer, the eligibility walk ending the
  phase, no longer ends it. The `Analyzed` observation carries the IR analysis, and an observer resolves its
  catalog-keyed values through a resolver the observation lends for the callback's scope. The `terminal` module's
  types (`Context`, `UnknownReason`, `Limits`, `Resource`, `Statistics`, `Stop`) are unchanged. The change is a
  breaking 0.x change, recorded in the CHANGELOG.
- **Route scope: the formula route only**, in the existing domain phase before support completion
  (`formula_ground.rs`). Both sub-routes run that phase: eager grounding, including hybrid preparation, and direct
  source evaluation (`formula_source.rs`), which completes support with the same domain options, then classifies
  families and evaluates exact activity, with no final emission. The lazy route is outside.
- **The analysis runs only when the caller requests a domain attempt** (`with_domain_analysis`,
  `formula/preparation.rs`). The command requests it (`crates/zetesis-cli/src/admission.rs`), so 103 of the 105
  maintained programs get it: every one except `network-repair.lp` and `reachability.lp`, which take the lazy route.
  The method's documentation is revised: it enables the analysis and its consumers, not only the positive-flat guard.

### The abstract domain

Each signed predicate argument carries three components:

- **values:** authentic keys within `max_values_per_argument` (default 256), or `Unknown`;
- **integrality:** `Integer`, every value the argument can take is a number, or `Mixed`;
- **interval:** ⊥ (no numeric value), `[low, high]` within i32, or `Unbounded`. It speaks only of numeric values.

`#inf` and `#sup` are not numbers. Term order ranks `#inf` below every number and every symbol, string and function
above every number (`crates/zetesis-core/src/term_order.rs`), so an argument that can hold one of them is `Mixed`, and
its interval says nothing about those values. The join is set union (widening to `Unknown` at the width), interval
hull (⊥ is its unit) and `Mixed` if either side is. A finite set whose values are all numbers determines the other two
components: `Integer`, with the interval met with the set's hull. Congruences are left out; range multiples serve the
one divisibility shape the corpus uses. `Analysis::entries` exposes each finite key with its analysis-local
`ConstantId`, ordered by registration coordinate for finite candidate intersections. These coordinates are neither
ASP term order nor comparable across analyses; the key reader owns semantic equality.

**Two intervals per slot.** Every reader states which one it reads.

- **The domain.** For a relational slot, the meet of its positive occurrences' argument values, unrefined; for a
  `Bind` or a generating `Range`, the transfer over its operands' domains. It covers every **complete binding** of the
  rule, a substitution the join completes, including those a comparison will exclude.
- **The produced range.** The domain met with the rule's guard refinements. It covers only the substitutions that
  produce a head.

Only the head transfer reads produced ranges. Every definedness premise reads domains: joins evaluate expressions on
rows that a comparison will exclude, and the contract makes that legal ("a failure met before the relation that
decides an exclusion is still no refusal"). Complete coverage skips prefix filtering for bodies with division or
remainder, and family-evidence cursors keep every false row (`formula_ground/arithmetic.rs`).

**The domain does not cover a row read at a join prefix.** The join binds a slot at the first occurrence that names
it and decides each comparison at the depth that binds its last variable (`Decisions`, `formula_support/order.rs`).
Between those depths the slot's value lies in the first occurrence's argument value, not in the meet. This is the
scope the [correspondence](../book/lean/correspondence.md) already states for today's certificate, which establishes
definedness on full bindings only, and the scope of `DomainBindings`' complete positive binding. A consumer that
evaluates at a prefix needs another premise: the hull over the occurrences that can bind the slot first, or today's
retention of prefix failures until their depth is undone (`filter_prefix`, `formula_support.rs`).

### Sources

- **Facts** seed arguments once before iteration. The IR lowering visits each fact block's columns, reading scalar
  runs and compact integer segments without constructing a tuple product or reconstructing a source fact view.
  Small segments submit their actual scalar-occurrence keys; a wide segment contributes its checked inclusive hull
  directly, with an Unknown finite component. Scalar occurrences are classified before semantic deduplication.
  A per-column dictionary remains later work; there is no claim of one read per distinct repeated scalar.
- **Constants.** A number is a point. A symbol makes the argument `Mixed`, with interval ⊥ for that value.
- **Ranges.** A generating `X = L..U` is `Integer`, from the low end of L's numeric values to the high end of U's,
  or ⊥ when either endpoint has no numeric value or the resulting interval descends. Its finite component stays
  `Unknown`: the engine does not manufacture catalog keys or arithmetic image sets. A range over an already bound
  target is a membership test and generates nothing.
- **Assignments.** The first slice transfers `#count`, `#sum` and `#sum+` to `Integer` and `Unbounded`; `#min` and
  `#max` to `Mixed` and `Unbounded`, including their empty-selection infinities. Tuple-count and weight envelopes,
  negative-weight sums, required-element evidence and overflow bounds remain prerequisites for tighter transfers.
  Consequently the first slice does not establish a useful traveling-salesman `high(M)` from its `#sum` horizon.
- **Comparisons** refine produced ranges only.

### Transfers

One case for every construct of the prepared IR that binds a slot or puts a value in a head:

| Construct | Transfer | Widens |
|---|---|---|
| `Atom(None, pattern)`, positive | Each variable position meets its slot's domain with the argument's value. A constant position filters rows only. | — |
| `Atom` or `ProjectedAtom` under `not` or `not not` | Dropped: it binds nothing, and dropping a condition only enlarges the possible values. | — |
| `ProjectedAtom(None, projection)` | As `Atom` for the arguments it binds; witness slots are local. | — |
| `PatternAtom` | A slot bound inside a function term is ⊤ (`Unknown`, `Mixed`, `Unbounded`). | that slot |
| `Bind`, arithmetic | Interval: the `Range` transfer over the operands' domains, for operators whose laws are proved (Lean). A declined transfer or an unproved operator gives `Integer` and `Unbounded`: a defined result is an i32, and an undefined one produces no head or refuses the program. Finite component: `Unknown`; arithmetic image sets have no consumer in this slice. | the target slot |
| `Bind`, copy `Y = X` / constant / constructor `Y = f(X)` | X's whole value / a point, or `Mixed` with the symbol in the set / ⊤. | the target slot, for a constructor |
| `Range` as a generator (`binder: true`, or made one by `formula_binding_plan.rs`) | `Integer`, with interval and set as under Sources. | — |
| `Range` over a bound target (membership test) | Binds nothing, adds nothing to its target's domain and excludes nothing. With the target and both endpoints `Integer`, it meets the target's produced range with `[low(L), high(U)]`. | — |
| `Aggregate`, an assignment | As under Sources. | the assigned slot |
| `Aggregate` with guards only; `Conditional` | Bind nothing outside their scope. | — |
| `Compare`; `ArgumentCheck` (as `=`) | Refines produced ranges when both operands are `Integer`. | — |
| `TupleCompare`, `Guard`, `HeadGuard` | Refine nothing. | — |
| `HeadIr::Normal(None)` | Produces nothing. | — |
| `HeadIr::Normal(Some(pattern))`; each atom of `Disjunction` | Per argument: a slot gives its produced range and set; a constant a point or a symbol; an arithmetic term its transfer over produced ranges, met with any guard bound on a structurally equal expression, with an Unknown finite component as for `Bind`; a constructor term ⊤ (`HeadTerm`). | that argument |
| `ConditionalDisjunction`, `Choice` | Each element's atom over the rule body plus its condition, read as a body of its own scope. | that argument |

**Granularity.** Every widening widens one slot or one argument. A logical-limit stop is the one whole-analysis
outcome: every query becomes `Unknown` and nothing partial is published. No construct of the prepared IR makes the
whole analysis `Unknown`. The source-level analysis's whole-analysis reasons never reach the IR, because the formula
route resolves constants and includes at input and admits no named or parameterized parts, `#external`, theory or
scripts. The lowering matches the IR exhaustively and widens what it cannot transfer precisely, which is sound. The IR
analysis's status therefore has no `Unknown(UnknownReason)`.

**Guard refinement**, only when both operands are `Integer`. For `e1 rel e2` with rel among `<`, `<=`, `>`, `>=` and
`=`, each operand's value is met with the bound the other imposes; for example `<` meets `e1` with
`[MIN, high(I2) - 1]` and `e2` with `[low(I1) + 1, MAX]`. A head argument whose expression is structurally equal to an
operand takes that meet. An operand `X`, `X + c`, `X - c` or `c - X` also meets X's produced range. `!=` refines
nothing, and a membership test refines as `L <= X, X <= U`. Opaque `Unknown` expressions are precision summaries,
not denotational identities. They are never interned; expressions containing them receive neither correlated
subtraction facts nor stored expression-specific guard refinements, even if a caller reuses one coordinate.

**One pass, in IR order.** In each round a rule's produced ranges start at its slots' domains, and the refinements run
once over its comparisons and membership tests in IR order, each meeting its operands' current values. Each meet
removes only values that cannot produce a head: a substitution whose comparison is defined and false is excluded, and
one whose comparison is undefined omits its instance or refuses the program. The result is not a local fixed point.
For `p(X,Y) :- a(X), b(Y), X < Y, Y <= X.` over [0, N], one pass gives both X and Y within [1, N−1], where a local
fixed point would reach the empty result after about N/2 passes; the envelope derivation bounds its endpoints for the
same reason. Every value the pass keeps is covered, so the result is sound and only less precise. Refinement lookup and identity walks also contribute charged work; source order makes the pass deterministic. Thus
`visit_time(Dst,S,T1+ET) :- ..., horizon(M), T1+ET <= M.` gives its third argument at most `high(M)` in every round
whenever M is `Integer`. A symbolic M or `#inf` is `Mixed`, and the comparison refines nothing.

### Fixed point, widening and narrowing

- **Rounds.** Every producer reads the same previous argument vector. The next image starts from the once-only
  fact seeds and unions every producer contribution. Ascending rounds join that image with the accumulated state;
  no argument narrows before convergence. Complete-binding domains and the produced guard pass stay distinct.
- **Thresholds.** T contains each submitted numeric constant and compact segment endpoint, its representable
  neighbors c − 1 and c + 1, and the i32 limits. Missing constants from an imprecise lowering only reduce precision.
- **Widening.** After two outward changes, a growing endpoint jumps to the next outward threshold. Finite sets
  widen locally beyond `max_values_per_argument`; integrality can change from `Integer` to `Mixed` only once during
  ascent. The static finite threshold set and width bound force stabilization, or a logical limit stops the attempt.
- **Narrowing.** A stable accumulated value W establishes F(W) ⊑ W. One complete synchronous, unwidened image
  F(W) replaces W: lfp = F(lfp) ⊑ F(W) ⊑ W. It is not an in-place pass or another join with W. Final per-rule
  domains and produced ranges are recomputed from that result. This can recover a known guard cap; aggregate-derived
  caps such as traveling-salesman's horizon remain open until their producer envelopes are implemented.
- **Work and space.** Bounds, finite references, expression visits, refinement and identity scans, finite-set
  comparisons, threshold searches and key inspections are charged before the operations. A finite meet scans its
  smaller set and binary-searches both operands; symbol comparison occurs at key registration, not at each meet.
  Let V be `max_values_per_argument`: one binary finite meet costs O(V log(V + 1)) key-coordinate inspections and
  O(V) live temporary references. Ordered refinement scans and repeated expression walks are counted explicitly;
  the cost is not claimed linear in distinct expression DAG nodes. Seed, current, candidate and scratch sets all
  share `max_value_entries`; plans, keys, thresholds and provenance share `max_links`. Logical limits bound the
  attempt; neither describes allocator overhead or peak RSS. A stopped analysis publishes no bounds.

### The integer certificate

The planned static consumers need a stronger premise than a numeric bound. **An integer certificate** states that for every complete binding
within a scope an expression is defined, its value is an integer, and the value lies within `[l, h]`. It says nothing
about a join prefix. The expression may be a slot, a column, or arithmetic over slots or over one occurrence's
columns. Its fields are the expression, the interval (never ⊥), the scope and the producer.

| Producer | Scope | How it establishes the premise |
|---|---|---|
| the static analysis | every complete binding in every snapshot of every round, and in final emission, of one preparation | every slot the expression reads has an `Integer` domain, every operator has a proved interval law, and the `Range` transfer over those domains succeeds |
| completed-column hulls (today's totality premise) | the completed support snapshot | every distinct input of the full positive columns is a number, and the transfer over the hulls succeeds |
| index builds (Stage 2) | one relation snapshot | every distinct value (2a), or every row's key (2b), is read or evaluated with checked operations at build time; any failure means no index |

The neutral `Rule::integer_queries` list requests roots in that rule's expression graph. The existing final pass
publishes `RuleBounds::integer(query)` certificates with private constructors, borrowing that exact `RuleBounds`.
`integer_query_count()` includes declined requests. The producer checks every child before its parent against
`domains()`, never `produced()`: a narrow result of defined-value transfer, a later guard or an `e-e` correlation
cannot hide an undefined intermediate. Fact columns supply seeds, not expression-totality certificates.
Consumers query the resulting premise:

- **`integer(e)`:** a certificate for e exists in the consumer's scope;
- **`within(e)`:** its interval;
- **`separated(e1, rel, e2)`:** both are certified in one scope and the intervals' endpoints decide rel on every pair
  (Lean statement 2). The answer carries the comparison's constant truth and its **exclusion role**: whether the
  join's plan decides the comparison. That holds for a `Compare` or `ArgumentCheck` every variable of which a positive
  occurrence or the prefix binds (`Decisions::decides`, `formula_support/order.rs`). Only such a comparison excludes a
  substitution when it is defined and false (`formula_support/filters.rs`). A binder, a membership test, a tuple
  comparison, an aggregate guard and a comparison over a generated variable exclude nothing. The role is static: it
  depends on which occurrences bind the variables, not on the join order.

Separated comparison verdicts remain diagnostic only. Planned (1b) and Stage 3 consumers must retain the exclusion role;
Stage 2 continues to use its existing checked index and residual filtering contract.

Computed keys become the one-column case of Stage 2's expression keys. Today they invert one-variable expressions under
the totality certificate (`Projections::total_body`, `formula_support/projections/totality.rs`) and a cover built from
every distinct input of the column (`formula_support/projections/domains.rs`). Stage 2 has one index structure and one
fallback rule: an ordinary posting key first, then an exact expression key, then a covering range probe whose superset
the residual comparison filters. An available computed-domain selection keeps precedence over this fallback because
it can combine restrictions on several columns. The computed-key path is retired once Stage 2's acceptance shows
identical routes on the programs it serves today.

### Diagnostic census and occurrence correspondence

`GroundingObserver::domain_diagnostics_enabled()` requests the diagnostic callback, certificate queries and
census preparation independently of detailed timing. Its default is true, preserving existing diagnostic observers;
no observer requests no diagnostics, and timing-only observers explicitly decline. Shared numeric analysis and finite
guards still run when `with_domain_analysis` requests them. The independent (1d) consumer requests scalar operands for
eligible partial normal bodies even without diagnostics. Declined diagnostics add no report-only queries, retain no
occurrence reports and do not prepare a census; no second graph or analysis is introduced. Successful execution
certificates retain only their exact source occurrence, after the full analysis reaches a fixed point.

`NumericAnalysis::census()` returns `Some(NumericCensus)` only after a completed analysis and report preparation;
logical stops return `None`, not successful zero populations. `NumericAnalysis::rules()` lends reports in non-fact
compiled occurrence order. Compact fact rows contribute seeds, not millions of empty-body requests. The census
counts argument positions, integer and bounded-integer positions, retained finite sets/entries, empty domains, mixed
positions and unbounded numeric intervals. Mixed and unbounded counts may overlap.

Every normal `RuleIr` retains its own borrow, the neutral `RuleId` returned when that exact scope was submitted,
and literal/operand query positions referencing the already lowered roots. Source sites can repeat after expansion;
they are evidence, not lookup keys. Choice and conditional/local scopes still contribute every producer, but are
unsupported report consumers in this slice. Their extra neutral rules cannot shift a later normal occurrence's
mapping. No full source `Program`, second analysis engine or duplicate expression graph is retained for reporting.

`NumericRuleAnalysis` exposes the occurrence, original site, body status and scalar comparison results. The initial
body profile permits normal rules/constraints without generated head slots, ordinary atom leaves (including default
negation) and scalar comparisons. Every slot must have ordinary positive coverage. A bound variable or admitted
constant leaf is total as a term without acquiring a numeric type. `NumericOperand` records `Integer(Range)`,
`Leaf` and `Uncertified` separately. Every other operand needs its own integer
certificate. `NumericBody` distinguishes unsupported profiles, empty scopes, uncertified bodies and total bodies.
A total body is not evidence that a complete binding or an original arithmetic-family witness exists. Rules with a
normal atomic head and constraints are counted separately.

Two same-scope certified intervals may yield a true/false separated comparison. Its exclusion role reuses the
join's `Decisions` variable-readiness fold with an empty prefix and ordinary-positive slot coverage. Unsupported
profiles report that role as unclassified. The callback reports these facts; it does not remove a literal, prune a
join, bypass family evaluation or grant computed-index/round authority. Disabled analysis remains
`DomainObservation::Disabled`. Observation is available independently of detailed timing and counter opt-in.

Neutral query roots and result cells count against `max_links`; certificate traversal shares the existing analysis
work meter and final-pass stop policy. IR mapping copies and report scans are charged preparation work;
`DomainPrepareWork` includes the engine receipt and, when requested, report preparation. Declining diagnostics
removes their actual work and link charges; their previous refusal boundaries are not simulated. Shared-analysis
logical stops and typed failures keep their existing contracts. Report metadata is bounded by admitted
rule/expression/work populations, not a new allocator-byte or RSS promise. A failure before report publication
preserves its typed source context and emits no successful census prefix. Retained finite guards keep their existing
eligibility and behavior. The census is evidence of applicability, not a speedup claim or activation of (1b)/(1d).

### Stage 1: the analysis, the certificate and two cheap consumers

Stage 1 delivers the analysis, the static certificate, the preparation receipt and a census: the analysis's own count,
per program, of typed and bounded arguments, statically separated comparisons and statically total rules. The analysis
runs and charges on every formula-route program with a domain attempt, so it is not accepted on its cost alone. It
lands with at least one target it serves and is accepted on that target's measured gain. Its targets:

1. **Stage 3**, whose planned admission certificate uses the static analysis. It targets the profiled cost of
   support completion on the time-window classes; its benefit has not yet been demonstrated.
2. **The fact input of columnar Stage 2**: the lowering reads the fact blocks, and columnar Stage 2 needs no fact view.
3. **(1d)** on prime-sieve's support completion, conditional on runtime correspondence and measured gain.
4. **(1b)**, where the census shows reach.

Each consumer is implemented only where the census shows it reaches the maintained programs; otherwise it is recorded
and deferred.

**(1b) Static decisions.** Query: `separated(e1, rel, e2)` with static certificates, using both the constant truth
and the exclusion role.

- **Always true:** the literal leaves the join's filters. No complete binding's verdict changes. The literal may be
  false at a prefix, so removing it can remove prefix pruning: in
  `q(-1..1). r(1..9). s(1..5,1..9). p(W) :- q(X), r(Y), X > 0, s(X,W).`, `X > 0` prunes X ≤ 0 at `q`'s depth before
  `s(X,W)` is joined. Without it the join visits more rows; both solvers answer `p(1)`..`p(9)` either way. A failure
  such a prefix row meets is retained, as today, so admission does not change.
- **Always false:** support completion and emission skip the fragment's joins, since a defined false body literal
  produces no head and no ground instance. Family classification skips the fragment only when both hold:
  1. the comparison's exclusion role is plan-decided, so it excludes every substitution;
  2. every other expression of the fragment carries a static certificate, so every substitution is jointly defined.

  Then every substitution is a defined, excluded witness of its source family (`proofs/Zetesis/ArithmeticFamilies.lean`),
  and the fragment contributes `filtered` exactly when its positive join is nonempty, which takes one positive binding.
  `d(1). p :- d(X), X/(1;0) > 5.` keeps its `filtered` witness from `X/1 > 5` and stays admitted with one warning,
  from `X/0`. The two conditions keep out the substitutions that are not jointly defined and excluded:
  - `d(1). p :- d(X), Y = X+1, Y > 5, 1/(X-1) = 0.` is refused today. `Y > 5` is statically false but over a
    generated value, so it excludes nothing, the division by zero makes the family entirely undefined, and condition 1
    keeps the program refused.
  - `d(1). z(0). p :- d(X), z(Z), X > (5;0), X/Z > 0.` is refused today. The fragment with `X > 5` is false and
    plan-decided, but `X/Z` has no certificate; an excluded but erroneous substitution is no witness
    (`formula_support/family.rs`). Condition 2 keeps it refused.

**(1d) Static totality during support rounds.** Query: `integer(e)` with static certificates, for every arithmetic
expression of a rule body that has a head. It grants prefix pruning (`Coverage::Selected`) for that rule's joins in
every round, delta joins included, where the snapshot-local certificate alone requires complete coverage for bodies
with division or remainder. It grants no computed keys during rounds; those are Stage 2's. The
documented boundary restricts round-local proofs to the multiples selector because one snapshot's columns do not
cover later rounds' rows. A static certificate's premise covers every complete binding of every round:

- no complete binding has a failure for pruning to hide;
- a row at a prefix may still fail on a value outside the meet, and the join retains that failure until its depth is
  undone, as today;
- pruning a false plan-decided prefix removes only rows whose completions are excluded.

The [grounding chapter](../book/architecture/grounding.md), the correspondence and the comment in
`formula_support/projections/totality.rs` gain the static certificate as the second premise source. Reach in the
maintained programs is one rule, prime-sieve's composite rule, on both formula sub-routes. Its `N \ D` is a remainder
by a variable divisor whose domain excludes zero. `RemainderIntervals` now proves the general interval law and
checked-scalar fault conditions; Rust/source correspondence and measured acceptance remain open. Range
multiples already select its divisors during rounds, so any gain must be measured on both sub-routes before (1d)
is accepted.
Sudoku's block constraints have no head and are joined in emission, where today's certificate already applies.

**(1a) Typed integer comparison forms: deferred.** Typing would remove a branch, not a read: the match on a number
both checks and extracts the value. A form that never declines would be unsound at prefixes. The forms are evaluated
at the decided prefix, where a slot's value lies in its first occurrence's argument value, not in the meet. In
`q(-1..1). r(1..9). s(1..5,1..9). p(W) :- q(X), r(Y), Y/X > 1, s(X,W).`, X's domain is [1,1], but `Y/X > 1` is decided
at X = 0 before `s(X,W)` is matched. Today the form declines on the checked division and the general path retains
the failure. If a profile shows the checked branches matter, (1a) returns with three conditions: a typed row keeps the
decline for read failures; "never declines" rests on a prefix premise (each slot `Integer` within the hull of the
occurrences that can bind it first, at the evaluating depth); and a Lean statement that plain evaluation equals
checked evaluation within proved intervals.

**(1c) Interval guards: folded into (2a).** Finite guards compare dictionary identifiers, whose order is not numeric
order. An interval check on a row would read the row's term, as the integer form already does at the same depth.
Restricting rows before they are read needs an ordered index.

### Stage 2: pushdown into joins

The implemented flat-body routes are described in the [grounding chapter](../book/architecture/grounding.md).
One snapshot-owned index stores sorted `(integer key, source row)` entries, with `O(n*m + n*log(n))` build work
for `n` rows and `m` expression nodes and `O(n+m)` storage. Equality and ordering use the same descriptor and
certificate, including a cached decline for a wholly checked nonnumeric or undefined key. Equality lends its posting
in source order, with `O(log(n))` lookup work and constant query storage. An ordering query uses numeric binary bounds,
checks each selected entry against its occurrence's delta interval, then sorts the selected source row IDs before
visiting them. For `k` numeric candidates and `s` rows inside that interval, its work is
`O(log(n) + k + s*log(s))` and its transient leased storage is `O(s)`. Numeric endpoint comparisons avoid adding or
subtracting one at i32's bounds. No separate column dictionary or per-variant index is retained.

The gate admits only normal heads whose variables all belong to the body and whose body consists of flat atoms,
comparisons and tuple comparisons. Generated bindings and head arguments, argument checks, structural patterns and
nested scopes remain outside it. Only a plan-decided `Compare` supplies a selector. Ordinary bound-column postings
remain first choice; expression equality precedes ordering. The ordering key must be one column, while the already
bound opposite operand may be a checked expression. A nonselective ordering query still pays for its temporary union;
no cost-based decline is implemented. Index preparation authenticates the catalog workspace and relation owner before
any cached result can be used.

Stage 2's premises come from index builds, so it does not wait on Stage 1. Every index re-checks its premise on the
data it indexes; the static certificate only predicts where an index will succeed.

- **(2a) Range probes.** Query: `integer(column)` from the index build. An ordering comparison between a bound integer
  and a column (`Start < Earliest`; `D < N`; `R1 < R2`) probes the column's checked `(integer key, source row)` index
  instead of visiting every row, and stays as a residual filter. The index is built on demand, once per descriptor and
  relation snapshot; a non-number in the column means no index for that snapshot. Today's ordinary postings are keyed by term identifier
  (`formula_support/relations.rs`), not by numeric order.
- **(2b) Expression keys.** Query: `integer(key)` from the index build. For an equality with one side an expression
  over one occurrence's columns (`R2 - C2` in n-queens), the occurrence's rows are indexed by the expression's value,
  evaluated per row with checked operations at build time, and the other side probes the index. A row whose key fails
  to evaluate means no index. At n = 8 a diagonal constraint would read about 5.4 rows per outer queen instead of 64
  (the diagonal through a cell has 344/64 ≈ 5.4 cells on average), roughly twelvefold fewer; the estimate is owed a
  measurement.
- **Only plan-decided comparisons are pushed down.** A comparison over a generated variable, a membership test, a
  tuple comparison or an aggregate guard excludes nothing when false, so it is never pushed down.
- **Where they apply:**

  | Traversal | (2a), (2b) |
  |---|---|
  | support rounds, flat normal rules with no generated head arguments, both formula sub-routes | yes: one index per descriptor and round snapshot, each probe cut to its occurrence's delta interval |
  | family evidence (`formula_ground/arithmetic.rs`) | no: evidence cursors keep every false row as a witness of defined arithmetic |
  | final emission (eager grounding), the same flat normal scope including constraints | yes |
  | exact activity on direct source evaluation (`evaluate_acyclic`, `formula_source_activity/acyclic.rs`) | no; a later extension with its own argument |
  | retained emission, hybrid capture, hybrid region and model checks | no; their support completion may use the support-round route above |

- **Probes keep the first-new partition.** Support rounds join through delta variants: earlier occurrences select old
  rows, later occurrences all current rows, and the pivot new rows. Every existing probe is cut to its occurrence's
  interval with `delta::Rows::within`, and a Stage 2 probe is intersected the same way before any row is read, reusing
  `delta::interval` and building no per-variant index. The law preserved is `DeltaRounds`
  (`coverage_of_first_delta`, `delta_step_exact`). Without the cut a variant would read rows outside its partition:
  bindings produced twice, duplicate derivations charged, and a producer plan that no longer describes what ran.
  Possible atoms alone would not detect this: exact per-variant binding sequences and probe-row receipts protect the
  partition, including duplicate keys and an empty later variant.
- **The typed-failure half.** A probe skips rows whose selected comparison is defined and false (or which cannot match
  the occurrence). The flat-body complete-row policy excludes such substitutions even if another scalar expression
  on them fails; a retained substitution still reaches every residual check and preserves its failure and source
  site. Arithmetic admission is not assumed to establish totality. The separate family-evidence traversal keeps all
  false rows and never enables these probes. Generated bindings/heads and structural or nested evaluation are excluded
  by the gate, so their earlier failures are not hidden. Building an index uses checked scalar operations; nonnumbers
  and undefined key expressions decline the whole index, while resource, interruption and foreign-owner failures
  propagate. The query clips and sorts source IDs before the ordinary matcher, which retains alias and constant
  checks. A skipped visit's matcher work is not performed; builds, comparisons, interval checks, buffer growth and
  sorting charge actual work. Explicit work or byte ceilings can therefore move in either direction.

### Stage 3: fixing atoms false

- **The shape.** A constraint whose body is one possible atom, certain positive literals and comparisons, over a
  restricted predicate with no disjunctive or conditional-disjunctive producer. Examples:
  `:- visit_time(V,_,T), time_window(V,Min,_), T < Min.` and its `T > Max` twin
  (`traveling-salesman/variant-05.lp`); `:- start_time(T,Start), time_window(T,Earliest,_), Start < Earliest.` and
  `:- end_time(T,End), time_window(T,_,Latest), End > Latest.` (`task-allocation/variant-04.lp`).
- **Certainty is fixed before completion.** A certain literal is a positive atom in every answer set. The head filter
  runs during support completion and is sound only if every literal it treats as certain is certain when it runs, so
  the certain-atom relations are complete before support completion begins, as certain aggregate elements already
  require of their keys.
- **Out of the shape:** negative literals, since no complete static source of impossibility exists before completion
  (an atom not yet derived may be derived in a later round); disjunctive producers, whose removal of a fixed-false atom
  needs its own statement and serves no target; and inversion through other rules (for example `End > Latest` through
  `End = Start + D`), which brings in a second possible atom.
- **The restriction.** Such a constraint shows a possible atom `p(t)` false in every answer set wherever some certain
  binding of the other literals makes the comparisons defined and true.
- **The operation fixes the atom false.** A choice element for `p(t)` is dropped, since it forces nothing. A normal
  producer instance for `p(t)` stays, emitted as a constraint on its body. Dropping it would be wrong:
  `{a}. b(5) :- a. :- b(X), X > 3.` has one answer set, ∅. Fixing `b(5)` false turns its producer into `:- a.`, which
  keeps ∅; removing the producer leaves `{a}.`, with two.
- **The per-round head filter.** When support completion derives an atom of a restricted predicate, it evaluates the
  restriction with the atom's arguments bound: a join over the certain literals' relations, then the comparisons with
  checked operations. If the restriction holds, the atom is not inserted into possible support, and its producer
  instance is recorded for emission as a constraint, or its choice element dropped. If evaluation fails, the atom
  stays possible, which is sound. Downstream, rules that read a fixed-false atom lose those substitutions; in
  traveling-salesman the recursion stops at visits beyond the window.
- **Admission.** Fixing atoms false prunes below their producers' closure, and the grounding chapter asks such pruning
  for a certificate preserving admission throughout the affected consumers. An atom is fixed false only when every
  family that reads it, transitively through the atoms whose support its absence removes, is statically total. Those
  families' outcomes are then only `produced` or `filtered`, so admission and warnings are identical with or without
  the removed substitutions (Lean statement 5). In traveling-salesman, `visit_time`'s readers are the recursive rule,
  whose `T1+ET` stays within the narrowed bounds, and constraints without arithmetic. The grounding chapter's passage
  on pruning below the closure is revised with the stage.
- **Reach.** Traveling-salesman variants 4 and 5, where the recursion stops at the windows, and task-allocation
  variant 4, where `end_time` atoms beyond a window's close leave possible support. Its `start_time` constraint is
  eligible too and removes nothing on the maintained scenarios only because every window there opens at 0.

### Stages 4 and 5

**Stage 4: admission refinements.** These would admit programs refused today: variables bounded by comparisons
against relational bounds (relaxing the envelope's closed-endpoint rule), and products of bounded variables. The
choice waits until Stage 2 is measured.

**Stage 5 (research): linear constraints without enumerating combinations.** A linear constraint over bounded
choice-bound integers, such as the natural SEND+MORE=MONEY encoding, is grounded today as every combination. Without
propagation during search, the route inside ASP is a decomposition into auxiliary atoms, such as partial sums or
carries over bounded domains, with a theorem that answer sets correspond by projection. It is recorded as the
direction the analysis makes possible, not designed.

## Correctness

**Concrete semantics.** The possible atoms are the least fixed point of the program's positive relaxation, which
support completion computes; every answer set's atoms are possible atoms.

**Soundness of the abstraction**, by induction on a possible atom's derivation, as
`DomainProducers.derivation_covered` proves for set domains. The additional obligations are numeric transfer and
refinement producers; these are not a proved Rust refinement:

- A slot's domain covers every complete binding: every positive occurrence's column covers the slot's value; a `Bind`
  value lies in its transfer by the operators' proved interval laws and the checked semantics (`ScalarArithmetic`),
  and an unproved operator transfers to `Unbounded`; arithmetic finite components stay Unknown; a
  generated range value lies within its endpoints; a membership test binds nothing.
- A head value lies in its image, the head term's transfer over the slots' produced ranges.
- Refinement is sound for produced heads: a defined false comparison excludes its substitution, and an undefined one
  omits the instance or refuses the program. Refinement needs both operands `Integer`, since term order decides a
  comparison involving a non-number.
- Widening only enlarges, and the narrowing pass stays above the least fixed point.

Hence every argument of every possible atom, and so of every answer set, lies in the computed domain, and every
complete binding lies in its slots' domains.

**Each consumer.** (1b): when the endpoint criterion holds, the comparison has one verdict on every complete binding;
dropping an always-true literal changes no complete binding's verdict, and an always-false literal makes every
substitution of its fragment produce nothing, with family classification skipped only under the two conditions.
(1d): today's totality argument, with the premise from the static certificate. Stage 2: the residual comparison is
kept, so a covering probe followed by the guard recovers exactly the satisfying bindings
(`IntegerEnvelopes.filtered_bindings_exact`); an expression index returns exactly the rows whose defined key equals
the probe; skipped rows are excluded substitutions; and the delta cut keeps `DeltaRounds`' partition. Stage 3: fixing
false an atom false in every answer set preserves the answer sets (Lean statement 4); certainty is complete before
support completion begins; and admission is preserved by the static-totality certificate.

**Failure boundaries.** An unsupported local transfer widens conservatively. A logical limit produces `Stopped`,
clears all queryable results, and retains its original context and committed work. A reader/control failure, invalid
neutral input or fallible allocation remains a typed failure with the committed receipt, never Unknown data. The
source compatibility wrapper retains its existing host-allocation boundary; it does not fabricate a logical stop.
Consumers act only on established premises; otherwise the general path decides, with today's diagnostics. Refusals
stay refusals and warnings stay warnings, and the family table in the language reference is never pre-empted.

**Preservation.** The analysis over-approximates possible atoms and decides no membership; consumers change which rows
are read and which instances are formed, never a verdict, and Stage 3 fixes false only atoms false in every answer set,
keeping their normal producers as constraints. Static decisions, pruning, probes and indexes are join primitives; the
analysis is preparation. Nothing propagates during search, and membership certificates are untouched. Facts are read
from the fact blocks' columns, and ordered and expression indexes are columnar structures a device can share.

## Costs and accounting

The shared engine changes the source analysis's logical work schedule: synchronous snapshots, argument metadata,
registration, finite intersections and their actual inspections now determine receipts. Compatibility preserves
inclusive meanings and typed context, not old numeric receipts. The source wrapper retains infallible host
collection allocation and final public `BTreeSet<&Symbol>` materialization. That conversion is outside the engine
work meter: on completion `value_entries` counts its entries, and `work` counts lowering plus engine operations.
Its finite cell count and each symbol's structural/payload size are bounded by the same admitted limits; conversion
uses O(Σ arguments V log(V + 1)) bounded symbol comparisons and O(total finite entries) references. The IR consumer
reads authentic coordinates/keys directly and has no source-set conversion. Its metadata allocation is fallible.
Neither boundary turns allocation failure into an invented logical ceiling.

| Domain limit | Meaning in the shared engine |
|---|---|
| `max_work` | Admitted lowering, registration, transfer, inspection, comparison and metadata-copy work |
| `max_predicates`, `max_positions` | Registered signed predicates and their argument positions |
| `max_links` | Retained plan records, slots, bindings, guards, heads, provenance, constants and thresholds |
| `max_values_per_argument` | Finite-set width; excess widens the local finite component |
| `max_value_entries` | Live finite references in seeds, arguments and temporary sets; released with their owner |
| `max_rounds` | Started synchronous image passes, including the unchanged ascent pass and narrowing |
| `max_inspected_bytes` | Cumulative name/string payload admitted before inspection and comparison |
| `max_symbol_nodes`, `max_symbol_depth`, `max_symbol_bytes` | Per-key finite admission; excess drops finite precision while retaining classification |

`Statistics::value_entries` is live occupancy at the receipt boundary, not a peak or total allocation count.
Failed temporary work releases its finite reservations; the `Stop` records the refused attempted count.

The analysis charges the formula work budget under its own logical limits, as the domain phase does today
(`DomainPrepareWork`, `formula_domains.rs`). Consumers charge the work they perform, so counters fall where machinery
is skipped and rise where indexes or filters are built.

| Mechanism | Stops being charged | Starts being charged | Net |
|---|---|---|---|
| the analysis | the set analysis's rounds on positive-flat programs | its rounds, within `max_work` and the formula work remaining, on every formula-route program with a domain attempt; the eligibility walk (`PositiveSource::check`) is still charged, and on programs that are not positive-flat the rounds are new work | rises |
| (1b), always true | the literal's evaluations | rows its prefix pruning removed, read until a later literal decides them | data-dependent |
| (1b), always false | the fragment's support and emission joins, and its classification traversal under the two conditions | one positive binding per such fragment | falls |
| (1d) | rows that complete a false prefix | — | falls |
| (2a) | rows outside the probed range | the shared index, `O(n log n)` per column snapshot over n rows; each query's binary bounds, candidate interval checks and source-order sort | data-dependent |
| (2b) | rows whose key differs | the shared index, `O(n*m + n log n)` per descriptor/snapshot for n rows and m nodes, and its binary lookups | data-dependent |
| Stage 3 | derivations of atoms fixed false and their downstream substitutions | one restriction evaluation per derived atom of a restricted predicate, plus the producer instances recorded as constraints | data-dependent |
| storage | — | indexes, leased like the join's other storage (`SupportBytes`) | rises while held |

Under a bounded profile the analysis and index builds can move a work ceiling earlier, and pruning can move it later. A
cell that flips between refused and completed is reported with both work totals.

**Receipts.** A preparation receipt, uncharged and bounded by the program's size like `comparison_forms()`: arguments
analyzed, `Integer` and bounded; comparison literals decided statically, true and false; rules statically total; and
the analysis's rounds, work and status. The public observation carries the analysis itself. Implemented Stage 2
counters are observed in support completion and eager emission: `numeric_index_builds` counts published certificates
and `numeric_index_declines` completed cached data declines; `range_probes` and `expression_key_probes` count completed
queries, including empty results; `range_probe_rows` and `expression_key_rows` count rows actually visited after the
delta cut and before the matcher. These are execution receipts, not proof or timing claims. Stage 3's
`fixed_false_atoms` and `fixed_false_instances` remain planned. Hybrid check joins are not observed, as for the
comparison forms.

## Tests

- **Transfer soundness.** A property test over random intervals and values for every operator, against the checked
  scalar operations. It checks the Rust implementation against the laws, which are proved in Lean; an operator whose
  law is not proved must transfer to `Unbounded`, and a test pins that for each such operator.
- **Exclusion role.** For each literal kind (`Compare` and `ArgumentCheck` over occurrence-bound variables and over a
  generated variable, a membership test, a `TupleCompare`, an aggregate guard), the static role equals the join's
  `Decisions::decides` on every join of the maintained programs and the regressions.
- **Guard refinement.** On `p(X,Y) :- a(X), b(Y), X < Y, Y <= X.` over [0, N], one pass gives X and Y within
  [1, N−1], deterministically.
- **Set component.** A finite set of numbers narrows the interval to its hull. Arithmetic and generated ranges
  keep Unknown finite components; copies and intersections preserve authentic keys and semantic equality.
- **Opaque summaries.** Distinct and explicitly reused Unknown nodes, including enclosing arithmetic trees, cannot
  establish subtraction correlation or carry guard restrictions to a head. An imprecise producer still widens a
  predicate seeded by precise facts.
- **Work boundaries.** Finite equality, membership and reduction; threshold and refinement searches; widening in
  both directions; identity traversal; and metadata copies stop before the first inadmissible inspection. Global
  stops clear every bound and preserve committed receipts and source contexts.
- **Probe partition** (Stage 2). Exact per-variant binding sequences are compared with the traversal without probes,
  including a matching new row in the first pivot and an empty second pivot. Removing the source-interval cut must
  introduce extra bindings and change the exact query-row receipt. Recursive fixtures compare the full catalog and
  graph and require support-phase index/query receipts. Producer visits and published support stay invariant;
  preparation, query, join and work receipts reflect the different operations actually performed.
- **Numeric query boundaries** (Stage 2). All four ordering operators over duplicate keys, i32 endpoints, out-of-order
  numeric values and full, partial and empty source intervals return the exact source-order positions. Equality and
  range queries share one descriptor. Tests check temporary leased bytes, refusal without leaked storage, and whole
  index decline for mixed values.
- **Failure and route boundaries** (Stage 2). Selected and reference runs compare exact catalog/graph results or the
  typed scalar failure and source site. Independent fatal expressions on excluded versus retained bindings protect
  the complete-row policy. Family evidence retains its false rows with zero index/query receipts; hybrid capture
  keeps its emission path. Broader traversal coverage remains a separate extension.
- **Coverage.** On small programs every possible atom lies in the analysis's domain, differentially against the
  exhaustive reference where it applies.
- **Widening and precision.** A recursive increment with a known constant guard cap checks outward widening and
  recovery of the cap. Traveling-salesman's `high(M)` case remains an acceptance obligation after aggregate envelopes.
- **Named regressions**, as answer and admission fixtures with the consumer's receipt, each checked with
  `--grounder eager` and against clingo 5.8.2:

  | Program | Expected |
  |---|---|
  | `q(a). q(1). p :- q(X), X > 5.` | `p q(1) q(a)`; `X > 5` gets no static decision |
  | `r(#inf). r(#sup). r(3). p :- r(X), X > 5. s :- r(X), X < 2.` | `p r(#inf) r(3) r(#sup) s` |
  | task-allocation variant 4's `:- same_agent(T1,T2), T1 < T2, overlaps_with(T1,T2).` | answers identical to clingo; `T1 < T2` compares task symbols and gets no decision |
  | `q(-5..5). r(1..9). p :- q(X), r(Y), X > 0, Y/X > 1.` | `p` with every fact, admitted with no warning; no consumer treats `Y/X` as defined |
  | `d(1). p :- d(X), X/(1;0) > 5.` | `d(1)`, admitted with one zero-divisor warning |
  | `{a}. b(5) :- a. :- b(X), X > 3.` | one answer set, ∅; under Stage 3, `b(5)` is fixed false and its producer kept as `:- a.` |
  | `d(1). p :- d(X), Y = X+1, Y > 5, 1/(X-1) = 0.` | refused today (`zetesis::source-expansion`) and stays refused; clingo answers `d(1)` with an info message |
  | `d(1). z(0). p :- d(X), z(Z), X > (5;0), X/Z > 0.` | refused today and stays refused; clingo answers `d(1) z(0)` with info messages |
  | `q(0..5). p :- q(X), X = 1..5, X > 0, 1/X >= 0.` | `p` with every fact, admitted with no warning: `X = 1..5` is a membership test, so X's domain stays [0,5] and `X > 0` is not decided statically |
  | `q(-1..1). r(1..9). s(1..5,1..9). p(W) :- q(X), r(Y), Y/X > 1, s(X,W).` | `p(1)`..`p(9)` with every fact, admitted with no warning: a typed form must not divide by zero at the prefix X = 0 |
  | `q(-1..1). r(1..9). s(1..5,1..9). p(W) :- q(X), r(Y), X > 0, s(X,W).` | `p(1)`..`p(9)` with every fact, unchanged by (1b) dropping `X > 0`; its rows read are reported |

- **Route receipts** for each consumer, and **families**: answer families unchanged on the portable, oracle and
  scalability suites, and against clingo on the aggregate and arithmetic campaigns.

## Lean

The proofs already state the parts this composes: set producers (`DomainProducers.transfer_monotone`,
`derivation_covered`), integer envelopes (`IntegerEnvelopes`), the checked semantics (`ScalarArithmetic`), guard
exactness (`DomainBindings.guarded_continuations_exact`) and the family policy (`ArithmeticFamilies.Admissible`,
`prefix_does_not_establish_refusal`). New statements, in one module, `NumericDomains`:

1. **Image and refinement producers.** A producer's output is an operation applied to values drawn from each input's
   domain, met with guard refinements over `Integer` operands; the transfer is monotone and every derivation is
   covered; widening enlarges, and one narrowing step from a post-fixed point stays above the least fixed point.
2. **Interval separation as an endpoint criterion per relation.** For integer intervals I1, I2 and x ∈ I1, y ∈ I2:
   for `<`, high(I1) < low(I2) makes every pair true and low(I1) ≥ high(I2) every pair false; for `<=`,
   high(I1) ≤ low(I2) and low(I1) > high(I2); `>` and `>=` mirror them; for `=`, one shared single point makes every
   pair true and disjoint intervals every pair false; `!=` negates `=`'s cases. With both operands `Integer`, term order
   on numbers is integer order, so the criterion decides the source comparison. `OrderedBounds.separated_comparison`
   takes separation as a hypothesis, for aggregate masks, and does not provide this.
3. **Expression-index exactness.** An index built from every row's defined key returns exactly the rows whose key
   equals the probe.
4. **Fixed false.** For an atom false in every answer set with no disjunctive or conditional-disjunctive producer,
   turning each normal producer instance into a constraint with the same body and removing each choice element for it
   preserves the answer sets.
5. **Admission beside statement 4.** A family whose every instance is defined has only `produced` and `filtered`
   outcomes; it and every subfamily are `Admissible` without a warning, so removing substitutions from statically total
   families changes no admission outcome.
6. **Interval laws `IntegerEnvelopes` does not cover.** It proves addition (including correlated operands), negation,
   products by a literal, truncating quotients by a positive or negative literal, and the remainder's magnitude for a
   nonzero literal. `NumericDomains` adds products of two variable intervals by their corners; quotient and remainder
   by a divisor interval excluding zero, with `MIN / −1` excluded; fixed nonnegative exponents; absolute value, with
   `|MIN|` excluded; and the remainder's sign. Each is stated over the integers with the i32 confinement as a
   hypothesis, as `Range` declines a result that leaves i32.

The requested certificate grammar is constants and bounded integer slots, unary minus, `+`, `−`, multiplication by
a literal factor, division by a nonzero literal, and truncating remainder over a divisor interval excluding zero.
The remainder also declines a possible `MIN / −1` pair. Every intermediate is checked, including both children of
correlated subtraction. Absolute value, bitwise operations, powers and variable multiplication/division decline in
this certificate producer even where the run-time `Range` implementation has a transfer. Ordinary defined-value
propagation keeps its separate precision and does not establish expression totality. Static-totality support selection
retains the original source-family checks; measured-route evidence and its concrete Rust correspondence remain separate
obligations. Separated comparison decisions remain diagnostic.

**Rust obligations**, listed in the correspondence: the IR lowering covers every produced value (the transfer table,
matched exhaustively); `Range` implements the laws within i32 for the operators whose laws are proved, an obligation
that becomes answer-critical for (1b) and Stage 3, which remove literals and atoms without a run-time re-check, and
admission-critical for (1d); the exclusion role agrees with the join's `Decisions`; the fixed point, widening and
narrowing are implemented as stated; and limits and stops publish nothing partial. Stage 2 does not rest on `Range`:
its indexes re-check their premise on the data.

## Measurement and acceptance

For every stage, against the parent commit's release build: reach, counted by the receipts per program and class over
the corpus and scalability populations; interleaved A/B of grounding; and the high-resolution corpus campaign at one
and fourteen workers. Accept when:

- answer families are identical;
- admission is identical: warnings and refusals compared by source site on the corpus, the scalability population and
  the arithmetic validation fixtures, with any difference listed for the maintainers;
- no cell flips between refused and completed at the default limits, and a flip under another profile is reported with
  both work totals;
- no cell is slower by more than 3% with separated ranges, including the fixed-cost regime, where the analysis's cost
  lands on small programs;
- the stage's target holds: Stage 1 with the target it lands with (Stage 3's target; prime-sieve's support completion
  falling on both sub-routes for (1d); the census's reach and rows removed for (1b); columnar Stage 2's acceptance with
  no fact view built), its census reported in every case; Stage 2, n-queens variants 1–4 grounding falling; Stage 3,
  support completion falling on the time-window classes;
- the report states how much arithmetic grounding the stage removes: rows visited, instances formed and, in Stage 3,
  possible atoms.

Expected reach: Stage 2 on n-queens 1–4, task-allocation 3–4, traveling-salesman 4–5, shortest-path 3–4, prime-sieve
and pigeonhole; Stage 3 on traveling-salesman 4–5 and task-allocation 4. Owed measurements: the analysis's cost in the
fixed-cost regime, Stage 2's per-round index rebuilds, the n-queens estimate, Stage 3 on the time-window classes (whose
losing instances are tiny: traveling-salesman variant 5 case 05 has six vertices and runs in 5.6 ms) and (1d)'s gain
on both sub-routes.

## Alternatives

- **gringo's approach.** clingo grounds every arithmetic instance bottom-up, with predicate domains but no numeric
  typing, and leaves non-grounded arithmetic to clingcon, which propagates over integers during search. Ruled out.
- **Relational abstract domains** (octagons, polyhedra). They capture correlations such as `R - C` at more cost, and no
  Stage 1–3 consumer needs them: expression keys handle the n-queens correlation exactly, from the index build.
- **Refinement types.** The analysis is such an inference, with the program as its only annotation. The representation
  (three components per argument) is kept separate from the inference, so declared sorts in a future dialect can seed
  or check it.
- **Run-time column typing alone.** Exact and cheap on a snapshot, and Stage 2 rests on it. The static analysis adds
  what run time cannot: bounds during rounds and through recursion, decisions before data exists, and admission.
- **Extending the source-level set analysis in place.** It reads the themelios program, while the numeric transfers
  need the IR's slots and expressions.
- **Thresholds read from guard operands during iteration.** Thresholds that change during iteration void the
  termination measure; narrowing recovers the same caps with no termination question.
- **Extending the finite guards to every rule the analysis covers.** Each kind of rule would need a soundness argument
  under `DomainBindings`' premises, receipts and an acceptance clause, and `Candidates::prepare` declining instead of
  refusing. A later design.
- **A per-column fact dictionary now.** No landed stage has one, and the analysis reads facts once per preparation. A
  later columnar step.

## Decisions

1. Widening thresholds come from the program; nothing is configured.
2. Stage 3 is an execution choice resting on answer-set equivalence, with its Lean statement. Its admission argument
   is the static-totality certificate (statement 5), so admission is exactly where the contract puts it. A
   redefinition of the arithmetic family over the restricted support may be weighed only if Stage 3's measured reach
   under the certificate proves too small.
3. Stage 4's choice waits until Stage 2 is measured.
4. Stage 5 is within the thesis, as research: a decomposition inside ASP, not propagation during search.
5. One analysis: the engine absorbs `zetesis-domain`'s set analysis, with the lowering from the IR.
   `zetesis_domain::analyze(&Program)` and its compilation stay for library callers.
6. The analysis inherits the `with_domain_analysis` opt-in, so no embedder's work budget or refusal points move unless
   it asks. Running it unconditionally is revisited once Stage 2's measurements show the analysis's fixed cost.
7. Facts reach the analysis from the fact blocks. Columnar Stage 2 drops its fact view, and Stage 1 of this design lands
   no later than columnar Stage 2.
8. Observers resolve values through a lent resolver, and variants left without a producer, `Inapplicable` among them,
   are removed, as a breaking 0.x change.

## Open review items

1. The neutral result is `zetesis_domain::numeric::Analysis<K, C>`; `Input`, `Values`, `Budget`, typed coordinates,
   `Bound`, `RuleBounds`, status and failure types are public. The IR observation lends authenticated key resolution;
   its adapter and callback contract must stay synchronized with this shared result.
2. One engine with two lowerings and direct fact-column input is implemented. Source `analyze(&Program, Limits)`
   retains its public types and fallback producers; the IR uses local Unknown summaries for imprecise outputs.
   Source and IR adapter correspondence, including every producer, remains an executable obligation.
3. Limit meanings and changed work receipts are stated above; source allocation retains its compatibility boundary,
   while fallible prepared-IR failures remain typed.
4. Make the finite guards' receipts claim true: state whether running the analysis turns the guards on, that (1d)'s
   `Coverage::Selected` turns guard rejection on for the rules it reaches, and the fall in candidate-preparation work.
5. Conservative aggregate transfers are implemented as stated under Sources. Tighter count/weight envelopes,
   negative sums, i32 overflow and min/max nonempty evidence remain open; no traveling-salesman horizon is claimed.
6. Say what supplies certain literals to Stage 3: an existing analysis, a planned one, or fact atoms only. Stage 1's
   largest target waits on that analysis; scoping Stage 3 to fact certainty would avoid the wait.
7. Restrict (1b)'s family shortcut to fragments with a complete binding, and add the fixture
   `d(0). p :- d(X), Z = 1..X, X > 5.`, admitted with no warning.
8. Correct the Lean plan's coverage: state Stage 3's certainty premise as a Rust obligation; record `Range` as
   admission-critical where an admission argument rests on it; list `\` by a literal as covered for its magnitude bound
   only; add `~` to statement 6 or transfer it to `Unbounded`; and restate the bitwise sentences.
9. Arithmetic image sets are omitted. Authentic finite keys serve the existing finite consumer; no unused image
   representation or synthetic catalog values are introduced.
10. Replace the probe-partition test's program with a recursive relation probed against itself, so that disabling the
    delta cut fails the test.
11. Fact blocks are seeds, not expression-totality certificates; the producer table reflects this distinction.
12. Verify every cited site at the branch head; write the width bound as `max_values_per_argument`; and state in every
    place that a failure in Stage 3's restriction evaluation leaves the atom possible.
