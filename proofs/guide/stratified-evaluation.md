# Stratified evaluation

[`StratifiedEvaluation.lean`](../Zetesis/StratifiedEvaluation.lean) relates
evaluation by strata to the actual answer-set definition. It extends the
least-consequence treatment of positive programs to rules whose default
negation refers only to lower strata.

Each atom has a natural-number rank. A producer's positive body atoms have rank
at most that of its head. Its frozen gates have strictly smaller rank. Ordinary
default negation uses the false gate; the statement also covers true gates
under the same strict condition. Constraints produce no atoms and do not need
a rank restriction.

Positive cycles may stay within a stratum. A negative dependency cannot do so.
The condition is about complete rule dependencies, not the order in which rules
happen to be visited.

## Evaluation and its proof

Let `Gamma(P, M)` be the complete least consequences of the reduct whose gates
are frozen by `M`, as defined in `Semantics`. Define:

```text
stage(0)     = empty
stage(n + 1) = Gamma(P, stage(n))
```

This is a denotational construction. Each call computes a complete positive
closure. Higher strata can change while lower strata settle, so the stage
sequence is not claimed to grow monotonically as a whole.

The argument has four steps:

1. **Lower agreement determines the next stratum.** If two frozen
   interpretations agree below rank `n`, their complete reduct closures agree
   below rank `n + 1`. A producer in that prefix reads positive atoms inside the
   prefix and gates below it. Least closure supplies the claim even when
   positive dependencies form a cycle. This is `gamma_agrees`.
2. **Completed strata remain fixed.** After `n` closures, atoms below rank `n`
   retain their truth at the next stage. The empty prefix starts the induction;
   the first result advances it. This is `consecutive_agree`.
3. **The final result is unique.** If every atom's rank is below `levels`,
   `stage(levels)` equals its own reduct closure. Every other fixed point agrees
   with it, again by induction on ranks. This is `fixed_iff_final`.
4. **Constraints select that answer or none.** `NormalFerraris` already proves
   the correspondence between normalized reduct semantics and Ferraris
   satisfaction/minimality. `ConstrainedPositive.stable_append_constraints`
   proves that arbitrary constraints only filter answer sets. Applying both
   yields `answer_set_iff`.

The final statement is:

```text
M is an answer set of translate(P) plus constraints
iff
M = stage(levels)
and all original and added constraints hold in stage(levels).
```

The hypotheses are syntactic rank inequalities and a rank bound. They do not
assume that an evaluator's result is an answer set or even a fixed point.
Constraint bodies added at the Ferraris boundary may contain arbitrary formulas.

## Implementation boundary

The result justifies a semantic specialization, not a verified Rust evaluator.
These obligations remain separate:

- Account for every producer and dependency when translating the admitted
  formula grammar to normalized rules. Preserve original atom identity.
- Establish the rank inequalities from the complete signed dependency graph;
  a positive-only graph is insufficient for this purpose.
- Show that evaluating each SCC to least closure, in dependency order, returns
  the staged result. The mathematical construction does not prescribe repeated
  full-program closure in the implementation.
- Preserve original constraint checking, objective evaluation, displayed terms,
  and complete enumeration conclusions after constructing the one candidate.
- Preserve allocation, cancellation, deadlines and incomplete outcomes. No
  incomplete closure supplies the final interpretation in these theorems.

The library statements do not require finite rule iteration, dense atom IDs,
an eager ground store, a particular device, or a particular queue discipline.
Those implementation choices must establish their own correspondence.
