# Costs and shown terms

Answer-set membership, objective costs and output observations answer different
questions. Membership uses the original theory and its frozen reduct. An
objective reads the complete interpretation to compute a score. An observation
reads that same interpretation to construct displayed terms. Neither operation
supplies support for an atom.

The example chooses one of two sites. Choosing east costs three units plus a
two-unit penalty for not choosing west. Output adds two to the site's input
value for display; this arithmetic does not alter the objective or introduce
new atoms. Complete collection retains both answers, including the more costly
one.

```rust
# extern crate zetesis_solve;
# extern crate zetesis_core;
# extern crate zetesis_cpu;
# extern crate zetesis_themelios;
{{#include ../examples/costs-and-output.rs:example}}
```

Use [optimal selection](outcomes.md#check-selection-and-complete-capture) when
only optimum ties are wanted. A score attached to one answer does not prove
optimality; that conclusion also requires completed search. Identical displayed
terms do not merge distinct answers.

## Library boundaries

`zetesis-objective::ObjectiveTemplate` describes an objective contribution.
Positive relational bindings and scalar filters can be combined with a closed
`Condition` over complete typed atoms. `ObjectiveProgram::new` checks the
condition's acyclic references and admission limits. `evaluate` reads the
supplied model; the caller establishes whether that model is an answer set.
It uses one contribution-key and priority-ordering contract for all templates.

At the source boundary, admitted scoped weak constraints use the existing body
compiler and aggregate operations. Their temporary formulas become closed
queries over typed atoms; they never become program roots. Source body storage,
retained query storage and cumulative preparation work have separate limits.
The [scoped example](../reference/language.md#objectives-and-observations)
combines a count head, aggregate assignment, conditional cost and shown count.

Source priority presence is a separate preparation result. A retained priority
may have cost zero in every answer. In particular, a condition's false value
on the current answer is not permission to remove its priority slot. The
[language reference](../reference/language.md) states the admitted finite
source profiles and their remaining boundaries.

An always-zero slot can differ from clingo's reported vector without changing
objective ordering. For example, `a.a|b.#minimize{1:b}.` has the sole answer
`{a}`. zetesis retains priority 0 with cost 0; clingo 5.8.2 reports no cost slot.
Replacing a fact by an equivalent derivation can change clingo's retained slots.
Source activity therefore is not a certificate of an identical clingo display.
Compare costs at identified priorities, distinguish an absent slot from a retained
zero in reports, and establish that any extra slot is zero across all answers
before treating the two rankings as equivalent.

[`ObjectivePriorities.zero_slot_comparison`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ObjectivePriorities.lean)
proves that inserting a zero slot at a fixed position preserves lexicographic
comparison; `zero_slot_optima` preserves every optimum tie of the unchanged
theory. Neither law proves that a particular source row is inactive. Source
coverage and condition evaluation must establish that premise separately.

`ObservationProgram::evaluate` returns typed shown symbols;
`ObservationProgram::render` composes these with the selected atom channel.
Both preserve complete model identity. A term and an atom with the same printed
text remain separate output contributions. Consumers may retain typed values
instead of rendering them.

Atom-channel selection has one implementation for library views, human text and
JSON. `AtomSelection::signatures()` borrows a sorted, unique slice of signed
predicate signatures. This replaces its previous `BTreeSet` return type; callers
can iterate the slice or use `includes` for membership. Name, arity and sign all
participate in identity. Explicitly empty selection differs from implicit all-atom
selection, and neither changes the underlying answer set.

`includes` and `try_includes` use the same binary lookup. The latter accepts a
fallible work callback and charges before each comparison: one unit plus both
predicate-name byte lengths. For `S > 0` signatures, a lookup performs at most
`floor(log2(S)) + 1` comparisons; implicit all-atom and explicit empty selection
need none. Human and JSON encoders apply their own budgets through this callback.
Interrupted lookup returns no selection decision and leaves the policy intact.
This bounds lookup work without a separate index or duplicated signature store.
Source collection appends signature occurrences to a private builder, then sorts
and deduplicates once before publishing `SourceMetadata`. Constructed directives
affect the same selection without inventing source locations. For `S` signature
occurrences, selection construction requires `O(S log S)` comparisons and `O(S)`
cells. A partially collected policy is not exposed as a completed selection.

Term queries borrow the supplied model in canonical atom order. Equal signed
predicates form contiguous ranges, so two binary bounds locate a relation
without copying its atoms. Each query prepares its alternative ranges once and
reuses them across enclosing bindings. The cursor retains source-alternative
order and canonical row order; repeated variables still match one complete
tuple. Fixed and generated atom conditions use the same predicate bounds.

For `A` model atoms and `K` relational alternatives, preparation retains `A`
atom references and `K` ranges, with `O(A + K log A)` work apart from predicate
name comparisons. Nested queries prepare their ranges when entered. Tuple
matching examines the product of the relevant relation sizes, rather than the
whole-model product. Cartesian products within those relations remain possible.
Bindings, constructed terms and output sorting incur their own measured work.
These bounds describe operations and storage; they are not a wall-time claim.
The implementation is in
[`observation::evaluate::rows`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/observation/evaluate/rows.rs)
and the [query cursor](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/observation/evaluate/query.rs).

Observation limits separately bound work, completed bindings, constructed
symbols, local owned payload and output. The local byte budget counts semantic
nodes and text; it excludes borrowed model values and allocator overhead.
These are deterministic operation budgets, not process-memory limits. Reached
undefined arithmetic, cancellation or a resource refusal returns an error,
without a partial observation. An observation error does not invalidate an
already verified answer set or complete world view.

The [Lean correspondence](../lean/correspondence.md) distinguishes the laws for
original-model conditions and contribution transport from the remaining source,
Rust and device refinement obligations.
