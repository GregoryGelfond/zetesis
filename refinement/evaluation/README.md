# Refining CPU checks

This separately built implementation-refinement package connects finite completed
calls of the CPU scalar reference checker to the Ferraris answer-set definition.
Returning reads and reservations preserve the public result or its actual typed
refusal under explicit library and runtime contracts. The scope is one candidate
of one finite ground formula theory. The package also proves theory admission
and construction, finite interpretation construction, and stored-reduct
construction and public querying under their stated contracts.

The reusable ASP theory lives in [`proofs`](../../proofs/README.md). This package
imports the reduct-evaluation, packed-subset and admission sources directly and checks them
with the extraction backend's Lean 4.31.0. The main library retains Lean 4.33.1.
No semantic definitions are copied or replaced; object files from different Lean
versions are not mixed. `semantic-inputs.sha256` identifies the shared sources.

## Tight recognition and single-root producers

The tight specialization is being connected to the same answer-set definition.
Its first correspondence is the actual `tight::compile::classify` scan: successful
classification covers the complete ordered node table and agrees with
[`TightBodyRecognition`](../../proofs/Zetesis/TightBodyRecognition.lean).
That reusable formula-level recognizer preserves exact syntax; a separate test
identifies atom occurrences outside every default negation. It does not rewrite
classically equivalent formulas into the grammar.

`TightDag` connects bounded node lookups to their unfolded formulas.
`TightClassificationSemantics` identifies the finite classification fold with
formula recognition. `TightClassificationStep` and `TightWork` prove the actual
translated steps, including uncharged refusals and one charge per classified
node. The loop and entry proofs retain full-node coverage and the reservation's
explicit sequence contract. These are fixed-provider proofs, not a new
runtime-event correspondence for tight checking.

[`AtomicChoice.atom_meaning_iff`](AtomicChoice.lean) proves the actual head
recognizer accepts exactly `a ∨ not a` or its reversed order in an ordered DAG.
The two atom nodes may be distinct occurrences of the same semantic atom.
Bounded decoding supplies formula equality; no classical rewriting or assumed
recognizer agreement is used. The operation allocates nothing and charges no
work itself: its callers account for the inspection. Its pure standard-library
models preserve eager `then_some` and the None-only fallback of `or_else`.

Ten laws in `TightProducerSemantics`, `TightProducerReads` and
[`TightProducer`](TightProducer.lean) connect a completed actual single-root
producer call to an exact original `TightPlans.Producer`. The witness retains the
head, indexed body, source root and ordinary/choice kind. Facts remain facts;
reversed choices keep their original orientation. `after_classification` derives
the body-class premise from the actual completed classifier. Skipped roots are
falsum or arbitrary default negations; typed refusals retain the source's
head-before-body order.

These single-root laws do not establish a complete tight certificate. The
two-pass producer extraction still needs complete asserted-root and occurrence
coverage; rank validation and the support-checking loop need their implementation
proofs. Existing ranked-support laws provide their semantic endpoint without
supplying those unproved premises.

## Central result

[`RuntimeMembership.completed_answer_set`](RuntimeMembership.lean) proves that a
completed public eventful `check` returns `Stable` exactly when its candidate is
an answer set of the stored formula theory. Its negative laws retain the actual
asserted false root or program-owned proper-subset model of the frozen reduct.

The premises require exact candidate word storage, ordered child indices,
bounded roots and consistency of both theory views with one immutable heap.
[`RuntimeProjection`](RuntimeProjection.lean) derives the generated checker's
actual completed call from the observed execution, with the same verdict and
statistics. It supplies the logical reservation contracts needed by
[`PublicMembership`](PublicMembership.lean); allocation success and successful
inner calls are not assumed. Boolean workspaces are cleared before use.

[`RuntimePublicRefusal`](RuntimePublicRefusal.lean) traces a public typed stop to
its reached source operation and cause. Owner and reservation refusals retain
source order. Local cancellation and then optional slot cancellation precede
deadline expiry; clear controls precede
work or candidate-limit tests. Refused ticks charge no new work. Loop receipts
retain executed prefixes and partial state, while the public error carries no
membership verdict or partial statistics.

These results concern finite returning executions of the source-checked
contexts. They make no progress or fairness guarantee for unanswered requests,
backend failure, divergence or arbitrary providers. The trusted correspondence
from Rust and its libraries to these contexts is stated below.

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

The constructor composition uses explicit contracts for retained Arc values and
reservation sequences; it assumes neither allocation success nor semantic oracle
agreement. `RuntimeMembership.completed_subject` supplies the eventful owned check
after those successful constructors. Its `completed_stable` theorem follows the
actual conversion and retains the same candidate as an answer set. Constructor
providers describe their individual calls; the checking history can contain
independently observed reservations, without a shared deterministic allocation
history.

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
interpretation constructors below supply its input invariants. The runtime
projection composes these contracts with the eventful reference check.

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
| `Control`, `ControlReads` | Exact local/slot/deadline precedence, read receipts and bounded work increments |
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
| `PublicMembership` | Completed generated verdicts give answer-set equivalence and their actual false-root or proper-subset evidence under explicit input and library contracts |
| `RuntimeContexts`, `CheckerContexts`, `ReferenceEvents` | Audited source contexts and one operational composition retain pure calls, phase order and independently returning reads and reservations |
| `RuntimeProjection`, `RuntimeMembership` | Successful eventful public calls yield the generated result and its answer-set meaning, including the retained subject and negative evidence |
| `RuntimePublicRefusal` | A public stop identifies its reached phase and exact cause; supporting loop receipts retain executed prefixes and partial state |
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
routes. The [reproduction guide](REPRODUCING.md#generated-checker-boundary)
records the supplied-operation and source-context adaptations.

## Observations and trusted contracts

The extracted backend's atomic model reads a supplied fixed Boolean or U64.
The event interpretation instead requests a fresh response at each reached read,
keeping the same owner and field identity. Only the Relaxed loads used by this
code are modeled. Local cancellation is read first, optional slot membership
second, and optional expiry last. The generated membership operation compares
the observed U64 with the token's captured active word.

[`ControlReads`](ControlReads.lean) characterizes both successful and refused
read sequences. `ContextEvents.poll_reads` derives that normal form from the
source-checked contexts. A mismatching slot receipt retains the actual observed
word and stops before expiry; success requires a matching word when present.
`RuntimeControl.representative` clears unused fixed Boolean observations and
sets the fixed slot observation to its captured active word. It preserves slot
presence, captured word, owner handles and deadline configuration. This ghost
choice changes no Rust token and constrains no runtime response.

[`RuntimeEffects`](RuntimeEffects.lean) preserves embedded backend success,
failure and divergence. A reservation response supplies capacity evidence or a
typed refusal, while the logical vector sequence is preserved.
`RuntimeContexts` and `CheckerContexts` replace only named effect/phase calls and
the result monad in audited generated definitions. Exact reconstruction laws and
a source checker verify that restoring the original calls recovers their bodies.
This adaptation is explicit; the generated backend itself has not acquired a new
effect type. `ReferenceEvents` supplies one composition of these contexts.

Successful executions project phase by phase to the generated fixed-observation
checker, preserving outputs, control handles and work. Refused executions retain
the actual reached operation and ordered observation history instead of being
normalized into successful results. The older fixed-token laws and broader
`Trace` relation remain separate: not every refreshed trace is a fixed loop run.

Rust, its standard library, `Arc`, allocator, OS and hardware are trusted to
satisfy their contracts. The immutable handle/field mapping, heap consistency,
physical capacity and library-model correspondence are explicit assumptions.
The compiler, Charon, Aeneas and audited extraction/context adaptations are also
trusted. These proofs do not verify slot generation allocation, compare-and-swap,
window retirement, reference counting, destruction, timers or the platform
memory model. No allocation-success or fairness assumption is
introduced, and there are no project axioms, proof holes or native proof shortcuts.

The covered boundary is a finite completed or refused scalar reference check of
one candidate against a finite ground formula theory. It does not establish
termination of unrestricted iterators, source grounding, candidate enumeration,
optimized checking routes, parallel schedules, GPU execution or whole-solver
correctness. The reusable body-recognition laws extend the general ASP library
without changing its existing semantic definitions.

## Extraction identity and reproduction

The generated types and functions come directly from production Rust, including
the four private subset-search operations, `Theory::new` with its private
admission step and four validators, `Interpretation::new` and its private
`insert_atoms` helper, and the stored-reduct constructor, public query and
reservation wrapper, together with public `oracle::check`, `check_interpretation`
and its decision/accepted-interpretation accessors and consuming conversion.
The tight-body classifier, its work/reservation helpers and node equality are
selected with the exact atomic-choice recognizer, its pair helper and the
single-root tight producer. The selected `Cancellation::poll` reaches the actual
`Membership::is_cancelled` word read and comparison. Slot opening, pulling and
retirement are outside this extraction. The producer copies its root and head nodes to local
values before their unchanged matches, as the shared head recognizer does for
its inspected nodes. These copies preserve reads, short-circuit order and typed
refusals; they introduce no representation, allocation or API change.
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
is_satisfied_by}`, `Interpretation::new`, `check_interpretation`, `tight::reserve`
and `tight::compile::classify`. No body is
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
