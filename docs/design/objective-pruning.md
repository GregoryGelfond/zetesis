# Incumbent bounds for answer-set candidates

An objective bound excludes only candidates whose exact score is worse than an
already verified, retained answer set. It changes the outer candidate query. The
original theory, original atom identities and Ferraris reduct membership check
remain unchanged. Optimization still compares fully evaluated scores of complete
verified answer sets.

## Construction

The optional `ObjectivePlan` consumes the completed possible atom catalog used
by source admission and the existing lifted objective templates. Positive joins
and scalar filters produce eligibility conjunctions over original atom IDs.
Equal `(priority, numeric weight, complete tuple)` keys are combined globally by
disjunction, including keys contributed by different objective statements.
Nonnumeric weights contribute nothing. Zero and negative numeric weights retain
their ordinary objective meaning. The catalog preserves original index order;
the planner checks its size and uniqueness but does not establish the caller's
stable-model coverage obligation.

For a verified incumbent, each priority compiles exact sum comparisons. The
combined constraint is lexicographic `candidate_cost <= incumbent_cost`, with
higher numeric priorities first and absent slots interpreted as zero. Equal
scores stay eligible. Nonnegative threshold families use the existing shared
dynamic program. A signed family can use the following classical normalization
after full-key coalescing:

\[
\sum_i w_i[E_i] = \sum_{w_i<0}w_i
  + \sum_i |w_i|[w_i<0\;?\;\neg E_i\;:\;E_i].
\]

Both the less-than and equality guards shift by the same negative offset. Each
original complete key remains a separate entry; equal transformed magnitudes or
conditions are not coalesced again. This changes only the classical candidate
restriction. It is not a rewrite of source signed aggregates or their reducts.

The planner estimates the required threshold cells and checks element, state,
node and work ceilings before copying a normalized family. Every scan, entry and
complement node remains charged. An unsuitable threshold shape, `i32::MIN`, or
an unrepresentable shifted guard retains the bounded signed lowering. Selection
is a representation heuristic, not a runtime improvement guarantee. Optional
construction can still decline within the same explicit limits.
This is an optional optimization, so a refused plan does not refuse an otherwise
admitted source or silently approximate an objective.

## Search protocol

The CLI requests a bound only after finding a strictly better model, completing
its original reduct check, evaluating its full objective and retaining it within
the incumbent storage limits. It verifies that the bound names the same immutable
original theory as the active search.

`StableModels::restrict_candidates` appends a separate classical encoding with
fresh auxiliary IDs. It preserves original clauses, previous bounds and all exact
semantic blocks, then restarts the outer traversal. Previous bounds remain sound
when an incumbent improves. Ordinary yields retain the traversal; equal scores
do not restart it. Restrictions never enter the inner proper-subset query.

An append failure restores the prior CNF and cursor. Spent work stays charged.
The CLI disables further optional bounds and continues exact search within the
previous sound restrictions; exhausted work or cancellation still interrupts
the next search operation. `--max-objective-bound-work` independently bounds
cumulative plan and bound construction; zero disables this optimization.
Encoding restrictions remains part of `--max-search-work`. Diagnostics identify
successful bounds and construction work, or the reason optional pruning stopped.

## Completion and evidence

Restricted search exhaustion establishes coverage within the retained candidate
region. The optimization protocol additionally establishes that every omitted
candidate is worse than a retained verified incumbent. Together these establish
optimality and all optimal full-model ties. The display limit may retain fewer
models for output while still counting every optimal tie. An interruption leaves
an incumbent, not an optimum. Bounds are never added before a verified model
exists, so a zero-model exhausted run still establishes UNSAT.

The frontend's independent adversarial campaign covers 24 sources and 97 complete
external model/cost records. For every verified incumbent it compares guard truth
over every semantic valuation with the lifted evaluator, and checks restricted
outputs against exhaustive original-Ferraris stability. Repeated improvements
exercise optimal ties and semantic blocking across restarts. CLI tests compare
pruned and disabled runs, including hidden ties, priorities, signed/global keys
and optional resource refusal. These finite tests do not establish general
source-to-Lean refinement or full clingo compatibility.

The Lean development separates the dominance argument from original stability.
Its abstract cost and coverage assumptions do not verify the Rust planner's
tuple joins, lexicographic lowering, CNF extension, allocator or cursor.
`SignedObjectiveBounds` additionally proves the offset identity, shifted
comparisons and priority ordering, candidate-region equivalence and preservation
of original stable optimal ties after complete-key coalescing. Machine-integer
checks and selection/fallback behavior remain implementation obligations.
