# zetesis-ferraris

A native exact kernel for **finite propositional formula reducts**. The Horn
oracle in `zetesis-cpu` computes a least closure. A general reduct may have
incomparable minimal models; this kernel checks proper subsets instead.

The input is a bounded, immutable DAG of atoms, falsum, conjunction, disjunction,
and implication, plus asserted roots. Default negation is implication to falsum.
One pass computes the candidate's classical truth mask. A second transform
evaluates the frozen reduct in a proposed countermodel: every candidate-false
node becomes falsum. The checker rejects a classical nonmodel immediately,
otherwise enumerates proper subsets until it finds a countermodel or proves
minimality. Rejection includes the failed root or a proper-subset witness.

The exhaustive reference checker has work/subset bounds and shared CPU
cancellation/deadline control. It is not a production search strategy for large
disjunctive programs. The aggregate constructor below translates already finite
scalar aggregates into this same formula DAG. The crate does not ground source,
evaluate optimization objectives, or run on a GPU. Source translation and execution
refinements remain separate obligations.

`proofs/Zetesis/Ferraris.lean` establishes the denotational definition and its
equivalence to minimal reduct models. It does **not** prove this Rust code.
`proofs/Zetesis/FerrarisMask.lean` separately proves that a correct frozen truth
mask can replace explicit reduct construction during formula and theory
evaluation. Its hypothesis requires mask correctness; it does not verify DAG
indices, packed bits, work accounting, or the subset counter.
Tests compare the fused transform against an independently materialized tree
reduct for 3,600 two-atom theories, all four candidates, and all four tested
interpretations, including nested implication and non-Horn reducts. Boundary
tests cover finite identities, packed words, empty theories, exact limits,
cancellation, and deadlines.

## Reusing a candidate's reduct

`FrozenReduct::new(&candidate, limits, control)` freezes the candidate's own
theory once. The value borrows that exact interpretation and owns its Boolean
truth mask. `is_satisfied_by(&tested, limits, control)` tests any interpretation
of the same theory instance, including interpretations outside the candidate.
Independent admission of identical syntax does not preserve instance identity.
No candidate words or original DAG are copied; `candidate()` and `theory()`
expose the bound subject. A frozen value is neither a model certificate nor an
answer set. Use the checked membership API for that conclusion.

Construction charges one evaluation per DAG node and retains one Boolean per
node. Each query allocates its own temporary Boolean workspace and charges one
evaluation per node, followed by root tests through the first failure. The value
is immutable and can be shared between independent callers. Construction and
each query have separate work budgets; these operations perform no subset search.
Cancellation, deadlines, foreign identity, allocation failure and exhausted
work remain explicit stops. A stopped query leaves the freeze unchanged.

The existing `models_reduct` wrapper still freezes anew on each call and uses one
combined work budget. Its identity checks and stop precedence are unchanged.
The exhaustive `check` operation already freezes once per candidate, so this API
does not speed up that checker. It gives library callers a way to avoid repeated
freezing when they separately test multiple interpretations against one reduct.
The `frozen_reduct` Criterion benchmark compares these caller patterns, including
and excluding initial freezing explicitly. It measures neither ordinary solving
nor grounding. No timing improvement is claimed without measurements.

Proptest adds generated DAGs with one to six atoms, up to 48 topological nodes,
shared subexpressions, deep chains, multiple roots, and permuted atom indices.
Each generated case checks classical truth, a frozen reduct under an arbitrary
tested interpretation, exhaustive stable membership, and returned countermodels
against an independently materialized tree. Test generation limits each root's
unrolled tree to 1,024 nodes; this protects the reference comparison from DAG
expansion without narrowing the admitted production language.

Proptest shrinks the raw instructions while preserving valid topological DAGs.
Failing seeds are persisted and replayed from
`tests/regressions/formula-dag.txt`; commit new entries after fixing their causes.
The tracked file is initially empty of failures because none has been observed.
Normal runs use Proptest's default case count; `PROPTEST_CASES` can increase it.
Focused deterministic tests also exercise shared DAG slots, sparse subset carries
across three machine words, exact work/subset limits, and all identity checks.

## Checked tight producer plans

`TightPlan::compile` extracts normal and atomic-choice producers from every
original root and checks strict positive ranks on a linear-size formula/atom
graph. `TightPlan::certify` validates a proposed rank against the same complete
extraction. Source-analysis labels alone do not authorize either route. Bodies
admit conjunction, disjunction and default negation; each default-negated
interior is candidate-frozen. Constraints and support guards retain their
original truth checks. General disjunction, unnegated body implication and
positive cycles remain on the general reduct path.

`TightPlan::check` evaluates original satisfaction and every present atom's
support. A completed ranked support check proves stability without a subset
query. A false root remains an original-model failure; missing support returns
`Residual` until a caller completes a witness-based rejection. The plan retains
immutable theory identity and has separate producer, dependency, logical-byte
and work limits. An incomplete scan never supplies a successful certificate.

`proofs/Zetesis/TightPlans.lean` proves the formula-level satisfaction/support
characterization for this grammar, including original choices in both branch
orders. It does not prove the Rust extractor, rank implementation or missing
lazy source coverage. Tests compare finite trees and their materialized reducts,
validate rank/root/identity/resource boundaries, and exercise the ordinary
candidate batch protocol. Two maintained corpus experiments report
[certificate eligibility](../zetesis-themelios/tests/tight_plans.rs) and
[complete enumeration comparisons](../zetesis-themelios/tests/tight_completion.rs).
The latter checks full model/score equality, optimum ties, candidate counts and
avoided reduct queries against scalar checking. Their output measures eligibility
and work counts, not elapsed time or GPU performance; it does not select a
default execution policy.

Run them from the repository root and retain the output with the source revision:

```sh
cargo test --locked -p zetesis-themelios --test tight_plans original_corpus_eligibility -- --ignored --exact --nocapture --test-threads=1
cargo test --locked -p zetesis-themelios --test tight_completion unchanged_corpus_complete_batch_experiment -- --ignored --exact --nocapture --test-threads=1
```

## Finite scalar aggregates

`append_aggregate(nodes, elements, comparison, bound, limits, control)` appends a
formula to an existing topological `Vec<Node>`. It returns an absolute root index,
appended-node count, translation profile and statistics. The root is not asserted
and no surrounding rule is added. Every new node is an ordinary connective;
compilation introduces no semantic atoms or separate support rules.

Each `AggregateElement { weight: i32, condition: usize }` represents one distinct
whole tuple and its existing eligibility formula. Use weight one for `#count` and
the integer first tuple component for `#sum`, `#min` or `#max`. The caller must group equal complete
tuples and OR all their eligibility conditions before calling; equal weights are
not enough to group tuples. Arbitrary formula conditions, including default
negation, remain intact. In particular, an eligibility formula `p or not p` must
not be replaced by true.

The constructor admits a fixed `i64` scalar guard and `Eq`, `Ne`, `Lt`, `Le`, `Gt`
or `Ge`. The caller handles source local bindings, tuple expansion, assignment
values, multiple guards, head choices and source origins. Source intervals in a
guard, undefined/nonnumeric weights, `#sum+` and theory aggregates are not
implemented. Numeric `#min` and `#max` use the separate constructor below.
Default negation is added afterward as
`Implies(root, falsum)`; it must not be implemented by inverting the comparison.

The general translation uses one implication for every failing subset Δ:

```text
AND over failing Δ: (AND of conditions in Δ) -> (OR of conditions outside Δ)
```

Empty conjunction/disjunction mean true/false. This is the finite formula
translation from [Abstract Gringo, corrected v2, §4.7](https://arxiv.org/pdf/1507.06576v2).
Tuple grouping preserves each tuple's eligibility in both the candidate and the
frozen reduct because disjunction preserves the union of its alternative supports.
Grouping does not justify simplifying conditions by classical truth alone.

For nonnegative weights, a dynamic program constructs shared formulas `G(k)` for
sum ≥ k using only AND/OR and constants. Each cell combines excluding an element
with including it and the previous row's residual threshold. This is a compact
factorization of the monotone aggregate formula. `>` shifts the threshold; `<`
and `<=` negate the corresponding monotone threshold. Equality conjoins `G(k)`
with `not G(k+1)`. Not-equal uses `G(k) -> G(k+1)`, preserving the implication
needed at a nonconvex guard.

This follows the reduct equivalence and monotone/antimonotone laws in
[Ferraris, §§3.2–3.3, Propositions 12–13](https://arxiv.org/pdf/0812.1462).
For the not-equal specialization, frozen condition truth can only remove eligible
elements. If the candidate sum is below k, the implication's antecedent is frozen
false; if it equals k, the entire root is frozen false; if it exceeds k, the
implication tests whether the reduct-eligible sum is below or above k. This
argument requires nonnegative weights. Signed weights always use the full subset
implications and are never complemented and shifted by classical algebra.

For example, `p :- #count {1:p} != 0.` has only the empty stable model, whereas
`p :- not #count {1:p} = 0.` also has `{p}`. Both are regression cases. The
constructor is source-backed executable code; its aggregate translation has not
been refined into Lean.

## Shared comparison families

`append_aggregate_family(nodes, elements, guards, limits, control)` compiles an
ordered list of `AggregateGuard { comparison, bound }` against one identical
coalesced element family. It returns one root per guard, preserving order and
duplicate requests. `AggregateFamilyLimits` contains the existing aggregate
limits plus `max_guards`, which bounds the output-root vector independently.
The original single-guard API and its accounting contract remain unchanged.

For nonnegative weights, the family constructs one threshold table through the
largest needed nonconstant threshold. Every requested inequality or equality
then refers to its final row. Both thresholds of an equality share this table;
`!=` retains the implication between the corresponding thresholds. Thresholds
at most zero or above the total weight use constants and do not enlarge the
table. This preserves the same failing-subset and frozen-reduct semantics as
individual compilation, while avoiding repeated DP work. The complexity is
proportional to elements times the largest needed threshold, plus requested
guards and prefix validation. It remains pseudopolynomial.

Signed families share validation and constants, then compile the full
failing-subset formula for each guard. Work and subset limits apply cumulatively
to the whole transaction. A later guard that exhausts a limit rolls back every
new node; callers never receive a partial root family. Empty families still
validate the prefix and conditions and poll control, but append no nodes.
Temporary-state accounting counts DP cells or subset bits, excluding returned
roots and DAG storage. Root storage is capped by `max_guards`, whose default is
4,096; existing aggregate defaults are unchanged.

The caller must establish that tuple keys and eligibility conditions are the
same for every requested guard. In source assignment, this requires a complete
fixed support universe and independence from the changing assignment target.
This numeric constructor neither checks source binding independence nor proves
a source cache valid. Interning an appended DAG remains a separate transform.

Eight portable family tests compare 6,300 exhaustive guard configurations and
192 generated families with an independent full failing-subset definition, for
all two-atom candidates and tested reduct interpretations. Further checks cover
guard order, duplicate guards, extreme bounds, zero weights, inclusive limits,
empty families, cancellation and rollback after a later signed guard stops.
A 16-element count family covering every equality and a duplicate request uses
one threshold table and less than one fifth of the work of separate calls.

## Numeric minimum and maximum

`append_extremum(nodes, elements, extremum, comparison, bound, limits, control)`
appends a `Min` or `Max` comparison over the same coalesced tuple elements. The
numeric first component may be any `i32`, including negative and extreme values.
`ExtremumBound` is `NegativeInfinity`, `Number(i64)` or `PositiveInfinity`, with
conversions from `i32` and `i64` for direct numeric arguments. Infinite guards are
distinct from the smallest and largest integers. The empty minimum is `#sup`;
the empty maximum is `#inf`, following
[Abstract Gringo, corrected v2, §2.1](https://arxiv.org/pdf/1507.06576v2).
Nonnumeric or explicitly infinite element values are outside this numeric API.
Source assignment and the discovery of possible assignment values remain caller
obligations; this constructor alone admits no additional source program.

For maximum, `G` is the disjunction of conditions whose value is at least the
guard, and `H` uses strictly greater values. For minimum, these tests reverse:
`G` uses values at most the guard and `H` uses strictly smaller values. At the
appropriate empty-set sentinel, `G` includes true. The remaining comparisons
use these two monotone formulas:

| Comparison | Maximum | Minimum |
| --- | --- | --- |
| `>=` | `G` | `not H` |
| `>` | `H` | `not G` |
| `<=` | `not H` | `G` |
| `<` | `not G` | `H` |
| `=` | `G and not H` | `G and not H` |
| `!=` | `G -> H` | `G -> H` |

This is a linear factorization of the full failing-subset formula, justified by
the same monotone/antimonotone laws above. For `!=`, frozen conditions can only
remove eligible elements. When `G` is candidate-false its implication freezes
to true; at equality the entire root freezes to false; when both `G` and `H`
hold, the implication retains exactly the forbidden equality test on the
reduct-eligible elements. For minimum this argument follows increasing witness
sets for `<=` and `<`, rather than the order of numeric results. It requires no
sign restriction on the finite values. Default negation still wraps the result
with implication to falsum. For example,
`p :- #min {1:p} != #sup.` has only the empty stable model, while replacing its
body with `not #min {1:p} = #sup` also allows `{p}`.

The extremum profile charges prefix validation and a linear input scan. It adds
at most `2*n + 4` nodes and retains no dynamic threshold rows or subset carrier;
`max_states` and `max_subsets` can both be zero. Fixed local accumulators are not
counted as dynamic state cells. Signed extremes introduce no threshold shift or
integer negation. Work, nodes, elements, control polling, source-origin capacity
and transactional rollback follow the same contract as `append_aggregate`.

Seven portable tests check 12,600 exhaustive configurations and 192 generated
configurations against both direct selected-value semantics and the independent
full failing-subset formula. They compare every two-atom frozen candidate and
tested interpretation, including interpretations outside the candidate. Separate
cases cover both infinities, empty and tied values, composite and default-negated
conditions, all integer endpoints, malformed DAGs, exact budgets, cancellation
and a 4,096-element linear compilation with no subset or state budget.

Three optional equivalence tests compare 840 complete recursive source programs
with clingo, covering all comparisons, both infinities, zero/one/two default
negations, tuple coalescing, ties, interior numeric endpoint neighbours and
extreme element values. A fourth test characterizes the six known discrepancies
below; its successful execution does **not** count as six equivalence passes.

```sh
cargo test -p zetesis-ferraris --test extrema_clingo -- --ignored
```

### Known clingo numeric-endpoint compatibility gaps

The direct kernel follows exact finite integer and infinity semantics. clingo
5.8.2 disagrees for the following original source shape:

```text
p :- #FUNCTION {W,k:not p} OP W.
```

| Function | `W` | `OP` |
| --- | --- | --- |
| `min` or `max` | `-2147483648` or `2147483647` | `!=` |
| `min` | `2147483647` | `>` |
| `max` | `-2147483648` | `<` |

Each of these six cases has `{}` and `{p}` under the full failing-subset
definition; clingo 5.8.2 returns only `{p}`. Adjacent interior guards agree.
The operational cause has not been established by this kernel work, and no
integer wraparound has been introduced to imitate the observation.
[The boundary evidence](tests/fixtures/extrema-clingo-5.8.2-boundaries.json)
retains all 24 endpoint/comparison probes, their exact source, raw clingo output,
ground text and separate equivalence/mismatch classification.
`known_clingo_integer_endpoint_gaps_are_reported_separately` checks the six
observations and requires reassessment if the oracle changes.

A frontend promising clingo compatibility must refuse evaluated numeric min/max
guards at `i32::MIN` and `i32::MAX` until an explicit compatibility policy is
implemented. This restriction does not apply to the direct mathematical kernel,
nor does it identify `#inf`/`#sup` with numeric endpoints. No universal source or
clingo compatibility claim follows from the successful kernel tests.

## Aggregate compilation bounds

`AggregateLimits` independently caps input elements, absolute total DAG nodes,
charged work, peak temporary state cells and enumerated subsets. The threshold
profile uses two rows of threshold indices and takes work proportional to the
number of elements times the scalar threshold; its arithmetic complexity is
pseudopolynomial. Large weights or bounds can trigger `StateLimit` even with few
elements. Signed compilation is exponential and admits the entire subset count
before enumeration; `max_subsets` includes the empty subset. The threshold profile
does not enumerate subsets. A 64-element exact-eight count is covered by tests
with a zero subset budget.

Existing edge topology and element indices are checked under the work budget.
`Theory::new` remains responsible for validating atom-universe indices. Every
charged operation polls shared cancellation/deadline control; storage reservations
and arithmetic are checked. The state count excludes DAG storage and allocator
overhead. All ceilings are inclusive, and zero is a real ceiling.

On failure, appended nodes are removed and the original prefix and length are
preserved; vector capacity may have changed. Error statistics report work done
before rollback. Callers maintain origin tables separately and can clamp the total
node limit to the number of origins they can admit. Any stopped compilation is a
refusal, never an unsatisfiable formula.

Eight portable aggregate tests cover 6,300 exhaustive configurations and 192
generated configurations against independent frozen-condition semantics, checking
all two-atom candidate/tested worlds, including tested worlds outside the
candidate. They also cover empty aggregates, zero/signed/extreme weights, repeated
condition IDs for distinct tuples, exact budgets, rollback, bad DAG edges and
control interruption. Two optional tests compare 152 complete small recursive
source programs with clingo, including OR-coalesced tuples and default-negated
aggregates:

```sh
cargo test -p zetesis-ferraris
cargo test -p zetesis-ferraris --test aggregate_clingo -- --ignored
```

The optional subprocesses use temporary files, a five-second deadline and a
64-KiB captured-output ceiling. clingo is only an independent test oracle.
