# Refining formula evaluation

The separately built [implementation-refinement package](https://github.com/GregoryGelfond/zetesis/tree/main/refinement/evaluation)
connects the CPU reference checker's actual generated evaluation, root scan,
atom selection and proper-subset search to the Ferraris answer-set definition.
The result concerns one candidate of a ground formula theory under fixed
observation tokens; public allocation and owner checking remain outside it.

## From original truth to answer-set membership

Let `M` be the candidate and `T` the theory asserted by the stored roots.
[`MembershipSearch.completed_answer_set`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/MembershipSearch.lean)
composes the actual semantic phases:

```text
original evaluation completes; original root check finds no false root
atom selection completes; proper-subset search completes with found

found = false  iff  M is an answer set of T
```

The theorem requires represented candidate storage, ordered child indices,
bounded roots, matching atom counts, an empty selection vector and packed
subset storage representing the empty interpretation. Each phase receives the
preceding phase's returned work. Original evaluation supplies the frozen mask;
actual selection supplies the candidate's atoms without a separate coverage
assumption. The search proof derives proper-subset coverage and query meaning
from the generated operations and the existing ASP laws.

Typed stops retain their state and work and establish no answer-set verdict.
The proof does not establish the public wrapper's allocation, owner checks or
buffer construction, nor coverage of candidate generation or optimized search
routes.

## Initial storage and program ownership

[`PackedSetup`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/PackedSetup.lean)
connects the backend's zero-resize operation to the empty-subset invariant.
Exact candidate word length remains a constructor premise. The resulting
membership theorem no longer needs an assumed meaning for the initialized words.
[`MembershipVerdicts`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/MembershipVerdicts.lean)
also establishes both negative branches: original-root failure and the actual
proper-subset witness returned by search.

[`OwnerChecks`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/OwnerChecks.lean)
proves the generated identity check and clone under explicit library contracts.
Owner identity is distinct from data equality. Consistency with one immutable
heap permits a successful identity check to establish equal theory data;
independently owned equal theories remain distinct. These laws do not verify
reference counting or allocation.

## From evaluation to theory satisfaction

The lower-level stored-reduct query accepts any tested interpretation `J`:

```text
Represents frozen: its stored mask is original node truth at M
completed actual private satisfied_by query at J → Boolean answer

answer = true  iff  J models the Ferraris reduct of T frozen at M
```

[`FrozenQuery.completed_satisfaction`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/FrozenQuery.lean)
requires this mask agreement, represented packed input, ordered children and
bounded roots. It derives the two internal calls from the actual query result,
then establishes their meaning and read bounds. `represents_from_evaluation`
derives the mask agreement for the corresponding record from completed original
evaluation; it does not verify `FrozenReduct::freeze` or its allocation.
The existing `TheorySatisfaction` results cover separate original and reduct
calls. No candidate modelhood or tested-subset premise is required; satisfaction
does not establish answer-set membership.

The query retains one work record across evaluation and root checking. For `N`
nodes and `R` root occurrences, every typed return charges at most `N + R`,
without resetting prior work. Limits and subset statistics are preserved.
An evaluation stop skips the root scan; a root-scan stop remains the same error.
The argument composes these boundaries:

| Boundary | Argument |
| --- | --- |
| Setup | The generated entry operations produce a zero cursor and empty output |
| Packed membership | The extracted atom query returns the stored bit |
| One node | Actual reads and Boolean branches append exactly the masked truth |
| Work and stopping | Each continuation charges one unit; a stop preserves the prior prefix |
| Loop | Actual returned-control threading gives a terminating, trace-correct outcome under fixed tokens |
| Reduct | Two successful generated evaluations compute explicit Ferraris reduct truth |
| Roots | The actual scan preserves root order, duplicates, first-failure identity and exact work |
| Theory | A completed scan returns no failed root exactly when the tested interpretation models the asserted theory |
| Stored query | The actual method uses the represented mask and shares evaluation's returned work with the root scan |

An exhausted iterator completes without polling or charging a node. With a node
present, the iterator fetches it before polling; cancellation, deadline and work
refusal precede its evaluation and append. A typed stop leaves a strictly shorter
correct prefix. `FixedLoop` proves that the generated function terminates under
its input invariants and fixed-token model; termination may be such a refusal,
not a successful evaluation.

The root scan charges every tested occurrence, including duplicates and the
first false root. It returns that root's node identifier, not its list position
or the smallest false identifier. Empty roots complete without polling or work.
A typed stop certifies only the preceding true occurrences, not satisfaction
or rejection of the whole theory. Checked
[examples](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/RootScanExample.lean)
exercise empty and repeated-root scans. [Query examples](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/FrozenQueryExample.lean)
cover cumulative work refusal, a nonmodel candidate with a valid mask, and a
successful tested non-subset.

## Actual proper-subset search

[`FixedSelection`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/FixedSelection.lean)
proves the actual atom-selection loop and entry function using `SelectedAtoms`'s
range, membership and vector-append laws. Every typed return gives the exact
visited prefix and work charge. A completed scan returns precisely the
candidate's atoms, in increasing order and without duplicates.

[`SubsetCarry`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/SubsetCarry.lean)
connects the actual packed updates to the shared positional counter. A completed
proper carry preserves the exact selected subset and population, with one tick
per visited coordinate. A typed stop retains partial words and count; no
completed-successor claim is made for that state. The outer guard excludes the
full candidate before querying or carrying it.

[`SubsetQuery.completed_reduct`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/SubsetQuery.lean)
proves each completed query's reduct-satisfaction result using the actual
original evaluation. `SubsetQueryTotal` constructs every typed query outcome.
Admission charges one subset, including when evaluation or root checking later
stops; refusal before admission preserves the old output and work. Each query
adds at most the node count plus the number of root occurrences to work.

[`FixedSearch.calls_refine`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/FixedSearch.lean)
constructs the actual search execution by decreasing the number of remaining
counter states. A true query gives a proper-subset model of the frozen reduct;
a false query followed by a successful carry advances exactly one state.
Exhaustion therefore refutes every proper subset. Stopped queries and carries
retain their actual outcomes and cannot establish exhaustion.

`MembershipSearch` composes this search from the empty packed interpretation
with completed atom selection and original modelhood. The proof uses the shared
counter coverage theorem rather than assuming a correct oracle or complete
external proposals. It establishes the answer-set criterion for the reference
checker's semantic phases, not the public wrapper or other checking routes.

## Reusing the ASP library

The package imports `ReductEvaluation`, `PackedSubsets` and their dependency
closures directly from the general library. The same source files compile under
both the library's Lean 4.33.1 and the extraction package's Lean 4.31.0.
The bridge is checked in the latter toolchain with its own audit; it does not mix
object files from different versions or duplicate the semantic definitions.

A structural conversion maps extracted node constructors and machine indices to
the library's formula DAG. Fold correspondence then connects completed generated
calls and their asserted roots to the existing reduct theorem. The result is a
checked connection to the same mathematical theory used elsewhere in the library.

## Remaining implementation boundary

`FixedLoop` follows the generated loop's control threading: repeated token reads
return the same value. The broader authored `Trace` relation permits separately
supplied controls between body calls. Fixed-loop calls form a trace, but the
[checked cancellation example](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/FixedLoopExample.lean)
shows that a refreshed trace can stop after one node while the fixed-clear loop
completes two. The pinned extraction effect cannot express changing atomic read
results, so correspondence with concurrent Rust histories remains open.

A separate checked `RuntimeEffects` model admits returning read and reservation
events and preserves embedded backend computations. It is not yet connected to
the generated checker. Supporting those effects through the extraction backend's
function and loop interfaces remains necessary.

The calls share stored nodes, roots and numeric atom vocabulary. `FrozenReduct`
construction, public allocation, composition of owner checks with the public
wrapper, and actual admission constructors remain unproved. Source grounding, candidate
enumeration and optimized checking routes are separate obligations. Allocation,
reference counting, timers, concurrent memory, machine code and GPU execution
remain outside these models. The
[reproduction guide](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/REPRODUCING.md#current-subset-search-extraction-limit)
distinguishes this loop from the public wrapper's allocation and ownership models.

The [package guide](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/README.md)
records the exact scope, source hashes and reproduction commands. Generated code
comes from production Rust. Recorded local-name adjustments avoid generator
namespace collisions; exported destination metadata uses a portable path. An
unreferenced derived `Debug` implementation and its ordered registration are
also removed after an explicit reference check. Two unused range-trait method
registrations are omitted from the trait and its implementation to match the
pinned backend, without shifting indices. These reversible, audited selections
leave executable bodies unchanged. Translation tools, preprocessing
and model correspondence remain trusted. The [correctness plan](correctness.md) keeps these limits separate
from whole-solver soundness and complete enumeration.
