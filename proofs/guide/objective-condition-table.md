# Reading the objective condition table correspondence

[ObjectiveConditionTable](../Zetesis/ObjectiveConditionTable.lean) connects a
shared node table to the already defined closed
[ObjectiveConditions.Query](../Zetesis/ObjectiveConditions.lean). It concerns
Boolean observations of an original interpretation. It does not change the
Ferraris theory or establish which source objective rows should exist.

Each node is a Boolean constant, an atom, a negation of an earlier node, or the
conjunction/disjunction of two earlier nodes. Indices start at zero. For example,
the table `atom a; neg 0; disj 0 1` shares the first node and unfolds to
`a or not a`. Sharing does not change the query's original truth.

`Backward length node` requires every operand to be below the length of the
completed prefix. `AdmittedFrom` checks that condition in source order, growing
the prefix length after each node; `Admitted` starts at zero. These are explicit
mathematical admission premises, not an assertion about the Rust validator.

The two traversals keep the same indices. `unfoldRows` appends a closed query;
`evaluateRows` appends its Boolean value. A missing operand makes either traversal
return `none`. The final result uses the last node, or true for an empty table.
Every table node is visited, including nodes not referenced by the last one.

The proof has three main steps:

1. `node_correspondence` proves that one step preserves the prefix invariant:
   evaluating a mapped query prefix gives the same result as unfolding the next
   node and evaluating that query. Each operand lookup refers to the same index.
2. `rows_correspondence` inducts over the unvisited suffix. A successful step
   appends the corresponding query/value pair; a failed lookup produces the same
   failure on both sides. `admitted_rows_total` separately proves that backward
   references always succeed, with one result per node.
3. `unfolding_correspondence` selects the corresponding final entries, handling
   the empty-table default. `admitted_correspondence` combines existence with
   this equality. `original_truth` then applies the existing query-to-formula
   law to obtain original Ferraris satisfaction.

`model_identity` requires atomwise equal interpretations. A displayed model that
omits true atoms does not generally meet that condition. These queries do not
use a frozen reduct to select an objective contribution.

The result narrows the representation gap between the node table and the
unfolded Boolean query. Rust indexing and atom conversion, source binding and
eligibility, priority presence, checked arithmetic, allocation, work limits and
cancellation remain separate implementation obligations. Unfolding is a
mathematical specification and may duplicate shared subqueries; it is not a
runtime algorithm or a performance claim.

For a scoped weak body, pooled conditional consequents use the same runtime
formula as rule-body conditionals. In
`d(1;2). {p(1);p(2);p(3)}. :~p(X;X+1):d(X).[1]`, the objective condition is
`(p(1) or p(2)) and (p(2) or p(3))`. The family containing only `p(2)` therefore
incurs cost one. [ConsequentAlternatives.original_semantics](../Zetesis/ConsequentAlternatives.lean)
and [ProjectedConditionals.original_semantics](../Zetesis/ProjectedConditionals.lean)
state the universal-row/existential-alternative order for supplied complete
families. `ObjectiveConditions.formula_query_truth` connects the resulting
formula's original truth to its Boolean query.

The separate Rust dependency-analysis projection replaces each pooled consequent
with several pool-free occurrences so established analysis can inspect their
signatures and polarities. Its conjunction would require all three `p` atoms in
the example and is deliberately not used as an equivalent objective condition.
`AnalysisBasis::DependencyProjection` exposes that distinction. The shared
projection preserves each weak statement's complete weight, priority and tuple,
while runtime compilation reads the original scoped body. Signature coverage,
source provenance, the independent pre-clone bounds and Rust representation
correspondence remain implementation obligations; these semantic laws do not
certify the analysis transformation.
