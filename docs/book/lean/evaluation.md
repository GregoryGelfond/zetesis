# Refining CPU checks

The separately built [implementation-refinement package](https://github.com/GregoryGelfond/zetesis/tree/main/refinement/evaluation)
connects finite completed public calls of the CPU scalar reference checker
to the Ferraris answer-set definition, including returning control reads and
reservations. The result concerns one candidate of one finite ground formula
theory under explicit library, runtime and extraction contracts. The package also proves theory admission and construction, finite
interpretation construction, and stored-reduct construction and public querying.

The unprefixed refinement declarations on this page concern the retained binary
`Evaluator` generation. Native row, admission, evaluator, query and public
membership proofs live separately under `Native`; their current scope is listed
in the [correspondence chapter](correspondence.md#indexed-reads-counters-and-packed-scans).
Native runtime-effect composition and the outer tight classifier, recognizer and
producer proofs remain separate. Historical source records do not qualify changed
native operations.

## Recognizing bodies for tight checking

The optimized tight check needs a structural certificate before it may replace
proper-subset reduct search. Its body classifier distinguishes exact admitted
body syntax from unsupported implications, then records whether an atom occurs
outside default negation. Negation freezes its entire interior, including any
implications there.

[`TightBodyRecognition`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/TightBodyRecognition.lean)
proves recognition and positive-occurrence laws over formulas independently of
Rust. [`TightClassification.completed_semantics`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/TightClassification.lean) connects the generated node scan to
those laws. Each successful scan classifies every node in its original order;
its classes are `Opaque`, `Frozen` or `Positive` according to that exact grammar.
These describe syntax, not truth: a conjunction containing falsum can still
have an unfrozen positive occurrence.

The implementation proof retains the supplied reservation and control contracts,
bounded reads and exact work charged by the scan. It does not assume allocation
success or a correct classifier. Complete producer extraction and root coverage,
rank validation and the optimized membership verdict remain separate obligations.
The classifier's fixed-provider proof does not extend the reference checker's
runtime-event theorem to the optimized route.

## Recognizing atomic choices

[`AtomicChoice.atom_meaning_iff`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/AtomicChoice.lean)
connects the actual head recognizer to exact formula syntax:

```text
recognize(head) = Some(a)
  iff meaning(head) = a ∨ not a or meaning(head) = not a ∨ a
```

The node table must be ordered and the head index bounded. Distinct stored atom
occurrences are allowed when they name the same atom; alternatives involving
different atoms and richer equivalent formulas are excluded. The proof derives
the stored-node witnesses from execution and uses DAG decoding in both directions.
The operation has no allocation, cancellation read or separate work charge.

The existing choice-reduct laws apply to these recognized shapes.

## Recovering an original producer

[`TightProducer.completed_producer`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/TightProducer.lean)
connects a completed single-root call to an explicit `TightPlans.Producer` with
the same original formula, head, indexed body and ordinary/choice kind. Facts
remain facts; both choice orientations retain their syntax. The proof composes
the actual stored reads, completed body classification and head recognition.

A skipped root denotes falsum or default negation. Unsupported heads and bodies
retain their typed refusal and inspection order. These results do not establish
that the caller has visited every asserted root: complete two-pass extraction,
rank validation and the support-checking loop remain separate obligations.

## From the public check to answer-set membership

Let `M` be the candidate and `T` the theory asserted by the stored roots.
[`RuntimeMembership.completed_answer_set`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/RuntimeMembership.lean)
proves for a completed execution of the source-checked runtime contexts:

```text
actual public check(T, M, limits, control) completes with checked

checked.verdict = Stable  iff  M is an answer set of T
```

The premises require exact candidate word storage, ordered child indices,
bounded roots and consistency of both theory views with one immutable heap.
The runtime reservation projection supplies sequence preservation for the selected
atoms and subset words. Boolean workspaces need no content contract because
evaluation clears them. Reservation success is not assumed.

`RuntimeProjection` derives the actual generated public result from the observed
execution. [`PublicCheckPhases`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/PublicCheckPhases.lean)
then recovers the owner check, initial poll, reservations, initialization and semantic
calls from the actual completed wrapper return. Each phase receives the previous
phase's work. Evaluation derives the frozen mask; selection derives the candidate
carrier; the search proof establishes proper-subset coverage and reduct truth.
No successful inner call, correct oracle or complete enumeration is a premise.
The wrapper publishes the last returned work record's statistics.

`completed_not_model` identifies an asserted root with a present false original
value. `completed_nonminimal` identifies the returned program-owned proper-subset
reduct model. [`PublicCheckBoundary`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/PublicCheckBoundary.lean)
also proves wrong-owner refusal, initial-control refusal and allocation refusal
at the first reservation. Public stops carry no verdict or partial statistics.

`RuntimePublicRefusal` additionally follows every finite public typed stop to
its reached operation and cause in source order. Supporting loop laws retain
executed prefixes and internal partial state. Cancellation precedes expiry and quota checks; a refused tick charges no
new work. This is not a proof of arbitrary-provider termination, fairness,
candidate generation or optimized checking routes.

## From construction to a retained answer

[`SubjectMembership.completed_subject`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/SubjectMembership.lean) starts with
successful generated `Theory::new`, owned-vector `Interpretation::new` and
`check_interpretation` calls. It derives the node, root, storage and owner
invariants rather than requiring them again. The returned decision retains the
exact candidate, and its acceptance accessor returns true exactly when that
candidate is an answer set of the constructed theory.

`completed_stable` carries this result through the actual consuming conversion
to `StableInterpretation`. [`CheckedResults`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/CheckedResults.lean) proves that
accessors preserve the stored values, successful conversion retains the subject,
and rejected conversion retains the entire decision. An arbitrary Lean record
with a Stable field is not thereby semantic evidence; the theorem requires the
actual constructor and check calls that produced it.

The constructor composition uses explicit contracts for retained Arc values and
reservation sequences; it assumes neither allocation success nor semantic oracle
agreement. `RuntimeMembership.completed_subject` supplies the eventful owned check
after those successful constructors. Its `completed_stable` theorem follows the
actual conversion and retains the same candidate as an answer set. Constructor
providers describe their individual calls; the checking history can contain
independently observed reservations, without a shared deterministic allocation
history.

## Initial storage and program ownership

[`PackedSetup`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/PackedSetup.lean)
connects the backend's zero-resize operation to the empty-subset invariant.
Exact candidate word length remains a premise of this membership composition;
the constructor results below supply it for completed finite inputs. The theorem
no longer needs an assumed meaning for the initialized subset words.
[`MembershipVerdicts`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/MembershipVerdicts.lean)
also establishes both negative branches: original-root failure and the actual
proper-subset witness returned by search.

[`OwnerChecks`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/OwnerChecks.lean)
proves the generated identity check and clone under explicit library contracts.
Owner identity is distinct from data equality. Consistency with one immutable
heap permits a successful identity check to establish equal theory data;
independently owned equal theories remain distinct. These laws do not verify
reference counting or allocation.

## Admission validation

`Theory::new` runs the private step `admit` before it allocates the shared
theory. `admit` checks dimensions and the padded word count, then calls two
private validators. `validate_nodes` checks every node in stored order against
the atom universe and the nodes before it; `validate_roots` checks that every
asserted root names a stored node.
[`AdmissionValidation`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/AdmissionValidation.lean)
proves their generated code against the authored checks of the general
library's [`TheoryAdmission`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/TheoryAdmission.lean):

```text
validate_nodes(atoms, nodes) returns the verdict of scan(atoms, 0, nodes)
validate_roots(count, roots) returns the verdict of rootScan(count, roots)
```

`TheoryAdmission.validate_phases` identifies `TheoryAdmission.rootScan` as the
root clause of `validate`. This checks root indices, whereas the evaluator's
`RootScan` checks root truth. Both equations hold for every represented slice.
The generated loops always return, enumeration positions are list positions,
and neither
validator reports an allocation refusal. Acceptance derives ordered children,
in-universe atoms and bounded roots; `accepted_structure` states these for the
vectors that `admit` validates. A node refusal reports the error of the
first refused node in stored order, every earlier node having passed; this
follows from the general `scan_refusal_exact`. Root validation accepts exactly
when every root index is within the node table. Repeated in-range indices and
an empty root list are accepted; an out-of-range index is refused wherever it
occurs.

[`AdmittedData`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/AdmittedData.lean)
proves the generated `admit` itself. It returns the verdict of the authored
`validate` with the host's `Usize.max` as the maximum, so Rust's
`checked_add(63)` is exactly the authored padded-count check. Success returns
the supplied atom count and vectors unchanged, in their stored order and
multiplicity, and the admitted atom count has representable 64-bit and 32-bit
word counts.

[`TheoryConstruction`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/TheoryConstruction.lean)
connects admission to the generated `Theory::new` wrapper. A successful return
establishes both successful admission and allocation. Typed refusal is exactly
admission refusal; allocation failure or nonreturn cannot become a successful
theory. Under the trusted `Arc` contract that allocation stores its input,
`returned_structure` derives ordered nodes and bounded roots. Those remain
premises of the membership theorems, now supplied by the constructor.

The generated wrapper has an explicit allocation parameter, introduced by a
checked, reversible binding adaptation; its body is unchanged. This models one
invocation without assuming allocation succeeds. Freshness is a separate
per-invocation heap contract. A single pure provider does not model repeated
fresh allocations of equal data.

[`InterpretationConstruction`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/InterpretationConstruction.lean)
proves the actual interpretation constructor's phase order: word counting,
reservation, resize, iterable conversion, insertion and final theory clone.
Reservation refusal precedes conversion and insertion. A successful return
derives successful internal calls, retains the supplied theory and contains
exactly the insertion helper's returned words. The supplied generic iterable and
iterator operations are retained.

[`UsizeCeiling.word_count64`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/UsizeCeiling.lean)
connects the constructor's modeled `usize::div_ceil` to the shared packed word
count. Its correspondence to Rust's standard library remains trusted.
[`SliceInsertion`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/SliceInsertion.lean)
relates actual checked scalar and slice operations to packed insertion.
[`InsertionLoop`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/InsertionLoop.lean)
then proves the actual helper terminates with sufficient word storage and
`FiniteInput`, a contract consisting only of actual next observations. It writes
exactly the prefix before the first
invalid atom, returns `Atom` on that refusal, and preserves word count. Duplicates
and arbitrary order are permitted.

[`InterpretationStorage.completed_pack`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/InterpretationStorage.lean)
derives valid atom bounds, retained theory and exact packed contents from a
successful constructor, using the explicit contract that successful reservation
preserves its empty input. `packed_queries` supplies exact storage, zero padding
and actual membership answers. [`VectorInput`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/VectorInput.lean)
derives the finite contract for the imported owned-vector iterator; the resulting
`completed_vector` theorem needs no supplied iterator trace or validity premise.

[Boundary examples](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/InsertionBoundaryExample.lean)
show first-invalid refusal before a diverging tail and completion at the first
None of a nonfused input. These laws assume neither allocation success nor
termination of arbitrary iterators. The helper returns no terminal iterator state.

## From evaluation to theory satisfaction

The public stored-reduct constructor and query now compose directly:

```text
actual FrozenReduct::new at M completes with frozen
actual frozen.is_satisfied_by at J completes with answer

answer = true  iff  J models the Ferraris reduct of T frozen at M
```

[`PublicFrozenQuery.constructed_satisfaction`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/PublicFrozenQuery.lean)
requires represented storage for both interpretations, ordered children and
bounded roots. Actual construction establishes the stored mask's meaning and
retains the supplied candidate; no mask agreement or successful internal call
is assumed. No candidate modelhood or tested-subset premise is required.
This is reduct satisfaction, not answer-set membership.

Construction and query use separately supplied reservation operations, limits
and fixed control tokens. The theorem concerns their completed calls and assumes
no allocation-success law. The public query checks owner identity before polling
or reservation. Source reservation refusal becomes `Stop.Allocation`; backend
failure and divergence remain distinct. Evaluation clears the reserved workspace,
so its prior contents need no preservation premise. Physical capacity and the
correspondence to allocator behavior remain trusted library contracts.

[`FrozenConstruction`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/FrozenConstruction.lean)
proves the producer invariant consumed by the private
[`FrozenQuery`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/FrozenQuery.lean).
For `N` nodes and `R` root occurrences, a completed freeze charges exactly `N`
node visits. The private query retains one
work record across evaluation and root checking: every typed return charges at
most `N + R`, preserving limits and subset statistics. The public constructor
and query each start fresh counters, so query limits exclude construction work.
An evaluation stop skips the root scan; a root-scan stop remains the same error.
The existing `TheorySatisfaction` results cover separate original and reduct
calls. The argument composes these boundaries:

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
| Construction and public query | Actual construction establishes that mask; the public query checks ownership, polls and reserves before querying |

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
external proposals. `PublicMembership` derives these phase calls from the public
wrapper and discharges their setup premises under its explicit input and library
contracts. Other checking routes remain separate.

## Reusing the ASP library

The package imports `ReductEvaluation`, `PackedSubsets`, `TheoryAdmission` and
their dependency closures directly from the general library. The same source files compile under
both the library's Lean 4.33.1 and the extraction package's Lean 4.31.0.
The bridge is checked in the latter toolchain with its own audit; it does not mix
object files from different versions or duplicate the semantic definitions.

A structural conversion maps extracted node constructors and machine indices to
the library's formula DAG. Fold correspondence then connects completed generated
calls and their asserted roots to the existing reduct theorem. The result is a
checked connection to the same mathematical theory used elsewhere in the library.

## Runtime correspondence and remaining scope

The generated backend reads fixed observation bits. `RuntimeContexts` and
`CheckerContexts` retain its operations and control flow while named reads and
reservations receive returning observations. Exact reconstruction laws and a
source checker audit this adaptation. `ReferenceEvents` is their common operational
composition; it introduces no alternative evaluator or subset algorithm.

`RuntimeProjection` recovers the actual generated result from every successful
finite execution. `RuntimeMembership` supplies answer-set equivalence, negative
evidence, and the constructed subject's acceptance and conversion laws.
`RuntimePublicRefusal` classifies stops at the reached operation and identifies
their terminal observations. Supporting loop laws retain actual executed prefixes
and partial state. Backend failure,
divergence and unanswered requests are not completed verdicts or typed stops.

Read handles and deadline configuration stay unchanged. The clear representative
used to apply the fixed-model proofs changes only unused logical bits, not a live
Rust cancellation token; it imposes no restriction on observed read responses.
Reservation events carry success-capacity evidence or refusal and preserve the
logical vector sequence. Successful allocation is never assumed.

The trust boundary includes Rust and its standard library, `Arc`, the allocator,
OS and hardware, the immutable mapping of live objects and fields to handles,
and the compiler/extraction tools and audited source-context adaptation. The
proofs establish zetesis's use of those contracts, not their implementations.
They require no scheduling fairness or eventual response assumption.

This covers finite completed and refused public scalar reference calls for one
finite ground formula theory, including the subject-bound API. It does not prove
source grounding, candidate enumeration, optimized membership routes, parallel
schedules or GPU execution. Finite interpretation insertion does not establish
termination of arbitrary iterators or a caller's borrowed-iterator destruction
behavior. The general ASP library remains independent; the added body-recognition
laws preserve its existing semantic definitions.

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
