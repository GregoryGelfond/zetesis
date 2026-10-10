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
Resolved contribution tuples contain borrowed term references. A distinct key
retains that tuple buffer directly; a duplicate discards it. Neither operation
copies the referenced logical payload.

After source admission, formula sessions reuse the same immutable
[`ObjectivePlan`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/objective_bound.rs)
for score reads and optional incumbent constraints. Preparation combines every
alternative for one normalized `(priority, weight, tuple)` key with OR. Each
`ObjectivePlan::score` evaluates the retained eligibility DAG on an
interpretation of that exact original theory, then calls the typed
`zetesis_objective::reduce_costs` reduction. It retains inactive priority slots,
objective presence and signed totals, without rebuilding joins or tuple keys.
The caller must still establish complete catalog coverage and answer-set
membership. Equal-looking theories are distinct owners.

The ordinary `evaluate` API remains the contribution-evidence path. Prepared
scoring also declines to it when possible-population binding, key or byte counts
cannot establish the requested per-model ceilings. That decline spends no work;
other typed failures retain their actual prefix instead of silently retrying.
Optional pruning has separate mutable status: disabling it, or refusing an
incumbent constraint, leaves scoring preparation available. Enumerating all
answers also reuses preparation while leaving pruning disabled.

For region search, each improved incumbent replaces the previous optimizer
bound through `StableModels::tighten_candidate_bound`. Independent caller
restrictions remain. The caller must establish that the original theory and new
bound together imply the previous bound; the API does not check that implication.
Active workers may finish
under an older bound, so returned answers are still scored against the current
incumbent. The original theory and reduct remain unchanged.
[`ObjectiveBounds.replacement_preserves_candidates_relative`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ObjectiveBounds.lean)
states the preservation law; it does not verify worker synchronization.

Source admission also retains checked required-choice groups when the program
has objectives. These are complete positive head groups with a positive numeric
lower bound; the group's body remains its activation condition. Conditional
element eligibility still guarantees a true head in an active group. It does
not justify an upper bound on all true heads, so the separate count-partition
consumer continues to require unconditional element eligibility. Eager and hybrid admission use the same capture.
`RequiredChoices` belongs to the exact admitted theory. Capture has its own
bounded optional work and storage account, reported by `capture_statistics`
and `capture_failure`; it does not consume the mandatory grounding allowance.
Failure leaves original admission and exact objective evaluation intact.

For an optimal-selection session, `ObjectivePlan::prepare_choice_bounds` can
prove that each member of such a group implies an objective contribution. It
uses original normal-rule implications, facts, conjunctions and disjunctions;
a cycle or unsupported implication supplies no proof. Complete objective keys
are already coalesced, and selected groups never share a key. At a priority whose
contributions all have nonnegative weights, the bound prepays an active group's
least allocated weight and retains each eligible key's excess over that minimum.
Unallocated contributions keep their exact weights and conditions. Priorities
containing negative weights retain the exact bound without this additional
grouping.

This lower cost gives a necessary candidate condition alongside the exact
incumbent bound. It cannot replace the original theory, its reduct or exact
scoring. Equal-cost answers remain eligible. Preparation uses checked vector
capacities under the candidate-memory allowance, with source premises counted
once; this account excludes unrelated owners and allocator bookkeeping.
The fixed activation closure is prepared once in the existing n-ary formula
representation. Preparation also compiles each changed priority's residual
weights and releases the temporary member-to-key allocation lists. Unchanged
priorities borrow the exact objective; the retained lower objective contains
only activation floors and nonzero residual contributions. Both inputs use the
same checked priority ordering, weight normalization and lexicographic compiler.
Each improved incumbent first constructs its exact bound. Optional strengthening
copies that bound and the compact activation closure; it does not rescan unrelated
original formulas or recompute the allocated minima. Exact score evaluation is
unchanged. An optional bridge or bound failure leaves the ordinary exact bound
available.
After an improved incumbent, that fallback still implies the preceding bound
on original models: the required-choice lower cost never exceeds their exact
cost. Interpretations violating the original theory need not obey that
implication. Bound replacement rebuilds its propagation knowledge; previously
pruned regions need not reopen.
The [required-choice cost laws](../lean/theorems.md) state the partitioned-cost
and lexicographic preservation arguments; correspondence of the concrete source
capture and implication search remains a separate proof obligation.

`SolveConfig::max_objective_work` counts one preparation attempt plus every
score read or detailed fallback, including refused prefixes. Preparation is
capped by both the remaining objective allowance and the standalone plan's
10-million-unit default. Ordinary solving sets the cumulative allowance to
`u64::MAX`; Rust callers can supply a smaller explicit limit. Preparation can
consume that limit; detailed fallback remains subject to the actual
remainder and cannot report a score or complete optimum after exhaustion.
Zero skips optional preparation and preserves the mandatory evaluator's zero-work
refusal. `max_objective_bound_work` counts optional constraint generation alone;
zero disables pruning without disabling scoring preparation. These are logical
operation accounts, so numerical cutoffs can differ from earlier scoring routes.

Admission consumes the owned template descriptions. `ObjectiveTemplateRef`
borrows weight and tuple terms, positive patterns and filters from the shared
core `TemplateCatalog` representation. Priorities remain a fixed distinct
layout, with each original template occurrence naming its slot. This storage is
independent of rule topology and supplies no logical support.

Closed conditions retain ID-only operations. Owned condition atoms share the
objective's canonical vocabulary and one published tuple catalog; an already
canonical condition retains its supplied source catalog. `AdmissionLimits::max_bytes`
bounds the composed canonical payload and metadata, including admission scratch
and publication overlap. `max_condition_node_bytes` separately bounds each
condition's node metadata. Exact shared owners count once; independent prefixes
may conservatively recount shared segments. Caller construction descriptions
and allocator bookkeeping are excluded. These are admission contracts, not a
claim that generated source bindings and displayed symbols share one owner.

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
JSON. `AtomSelection::signatures()` returns a borrowed `Signatures` view of
sorted, unique signed predicates. Use `.iter()` to obtain `PredicateRef` values,
`.at(position)` for indexed access, or `includes` for atom membership. The view
replaces the former owned-predicate slice; it does not expose canonical IDs as
ordering keys. Name, arity and sign all participate in identity. Explicitly empty
selection differs from implicit all-atom selection, and neither changes the
underlying answer set.

`SourceMetadata::directives()` similarly returns a borrowed `Directives` view;
its `.iter()` and `.at(position)` yield located occurrences, including authored
duplicates. Directives, output and projection signatures, and observation
constants and constructor names share one immutable metadata vocabulary. Their
public comparisons use typed contents across independently compiled owners.
`ObservationProgram` retains that vocabulary with its query topology; evaluation
can read any supplied full model without requiring the model to share it.

`MetadataStorageLimits::max_bytes` defaults to 64 MiB and bounds the canonical
vocabulary and component capacities, including their named construction and
publication overlap. `FormulaLimits::metadata_storage`, `MetadataLimits::storage`
and `AtomSelectionLimits::storage` configure their respective admission doors.
Source topology, compiler maps and temporary coordinate vectors retain separate
logical admission policies. This allowance excludes allocator bookkeeping and
process RSS; storage refusal publishes no partial metadata policy.

`includes` and `try_includes` use the same binary lookup. The latter accepts a
fallible work callback and charges before each comparison: one unit plus both
predicate-name byte lengths. For `S > 0` signatures, a lookup performs at most
`floor(log2(S)) + 1` comparisons; implicit all-atom and explicit empty selection
need none. Human and JSON encoders apply their own budgets through this callback.
Interrupted lookup returns no selection decision and leaves the policy intact.
This bounds lookup work without a separate index or duplicated signature store.

Repeated answers over one vocabulary need not repeat that lookup per atom.
`OutputSelection::prepare_with(read, charge)` decides every predicate of the
catalog read once, charging each decision exactly as `try_includes` charges an
atom of that predicate, and returns a `PreparedSelection`. It retains one bit
per predicate; implicit and explicit-empty selections decide nothing. A prepared
decision charges one unit per atom; any atom outside the prepared vocabulary
prefix is searched with the usual charges, so the prepared selection answers
exactly as the policy does. `ObservationProgram::view_prepared` builds views
over it, and `observation::prepare_selection` prepares within one observation's
`max_work`, keeping every atom's search when that ceiling would be exceeded.
The command line prepares at the first answer of a run and reuses the result
for the rest, so its human and JSON output work falls by the saved comparisons.
A stopped preparation leaves the selection unprepared; the stop is then reported
in the same publication phase as without preparation.
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

For `A` model atoms and `K` relational alternatives, the row view borrows the
model directly, and each query retains `K` ranges. Range preparation takes
`O(K (1 + log(A + 1)))` work apart from predicate-name comparisons, without an
`A`-element reference array. Nested queries prepare their ranges when entered.
Tuple matching examines the product of the relevant relation sizes, rather than the
whole-model product. Cartesian products within those relations remain possible.
Bindings, constructed terms and output sorting incur their own measured work.
These bounds describe operations and storage; they are not a wall-time claim.
The implementation is in
[`observation::evaluate::rows`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/observation/evaluate/rows.rs)
and the [query cursor](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/observation/evaluate/query.rs).

Observation limits separately bound work, completed bindings, constructed
symbols, logical local payload and output. Captured input terms stay borrowed;
generated values use scoped IDs in one per-operation derived arena. The local
byte budget still counts semantic nodes and text, independently of physical
sharing, and releases its charge when a local query or key scope ends.

`Limits::max_term_storage_bytes` defaults to 64 MiB. It bounds the combined named
capacity of the derived arena and retained typed wildcard-key graph, including
replacement overlap; it excludes borrowed inputs. Transient ID frames each use
this ceiling independently, so their combined capacities are not part of this
receipt. `Statistics::term_storage_bytes` and `peak_term_storage_bytes` report
the named arena-and-key account. Logical local and construction allowances
remain independent. Neither account includes allocator overhead or measures RSS.
Reached undefined arithmetic, cancellation or a resource refusal returns an
error without a partial observation. An observation error does not invalidate
an already verified answer set or complete world view.
Errors retain their typed cause, original statement site and partial statistics.
`site()` preserves the logical subject for resolution through its formula owner;
`location()` returns a source coordinate only when one exists. The
fixed diagnostic box is allocated only on refusal and follows the standard
allocator's failure policy; these limits do not promise recovery from every
process allocation failure.

The [Lean correspondence](../lean/correspondence.md) distinguishes the laws for
original-model conditions and contribution transport from the remaining source,
Rust and device refinement obligations.
