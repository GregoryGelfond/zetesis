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

## A proved normal producer

[NormalSupport](../Zetesis/NormalSupport.lean) supplies a concrete producer over
the existing mathematical normalized rules. A rule proposes its atomic head
when its ground filter holds and its positive body belongs to the carrier.
It ignores candidate gates. A constraint proposes no head.

`projection_compatible` proves the required property:

1. Suppose `M` models the program and `C` is closed under possible heads.
2. If a frozen rule's body holds in `M ∩ C`, its positive body holds in both
   `M` and `C`. Original satisfaction puts the head in `M`; producer closure
   puts it in `C`. Thus the head belongs to `M ∩ C`.
3. Removing positive atoms preserves constraints with the same frozen gates.
4. `NormalFerraris.models_frozen_translate` transfers that normalized reduct
   model to the formula reduct.

`projection_compatible_of_coverage` extends the result to any producer containing
every normal proposal at every carrier. Extra possible heads do not invalidate
coverage. `answer_set_inside_closed` then applies reduct minimality.
The existing `NormalFerraris.ferraris_answer_set_iff_closure` separately gives
the exact least-closure-and-constraints membership test. These results share
the normalized-rule semantics; they do not replace the general formula reduct.

## Source producer correspondence

| Producer | Established mathematical part | Remaining implementation obligation |
| --- | --- | --- |
| Normalized atomic heads and constraints | `NormalSupport` proves projection compatibility, including producers with extra proposals | Rust must supply every relevant typed ground instance and preserve checked filters, atom identity and completion |
| Source normal rules with joins and generated values | The normalized-rule result applies after faithful instantiation | Prove complete whole bindings and generated head values; source classification or a completed loop alone does not establish this |
| Disjunctive and choice heads | Existing head/support laws describe necessary producers and head permissions | Connect local eligibility, every positive head occurrence and the original formula reduct to possible-support projection |
| Aggregate assignments and measured heads | `AggregateAssignment`, `ValueExtrema` and `SourceMeasures` supply key/value coverage laws | Preserve correlated keys/values, checked arithmetic and independent head permissions through concrete joins and lowering |

The first row is a theorem over the mathematical representation. The other rows
identify the source-to-representation work still needed. In particular, a
dependency projection's class verdict cannot certify omitted source semantics.

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
