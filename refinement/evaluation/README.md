# Refining formula evaluation

This separately built implementation-refinement package connects the generated
private `FrozenReduct::satisfied_by` query to Ferraris reduct satisfaction under
fixed observation tokens. It composes the generated evaluator and root scan,
with explicit stored-mask agreement and shared work accounting.

The reusable ASP theory lives in [`proofs`](../../proofs/README.md). This package
imports the reduct-evaluation and packed-subset sources directly and checks them
with the extraction backend's Lean 4.31.0. The main library retains Lean 4.33.1.
No semantic definitions are copied or replaced; object files from different Lean
versions are not mixed. `semantic-inputs.sha256` identifies the shared sources.

## Central result

[`FrozenQuery.completed_satisfaction`](FrozenQuery.lean) proves that a completed
actual private query returns `true` exactly when the tested interpretation `J`
models the asserted theory's Ferraris reduct frozen at `M`.

Its explicit `Represents` premise says the stored mask is original truth for the
stored candidate and theory. `represents_from_evaluation` derives that agreement
for the corresponding record from an actual completed original evaluation;
it does not prove the `freeze` constructor. Other premises require represented
packed input, ordered child indices and bounded roots. The single query result
determines its evaluation and root-scan calls; neither is assumed correct.

The query passes evaluation's returned work into the root scan. It preserves
limits and subset statistics and charges at most `N + R` for `N` nodes and `R`
root occurrences, including typed stops. Neither original modelhood of `M` nor
`J ⊆ M` is required; this is satisfaction, not answer-set membership.
`TheorySatisfaction` retains the separate-call original and reduct results.

## Argument

| Module | Established boundary |
| --- | --- |
| `Membership` | The extracted packed query returns its declared atom's bit |
| `Iteration` | Actual slice/vector operations respect finite bounds |
| `Control` | Poll precedence and bounded work increments |
| `Step`, `Progress` | Exact node value, masked append and position alignment |
| `Setup` | Actual entry operations produce a zero cursor and empty output |
| `Specification` | A finite truth fold and its prefix laws |
| `Trace` | Actual step traces preserve exact prefixes and work counts |
| `FixedLoop` | The generated loop and entry function return trace-correct outcomes under fixed tokens |
| `EvaluationAccounting` | Every actual typed return preserves limits and subset statistics and charges its returned prefix length |
| `Semantics` | Concrete folds equal the existing original/reduct folds |
| `ReductTrace`, `FixedReduct` | Two completed traces, or two successful generated calls, establish per-node reduct satisfaction |
| `RootScan` | The actual root scan returns the first false occurrence or complete success, with exact work and typed stops |
| `RootSemantics`, `TheorySatisfaction` | Generated evaluation and root scanning decide original or reduct theory satisfaction on completion |
| `FrozenQuery` | The actual private query decides the represented reduct and threads one work record through both phases |
| `SubsetQuery` | One actual subset query decides reduct satisfaction from a computed original mask and charges exactly one subset on completion |

All five node forms retain the source's Boolean short circuits. A stop precedes
node evaluation and append, although the iterator has already fetched the node.
An exhausted iterator completes without polling or charging work.

`FixedLoop` constructs the calls made by the generated loop, passing each
returned control value to the next call. It proves termination with the exact
truth prefix and work count, preserving limits and the subset counter. Successful
exhaustion returns the full value sequence; a typed stop returns a strictly
shorter correct prefix. Termination may therefore be a refusal, not successful
evaluation.

`RootScan` preserves stored root order and duplicate occurrences. It returns the
first failing node identifier, charging each tested occurrence including the
false one. A refused tick charges no next-root test and remains a typed stop.
Empty roots complete without polling or work. The checked
[examples](RootScanExample.lean) cover empty roots despite cancellation and a
repeated-root sequence whose first false identifier is not the smallest one.
[Query examples](FrozenQueryExample.lean) cover cumulative work refusal, a valid
mask for a nonmodel candidate, and a successful query at a non-subset.

## Subset-search foundations

[`SelectedAtoms`](SelectedAtoms.lean) composes the backend's range and vector
operations with the generated packed membership query. An exact selected prefix
stays exact after the next coordinate; it is ordered and has no duplicates.
At the end of the universe, it agrees with the ASP library's `selectedAtoms`.
The remaining coordinate supplies room for both the checked successor and push.

[`ScalarSubsets`](ScalarSubsets.lean) connects checked division, remainder,
shift and mutable word indexing to the shared packed set/clear laws. Only the
selected bit changes. Population increment requires a representable selection
width; decrement requires a currently true positional bit. The
[boundary examples](ScalarSubsetsExample.lean) set and clear atom 64 while
preserving its neighbors in both words.

The Rust reference checker names atom selection, subset advancement, one reduct
query and proper-subset search as private operations. Allocation remains in
`check`; `find_countermodel` owns the proper-subset guard and retains the first
witness or typed stop. All four operations are extracted unchanged.
[`SubsetSteps`](SubsetSteps.lean) proves that their generated bodies finish an
exhausted iterator without polling and preserve the returned state and work on
a typed refusal. A present coordinate is fetched before the tick; a refused
step performs no membership read or bit update.

[`SubsetQuery.completed_reduct`](SubsetQuery.lean) proves that an actual completed
`check_subset` returns true exactly when the tested interpretation satisfies the
Ferraris reduct. The mask comes from an actual successful original evaluation;
its meaning is derived, not assumed. Separate laws recover the internal calls,
establish admission and prove the completed query charges exactly one subset.
They do not establish properness or coverage of the search.

The primitive bridges do not yet prove successful helper iteration or complete
membership. The search loop retains its result and exits before the caller
propagates it. This removes the early-return translation obstacle without
changing enumeration or resource checks. The
[reproduction guide](REPRODUCING.md#current-subset-search-extraction-limit)
records the remaining allocation and ownership boundary. The carry correspondence must use the outer proper-subset
guard: calling the Rust carry on a full selection would clear it, whereas the
mathematical counter reports overflow.

## Observation boundary

The atomic external model returns the Boolean stored in its supplied token.
The body reads cancellation at most once and optionally expiry at most once.
A single invocation can use fresh observations; `FixedLoop` follows the actual
generated loop's control threading, so repeated reads return fixed values.
Only the Relaxed loads used here are modeled; other model orderings do not
assert which loads are valid in Rust.

`Trace` is broader: each body call may receive a separately supplied control
value. Fixed-loop calls form such a trace; refreshed-control traces need not be
executions of that loop. A checked [example](FixedLoopExample.lean) stops after
one node and work unit when cancellation is refreshed, while the fixed-clear
loop completes two nodes and charges two units. The pinned result effect has no
changing external-read event. Correspondence with varying Rust atomic observations,
shared-object identity and concurrent histories remains open.

## Representation and trust

Both passes use the same stored table and numeric atom vocabulary. The Arc
value model does not prove Rust pointer identity or owner checks. Aeneas's vector
model records logical elements and checked indices, not allocation capacity or
allocation failure. Unused clock and synchronization fields have tokens but no
modeled operations. Reference counting, destruction, timers, concurrent memory,
machine code and GPU execution remain outside this model.

The Rust compiler, Charon and Aeneas translations, and the correspondence of
library models to Rust, remain trusted boundaries. There are no project axioms,
proof holes or native proof-evaluation shortcuts. The package does not establish
`FrozenReduct` construction, its public allocation and owner-checking wrappers,
subset search, source grounding, or end-to-end solver verification.

## Extraction identity and reproduction

The generated types and functions come directly from production Rust, including
the four private subset-search operations. The LLBC destination becomes portable,
and local names change from `theory` to `program` to avoid namespace
collisions; operands retain their local IDs. An unused derived `Debug`
implementation whose formatting method was excluded is removed with its
ordered registration, after checking for surviving semantic references.
Two unused range-trait method registrations are omitted from both the trait and
its implementation to match the pinned backend model, with indices preserved.
The selected operations do not refer to either method. Restoring these selections
and metadata/name fields recovers the parsed raw input. No executable body is
rewritten; regenerated Lean is retained unedited.

These are audited preprocessing steps, not verified transformations or a claim
of byte-identical raw extraction. `provenance.json` and the source/artifact
inventories identify the exact boundary. The historical
[membership package](../membership/README.md) remains unchanged; its packed-query
argument is instantiated here against the current extraction's types.

The [reproduction guide](REPRODUCING.md) gives pinned setup, strict checks and
regeneration commands. `Audit.lean` covers all authored package theorems;
`SharedAudit.lean` covers the imported semantic declarations. The records qualify
only the hashed sources. This separately built package remains outside the main
proof library's declaration count. No compiler, downloaded dependency or cache is
vendored.
