# Refining formula evaluation

This separately built implementation-refinement package connects the CPU
reference checker's generated evaluation, root scan, atom selection and
proper-subset search to the Ferraris answer-set definition under fixed
observation tokens. The public allocation and owner-checking wrapper remains
outside the proved composition.

The reusable ASP theory lives in [`proofs`](../../proofs/README.md). This package
imports the reduct-evaluation and packed-subset sources directly and checks them
with the extraction backend's Lean 4.31.0. The main library retains Lean 4.33.1.
No semantic definitions are copied or replaced; object files from different Lean
versions are not mixed. `semantic-inputs.sha256` identifies the shared sources.

## Central result

[`MembershipSearch.completed_answer_set`](MembershipSearch.lean) proves that,
after actual original evaluation and a successful original root check, completed
actual atom selection and proper-subset search return no countermodel exactly
when the candidate is an answer set of the stored formula theory.

The premises require represented candidate storage, ordered child indices,
bounded roots, matching atom counts, an initially empty selection vector and
packed subset storage representing the empty interpretation. The theorem threads
work through these actual calls in source order. It derives the frozen mask's
meaning and the selected atoms' coverage; it does not assume a correct subset
oracle or complete enumeration. `FixedSearch.calls_refine` constructs the actual
search execution, including typed stops. A stopped search establishes no
membership verdict.

These premises describe the reference checker's semantic phases. They do not
prove that the public wrapper allocates the initial buffers, establishes their
owners or handles every setup failure correctly.

## Storage and ownership

[`PackedSetup`](PackedSetup.lean) proves that the backend's zero-resize operation
produces the exact empty packed interpretation needed by subset search. Its
`initialized_membership` theorem supplies that result to the membership proof.
Exact candidate word length remains a constructor premise: having enough words
for reads alone does not establish the required shape.

[`MembershipVerdicts`](MembershipVerdicts.lean) treats the negative branches.
An actual original-root failure excludes membership. A completed positive subset
search returns its own proper-subset reduct model; that witness excludes
membership independently of whether the original candidate was a model.

[`OwnerChecks`](OwnerChecks.lean) proves the generated owner check and clone
against explicit owner-token contracts. [`RuntimeOwnership`](RuntimeOwnership.lean)
requires views of immutable data to agree with one heap. Under that invariant,
a successful owner check supplies equality of the stored theory data. Equal
values in different owners still fail the check. Reference counting and the
correspondence of tokens to live allocations remain trusted library contracts.

[`OwnedMembership.completed`](OwnedMembership.lean) combines the actual owner
check and zero initialization with the membership phases. It derives agreement
of the atom universes and the initial empty subset instead of assuming them
separately. Admission and reservation remain outside this composition.

These results do not yet compose the entire public wrapper or prove the actual
admission constructors. They establish specific setup obligations without
assuming successful allocation or equating owner identity with value equality.

## Stored reduct queries

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
| `SubsetQuery`, `SubsetQueryTotal` | Actual subset queries decide reduct satisfaction on completion; every typed return retains the admission charge and shared work bounds |
| `FixedSelection` | The actual scan returns the exact ordered candidate atoms, or a stopped prefix with exact work |
| `SubsetCarry` | Actual proper carries preserve packed selection and population on completion, retaining partial state on stops |
| `SearchSteps`, `CountermodelTrace` | Exact generated search branches and their finite composition retain returned state and work |
| `SearchRepresentation`, `SearchSemantics` | Packed counter states denote semantic subsets and queries use the actually computed original mask |
| `FixedSearch`, `MembershipSearch` | The actual search covers proper subsets, and its completed result composes with original modelhood to decide answer-set membership |

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

## Proper-subset search

[`SelectedAtoms`](SelectedAtoms.lean) and [`ScalarSubsets`](ScalarSubsets.lean)
connect range advancement, vector append and checked word updates to the shared
selected-prefix and packed-counter laws. [`FixedSelection`](FixedSelection.lean)
uses those operations to prove the actual selection loop and entry function.
Every typed return retains the exact visited prefix and its work charge; a
completed scan gives the candidate's distinct atoms in increasing order.

[`SubsetCarry`](SubsetCarry.lean) proves the actual carry loop and entry function.
A completed proper carry gives the next positional selection, its exact
population and its work charge. A stopped carry retains the actual partial
words and count, with unchanged theory field and word-array length; it is not
claimed to represent a completed successor. The outer proper-subset guard is
essential: carrying a full selection would clear it, whereas the mathematical
counter reports overflow.

[`SubsetQuery.completed_reduct`](SubsetQuery.lean) derives the completed query's
meaning from actual original evaluation. [`SubsetQueryTotal`](SubsetQueryTotal.lean)
constructs every typed outcome: admission charges one subset even when later
evaluation or root checking stops, while refusal before admission leaves the
work record and old output unchanged. Node and root work grows by at most
`N + R` per query.

[`FixedSearch.calls_refine`](FixedSearch.lean) constructs the actual search calls
by induction on the number of remaining positional states. Each false query is
followed by the actual carry; a successful carry increases rank by one and
preserves the packed invariant. A true query returns a proper-subset model of
the frozen reduct. Exhaustion refutes every remaining proper subset. Query and
carry stops return their actual state and work without claiming exhaustion.

[`MembershipSearch`](MembershipSearch.lean) starts this argument at the empty
packed interpretation, obtains the selected atoms from the actual scan, and
composes search with the actual original root check. The resulting answer-set
criterion concerns one candidate of the ground formula theory. It does not
establish coverage of the solver's candidate generator or optimized checking
routes. The [reproduction guide](REPRODUCING.md#current-subset-search-extraction-limit)
records the remaining public allocation and ownership boundary.

## Observation boundary

The atomic external model returns the Boolean stored in its supplied token.
An evaluator body reads cancellation at most once and optionally expiry at most
once.
A single invocation can use fresh observations; `FixedLoop` follows the actual
generated loop's control threading, so repeated reads return fixed values.
Only the Relaxed loads used here are modeled; other model orderings do not
assert which loads are valid in Rust.

`Trace` is broader: each body call may receive a separately supplied control
value. Fixed-loop calls form such a trace; refreshed-control traces need not be
executions of that loop. A checked [example](FixedLoopExample.lean) stops after
one node and work unit when cancellation is refreshed, while the fixed-clear
loop completes two nodes and charges two units. The pinned result effect has no
changing external-read event. Correspondence with varying Rust atomic observations
and concurrent histories remains open.

[`RuntimeEffects`](RuntimeEffects.lean) provides a separately checked event model
for that boundary. Reads can return different values for the same handle;
reservation can return a capacity certificate or a typed refusal. Its embedding
of existing backend computations preserves success, failure, divergence and
sequential composition. The model is not installed as the generated checker's
result type. Its examples therefore establish feasibility, not correspondence
of the current extraction to concurrent execution.

## Representation and trust

Both passes use the same stored table and numeric atom vocabulary. The Arc
model carries explicit owner identity; its connection to Rust allocations and
immutable-heap consistency are library contracts. Aeneas's vector
model records logical elements and checked indices, not allocation capacity or
allocation failure. Unused clock and synchronization fields have tokens but no
modeled operations. Reference counting, destruction, timers, concurrent memory,
machine code and GPU execution remain outside this model.

The Rust compiler, Charon and Aeneas translations, and the correspondence of
library models to Rust, remain trusted boundaries. There are no project axioms,
proof holes or native proof-evaluation shortcuts. The package does not establish
`FrozenReduct` construction, public allocation and owner-checking wrappers,
source grounding, candidate enumeration, optimized checking routes or end-to-end
solver verification.

## Extraction identity and reproduction

The generated types and functions come directly from production Rust, including
the four private subset-search operations and the four private admission
validators of `Theory::new`. The LLBC destination becomes portable,
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
