# Conditional partition consequences

The `zetesis_ferraris::partition` library derives local cardinality bounds from an
explicit finite partition. It is an experimental candidate-planning primitive.
`zetesis_sat::partition` re-exports the same types for existing consumers. The
pure planning and formula-emission implementation has no candidate-search
dependency, so a source compiler can reuse it without depending on a solver.
Ordinary source admission, candidate search defaults and reduct acceptance are
unchanged. The optional frontend `CountPlan` now establishes one narrow family
of source premises; the generic partition constructor still accepts only stated
premises and does not certify their relationship to a program.

For distinct members `U`, disjoint groups `G_i` covering `U`, a stated total lower
bound `L`, and stated group capacities `u_i`, let `c_i = min(u_i, |G_i|)`. Every
interpretation satisfying those premises obeys

```text
|M ∩ G_i| ≥ max(0, L − Σ(j ≠ i) c_j).
```

If `L > Σ c_i`, the premises are inconsistent. In particular, a lower total of
`n` over `n` disjoint at-most-one groups forces one selected member in each group.
The lower total need not be an equality. Empty groups and loose capacities are
well-defined; overlapping, missing, repeated or undeclared members are refused.

## What validation establishes

[`Premises`](../../crates/zetesis-ferraris/src/partition.rs) contains semantic atom IDs,
the declared member set, a total lower bound and ordered group capacities.
`Plan::new` verifies the finite universe and exact partition shape, then computes
the conditional consequence. It does not inspect a `Theory` or certify that a
program entails the supplied premises. The owned plan retains group/member order
and reports separately bounded construction work and requested vector payload.

The caller must establish the premises for every candidate region to which a
restriction is applied, with the same atom index meanings. A conditional group
cannot supply an unconditional capacity unless its activation is established for
that region. The API deliberately has no unchecked method claiming a
subject-bound theory certificate. The separate closed source bridge below
retains group, activation and original-theory evidence. It neither infers facts
from a filename nor reconstructs aggregate intent from Boolean gate shapes.

[`Plan::restriction`](../../crates/zetesis-ferraris/src/partition/restriction.rs) emits
only derived local lower bounds through the existing exact cardinality
translator. It returns a separate Boolean theory, including falsum for
inconsistent premises. This is a candidate-only view. Applying it through
`StableModels::restrict_candidates` preserves the immutable original theory as
the subject of every frozen-reduct check. Full original model coverage requires
the entailment premise above. The existing restriction API may change search
order; it does not promise the former enumeration order.

Emission has independent atom/node/root, per-group element/frontier and cumulative
work ceilings. The final bounded theory shape scan is documented separately from
the charged construction operations. No partially emitted theory escapes an
error. Neither construction nor emission performs search, discovers cliques, or
adds learned clauses.

## Optional source-derived plans

`PreparedFormula::ground_with_count_plan` and its bundle counterpart expose the
[closed frontend boundary](../verification/source-count-plans-20260908/README.md).
They return the unchanged original admission and one of `NotRequested`,
`NoPlan`, `Ready` or `Incomplete`. A ready plan retains shared original Theory
identity, original locations and a separately owned guarded restriction. There
is no frontend dependency on candidate search. A consumer can install that
restriction before proposal while checking every answer set against the original
program and its frozen reduct.

The initial profile uses ordinary choices and complete bijective count-head
groups with integer interval bounds. Element eligibility must be the literal
canonical true formula after coalescing; a domain fact is not accepted merely
because the program entails its truth. Both tuple/head alias directions are
validated before capture. Capacity groups must be unconditional or have exactly
the global activation; each emitted lower bound keeps that activation.

Greedy discovery follows retained grounding order and does not backtrack. For
`2{a;b;c;d}2.{a;b}1.{a;c}1.{b;d}1.`, selecting ab first misses the valid ac/bd
cover. `NoPlan` therefore describes the selected policy's result, not absence of
an applicable consequence. Optional stops preserve usable original admission;
no partial restriction is published. Limits and work/storage prefixes are
independent of source-grounding budgets.

This source profile does not infer capacities from ordinary pairwise exclusions
or recognize body-count totals. It consequently does not recognize queens02.
The manual corpus experiment below remains manually supplied evidence. No
ordinary default, runtime benefit or native aggregate-evaluation acceleration
is established by adding this opt-in source consumer.

## Qualification and experiment

The [finite controls](../../crates/zetesis-sat/tests/partition.rs) enumerate
interpretations independently of native candidate search. They compare the
stated mathematical premises with the derived lower bounds and compare emitted
truth with independently counted groups. Exact resource ceilings, cancellation,
empty groups and malformed partitions receive separate controls. The
[composition controls](../../crates/zetesis-sat/tests/partition_search.rs) compare
native enumeration with exhaustive Ferraris subset checking, and demonstrate
that a candidate bound cannot supply support for an unfounded positive cycle.
They also retain a counterexample to treating an inactive conditional cap as an
unconditional premise.

The [manual corpus experiment](../../crates/zetesis-sat/tests/partition_corpus.rs)
supplies row and column partitions explicitly for all six unchanged clean queens
encodings. This test fixture recognizes its own declared subject; the production
planner contains no predicate or filename recognition. Before applying bounds,
the experiment exhausts every original classical candidate, obtains exact tight
membership for each, and checks the supplied total/capacity premises over that
complete family. It then compares complete full interpretations after applying
the restrictions. This expensive baseline validates the experiment's descriptors;
it is not a proposed automatic planning algorithm.

The retained untimed manual experiment found:

| Encoding | Original search work | Restricted search work | Original decisions | Restricted decisions |
| --- | ---: | ---: | ---: | ---: |
| 01 | 1,232,380 | 1,497,045 | 401 | 401 |
| 02 | 22,930,544 | 3,062,078 | 17,660 | 400 |
| 03 | 1,129,451 | 1,390,001 | 401 | 401 |
| 04 | 985,794 | 1,214,297 | 395 | 395 |
| 05 | 1,504,620 | 1,744,893 | 391 | 391 |
| 06 | 1,704,842 | 1,949,780 | 391 | 391 |

All twelve enumerations complete with the same 92 full models per original
encoding; cross-encoding hidden interpretations are not identified. Search work
includes encoding, installed restrictions, certification and search. Additional
planner work is 576 operations for 01–04 and 832 for 05–06; separate restriction
emission costs 2,480 operations each. Every case retains 1,536 logical plan-vector
bytes and emits 416 Boolean nodes with 16 roots. These figures are not timing,
hardware instructions, process RSS, or the total memory cost of the added CNF.

Encoding 02 supplies the intended work-reduction evidence. The other five expose
the cost of redundant bounds: decisions do not fall and search work increases.
This experiment therefore supports selective applicability, not a default rewrite
or a claim that ordinary solving is already faster. The narrow source bridge
above does not supply these queens premises; broader source recognition and a
qualified selection policy remain necessary before their automatic use.

Run the deterministic diagnostic with:

```sh
cargo test --locked -p zetesis-sat --test partition_corpus -- --ignored --nocapture
```

The retained tranche record records source identity, complete output and gates.
No elapsed value printed by the test harness is accepted as a benchmark.
