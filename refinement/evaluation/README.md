# Refining the reference checker

This separately built implementation-refinement package connects completed calls
of the CPU reference checker's generated public `check` to the Ferraris answer-set
definition under fixed observation tokens and explicit library contracts. It
derives the actual evaluation, root scan, atom selection and proper-subset search
from the wrapper's return. The package also proves theory admission and
construction, finite interpretation construction, and stored-reduct construction
and public querying under their stated allocation and reservation contracts.

The reusable ASP theory lives in [`proofs`](../../proofs/README.md). This package
imports the reduct-evaluation, packed-subset and admission sources directly and checks them
with the extraction backend's Lean 4.31.0. The main library retains Lean 4.33.1.
No semantic definitions are copied or replaced; object files from different Lean
versions are not mixed. `semantic-inputs.sha256` identifies the shared sources.

## Central result

[`PublicMembership.completed_answer_set`](PublicMembership.lean) proves that an
actual completed public `oracle::check` returns `Stable` exactly when its
candidate is an answer set of the stored formula theory.

The premises require exact candidate word storage, ordered child indices,
bounded roots, consistency of both theory views with one immutable heap, and
preservation of the empty input sequence by two successful reservations: the
selected-atom buffer and the subset-word buffer. Evaluation clears its Boolean
workspaces, so their previous contents need no such contract. No reservation is
assumed to succeed.

[`PublicCheckPhases`](PublicCheckPhases.lean) recovers the actual owner check,
initial poll, reservations, initialization and semantic calls from the completed
wrapper return. The proof derives the frozen mask's meaning, selected-atom
coverage and proper-subset completeness. It assumes neither successful inner
calls nor a correct subset oracle. The published statistics are those of the
last returned work record.

`completed_not_model` identifies an asserted root with a present false original
value. `completed_nonminimal` identifies the actual returned program-owned proper
subset that models the candidate's reduct. These are the wrapper's own negative
evidence. [`PublicCheckBoundary`](PublicCheckBoundary.lean) separately proves
wrong-owner refusal, initial-control refusal and the first reservation's
allocation refusal, in source order.

These are completed-result theorems under one supplied pure reservation operation
and fixed observations. They do not assert termination of an arbitrary provider
or correspondence to changing runtime histories. A public stop carries no
membership verdict or partial statistics.

## From construction to a retained answer

[`SubjectMembership.completed_subject`](SubjectMembership.lean) starts with
successful generated `Theory::new`, owned-vector `Interpretation::new` and
`check_interpretation` calls. It derives the node, root, storage and owner
invariants rather than requiring them again. The returned decision retains the
exact candidate, and its acceptance accessor returns true exactly when that
candidate is an answer set of the constructed theory.

`completed_stable` carries this result through the actual consuming conversion
to `StableInterpretation`. [`CheckedResults`](CheckedResults.lean) proves that
accessors preserve the stored values, successful conversion retains the subject,
and rejected conversion retains the entire decision. An arbitrary Lean record
with a Stable field is not thereby semantic evidence; the theorem requires the
actual constructor and check calls that produced it.

The composition uses explicit successful Arc-value and reservation-sequence
contracts. It does not require allocation to succeed on every invocation or
assume semantic oracle agreement. A supplied allocation operation and one shared
reservation provider interpret these calls under fixed observations. Correspondence
to changing runtime histories remains the outstanding membership boundary.

## Storage and ownership

[`PackedSetup`](PackedSetup.lean) proves that the backend's zero-resize operation
produces the exact empty packed interpretation needed by subset search. Its
`initialized_membership` theorem supplies that result to the membership proof.
Exact candidate word length remains a premise of this composition; the
constructor proofs below now supply it for completed finite
inputs. Having enough words for reads alone does not establish the required shape.

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

These results supply the public composition without assuming successful
allocation or equating owner identity with value equality. The theory and
interpretation constructors below supply its input invariants. Correspondence
to changing runtime histories remains a separate obligation.

## Admission validation

`Theory::new` runs the private step `admit` before it allocates the shared
theory. `admit` checks dimensions and the padded word count, then calls two
private validators: `validate_nodes` checks every node in stored order against
the atom universe and the nodes before it, and `validate_roots` checks that
every asserted root names a stored node.
[`AdmissionValidation`](AdmissionValidation.lean) proves the generated code of
both validators, with no premise on the input slices.

`validate_nodes_exact` shows that the generated node validator returns the verdict
of the authored scan `TheoryAdmission.scan`, started at position zero, over the
converted nodes. Enumeration positions are list positions, and every enumeration
step is justified by the slice length bound, so the call always returns.
`validate_nodes_accepts_iff` derives acceptance exactly from ordered children and
in-universe atoms; `validate_nodes_refuses_iff` shows that a refusal reports the
error of the first refused node, every earlier node having passed its check.
`validate_roots_exact` identifies the generated root validator with the authored
root scan `TheoryAdmission.rootScan`, which `TheoryAdmission.validate_phases`
identifies as the root clause of `validate`. This checks root indices, whereas
the evaluator's `RootScan` checks root truth. It accepts exactly when every root
index is within the node table. Repeated in-range indices and an empty root list
are accepted; an out-of-range index is refused wherever it occurs.

`accepted_structure` applies both acceptances exactly as `admit` applies
the validators to its input vectors. It derives ordered children, bounded roots
and in-universe atoms for those vectors. Neither validator can report an
allocation refusal. [Examples](AdmissionValidationExample.lean) check refusal
order in both directions, repeated roots and empty input on the generated
functions.

[`AdmittedData`](AdmittedData.lean) proves the generated `admit` itself.
`admit_exact` shows that it returns the verdict of the authored
`TheoryAdmission.validate` with the host's `Usize.max` as its maximum, so Rust's
`checked_add(63)` is exactly the authored padded-count check, and that success
returns the supplied atom count and vectors unchanged, in their stored order and
multiplicity. `admit_accepts_iff` and `admit_refuses_iff` read off acceptance and
refusal; `admitted_word_counts` derives the representable 64-bit and 32-bit word
counts. The equation holds for every input, so `admit` always returns and never
reports an allocation refusal.

[`TheoryConstruction`](TheoryConstruction.lean) proves the generated
`Theory::new` wrapper with a supplied allocation operation.
`completed_phases` derives successful admission and allocation from a successful
constructor return. `refused_iff` identifies each typed constructor refusal with
its admission refusal. Allocation failure or divergence is not converted to an
admission error or a successful theory.

Under the library contract that allocation returns the supplied value,
`returned_structure` derives ordered nodes and bounded roots for the returned
theory. The membership theorems retain those premises; the constructor now
supplies them. `returned_heap` additionally preserves existing views under an
explicit fresh heap transition. Rust, `Arc`, the allocator, OS and hardware are
trusted to satisfy their contracts, not reimplemented or verified here.

The allocation parameter is an audited addition around the generated wrapper;
its body is unchanged. It describes one invocation. One pure provider cannot
model repeated fresh allocations of identical data; their composition requires
separate invocation/heap correspondence. There is no global provider instance
or assumption of allocation success. See [reproduction](REPRODUCING.md).

## Interpretation construction

[`InterpretationConstruction`](InterpretationConstruction.lean) proves the phases
of generated `Interpretation::new`, retaining its supplied iterable and iterator
operations. `phases_exact` follows word counting, reservation, resize, iterable
conversion, checked insertion and the final theory clone. A reservation refusal
returns `AdmissionError::Allocation` before conversion or insertion. Insertion
refusals pass through unchanged; backend failure and divergence remain distinct.

`completed_phases` derives successful internal calls from a successful constructor
return. Its word count equals the shared 64-bit packed count, and its words are
exactly the insertion helper's returned slice. `completed_theory` establishes
retention of the supplied theory, including its owner. The private `insert_atoms`
helper takes only the atom bound, iterator and mutable word slice; allocation
and publication stay in the public constructor.

[`UsizeCeiling.word_count64`](UsizeCeiling.lean) establishes the count for every
host unsigned value under an authored model of Rust's `usize::div_ceil`.
Correspondence of this external model to the standard library remains trusted.
[`SliceInsertion`](SliceInsertion.lean) connects the actual checked arithmetic,
slice read and update to the shared packed insertion operation.
[`InsertionLoop.insert_exact`](InsertionLoop.lean) proves termination for a
`FiniteInput` contract consisting only of actual iterator observations, given
sufficient word storage. The helper inserts exactly the bounded prefix,
preserving word count, and returns
`Atom` at the first invalid coordinate. Duplicates and arbitrary input order
are allowed; behavior after the first None is irrelevant.

[`InterpretationStorage.completed_pack`](InterpretationStorage.lean) derives
retained theory, valid input bounds and exact packed contents from a successful
actual constructor with finite observed input. It uses the explicit library contract that successful
reservation preserves the empty input vector; resize alone would retain an
existing prefix. `packed_queries` then supplies exact storage, zero padding and
actual membership answers. [`VectorInput`](VectorInput.lean) derives the finite
contract from the imported owned-vector iterator, so `completed_vector` needs no
supplied iterator trace or input-validity premise.

[Boundary examples](InsertionBoundaryExample.lean) prove refusal before a
diverging tail and completion at the first None of a nonfused input. These laws
assume neither allocation success nor termination of unrestricted iterators.
The helper returns no terminal iterator state; correspondence to a Rust caller's
borrowed state and destruction remains a library/translation boundary.

## Stored reduct queries

[`PublicFrozenQuery.constructed_satisfaction`](PublicFrozenQuery.lean) proves
that, after actual `FrozenReduct::new` and `is_satisfied_by` calls complete,
the query returns `true` exactly when the tested interpretation `J` models the
asserted theory's Ferraris reduct frozen at the supplied candidate `M`.
It requires represented storage for both interpretations, ordered child indices
and bounded roots. Neither original modelhood of `M` nor `J ⊆ M` is required;
this is satisfaction, not answer-set membership.

[`FrozenConstruction`](FrozenConstruction.lean) derives candidate retention and
the stored mask's agreement with original truth from actual construction.
[`PublicFrozenQuery`](PublicFrozenQuery.lean) follows the public query's owner
check, initial poll and reservation before applying the private
[`FrozenQuery.completed_satisfaction`](FrozenQuery.lean) result. The composed
theorem assumes neither mask agreement nor successful internal calls. A wrong
owner is refused before polling or reservation, even when theory contents agree.

Construction and query have separate reservation providers, limits and control
tokens. These describe their individual invocations, not an allocator history;
no provider instance or allocation-success law is installed.
[`ReservedStorage`](ReservedStorage.lean) proves that the actual reservation
wrapper maps a source refusal to `Stop.Allocation` and retains backend failure
and divergence. The semantic construction proof needs no premise about reserved
contents because evaluation clears its workspace. Physical capacity remains a
trusted library contract.

For `N` nodes and `R` root occurrences, a completed freeze charges exactly `N`
node visits. The private query shares one
work record across evaluation and root scanning, preserving limits and subset
statistics and charging at most `N + R`, including typed stops. The public
constructor and query each start fresh counters; query limits do not include
construction work. A stop remains an error, not a satisfaction verdict.
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
| `ReservedStorage`, `WorkInitialization` | Actual reservation outcomes are preserved and public calls start with zero counters |
| `FrozenConstruction`, `PublicFrozenQuery` | Actual construction establishes the mask invariant; the completed public query decides that reduct under supplied reservation operations |
| `SubsetQuery`, `SubsetQueryTotal` | Actual subset queries decide reduct satisfaction on completion; every typed return retains the admission charge and shared work bounds |
| `FixedSelection` | The actual scan returns the exact ordered candidate atoms, or a stopped prefix with exact work |
| `SubsetCarry` | Actual proper carries preserve packed selection and population on completion, retaining partial state on stops |
| `SearchSteps`, `CountermodelTrace` | Exact generated search branches and their finite composition retain returned state and work |
| `SearchRepresentation`, `SearchSemantics` | Packed counter states denote semantic subsets and queries use the actually computed original mask |
| `FixedSearch`, `MembershipSearch` | The actual search covers proper subsets, and its completed result composes with original modelhood to decide answer-set membership |
| `SearchFrame` | Typed search returns preserve theory, word length, limits and control, with nondecreasing work and subset counts |
| `PublicCheckBoundary`, `PublicCheckPhases` | Early refusals follow source order; completed public returns supply the actual setup and semantic calls |
| `PublicMembership` | Completed public verdicts give answer-set equivalence and their actual false-root or proper-subset evidence under explicit input and library contracts |
| `AdmissionValidation` | The generated node and root validators compute the authored node and root scans, report the first refused node's error, and supply ordered-children and bounded-root premises |
| `AdmittedData` | The generated admission step computes the authored validator, including the padded-count check, and returns the supplied data unchanged |
| `UsizeCeiling`, `InterpretationConstruction` | Exact packed word count and actual constructor phase order, reservation refusal and retained theory |
| `SliceInsertion`, `InsertionLoop` | With covered word storage, actual finite insertion terminates with exact bounded-prefix writes, first-invalid refusal and preserved word count |
| `VectorInput`, `InterpretationStorage` | Owned-vector observations discharge the input contract; completed construction yields exact packed contents and padding under successful-reservation sequence preservation |
| `InsertionBoundaryExample` | First-invalid refusal needs no returning tail; the first None ends a nonfused input |

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
records the supplied-operation and runtime-history boundary.

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
model records logical elements and checked indices, not allocation capacity.
`VectorReservation` supplies fallible reservation outcomes without assuming
success or modeling physical storage. Unused clock and synchronization fields have tokens but no
modeled operations. Reference counting, destruction, timers, concurrent memory,
machine code and GPU execution remain outside this model.

The Rust compiler, Charon and Aeneas translations, and the correspondence of
library models to Rust, remain trusted boundaries. There are no project axioms,
proof holes or native proof-evaluation shortcuts. The package does not establish
termination of unrestricted iterators, correspondence to changing allocation
or control histories, source grounding, candidate enumeration, optimized checking routes or end-to-end solver
verification.

## Extraction identity and reproduction

The generated types and functions come directly from production Rust, including
the four private subset-search operations, `Theory::new` with its private
admission step and four validators, `Interpretation::new` and its private
`insert_atoms` helper, and the stored-reduct constructor, public query and
reservation wrapper, together with public `oracle::check`, `check_interpretation`
and its decision/accepted-interpretation accessors and consuming conversion.
The LLBC destination becomes portable,
and local names change from `theory` to `program` to avoid namespace
collisions; operands retain their local IDs. An unused derived `Debug`
implementation whose formatting method was excluded is removed with its
ordered registration, after checking for surviving semantic references.
Two unused range-trait method registrations are omitted from both the trait and
its implementation to match the pinned backend model, with indices preserved.
The selected operations do not refer to either method. The generic interpretation
constructor and insertion loop are translated with their iterable and iterator
arguments intact. Restoring the selections and metadata/name fields recovers the
parsed raw input.

Scoped section binders supply `ArcAllocation` to generated `Theory::new` and
`VectorReservation` to `oracle::{reserve, check}`, `FrozenReduct::{freeze, new,
is_satisfied_by}`, `Interpretation::new` and `check_interpretation`. No body is
rewritten. Removing the
recorded insertions restores the exact generated Lean. This dependency
parameterization is distinct from metadata normalization and remains part of the
trusted extraction adaptation.

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
