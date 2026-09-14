# One network for all frozen reducts

`Zetesis.ParametricReduct` describes a fixed connective network with two inputs:
original truth in a candidate M, and atom membership in a prospective smaller
interpretation N. It preserves the existing Ferraris reduct. It does not replace
that reduct with the original classical formula plus candidate assumptions.

For an atom a, the network reads N(a). Bottom stays false; conjunction and
disjunction compose their child values. An implication F → G reads

```text
original_truth_M(F → G) ∧ (network_N(F) → network_N(G)).
```

The first theorem, `satisfies_original`, says a true network value is true in M,
provided N ⊆ M and the original-truth parameters are accurate. Atoms use subset
containment, conjunction/disjunction use their child results, and implication
uses its explicit guard. In particular, a formula false in M cannot become true
in this network.

The second theorem, `satisfies_reduct`, proves equality with frozen-reduct truth
by induction on syntax. Atoms outside M are absent from N. True conjunctions and
disjunctions have the same reduct children; a false compound is false by the
first theorem. Implication retains its original-truth guard, so its false case
is bottom and its true case has the same reduct children. This explains why only
implication nodes need explicit original-truth parameters.

The third theorem, `countermodel_iff`, lifts that equality to all asserted roots
and proper subsets. Original satisfaction of M remains a separate obligation
before answer-set membership can follow from absence of a countermodel.

The Rust correspondence is `PreparedReduct` in `zetesis-sat`. It retains one CNF
and exact immutable theory owner. Original atom slots represent N; separate
inputs represent M and original implication truth. It asserts N ⊆ M, at least
one M atom absent from N, and every network root. A successful
`EvaluationWorkspace` evaluation authenticates the original-truth parameters for
the exact candidate and theory. Before each subset search, a worker replaces
assumptions and clears its candidate-dependent assignment, decision and ordering
state. It retains a completed watch index and unconditional unit-propagation
prefix for the exact unchanged owner. Every prefix assignment follows from the
encoded query before candidate parameters. A false prefix watch remains blocked
by a true prefix watch after the candidate suffix is undone. Candidate parameters
are supplied afresh; no candidate-derived truth enters the retained prefix.
An incomplete index must be rebuilt; interruption during later propagation may
preserve it only if each completed watch update leaves its links consistent.
An unfinished unconditional prefix must be undone and recomputed. Only an
unconditional conflict can refute later parameterizations without search.
Returned subsets are checked independently against the original frozen reduct.

`Propagation.unconditional_sweeps_models_iff` gives the domain-level preservation
argument. Any model of the unchanged query fits its unconditional narrowing, so
intersecting that narrowing with candidate restrictions preserves precisely the
restricted models. This law does not establish the concrete unit-propagation
schedule or its watch invariants.

For A atoms, N DAG nodes, I implication nodes and R roots, conservative submitted
bounds are 3A + N + 2I variables, 3N + 3I + 4A + R + 1 clauses, and
7N + 7I + 10A + R literal occurrences. Simplification and repeated gate aliases
may reduce actual dimensions. Preparation has separate cold work and storage
receipts. Query parameter work is included in SAT work; original evaluation and
returned-witness verification retain their independent per-call ceilings.

The Lean laws concern formulas and interpretations. They do not prove Rust DAG
indices, checked dimensions, Tseitin clauses, assumption/backtracking behavior,
watch-index retention, owner authentication, allocation, quotas or the SAT
implementation. Those remain
representation/execution obligations, with differential and failure controls.
The fixed network may be larger and propagate more slowly than a freshly
simplified reduct; reuse alone proves no performance gain.
