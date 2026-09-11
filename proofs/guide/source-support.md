# From completed support to answer-set coverage

[SourceSupport](../Zetesis/SourceSupport.lean) separates two questions: did finite
source generation finish, and does its result cover every atom in an answer set?
A possible atom need not occur in any answer. A proposed aggregate value need
not be realized with the other proposed values.

Let `propose(C)` contain the positive head atoms obtainable from the possible
carrier `C`. A round retains old atoms and adds proposals:

```text
round(C) = C ∪ propose(C)
Closed(C) means propose(C) ⊆ C
```

`unchanged_round_closed` proves that a complete round adding no new atom yields
closure. `stages_inside_closed` assumes monotone proposal generation and proves,
by induction, that every finite generation stage lies inside a closed carrier.
Neither result proves that generation terminates or that a stopped prefix is
complete.

## The reduct premise

Closure alone does not establish answer-set coverage. `ProjectionCompatible`
requires that, for every closed carrier `C` and original model `M`, the
intersection `M ∩ C` satisfies the same frozen reduct `T^M`.
`stable_inside_closed` then proves `M ⊆ C` for an answer set `M`:

1. `M ∩ C` is a subset of `M`.
2. The projection premise makes that intersection a model of `T^M`.
3. Minimality of `M` as a reduct model requires every atom of `M` to remain in
   the intersection. Hence all of `M` belongs to `C`.

The source adapter must justify the premise through complete typed rule
instances, local bindings and aggregate-value proposals. Positive joins retain
whole bindings; nonbinding aggregate and conditional truth does not prune
possible producers. Aggregate assignments propose every covered value while
retaining their original equalities in the formula theory. Head permissions
remain independent of measured contributions. The existing aggregate coverage
laws are ingredients of this argument, not a verification of the Rust compiler.

## The completed owner and its consumers

The Rust [support builder](../../crates/zetesis-themelios/src/formula_support.rs)
creates `CompletedCatalog` only after an entire support round adds no atom.
Its `CompletedSupport` view borrows the authoritative catalog; intermediate
snapshots have only the `Support` type. Objective preparation and final formula
grounding use the same completed relation view. A work, round, value or storage
failure returns an error before an incomplete objective program can be supplied.
Recursive value generation may continue indefinitely unless stopped by a limit.

`completed_activity_covers` applies the coverage result to objective activity:
covered atoms are optional, and an atom outside the carrier is absent. Optional
retains both truth possibilities; it does not prove simultaneous realizability
or a particular grounder's priority layout. The actual objective query still
reads the original model. Weight, priority and complete tuple come from one
binding, and equal normalized keys contribute once.

[Rich cyclic objective contracts](../../crates/zetesis-themelios/tests/objective_rich_cycles.rs)
retain complete scored families, typed values, source-order cases, independent
carrier refinements and bounded failures. Versioned reference records identify
zero-cost priority slots that clingo omits. Those metadata differences are
separate from answer-set identity and optimum ties.

The Lean laws state the semantic assumptions explicitly. Source safety,
proposal completeness, checked arithmetic, concrete resource accounting and
Rust execution remain refinement obligations. Observing an empty delta cannot
replace the reduct-projection premise.
