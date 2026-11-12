# Connecting proofs to implementations

The reduct is the common semantic foundation. Connecting it to an executable
solver requires several separate arguments; a theorem at one level cannot
silently stand for all of them.

The [solver correctness plan](correctness.md) orders these obligations into
verified milestones. This chapter records the more detailed correspondences.

The relational closure route was removed in 0.7.0. Laws whose only Rust
counterparts were that route's code remain true as statements and have no Rust
counterpart now:
- `FiniteClosure` (the static closure);
- `Bounds` (`Candidates::bounded`);
- `GateRestrictions`;
- `GatePositions`;
- `LazyRounds` (world rounds);
- the `Refinement.PackedClosure` and `Refinement.PackedAcceptance` refinements;
- the static closure transport.

The entries below that describe those correspondences record what the removed code
was required to satisfy. The relational `Program`, `Seed`/`SeedSelection`, gate-token
and carrier owners, and `from_ground_program` translation are also retired. Their
correspondences below are historical implementation obligations, not live API
claims. The general Lean laws, canonical atom/template storage, relations and
formula consumers remain; removing one consumer does not retire a law or verify
a different consumer.

The retained binary `Evaluator` generation supplies the historical full-package
results. The native `Native` generation has separate checked row, admission,
constructor, evaluator, frozen-query, proper-subset search and public membership
laws described below. Its runtime-effect and tight-path migration remains
separate; the two generations are not interchangeable proof records.

| Layer | Existing mathematical or executable object | Additional correspondence required |
| --- | --- | --- |
| Semantic definition | Predicate interpretations, rules, formula trees, reduct and minimality | Relate the intended source language to these definitions |
| Source transformation | Laws over finite bindings, eligible tuples and complete families | Prove concrete cursor coverage, local scope, provenance and translation |
| Evaluation | Least-consequence laws and frozen-mask equivalence | Relate Rust DAGs, dense IDs and packed membership to their denotations |
| Scheduling | Independent-world and completed-batch laws | Prove actual buffers, snapshots, submission identities and commits implement that schedule |
| Machine execution | Checked Rust behavior and qualified WGSL paths | Establish executable refinement, arithmetic and device semantics |
| Observation | Semantic coverage and delivery laws | Connect actual output writes and counters to the retained semantic evidence |

The current Rust formula representation additionally admits native nonnegative
weighted thresholds and ordinal min/max cuts. The retained `Native` extraction
predates those constructors;
its source hashes and replay guard must remain tied to the Boolean generation.
Its theorems therefore do not yet certify the numeric-row Rust entrypoints,
even on Boolean input. Migrating admission, numeric rows, evaluation and their
composition requires new extracted operations and proofs. The general
`ThresholdReduct` laws supply the semantic bridge; they are not that executable
refinement.

## Ordinal extrema

[`OrdinalExtrema`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/OrdinalExtrema.lean)
proves one paired min/max fold, its agreement with selected extrema, carrier
bounds and the prefix/suffix witness laws. The empty minimum is the greatest
rank and the empty maximum is zero. Interior cuts are disjunctions of qualifying
contributions; cuts at zero or immediately above the carrier have explicit
constant meanings. [`RankedExtrema`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/RankedExtrema.lean)
supplies the separate source-order and arbitrary frozen-interpretation laws.

The Rust family compiler uses a guard-independent dictionary of typed values
and both endpoints. The shared
[numeric workspace](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-ferraris/src/numeric_evaluation.rs)
retains one completed reduction per authoritative row during a whole-theory CPU
pass: either a checked weighted sum or a pair of extrema, interpreted by the
row's domain. Reuse requires completed predecessor truth. A complete pass
invalidates every row first. A retained tight evaluation adjusts a valid sum
once per changed contribution and invalidates changed extrema. Ascending node
order applies every changed condition before any guard reads the row; repeated
conditions retain their separate weights. Interrupted updates invalidate their
owning evaluation. `AggregateAssignment.selected_sum_update` proves the integer
identity behind adding newly selected positions and subtracting deselected ones,
including repeated contributions. It does not establish Rust occurrence indexing,
finite-width bounds or interruption handling. Every frozen guard separately
applies its original truth gate.
Admission records whether any numeric node is present, so unused raw descriptors
alone require no workspace. This shape flag, compact slot encoding, completed-only
publication and reset/work/storage receipts are additional Rust obligations;
the laws do not verify them or the changed `Theory` data layout. The
[reading guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/ordinal-extrema.md)
distinguishes these mathematical facts from the remaining proof of the Rust
sort, row identity, packed validity, traversal and memory accounting. Ordinary
GPU formula transport preserves native cut nodes and shared condition/rank
columns. Original truth reduces both extrema once per row and applies each cut,
including its empty endpoints. The earliest nonconstant guard owns those writes;
followers do not overwrite them. The shared dependency level must place every
condition before the owner and every guard before its parents. Frozen propagation
still applies each guard's original truth gate before projecting its witnesses.
Cold row summaries justify constant classification and arithmetic width, then
are released after packing, or immediately when all guards are constant. The
width includes the empty-minimum sentinel, not just ranks present in the row.
Computing `rank_count - 1` still requires exact subtraction before narrowing;
for example, a carrier of size `2^32` has a 32-bit sentinel. Portable limb
comparison, owner/guard coverage, physical addresses,
barriers, atomic observations and resource receipts remain unproved executable
correspondences. The retained extracted
evaluator does not yet cover ordinal nodes.

## Shared GPU row charges

[`SharedRowCharges`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SharedRowCharges.lean)
proves that charging one live row owner plus all guard visits is no larger than
charging a row traversal per live guard. The finite-sum and admitted-prefix laws
retain every common term. In `zetesis-wgpu`'s formula graph, the frozen threshold
sweep uses one shared row charge and the existing guard supplement; extrema
retain their per-guard charge. Repeated condition occurrences and distinct
source-row identities remain part of the authoritative inventory.

[`OriginalRowCharges`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/OriginalRowCharges.lean)
models the minimum of the legacy original-evaluation tariff and one row tariff
plus the inherited guard supplement. Its discount and conservation laws justify
subtracting the resulting bounded discount after the full legacy setup expression
has passed its checked arithmetic. The original evaluation still runs; this
change does not admit supplied truth or remove an execution phase. A row with
no live guard contributes no row execution, and every live guard remains charged.

Both laws require the Rust inventory to authenticate row ownership and retain
all guard and condition occurrences. The inherited tariffs must dominate their
shader operations independently before their minimum is conservative. Actual
packing, owner selection, integer overflow checks, polling order, memory limits,
shader traversal and hardware behavior remain implementation obligations. These
are logical-work charges, not machine instruction counts. Smaller charges may
admit additional sweeps under the same ceiling; that does not establish a timing
improvement or eliminate exact CPU completion of residual regions.

## Ordinary resource policy

`zetesis_solve::Resources` derives ordinary admission, execution and publication
capacities from memory and worker count. It selects no operational ceiling on
mandatory CPU work; checked counter maxima, cancellation and allocation failures
remain. Tight, positive and stratified certificate preparation uses the session's scratch
allowance and remaining search work, with checked population counters. Other
optional fail-open analyses and individual GPU dispatches retain separate bounds.
Materialized source families have a separate memory capacity from cumulative
copying work. The certificate grammar, complete-root coverage and rank or
least-consequence obligations do not depend on these policy limits.

The existing bounded-operation laws remain applicable to explicit library
limits. Removing CLI policy choices does not discharge counter-overflow,
allocation, interruption or liveness obligations. The policy factory and its
storage estimates are outside the extracted semantic checkers; this change
claims neither a proof of total process memory nor end-to-end verification.

## Nonnegative aggregate thresholds

`ThresholdReduct.reference_original` and `reference_frozen` connect the
nonnegative threshold recurrence to the complete-mask aggregate semantics.
Conditions may be arbitrary formulas, and the frozen law holds for arbitrary
interpretations M and J. Direct evaluation retains the original threshold gate
and reduces the weights of recursively frozen eligible positions. Distinct tuple
positions retain their contributions even when their conditions are equal.
`ThresholdPropagation` proves lower/upper sum bounds, mandatory contributions
with repeated condition identities, and the corresponding formula consequences.
These are general laws over complete rows and truth bounds.

`ThresholdPropagation.condition_required_true_of_occurrence` and
`condition_required_false_of_occurrence` allow a conservative deduction using
one occurrence's weight. For concurrent reads, the index type denotes operand
occurrences and actual truth is the physical interpretation lifted to those
positions. Lower and upper observations may differ between positions that alias
the same physical slot; no coherence between those observations is assumed.
`occurrence_le_coefficient` supplies the selected contribution's weight bound.
The forcing laws require compatibility at every position and inclusion or
exclusion of the selected occurrence in the reading used to form the sum.
A later atomic load does not by itself establish that earlier-read premise.
Relating those observations under shrinking domains, preserving occurrence
multiplicity, and realizing natural-number sums without limb overflow remain
Rust/WGSL correspondence obligations.

The ordinary GPU formula and tight checkers retain shared native threshold rows.
Four little-endian `u32` wire limbs preserve admitted nonnegative sums and signed-bound
constants. The complete nonnegative row total, or ordinal empty-minimum sentinel,
certifies arithmetic width; every retained live bound and every intermediate
must fit that certificate. Optional integer-64 operations preserve the same wire
values, with portable carry/borrow operations when unavailable. These width and
instruction selections remain executable refinement obligations. Original-truth evaluation
reduces each authoritative row once and publishes all its live bound results at
the original node IDs; no candidate measure cache is retained. The packing must
choose the earliest live owner, cover each live guard exactly once, preserve
repeated contributions, and retain one dependency level for the whole family.
The general kernel's two linear scans use conservative lower/upper sums; repeated physical slots and
concurrent shrinking domains can weaken deductions. An original-false node stays
false without constraining its children. A residual projects the remaining domains onto a region of semantic atoms.
The host searches that region under the same frozen candidate and independently
validates each witness. Completeness requires every proper-subset reduct
model to survive the completed device sweeps and their projection. The region
coverage laws then apply to the host walk. The connection to actual shader
execution still depends on faithful packing, limb arithmetic, atomic
observations and readback. The physical differential checks coverage on its
finite fixtures.

[`ThresholdResidues`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ThresholdResidues.lean)
separates a compatible sum into known and undecided contributions. A common
divisor of the undecided coefficients confines every completion to one arithmetic
progression; `interval_refuted` and `formula_interval_refuted` exclude a required
interval when that progression misses it. Correlated conditions may make further
values unreachable, so a nonempty intersection is not a satisfaction certificate.
A further divisor reduction preserves any existing progression point; this
justifies ending the gcd scan once its current progression intersects the interval.
`wide_interval_intersects` supplies such a point when a positive coefficient fits
inside the interval’s width; `open_coefficient_intersects` preserves that point for
every divisor of the coefficient. Applying the latter to the first undecided
coefficient can end the coalesced residual scan before a remainder calculation,
without establishing an attainable sum.
The [reading guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/threshold-residues.md)
distinguishes this restriction from full attainable-sum computation.

The original-only Rust check uses existing grouped rows after mandatory node,
bound-transfer, witness and support propagation reaches a fixed point. Separate
coalesced queues keep arithmetic notifications from changing bound-transfer order.
The check may refute a region or force a condition by excluding its other value.
`bounded_interval_refuted` includes the upper reading in the arithmetic exclusion;
`condition_required_true_by_residue` and `condition_required_false_by_residue`
justify the two deductions for every compatible satisfying reading.
Complementary gcds share one suffix vector and forward reduction. Each batch reads
one drained snapshot; unique grouped conditions and deferred event processing
keep preceding publications from changing the remaining reads. Every new wave
fixes an open condition and returns through the ordinary queues.
Before preparing complements, Rust can find one open coefficient fitting the
interval width and a positive partial gcd of the other open coefficients fitting
that width. Every complement except that coefficient's retains it; its own final
gcd divides the observed partial gcd. Together with the drained ordinary bounds,
these witnesses make every branch's lattice test inconclusive. This is a reason
to omit those tests, not evidence of an attainable sum. The arithmetic laws
support this argument but do not formalize the Rust filter. Grouping, gcd
computation, checked arithmetic, snapshot consistency, queue termination and
storage accounting remain executable correspondence obligations. The laws do
not verify this scheduling policy or qualify the retained native extraction or
frozen-reduct propagation.

[`AggregateAssignment.bounded_selected_sum_is_candidate` and
`bounded_sum_interval_refuted`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AggregateAssignment.lean)
justify bounded possible-sum exclusion for nonnegative contributions. Every
selected sum below the ceiling remains represented when each reduction discards
larger values. If these values, added to the known contribution, miss the required
interval, no represented selection satisfies it. Correlated conditions can make
fewer selections possible; an intersection does not establish a model. The Rust
check uses this relaxation only after original-subject mandatory and lattice
propagation leave the row unchanged. Its packed shifts are shared with source
assignment-sum computation. Complete open-condition coverage, grouping, processed
sum correspondence, machine-word shifts, optional-work admission, storage and
interruption remain executable obligations. The fused Rust shift checks only
translated bits for the first interval witness; its early exit discards a
possibly incomplete union and makes no exclusion claim. Proving the witness
probe, carry handling and disposal is separate from complete-union correspondence.
These laws do not verify the Rust implementation or change frozen-reduct semantics.

[`ThresholdDivision.equivalent`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ThresholdDivision.lean)
proves that dividing every nonnegative weight by a positive common divisor and
rounding a positive threshold upward as `1 + (bound - 1) / divisor` preserves
original and arbitrary frozen truth. A zero bound remains zero. Every selected
sum scales exactly, including repeated condition occurrences, so both original
and frozen gates retain their meaning. The existing context laws carry the
result through conjunction, disjunction and implication. Each primitive cut is
rounded independently: equality and disequality use both `k` and `k + 1`.
The [reading guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/threshold-division.md)
explains why rounding the source comparison bound before forming its successor
would be incorrect. Computing the divisor, raw-input validation, signed bound
handling and the Rust compiler's checked arithmetic and work charges remain
separate obligations. This law does not update or qualify the retained native
extraction, whose Boolean-generation source boundary remains unchanged.

[`ThresholdPrefixes.original_implies` and `original_contrapositive`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ThresholdPrefixes.lean)
prove that reaching a nonnegative threshold implies reaching an extended-row
threshold at a no-greater bound, and that failure transfers in the reverse
direction. The prefix contains complete weight/condition pairs, including
repeated contributions. Its separate frozen laws hold for arbitrary M and J;
the [Rust prefix transfer](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-ferraris/src/regions/thresholds/prefixes.rs)
is enabled only for original-candidate narrowing. It passes numeric requirements
through intermediate rows without matching guards, using immutable index links,
worker scratch and existing per-region threshold state. It adds no state to
region copies and leaves original evaluation and reduct construction unchanged.
Recognition covers exact prefixes in prepared coefficient order, rather than
arbitrary subsets. Prefix discovery, grouped-row correspondence, queue execution
and resource accounting remain executable obligations; the law does not qualify
the retained native extraction.

[`ThresholdConnectives.conjunction_original` and `disjunction_original`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ThresholdConnectives.lean)
identify finite Boolean connectives with unit-weight thresholds: conjunction
requires the normalized list's length, and disjunction requires one. The caller
supplies exact membership equivalence between the source and normalized lists;
empty lists and repeated conditions need no extra premise. These laws justify
Boolean sorting and deduplication, not removal of weighted aggregate occurrences.
They are original-satisfaction laws, not a proof of a frozen rewrite.

[`ThresholdCapacity`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ThresholdCapacity.lean)
counts the union of held and necessary conditions without counting overlap twice.
Its lower-bound law permits the recorded held sum to underestimate current truth;
the unit-count corollary requires a duplicate-free carrier. A body is excluded
when its necessary conditions would reach a forbidden threshold. This is an
original-candidate consequence: original producer implications do not supply the
same premise in an arbitrary frozen reduct.

The [Rust capacity projection](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-ferraris/src/regions/thresholds/capacity.rs)
must establish that each projected condition belongs to the unit row and follows
from the body, including the root authority of implication aliases. Grouped row
identity, valid held sums, checked arithmetic, event scheduling and charged reads
remain executable obligations. Pending true events may weaken the lower bound;
the semantic law does not establish their eventual processing or Rust refinement.

[`AssociativeWitnesses`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AssociativeWitnesses.lean)
proves the original truth of associative cuts, distinct-leaf unit consequences,
and witness transfer under explicit coverage or containment premises. A cut may
stop at a compound condition, and separate row conditions may occur in different
cuts of the same formula. These are semantic implications, not certificates for
the runtime traversal or frozen propagation.

The original-candidate bridge reuses existing Boolean chain leaves and counters.
Flat chains use least-condition anchor postings and exact prefix authentication.
Shared chains discover candidates through existing links and authenticate
associative witnesses. Expanded discovery filters bound proposals before retaining
candidates, then checks usefulness again before authentication. Its Rust argument
requires monotonic bounds: a redundant proposal stays redundant through closure.
This does not change the coverage or containment premises. Local parent
roles select ancestor discovery, conservatively retaining compound anchors and
ordinal parents on that route; unrelated sharing does not change flat discovery.
This local eligibility argument remains a Rust obligation, as does the prepared
per-chain bit that skips discovery when no row anchor lies in a chain's
same-kind cone: it must mark every chain whose walk or least-leaf posting can
reach a row. The bridge keeps no
table of all row/chain pairs and adds no state to region copies; its temporary
walks use accounted worker scratch. Enqueueing uses processed operand counters
and frozen gates: unknown roots may relay changes, while a settled root needs
a remaining expanded-witness obligation. Later ordinary root events seed new
obligations. Settled roots omit further possibility notices after those events
have drained. Required-witness
checks stream distinct leaves and stop at an absorbing witness or two open leaves.
Forcing a sole open leaf or detecting none requires a complete walk. Runtime visits
and comparisons consume propagation work; resumable frames read only reached
child edges, retaining distinct-leaf order and abandoning partial walks on refusal.
Coverage checks share that traversal, stopping below an accepted compound
condition or at the first uncovered leaf. Preserving the exact coverage verdict,
visit order, charged reads and reusable scratch on refusal remains a Rust
obligation. Each frozen node keeps its truth gate.
Denoting a chain root by its distinct leaves, authenticating canonical condition
IDs, recognizing unit-row relationships, and
implementing checked bounds, event scheduling and resource accounting remain
Rust obligations. The semantic laws do not establish those correspondences or
qualify the retained native extraction.

Exact disjunction-witness equality is established once in the immutable index.
Fingerprints select candidates for complete equality checks; runtime transfers
truth through the resulting classes, retaining each formula's frozen gate.
Partitioning, member lookup, traversal order, duplicate suppression, interruption
cleanup and storage receipts remain Rust correspondence obligations. The
original-only coverage and containment laws do not establish those properties.

This extends the classical law in `Thresholds`; it does not yet verify source
coalescing, native formula storage, propagation, finite-width arithmetic, Rust
evaluation or device execution.

## Typed extrema families

[`RankedExtrema`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/RankedExtrema.lean)
shows when minimum and maximum can operate on finite integer ranks. Weak order
preservation permits values in an unobserved interval to share a rank; each
declared guard must also preserve its strict and inclusive cuts. The actual
empty endpoint participates in that agreement. `original` and `frozen` derive
aggregate truth for original and arbitrary frozen interpretations, keeping both
the original guard and the guard over recursively frozen eligibility.

The typed compiler uses a stronger condition: its dictionary preserves every
distinct value in the complete row and includes both empty endpoints. Guards
read cuts in that row's order. Preparation checks and reuses nondecreasing input
ASP term order; rows that fail this check use the same typed sorting boundary.
Numeric source contribution order agrees with ASP order, while mixed
storage-identity contribution order can differ. Each guard obtains
both cuts from one search in the unique dictionary. These steps reuse the
existing rank and dictionary buffers. Ranks remain local to the row; they are
not canonical term IDs. Typed comparison, dictionary construction, shared native
storage and resource accounting remain Rust correspondence obligations. The
ordinal laws above connect the selected ranks with witness cuts; they do not
qualify runtime propagation or GPU execution.

## Tight-body classification

CPU tight membership retains its truth and packed support vectors in a
worker-local `TightWorkspace`. A complete check gives every formula node fresh
truth, while conjunctions and disjunctions stop reading operands at the first
decisive value. With the plan's readers index, a later check updates the
retained evaluation instead: it re-evaluates, in node order, exactly the nodes
with a changed input and maintains support counts and the false-root and
unsupported-atom sets. `TightEvaluation.updated_values_exact` proves the
node-table rule: re-evaluating, in table order, exactly the nodes whose inputs
changed and keeping every other node's retained truth yields
`TightEvaluation.values` of the new candidate. The worklist and reverse index
that realize the rule, numeric updates and the maintained support counts and
sets are argued in the source documentation of `TightWorkspace` and its update
and checked by a differential property against complete evaluation; no Lean law
covers them.
The device's update route applies the same rule in dependency-level order, a
permutation of table order that respects every dependency, since each reader's
level exceeds the levels it reads. The level-order permutation, the readers
index, the packed support counts, the settled unsupported and false-root sets,
and the owner's re-reduction of a numeric row, which writes the row's followers
in its own round, are WGSL and Rust obligations: `TightEvaluation`'s node type
does not model row re-reduction. Differential physical tests compare every
update with the scalar certificate and assert each slot's route and receipts.
Which route evaluates a batch, including the runtime guard's choice between the
update and the dense routes, is an execution choice: every route returns the
same verdict.
All original roots and producers remain covered. On the complete route support
is reset before reuse, and the final check intersects each candidate word with
the complement of its support word, returning the first unsupported atom; an
update reports the least atom of its maintained unsupported set, the same
witness. Work counts visited operand prefixes and support words on the complete
route, and the update units `TightCheckLimits::max_work` lists on the update
route. Existing `TightEvaluation` laws still require exact
original truth, complete producers and valid ranks. They do not prove the Rust
workspace's allocation, packing, reset or retained-capacity behavior; those
boundaries have regression checks. General original and frozen evaluation now
use the same decisive-prefix Boolean reduction as tight checking.
`OperandArena.reduce_prefix_exact` proves the reduction's truth law, and
`OperandArena.prefix_reads_le` bounds its operand reads by the complete row.
Every DAG node is still evaluated; only redundant reads of completed child
truths are skipped. The retained extracted general evaluator reads every
operand: its recorded N+E equality applies to that older generation, not this
changed loop. Refreshing the concrete extraction and work correspondence remains
separate from these semantic laws and the differential Rust checks.

`TightClassification.completed_semantics` connects the retained binary generation's
tight-body classifier to `TightBodyRecognition`: successful classification covers every
stored node, preserves its exact formula syntax, and distinguishes bodies with
and without atom occurrences outside negation. Its loop receipt derives the
full class sequence and work count from actual steps. The proof uses ordered
nodes, a valid work counter and the successful reservation's sequence contract;
it assumes neither allocation success nor class agreement. This is the first
optimized-route correspondence, not a proof of the whole tight certificate.
Producer coverage, ranks and membership checking remain separate obligations.
See [the refinement account](evaluation.md#recognizing-bodies-for-tight-checking)
for the fixed-provider scope.

## Stratified normal evaluation

[`AcyclicEvaluation.answer_set_iff`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AcyclicEvaluation.lean)
handles the strict-rank case: every producer reads only lower-ranked positive
atoms and frozen gates. A single complete producer scan per rank equals the
stratified result; constraints then decide answer-set membership. Rust's
`AcyclicPlan` checks the complete normalized source and orders all producers:
its rules and its fact blocks, whose rows it stages required at the block's
position, as the scan computes for a rule with an empty body.
`AcyclicSource` evaluates that order after complete support and original-family
arithmetic admission, retaining the original program and exact interpretation.
Source normalization, rule enumeration, rank recognition, canonical storage and
bounded publication remain implementation obligations. The law proves no Rust
execution or performance claim.

[`StratifiedEvaluation.answer_set_iff`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/StratifiedEvaluation.lean)
derives a unique answer candidate from syntactic rank conditions. Positive
dependencies may stay in a stratum; frozen gates must read earlier strata.
Repeated complete reduct closures settle those strata, giving a unique fixed
point. The existing normalized/Ferraris bridge and constraint-filtering law
then characterize answer-set membership, including arbitrary extra constraints.

Rust `StratifiedPlan` recognizes a narrower normal formula grammar, computes
signed strongly connected components and propagates positive consequences in
that order. The theorem does not yet prove this graph recognition, the formula
translation, or equivalence of the component queue to the staged definition.
Those obligations are distinct from the established mathematical law. Exhaustive
small-theory comparisons exercise the concrete result against general reduct
membership; interruption and storage tests check its bounded outcomes.

## Atomic-choice recognition

`AtomicChoice.atom_meaning_iff` connects the retained binary generation's shared
head recognizer to exact `a ∨ not a` syntax in either order. The proof derives stored-node witnesses,
allows distinct occurrences of the same semantic atom, and decodes an ordered
DAG at a bounded head. The general choice-reduct laws then supply the meaning of
that shape. Complete producer extraction, root coverage and each consumer's
work accounting remain unproved; this result concerns the shared recognizer.
See [the recognition contract](evaluation.md#recognizing-atomic-choices).

## Single-root tight producers

The ten laws in `TightProducerSemantics`, `TightProducerReads` and `TightProducer`
connect the retained binary `tight::compile::producer` call to an exact original
producer.
[`TightProducer.completed_producer`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/TightProducer.lean)
retains the root, head, indexed-body formula and ordinary/choice kind. Facts and
both choice orientations keep their original syntax. `after_classification`
supplies the class-table correspondence from the actual completed classifier,
using its explicit successful-reservation sequence contract.

`completed_none` identifies falsum or arbitrary default negation. The refusal
laws recover an unsupported head before any body-class diagnostic, or an already
recognized head with an unsupported body. Missing reads and backend nonreturns
are not converted to typed admission errors. The Rust producer copies its two
inspected nodes before matching; this preserves its reads and branch order.

These are completed single-root laws, not complete plan extraction. The actual
two-pass root scan must still establish asserted-root membership, occurrence
coverage and count/append agreement. That supplies the premises of
`TightPlans.OriginalProducers`, `Covered` and `TightEvaluation.Represents`;
rank validation and the optimized membership loop remain separate obligations.

## Executable finite algorithms

`ReductEvaluation.values_correspond` proves the actual two-fold Lean algorithm
for original truth followed by frozen-reduct truth. It derives the mask from
`TightEvaluation.values`; the public theorem does not assume mask correctness.
`roots_true_iff` handles arbitrary tested interpretations. The corresponding
Rust loop is `zetesis_ferraris::oracle::evaluate`, used by `FrozenReduct`, but
these proofs do not extract or verify that Rust loop. The separate
[implementation-refinement package](evaluation.md) proves the generated evaluator,
root scan, stored-reduct construction/query, reference proper-subset search and
completed public membership checks under explicit library contracts. Its runtime
projection additionally covers finite successful and refused public reference
calls through audited contexts with returning reads and reservations.
The following unprefixed refinement declarations concern the retained binary
generation. Its runtime composition does not qualify the native implementation.
`FrozenQuery.completed_satisfaction` connects the private query Boolean to Ferraris
reduct satisfaction under explicit stored-mask agreement. `FrozenConstruction`
derives that agreement and candidate retention from the actual producer.
`PublicFrozenQuery.constructed_satisfaction` composes successful public constructor
and query calls without an assumed mask, given represented inputs, ordered nodes
and bounded roots. Construction and query use separate reservation providers,
limits and fixed controls; this does not model an allocator history or assume
allocation succeeds. Neither original modelhood nor a tested-subset relation is
required. `TheorySatisfaction` retains the separate-call original and reduct
results, using the same semantic sources under Lean 4.31.0.

`EvaluationAccounting` derives the exact charged prefix length from every typed
evaluator return. `FrozenQuery.returned_work_bound` composes that receipt with
the actual root scan over the returned work record: limits and subset counts
are preserved, and the query charges at most `N + R` for `N` nodes and `R` root
occurrences. Typed stops remain errors. The broader trace relation permits fresh
control inputs; its traces need not be executions of the fixed-token loop.
The public reduct query checks owner identity before polling or reservation;
reservation refusal remains `Stop.Allocation`. Its fresh query budget excludes
construction work. These stored-reduct constructor/query laws retain their
per-invocation fixed-provider scope; the public reference check has the separate
runtime projection described below. Physical storage and owner-token
correspondence remain trusted library contracts. The finite construction results below supply exact
contents and padding under their stated input and reservation contracts.

`FixedSelection` proves the actual selected-atom loop and entry function. Each
typed return retains the exact visited prefix and work; completion produces the
candidate's atoms in order, with no duplicates. `SubsetCarry` proves actual
packed advancement and population updates under the proper-subset guard. A
completed carry corresponds to one positional increment; a stopped carry retains
its partial state without claiming that increment completed.

`SubsetQuery.completed_reduct` connects the actual per-subset query to Ferraris
reduct satisfaction using the actual original evaluation. `SubsetQueryTotal`
constructs every typed outcome and retains its admission and work accounting.
An admitted query charges one subset even when later evaluation or root scanning
stops; refusal before admission preserves the old output and work.

`FixedSearch.calls_refine` constructs a finite execution of the actual search
from its packed counter invariant. Successful carries increase positional rank;
false queries refute the visited states. It derives proper-subset coverage,
returns a proper-subset reduct model on a true result, and propagates typed stops
without treating them as exhaustion.

`MembershipSearch.completed_answer_set` composes actual original evaluation,
a completed original root check, completed selection and completed search. It
threads work through those phases in source order. Given represented candidate
storage, ordered nodes, bounded roots, matching atom counts, an empty selection
vector and zero-filled packed subset storage, search returns no countermodel exactly
when the candidate satisfies `Ferraris.Stable` for the asserted theory.
The mask's meaning, selected-coordinate coverage and query correctness are
derived, not supplied as oracle equations.

`SubjectMembership.completed_subject` further derives those input invariants
from successful actual theory and owned-vector interpretation constructors, then
recovers the candidate's own check from `check_interpretation`. Its acceptance
accessor agrees with answer-set membership. `completed_stable` follows the
actual consuming conversion and proves that its accepted interpretation retains
the same candidate. `CheckedResults` establishes the accessor and conversion
contracts, including full retention of a rejected decision. These results keep
the fixed-observation and successful-library-value contracts explicit; they do
not assume correct inner evaluation or a supplied verdict's semantic validity.

`PublicMembership.completed_answer_set` recovers those phases from an actual
completed public `oracle::check`: its verdict is `Stable` exactly for a Ferraris
answer set. Its premises are exact candidate word storage, ordered nodes, bounded
roots, a common consistent immutable heap, and empty-sequence preservation by the
successful selected-atom and subset-word reservations. The actual calls establish
owner agreement and initial buffers; Boolean workspaces are cleared before use.
No allocation success or successful inner call is assumed. The negative theorems
identify an asserted false root or the actual program-owned proper-subset reduct
model. `PublicCheckPhases` also identifies the published final statistics;
`PublicCheckBoundary` proves wrong-owner, initial-control and first-reservation
refusals. A public stop returns no partial statistics or membership verdict.

These are completed-result laws for the generated reference checker. Candidate
generation, optimized checking, source grounding and device execution remain
separate. The [reproduction guide](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/REPRODUCING.md#generated-checker-boundary)
records the supplied-operation and audited context-adaptation boundaries.

`PackedSetup.initialized_membership` derives the zero-storage premise from the
backend's actual resize operation and exact candidate word length.
`MembershipVerdicts.returned_witness` identifies the proper-subset reduct model
returned by the actual search. `OwnerChecks` connects generated identity and
clone operations to owner tokens and immutable-heap consistency. These results
supply the completed public composition; the constructor laws below establish
exact packed storage for finite inputs. `SearchFrame` retains the actual theory,
word length, limits and control across typed search returns, with nondecreasing
work and subset counts. `AdmissionValidation` proves the node and root validators
called by `admit` for `Theory::new`; it is described with
`TheoryAdmission` below.

`RuntimeEffects` permits changing read observations and typed reservation failure,
while preserving embedded backend success, failure and divergence. Audited
`RuntimeContexts` and `CheckerContexts` generalize named calls in the generated
bodies; reconstruction laws restore the exact original operations. `ReferenceEvents`
supplies one eventful checker from those contexts. The generated backend's own
result type is unchanged. `ContextEvents.poll_reads` connects the actual polling
context to `ControlReads`: local cancellation precedes an optional U64 slot read,
whose actual value is compared with the captured active word, and then optional
expiry. Exact receipts retain mismatching words and require a matching word for
success. No comparison verdict is assumed.

`RuntimeProjection` derives the same generated public result from a successful
finite event execution. `RuntimeMembership` then proves answer-set equivalence,
actual negative evidence, and retention of a constructed subject through its
accepted-result conversion. No inner oracle, successful allocation or always-clear
read history is assumed. The comparison representative clears unused fixed
Boolean observations and sets the unused fixed slot observation to its captured
active word. It retains owner handles, slot presence, captured word and configured
deadlines, and changes neither Rust state nor runtime responses.

`RuntimePublicRefusal` identifies the actual reached operation and cause of a
public typed stop. The proof preserves local-cancellation, slot-cancellation, expiry and quota
precedence, together with reservation refusal. Supporting loop receipts retain executed
prefixes and partial state. A refused tick charges no new work. The public error contains no membership
verdict or partial statistics. These finite-run laws make no fairness or eventual
response claim. Slot generation allocation, compare-and-swap and window retirement
remain Rust obligations. Rust/std/Arc/allocator/OS/hardware contracts, immutable handle
mapping, extraction and source-context adaptation remain trusted boundaries;
source grounding, candidate enumeration, optimized routes and devices remain
separate proof obligations.

`FiniteMembership.check_iff_answer_set` adds a finite reference subset search.
Its coverage proof constructs a selection representing every semantic subset
of the supplied candidate. Original satisfaction and absence of a proper-subset
reduct model therefore follow from executable definitions. The theorem does
not assume a correct external oracle or complete external proposals. It is
a reusable ASP theorem. The reference-checker refinement above connects the same
semantic definition and packed counter laws to actual operations; optimized
candidate search remains separate.

`FiniteClosure.closure_exact` proves an in-place-order mathematical rule scan,
rather than assuming it equals synchronous iteration. Soundness holds at every
prefix. An unchanged full scan is closed; each changing scan adds a fresh head.
The head-list length plus one gives a constructive scan bound without a finite
ambient atom universe. `accepts_exact` adds the completed constraint and gate
projection checks. Its Rust counterpart, the closure route's `static_oracle::closure`,
was removed with that route; the law remains as a statement.
The finite rule presentation has already discharged ground filters; no source
filter evaluation or grounding claim follows from this theorem.

`Refinement.PackedInterpretations` models zero-initialized 64-bit words, numeric
mask insertion, membership and low/high 32-bit export. `pack_exact` derives
storage length, exact input-set membership and zero padding from the operations.
`export32_exact` then establishes membership and valid word reads at the second
width. Predicate-set equalities connect these representations to the semantic
library. The supplied in-range coordinates are a structural premise, not an
assumption that membership is correct.

The general formula and tight GPU checkers both call the WGSL function
`candidate_contains` in `formula_graph/truth.wgsl`.
`Zetesis.Refinement.PackedInterpretations.candidate_word_read` proves that
in a batch of rows exported at a common width, loading word
`world * stride + atom / 32` and testing bit `atom % 32` reads that world's
membership for an in-range atom. Rows on either side may contain different
candidates. The shared `packed_bit` in `words.wgsl` supplies the word index
and one-hot mask. Formula wakes, tight updates and support, and aggregate
occurrence masks use the same coordinates with their own storage bases.
`Zetesis.Refinement.PackedInterpretations.mask_bit` relates the in-word mask
test to the selected bit. Host tests check which buffers the assembled
shaders read.
Checked device address ranges, successful upload and physical execution remain
separate obligations.

The current `zetesis_algebra::words::Words` storage and borrowed views share the
membership, ascending bit traversal and numeric-half operations used by
`Interpretation`. The width-general `insert_bit_exact` and `clear_bit_exact`
laws establish single-coordinate updates at any positive lane width; the
existing 64-bit laws specialize them. `combine_prefix_exact` models a successful
prefix of common-prefix lane combinations, preserving the destination's shape
and unmatched suffix. Its operation parameter takes the existing bit-vector
OR, AND or XOR operation; the corresponding `BitVec.getLsbD_*` laws give each
lane's set interpretation. The successful-write count is not a work allowance
or a claim about which callback was executed.

`Words::is_subset_of` tests equal-length physical storage with an ascending
NOT/AND/zero-test scan. `PackedInterpretations.and_not_zero_iff_bit_imp` relates
one word's zero difference to bit inclusion; `subset_scan_exact` covers every
paired lane on completion. All stored bits participate. A semantic inclusion
equivalence additionally needs exact carrier coverage and zero padding, not
merely equal physical lengths. The Rust callback refusal boundaries remain a
separate obligation: each visited lane admits its three primitives separately,
and a first missing bit stops before the suffix.

The shared arithmetic `Snapshot::is_ancestor` applies this operation to prior
held/current held, then prior cut/current cut. Under the owners' mask invariants,
these inclusions express `FormulaBounds.Inside current prior`: the upper bound
is the complement of cut. This checks descent, not prepared-system identity.
Detected faults still become `InvalidProgram`, never semantic refutation.

`Region` and the six-span knowledge masks retain their own extents and
allocation boundaries. Wrapping storage does not establish padding, owner
identity or a semantic interval: those remain the caller's invariants.
The representation laws are correspondence targets for these shared operations,
not a proof that the Rust wrapper, its refused-write boundary or every packed
consumer refines them.

This is newly authored Lean, not a fresh extraction of `Interpretation::new`,
`contains` or `words32`. The historical membership extraction retains its own
source identity and toolchain. A current implementation proof must establish
admission, all internal interpretation producers, machine-index bounds,
allocation/control behavior and owner identity. The historical immutable-value
`Arc` model alone does not establish `Arc::ptr_eq`. The word-export result does
not certify a WGSL kernel, device synchronization or readback.

The [reduct](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/reduct-evaluation.md),
[membership](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/finite-membership.md),
[closure](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/finite-closure.md) and
[packed representation](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/packed-interpretations.md)
guides explain these arguments independently of Rust details.

## Indexed reads, counters and packed scans

Four representation algorithms make additional implementation obligations
explicit without changing the answer-set definitions:

- `Refinement.IndexedEvaluation` uses checked reads rather than assigning a
  value to an absent entry. Its prefix invariant proves child availability,
  computed-mask coverage and transport of admitted root bounds. Two checked
  passes followed by root checking preserve Ferraris reduct satisfaction.
- `SubsetCounter` constructs the low-bit-first carry and proves exact proper
  subset coverage. It also proves the maintained population updates and the
  `present < width` guard. Distinct atom positions must denote distinct atoms;
  the theorem states that premise explicitly.
- `Refinement.PackedClosure` initializes 32-bit words directly, selects frozen
  consequence rules, and performs immediate mask updates. Its changed flag is
  false exactly when the whole reference scan is unchanged. Completed iteration
  yields the least reduct closure with exact storage and zero padding.
- `Refinement.PackedAcceptance` checks selected constraints and agreement on the
  supplied gate carrier. Seed admission into that carrier is explicit. Together
  with computed closure, these checks decide the semantic acceptance predicate;
  complete gate coverage then establishes answer-set soundness.

These are proved authored algorithms, not extractions of the corresponding
Rust loops. They remove assumed evaluator, counter and packed-scan agreement
from the mathematical results. The optional reference-checker refinement above
supplies the selected-atom and packed-counter correspondences under its stated
models; it does not cover the normal-closure checker or public ownership and
allocation. In particular, the final carrier comparison cannot itself
establish seed admission. In the retired normal-closure route, the seed
constructors and their mapping into `GroundProgram` were responsible for that
condition. Those APIs have no current executable counterpart; the structural
premise remains part of the mathematical checker.

The [indexed-evaluation](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/indexed-evaluation.md),
[subset-counter](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/subset-counter.md),
[packed-closure](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/packed-closure.md)
and [packed-acceptance](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/packed-acceptance.md)
guides describe the invariants, completion arguments and remaining boundaries.

[`Refinement.OperandArena`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/OperandArena.lean)
adds a separate finite-connective layer. `validate_exact` checks the complete operand
span and every backward reference; `original_exact` and `reduct_exact` compose
its ordered child reads using the existing finite conjunction/disjunction laws.
Repeated operands and empty identities are included, and the tested interpretation
need not be a subset of the outer candidate. Nonempty groups introduce no identity
leaf: `formula_pair` preserves the exact binary syntax used by class certificates.
`frozen_exact` additionally requires
the current node's stored bit to equal its original truth. Missing reads remain
incomplete.

[`Refinement.OperandTable`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/OperandTable.lean)
composes those checked layers into an original pass and a paired original/reduct
pass over an admitted topological table. The latter computes each original mask
bit before using it, so mask agreement follows from the operation. This
mathematical paired schedule differs from the runtime's separate-pass schedule.
The native extracted laws below establish the separate-pass result through its
actual completed calls. Allocation capacity, live ownership, runtime observation
histories and GPU execution remain separate correspondence obligations.

[`NativeRows`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/NativeRows.lean)
checks the actual generated native row getters against their imported machine
and sequence models. Inline pairs and checked wide spans retain operand order
and repeated IDs. For raw input, short wide arity returns `Arity` before any
range check, unavailable or overflowing spans return `Span`, and absent node
indices return `Edge` before node decoding. The paired-parts view preserves both
sequences but does not validate occurrence metadata.
[`NativeRowsArena`](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/NativeRowsArena.lean)
converts a successfully read wide row to the general arena's natural-index list;
backward references remain an explicit premise. This establishes the arena
validator's returned row. The native admission laws now supply the backward
reference premise from actual complete validation; the row bridge alone does
not assume it. The [package account](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/README.md#native-operand-row-getters)
records the isolated namespace, provider binding and remaining model boundary.

`NativeAdmissionValidation` and `NativeBoundsValidation` prove the actual ordered
node, child and root scans. `NativeAdmittedData` composes all admission phases,
including checked occurrence counting, physical arena and logical operand
limits, atom padding and exact replacement of the input occurrence cache.
`NativeTheoryConstruction` derives actual successful phases from the constructor
return under the explicit allocation-value contract, retaining typed refusal,
nonreturn and per-invocation fresh-heap boundaries. These are laws of the retained
generated `Native` operations. The refreshed extraction and its [native source record](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/native-provenance.json)
include the shared raw-admission dimension helper; a successful Lean build alone
does not establish source identity. The consuming `TheoryAdmission` path's retained topology frontier
is a separate concrete invariant, not a premise supplied by the raw constructor
proof.

`NativeLoop` proves finite actual evaluation with a complete truth prefix and
partial-node work on refusal. `NativeExecution.completed_original` and
`completed_frozen` connect completed actual calls to the decoded formulas and
their Ferraris reducts. The second pass uses the first actual output as its mask;
semantic mask agreement and a tested-subset condition are not assumed. Completed
work is N plus the decoded operand-occurrence sum E. Structural validity and
represented packed words remain evaluator premises.

`NativeRootScan` connects the actual ordered root scan to the first false root
or complete satisfaction, retaining work already spent when a later tick stops.
`NativeFrozenConstruction` derives the original mask from actual construction;
`NativePublicFrozenQuery.constructed_satisfaction` connects public construction
and querying to Ferraris reduct satisfaction. Neither candidate modelhood nor a
tested-subset relation is assumed. Query work is bounded by N+E+R, counting root
and operand occurrences. `NativeAdmittedQuery` derives topology, root coverage
and recomputed N+E from actual theory admission before composing those calls.
Successful-library value preservation remains explicit.
`NativeInterpretationStorage.completed_vector_queries` derives packed
readability, exact storage and zero padding from actual owned-vector construction,
under the successful reservation's empty-sequence contract. It also establishes
the actual membership answer for every machine coordinate. Generic iterators
instead require a finite trace of their actual next calls. Changing runtime
observations remain a separate composition obligation.

`NativeAdmittedQuery.constructed_vector_satisfaction` composes actual theory
and interpretation construction with public frozen construction and querying.
It derives representation invariants at each boundary. `NativeFixedSelection`
supplies the actual ordered selected-atom scan and its visited-prefix receipt;
`NativeSubsetQuery` and `NativeSubsetQueryTotal` supply one actual query's
reduct meaning and bounded work, including typed stops.

`NativeMembershipSearch` adds actual packed-counter progression and complete
proper-subset coverage. A true return identifies the returned countermodel; a
completed false return excludes every proper-subset model of the frozen reduct.
Its `completed_answer_set` composes actual original evaluation, root checking,
selection and search into the answer-set definition. It still requires the
public wrapper to establish ownership and empty subset storage.

`NativePublicCheckPhases` derives the actual calls and published verdict from
a completed public check. `NativePublicCheckBoundary` preserves the order of
early refusals, and `NativeCheckedResults` retains the exact checked subject.
A stored verdict alone is not evidence of answer-set membership.

`NativePublicMembership.completed_answer_set` composes the recovered calls into
Stable exactly when the candidate is an answer set. The corresponding negative
laws establish a present false asserted root or the actual returned proper-subset
reduct model. Common-heap owner consistency and successful reservation's
preservation of empty sequences remain explicit.
`NativeSubjectMembership.completed_stable` derives native structure and packed
storage from actual theory and owned-vector interpretation construction, then
composes the actual subject check and consuming conversion. Its returned
interpretation is the same constructed subject and an answer set of that theory.
These laws use fixed observations and supplied pure library operations; they
neither assume allocation succeeds nor prove changing runtime-effect projection.

`NativeTightWork` and `NativeTightOperands` establish the tight classifier's
actual inner-loop work and typed refusals. A successful row visits every child
occurrence; an interrupted row retains only its charged prefix. Every extracted
tick predates metered polling: the extracted `tight::Work::tick` and
`oracle::Work::tick` poll before every charged operation, as the recorded source
did, while the current ticks observe control only at metered boundaries and
before a limit refusal. These laws, `NativeControl`'s tick laws and the typed-stop
receipt of `NativeLoop` therefore describe the recorded generation, not the
current control cadence. The class fold
uses the existing body recognizer. The native outer classifier, exact syntax
recognition and producer composition remain separate obligations.

The native production control path borrows a `CancellationPoll` view. The owning
poll delegates through that view, and a metered operation's meter retains it
between operation boundaries. The borrowing conversion preserves the represented flag references
and slot identity under the same fixed per-invocation observation contract.
This correspondence does not extend to concurrent atomic behavior, Rust borrow
lifetimes, timers or eventual response to changing control flags.

[`ChargeCadence`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ChargeCadence.lean)
models a charge as a half-open range of zero-based operation numbers.
`dueRule_iff` equates the remainder-based decision with the presence of a
period boundary. `observes_next_multiple` proves that a sequence reaching the
next boundary observes there or earlier, fewer than one positive period of
charged operations away. Zero charges admit nothing. The Rust `poll_due`
operation uses the same rule with period 256; machine-width correspondence,
actual observations, exit settlement and concurrent control remain separate
obligations. This is no bound on elapsed time or unmetered work.

## Iteration and completed search decisions

`Refinement.PackedWordIterator` connects individual checked half-word reads to
the packed export. It proves cursor progress, exact remaining output and fused
exhaustion. Composing construction and iteration derives membership and zero
padding without an output-agreement premise. Rust's byte conversions, borrowing
and machine indices remain separate implementation obligations.

`Refinement.PackedWordIterator.SetBits.least_clear_suffix` separately proves
that a nonzero lane's trailing-zero count selects its least true coordinate,
and that complement-mask clearing leaves precisely the greater true
coordinates. `BitVec.zero_iff_eq_false` supplies the zero-lane exhaustion
criterion. These are physical set-bit laws, not the half-word export theorem.
`SetBits.lowest_bit_arithmetic` identifies fixed-width `x & (x - 1)` clearing
and `x & -x` isolation with the same least-bit mask. This closes the mathematical
arithmetic identity, including modular negation; it does not identify Rust
intrinsics with Lean primitives. Composing repeated cursor steps and preserving
machine bounds, effectful callbacks and refusal order remain refinement
obligations.

`Refinement.PackedSubsets` derives distinct selected coordinates by scanning
finite packed truth. Its set/clear operations preserve correspondence with the
selected set and its population, along with storage shape and padding. Their
proved sequence covers every proper
subset and no other interpretation. The final mathematical checker therefore
decides answer-set membership directly from packed candidate truth.

`Refinement.CounterSearch` proves a complementary control property: compute the
original truth mask once, stream the counter states, and stop on a countermodel.
Every completed verdict agrees with answer-set membership, even under a query
allowance smaller than the complete enumeration bound. An unfinished result
establishes neither acceptance nor rejection. This allowance is not Rust's work
budget, and the theorem does not model cancellation or allocation failure.

`Refinement.PackedCounterSearch` composes these results. It retains one packed
subset and population, queries the original computed mask, and carries only
after a completed false query. `search_refines` proves equality with the
positional stream at every allowance; `completed_check_iff_answer_set` proves
both positive and negative completed verdicts exact. Candidate construction
derives storage and padding, while selected-coordinate production supplies
uniqueness and coverage. No visit list or caller-supplied oracle agreement is
needed. This composes the authored Lean algorithms independently of Rust. The
optional reference-checker refinement above separately proves actual reads,
updates and control flow under fixed observations, then composes public ownership,
reservation and returning-observation contexts under the contracts stated above.

`Refinement.TheoryAdmission` checks dimension limits and padded atom count,
then every node, then asserted roots. `validate_exact` derives the DAG, atom
and root premises from successful checks, and `scan_refusal_exact` identifies
the first refused node of a refused scan. `validated_reduct_exact` supplies
those premises to checked evaluation. The supplied host maximum also bounds
word counts and live export-cursor increments. This is an authored validator
over represented natural-number lengths.

`root` and `rootScan` name the root check; `validate_phases` shows that
validation is the dimension check, then `scan`, then `rootScan`, each stopping
at its first refusal.

For the retained binary generation, `AdmissionValidation` proves that the generated node
and root validators called by `Theory::new` compute `scan` and `rootScan`
exactly, for every represented slice. The generated loops always return, and a
node refusal reports the error of the first refused node. `accepted_structure`
derives the evaluator's ordered-children and bounded-root premises from both
acceptances, as applied to the constructor's input vectors. `AdmittedData`
proves the generated admission step that `Theory::new` runs before allocating:
it computes `validate` with the host's `Usize.max`, including Rust's checked
padded-count addition, and returns the supplied vectors unchanged.
`admitted_program_structure` derives the evaluator's premises for any theory
whose stored value is admitted data. Those premises remain premises of the
composed membership theorems. `TheoryConstruction` supplies them from the
generated `Theory::new` return under an explicit allocation-value contract.
It also preserves the exact admission refusal and separates it from allocation
nonreturn. The wrapper's allocation parameter is an audited binding adaptation;
its body is unchanged. A per-invocation fresh-heap contract supports ownership,
without claiming that one pure provider models repeated fresh allocation.

`InterpretationConstruction` proves the generated interpretation constructor's
phase order and retention of the supplied theory. A successful constructor return
derives successful reservation, resize, iterable conversion and insertion calls;
its words are exactly the insertion helper's returned slice. Reservation refusal
precedes iterable conversion, and insertion refusals pass through unchanged.
The helper takes an atom bound and mutable words while the constructor owns
allocation and publication. Generic iterable and iterator operations remain
explicit in the generated code.

`UsizeCeiling.word_count64` connects the modeled standard-library ceiling division
to the shared packed word count for every host unsigned value. `SliceInsertion`
relates checked scalar and slice operations to shared packed insertion.
`InsertionLoop.insert_exact` proves the actual helper terminates with sufficient
word storage and a finite sequence of actual next observations, writing exactly
the prefix before the
first invalid coordinate and returning `Atom` there. Word count is preserved;
duplicates, arbitrary order and nonfused iterators are permitted.

`InterpretationStorage.completed_pack` derives retained theory, valid input
bounds and exact packed contents from actual constructor success. Its library
contract says a successful reservation preserves the empty input; no allocation
success is assumed. `packed_queries` supplies zero padding, exact storage and
actual membership answers. `VectorInput` derives the finite observation contract
from the imported owned-vector iterator, so `completed_vector` needs no supplied
trace or validity premise. `InsertionBoundaryExample` checks first-invalid
refusal before a diverging tail and completion at the first None.

These results do not establish unrestricted iterator termination or expose a
terminal iterator state that the helper does not return. The runtime public-check
composition uses these constructor results without asserting a physical allocator
implementation or termination of unrestricted iterators.

The [iterator](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/packed-word-iterator.md),
[packed-subset](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/packed-subsets.md),
[streaming-search](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/counter-search.md),
[packed-streaming](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/packed-counter-search.md)
and [admission](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/theory-admission.md)
guides explain the arguments and remaining boundaries.

## Keyed constraints and checked source arithmetic

`KeyedConstraints.one_value` and `asked_constraints_preserve` justify replacing
constraints that reject the same answer sets under an established keyed-relation
property. `digit_carry_asked` proves the digit/carry equation over mathematical
integers with truncating division. These laws do not establish the source
compiler's type checks, `i32` arithmetic, evaluation reachability or diagnostics.

The Rust recognizer in `formula_keys` requires an independent interval proof
for every arithmetic operation of the original constraint, including retained
body terms. Fact-only relations or fact-only conditions of keyed values provide
numeric bounds; source comparisons do not. Every intermediate must fit `i32`,
and the digit/carry dividend must be numeric. An incomplete or stopped proof
leaves the checked source constraint in place. This protects both required
refusals and the exclusion of substitutions where no arithmetic is reached.
The concrete interval analysis and its connection to all possible source
bindings remain Rust obligations, covered by paired rewritten/unrewritten
tests; they are not an executable Lean refinement.

The rewrite also needs an unambiguous original statement. The compiler's owner
map associates a canonical analyzed carrier with its original `StatementId`
only when each original emits one carrier and distinct originals do not collapse
to the same carrier. Other constraints remain unchanged.
This permits the same rewrite for parsed and constructed programs; provenance
supplies diagnostic evidence, not identity. The owner map and replacement step
must establish this association before applying the contextual preservation law.

The fixed-witness source specialization uses the same contextual replacement
law. A complete all-producer fact certificate makes the selected block true for
exactly its matching tuples on every answer set of the unchanged rest. Projecting
whole tuples onto the block variables still read by the residual body then preserves
whether some constraint fires. The
[`FixedWitnessConstraints.bodies_fire_together`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FixedWitnessConstraints.lean)
law states this step under fixed-condition and exact projection premises;
`KeyedConstraints.asked_constraints_preserve` supplies answer-set equality.
Neither law establishes the Rust recognizer's producer coverage, scope analysis,
typed substitution, original-owner mapping, constructor validation or charged
failure behavior. The implementation conservatively retains unsupported or
unfinished families and introduces no new semantic atoms.

## Arithmetic families

[`ArithmeticFamilies`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ArithmeticFamilies.lean)
specifies admission from the classified outcomes of a complete substitution
family. Empty joins are admissible and silent. A defined but false substitution
is a valid definedness witness. Fatal evaluation failures cannot be rescued by
other substitutions. Reordering or revisiting the same outcomes preserves both
admission and warnings.

The counterexamples state two required boundaries: an unfinished prefix cannot
establish that a family is entirely undefined, and combining families from
different outer bindings can conceal a local refusal. Rust must establish
complete original-family coverage, joint expression definedness, correct
zero-divisor classification and those local scopes. Normalized fragments must
preserve their original family identity; sharing a diagnostic span is not
enough. These laws do not certify the source walker, its work limits or checked
arithmetic. An arithmetic fault and a missing positive fact are different objects.

The Rust [whole-family omission](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground/arithmetic/certified.rs)
uses a stronger implementation premise: every normalized sibling is flat, its
scalar expressions are total on completed positive columns extended by checked
body assignments, and every body and head key is covered by those inputs.
Head suffixes may copy values or construct terms from covered inputs, retaining
ordinary construction limits; arithmetic head expressions decline. It can then omit a scan
that would discover neither an undefined instance nor a fatal diagnostic. The
proof attempt does not construct a definedness witness or claim that a join is
nonempty; empty required columns and any unproved sibling retain the ordinary
whole-family traversal. `ArithmeticFamilies` does not prove this Rust classifier,
its original-family association, scalar/key coverage, temporary storage or
failure behavior. Those remain correspondence obligations alongside the existing
column-totality transfers described below.

[`ScalarArithmetic`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ScalarArithmetic.lean)
specifies checked scalar values and faults, and its `evaluate` plan stops at the
first fault. The source evaluator shares the scalar operations but has an
additional traversal policy: after a numeric zero divisor it checks independent
DAG branches within the reached evaluation phase, refuses a fatal error in one
of them, and skips operations whose
operands are undefined. The strict first-fault plan does not prove that traversal
or its fatal-error precedence. `ArithmeticFamilies` starts after each instance
has been correctly classified; neither module proves the concrete classifier.
Body-before-head and condition-before-consequent staging remain separate Rust
obligations: omission in an earlier phase does not invoke a later phase merely
to search for another fault.
Closed-term preparation and post-solve observations retain strict evaluation.

## Ownership and execution correspondence

An ownership refactor can preserve the semantic theorem statements. Its proof
obligation is then to preserve the denotation and evidence through the changed
representation or transition, rather than to define another answer-set relation.

The formula grounder's consuming phases retain atom/node/root order, origins,
activated objectives and the original cumulative budget. Discarded support and
construction indexes have no later semantic consumer. The ID-only atom catalog
requires lookup soundness, lookup completeness and stable insertion IDs under
full typed equality. The dictionary laws below express the lookup contract
abstractly; checked typed comparison, AVL ordering and balancing, the bounded
direction record and its exclusive-tree replay, transactional publication and
fallible Rust allocation remain executable obligations. The Rust lookup module
states the height bound and replay argument; the abstract dictionary laws do not
prove that concrete AVL representation.

`SourceContributions.chain_unique` identifies the ordered observations of a
finite chain in shared storage. `chain_preserved` shows that changing other
entries leaves those observations unchanged. The append-log laws retain each
atom's producer order, including duplicate occurrences, and isolate contributions
to distinct atoms. The formula grounder's shared metadata must additionally
establish valid owner-local links, exact iterator lengths, sorted unique source
locations and correspondence between producer chains and the append log.
Those Rust mutation, allocation and accounting obligations are not proved by
the abstract chain laws. Preserved producer and origin sequences are consumed
at the same support-guard construction phase; the original theory and reduct
remain the semantic authorities.

Device context identity, logical subject identity and submission identity remain
separate. Sharing a context changes resource and failure ownership, not a
candidate's frozen reduct. A busy refusal starts no device query. Context-wide
invalidation prevents a later primitive from reusing failed execution state.
`LazyRounds` and `BatchAccounting` supply the existing semantic round and
coverage laws; context leases, scope cleanup and their concrete Rust transitions
remain executable correspondence obligations.

Ordinary sessions can retain the same device through explicit execution
resources. Each start creates a fresh candidate stream, budget and outcome over
its original logical subject. The correspondence must preserve that independence under
sequential reuse, CPU execution with supplied device resources and shared device
failure.
The builder changes request composition; it does not change candidate coverage,
the frozen reduct or the point at which a checked result is committed.

`zetesis_solve::SessionBuilder::collect` consumes a fresh request, selects the
unrestricted original family and retains each checked answer once in a native
`zetesis_solve::WorldView`. Its empty family establishes inconsistency.
`WorldViews` requires original-answer coverage, completed accounting and complete
capture; its prefix law applies to interruptions and retention refusals. A
selected optimum or the remaining suffix of an already consumed session cannot
replace that family.

[`AxisFolds`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AxisFolds.lean)
states the two rules of a reduction along an axis once. A commutative monoid's
fold is the same over any permutation of its members (`Reduction.fold_perm`),
and once the accumulator reaches an absorbing element no further member changes
it (`Reduction.fold_decided_prefix`). For an idempotent reduction, a list of
answer sets whose fold reached the absorbing element decides the fold of the
whole world view (`world_view_fold_decided`), and any two lists that represent
the world view fold alike (`world_view_fold_complete`). The laws concern lists
of values. That an implementation folds only checked answer sets, that its
accumulator computes the named reduction's operation, and that it tests for the
absorbing element exactly are implementation obligations, as is the
`WorldViews.Represents` premise of a complete capture.

The shared Rust value operations in
[`zetesis_algebra::reduction`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-algebra/src/reduction.rs)
include the sealed, statically dispatched `Reduction` implementations `All`, `Any`
and tuple products. These correspond to `AxisFolds.all`, `any` and `pair`:
conjunction has unit true and absorber false; disjunction has unit false and
absorber true; a product combines both coordinates and absorbs when both do.
These operations contain no answer-set, witness, allocation or control policy.
Their laws concern values, not a reordering of fallible effects.

The native session's world-view questions
([`world_view/fold.rs`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/world_view/fold.rs))
use `All` for `known`, `Any` for `possible`, and `(Any, Any)` for the query's
not-true/not-false flags. Only checked answer sets of the original input reach
the accumulators. The request selects all answers with full identity and clears
the requested answer limit, so objective selection or projection cannot narrow
the questioned family. Cautious and brave consequences still use their existing
fallible Model intersection and union. First-value seeding distinguishes no
answers from one empty answer; the intersection's mathematical unit is the
universal set, not an empty Model. Brave's universal-set absorber is not tested,
so brave consequences still require exhaustion.

The shared `Verdict` separates `Absorbed`, `Complete`, `Empty` and `Undecided`.
Solve projects these into its existing `WorldViewVerdict::Decided`, `Complete`,
`Inconsistent` and typed `WorldViewDecisionFailure`, retaining the original input and
session evidence. A step publishes absorption only after its fallible witness
retention succeeds; an absorbing flag left by a failed step is not a decided
result. Accepted absorption remains decisive after the session is stopped.
Otherwise a value or empty-family verdict requires unrestricted exhaustion.
`Verdict::completed` classifies an optional value only under that caller-supplied
coverage premise; it does not establish membership or completion itself.

`Algebra.FoldVerdict` and `foldVerdict_absorbed`, `foldVerdict_complete`,
`foldVerdict_empty` and `foldVerdict_undecided` describe these four cases. The
mathematical absorbed witness field contains the whole emitted list. Rust keeps
one Membership witness, one or two Query witnesses, and no cautious witnesses.
The correspondence therefore concerns the reduced value and its accepted
history, with compact witness selection as a separate obligation; it is not
literal equality of witness fields. Source identity, coverage, typed stops,
allocation failures and receipt ordering remain executable obligations. The
complete-family and query conformance tests check concrete readings; the general
laws do not prove the Rust implementation.

`zetesis_algebra::reduction::sets` supplies occurrence-stream refinements of
intersection and union. `intersection` selects faithful membership records;
`union` preserves left-then-right occurrences, including duplicates; and
`ordered_union` owns the canonical two-input merge with left representatives on
equality. `Model` remains the sole retained set and owns catalog coordinates,
reservation, copying, typed failures and publication. Its fallible construction
does not implement the pure fixed-value `Reduction` contract.

`AxisFolds.SetStreams.selected_inter`, `append_union` and
`ordered_union_value` relate decoded finite occurrences to the existing set
operations. `ordered_union_equal_left` preserves the exact left representative
on a tie. `ordered_union_sorted_unique` proves that the decoded output is
strictly ordered and contains each atom at most once when both inputs are
strictly ordered and the comparator follows the same transitive, irreflexive
order on decoded atoms. Rust must supply faithful membership reads, comparison
results and iterator advances. Cursor read order, unread suffixes after failure
and successful Model materialization remain concrete correspondence obligations. No set law commutes
allocation or failure effects. First-answer seeding, cautious empty absorption
and brave exhaustion retain their existing coverage and verdict contracts.

`CheckedSum` shares checked count addition across search counters, scored
incumbents, optimal ties, hybrid core answers and terminal base answers. Its
value is an exact `Some(n)` or overflow `None`, with identity `Some(0)`.
`AxisFolds.BoundedSum` gives the corresponding bounded nonnegative sum:
`op_assoc`, `op_comm`, `zero_op` and `overflow_absorbs` state its value laws.
At the bound `u64::MAX`, every representable exact Rust value is in that
carrier. Rust's checked addition must refine `BoundedSum.encode`; the laws do
not verify machine arithmetic or the adapters' failure order. Overflow remains
each caller's existing typed failure and is never successful early absorption.

`AlgebraicRows.bounded_counting_fold` and `bounded_counting_fold_one` relate
this bounded operation to natural sums and occurrence counts;
`bounded_counting_overflow_iff` characterizes the overflow boundary. A family
cardinality additionally requires complete coverage and distinct identities.
`SemanticOutcome::complete_answer_count` retains those existing selection and
coverage checks through a private `Verdict`: zero is Empty, a positive exact
count is Complete, and missing coverage or pruned optimal ties is Undecided.
The count includes verified queued answers and unretained optimal ties, not
merely published or retained models. Counts, typed failures and their mutation
order remain owned by the existing sessions; no alternate enumeration or
membership operation is introduced.

`Minimum` is the built-in optional-value minimum under a pure, allocation-free,
total order consistent with value equality. Rust's `Ord` bound does not establish
these premises. Solve uses a private borrowed `ScoreValue` that delegates to
`Score::compare_costs`, leaving public Score equality, priority slots, objective
presence and payload unchanged. `Minimum::step` returns both the selected value
and its incoming comparison: no comparison for the first incumbent, one for each
later decision. Ties retain the earlier representative. The existing retention
and typed failure effects still occur after comparison at their original sites.

`AxisFolds.Minimum.reduction`, `choose_assoc`, `choose_comm` and
`reduction_idempotent` establish the value laws under explicit order premises.
`step_equal_retains_current` describes the tie representative;
`step_value_map` relates a selected representative's denotation to the optional
minimum. For Score, that denotation is the finite-support priority-cost function,
with missing slots read as zero. The law does not identify structurally different
Scores or prove the sparse Rust comparator. Comparison purity, the comparator's
agreement with that denotation, exact retention effects and bounded-search
completion remain implementation obligations. An incumbent minimum never by
itself establishes optimality, and the runtime instance reports no early absorber.

`SemanticOutcome::optimum_proved` projects a private minimum `Verdict` from the
existing retained score and completion evidence. Complete borrows that score;
Empty requires exhausted relevant search with no verified model; otherwise the
minimum is Undecided. There is no minimum absorber or new optimum witness.
An exhausted strict pass may establish Complete cost evidence even when its
later tie pass stops, while the exact-family count remains Undecided. Typed
stops and receipts stay in the original outcome. The classifier and public
query remain const without changing the generic completed-value constructor;
this classification does not prove the bounded search's coverage premises.

The facade re-exports the upstream `zetesis::query::WorldView`, a nonempty live
stream that may still be incomplete. Its materialized `Snapshot` is complete
and nonempty. These are the original upstream types; type identity supplies no
proof connecting their construction or readings to the native complete family.
`zetesis-engine` implements that backend contract using native CPU sessions.
Its preservation of full answer sets, display policy and completion evidence is
covered by executable conformance tests. A Lean correspondence for the adapter
and query readings remains to be established.

`ProjectedAnswers.covered_key_image` identifies the key image represented by a
selected family. `selected_property_survives` preserves answer-set membership
and established optimality because representatives remain selected original
answers. These laws assume a fixed key and complete representative coverage.
`retain_covers` and `retain_unique` establish the coverage and uniqueness
invariants for one exact identity decision over an already consumed prefix.
They do not prove source `#project` compilation, concrete history lookup,
allocation, cancellation or receipt updates. Native `zetesis_solve::WorldView`
collection retains full identity and does not use projected representatives as
a complete family.

A shared `GpuFormulaProfile` retains one exact compiled pipeline, context and
gate projection. Each oracle starts with fresh residency, epochs and execution
counters. This does not change the masked satisfaction or frozen-reduct laws.
Pipeline identity, capability checks, initialization and device execution remain
Rust/WGSL correspondence obligations; existing Lean theorems do not certify
those handles or shader compilation.

The GPU transport derives complete native operand rows from the admitted paired
graph and keeps them beside fixed node headers in one immutable buffer. Its
correspondence requires complete ordered reads, checked header/tail offsets,
all-child dependency depths and exact theory ownership. The formula kernel's
native AND/OR transfer narrows a forced witness only when its possible
occurrences share one physical slot; all-child forcing visits the full row.
Preserving satisfying completions under stale atomic observations is the
required relation invariant. `GateProjection`'s exact three-position laws cover
binary gates, not this native reduction. `OperandTable` supplies a mathematical
original/reduct table semantics; neither module proves the concrete packing,
native shader transfer, barriers, work receipts or device execution.

The `zetesis-solve` boundary owns semantic sessions and outcomes. The CLI
publishes their checked values through the same public API. Its error adapter
moves the original cause and evidence; a presentation value cannot create a
solver conclusion. Crate extraction changes neither the mathematical subject
nor the required preservation laws.

Typed execution observations describe attempted setup or execution choices.
They are not membership or coverage receipts. A failed observer can stop driving
the session but cannot reinterpret its error as a device failure, request
fallback or establish a conclusion. The observation boundary must preserve
already checked evidence and distinguish it from publication, as required by
`Outcomes`. Tests exercise these failure paths; they do not close the formal
refinement.

The ordinary driver's semantic snapshot is the authority for membership,
completion and incumbent evidence. Publication acknowledgements and timings
remain separate; successful and partial reporting values are derived views.
The refinement obligation is that every projection retains the snapshot's
conclusions, including absent completion, and cannot feed fabricated conclusions
back into it. `Outcomes` already distinguishes established membership, complete
search and delivery. Removing redundant mutable copies does not turn these laws
into a proof of the concrete Rust projection or writer behavior.

The streaming `AnswerRenderer` boundary consumes a borrowed interpretation,
evaluated `#show` channels and objective score. Its controller acknowledges an
answer only after the callback succeeds and fixes the terminal callback stage
once per invocation. Human and JSON encodings share that controller; custom
views do not participate in membership checking. These are concrete publication
contracts under the same `Outcomes` obligations, exercised by Rust regression
tests. They add no end-to-end refinement theorem.

The [ownership chapter](../architecture/ownership.md) and
[session example](../rust/sessions.md#reuse-and-identity) connect these obligations
to the maintained implementation.

The retired relational CPU source join borrowed bound values from its immutable
relation snapshot. The following describes its historical ownership obligations.
Backtracking clears slots; a ground instance lends checked keys over those
bindings, and retained consequences select canonical identities. This changes
storage ownership without changing the substitution used by whole-tuple
matching, filters or frozen gates. Each live binding must denote its matched
source value until that branch ends, and consequence admission must preserve
full typed identity. Rust lifetimes prevent the snapshot from being invalidated
while those references are live; identity and join tests check the concrete
behavior. This does not establish source-join coverage or a Lean-to-Rust
refinement.

The formula join lends its completed partial frame to both support generation
and rule instantiation through the same `Binding` interface when no generator
needs ownership. The relevant existing
laws are `BindingScopes.readAll_agrees`, `AtomKeys.tuple_agrees` and
`TableBindings.join_family_preserved`: equal checked reads and equal ordered
join-step families preserve typed atom identities and the joined family. The
implementation must establish those premises while delaying the final-depth
undo until the borrower finishes. Owning continuations still materialize before
filters; lending does not copy scalar payload or record an owned snapshot.
Rust lifetimes, pending-undo sequencing, substitution/work ceilings and the
ordering of resource refusal versus source diagnostics remain executable
obligations, covered by cursor, ownership, bounded-prefix and source tests. No
new theorem or existing law certifies the concrete cursor state machine.

In that retired relational route, gate and consequence membership used a
checked `AtomKey` over the borrowed assignment. The following storage and charge
description is historical; `AtomKey` and its general identity laws remain. The key denotes the same signed predicate and complete typed tuple
as materialization. Gate lookup and duplicate-head lookup create no owned atom;
a new tree consequence retains a discovery ID after canonical import, and a
dense consequence is a pending coordinate bit. Key construction charges
the argument span separately from catalog lookup receipts, including a deferred
gate with a missing slot. CPU source instances borrow checked keys for the
callback lifetime; their buffers own metadata, not logical payload.
`AtomKeys.tuple_agrees` equates views agreeing on all requested reads;
`membership_identity` connects successful substitution to extensional tuple
membership. Rust comparison/hash equivalence, index construction and binding
lifetimes remain implementation obligations.

The current formula join also uses a borrowed `AtomKey` at its indexed fallback
when every argument of a flat pattern is bound. `AtomKeys.key_identity` and
`membership_identity` identify the exact tuple; the catalog's unique insertion
invariant supplies the premise of `AtomCatalogs.identity_determines_position`.
`ColumnRelations.full_matches_preserved` and
`TableBindings.indexed_matches_preserved` justify selecting that row before the
unchanged matcher, and `TableBindings.join_family_preserved` lifts equal step
families to the join. The lookup in `formula_support::queries::exact` retains
original predicate-local row IDs, intersects the original occurrence's delta
partition, and leaves downstream row filters and scalar checks in place. Its
Rust obligations include complete binding validation, typed term comparison,
snapshot ownership, route precedence, charged key scans and exact failure
receipts. Canonical tests compare the point query with a full scan and compare
grounded subjects with the shortcut disabled. These laws do not certify those
implementation and resource boundaries.

The removed closure route's dense relation stored a bounded predicate's rows as
bits over the mixed-radix index of the arguments' ranks in their bounds. Its
round and block-step correspondence below is historical; the `RowSteps` laws
remain general statements. A round's dense
heads are marked as pending bits and joined into the relation after the
round, and a block step marks a block of heads from a block of rows by words.
[`RowSteps`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/RowSteps.lean)
states the laws of these steps over an abstract position function:
`marks_are_new_atoms` and `absorbed_is_union`, that under a position
injective on a carrier holding every derived head the marks are exactly the
new atoms and joining them publishes the held and the derived atoms;
`marks_distinct`, that distinct new atoms have distinct marks, so the marks
count what the derived-atom limit bounds; `block_heads`, that when no filter
or gate reads the stepped value, every other positive atom is fixed, and
the fixed part of the rule is live at some value of the stepped argument,
the other positive atoms holding and the filters and gates passing there,
the heads derived through the occurrence are the heads of the values whose
row the relation holds; and `block_places`, that relations placing a value
by the same rank after a base give a row and its head the same offset. The
Rust obligations are that the mixed-radix index is such a position, a
bijection between the tuples inside the bounds and the bit positions; that
the inferred bounds are such a carrier, an upper domain of every derivable
head, which holds because each head argument's bound is closed under every
template's contribution to it; that position order is canonical atom order,
which follows from each argument's values being kept in canonical order with
the first argument most significant; that the block-step plan admits only
rules and relations meeting the block's conditions, and a block is stepped
only at the innermost depth of a join, where every guard of the rule has
been judged; and the word arithmetic of the join. The argument bound, dense
relation, block-step plan and family tests check them. Under the laws and the
obligations, the closure over dense relations is the closure over catalogs,
step for step; the consequence step and constraint verdict of `DeltaRounds`
are the same whichever store holds the rows.

The dense New-word range must contain every new bit. Intersecting a prefix
window with this range therefore skips only zero words and preserves ordered
row coverage. Block offsets still refer to the original window, and an
exhausted scalar cursor still reaches its original end. Rust tests cover
advance/reset, partial words, shifted blocks and work refusal; the existing
Lean laws do not verify this concrete range bookkeeping.

Prepared ground-head coordinates specialize the same ordered substitution and
injective dense-position premises: every binding of a ground head denotes the
same tuple. Preparation computes its position once, without establishing that
the rule fires. The workspace is bound to the exact immutable layout owner;
switching preparations retires its retained relations before evaluation. This
also keeps block plans aligned with their actual relations. Tuple order, guard
evaluation, candidate-local marks, owner authentication and storage admission
remain Rust obligations. Regressions compare ordinary ranking with prepared
coordinates, alternate preparations with different inferred axes and retain
earlier interpretations across those calls.

The removed `DomainPositions` consumer read a constant's domain position
through a table of one slot per catalog term. Its historical invariant was: each domain value's slot names its domain
position, and every other term's slot is empty. An empty slot, a term the
program's read cannot name, or an absent table falls back to the domain's
content search, which decides presence and rank. The argument bounds and the
ground-head ranks it yields are therefore those of the content search. As for
the append dictionary's inverse below, the table's invariant, its build over
the canonically ordered domain and its fallback are executable obligations; no
new Lean law is claimed, and the closure's own ranking keeps its checked
canonical comparisons.

The objective consumers use `AtomLookup` over immutable model selections or an
`AtomIndex` over the original catalog. The index owns permutations of row IDs,
not additional atoms; a `CatalogIndex`, which hybrid constraint checkers of one
core share, also holds a handle to the catalog it orders, sharing its storage. Its required laws are exact full-key membership and
predicate filtering in original row order. Model lookup must additionally
exclude unselected catalog atoms. The checked value comparison must agree with
canonical storage identity, which is distinct from ASP term order. Existing
identity laws explain the denotation; stable merge sorting, binary search,
lifetimes and their charged failure prefixes remain Rust correspondence
obligations. Independent scan, shuffled-catalog and work-cutoff tests check
these concrete boundaries. A prepared `PredicateLookup` restricts both existing
orders to one complete signed predicate and arity. After the query predicate
matches, argument-only comparison must agree with full-key comparison. Exact
window bounds, original occurrence order, borrowed ownership and refusal
propagation are Rust obligations. The region consumer must associate each window
with the exact source occurrence and read current truth; final model lookup must
still exclude unselected atoms. No new truth or logical restriction is inferred
from preparing a window.

`SeedSelections.materialization_exact` shows that ordering and coalescing the
atom denotations of shared handles preserves their exact true set. Distinct
handles may denote the same atom. The retired Rust `SeedSelection` owned
canonical shared coordinate tokens, while `SeedView` borrowed their semantic
atom views without materializing owned atoms. Arc lifetime safety,
program-instance validation, canonical comparison and the executable candidate
cursor were separate correspondence obligations for that consumer. This interpretation law does not establish answer-set membership
or complete candidate coverage.

`GatePositions.atoms_exact` identifies the gate subsequence of an indexed atom
carrier. `retained_position_exact` relates each gate rank to its original dense
position and atom. The retired Rust `GateAtom` retained a Program-bound
`CarrierAtom` coordinate token and checked positive position.
`SeedAtom::resolve_in` used that position in the compiled gate table; the
relational CPU and GPU packing paths shared this resolver. That implementation
needed common predicate/domain order, inseparable token construction, program
identity and checked indices. The Lean laws prove the filtered-list
correspondence, not those historical Rust obligations or membership.
For that historical order bridge, admitted signatures and domain values were
sorted and unique. `AtomIter` advanced the last tuple coordinate fastest, giving
the same lexicographic tuple order as `Atom::Ord`, within its signature-first
order. Nullary predicates contributed one tuple even when the domain was empty.
The now-retired `crates/zetesis-core/tests/integration/gate_positions.rs` control
compared enumeration with an independently generated, sorted and deduplicated
product, including signed predicates and typed values. That source argument and
regression contract were not a Lean proof of the iterator. Canonical carrier
ranks remain distinct from append-assigned catalog identities and ASP term
comparison order.

[`CarrierCoordinates`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/CarrierCoordinates.lean)
makes the coordinate bridge explicit. `decode_injective` requires unique signed
signatures and typed domain values. `tuple_before_iff` and `decode_before_iff`
transport order only when those component decoders preserve and reflect the
chosen relations; increasing interning IDs do not supply that premise.
`sparse_membership_iff` identifies a selected atom with its unique coordinate
without assuming a finite or machine-representable whole carrier.
`shared_vocabulary_does_not_identify_atoms` separates shared term/signature
meaning from independent atom-row positions. These laws assume admitted
coordinate types. The removed Program-coordinate implementation separately
checked arity, bounds, exact Program applicability, immutable owner retention
and fallible coordinate construction. The general coordinate and shared-vocabulary
laws remain; canonical atom storage still has independent atom scopes.

`GatePositions.rank_fold_eq_blocks` relates the left-to-right domain-rank fold
to the sum of lexicographic block offsets. `blockOffset_lt_cardinality` places
valid digits inside a carrier of size `domain_size ^ arity`, including the one
empty tuple at arity zero. The retired `GateIndex` added the cardinalities of
earlier gate signatures and one to that offset. These arithmetic laws do not
establish its binary search, signature/domain order, checked powers and
additions, or connection to `AtomIter`; the historical full-carrier tests
exercised that connection. Its index token kept the original program and full
gate position even when only supported atoms were supplied to `locate`.

The retired closure candidate root could enumerate its completed upper bound's
gate atoms directly, holding its lower bound and offering only the remaining
upper atoms. The following coverage obligations describe that removed consumer.
`Bounds.narrowed_contains_accepted` supplies the coverage argument for every
completed pass. The semantic gate carrier stays the original full carrier:
atoms outside the upper bound are absent, including when read under default
negation. Neither the upper bound nor the selected handles replaces the
original program. A stopped upper-closure prefix cannot justify a smaller root;
`Bounds.closed_upper_sound` requires closure. The mixed-radix law supplies the
position calculation, not this semantic bound or executable enumeration proof.

[`RegionBounds.materialization_exact`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/RegionBounds.lean)
relates a descendant's borrowed root-coordinate bounds to the owned region formed
from its held and uncut atom selections. It assumes exact partial key lookup
and that each materialized list is a permutation of its selected denotations;
`SeedSelections.materialization_exact` supplies ordering and coalescing equality.
`gate_readings_exact` and `narrowing_exact` then identify both must/may readings
and the complete narrowing operation. The existing `Bounds` preservation and
conflict laws apply to that same region, not to a new acceptance test.

The removed closure-region consumer retained distinct canonical root coordinates
disjoint from fixed-held atoms. The concrete publication and control obligations
in this paragraph describe that historical consumer.
The theorem needs no disjointness assumption to establish union/set equality;
exact lookup already requires unambiguous coordinate identity. Matching region
length, signed and typed atom comparison, binary-search direction, original
program ownership and complete-root publication remain executable obligations.
An indexed missing key denotes false only in that completed-root representation;
the symbolic unbounded root and stopped root preparation keep their existing
owned-region/fallback semantics. Both closures must borrow the same pre-pass
decisions. All conflicts must be checked before committing any new decision,
including a previously held atom missing from the upper result or a lower-derived
gate already cut from the region. An upper constraint verdict does not refute
the region. A stopped upper computation publishes no smaller bound; earlier
completed decisions survive a later resource stop, while cancellation and
deadlines retain stopped coverage. These control and allocation properties are
Rust obligations, not consequences of the representation equality.

The removed scalar lazy closure retained one canonical tuple authority per
Program workspace. Its workspace/truth protocol below is historical; shared
relation storage and the `RelationExtension` laws remain. Per-predicate `Catalog` values retain scoped membership and
ordering/equality metadata. Every round borrows committed rows while a disjoint
appender admits newly discovered identities. Pending discovery IDs and dense
coordinate marks remain separate from truth until the complete scan finishes.
Final result publication selects the completed truth over an immutable prefix;
reset clears memberships, frontiers and pending marks while retaining identity
and reusable capacity for the next candidate. A dense relation also keeps, for
rows derived again, the discovery position its coordinate's identity already
has in that authority, so a later candidate resolves such a row without
rebuilding its atom. This is identity, not truth: the authority's discovery
positions never change, the pairs die with the workspace that owns the
authority, and which rows are true is still decided afresh by each closure.
`RelationExtension` states preservation of old row reconstruction and equality
selection when row references and dictionary meanings survive extension. It does
not establish the new Rust owner/prefix checks, publication protocol or reset
implementation. The catalog append session shares scalar insertion's one-row
transaction and retains its scratch between calls; its lent equality IDs feed
source postings only after a complete row commits. Correspondence still requires
that every successful session prefix has the same ordered rows and dictionary
meanings as the scalar insertion sequence, that refusal preserves that prefix,
and that retained scratch and capacity growth obey the reported byte limits.
`RelationExtension` can supply old-row preservation once these premises hold; it
does not prove this concrete session or its resource and failure protocol. The
retired oracle's `catalog_work` was a subtotal of its charged work; subtracting it
left other charged operations, not a runtime estimate. Existing selection and
round laws supply the semantic interfaces, while the complete composition with
canonical tuple storage remains an executable refinement obligation.

Increasing relation insertion retains a published right spine in the existing
row-index mutation buffer. A nonempty tree still compares each input with its
authoritative semantic maximum; no caller-supplied sortedness or freshness is trusted.
Executable correspondence must establish that every path writer revokes its
certificate before mutation, that only complete publication restores it, and
that the unchanged prefix really equals the published tree. Publishing only
the changed suffix also requires every changed AVL link and balance to be
included and pre-admitted, including rotations. `RelationExtension` preserves
old row meanings once those invariants hold; it does not prove this path reuse,
its charged work, retained capacity or refusal behavior.

[`ModelSelections`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ModelSelections.lean)
relates selected catalog positions to their interpretation. Canonical ordering
and coalescing preserve the true set; unselected catalog entries supply no truth,
and a renumbering preserves meaning when the selected atoms agree. Rust
[`Model`](../rust/models.md) retains a shared catalog and canonical selection.
Static and formula answers share their program catalogs; completed batched lazy
closures share a catalog frozen after the final complete round. Checked positions,
logical comparison, ownership and allocation remain executable obligations.

`rank_coalescing_preserves_interpretation` refines this boundary for cached
semantic ranks over valid positions in one fixed catalog. Equal ranks must mean
equal decoded atoms in both directions; permuting positions and coalescing their
ranks then preserves selected truth. `rank_order_iff` transports strict rank
order to an explicitly supplied atom order, and `increasing_ranks_decode_unique`
excludes duplicate decoded atoms from that selection. These premises do not
follow from increasing occurrence IDs, a shared vocabulary or equal catalog
lengths. Rust must construct and bind the ranks to the exact catalog, retain
selected representatives, and check work, capacity, cancellation and publication.
One-time order preparation and each selection receive independent work
allowances; their accepted charges accumulate in a separately checked session
counter. `WorkCharge.accepted_within_limit` applies to each local allowance,
not that cumulative receipt. The laws do not verify those Rust counters, the
semantic-order walk or its sorting algorithm.

The common `ModelRetention` ledger charges each distinct occurrence catalog's
portable encoding once, including repeated and unselected entries, then
selected-position records per entry and the consumer's score records.
`StorageOwners.sum_within_component_bounds` supplies
the finite-sum bound when those canonical components are completely covered
once. It does not establish the ledger's allocation-identity/hash invariant,
checked arithmetic or transactional publication. Rust keeps a live catalog
handle for every private identity key and commits only after both owner-index
and consumer-slot reservations. Abandoned or refused admissions preserve the
prior retained family; replacement admits the complete new family before
dropping the old one. The measure excludes canonical identities outside the
occurrence maps, capacity/allocator overhead and transient old/new overlap.
It is not an allocated-byte or process-memory bound, even when a retained
catalog's shared prefix contains those additional identities.

[`AtomCatalogs`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AtomCatalogs.lean)
describes a catalog split into a committed prefix and a pending suffix.
`commit_preserves_lookup` equates split lookup with lookup after concatenation;
`discovery_preserves_interpretation` shows that new identities add no truth to an
old selection. Successful equal lookups have the same position when the complete
catalog is unique. This last premise applies to unique builders, not arbitrary
public `AtomCatalog` inputs, which can retain duplicate dense slots. Index
ordering, transactional publication, allocation, machine bounds and Rust borrows
remain implementation obligations. These representation laws do not replace the
reduct or establish answer-set membership.

[`ClosedCatalog`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ClosedCatalog.lean)
composes a base-row lookup with a local lookup whose result is offset by the
base length. Sound component queries return the same atom as the concatenated
row decoder; complete component queries make absence exact. Only identifying
one exact row position additionally requires uniqueness across the whole
base and suffix. These laws reuse `AtomCatalogs` and supply the query premises
for canonical discovery below. They do not establish hash collision handling,
scope authentication, machine offsets, physical sharing or publication. The
storage prefix/suffix partition is independent of the semantic base/defined-atom
partition in `TerminalDefinitions`.

[`CanonicalCatalog`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/CanonicalCatalog.lean)
separates original occurrences from canonical IDs. `selection_exact` composes
those two decoders, including duplicate occurrences; `normalized_selection_exact`
preserves that interpretation after a specified permutation and duplicate
removal. `append_preserves_interpretation` requires every selected mapped ID to
lie in the old prefix, so an invalid old ID cannot silently become a new selected
atom. `owner_transfer_preserves_interpretation` requires agreement of decoded
atoms, and `raw_ids_do_not_determine_interpretation` shows why raw equality is
insufficient. The laws reuse `ModelSelections`; they do not establish DAG
interning uniqueness, Rust comparison/hash consistency, lifetimes or storage
accounting. In Rust, one atom operation resolves each canonical row once and
reads every field from that resolution while the immutable prefix is
borrowed; this is one evaluation of the decoder, not a retained copy. Atom
hashing writes the same fields as the ingress atom with equal contents, which
the canonical/ingress hash tests check across segments and readers.
Constructing a canonical view checks only that its identity lies below the
reader's counts: segments are contiguous from zero and end with those counts,
so every counted identity resolves when an operation reads it.

Canonical publication may extend exclusively owned snapshot directories rather
than copy their existing entries. The applicability checks establish the same
owner and current prefix; shared snapshots retain fresh-directory publication.
Both paths preserve the decoder agreement required by
`append_preserves_interpretation`. Rust regressions check repeated growth,
retained prefixes and bounded refusal. Exclusive ownership, fallible allocation
and transient storage accounting remain implementation obligations; this reuse
does not extend the scope of the Lean theorem.

`CanonicalCatalog.find_discovery` separates canonical query resolution from a
unique discovery map. `discovery_lookup_exact` equates that composition with the
existing structural occurrence lookup, assuming sound and complete canonical
queries, injective successful decoding, and an exact inverse of discovery.
`undiscovered_identity_absent` shows that a canonical row without a discovery
entry remains absent from every discovery slot. The inverse premise does not
apply to occurrence catalogs that repeat an identity at different positions.
Rust must maintain the inverse at publication, authenticate the readable scope
and prefix, and preserve refusal and allocation boundaries. These laws establish
neither a concrete hash/index implementation nor support or answer-set membership.

For an empty closed-catalog descendant, `discover_model_with` authenticates the
model's original writer scope and readable prefix, and the descendant's exact
shared base. The selected canonical IDs therefore decode to the same atoms in
both owners, without granting general cross-owner atom identity. The one
cross-owner identity Rust grants rests on the shared rows themselves: every
descendant holds the closed catalog's immutable row payload, so an ID below the
base's count names the same row in each of them and in the closed writer's own
snapshots. `AtomIdentityMap` therefore scopes such a row by the closed writer,
while same-catalog equality and atom acceptance still compare owners, and the
rows a descendant appends keep its scope. The model's
strict semantic order makes its completed prefix a unique discovery sequence;
predicate order matches the relation directory, and each next row follows the
last row of its predicate. Rust still has to preserve both AVL indexes, callback
and storage bounds, and complete-prefix refusal. The existing catalog selection
laws express the denotation obligation; they do not verify this insertion path
or establish that the supplied model is an answer set.

The selected discovery-order operation reuses `normalized_selection_exact` once
its result is a permutation of the supplied identities. Rust must establish
that the semantic index visits each discovery once, that the temporary mask
selects exactly the supplied positions, and that filtering preserves typed
order. `ColumnRelations.row_mask_roundtrip` uses the same membership,
uniqueness and ordered-filter argument for numeric row order; it is not a
verification of the AVL traversal. Dense traversal and sparse sorting therefore
share the denotation obligation while retaining separate executable checks for
ordering, scratch, interruption and transactional replacement.

Checked equality can reuse a canonical identity only after authenticating its
scope and readable prefix. Equal IDs then use the same decoder; deciding that
unequal IDs denote unequal terms additionally requires interning uniqueness.
The Rust stores must establish that stronger invariant, including normalized
aliases and exact hash-collision checks. These catalog laws do not prove it.
Foreign scopes and sign-adjusted predicate views require typed comparison.
Neither equality shortcut establishes numeric or ASP order. The callback before
the identity probe and preservation of its refusal are executable obligations.

`TemplateCatalog` stores shared ordered terms, patterns and filters beneath
separate semantic-owner policies. The retired relational `Program` retained
rule topology and explicit domain/signature order. `ObjectiveProgram` still
retains weight polarity, priority slots and ID-only closed conditions. Shared component storage creates
no synthetic support or objective rule.
The source compiler uses the same component schema for its constants, predicates,
patterns and filters. Its private coordinates are valid only with the
preparation that admitted them. Generated terms extend the vocabulary without
changing the meaning of the static prefix; truth and support remain separate
selections. This owner pairing and prefix preservation are concrete Rust
obligations, not consequences of integer equality.
[`CanonicalTemplates`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/CanonicalTemplates.lean)
proves that decoding commutes with substitution of ordered argument tuples,
including repeated occurrences and absent variables. Its variable-only case
equals the existing `BindingScopes.readAll` operation. The identity type ranges
over an available vocabulary: Rust must establish the scope and prefix checks
that make its decoder total. Refinement of the concrete template storage and
flat condition-node evaluator to these definitions and
`ObjectiveConditions.Query` remains an implementation obligation. These laws
neither prove source compilation nor infer answer-set membership from identity.

`AtomAppender::insert_pattern_with` borrows a checked projection of constants and
assigned variables, reused for discovery lookup and row admission. The tuple
is a mathematical sequence; the implementation need not materialize an ID vector.
`CanonicalTemplates.substitute_decodes` supplies its substitution obligation;
`AtomKeys.key_identity` and `membership_identity` relate the decoded tuple and
signed predicate to extensional lookup. Rust must establish the scope, prefix,
arity and complete-binding premises, preserve repeated arguments, and apply
logical limits even on an occupied lookup. Shared immutable input borrows must
keep each projected coordinate unchanged from validation through publication.
Fixed preparation metadata, scratch/growth accounting and
publication after all fallible checks remain executable obligations. Retaining
a complete canonical row after refusal does not establish discovery or support
membership.

Reusing a prepared canonical lookup additionally requires the same immutable
tuple and unchanged logical row index until publication. The exclusive Rust
entry enforces the writer boundary; index reservations may rehash storage but
must not change logical membership. The prepared result contains no bucket
address and is discarded on failure. These are implementation obligations for
the existing lookup law, not a separate answer-set semantics or a new proof of
the Rust implementation.

`DerivedTerms` gives registered input aliases and newly constructed terms one
fresh identity scope. Input edges borrow exact immutable catalog prefixes;
generated compounds retain local child IDs and borrowed constructor names.
`TermRead` exposes both through the same typed term view. The input payload is
not copied into the derived arena. Equal terms from different input owners, or
from an alias and a construction, must receive the same registered identity.

The existing laws state the required semantic boundary.
`CanonicalCatalog.owner_transfer_preserves_interpretation` requires decoded
agreement when identities change owners; it does not infer agreement from equal
integers. `CanonicalTemplates.substitute_decodes` preserves ordered child
occurrences, including repetitions, once scope and prefix checks establish the
decoder's domain. `StructuralBindings` and `Observations` then state matching
and completed-output laws over those decoded values. No new reduct is involved.

Exact alias interning, collision checks, cached expanded measures, borrowed
lifetimes and preservation of a complete prefix on refusal or a caught callback
panic remain Rust obligations. So does the absence of copied input payload.
`StorageOwners` applies only after a complete, disjoint accounting of the shared
inputs and the arena's own capacities; logical occurrence charges do not measure
physical sharing. The formula-DAG laws in `DagSharing` do not certify this typed
term representation or its Rust implementation.

[`PartitionedScan`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/PartitionedScan.lean)
proves that exact finite partitioning preserves an eligible-atom count and maximum
index. Zero availability excludes a fitting strict-subset completion; one
available atom must be false in every such completion. The maximum names that
atom only under the count-one premise. The formula kernel distributes this scan
over its workgroup's 32, 64, 128 or 256 invocations and merges their summaries. Supported subgroup reductions
may combine summaries first; no subgroup width is assumed and the final
workgroup barrier remains. Its strided coverage, bounded `u32`
arithmetic, immutable scan inputs, atom correspondence and synchronization remain
Rust/WGSL obligations. The laws justify domain narrowing, not answer-set acceptance.

Formula and tight shaders share `partial_maximum` in `reduction.wgsl` and its
granted-subgroup specialization in `reduction_subgroup.wgsl`. The portable
version publishes each lane's value. The subgroup version takes each part's
maximum and elects one publisher. The zero identity covers empty contributions.
`PartitionedScan.maximum_append`, `maximum_permutation` and
`partition_index_exact` justify merging these partial maxima under exact finite
partition coverage. Formula also computes its bounded additive partial before
branching on the shared election, preserving the count that distinguishes atom
zero from absence. Tight applies the maximum law with eligibility always true
to the lane tallies of each individual phase, then sums those phase maxima.
Combining tallies across phases before maximizing would change that path cost.
Its alternating slots, resets and publication barriers remain in the consumer,
including the verdict scan's separate final barrier. False-root witnesses
still precede unsupported-atom witnesses and use their existing minima.
These laws establish the unsigned fold equations. Subgroup uniformity,
election, finite machine bounds and the ordering of atomic publication and
reads remain WGSL obligations. Source specialization selects all fragments
before one checked setup allocation, including a subgroup-only tight source.

[`DependencySchedule`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DependencySchedule.lean)
preserves initial agreement with a reference valuation by induction over ready
assignments. The evaluator reads only declared dependencies; those dependencies already agree,
and the reference satisfies each local equation. Complete root coverage then
preserves ordered observations. This explains both formula kernels' shared
original-truth schedule, before reduct masking or tight-support checking.
Constructing its packed levels, preserving node
IDs and proving concurrent writes and barriers refine the sequential schedule
remain implementation obligations. A cached order alone does not authorize
candidate truth reuse; tight checking separately authenticates its retained batch.

Both kernels compose the immutable header/operand reads and binary Boolean
truth operation from `formula_graph/truth.wgsl`. The shared operation computes
conjunction, disjunction and implication over the same wire tags; each consumer
keeps its own original-truth traversal and publications. This factors those
local equations without identifying formula propagation with tight incremental
re-evaluation. Header/operand addressing and the consumer-specific fourth header
word remain representation obligations, not consequences of the schedule law.

`evaluate_original` in that fragment also owns the common serial or level-ordered
original-truth traversal. Both consumers retain their local node evaluation and
publication rules; tight evaluation keeps its reuse flag, while formula
evaluation requests fresh truth. Each level retains its storage/workgroup
barriers, and the callers keep their distinct final publication boundaries.
`DependencySchedule.execution_preserves_agreement` and `observed_roots_equal`
state the local-read, readiness and coverage obligations. They do not establish
WGSL uniformity, bounds or device visibility. Frozen propagation and the tight
incremental update traversal remain distinct.

The general kernel starts frozen propagation with a reverse visit to those levels,
then alternates reverse and forward visits.
This is not an application of the evaluation theorem: the local operations are
sound domain intersections, rather than assignments of original truth. Every
transfer must preserve all satisfying completions, including under stale sibling
reads, and original-false connectives remain disabled. Reordering such transfers
retains that invariant. A dense sweep visits every gate and then checks strict
subset availability; a fixed point or bounded stop remains a residual for exact
completion. Packed-level coverage, collective barriers, atomic refinement and
the changed work receipts remain Rust/WGSL obligations. No new shader-verification
claim follows from reusing the schedule.

Soundness alone does not make two schedules agree. A sweep that changes nothing
ends at the same fixed point under every schedule, and so in the same verdict
class away from round and work limits, only if every transfer is also
order-preserving in the domains it reads: from smaller domains it removes at
least as much, so a stale sibling read can only weaken a step. The contract
tests establish this for the binary gate transfer. For the native AND/OR group
transfers and the threshold and extremum-cut projections it is a further
Rust/WGSL obligation; the physical tests check verdict agreement across every
workgroup size on fixtures that each size schedules differently, which is
evidence, not proof. Answer sets do not depend on it: every outcome other than a
refutation is completed exactly.

Optional changed-dependency propagation requires a separate wakeup argument. Its
directory covers every operand and output domain, including atom aliases. Every
gate starts eligible; an intersection marks both current and next generations,
including intersections made by the strict-subset check. Active-word traversal
assigns disjoint words to lanes and uses private seen masks to visit a gate at
most once per sweep. Word reloads may discover new unvisited gates; wakes of
already visited gates or completed words must survive in the next generation.
If a sweep changes no domain, every pending transfer has observed that unchanged
state; quiescence still leaves an exact residual query. These directory, atomic,
coverage and generation invariants, and the extra storage and conservative work
charges, remain executable obligations. This mode preserves the same sound local transfers
and exact residual completion; existing Lean laws do not verify its implementation.

Optional incremental group propagation retains permission counts for native
AND/OR operand occurrences. Initial counts follow frozen-domain and root setup
at a uniform barrier. Each uniquely removed physical-domain bit decrements all
of its operand incidences before publishing wakes; the gate's separate output
dependency does not decrement those counts. Delayed notifications leave counts
as overapproximations, so they can postpone deductions but must preserve every
compatible completion. Distinct physical slots still determine unique witnesses,
even when one atom occupies several operand positions. Initialization, occurrence
multiplicity, atomic no-underflow, notification ordering, candidate isolation and
admitted storage/work remain implementation obligations; `FormulaChains` does
not by itself verify this concurrent representation.

The closure route's CPU oracle charged a payload amount before each comparison or
copy through `Work::charge`; it was removed with that route, and
[`WorkCharge`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/WorkCharge.lean)
remains as a statement. It equates an available work prefix with repeated unit
charging under unchanged control, including exact quota exhaustion. Formula source
admission instead checks an entire proposed charge before changing its counter.

The pure `zetesis_algebra::Meter::check_wide_receipt` checks a whole request
with checked u128 addition and an inclusive ceiling. It returns a proposed
total without changing a receipt or establishing that the work ran.
`Meter::check_receipt` widens its u64 receipt and ceiling into that same
calculation; `Meter::charge_receipt` delegates to the narrow check, then commits
a successful total into the caller's sole accepted counter. The observation
evaluator keeps its explicit cancellation/deadline poll before this operation
and preserves its source site and complete Statistics on failure. Importing evaluation work for
later rendering does not reconstruct a meter or reject a lower limit early;
the next charge, including zero, checks that limit.
[`AtomicWork`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AtomicWork.lean)
characterizes complete-demand admission, additive success and unchanged refusal.
`above_new_limit_refused` and `charge_zero` cover imported receipts and checked
zero requests. `accepted_composition` equates two successful receipts under one
ceiling with their combined request; it does not equate refused traces or permit
combining the caller's control observations.

Formula source `check_work` classifies admission before its control observation:
refused requests observe control before returning their native failure, while
admitted requests use the existing cadence. `count_work` records actual work in
Pending before updating the local count; checking a maximum does not count it.
A smaller actual charge requires an unchanged prefix and ceiling and an actual
amount within that maximum. `Counters::absorb` instead checks admission, counts
completed work, then observes control: interruption cannot erase that work,
and admission refusal performs no internal poll. Source overflow preserves its
saturated attempted-demand diagnostic. `AtomicWork.accepts_iff_fits` describes
the scalar preflight, not these caller invariants, native errors, control traces
or Pending settlement.

Source term expansion retains its u128 accepted-work field and leading
site-bearing control poll, including for zero requests. It uses
`check_wide_receipt`; on supported 32/64-bit hosts, its usize ceiling is below
u128::MAX, so mapping overflow to the native saturated attempted-demand
diagnostic preserves refusal. TermWork does not publish through `Pending.work`.
A tentative budget clone copies enforcement history but forks an empty pending
account; only its newly accepted ScalarBytes charges enter Pending, before
their local count is committed. Return, unwind and drop settlement publish the
pending charges. These control and publication steps remain Rust correspondence
obligations.

Search budgets reuse the same pure check for local work admission and the
committing operation for a bulk receipt after its quota succeeds. Their explicit
control poll and work-before-decision order remain unchanged. The local quota
retains its existing saturating admission at the largest u64 ceiling; a carry
then reports `CounterOverflow` at receipt publication. Shared leases can instead
consume part of a request before refusing, leaving the unsuccessful local bulk
receipt unchanged while retaining that work in the joined account. These native
error and permit protocols are separate from AtomicWork's scalar laws.

Shared direct charges check the real committed count against that count plus
currently available permits. This accepts exactly the requests covered by
available permits; outstanding grants remain unavailable. The conserved total
is the greater of the initial limit and imported work, so this derived ceiling
fits u64 even when the imported prefix exceeds the authored limit. Refusal
retains all fields; success subtracts available permits before publishing the
new committed count. The scalar law does not prove mutex behavior, concurrent
settlement or progress of waiting workers.

Domain analysis and answer projection use the same borrowed operation.
`zetesis_domain::numeric::Budget::work` keeps its existing `Cell<Statistics>`
mutation boundary; `KeyWork::step` retains its own accepted-step receipt.
Neither adds a control poll. Terminal-definition classification's `Engine::work`
uses the same domain helper for its single-unit requests, preserving typed stops
and withholding partial certificates. These requests fit u128, and a refused
charge reports the exact wide `Stop` for work.
The projection `Work` in `zetesis-solve` retains its explicit poll before
admission and preserves its native u64 overflow priority: a refused demand
above `u64::MAX` maps to `ProjectionError::Overflow`, while other refusals
retain the exact work-limit payload. Every u64 carry exceeds every u64 ceiling,
so this mapping occurs before any receipt can change. Each caller retains one
work counter and its other statistics; no stored meter or duplicate account is
introduced. `AtomicWork` describes their representable-sum admission and
unchanged refusal, not these native error mappings or control observations.

GPU relation planning also uses `charge_receipt` for its full-scan estimate.
Each query retains its validation before its representable u64 accumulation;
the authored whole-plan ceiling is checked only after all query costs. This
estimate is neither scheduled nor completed device work. Those receipts are
still published at submission and validated completion respectively. The scalar
laws do not establish query costs, transport correctness or device execution.

These are natural-number scalar laws. They do not verify the public
`MeterWorkError::Overflow` branch, u64/u128 conversions, exact error payloads,
polling or Rust execution. The observation adapter's invariant assertion relies
on its inspected count producers making a representable u128 sum; hypothetical
overflowing private calls are outside that correspondence. The existing
prefix-consuming `WorkCharge` laws remain unchanged and do not establish atomic
refusal. The [reading guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/atomic-work.md)
separates those policies and the remaining implementation obligations.

The byte dimension's `zetesis_algebra::MeterBytes::observe_peak`,
`observe_peak_usize` and `observe_peak_u64` share one scalar calculation at
u128, usize and u64 widths.
Each borrows the caller's actual peak, publishes `max(previous, total)`, then
checks only the supplied total against its inclusive ceiling. Diagnostic
widening preserves the native values; no converted peak or current/live account
is introduced.
The stateful `observe` assigns current before delegation; arithmetic preparation
retains its leading poll and records requested envelopes, while frozen-positive
witness completion retains its requested preflight, allocation, actual-capacity
observation and charged-copy order. Refusal retains the selected observation in
peak; a historical peak above a new ceiling does not itself refuse a fitting
total. These are distinct caller policies around one shared maximum/check.
Catalog replacement observation retains checked overlap construction, then peak
publication, retained-size arithmetic and commit, then any overlap refusal.
Retained-size overflow still takes precedence over that refusal. Answer
projection likewise keeps its retained-capacity write before constructing the
live total; an unrepresentable total leaves peak unchanged. Its constructor
observes actual capacity only after the existing preflight and allocation.
Neither consumer treats refused proposals as observed allocation or adds a
control poll.

Table and relation capacity observers retain their usize peak fields. Table
requested admission raises peak only on success; relation requested preflight
does not raise it. Both retain actual observations before byte-ceiling refusal.
Table checks actual usize arithmetic and observes peak before committing live.
Relation reserve converts and commits actual live before observing peak;
replacement commits retained live before converting the overlap. A refused
overlap conversion therefore preserves that live commit and the old peak,
whereas an actual byte-ceiling refusal retains both updated fields. Requested
checks, allocation, copy-work charges and release order remain distinct from
this shared scalar observation.

Observation wildcard-key replacement retains requested preflight, charged
allocation preparation and actual-capacity refusal before replacing its old
buffer. Compiled metadata likewise records its actual peak before refusal;
component errors propagate before the enclosing capacity refusal. These callers
borrow their existing u128 peak. Source attribution, nested-owner receipts and
the distinction between requested and actual storage remain unchanged.

Generator dependency preparation also borrows its existing u128 peak. It records
requested envelopes and actual capacity before refusal, but commits its current
byte count only after admission succeeds. Reservation keeps its leading control
poll and existing requested-before-allocation order; the shared observation adds
no work charge or poll and does not change the selected dependency preference.

`MeterBytes::check` exposes the same pure inclusive-ceiling calculation without
recording a peak. Native aggregate admission uses it for requested storage and
logical byte envelopes, while its actual-capacity observations borrow the
existing u64 peak. Current-capacity conversion and publication still precede
peak conversion; an unrepresentable peak fails before the storage ceiling is
checked. Logical peak assignment remains distinct from an observed maximum.
Source support publishes its existing byte event before checking the ceiling;
shared storage leases retain their atomic replacement and release ownership.
These adapters add no polling, allocation or second byte account.
The same pure check serves catalog requests, evaluation workspace admission,
tight-plan logical bytes, partition construction, projection capacity and
count-plan payloads. Each caller retains its preceding checked arithmetic and
native width conversion. Partition dimensions are recorded before refusal;
count-plan payloads commit only after admission. Domain inspection also uses
this decision, on cumulative inspected text rather than retained storage, with
its original Cell receipt and success-only publication.

[`BytePeak`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/BytePeak.lean)
proves the two decisions, retained maximum, coverage, zero behavior and
independence from the historical peak. `successive_receipts` composes only the
retained maxima; it neither equates decisions nor permits continuing after a
refusal. The [reading guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/byte-peak.md)
separates these scalar laws from actual capacity, allocation/release ownership,
finite-width total construction, typed failures, polling and Rust correctness.

The closure route's `PreparedQueries`, one exact-program preparation that
candidates reused with private truth state, was removed with that route. The
reuse argument stated for it remains as text and has no Rust counterpart now.
In that implementation, changed preparation owners retired retained workspaces,
even for the same program. Those admission and lifetime checks were separate
implementation obligations; the closure laws did not prove them. Historical
regressions checked identity sharing, preserved refusals and retained-answer
isolation.

The removed closure batch oracle partitioned ordered candidate occurrences
into disjoint ranges, each with an exclusive workspace. The following shared
accounting law remains general; the prepared-query accounting describes that
historical batch consumer. Its coverage obligation is that splitting
preserves every occurrence once and ordered concatenation restores the input
sequence. Its storage obligation includes shared preparation, idle capacities
and active reservations without counting shared headers twice.
[`StorageOwners.shared_idle_active_within_limit`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/StorageOwners.lean)
sums explicit shared and idle bounds with an active bound of
`max(retained, allowance)` per owner. The batch maps `allowance` to the scalar
limit minus the prepared queries' retained bytes and its shared closed fact
rows/index subtotal, after checked subtraction. That fact subtotal is counted
once in the shared component; each workspace still admits its full fact base
independently. Vocabulary retains its existing per-workspace accounting. The
separately allocated query-owner header enters the shared component once; its
remaining immutable payload has a separate preparation ceiling. An empty
batch reserves only retained storage. The consumer must account for every named
region with disjoint owner entries and count the shared component once. The
pointwise capacity bounds, Rust measurement, cache retirement, checked arithmetic
and range scheduling remain implementation obligations. The theorem does not
establish a hard allocator or RSS cap, including on a refused allocation. Reused
capacity can increase retained space or change work counts; neither this law nor
semantic equivalence proves a speedup.

The removed independent scalar rounds used the same occurrence partition over
stable catalog insertion IDs. The `DeltaRounds` premises remain general. `DeltaRounds.coverage_of_first_delta` requires a correspondence
between those IDs and complete body bindings. `delta_step_exact` then preserves
the complete inflationary step when old consequences are already present;
`latched_constraints_exact` additionally requires exact prior constraint history.
Bootstrap handles zero-positive rules and constraints. In the retired
relational representation, explicit core fact tables represented the same
zero-positive headed occurrences: `Program::rules()` had to preserve their
signed atoms, duplicate occurrences and anchors, and `FactRef::as_template()`
had to denote that head with empty bodies and filters. The old oracle emitted
these heads through the same binding and `record_head` path; its incremental
indexes contained only templates, since a fact reads no predicate.
`delta_step_exact` therefore uses the same old-consequence premise after
bootstrap. The shared closed store supplies identity only; each workspace starts
with empty discovery and truth. The historical full/delta, dense/tree,
source-order and reuse tests exercised these obligations, but did not prove the
Rust interning,
occurrence mapping, allocation ledger or lifetime implementation.

These premises explain
why the final no-change round can combine its selected scans with completed
history instead of revisiting every old binding. They do not certify the Rust
matcher, cache boundaries, failure cleanup or shared-world/device schedules.
The [delta-round guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/delta-rounds.md)
records the historical owner and publication boundary for those premises.

Eager support joins partition new tuple combinations by their first new source
occurrence. `DeltaJoins.partition_complete` proves coverage and
`partition_disjoint` proves uniqueness. Source occurrence identity survives
execution-order changes and repeated predicates. The Rust certificate must
select only the eligible positive producer grammar; posting slices must denote
the specified old/new intervals. Rich producers and final authored-body
validation retain separate complete scans. These laws establish the partition,
not the source evaluator or termination of value generation.

The join planner receives the partition before choosing its execution order.
It need not first prepare an unrestricted order. Relation handles belong to the
join's immutable support snapshot; a prepared rule cannot retain a directory
position across support publication. These choices preserve the same occurrence
partition. Handle resolution, comparison readiness and admitted work remain
Rust obligations.

The optional multiples selector composes this partition with necessary row
selection. Its scalar totality proof covers the full immutable support snapshot;
it does not establish completed-family evidence. Matching value postings are
intersected with the interval of the original occurrence before they are offered
to the cursor. This retains every contributing binding within that partition,
the premise of `DomainBindings.guarded_continuations_exact`.

`BoundedMultiples.bounded_multiple_iff` supplies the scalar arithmetic identity:
for a positive divisor `d`, the values in `lower..upper` with zero truncating
remainder are exactly `d*k` for integer factors between
`(lower + d - 1) / d` and `upper / d`, using floor division.
`multiple_membership` composes this identity with finite interval enumeration;
divisor one and empty intervals need no special premise. The implementation
must establish the selector's domain and source totality, finite-width arithmetic,
exact posting and interval intersection, row ordering, snapshot lifetime and
charged preparation. Neither the arithmetic nor partition laws verify those
Rust operations or allow generic computed-table selection during a support round.

`Meter::check_count` shares the checked-addition and inclusive-ceiling body with
wide work admission, while keeping named resource counts at their native usize
width. Positive and stratified dependency counters retain native Overflow;
source-program counts retain their saturated diagnostic on overflow, including
when the authored limit is usize::MAX. Original counters publish only successful
results, and no poll is added. `AtomicWork` supplies the whole-request admission
laws under the count's natural-number interpretation; `AxisFolds.BoundedSum`
characterizes native-width overflow separately from the configured ceiling.
These scalar laws do not identify counted source events with solver work or
verify the adapters' diagnostic and mutation order.
Objective construction also uses this count admission for proposed formula
nodes and operand occurrences. Its work tick still precedes node admission,
which precedes operand admission; subsequent validation and the sole graph
transaction retain their existing order and failures.
Projection keys and retained world-view counts use the same operation. Key
lookup still precedes new-key admission; answer-count refusal precedes atom-count
refusal, which precedes retained-payload admission and allocation. Formula
transactions compute all checked dimensions before applying their original
ordered ceilings; whole-theory admission still recounts actual occurrences.
These decisions neither change the graph's shape requirements nor authorize a
cached-count shortcut.

`Meter::check_wide_count` shares that calculation at u128 width. Source
expansion uses it for templates, values, origins and cumulative scalar-byte
charges, preserving the resource names and existing counters. These byte charges
are not actual storage or peak observations. Representable excesses retain the
full demand; overflow maps to the existing saturated source-limit diagnostic.
That mapping relies on supported 32/64-bit hosts having usize ceilings below
u128::MAX. Polling still precedes admission; successful scalar-byte charges
publish to their existing pending receipt before the local counter commits.
Domain count checks use this pure operation on already-computed demands, with
zero additional demand and the original named refusal. Output conversion uses
the corresponding wide work check after polling and publishing each attempted
symbol step. A refusal retains that attempt; cancellation before the increment
does not add one. The inherited increment's finite-width behavior is unchanged;
the scalar admission laws do not prove that this earlier increment cannot
overflow.

Tight GPU planning uses the same pure work check for optional schedule and
retained-truth increments, bounded by both the authored allowance and the u32
wire representation. The returned total is a proposed device allowance, not
executed work; route selection and physical execution remain separate contracts.

Parallel exact queries lease bounded work allowances from one shared owner.
`WorkPermits` partitions the allowance into spent, available and outstanding
permits. Granting preserves that total; settlement records consumed work and
returns the unused part. Empty availability is not exhaustion while a grant
remains outstanding. After a grant is fully consumed, Rust commits its used
permits and attempts to acquire a replacement under one ledger lock. Successful
renewal composes `settle_conserves`, with consumed and granted amounts equal,
then `grant_conserves` for a replacement bounded by availability. A successful
renewal returns no unused permits and retains an outstanding replacement, so it
cannot enable a previously blocked waiter. Settling the last outstanding grant
with no available permits must wake waiters even though no permits were returned.
The Rust implementation must additionally preserve conservation and these
notification obligations under word arithmetic, locking, unwinding and
cancellation; the Lean laws do not prove condition-variable progress. A joined
batch records spent operations only after all leases settle; a lease is not
evidence of candidate execution or membership.

`zetesis_algebra::PermitLease` supplies the incremental window used by the
stable search lease. It and `Meter` share the consume-one, bounded-prefix and
detach-before-publication operations over borrowed views of their actual fields.
`WorkPermits.take_preserves_window` and `take_partitions_remaining` justify
bounded consumption; `refund_preserves_window` and
`settlement_partitions_grant` describe returned permits and the exact transfer
receipt. The local lease clears its grant; the cumulative meter retains its
charged history. These are different post-settlement states governed by the
same used/unused partition, not interchangeable admission policies.

For `runtime::copy_words`, let `c` be charged work, `L` the admitted cumulative
limit, with `c <= L <= u64::MAX`, and `n` a nonempty copy run covered by
`n <= L - c`. The covered-success case of
`WorkCharge.repeated_eq_charge` identifies `n` unit charges with one charge of
`n`, both ending at `c + n`. Instantiating the numeric `WorkPermits.Window` with
`granted = L` and `remaining = L - c`, `take_partitions_remaining` and
`take_preserves_window` give the same remainder and used count. This cumulative
view is not the source account's outstanding grant; settlement uses that owner's
actual window and `settlement_partitions_grant`. At an empty balance the copy
retains the original one-word charge and write, so refill, polling, refund and
refusal boundaries remain unchanged. No exhausted-batch law justifies charging
across that boundary.

The Rust argument additionally requires representable counts and slice bounds,
one original mutable-backing acquisition, and an infallible `u64` copy with no
allocation, user callback or intermediate control observation. These premises
make the intermediate reordering unobservable at the existing operation
boundaries; they do not justify batching fallible maps or larger atomic
primitives. Exact quota and stopped-prefix tests are regression evidence, not a
Lean proof of Rust's finite arithmetic, borrowing or callback behavior.

Renewal remains inside the source's existing lock, with control observed before
locking. Its callback must operate only on that source account, without
re-entering the same lease, and successful refill leaves a positive unconsumed
grant. Ordinary settlement still detaches before acquiring the source lock.
The stable wrapper retains drop settlement; the common window owns neither a
source nor a destructor refund. Its scalar laws do not authenticate callback
discipline, finite-width arithmetic, cancellation, locking or unwinding.
Whole-request admission retains refundable partial top-ups; incremental takes
retain consumed prefixes when a later refill fails. Their zero-request and
polling rules remain distinct and are checked at the concrete Rust boundaries.

`Quota::replenish` makes a bounded operation's unused borrow explicit when it
needs more permits. Its default preserves cumulative partial grants. A shared
lease instead returns that borrow to its original window before settling and
waiting for a replacement that covers the next atomic minimum. The meter writes
back the returned ownership on success and refusal alike; its performed-work
count does not change during replenishment. `WorkPermits.refund_then_settle`
establishes conservation and the exact committed prefix under the owned-borrow
premise. `settled_replacement` shows that a new grant adds no performed work;
`unused_window_does_not_commit` covers a wholly unperformed borrow.

The new grant is requested only after the old window is settled, so a waiting
worker retains no unused permits needed by a peer. If the available allowance
cannot cover the atomic minimum and no grants remain outstanding, the operation
is refused without running. Some allowance can remain unused. These laws do not
prove the minimum-aware admission, notification, polling or unwind behavior;
the concrete adapter must preserve those contracts and report no completion for
a refused operation.

Whole-kernel attempts use `Meter::under_reservation` with an already admitted
`Reservation`. The common operation validates that returned work does not exceed
the grant before committing it. This supplies the consumed-within-grant premise
of `WorkPermits.settle_conserves`; a native failed result with a valid receipt
settles that receipt in the same way as success. Without a usable receipt, the
shared guard retains its existing conservative settlement of the full grant.
The caller retains its zero-allowance policy and cumulative receipt publication;
acquiring, waiting for and dropping a shared grant remain Rust obligations.
These laws do not verify the callback, the guard's destructor or the concrete
transaction implementation.

The parallel region walk keeps one lease per worker across its consecutive
regions and settles it before stealing or waiting for another region and
before sending a model; a reservation settles it first, and exit or unwinding
drops it. The ledger is therefore locked about twice per grant rather than
twice per region, while no unused permit is held where the worker could
block. At the single-read boundary, exhaustion is reported only when no permit
is available or outstanding. Atomic operations additionally need their complete
minimum as described above. The total work bound is unchanged, but near a small
allowance a peer can wait
until a busy worker's local run ends before permits return, and mid-run
statistics trail committed work by up to one grant per worker. Its shared
allowance includes
coordinator certificate preparation; each worker reserves a finite checking
bound before running its certificate and settles its actual returned work.
The bounds follow the checkers' charged visits: nodes, operand occurrences, roots,
producers and atoms for tight checking; atoms, nodes, operand occurrences and
roots for positive or stratified checking. An unwind
consumes its reservation conservatively and yields an incomplete worker failure.
Returning unused grants before an idle wait, releasing blocked
sends before joining on iterator drop, and counting joined certificate checks
are Rust lifecycle and accounting obligations, beyond permit conservation. The
bounded subprocess regressions in `zetesis-stable/tests/integration/parallel_regions.rs` exercise
shutdown and idle grants; the certificate regressions compare scalar and
parallel region and construction work, check that every certified check
completes within its reservation, and reject an insufficient shared allowance. An injected
worker unwind also checks that idle peers wake and coverage remains incomplete.

The native parallel walk keeps one deque per worker. Its owner removes the
newest region, and an idle worker tries to steal the oldest region from a peer.
Each deque has its own mutex; a thief skips a busy deque. Payload preparation,
narrowing and membership checks hold no queue lock. A split first reserves entry
capacity for both children fallibly, then raises the unresolved-region count by
one before publishing either child under the same queue guard. A resolved
region decrements the count once. Taking or stealing only transfers ownership.
Thus pending and active
regions remain one frontier until refuted, split or checked, and an idle worker
can establish termination only when the count reaches zero. Idle workers wait at
a gate: the resolution that reaches zero, a stop and a close each wake them after
changing the count or the closed flag. A waiter registers in an idle count under
the gate, issues a sequentially consistent fence and then re-checks both conditions
and the peers' queues before waiting, so none waits past the end of the walk. A
split publishes its children, releases its queue, issues a sequentially consistent
fence and then reads the idle count without the gate, taking the gate to wake one
waiter only when one is counted. The two fences are totally ordered, so either the
waiter's re-check finds the published child or the publication counts the waiter,
whose gate it then waits for until the waiter is waiting. The wait stays bounded,
which is how an idle worker sees a cancellation, or a region left in a queue its
re-check found busy; completeness never depends on a wake. The memory-ordering
argument is a Rust obligation, exercised by a lost-wakeup protocol harness. It
rests on a premise about the platform's mutex: releasing it and `try_lock` are
atomic operations on its lock word, which the fences order.

Before its workers start, the walk runs on the calling thread: the coordinator
takes the newest region of deque 0 as a worker takes its own, and decides it with
the same per-region step. At engagement every outstanding region is in deque 0
and none is in hand, except at most one leaf held by its published query, whose
pending regions are all in the published-region deque 0, so the workers start
with the count equal to the regions in the deques plus that leaf. The
coordinator publishes only once its engagement deadline has passed, and the
pull then engages at once, so no query is running on the workspaces engagement
releases. The coordinator phase and this handoff are Rust refinement
obligations beside the queue and gate protocols.

A candidate's subset query may be published for any worker to serve. The
traversal surrenders its pending regions, the one it was about to narrow
included, so those and the regions it visited partition the query's root, and
each published region is one pending node of the query's coverage tree.
[`InnerQuery.exhausted_walk_stable`, `emitted_refutes` and
`exhausted_walk_decides`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/InnerQuery.lean)
compose `Pending.Walk.exhausted` with `ReductRegions.stable_iff_no_countermodel`:
a walk of the query tree that empties its frontier without emitting a leaf
proves stability, any emitted leaf refutes it, and an emptied walk's verdict is
whether it emitted one, whoever held each region. The Rust obligations are the
query cell's: its count of live regions is set to the published regions'
number before any is visible, raised by one before a split's children are
visible, and lowered once per region refuted, reaching the candidate itself,
yielding a validated countermodel, or dropped after the query is decided, so
it reaches zero only when the query's frontier is empty; a server writes its
witness or stop before its release decrement, and the decrement that reaches
zero is an acquire-release read-modify-write, so the stable verdict sees every
earlier one; one compare-and-swap decides the resolver; a validator's refusal
or stop sets the cell's stop before its region is retired and is never counted
as a resolved region; the published leaf counts in the walk until the cell is
decided, so the walk's count cannot reach zero beside a live query; and the
published-region deques hold one query's regions per worker at a time, at most
its candidate's atoms plus one, beside the candidate deque's `atom_count + 1`.
Unit tests drive one schedule by hand through each verdict and stop, and catch
a gain counted after its children are visible, a published leaf resolved at
publication, and a refused validation counted as resolved.

`Pending.Step.perm` and `Pending.Walk.exhausted` describe the abstract preservation
and exhaustion laws; the atomic count, the queue and gate protocols (including the gate's idle count) and absence
of lost ownership remain Rust refinement obligations. Depth-first local order
retains at most one older sibling per ancestor plus the newest children. Each
split decides another atom, and stealing starts only with an empty local deque,
giving at most `atom_count + 1` live entries per deque. Entry capacity grows
fallibly as needed and is released after joining, including a partially launched
worker set. This reservation does not make region/knowledge payload cloning
fallible.
The owned publication boundary distinguishes a semantic split from a refused
attempt to publish it. `Frontier::publish_split_with` reserves before consuming
the narrowed parent; refusal returns that parent and the original typed error.
`requeue` also returns its owner on refusal. Queues together with in-hand or
returned parents therefore remain the pending partition at this boundary.
`Region.split_partition` and `split_disjoint` justify a successful split, including
a slate with one queued child and the other in hand. The refused boundary makes
no further coverage-tree step: it corresponds to the identity case
`Pending.Walk.done`, after any already completed sound narrowing. Rust must still
establish exact ownership, fresh-atom selection, queue order, counter updates,
capacity admission and receipt preservation; the abstract laws do not verify them.

After a typed terminal stop is committed, a non-resumable concurrent schedule may
explicitly abandon the returned owner and its remaining queues. This cleanup is
not a `Pending.Step.refuted`, and an empty container after cleanup does not meet
`Pending.Walk.exhausted`'s premise of a covered walk to an empty frontier. Served
queries must record their stop before abandonment/retirement, so neither query
count teardown nor a released queue supplies `InnerQuery.exhausted_walk_stable`.
The scalar walk's retry and surrender contract instead retains the parent after
returning a capacity refusal. These lifetime distinctions are executable
correspondence obligations, not additional semantic outcomes or recovery APIs.

A native worker may retain one refuted region's knowledge allocation for its
next split. `Knowledge::clone_from` and `CandidateKnowledge::clone_from` must
overwrite every logical field from the parent, preserving the same knowledge as
a fresh clone and independent child ownership. Compatible arrays reuse storage;
incompatible shapes are replaced. The retained bundle is not a pending region
and carries no authority to narrow a different region. Complete overwrite,
allocation independence and the one-bundle retention bound are Rust obligations;
the frontier laws alone do not establish them.
An idle worker that observes cancellation or a deadline
records the typed stop before publishing closure. The coordinator may already
be waiting after its own control poll; channel disconnection must retain that
stop instead of establishing exhausted coverage. A concurrent close may follow a
worker's eligibility check; that already-active operation retains the existing
bounded-stop contract.
The Lean laws do not prove the scheduler's progress or Rust memory ordering.

Original and frozen `Narrower` operations expose metered entry points returning
an independent `NarrowingAttempt<Failure<Q::Error>>` receipt. The common
`zetesis_algebra::Meter` reserves permits from the injected search budget
through a typed `Quota`, at most `NARROWING_BATCH`
(256) at a time, and spends one before each charged read, so the shared
ceiling bounds execution itself. Cancellation and deadlines are polled at each
reservation, so at most `NARROWING_BATCH` charged reads apart. A budget grants
what remains when fewer permits than requested do, so a refused reservation
prevents the same read per-read charging would; all earlier reads remain in the
attempt and joined region counters even on a stop. Unspent permits are refunded
when narrowing returns, so the receipt and the shared ledger count exactly the
reads made. The failure carrier preserves the caller's original payload; local
stops retain their narrower-specific mapping. The stable adapter has no saved
last-error slot. The local `RegionLimits` APIs wrap the same closure and meter,
with their existing polling boundaries. Preservation of semantic narrowing
still depends on `FormulaBounds` and `FerrarisMask`; permit conservation does
not prove the reading rules or knowledge ownership. Prefix tests for original
and frozen narrowing and a shared one-permit regression exercise the Rust
admission/receipt boundary.

Candidate composition uses the private `zetesis_stable::subject::Subject<C>`
interface and `joint` driver. Its eager adapters call
`Narrower::propagate_known_reserved` to close the original theory, then each
permanent restriction and the active bound. Each eager subject drains its
propagation and support queues before reporting its local result. A streamed
subject may instead decide one fresh atom and immediately return to eager
closure. Only a complete late pass without changes establishes joint agreement.
If no later subject changes the region, all preceding closures remain valid:
their only shared mutable input is the region's decisions. A later restriction
or source consequence that decides an atom restarts from the original theory.
This stopping condition depends on complete per-subject closure, exact changed
flags and independent knowledge; those remain Rust correspondence obligations.
The semantic preservation laws do not establish this concrete loop's fixed point
or its work and interruption prefixes.

The shared `SubjectAttempt` publishes each attempt's search counts before
inspecting its result, including on a failed attempt. A streamed update checks
the cumulative receipt before polling, spending its existing one-unit
propagation charge and mutating the mask. It spends no split decision. Source
work, diagnostics and the retained source checker remain separately owned.
The frozen adapter performs its existing single narrowing/selection attempt
and publishes preference only after accounting; it has no candidate components.
These concrete sequencing and ownership claims require Rust validation, not
just the abstract Subject laws.

At that joint fixed point, the `Knowledge` preference operations select once,
ranking the original theory's atomic choice heads first and then the unknown-parent
counts of the last subject: the active bound, latest permanent restriction, or
original theory, in that precedence. With a restriction, the original theory's
unknown-parent count breaks ties through `preferred_atom_with_secondary_reserved`;
otherwise `preferred_atom_reserved` retains the original-only ranking. The two
knowledge values must share atom identities and be closed for the same region.
The ranking charges each inspected count, one per open atom without a
restriction and two with one. It reads the choice mark in the primary charge,
retains its charged prefix on refusal and installs no partial preference.
Packed enumeration must exclude padding and preserve ascending ties. Selecting
an open atom retains the split-partition obligation of `Search.CoverageTree`;
the existing laws do not verify these Rust ordering and accounting operations.

An implication's child event carries truth already installed in its knowledge.
A false antecedent or true consequent marks the parent true. Otherwise the
revisit reads the other operand, then the parent, to apply the remaining upward
and downward rules. Upward learning precedes downward teaching, and a newly
learned parent retains its own queued event. The parent's own event keeps the
ordinary downward operation; chains keep their indexed operation.

These cases use the existing implication constructors of `FormulaBounds.Known`.
Rust must establish that the event names an actual operand whose truth bit is
still present, and that the parent is unmasked in a frozen-reduct query. Complete
incidences, coalesced aliases, contradiction handling, event order and charged
prefixes remain Rust obligations. Finite-state regressions cover distinct and
aliased operands, either queued operand when both are known, and frozen child
and parent boundaries. They do not formally verify the concrete queue.

## The solver's algebra

The session's `zetesis_solve::Original` retains the immutable input identity
against which its results were established; their `original` fields and
accessors retain that association. `zetesis_themelios::ProgramOriginal`
identifies retained program, part or statement evidence. Neither is the
algebraic `Subject C` below. Renaming these owner APIs supplies no additional
membership, coverage or refinement theorem.

[`Algebra`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Algebra.lean) names the objects that candidate generation and the
membership check share and states the laws required of their mathematical
instances. The tables give concrete Rust declaration identities and Lean
identities with their kinds. Some declarations are private implementation
boundaries: public callers use the crate re-exports, including
`zetesis_algebra::{Meter, MeterBytes, PermitLease, Reservation}` and
`zetesis_algebra::words::Words`. An exact name locates a declaration; it does not
prove the relation sentence or a Rust refinement.

The representations need not have identical names or fields. In particular,
Rust `Entry` relates to Lean `Pending`, while a concrete `Frontier` must realize
the pending-partition invariant of a Lean walk. The tables state those relations
and their remaining obligations explicitly.

`Algebra.Subject.cohort_extensive`, `cohort_monotone` and `cohort_sound`
lift existing subjects pointwise to a finite indexed family of regions.
Each position retains its own class and subject; `cohort_members_iff` preserves
exactly that position's class members, and `cohort_refuted_iff` keeps refutation
local to it. `cohort_empty` leaves the empty family unchanged. These laws specify
no batching layout or execution schedule and establish no candidate's membership.
For countermodel queries, different candidates keep their different
`Countermodels T M` classes. The candidate and countercandidate quantifiers,
and their reductions, remain those of `answer_sets_nested`; the atom axis stays
inside each region. Concrete cohort rounds, their receipts, and their refinement
to these pointwise applications remain implementation obligations.

### Intervals, classes and coverage

| Rust declaration (kind) | Lean declaration (kind) | Relation and remaining obligations |
| --- | --- | --- |
| `zetesis_algebra::region::Region` (struct) | `Zetesis.Region` (structure) | For a fixed finite atom carrier, the held mask denotes lower and the real atoms not cut denote upper; the owner must establish disjointness, bounds, padding and the corresponding interpretation identity. This relates the concrete interval to Region, not its storage layout. |
| `zetesis_algebra::region::Region::split` (method) | `Zetesis.Region.split_partition` (theorem) | Splitting on a fresh open atom produces false and true children partitioning the parent interval; freshness, matching carrier identity and preservation of concrete masks are Rust obligations. |
| `zetesis_algebra::class::Class` (trait) | `Zetesis.Algebra.Class` (abbrev) | The sealed Rust trait separates classes of search; runtime theory, candidate and restriction identities instantiate the Lean membership predicate, which is not stored or proved by the marker trait. |
| `zetesis_algebra::class::AnswerSetsSought` (enum) | `Zetesis.Algebra.AnswerSetsSought` (def) | The candidate marker denotes answer sets of the authenticated theory satisfying the current restrictions; the marker alone does not authenticate any particular theory, restriction generation or accepted member. |
| `zetesis_algebra::class::Countermodels` (enum) | `Zetesis.Algebra.Countermodels` (def) | The frozen marker denotes proper-subset reduct countermodels for the authenticated theory and candidate; those identities and independent reduct membership remain concrete obligations. |
| `zetesis_stable::subject::Subject` (private trait) | `Zetesis.Algebra.Subject` (structure) | A fixed-class Rust subject must realize sound, monotone, decision-adding narrowing. An eager closure and one streamed late consequence have different completion meanings; the trait supplies neither the law nor interpretation authentication. |
| `zetesis_stable::subject::joint` (private function) | `Zetesis.Algebra.joint` (def) | Completed eager/late iteration must realize the common closure for a fixed authenticated subject family. Learned rows, bounds, restrictions and declined allowances can change that family; fixed-family order independence does not cover those changes, and stopped prefixes are not agreement. |
| `zetesis_algebra::walk::Walk` (struct) | `Zetesis.Pending.Walk` (inductive) | The runtime traversal must realize pending coverage and accepted-leaf steps of the Lean walk; scheduling and splitting do not provide the semantic leaf decision, and stopping does not establish exhaustion. |
| `zetesis_algebra::region::Entry` (struct) | `Zetesis.Pending` (structure) | An Entry owns a concrete interval, lineage state and split preference, while Pending carries the interval and a coverage subtree. Their relation is a coverage invariant, not field or layout equality. |
| `zetesis_algebra::walk::frontier::Frontier` (struct) | `Zetesis.Pending.Walk.exhausted` (theorem) | For stable search instantiated with Entry, queued and active ownership together must realize the Lean pending partition. Only resolved complete coverage yields the theorem; closing or abandoning workers is not exhaustion. |
| `zetesis_algebra::words::Words` (struct, re-export) | `Zetesis.Refinement.PackedInterpretations.pack` (def) | Words is storage-transparent and has no logical universe. Its 64-bit interpretation instances are related to pack under the owners' carrier, length, bounds and zero-padding invariants; this is not identification of the entire generic lane/storage owner with a 64-bit packing function. |
| `zetesis_algebra::words::storage::Words::half32` (method) | `Zetesis.Refinement.PackedInterpretations.half32` (def) | For an in-bounds half index in a 64-bit backing word, low-half-first extraction denotes the Lean numeric 32-bit half. Matching the byte round trip, logical export length and padding remain concrete obligations; other u32/u64 method families are separate instances. |

### Values and enumeration verdicts

| Rust declaration (kind) | Lean declaration (kind) | Relation and remaining obligations |
| --- | --- | --- |
| `zetesis_algebra::reduction::Reduction` (trait) | `Zetesis.AxisFolds.Reduction` (structure) | The sealed statically dispatched trait supplies pure value operations related to a commutative monoid. Each instance retains its order, bounded-carrier or denotation premises. Value laws neither commute effects nor prove accepted-member coverage. |
| `zetesis_algebra::reduction::All` (struct) | `Zetesis.AxisFolds.all` (def) | All combines Boolean accepted-member values by conjunction with true as identity and false as absorber; using absorption requires an authenticated nonempty deciding prefix. |
| `zetesis_algebra::reduction::Any` (struct) | `Zetesis.AxisFolds.any` (def) | Any combines Boolean accepted-member values by disjunction with false as identity and true as absorber; inner existence still concerns the authenticated proper-subset countermodel class. |
| `zetesis_algebra::reduction::CheckedSum` (struct) | `Zetesis.AxisFolds.BoundedSum.reduction` (def) | CheckedSum denotes bounded nonnegative addition at u64::MAX: Some is exact and None denotes overflow. Although overflow absorbs mathematically, the runtime never advertises early successful absorption; publishing an exact answer count requires complete distinct-member coverage. |
| `zetesis_algebra::reduction::Minimum` (struct) | `Zetesis.AxisFolds.Minimum.reduction` (def) | Minimum denotes optional minimum under an explicit pure lawful total order consistent with denoted value equality; Rust Ord does not prove that premise. Left representatives on ties, witness retention and optimality proof remain separate from the commutative value law. |
| `zetesis_algebra::reduction::Minimum::step` (method) | `Zetesis.AxisFolds.Minimum.step_value_map` (theorem) | The first incumbent requires no comparison; later steps compare once and retain the incumbent on equality. Value selection refines denoted minimum when comparison agrees with that order, including distinct Score representations of equal costs. Ordering receipts and retained witnesses are not commutative values. |
| `zetesis_algebra::reduction::Verdict` (enum) | `Zetesis.Algebra.FoldVerdict` (inductive) | Empty, Complete, Absorbed and Undecided classify authenticated accepted-member coverage. Compact Rust witnesses must relate to the Lean emitted history, and typed stops remain concrete payloads; the carrier itself proves neither membership, exhaustion nor minimum optimality. |
| `zetesis_algebra::reduction::sets::intersection` (function) | `Zetesis.AxisFolds.SetStreams.selected_inter` (theorem) | Filtering a finite occurrence stream denotes intersection only when decoded occurrences faithfully cover the left set and each membership flag faithfully states membership in the right set. Cursor/materialization failures and Model allocation remain outside the pure set-value law. |
| `zetesis_algebra::reduction::sets::union` (function) | `Zetesis.AxisFolds.SetStreams.append_union` (theorem) | Chaining the two finite occurrence streams denotes union after decoding; duplicates are allowed in the stream, while Model retains canonicalization, allocation and failure order. Iterator exhaustion and finite denotation are required. |
| `zetesis_algebra::reduction::sets::ordered_union` (function) | `Zetesis.AxisFolds.SetStreams.ordered_union_value` (theorem) | The merge retains the left representative on equality. For strictly ordered decoded inputs and a comparator faithful to their strict order, `ordered_union_sorted_unique` proves order and absence of duplicates in the merged stream. The Rust cursor must implement the specified advances, allocation and publication. |

Tuple products are the `zetesis_algebra::reduction::Reduction` implementation
for `(L, R)`; there is no separate Rust `Product` owner. Both coordinates are
combined, and both must absorb for the product to absorb.
`Zetesis.AxisFolds.pair` (def), `pair_absorbing` and `pair_idempotent` (theorems)
state the corresponding laws under the component reductions' premises.
Set-stream functions refine the set values of `Zetesis.AxisFolds.inter` and
`Zetesis.AxisFolds.union`; they do not implement a fallible `Reduction` or
represent its universal-set identity. An empty stream is not an empty-family
verdict. Minimum selection, proof of the optimal cost, exact counting and
exhaustion of optimal ties retain distinct contracts.

### Work, counts and control

| Rust declaration (kind) | Lean declaration (kind) | Relation and remaining obligations |
| --- | --- | --- |
| `zetesis_algebra::cancellation::Meter` (struct) | `Zetesis.WorkCharge.charge` (def) | A local Meter under a fixed inclusive bound relates to prefix charging from an admitted starting count. Whole-request checks, borrowed receipts and shared quota/refund ownership are distinct operations; finite-width arithmetic, cancellation and typed refusal remain concrete contracts. |
| `zetesis_algebra::cancellation::Meter::absorb` (method) | `Zetesis.WorkCharge.repeated_eq_charge` (theorem) | Absorbing nested work consumes the available local prefix and records the limit on overrun, under an initially valid count. The overrun control poll and the nested operation's own cadence remain outside the natural-number bookkeeping law. |
| `zetesis_algebra::cancellation::Meter::charge_receipt` (method) | `Zetesis.AtomicWork.charge` (def) | The operation checks the u64 recorded count plus the u128 request for u128 overflow, then against a u64 ceiling. Only acceptance writes the exact total to the borrowed receipt; refusal leaves it unchanged, including zero above a lowered ceiling. AtomicWork models representable sums, not machine overflow or effect order. |
| `zetesis_algebra::cancellation::Meter::check_receipt` (method) | `Zetesis.AtomicWork.charge` (def) | The u64/u128 adapter checks the complete sum for u128 overflow and against a u64 ceiling, returning a proposed u64 total without mutation. Zero still checks an imported prefix. AtomicWork gives the scalar decision; later partial publication requires unchanged prefix/ceiling and actual work no greater than the checked request. |
| `zetesis_algebra::cancellation::Meter::check_wide_receipt` (method) | `Zetesis.AtomicWork.charge` (def) | The pure u128 work check refuses addition overflow before its inclusive ceiling and returns only a proposed total. It neither records performed work nor reserves it. AtomicWork covers representable sums; native errors, polls and later publication remain concrete obligations. |
| `zetesis_algebra::cancellation::Meter::check_count` (method) | `Zetesis.AtomicWork.charge` (def) | The pure usize count check refuses addition overflow before its inclusive ceiling and returns only a proposed total, including for zero under a lowered bound. AtomicWork supplies the representable-sum admission law; resource meaning, attribution and commit order stay with the caller. |
| `zetesis_algebra::cancellation::Meter::check_wide_count` (method) | `Zetesis.AtomicWork.charge` (def) | The pure u128 named-count check has the same atomic decision on representable sums; it neither saturates nor commits. Counts can include cumulative byte charges, which are distinct from actual capacity and byte-peak observations. Native overflow and source publication remain concrete obligations. |
| `zetesis_algebra::cancellation::poll_due` (function) | `Zetesis.ChargeCadence.dueRule` (def) | At the positive POLL_INTERVAL, the zero-based half-open charged range is tested by dueRule without adding its endpoints; zero is never due. Actual cancellation/deadline observations and their sequencing are separate control obligations. |

### Grant ownership and settlement

| Rust declaration (kind) | Lean declaration (kind) | Relation and remaining obligations |
| --- | --- | --- |
| `zetesis_algebra::cancellation::Reservation` (trait) | `Zetesis.WorkPermits.Window` (structure) | An admitted reservation exposes a finite allowance and commits actual use; interpreting it as a valid grant window requires used no greater than granted and the concrete exactly-once settlement/refund ownership contract. |
| `zetesis_algebra::cancellation::Quota::replenish` (method) | `Zetesis.WorkPermits.refund_then_settle` (theorem) | The shared lease returns an owned unused borrow before settling its grant; the default quota retains cumulative partial grants. The scalar law preserves allowance and the exact performed prefix under its ownership premises. Native widths, returned ownership on failure, polling and unwinding remain concrete obligations. |
| `zetesis_algebra::cancellation::Quota::replenish` (method) | `Zetesis.WorkPermits.settled_replacement` (theorem) | Installing an admitted replacement adds no performed work. Atomic minimum admission, waiting and notification remain concrete obligations; the scalar law does not prove scheduler progress. |
| `zetesis_algebra::cancellation::Meter::under_reservation` (method) | `Zetesis.WorkPermits.settlement_partitions_grant` (theorem) | A returned valid work receipt is checked against the admitted allowance before commit, including when the returned value carries an inner failure. Valid use partitions granted work into used and unused. An outer typed error, overrun refusal, drop/refund and panic behavior remain concrete owner obligations. |
| `zetesis_algebra::cancellation::permits::PermitLease` (struct) | `Zetesis.WorkPermits.Window` (structure) | The lease stores a granted/remaining window whose used amount is their difference; native-width and callback invariants must realize the valid natural-number window without creating a second charge owner. |
| `zetesis_algebra::cancellation::permits::PermitLease::take_up_to` (method) | `Zetesis.WorkPermits.Window.take` (def) | Within a valid installed grant, the operation consumes min(wanted, remaining). Refill preserves its native error boundary and occurs on an exhausted lease even for wanted = 0; the pure Window.take law specifies neither refill nor control observations. |
| `zetesis_algebra::cancellation::permits::PermitLease::give_back` (method) | `Zetesis.WorkPermits.Window.refund` (def) | Returning unused permits realizes a window refund only under the bound that they were previously consumed; native overflow and callback/lease ownership are separate runtime invariants. |
| `zetesis_algebra::cancellation::permits::PermitLease::settle` (method) | `Zetesis.WorkPermits.settlement_partitions_grant` (theorem) | Detaching a nonempty valid lease invokes publication once with its granted/used/unused partition; an empty lease does not invoke it. The scalar partition law does not prove callback, reentrancy, unwind or exactly-once source-ledger behavior. |

### Byte requirements and observations

| Rust declaration (kind) | Lean declaration (kind) | Relation and remaining obligations |
| --- | --- | --- |
| `zetesis_algebra::cancellation::bytes::MeterBytes` (struct) | `Zetesis.BytePeak.observe` (def) | MeterBytes owns current capacity; observations update a separately borrowed peak and test the supplied ceiling. BytePeak specifies the scalar observation, not allocations, release lifetimes, logical requests or the whole owner layout. |
| `zetesis_algebra::cancellation::bytes::MeterBytes::check` (method) | `Zetesis.BytePeak.accepted_iff` (theorem) | The pure byte ceiling uses only the decision projection total ≤ limit of BytePeak; it records no peak or current bytes and does not turn requested envelopes into actual observations. |
| `zetesis_algebra::cancellation::bytes::MeterBytes::observe_peak` (method) | `Zetesis.BytePeak.observe` (def) | The borrowed u128 peak becomes max(previous, total) before either ceiling result; only total is compared with the ceiling. The caller retains current-byte ownership and distinguishes requested envelopes from actual capacity. Width conversion, observation identity and refusal order remain concrete obligations. |
| `zetesis_algebra::cancellation::bytes::MeterBytes::observe_peak_usize` (method) | `Zetesis.BytePeak.observe` (def) | The borrowed usize peak becomes max(previous, total) before either ceiling result; only total is compared with the ceiling. The caller retains current-byte ownership and distinguishes requested envelopes from actual capacity. Width conversion, observation identity and refusal order remain concrete obligations. |
| `zetesis_algebra::cancellation::bytes::MeterBytes::observe_peak_u64` (method) | `Zetesis.BytePeak.observe` (def) | The borrowed u64 peak becomes max(previous, total) before either ceiling result; only total is compared with the ceiling. The caller retains current-byte ownership and distinguishes requested envelopes from actual capacity. Width conversion, observation identity and refusal order remain concrete obligations. |

The formula and tight GPU packing adapters use `CheckedSum` for byte totals
and `MeterBytes::check` for the smaller of the granted buffer and binding
ceilings. Their typed errors, allocation boundaries and requested-versus-actual
storage accounting remain with the adapters. The value laws above cover the
sum and ceiling decisions; they do not establish device layout or execution.

### From the object laws to an execution

`Zetesis.Algebra.inside_refl`, `inside_trans` and `inside_antisymm` describe the
region precision order. A Subject is a monotone inflationary map of that poset,
or a pointed endofunctor of its thin category; `Subject.sound_of_narrower`,
`Subject.apply_sound` and `Subject.narrow_ideal` relate it to the abstraction of
its class members under the stated Galois insertion. The common `joint` closure
is an idempotent monad on that thin category, with the closed regions as its
algebras; an individual streamed progress step need not be idempotent. The `joint_closed`, `joint_least`,
`joint_extensive`, `joint_monotone`, `joint_idempotent`, `joint_sound` and
`joint_more_subjects` theorems, with `terminal_agreement`, `Reaches.eq_joint`
and `fair_iteration_reaches_joint`, retain their fixed-family premises.
`Zetesis.Algebra.refines_wf` supplies the finite strict-progress argument.

Rust ranks open atoms only after eager and late agreement. Knowledge must
correspond to the intended closure before the same-region/same-split argument
applies; the marker trait alone cannot establish that correspondence.
`zetesis_algebra::walk::inspect` classifies a narrowed entry without proving
membership or consuming it, and a chosen atom must still be fresh when split.
`Zetesis.CoverageTree.mem_outputs_iff` supplies the accepted-leaf coverage law;
`Zetesis.Pending.Step.perm` and `Zetesis.Pending.Walk.exhausted` describe pending
ownership and exhaustion. A walk unfolds a covering tree;
`Zetesis.Algebra.Subject.node`, `LeafVerdict`, `LeafVerdict.node`,
`answerSetVerdict` and `countermodelVerdict` keep the semantic leaf decision
separate from the schedule. An interrupted leaf decision stays undecided.
Scalar, parallel, slate and served-query schedules
must realize these transitions, including stopped prefixes. Sharing their Rust
operations does not itself prove that refinement.

For pure reductions, `Zetesis.AxisFolds.Reduction.fold_perm` and
`fold_decided_prefix` give value permutation and absorption laws. Complete
world-view folding also needs complete accepted-member coverage; repeated
members require idempotence or a distinct-member stream as appropriate.
`Zetesis.Algebra.foldVerdict_absorbed`, `foldVerdict_complete`,
`foldVerdict_empty`, `foldVerdict_undecided` and `foldVerdict_without_absorber`
keep those outcomes separate. `inner_verdict`, `answer_sets_nested` and
`countermodel_verdict_independent` retain the separate answer-set and
countermodel quantifiers. No value reduction or verdict carrier supplies the
semantic leaf decision, original-input authentication or optimality proof.

The word, meter and permit laws likewise state representation or scalar
obligations. They do not prove all concrete layouts, width conversions,
allocation failures, polling, synchronization, callback or settlement traces.
The correspondence of each consumer and physical implementation remains
separate from the existence of its shared Rust operation.

The retained [native replay](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/REPRODUCING.md)
records the earlier `zetesis_cpu::cancellation` namespace. Its generated Lean,
source hashes and replay guards remain tied to that extraction. Qualifying the
current `zetesis_algebra::cancellation` owner requires a new extraction and proof
refresh; editing a path or expected hash alone establishes no correspondence.

The subjects built from the library's laws:

| Instance | Law it rests on | Rust |
| --- | --- | --- |
| `readings`, and from it `original` and `restriction` | `FormulaBounds.known_sound`, `known_mono` | The original subject's readings and knowledge; a restriction, the objective bound among them, read without producers |
| `frozen`, the head of `countermodelSubjects` | `known_sound` over the frozen reduct, and `ReductRegions.masked_known_sound` for propagation under the candidate's mask | The private frozen adapter reads `zetesis_ferraris::FrozenSubject`. The reduct query takes the existing authenticated `FormulaEvaluation`, derives its theory, candidate and frozen truth, and constructs its adapter, subset region and fresh frozen knowledge internally. It accepts no supplied subject or root state. Published query continuations retain their own authenticated entries; they are not new root constructors. Ferraris now indexes its knowledge and private reading context by the sealed class markers. The frozen closure borrows only common worker scratch; its learning capability is unit. A candidate closure can borrow the worker-owned ledger, which is never copied into regional knowledge. The marker does not bind a particular theory, candidate or state; the existing runtime ownership checks remain necessary |
| `row` | `LearnedRegions.consequence_row_narrows` | The learned-row ledger's narrowing, and algebraic forcing rows (`AlgebraicRows.equation_forces_true`, `equation_forces_false`) |
| `exclusion`, `doubleCount`, through `refuting` | `AlgebraicRows.excludes_region`, `double_count_excludes_region` | Algebraic refutations, a recognized parity row's exclusion and the double count |
| the rows of `countermodelSubjects`: the parity rows and units through `row`, the count bounds through `doubleCount` and the guarded bounds | `Algebra.entailed_by_refutations` over `contains_decideAll` for a probed group's allowed patterns, `Algebra.saturation_unit` for a unit, `LearnedRegions.row_under_unit` for a row found under a guard the class cuts; `AlgebraicRows.upper_of_clique`, `bounds_of_allowed` and `lower_of_clause` giving the count bounds' `LowerOf` and `UpperOf` | The inner preparation probes the closed query root for rows of the countermodel class. The countermodel walk's late tier applies them through the shared parity and cardinality systems. The preparation is built from the theory and the candidate alone and dropped with the query |

What the Rust establishes for those rows, beyond the obligations the arithmetic subjects already carry for forced literals
and threshold guards: recognition reads only the masked DAG and the closed root's knowledge; every probe narrows a copy of
the closed query root with exactly the guard and the pattern decided, in the first probing round by the frozen subject
and in a later round by the frozen subject composed with the row subjects of the round before, each sound for the class
and so their joint closure (`Algebra.joint_sound`), the complete consistency check of those rows entering as a refuting
subject of the class (a `0 = 1` combination under the region's decisions is a sum of class equations,
`AlgebraicRows.equation_sum`, every variable of which the region decides, `equation_excludes_given` and
`excludes_region`); so a refuted probe is the premise of `entailed_by_refutations` with that round's subject as `s`. The
patterns a clause of the group forbids, and those whose restriction to a group whose support this one contains is such
a pattern, are marked refuted without a probe: the region deciding such a pattern lies in the closed root, so a member
of the class in it satisfies the clause's body under the pattern (the clause's rule and the chains to its base standing
in the reduct, their bodies true in the candidate, as the proposal pass reads them) and holds the clause's guard, which no
countermodel in the root holds, cut by the root or a unit by `saturation_unit`; the region holds no member, nor does its
narrowing inside it, which is the same premise, discharged by the proposal pass's reading of the clause rather than by a
probe. A clause too wide for a group whose pattern cuts its whole support is installed as a lower bound of one over that
support (`AlgebraicRows.lower_of_clause`), its all-cut pattern excluded by the same argument. A unit's probe narrows the closed root, `s.narrow (query M)`, with the guard held, where `saturation_unit` quantifies over
the raw query root with the guard held: a countermodel in the law's region holds the guard and lies in `query M`, so by
the frozen subject's soundness it lies in the closed root and, holding the guard, in the probed region, which is how a
probe that reaches the candidate or is refuted discharges the law's premise. A parity row or a count bound is installed only
under a guard the closed root cuts, a saturation unit or a residual root's own decision, and enters without the guard,
which the unit discharges (`row_under_unit`), every region the preparation serves descending from that root; the
preparation is built from the theory and the candidate, authenticated at every use, and never
consulted for another candidate; `add` is the only operation changing a parity row; and a stopped probe declines the
whole preparation rather than marking a pattern. The affine hull of the allowed patterns and the clique search are
trusted computations whose results are checked against the retained pattern tables, not re-read from formulas; the
proposal pass's reading of a clause, where it forbids a pattern without a probe, is a trusted computation beside them.

The support cut (`FormulaBounds.unsupported_cut_with_choices`) and the objective
bound's own lower-bound reasoning
(`RequiredChoiceCosts.lower_bound_preserves_answer_sets_strict`) still require
their full Subject correspondence. `StreamedSubjects` constructs the elementary
subjects of authenticated original constraints and relates completed scans to
closure; its concrete cursor refinement remains separate. The
approximation-fixpoint reading is a frame,
not a claim: regions are its consistent pairs, but downward propagation, learned
rows and algebraic rows are not approximators of an immediate-consequence
operator, so the joint fixed point is a sound closure relative to a class.

## Candidate generation and query representation

Candidate restrictions and storage transformations preserve different objects.
A necessary restriction may remove impossible proposals; a query representation
must preserve the same classical assignments. Neither operation changes the
original program or the reduct used to check membership.

| Law | Implementation boundary | Remaining correspondence |
| --- | --- | --- |
| [`GateRestrictions.answer_set_avoids`, `suffix_region_rejected`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/GateRestrictions.lean) | The closure route's source-derived gate restrictions and seed-region skipping, removed with that route | None now; the law remains as a statement. |
| [`Bounds.narrowed_contains_accepted`, `lower_constraint_refutes`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Bounds.lean) | The closure route's `Candidates::bounded`, removed with that route | None now; the law remains as a statement. |
| [The `split` constructor of `Search.CoverageTree`, `Region.split_partition`, `Region.split_disjoint`, `CoverageTree.mem_outputs_iff`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Search.lean), [`Bounds.conflicting_atom_refutes`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Bounds.lean) | `zetesis_algebra::walk::Walk`, the scalar walk used for candidates and proper-subset queries: a region is narrowed by the caller's narrowing, refuted, offered as one candidate when decided, split on the atom its narrowing preferred or else its highest open atom into the out and in regions, or counted as a flat interval when its narrowing decided nothing beyond the split and the caller counts; each caller supplies its original-theory or frozen-reduct narrowing. Under several workers the formula route walks the same tree in `zetesis_stable`'s parallel regions, a second implementation of the same law: each worker takes its newest region from a deque and idle workers steal older regions from peers | That the Rust split chooses a fresh atom and forms the two regions of the law, in the traversal and in the parallel walk alike, that the counted interval offers exactly the region's seeds, and that the leaf order is the counter's order when no atom is preferred and one worker walks are Rust obligations; with several workers, [`Pending.Step.perm`, `Pending.Walk.exhausted`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Frontier.lean) is the law: the frontier of pending regions, whoever holds each, and the leaves emitted are together a permutation of the tree's outputs after every step in any interleaving, so a walk that empties the frontier emits every accepted leaf once, in the schedule's order; that the workers' deque removals, transfers and active regions form one frontier of the tree is the Rust obligation; the tree's leaves are exactly the accepted seeds only because each leaf seed is checked in full. |
| [`TightPlans.stable_supported`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/TightPlans.lean) | A candidate the complete tight certificate finds with an unsupported present atom is rejected as `Check::Unsupported` without a reduct query; the candidate without that atom is the law's witness | The plan's coverage of the theory's producers is established by its construction, not yet by a proved refinement of that construction; the Rust check's evaluation of producer bodies remains executable evidence. |
| [`DisjunctiveSupport.answer_set_supported`, `answer_set_supported_with_choices`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DisjunctiveSupport.lean) | Necessary ordinary sole-head support and enabled atomic-choice support | Extraction must cover every asserted root, coalesce repeated ordinary head atoms and recognize exact atomic choices in either operand order using semantic atom identity. Choice bodies remain arbitrary original formulas; they supply permission, not a self-premise. The original theory remains the reduct subject. This is necessary support, not ranked sufficiency. The retained binary recognizer is refined by `AtomicChoice.atom_meaning_iff`; native recognizer refinement, complete Rust DAG extraction, Boolean encoding and bounded failure remain unproved. |
| [`PackedQueryLiterals.decode_encode`, `packed_truth`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/PackedQueryLiterals.lean) | None: the removed clause search's packed classical literals in `Cnf` and borrowed clause views | No current Rust correspondence; the law remains as a statement. |
| [`IndexedCandidates.index_equals_all_blocks`, `failed_literal_forced`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/IndexedCandidates.lean) | None: the removed clause search's semantic-projection exclusion index and its independently checked completed assignments | No current Rust correspondence; the law remains as a statement. |
| [`OptionalIndex.successor_fits`, `optional_round_trip`, `replacement_commutes`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/OptionalIndex.lean) | None: the removed clause search's optional positive-successor child links in its compact projection trie | No current Rust correspondence; the law remains as a statement. |
| [`FormulaRegions.classical_consequence_forces`, `classical_consequence_cuts`, `no_model_refutes`, `restricted_consequence_forces`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FormulaRegions.lean) | The removed clause search read as the coverage tree of `Search.lean`: a search node is a cube, the atoms its propagation forces or cuts narrow it, a conflict refutes it, and a complete assignment is a leaf the reduct decides; the support restriction is a restriction every stable model satisfies | No current Rust correspondence; the law remains as a statement of how a classical search relates to the region tree. |
| [`FormulaBounds.read_sound`, `never_root_refutes`, `known_sound`, `known_mono`, `unsupported_cut`, `sole_support_forces`, `unsupported_cut_with_choices`, `sole_rule_forces_with_choices`, `sole_choice_forces`, `known_blocked_no_support`, `known_choice_blocked_no_support`, `restriction_forces`, `restriction_cuts`, `restriction_contradiction_refutes`, `restricted_stable_narrowing`, `decided_leaf_models`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FormulaBounds.lean), [`FormulaBounds.disj_chain_sure`, `disj_chain_never`, `disj_chain_unit`, `conj_chain_sure`, `conj_chain_never`, `conj_chain_unit`, declared in `FormulaChains.lean`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FormulaChains.lean) | `zetesis_ferraris::Narrower::narrow_known`, the narrowing of the regions proposer in `zetesis-stable`: the sure and never readings of every node under a region, the knowledge closed in both directions over the DAG, the support cut and the sole-support rule, to a fixed point; `zetesis_algebra::walk::Walk` walks the tree of `Search.lean` for candidate generation and proper-subset queries with one worker, and `zetesis_stable`'s parallel regions walk it for the formula route with several | The Rust closure must agree with `Known` on the admitted DAG, and `producers` must extract the covered fragment from the roots. The Rust closure must derive only `Known` judgements; the fixed point is the least one because every rule only adds knowledge, and a child region may start from its parent's knowledge by `known_mono`. The closure reads each maximal tree of one connective as one node with a neutral-operand count exact until a processed absorbing witness, and applies the chain rules, each admissible in `Known` as a sequence of the binary rules along the chain (`FormulaChains`); that the Rust chains are such trees, with inner nodes that have that one parent, and that a node false under the frozen mask is read by its operands' masks, are Rust obligations. That a fully decided region no reading refutes is a classical model is `decided_leaf_models`, which is why a leaf is proposed to the reduct without a classical check. An atomic choice is a producer of its atom that only an impossible body blocks, a choice having no other head: `unsupported_cut_with_choices`, `sole_rule_forces_with_choices` and `sole_choice_forces` state the support cut and the sole-support rule over ordinary producers and choices together, from `DisjunctiveSupport.answer_set_supported_with_choices`. The narrowing blocks a producer by the theory's knowledge, a body known to fail or another head known to hold, and not by the readings alone; `known_blocked_no_support` and `known_choice_blocked_no_support` say such a producer supports its atom in no classical model of the theory inside the region, which is the premise those laws ask. That `producers` recognizes the choices of the theory, in either operand order, remains a Rust obligation. A restriction, the support restriction or an objective bound, is narrowed by its readings alone, without producers: `restriction_forces` and `restriction_cuts` are its knowledge sound in its models, and `restricted_stable_narrowing` is the region narrowed by both the theory's and the restriction's knowledge keeping every stable model that satisfies the restriction; that every model still sought satisfies each restriction the enumeration adds is the Rust obligation. |
| [`InnerQuery.exhausted_walk_stable`, `emitted_refutes`, `exhausted_walk_decides`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/InnerQuery.lean) | A published subset query in `zetesis-stable`'s parallel regions: its pending regions, served by any worker, decide the candidate through the query cell | That the surrendered and visited regions partition the query's root, and the cell's counting, ordering, resolution and stop rules, are Rust obligations, stated with the walk's frontier protocols above. |
| [`ReductRegions.leaf_refutes`, `exhausted_stable`, `countermodels_exact`, `stable_iff_no_countermodel`, `masked_read_eq_reduct`, `masked_reads_falsum`, `masked_known_sound`, `masked_known_mono`, `masked_known_narrows`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ReductRegions.lean) | The reduct's proper-subset query as a region tree: `ReductQuery` in `zetesis-stable` walks the subsets of a classical model with the frozen reduct's knowledge, returns a leaf other than the candidate as the countermodel and a covered tree as stability | That evaluation of the original DAG under the mask is the reduct's is `FerrarisMask`'s law. That the narrowing's two readings under the mask are the readings of the reduct is `masked_read_eq_reduct`; a masked node reads as falsum whatever its operands read (`masked_reads_falsum`), so it has no rules of its own, and the nodes above it combine that reading as any operand's. `masked_known_sound` says propagation under the mask, where every rule relating a connective to its operands asks that the connective be unmasked and a masked node is known to fail, knows of an original formula only what propagation over the reduct theory knows of its reduct, and `masked_known_narrows` that the query may narrow by it. `masked_known_mono` preserves this abstract knowledge in a more precise region with the same theory and candidate; every connective keeps its own-node mask premise. This does not establish equality with a concrete worklist closure or authorize transferring knowledge between candidates. That the stored mask is each node's truth in the candidate, and that the Rust propagates by those rules over the shared DAG, are Rust obligations; that the traversal covers the query root exactly is `Search.CoverageTree`; the countermodel is validated independently before it is returned. |
| [`CandidateCursor.completed_coverage`, `stable_outputs_exact`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/CandidateCursor.lean) | None: the removed clause search's retained candidate traversal, completed checks and exhaustion | No current Rust correspondence; the law remains as a statement. |

The [execution chapter](../architecture/execution.md) explains these operations
in the solver.

The packed `Region` stores disjoint held and cut masks over real atoms.
`Knowledge` owns one word block with disjoint node-sure, node-never, atom-sure,
atom-never, seen and processed chain-witness spans. Its chain-neutral counts
(exact until a processed absorbing witness) and exact atom-ranking counts
retain their selected width. Splitting the block once into borrowed slices changes no
`FormulaBounds.Known` judgement or propagation charge. Checked span boundaries,
complete copying of shape and contents, independent child ownership and truthful
retained-byte receipts remain Rust obligations; `known_mono` does not verify this
layout, allocation behavior or performance. Counter widths are chosen from
`u16`, `u32` and native width using the largest chain length and initial per-atom
parent count. Rust must establish that this bound includes every parent occurrence
of every node carrying an atom, including distinct nodes with the same atom.
Checked updates, unchanged width during propagation and complete independent
copies must preserve those counter meanings. These representation obligations add no
premise to the semantic propagation laws. The seen snapshot denotes the region
decisions already incorporated; differencing the
current decided mask supplies only new decisions, and the completed snapshot
also includes atoms learned by propagation. `FormulaBounds.known_mono` supplies
the mathematical ancestor-inheritance law. The Rust API additionally requires
the same narrower, theory, producer set and frozen truth, and knowledge closed
for an enclosing region. The law neither proves the packed representation nor
validates those unchecked caller preconditions. Exact word/index conversion,
zero unused tail bits, polarity decoding, complete seen snapshots and correct
child cloning remain Rust obligations. The cross-word original/frozen
regressions are executable evidence for those boundaries, not formal refinement.
Retained-byte accounting includes the owned masks; the full seen-mask scan and
snapshot writes are outside the existing charged-read work counters.

The producer support continuation retains two positions in each long head row.
Every producer before the first and strictly between the two positions has been
proved blocked by a false body or a competing held ordinary head. Those facts
are monotone within descendants under the same original subject. Both remaining
endpoints are rechecked; only complete coverage can establish no support or sole
support. An open head's first witness cannot substitute for the second witness
required after that head becomes held. Choices keep their existing support rule.
`known_blocked_mono` and `known_choice_blocked_mono` transfer the existing
ordinary and atomic-choice blocking judgments to every region inside the first,
using `known_mono`. These laws preserve blocked witnesses, not the number of
unblocked witnesses: a sole remaining supporter may later become blocked too.
`known_blocked_no_support`, `known_choice_blocked_no_support`,
`unsupported_cut_with_choices` and the sole-support laws establish their semantic
consequences. They do not verify the Rust cursor or define a complete support
Subject. Exact row identities and bounds,
complete prefix coverage, independent cloning and parent restoration, metered
initialization, typed interruption and actual-capacity receipts remain executable
representation obligations. No continuation state is used for frozen-reduct or
candidate-only restriction narrowing.

The chain accumulator retains exactly the information its consumers read.
For disjunction, false operands are neutral and a true operand is absorbing;
for conjunction these roles are reversed.
[`ChainEvidence.records_counts` and `propagation_observations`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FormulaChains.lean)
show that one neutral count and an absorbing witness preserve the observations
of two exact counts after every finite sequence of processed notifications.
The witness is set when an operand event is processed, not when its node bit
first changes. Rust must establish that each relevant operand occurrence
supplies exactly its applicable notification, that frozen masks skip the same
events, and that parent decisions, queues and charged work are unchanged.
The abstract accumulator laws do not establish those event-loop or packed-word
properties. `ChainEvidence.settled_record_observations` and
`settled_records_observations` additionally show that ignoring notifications
after a processed absorbing witness preserves the observations relevant to an
unrefuted chain. The Rust parent dispatcher applies this rule only after the
same event has established the root at its absorbing polarity. Its neutral count
is then retained unchanged; pending node bits do not establish that witness.
The parent visit remains charged, and every node event, other parent, support
wake-up and split-ranking update remains active. The laws do not establish that
Rust reaches only these consistent states or preserves those other obligations.

The last-operand scan in `Closure::unit` can return unchanged when an already
processed operand witnesses the required polarity: sure for a disjunction,
never for a conjunction. Its callers have counted every other operand at the
opposite polarity, so the sole target is already known. The existing
`FormulaChains.disj_chain_unit` and `conj_chain_unit` laws supply the semantic
unit rule, not this counter implementation. Rust must establish unique operand
incidences, counts that never lead their corresponding bits, preserved pending
events and contradiction handling. A set bit whose event is still pending does
not alone justify the shortcut. The omitted scan changes charged work and
resource-limited prefixes; no exact old work count is claimed.

The same processed-count invariant permits `Closure::teach_chain` to omit a
complete downward scan when every conjunction operand is already sure, or every
disjunction operand is already never. Each omitted update would be unchanged.
The root event still updates parents, ranking and support wakeups. Pending bits
without a complete processed count retain the scan. This does not change the
`FormulaChains` laws or discharge the concrete queue and counter obligations.

A propagation operation stops when it first establishes contradiction. The
remaining operands, parents and support wakeups cannot restore a model:
`FormulaBounds.known_contradiction_refutes` excludes every answer set in that
region. Rust must abandon its unfinished knowledge, restore borrowed state and
clear pending scratch before reuse. The earlier exit charges only work actually
visited; it may establish refutation before a quota would have stopped the old
trailing scan. This is a semantic justification, not a proof of those Rust
state-management obligations.

The region candidate walk and frozen proper-subset queries share one
immutable index constructed with the exact original `Theory`, built by its one
owner when the walk first needs it. Reuse checks instance identity. Each
region or query retains private `Knowledge`, and each candidate supplies
freshly authenticated frozen truth. Sharing preserves the subjects of the
existing `FormulaBounds` and `ReductRegions` laws, and building the index later
builds the same index; it changes ownership and timing, not the definition of
a reduct. Index construction, identity checks, mutable-state separation and
attribution of construction work remain Rust refinement obligations.

The immutable region adjacency stores each ordered incidence row as a slice
between two offsets in one contiguous entry vector.
[`AdjacencyRows`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AdjacencyRows.lean)
proves exact slices from concatenated mathematical rows and unchanged ordered
row folds. Decoding every Rust row must recover that original occurrence list
exactly, including duplicate atom
occurrences and original producer identity; equality of membership sets alone
would not preserve chain counters, split ranking or bounded traversal prefixes.
The same node used on both sides of an implication is still coalesced by the
existing incidence construction, before storage compaction. Distinct atom nodes
carrying the same atom remain distinct incidences. Exact row decoding leaves the
`FormulaBounds`, `FormulaChains` and frozen `ReductRegions` rules unchanged.
Those semantic laws do not establish the Rust count/prefix/scatter construction,
machine offsets, reservation failures or event order. Ordered-row and existing
propagation regressions provide executable evidence for those correspondences.
The checked compact builder adds linear construction passes while retaining the
existing logical node-index receipt; it does not change mutable `Knowledge` or
its copy behavior, and does not claim that all index allocations are fallible.

Chain preparation classifies single-parent interiors before collecting each
maximal chain once. It preserves root identity, left-to-right distinct operands
and the boundary at shared or asserted nodes. The existing `FormulaChains` laws
therefore remain the semantic contract. Differential tests compare the concrete
collector with a bounded recursive definition; these are implementation evidence,
not an extracted Lean proof of the collector. Current Rust formula storage
retains native conjunction and disjunction rows: pairs are inline, wider rows
use the paired operand arena, and `NodeView` exposes every operand. The abstract
`OperandArena` and `OperandTable` laws specify row decoding and original/frozen
truth. The extracted `NativeRows` and `NativeRowsArena` laws check the row
getters and their ordered-list bridge. `NativeAdmittedData` and `NativeExecution`
add actual paired-table admission and completed original/frozen evaluation.
Public membership, runtime effects and source-compiler correspondence remain
separate obligations.

The batched parallel proposer separates classical candidate production from
membership. Its workers use the original region readings and disjoint splits;
they do not certify stable models. Read `Frontier` with a proposal family that
contains every still-required stable interpretation. Necessary support cuts may
exclude other classical models, so classical truth alone is not the proposal
coverage predicate. `FormulaBounds` supplies the preservation premises for
these cuts and decided classical leaves. `Pending.Walk.perm` accounts for leaves
emitted so far together with those still held by the frontier. The producers'
frontier is their deques together with the region each producer holds: a split
publishes one child and keeps the other in hand, a steal or a return to a deque
only transfers ownership, and the count of unresolved regions is raised before a
published child becomes visible. A leaf that finds every output slot reserved
returns its region to a deque unresolved, which again only transfers ownership,
and a return refused entry capacity records a stop. So every stop-free step
permutes that frontier and the emitted leaves, and a slate reports exhaustion
only from a stop-free join at a count of zero. The count, the hands that bound
the regions in hand, the output-slot reservation at a leaf that bounds the
emitted candidates, the stop a refused return records, and the hand-over are
Rust obligations beside the walker's queue and gate protocols, as is the stop's
order: a failed region returns to a deque only after the stop closes the slate,
and a producer checks for the close after each take, so no region is stepped
again after its step failed. An optimizing search's producers share one deque,
taking the region published most recently; the frontier is still the deques
and the regions in hand, every transition only moves a region between them,
and the same permutation argument applies. Only the order of exploration, and
the bound on queued regions, differ.

After producers join, emitted candidates enter the pending membership ledger.
`BatchAccounting.exact_commit_preserves_soundness` justifies committing a fully
classified prefix; `completed_results_exact` additionally requires coverage and
an empty queued and pending remainder without delayed failure. Batch capacity
is a scheduling boundary, not exhaustion. A producer stop retains completed
candidates and reports the unresolved remainder as incomplete; it cannot turn
an empty delivered batch into a complete negative result. The concrete queue
ownership, reservation arithmetic, panic/cancellation paths, budget settlement
before checking and transfer between slates remain Rust obligations. The two
abstract laws do not themselves verify that concurrent implementation.

Joined producers retain source checkers and narrowing scratch across slates.
A stable owner outlives every borrowed checker; a producer's workspace is used by
one task at a time and keeps no candidate knowledge. Candidate knowledge remains
with the frontier entry. Terminal cleanup releases the workspaces without
changing pending candidates or failure receipts. These ownership and reuse invariants,
including the owner/dependent container, remain Rust correspondence obligations;
they introduce no new answer-set law and are not established by the existing
batch theorems.

The CPU batched completion and the GPU formula routes share one internal batch
boundary. Independent routes check original satisfaction before invoking a
checker; joint routes establish it together with the reduct verdict. The
host receipt consumer retains exact theory and candidate-slice association
and checks the result count. Neither check proves the soundness of a verdict or
its association with the right position within that slice, so the batch laws
take a sound route for the selected operation as an explicit premise; each route
is zetesis's own and is qualified with it. Exact residual completion uses the
existing host checker; an interruption or fault cannot discharge its pending
candidates. Rust ownership, resource compliance and transport identities remain
implementation obligations.

Adaptive GPU batching, the default for GPU formula sessions, chooses each
proposal target at the next selection after a complete successful commit, from
the committed slate's fill, the candidate bound's generation, the pending
owner's memory capacity and a shortened checked device prefix. None of these
observations is semantic evidence. Failed checks preserve pending candidates and
the prior target. The existing batch laws still require exact
classification of every occurrence before commit. Checked sizing, allocation
refusals and propagation of prefix observations remain Rust obligations.

Scalar and joined batch proposals retain one original `EvaluationWorkspace`
per owner. Every new candidate checks exact theory identity and recomputes original
truth, resetting numeric validity; capacity reuse supplies no semantic receipt.
Failed validation retains only the checked prefix. External-checker retry uses
that pending prefix without evaluating it again. Actual workspace capacity and
its header share the pending-byte allowance, and optional oversized scratch can
be released when that allowance decreases. Validation work is recorded apart
from search and reduct work. The existing evaluation and batch laws express the
required truth and occurrence invariants; they do not verify these Rust resets,
failure paths, retained-capacity charges or their composition with publication.

## Representation and source laws

The `zetesis` facade's construction macros generate the canonical themelios
constructors, and its modules re-export the original upstream types. These
exports add no admission or lowering step. Macro translation, source admission
and concrete lowering remain executable correspondence obligations; re-exporting
a type does not establish them.

The `zetesis-engine` adapter connects these values to native CPU sessions. Its
remaining correspondence obligations include preserving the program at `lower`,
enumerating the full unscored family, exporting every atom, keeping display
selection separate, and mapping a completed or interrupted run to its actual
conclusion. Named-part refusal retains the exact canonical part key, without a
fabricated statement or span. Executable conformance tests exercise this
boundary; they are not an end-to-end Lean proof. The existing scalar-checker
refinement does not establish adapter or query-layer correctness.

Source admission checks authored syntax before raising; typed admission bounds
the canonical rule, term and symbol structure. A logical program cannot recover
an erased empty program section or the original multiplicity of set-valued body
syntax. The source checks therefore remain in place. Both doors now use formula
preparation. The shared compiler, statement-evidence borrows and resource
traversal remain Rust correspondence obligations; semantic laws alone establish
neither source admission nor equality of the two doors' admitted input sets.

The full formula door, `prepare_program_formula`, borrows the canonical program
for structural, text and provenance preflight before normalization and analysis.
It then uses the same formula compiler and grounding routes as source admission.
The original `Arc<logical::Program>` remains available through preparation,
grounding and typed-input refusals. A statement `ProgramSite` identifies its position in
that owner's canonical order and carries a parsed coordinate only when one
exists. Retaining the right owner, resolving those positions and preserving
causes through deferred work remain Rust obligations. Neither shared code nor
retained evidence proves source-to-ground equivalence or makes the source and
canonical admission boundaries interchangeable.

Source scalar validation and construction share a bounded symbol walk and the
core borrowed node's text/spelling measures. The validation consumer retains
logical bounds without constructing an output value; actual capacity admission
belongs to construction. This changes storage work, not the value's mathematical
identity or the requirement to validate inactive authored expressions. The Rust
borrowed-view correspondence, traversal and capacity checks remain executable
obligations; no Lean source-to-value refinement is claimed by this separation.

The public `symbols::term_with` and `symbols::atom_with` exports construct an
upstream `Symbol` from borrowed core values. Their contract preserves complete
typed values and atom signs; display selection remains a separate operation.
Their callbacks bound work and permit interruption before navigation, copying
and assembly. `TermRef::depth_with` checks each step of an arbitrary ingress
subtree's allocation-free depth scan, whose worst-case work is quadratic;
complete ingress roots and canonical terms use cached depth. These conversions,
first-refusal behavior and cost bounds remain executable obligations. The
refreshed evaluator extraction does not select the depth API or symbol exporter.

[`ColumnRelations`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ColumnRelations.lean)
relates complete typed tuples to aligned equality-ID columns. Dictionary round
trips and exact cell encoding imply reconstruction; equality selection returns
exactly the satisfying subsequence in original order. `source_posting_exact`
preserves the complete ordered posting for one source equality, so a fixed
shortest-posting chooser can retain its original matcher visits.
`row_mask_roundtrip` decodes exact membership bits to the same increasing row
selection; it assumes unique positions and the correct row domain.
`full_matches_preserved` assumes a total Boolean
matcher and that every complete match satisfies the prefilter. The Rust
[`relation` module](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-core/src/relation.rs)
provides checked owner-bound views. Construction, catalog mapping, resource
accounting, fallible matching and any device masks still need executable
correspondence arguments. The [proof guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/column-relations.md)
explains the hypotheses with a correlated-tuple example.

[`CompactColumns`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/CompactColumns.lean)
adds exact finite-word storage under explicit representability hypotheses.
Successful packing preserves the complete column; widening preserves all old
cells, and compact equality selection retains the same ordered rows. These laws
cover the mathematical 8-, 16- and 32-bit representations. The Rust `OwnedColumn`
storage must still establish width sufficiency, atomic publication and capacity
accounting. The GPU transport must additionally establish its header offsets,
word alignment and lane decoding. Host and physical tests check those boundaries;
the abstract laws do not certify the Rust or WGSL implementations.

A consumer needing only the shortest posting can resolve each equality and
retain that posting without materializing an owned query. Every requested
equality must still be validated, including later columns after an absent value.
The dictionary and posting laws apply to completed resolution; they do not
justify skipping errors or charging work only after it has occurred.

Exact structural-argument probes use the same necessary-prefilter obligation.
When every named input is bound and no wildcard occurs, reverse preorder must
resolve the exact constructor value, preserving name, sign, arity, ordered
children and aliases. A successful full match must therefore satisfy that
whole-column equality; canonical absence must imply absence from the current
support snapshot. The Rust
[`structural` probe](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/structural.rs)
uses the existing term lookup and column postings, then retains the complete
matcher and source-occurrence delta interval. It writes no incoming slot and
publishes no term. This exact-value probe cannot narrow a partially bound or
anonymous argument.

For captured lazy constraints, the Rust
[`PatternRows`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/prepared/pattern_rows.rs)
retains original row positions accepted by the structural matcher with an empty
incoming binding. [`ConstructorPatterns.acceptance_iff_consistent`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ConstructorPatterns.lean#L173)
and [`StructuralBindings.matching_complete`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/StructuralBindings.lean#L151)
supply the abstract necessity argument: a successful bound match gives a
consistent extension, which also extends the empty binding, so additional
incoming equalities cannot rescue a static mismatch.
[`DomainBindings.guarded_continuations_exact`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DomainBindings.lean#L79)
preserves ordered complete continuations under such a necessary selection;
[`TableBindings.join_family_preserved`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/TableBindings.lean#L161)
composes unchanged step families. The runtime may choose this posting or a
smaller existing equality posting, while retaining the full matcher and original
source atoms. This grants no candidate truth. Correspondence still requires
complete pattern extraction, constants and aliases, exact occurrence and
completed-carrier ownership, increasing row positions, and unchanged residual
scalar and signed-body checks. Preparation runs after source-family capture;
it cannot replace that validation or suppress its warnings. Fallible matching,
charged preparation and retained capacity, cancellation, and interrupted prefixes
remain Rust obligations. These general laws and executable regression tests do
not constitute a refinement proof of `PatternRows`.

Within one Join, successful argument resolution may retain its complete scoped
input projection and canonical result for the same immutable argument.
[`BindingScopes.readAll_agrees`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/BindingScopes.lean)
states agreement on the selected inputs;
[`CanonicalTemplates.substitute_decodes`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/CanonicalTemplates.lean)
supplies the ordered substitution
interface. Neither proves the reverse-preorder constructor resolver. Reuse
requires present, equal inputs in the same vocabulary and a complete prior
success. Current reader authentication remains necessary: equality of scoped
IDs alone does not establish an accessible prefix. The fixed support and
append-only writer must preserve prior successful identities; missing identities
are never retained across growth. The full matcher and source-occurrence
selection still run. Dependency extraction, complete record replacement,
borrowed source lifetime, cancellation and leased-capacity accounting remain
concrete Rust obligations.

Constructor lookup soundness/completeness, authenticated scope and prefix,
reverse-preorder decoding, and leased scratch/refusal accounting remain Rust
obligations. The column laws do not prove these operations. Executable
regressions compare the complete recursive formula graph and source origins,
original truth, and frozen truth for arbitrary candidate/tested interpretations.
`StructuralBindings.retained_atom_context` gives the semantic context law once
the same source atoms are retained; it supplies neither the concrete lookup
proof nor a subset premise. Append and closed-support queries have separate
storage envelopes, and neither may retain a missing identity across writer
growth. No source-level executable refinement is claimed for this optimization.

The append dictionary's canonical inverse implements the same encoder. Each
published representative must have exactly one inverse entry naming its local
equality ID, and every inverse entry must name that representative's whole
canonical term. Under this invariant, an authenticated identity miss is a true
dictionary absence. The semantic AVL remains responsible for new-value placement
and queries without an eligible canonical identity. Rust must preserve the
invariant through append, refusal, capacity growth and clear, and validate the
vocabulary and readable prefix before lookup. These transaction and resource
properties are executable obligations; the dictionary round-trip laws do not
prove a particular hash-map implementation.

[`FiniteTables`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FiniteTables.lean)
relates intersections of value supports to surviving complete rows. Projection
preserves these rows, contracts their domains and is idempotent for a single
table. Recomputing from the original table preserves previously surviving rows
when domains widen. The [Rust primitive](../rust/finite-tables.md) borrows typed
values and retains row identity. Direct selection stops at an ordered row mask;
full projection additionally returns witnessed domains.

[`TableBindings`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/TableBindings.lean)
connects this row law to positive source matching. `flat_match_survives` derives
alias coherence and necessary constant/incoming-binding domains from
`StructuralBindings.matching_sound`; canonical table labels have explicit source
argument origins. `indexed_matches_preserved` preserves the ordered list of row
occurrences and resulting bindings. `join_family_preserved` composes equal step
families over the same finite schedule, retaining full source-occurrence/row
traces, including repeated predicates and duplicate-valued row positions.

The explicit table grounding strategy consumes completed possible support and
retains the existing matcher. Structural patterns and growth-round snapshots
keep indexed probes. Concrete scope construction, typed decoding, packed bits,
owner lifetimes and cumulative resource accounting remain executable
correspondence obligations. Relational mismatch in the mathematical matcher
does not represent interruption or an undefined authored expression. Comparisons,
negative conditions and aggregate validation keep their separate continuation;
the table cannot silently prune a required error. Source rows remain original
atoms in the emitted formulas. The [binding guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/table-bindings.md)
separates these obligations from the independent support-coverage and reduct
arguments.

[`AggregateDependencies.Reuse`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AggregateDependencies.lean)
models finite reuse across parent frames under one fixed support. The builder
receives only the complete projected input row; a missing input or failed build
is a typed failure. Restart compares newly read projections of the retained and
incoming frames, then resets the one successful carrier or builds a new one.
`restart_exact` and `reuse_rows_exact` preserve the full ordered values and their
current-parent association. `clauses_exact` maps successful traversal to the
existing clause sequence, so original activation and every equality retain their
original and arbitrary frozen M/J meaning. Carrier coverage still needs the
existing coverage hypotheses. The abstract traversal compares whole completed
lists or typed failures; it does not certify streamed failure prefixes, Rust
invalidation, canonical value identity, dependency extraction, support lifetime,
allocation, work charging or cancellation. Its finite output-step fuel is not a
bound on those implementation costs.

### Numeric selection in local joins

The common
[`Join::select_numeric_local`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/expression_keys.rs)
serves explicit aggregate and factored-component consumers. Its condition grammar
is restricted to positive flat patterns and comparisons of bare variables. Every
comparison read comes from the current prefix or a positive pattern in that join.
It changes neither the plan's order nor its coverage or source-evidence authority.
The numeric query separately authenticates the complete source-column certificate,
current bound, source occurrence and original row interval.

For each selected step, a complete passing continuation must satisfy the same
comparison that selected its row. An omitted row therefore has no complete passing
continuation. [`DomainBindings.guarded_continuations_exact`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DomainBindings.lean)
then preserves the exact ordered completion list, including repetitions;
[`TableBindings.join_family_preserved`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/TableBindings.lean)
composes equal ordered steps. This is a claim about successful continuations,
not equality of all intermediate row visits or an equivalence of error prefixes.

The Rust correspondence must establish that the admitted grammar has no skipped
arithmetic or nested-family diagnostics, that required reads are present when
used, and that numeric selection retains original source order and full matching.
Aggregate tuple evaluation occurs only after a passing local row in both routes.
Consequently the existing ordered-family replay obligation still requires the
same current passing stream; equality of tuple sets alone is insufficient.
Unsupported local syntax, mixed columns and unavailable current bounds retain
the ordinary route. Original family-evidence joins remain separate.

Fallible ownership checks, exact charged visits, temporary and retained capacity,
atomic selector publication, refusal priority and cancellation cadence remain
concrete Rust obligations. The named laws do not prove those operations or assert
that every eligible source will run faster. The executable fixtures check actual
aggregate/component selection, ordered native payloads and provenance, unsupported
routes, and direct charged-prefix publication boundaries.

### Ordered aggregate-family replay

[`AggregateOccurrenceReplay`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AggregateOccurrenceReplay.lean)
adds an explicit ordered observation boundary. `simulate` and `join_trace_exact`
require a relation between current producer and consumer states that preserves
each observation, exhaustion and semantic failure. `replay_exact` then aligns
retained coordinates with the actual fresh rows, including duplicate occurrences
and requiredness. `reduction_exact` feeds that identical ordered list to the
named `Reduction`; it does not appeal to set equality or commutativity.

The step-correspondence premise remains explicit. Its Rust instantiation must
cover derived local values and current control state as well as inherited
family inputs. Compiler read extraction, immutable-plan and prefix evidence,
exact source/support/vocabulary ownership, tuple decoding, stage publication,
allocation, work and cancellation remain correspondence obligations. The laws
preserve a completed observation or semantic failure, not an identical prefix
under a tighter resource ceiling. Exact original and arbitrary frozen M/J tests
check concrete emitted formulas; they do not discharge those obligations or
establish a performance gain.

The support producer's
[`completed_with`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_binding_cursor.rs)
check has a different observation boundary: it can omit an already exhausted
continuation whose complete source read summary agrees under the same rule and
support. `AggregateDependencies.SupportReuse.reuse_exact` states the separate
mathematical law: a finite traversal may coalesce consecutive projections when
all continuations are complete and equal projections determine the same head
image, defined flag and first-zero diagnostic. Head union and this evidence are
idempotent, with the earliest diagnostic retained. The module includes a
counterexample showing that head-image equality alone is insufficient.
This law concerns completed observations; an immediate fatal failure or an
unfinished continuation supplies no completion premise. Completing one
continuation does not finalize its enclosing source family's arithmetic. It is distinct from `reuse_rows_exact` and
`clauses_exact`, which preserve parent/child occurrences.
Positive base matching still runs, and final formula emission retains every
witness activation. The compiler's conservative `continuation_inputs` extraction,
generated versus relational slot separation, completed traversal, canonical
ownership, resource charging and diagnostic identity remain Rust correspondence
obligations. Support-only differential tests hold formula factorization disabled
on both sides and compare the unchanged graph, roots, provenance and diagnostic
outcomes; they do not prove the source scanner or a Lean implementation refinement.

The [matched-row formula consumer](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground/producer.rs)
reuses reached subformulas while retaining every complete producer occurrence.
Original conjunction semantics and repeated application of
[`RuleFactorization.reduct_conj`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/RuleFactorization.lean)
justify balanced conjunction grouping for arbitrary frozen M and J, without
asserting positive support atoms true. The Rust reuse key is an ordered tuple of
row positions under one group's fixed source occurrences and immutable join
snapshot. Equal positions under another relation or snapshot are insufficient.
Saved formula IDs must retain their meaning in the same append-only node owner.
Only complete selected descendants may populate the maps; all producer
conditions, roots and source origins must still be published.

These owner and traversal invariants, native node construction, cancellation and
charged map operations remain executable correspondence obligations. Retained
space grows with distinct reached subtuples, and sorted insertion can require
quadratic total movement; the semantic laws supply neither resource bounds nor
an admission guarantee. Differential tests compare original and arbitrary frozen
truth, with exhaustive small cases and sampled larger cases. They do not prove
the Rust map or producer traversal.

For independent component factoring,
[`RuleFactorization.original_factorization` and `frozen_factorization`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/RuleFactorization.lean)
prove the replacement of a complete Cartesian product of two finite body
families by one implication whose antecedent conjoins their disjunctions. The
conditions may be arbitrary formulas, and frozen equivalence holds for every M
and J. The laws assume that product explicitly; they do not prove its extraction
from source variables, arithmetic or completed support.

The Rust component route must establish independence after fixing the head,
check fixed assignments as equalities, and retain every producer alternative.
Its arithmetic certificate checks every matching completed head row before
emission; undefined arithmetic declines the route, while typed resource and
interruption failures propagate. The shared inhabited-witness preflight covers
both the ordinary and arithmetic profiles: every component supplies a selected
row before any component names atoms or records source provenance. This proves
a complete substitution exists, not that its body is true. These checks,
authoritative source-family diagnostics, provenance, rebuilt support guards and
charged transient storage remain Rust correspondence obligations. Existing
semantic laws do not establish an implementation refinement.

Component cursors share immutable ordering and comparison metadata only under
the same support owner, literal scope and fixed-slot shape. Their held positive
rows supply canonical atom identities at component-local literal positions;
unchanged-prefix node reuse stays within one cursor and fixed head. Neither
metadata nor identity reuse establishes body truth. Exact traversal, owner
checks, storage receipts and skipped scalar-rejected rows remain executable
correspondence obligations, covered by differential tests rather than new Lean
refinement claims. CPU and GPU-selected sessions share this host preparation.

Formula witness sharing has its own observation boundary.
[`RuleFactorization.original_factorization` and `frozen_factorization`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/RuleFactorization.lean)
replace a finite family `(W_i ∧ C) → H` by `((∨ W_i) ∧ C) → H`, specialized to
one common continuation C and head H. Each W_i is a whole correlated positive
witness, and C may be an arbitrary formula. The law applies separately when
different continuations have different heads; its frozen equivalence holds for
arbitrary M and J. The grouped antecedent also becomes the head's producer.
`satisfies_any` preserves the original truth of the OR of producer bodies, and
`Ferraris.double_neg_formula_reduct` transports that equality through the actual
double-negated support guards. The rebuilt guards must be included in the
equivalence, rather than treated as an unchanged surrounding theory.

The Rust route groups only consecutive equal inputs under one rule and immutable
support, using the complete continuation summary. It checks flat positive
patterns and total prefix decisions before looking ahead, validates rejected
rows, and waits for a selected complete row before publishing witness atoms.
These applicability checks, complete tuple and scoped read coverage, active versus
lookahead comparison certificates, typed ownership, arithmetic evidence and
interruption/resource admission are executable correspondence obligations.
Exhaustive small tests align decoded atom identities and compare the entire
original and arbitrary frozen theories, including support guards; they do not
establish a refinement of the Rust state machine.

[`SupportTransposition`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SupportTransposition.lean)
separately groups double-negated support guards by shared condition. Its
`SameIncidences` premise requires every head-condition pair in either orientation
to occur in the other. `original_exact` and `frozen_exact` preserve the truth of
the whole guard family, with arbitrary outer and tested interpretations.
`answer_sets_in_context` preserves membership in any unchanged surrounding
theory. These laws replace equivalent guard families; they require neither a
necessary-support entailment premise nor equivalence between individual roots.

The Rust [guard builder](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground/support_guards.rs)
selects heads with exactly one recorded producer. A conjunction contributes its
direct children; another body contributes itself as one condition. Complete
counting and filling by unchanged formula coordinates must establish the exact
incidence premise. Ordinary roots and unselected guards remain the surrounding
theory. Native disjunction and the theorem's `RuleFactorization.any` need the same
original truth, not identical syntax: double negation transports that truth to
the arbitrary frozen interpretation. Completed producer metadata, stable operand
storage, full source evidence and actual work, operand and storage admission
remain executable obligations; the semantic theorem does not verify them.

The same module's `ConjunctOf` relation follows conjunctions only.
`nested_conjunct_required` and `necessary_conjunct` justify requiring a nested
condition whenever a complete singleton producer's head is true. The
`supplemental_original`, `supplemental_frozen` and `supplemental_answer_sets`
laws allow double-negated consequences to be appended while retaining the
original guards. They require entailment of the added requirements; unlike
direct transposition, they do not require two equivalent incidence tables.
The Rust nested pass must establish complete producer eligibility, exact shared
condition identity and duplicate-path handling. A reused disjunction must
denote a covered subset of the requiring heads; the original guards still
establish the requirements of every omitted head. The laws
do not verify that traversal, canonical lookup or its resource behavior.

[`PairwiseExclusions`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/PairwiseExclusions.lean)
replaces a complete family of pairwise negative conjunctions with one negated
unit threshold at two. Its carrier has distinct indices; every edge has two
distinct carrier endpoints, and every unordered carrier pair occurs. Edges may
repeat or reverse. Conditions may be arbitrary formulas. `original` proves equal
original truth, `frozen` equal truth at every frozen candidate and tested
interpretation without a subset premise, and `stable_in_context` equal answer-set
membership in any unchanged surrounding theory. The frozen step uses the general
fact that a negative constraint freezes entirely according to its original truth.

The Rust [pair collector](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground/pairwise_exclusions.rs)
uses exact interned atom-node identities after separately certified explicit
facts are removed. A completed original constraint join delimits the population;
source provenance alone cannot establish coverage. Sorting, duplicate-edge
removal, component membership and the distinct-edge count must establish the
Lean coverage premises. Eager compilation may replace separate complete
components; hybrid capture retires a whole streamed template only when every
passing row is a pair and every component is certified or has exactly two
atoms. A two-atom component's pair constraint is emitted once, since repeated
and reversed occurrences are the same constraint, so its part of the retirement
rests on that constraint itself rather than on the law. Full source validation,
original atom and producer ownership, root provenance, typed failures, charged
storage/work and the retirement of exact original rule positions remain
executable obligations. The Lean law does not verify these Rust operations or
native threshold storage and propagation. The [compiler contract](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/docs/pairwise-exclusions.md)
states their scope and resource boundary.

[`GroupedExclusionCoverage`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/GroupedExclusionCoverage.lean)
and [`GroupedWitnessExclusion`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/GroupedWitnessExclusion.lean)
use distinct binding lanes inside each exact shared key. `witness_pair_any`
reduces a complete product of condition witnesses to Any within each lane.
`witnessed_fibers_original`, `witnessed_fibers_frozen` and
`witnessed_fibers_stable` then replace the covered unequal pair family by a
negated unit threshold under its original common guard. Equal formula values
do not identify distinct lanes. Shared guards may be derived, and the frozen
law does not assume a subset relation. The counterexample laws expose why
merging lanes, dropping a guard, merging keys or omitting a pair is unsound.

The Rust [grouped emitter](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground/grouped_exclusions.rs)
admits only the eager original-theory constraint grammar described in its
[contract](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/docs/grouped-exclusions.md).
`plan::Plan` partitions positive flat atom occurrences around exactly one bare
variable disequality, checks occurrence-multiset symmetry, and retains all
nonlane variable values in the key. No atom may correlate both lanes. Completed
Support and the same immutable component plan delimit both fresh traversals.
The first establishes distinct typed lane identities; the second constructs
fresh original conditions and their Any folds. Native rows retain unit
occurrences, including equal condition IDs, and retain the common guard outside
the count. These concrete steps must establish source Cartesian coverage,
typed term identity and native threshold meaning; the Lean statements do not
prove those Rust operations.

First-origin atom metadata is preserved by naming only groups with a partner;
root origins still name the original rule. These locations follow canonical
prepared-program emission order, not lexical statement order. Canonical
end-to-end tests compare exact producer locations; a separate direct-Builder
test exercises the constraint-before-producer boundary. Exact internal order, work receipts,
substitution counts and tight-limit refusal points need not match the pair
emitter. No error or partial group traversal establishes a successful theory.
Canonical semantic, native-occurrence, per-atom provenance and resource tests
exercise those obligations without claiming whole-implementation verification.

[`DomainProducers`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DomainProducers.lean)
models possible argument values before binding enumeration. One producer
intersects its mandatory inputs and local bound; alternative producers contribute
their union. `transfer_monotone` preserves pointwise inclusion, while
`derivation_covered` puts every finite abstract derivation inside any closed
upper-bound assignment, including through recursive dependencies. Unknown is
the universal predicate. Rust must establish that source extraction covers
actual values, finite sets implement these operations, and a reported fixed point
is closed after widening. The laws do not prove source admission, termination of
the Rust attempt, arithmetic diagnostics or its resource behavior.

[`DomainBindings`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DomainBindings.lean)
addresses a different filtering boundary: a local match may have no complete
continuation. Conservative argument coverage makes the intersection for each
source variable necessary, and `kept_binding_survives` keeps it necessary once
every value a comparison over that variable alone excludes is removed, for the
bindings the exclusion rule keeps. Selecting every row that can finish then
preserves the exact ordered completion list, including multiplicity. The
optional eager consumer applies these guards to the positive-flat rules of the
exact normalized program, in every support-completion round and in final
instantiation; the ordinary command requests it. Objective declarations may
coexist with those rules because they introduce no argument values. Their local
joins, checked arithmetic, tuple identity and costs retain ordinary evaluation.
This applicability extension does not broaden the producer-scheduling certificate.
Unknown,
stopped and inapplicable analysis supply no narrowing. Concrete analyzer
soundness, source/IR correspondence, dictionary identity, the agreement of the
guard's comparison verdict with the join's, and recursive matching remain
unproved implementation bridges. Prepared candidates now retain scoped source
term IDs. Each support snapshot decodes them into its own relation dictionaries;
the preservation premise concerns decoded values, not equality of numeric IDs
across those authorities. The
[domain-binding guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/domain-bindings.md)
also states the separate work, storage and authored-error obligations.

[`DomainContraction`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DomainContraction.lean)
proves concrete singleton-distinctness and affine filters preserve compatible
assignments under fixed constraints. Finite compositions preserve that family;
an empty mandatory domain excludes the certified branch. Relating these
assignments to answer sets requires a fixed assignment interpretation and sound
recognition of the active constraints. The laws do not
implement constraint recognition or establish source admission, support,
minimality or existence. In particular, applying a filter before source
evaluation requires a separate argument that required arithmetic errors remain
observable. The [domain guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/domain-contraction.md)
distinguishes these prospective filters from the existing source-domain analysis.

The normal-rule and Ferraris foundations retain independent definitions.
The [NormalFerraris bridge](normal-rules.md) proves their equivalence under the
specified normalized-rule translation, including its filtered/direct-map
distinction. The `from_ground_program` Rust translator is now removed; the
bridge did not verify that implementation and does not verify current atom
interning or formula DAG construction. `FrozenReduct` represents a fixed
candidate's reduct.
The separate generated-code proofs establish its stored-mask invariant through
actual construction and compose it with the public query under per-invocation
reservation providers and fixed observations. Source lowering remains separate.
`PublicMembership` supplies the generated public membership composition, and
`RuntimeMembership` connects it to finite eventful reference checks. Interpretation
construction has its finite-input and reservation-contract proof above.

`FormulaNodes` retains a completed topology-validation prefix across aggregate
compilations. Its obligation is the inductive one in `DagSharing.WellFormed.snoc`:
a previously admitted prefix remains unchanged, and each additional node names
only earlier children. `DagSharing.stored_meaning` then explains why extending
the table cannot change a prior formula's meaning.
`DagSharing.intern_stable_under_extension` additionally keeps an existing
node's exact index under append-only extension. A runtime cached node therefore
needs the same owner and an unchanged prefix; a cache cannot survive a rollback
past its entry merely because its atom still exists. The Rust owner must keep
that paired prefix immutable and check every replacement suffix. Rollback and
detachment restore the checkpoint's validation frontier and numeric validation
evidence, as well as every owned arena length: an unchecked raw span can
temporarily borrow newly appended arena cells, so validation discovered inside
that append cannot survive their removal.
A checked logical append extends a fully validated prefix because it checks every
child and constructs canonical storage; it cannot skip an unchecked raw prefix.
This establishes topology only, leaving atom-universe admission separate. Its scalar, family and extremum methods reuse the
existing compilers, without a formula rewrite. These laws describe the invariant;
they do not verify the concrete validation cursor, cancellation, allocation or
rollback accounting. Raw paired constructors retain their full-prefix checks.

`EvaluationWorkspace` computes original node truth before exposing a borrowed
`FormulaEvaluation`. Rust's ownership boundary ties that view to the exact
interpretation and prevents reuse of its backing workspace while the view lives.
The denotational satisfaction relation supplies the mathematical specification.
The retained Boolean-generation laws establish its generated inner admission,
evaluation and root scans under their stated library-value and observation
contracts. They do not establish today's numeric-row evaluator, even on Boolean
input. `EvaluationWorkspace`'s row reuse, reservation, borrowed-view and
retained-capacity wrapper, and its composition with changing runtime observations, remain refinement
obligations. Independent syntax-tree controls also check satisfaction and
asserted-root truth across a finite test family; those tests do not extend the
generated-code proof boundary.

[`ParametricReduct`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ParametricReduct.lean)
proves that a fixed connective network agrees with frozen-reduct truth below a
candidate when its original-truth parameters are accurate. Atoms read the tested
subset; implication retains its original candidate-truth guard. The countermodel
law covers every asserted root and a proper subset, while original candidate
satisfaction remains separate. Its Rust correspondent, the clause search's
`PreparedReduct`, was removed with that search; the law remains as a statement,
and the region query's own correspondence is `ReductRegions`.
`Propagation.unconditional_sweeps_models_iff` proves the abstract composition:
intersecting arbitrary candidate domains with unconditional narrowing preserves
exactly their completions of the same query. The unit propagator it described
was removed with the clause search.

[`PositiveTheory`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/PositiveTheory.lean)
characterizes complete atomic-head theories with monotone bodies, including
positive cycles and exactly the truth constructor `False → False`. The least
producer-closed interpretation is the sole possible answer set; it is an answer
set exactly when all positive constraints hold there. Under this strict grammar,
a failed constraint rules out every classical model. Rust `PositivePlan` uses
that positive producer argument and the arbitrary-constraint extension below.
Its CSR correspondence must establish sound
activation, complete child incidences including aliases, finite queue exhaustion
and complete constraint checking against the same owner. The law does not prove
Rust indexing, allocation or work admission, nor source completeness or ordinary
candidate enumeration. A dependency projection or class hint cannot replace
the original-root certificate. See the [reading guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/positive-theory.md).

[`ConstrainedPositive`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ConstrainedPositive.lean)
proves that any original constraint `F → False` satisfied by a candidate has a
reduct true in every interpretation. It needs no subset or positive-body
premise. Adding arbitrary constraints therefore filters existing answer sets
without supplying support. For positive producers, the least consequences are
the unique answer set exactly when the original constraints hold there. Failure
rules out answer sets, but need not rule out larger classical models. Rust must
classify every original root, compute closure only from positive producers,
then evaluate the same original theory through `EvaluationWorkspace`. A false
producer is an invariant refusal, not a constraint verdict. Exact evaluation
consumes remaining work and counts its actual capacity beside the retained least
interpretation after CSR release. The append/partition law does not prove those
Rust ownership, work, first-error or source-completeness obligations.

`ConstrainedPositive.stable_iff_frozen_least` also permits a positive presentation
of one candidate's reduct. The original candidate must first satisfy the theory.
For every subset of that candidate, satisfaction of the presentation must agree
with satisfaction of the entire frozen reduct. Under those premises, the
candidate is stable exactly when it equals the presentation's least consequences.
The original program need not be positive or acyclic. Recognizing all original
roots, freezing negation, preserving atom identity and computing the least set
from the empty interpretation remain executable correspondence obligations.

[`PossibleFoundedness`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/PossibleFoundedness.lean)
gives a separate candidate restriction. An upper relaxation must retain every
body whose frozen reduct can hold below a possible set `P`. If `P` is closed
under every permitted head of every possible producer, complete producer and
choice coverage implies that each compatible answer set is contained in `P`.
The proof intersects an answer set with `P`, establishes a model of the whole
reduct, and applies minimality. It requires a closed upper set, not necessarily
the least one; it does not choose one disjunctive head. `body_upper_sound` justifies the positive Boolean relaxation
with candidate-valid false masks. `body_upper_mono` establishes the relaxation's
order property: shrinking possible atoms and adding false marks cannot make an
impossible body possible. It does not assert monotonicity of an arbitrary closed
set selected by a computation. A founded Subject still needs an explicitly
related regional result and closure computation. Rust must additionally establish
complete extraction, native aggregate relaxation, flattened-incidence correspondence,
completed closure, valid cached acyclicity and exact interruption/accounting.
Those obligations are not discharged by the semantic theorem or the small
exhaustive tests.

Rust restricts repeated closure to heads in exact positive cyclic components.
Every uncut noncyclic atom is a conceptual seed; held cyclic atoms are not seeds.
Complete cyclic-head bodies retain all Boolean and opaque boundary inputs, so
the resulting set is closed under every producer. Only missing cyclic heads
are published as cuts. One pass can be weaker than whole-program closure;
ordinary Boolean/support propagation and cyclic closure alternate to saturation.
Equality at saturation additionally requires induction over the complete positive
component order: an acyclic input to a cyclic component must precede it, and
ordinary propagation must expose every false relaxed Boolean body. Exact component
recognition, boundary invalidation, saturated equality and the packed publication
remain Rust refinement obligations. Exhaustive partial-region and complete-family
comparisons supply executable evidence, not a formal proof of this implementation.

On exact-owner reuse, the body projection also prepares ordered parent incidences whose
destinations are included, nonabsorbed Boolean nodes. Omitted destinations are
either outside the projection or opaque inputs initialized true or inactive;
their parent events cannot change the closure. Every remaining occurrence keeps
its original order and multiplicity. This filtering depends only on the immutable
theory, never on a region's false mask. The same preparation flattens producer
heads into ordered cyclic-head rows, including a separate row for selected
facts. It retains every relevant occurrence, while keeping original producers
available for explanation coverage. Parallel to the selected body nodes, an
immutable role records each node's initial state or noncyclic boundary atom.
Current node and atom falsity determine each reinitialized state; no prior body
closure truth is reused. The first pass uses original rows and node kinds;
included cells remain included even after their runtime state changes.
The parent rows, head rows and roles publish together under the exact owner.
Preparation also partitions selected IDs and roles together, placing mutable
counters before seed roles. Full replay resets the counters first. It then reads
the current masks for each seed and immediately replays a true seed's outgoing
rows; ordinary atom and derived-node events drain only after all seeds have
current states. Projected parents cannot target boundary or opaque seeds, and
an empty conjunction has no incoming occurrence. These conditions justify
removing initial seeds from the queue while retaining every activation once.
No previous closure truth is reused, and cuts still require full completion.
The first pass retains the queued schedule. Alignment of the partition, reset
before replay, complete outgoing incidences and equality of the two completed
closures are executable refinement obligations; the semantic upper-set law
needs no change. Exhaustive mask comparisons against the queued schedule give
regression evidence, not a Lean proof of these Rust operations.
Construction and retained capacities are charged; a failed preparation clears
that authority and a retry repeats body preparation. Producer-free restrictions
and frozen-reduct passes do not consume or invalidate this preparation. Their
separate knowledge never establishes founded completion; returning to a
producer-backed original subject authenticates the retained owner before false
events. A genuinely different original still discards foreign authority.
Preservation of completed closure across the different event schedules remains
a Rust correspondence obligation; interruption tests also exercise partial
preparation and current-mask resets.

Inherited completion also requires a reuse argument: the prepared positive body
projection belongs to the exact original theory, and no relevant input has become
false. An atom with no occurrence in that projection can only remove itself from
the previous upper closure; remaining uncut atoms stay covered. Ordinary propagation
must still invalidate completion when an opaque body becomes false. Unknown,
foreign or interrupted preparation supplies no exclusion evidence. The Rust
membership states, event invalidation and sibling isolation remain refinement
obligations; the semantic closure theorem alone does not verify this reuse.

`PossibleFoundedness.stable_sub_locally_closed` separates selected heads from
boundary seeds. If every permitted atom outside the selection belongs to `P`, and
all enabled ordinary and choice producers entering the selection are closed in
`P`, the whole possible set is producer-closed. Complete coverage and a sound
upper relaxation then put every compatible answer set inside `P`. The selection
may be arbitrary: this law establishes safe exclusion, not equality with a least
closure, and does not assume or prove inherited completion.

`PossibleFoundedness.PublicationErasure.erasure_preserves_least_closed`
proves that erasing atoms outside a monotone operator's least closed set from
both inputs and outputs preserves that least set. The positive formula
relaxation instantiates this law through `completed_publication_preserves_least`.
`whole_publication_baseline` also establishes that every surviving permitted
cyclic head belongs to the completed closure after all missing cyclic heads
have been published. Erasing a partial publication prefix still preserves the
least set, but does not establish that all remaining cyclic heads belong to it.
The boundary precedes dependent ordinary propagation; it does not freeze opaque
body truth across that later work or establish answer-set membership.

Rust's [selective replay](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-ferraris/src/regions/founded/local.rs)
collects false events from an authenticated completed original baseline. That
baseline may be inherited at entry or re-established after a completed founded
pass publishes its whole missing-head set. Exact owner and projection
authentication are repeated immediately before replay. A stopped publication
prefix cannot rearm collection; a completed pass without new publication does
not supply a new event boundary. Its forward cone must include every affected
live cyclic head `H`, from all false inputs since that baseline, including opaque
bodies and already-false occurrences of newly cut atoms. Rearming may clear the
completed founded event history, but must retain pending ordinary events and
collect subsequent atom cuts and ordinary false events. It does not mark
ordinary closure or founded confirmation complete. Inputs a sibling recorded
before it was refuted or stopped describe that sibling's region alone; the next
inherited narrowing resets those marks, one charged operation per mark, before
it collects, so they never enter its cone and grant no authority. Forward
traversal may stop at a currently false destination only because a newly false input
retains its own notification and an older false input was inactive at the
completed baseline. The head-prefix partition
must preserve every discovered vertex, head order and membership marks; it does
not supply new closure authority. Reverse selection must cover every producer entering an uncut head in
`H` and every relevant relaxed Boolean operand in `B(H)`. A current false body
may stop reverse traversal only because initialization keeps it inactive; any
operand needed through another live path remains selected. A cut head cannot
activate and needs no incoming producer evaluation. Fresh initialization and filtered shared events
must saturate that complete body selection while seeding unaffected uncut atoms.
The result must include the boundary seeds as well as the local closure before
publication or explanation capture. Proving that no newly invalidated unblocked
dependency can change a boundary seed, and that the selected completed baseline
justifies those seeds in the current least closure of the full cyclic-head
relaxation, remains an executable correspondence obligation. Complete producer
coverage, n-ary Boolean projection and native opaque-body treatment must realize
the formula-level law; it does not verify the indexed Rust DAG. Further
correspondence obligations include occurrence
multiplicity, sibling isolation, full fallback, typed interruption and actual
storage accounting. Exhaustive cut comparisons and interrupted-prefix tests supply
regression evidence for these obligations; the partition law does not discharge
them or establish a performance result.

`PossibleFoundedness.conditional_unsupported_absent` states the reusable
conditional law. For every answer set satisfying condition `B`, the same upper
relaxation must be sound below the complement of `U`. Every ordinary producer
whose head meets `U` and every atomic choice into `U` must be disabled. Applying
`stable_sub_possible` to that complement then excludes every atom of `U`.
This covers partial conditions independently of the ancestor region that exposed
them; a classical model or a model of a frozen reduct need not satisfy the cut.

`PossibleFoundedness.conditional_unsupported_forces_complement` gives the
corresponding deduction from an incomplete match. If `C ∧ ell` excludes `U`, an
answer set satisfying `C` and containing an atom of `U` cannot satisfy `ell`.
The proof applies the conditional absence law and contradicts that present atom.
The conditions are arbitrary interpretation predicates; the law does not model
Rust's signed coordinates or its classification of partial knowledge.

The Rust [collector](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-ferraris/src/regions/founded/explanations.rs)
starts from one uncut atom missing after a completed original founded pass,
before publishing that pass's cuts. It prefers a held atom; if no missing atom
is held, it may capture an open one. The conditional absence law does not
require the starting atom to be held: complete producer coverage with sound
false-body witnesses establishes that conclusion. Discovery, collection and
redundancy checks keep the same optional allowance. The collector reconstructs
every incoming producer's false-body witness using positive Boolean structure,
absent external atoms and exact negated atoms whose atom is held. A conjunction
needs one false operand; a disjunction needs them all. Other implications supply
no witness. Adding an unsupported atom requires checking all its producers, and
a failed alternative rolls back its speculative additions. A held cohead cannot
discharge that coverage duty.

[AggregateBodyWitnesses](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AggregateBodyWitnesses.lean)
states the corresponding aggregate-body laws. Every omitted contribution has a
false reduct outside the removed set under the recorded condition. A nonnegative
threshold fails when the total of all remaining possible contributions is below
its bound. Interior ordinal cuts fail when every qualifying contribution is false;
endpoint cuts require their separate constant laws. The false-body property is
preserved when the removed set grows, but every added atom still needs complete
producer coverage.

Rust implements threshold collection with a bounded frame cursor over the
original weighted row. Successfully explained occurrences reduce the remaining
possible weight; failed contributions roll back only their speculative additions.
Unexplained occurrences keep their full weight. Zero weights need no condition
witness. Native min/max cuts reuse their prepared qualifying disjunctions;
endpoint constants are handled first, and an interior cut with no chain is
verified by a charged scan. The laws do not prove the cursor, finite-width sums,
rollback, rank classification or original-row identity.

The worker-local [ledger](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-ferraris/src/regions/ledger.rs)
holds one tier of an owner's rows. The owner (`LearningOwner`) names the class,
today the answer sets of one exact original theory, and the tier's limits. Only
the owner's carrier, a subject built by `OriginalSubject::carrier` over the
owner's own theory, matches and captures, so every captured row belongs to the
owner's class; capture also requires complete producer coverage. Restriction,
bound, filter and frozen visits are not carriers and suspend reuse; a carrier
of a different class clears the rows. A complete match gives each open output
its sign or refutes a contradicted output. With exactly one unknown signed
condition, all others satisfied and an output's opposite decided, Rust instead
forces the unknown condition's opposite through the existing atom events
(`consequence_row_forces_complement`). A condition known opposite disables the
row; multiple unknowns or no contradicted output do not authorize this
additional deduction. By default a tier retains at most 64 rows after at most
256 capture attempts, sharing 131,072 discovery, collection and
redundancy-comparison reads. One collection permits at most 64 unsupported
atoms, 16 literals, 4,096 reads and 65,536 workspace bytes. A consolidated row
keeps the same 64-distinct-output bound. A full tier evicts the row with the
fewest decayed hits, oldest first, except founded rows within their reserved
floor, which by default is the whole tier; deleting a row loses pruning only
(`row_dominance` needs no deleted row). Consolidation does not expand the
discovery allowance. Matching remains charged to ordinary narrowing work.
Optional limits decline capture; global quota, cancellation and allocation
failures preserve their typed outcomes.

Above 64 rows a matching step reads an occurrence index covering every row's
condition and output atoms. The Rust obligation is that the index skips only
rows whose deduction cannot have changed: within one narrowing knowledge only
grows and a deduction reads only the row's atoms, so the rows the index omits
for the atoms decided since the previous step have the deductions that step
applied. A narrowing's first step and the step after any change test every row,
and an interrupted rebuild is never read. Exhaustive comparison of indexed and
scanning tiers over partial regions checks this obligation; no Lean law states
it.

The store (`LearnedStore` in zetesis-stable, over zetesis-ferraris's
`LearningStore`) holds rows by class. Its Rust obligations: a tier delivers only
under its own owner's class, so every store row of a class was learned by that
class's carrier and is a row of the class (`founded_row` for every row today);
seeding and delivery are retention, publishing only complete rows under the
same allowances, typed stops and transactional failure as capture; they run
only at walk boundaries, never inside a narrowing, so one narrowing reads one
tier throughout; and no store row reaches a membership check or a coverage
claim. Sharing needs no new law: `RowOf` does not mention regions, so a row of a
class is a row for every walk of that class, and dropping one, by eviction or by
the quality filter, loses pruning only.

`PossibleFoundedness.conditional_unsupported_union` combines already established
absence consequences under the same interpretation condition by taking the union
of their unsupported atoms. Their producer witnesses may differ. The ledger can
therefore share outputs when its signed conditions are identical.
`PossibleFoundedness.conditional_unsupported_dominance` transfers an absence
consequence to a stronger condition and a smaller output set: a row requiring a
subset of another row's signed conditions and excluding a superset of its outputs
subsumes that row. This supports either omitting a new row or retiring an old one
after a stronger replacement is retained. Both laws concern answer sets of the
same original theory, not arbitrary classical models or frozen queries.

Concrete condition identity, complete ordered union and inclusion, preservation
of partial-match deductions, and publication of a completed replacement remain
Rust obligations. Union and retirement must preserve the existing capture bounds,
optional allowance, typed failure behavior and retained/transient storage
accounting. These laws do not prove the ledger's representation or propagation.

[LearnedRegions](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/LearnedRegions.lean)
states the general row. A row pairs condition literals `B` with output literals
`O`, each an atom required present or absent, and belongs to a consequence class
`C`, a predicate on interpretations, when every member of `C` satisfying `B`
satisfies `O` (`RowOf`). For a region whose bounds decide every condition,
`consequence_row_refutes` excludes every member of `C` when the region also decides
an output's negation, `consequence_row_forces` gives every member every output,
and `consequence_row_narrows` restates that as `Region.narrow_preserves_valid`'s
premise with `C` as the valid predicate. `consequence_row_forces_complement` is
the unit rule: with every condition but one decided and an output's negation
decided, the remaining condition fails in every member. `row_of_narrower_class`
carries a row to any narrower class, so a row of the original theory's answer
sets also holds for the answer sets satisfying a streamed core. `row_union` and
`row_dominance` generalize the union and dominance laws above to signed outputs.
`founded_row` is the possible-foundedness collector's discharge of `RowOf`: it
restates `conditional_unsupported_absent` for a condition given as signed
literals and outputs given as absences, with the theory's answer sets as `C`.
The laws are over interpretations and regions; the ledger's encoding, matching,
retention, eviction, ownership, delivery between workers and resource handling
remain Rust obligations, stated with the ledger below.

[`CountermodelReuse`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/CountermodelReuse.lean)
states when a proper subset that refutes one candidate also refutes a larger
one. `LearnedRegions.antimonotone_superset_refutes` requires that enlarging a
classical candidate preserve reduct models below the smaller one.
`witness_rules_persistent` proves that premise for positive facts and rules
with a positive head and a conjunction of positive formulas and default-negated
atoms in the body. This includes disjunctive heads and integrity constraints.
`falsum_free_countermodel_does_not_refute_superset` gives a theory with no
occurrence of falsum where the implication fails. Recognizing a proved
fragment in a Rust formula DAG and attaching its witness to a candidate row
remain implementation obligations.

[`CountermodelReplay`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/CountermodelReplay.lean)
gives a separate rule for any finite propositional theory in the Ferraris grammar.
`LearnedRegions.replayNodes` selects formula occurrences at a fixed witness. A
false conjunction or true disjunction can use one decisive branch. At a nested
implication, the selection also records whether its original root was true:
a false root freezes to falsum even if its antecedent would be false at the witness.
`replay_nodes_agree` transfers the frozen reading when the two candidates contain
the witness and agree on the selected nodes. `countermodel_row_refutes` applies
this law to all original roots. A new candidate containing the witness and one
extra atom cannot be stable under that agreement. It need not contain the old
candidate. A concrete row would have to bind the validated witness and selected
signed nodes to the same original theory and use node knowledge sound for every
answer set in the region. Shared node IDs, supported lowered operators, extraction
cost, row limits and publication remain implementation obligations.

[`NumericWitnessReplay`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/NumericWitnessReplay.lean)
uses one index into both columns of a numeric row. An occurrence retains its
condition node even when it has zero weight or shares that node with another
position. The original node mask and the guard on frozen predecessor values
determine the reduct reading. The guards cover an inclusive signed threshold
and both ordinal cuts with their empty endpoints. The corresponding Rust owners
are `NumericStorage::view` and `cut_view` in `zetesis-ferraris/src/theory/numeric.rs`,
the threshold and extremum reductions, and `oracle::evaluate` and `oracle::evaluate_node`.
The proof starts at the admitted span, using exact arithmetic. Extraction of
descriptor and owner identity, machine bounds, cache validity and source tuple
coalescing remain implementation obligations.

For a candidate region that already holds a proper witness and another atom,
`known_countermodel_refutes_region` discharges a selected original root from
modelhood. Other selected signs must be known in the same original theory and
region. `FormulaBounds.Known` suffices for that theorem. A deduction specific
to stable completions needs its own preservation argument, as does reading the
Rust node-knowledge mask. No learned fact enters a membership check.

[`MixedFormulaGraph`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/MixedFormulaGraph.lean)
composes the numeric row and finite operand laws across an immutable mixed graph.
The checked reader resolves a descriptor on its own columns, verifies the row
domain, and requires every operand occurrence to refer to an earlier node.
An indexed original and frozen pass computes the two complete Ferraris truth
columns from the empty prefix. The root theorem checks the original root IDs
in order, including repetitions. The Rust correspondence starts at admitted
`Theory::view` and numeric storage. Actual group views may borrow an inline
pair or an arena slice. The proof uses their ordered contents, mathematical
indices and exact arithmetic. Compiled extraction and refusal costs remain
separate obligations.

[`StableProducerKnowledge`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/StableProducerKnowledge.lean)
follows the candidate support scan in `zetesis-ferraris/src/regions/support.rs`.
An ordinary row is unavailable when its body is known false or a different
head is held. Atomic choices use the body test alone. With complete producer
coverage every stable completion containing the atom retains a supporter.
An empty scan therefore excludes the atom; a held atom with one surviving
source forces that source's body. Either sign can justify a later scan or
a witness condition in the same region. The class here is the stable
completions. The root and witness checks still use the original theory and
its own frozen reduct. Authenticating a particular shared-node justification,
inherited cursor skips, and the other sources of region knowledge remain
separate from the full producer-filter law.

[`SignedOriginal`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SignedOriginal.lean)
relates each signed node ID to its position on the same decoded mixed graph.
A numeric guard's false original mask already fixes its frozen value. When
that guard is true, agreement on its referenced condition IDs transfers the
guard between candidates.

[`CheckedSignedReplay`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/CheckedSignedReplay.lean)
checks a proposed witness row. It confirms the old frozen roots and each
submitted original sign, then restores the positive signs of asserted roots.
At the fixed witness, it propagates definite bits through the mixed graph.
A decisive operand suffices for an ordinary conjunction or disjunction.
A false original numeric mask settles the node; a true guard can be computed
when its child occurrences are known. The checker requires every asserted
root to be definitely true. Unknown bits and absent IDs cannot supply evidence.
The theorem `checked_row_excludes` proves the old proper countermodel and
excludes a stable completion of a region holding the witness and an additional
atom, provided the nonroot facts hold of stable completions in that region.
It does not assume the selector covered enough nodes; the executable check
establishes sufficiency. Rust would still need to authenticate the witness,
root and sign owners at row creation and use, connect actual region knowledge
to those signed facts, and enforce the row and work allowances. This check
is a reference over the graph. The solver does not install or prune with it.

[`RegionalOriginal`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/RegionalOriginal.lean)
establishes the signed facts needed by the witness check. Starting with the
atoms held or cut in a region, a forward scan reads the graph in node order.
For a numeric guard it evaluates the least and greatest possible selections
of its input occurrences. When both give the same result, every completion
has that reading. An unknown input can therefore leave the guard decided.

[`RegionalProducer`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/RegionalProducer.lean)
adds one round of producer reasoning between two such scans. If a held atom
has exactly one surviving producer, its body must hold in every stable
completion. The proof requires a complete inventory for the original theory
and a checked link from each recorded body ID to its formula. A descendant
can use these signs when it still holds the witness and at least one more atom.
A sibling must establish the signs in its own region. The Rust
`a_countermodel_operand_sign_is_local_to_its_branch` test exercises this
distinction on an implication whose sign changes between siblings.
Correspondence for the production producer inventory and for storing and
charging witness rows remains open.

[`AlgebraicRows`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AlgebraicRows.lean)
states algebraic consequences for any consequence class, as `RowOf` does. A term
pairs an atom with a value, and a list of terms is measured in an interpretation
by folding a commutative monoid (`AxisFolds.Reduction`) over the values of the
terms whose atoms hold; `measure_congr` makes the measure local to the terms'
atoms. In an abelian group, equations of a class add (`equation_sum`), terms with
inverse values cancel (`measure_cancel`), and literals giving every other atom
its value isolate one atom's contribution (`equation_isolates`). The isolated
value forces the atom true or false, a row of the class
(`equation_forces_true`, `equation_forces_false`), or excludes the conditions
given (`equation_excludes`, `equation_excludes_given`); `excludes_region` and
`consequence_row_forces` carry either into a region that decides the
conditions (`decides_truth`). In an ordered commutative monoid, two families of
rows covering the same terms exclude the class when their lower bounds' fold
does not lie below their upper bounds' (`double_count_excludes`). In a region,
held atoms may raise a lower bound and atoms not cut lower an upper bound
(`lower_in_of_held`, `upper_in_of_possible`, `double_count_excludes_region`), and
`cut_excludes_region` accepts a bipartite cut certificate whose missed terms
count as the region's held and open atoms. The instances are `parity`, the
Booleans under exclusive-or (`parity_forces`), and `counting`, the natural
numbers under addition, which `count_measure` connects to `PartitionCapacities`
(`double_count_groups`).

A row enters its class through its provenance, checked once. `entailed_by_patterns`
gives every model of a group of formulas a property that depends only on a set
of atoms the formulas mention, provided every interpretation inside the set
that violates the property violates a formula of the group; it rests on
`Ferraris.satisfies_congr`, and `entailed_by_part` carries the property to the
answer sets of any theory containing the group. `entailed_by_completion` does
the same for an atom's completion, from `DisjunctiveSupport.answer_set_completion`,
which requires a complete inventory in which every producer of the atom is a
normal rule with it as its sole head and no choice has it; `xor_definition` is
the two-rule exclusive-or instance. `lower_of_threshold` and
`upper_of_negated_threshold` read a count bound from a threshold whose
conditions are atoms, with the weights as the terms' values, and
`guard_releases` applies a guarded constraint where its guard holds.

The bounded source views use `conjunction_assoc_original`,
`conjunction_chain_original` and `conjunction_flatten_original` to read nested
conjunctions with their leaf occurrences intact. `canonical_truth_original`
authenticates the meaning of the exact `False -> False` formula, and
`truth_implication_original` gives `TRUE -> F` the original truth of `F`.
`double_negated_conjunction_original` composes the existing
`Ferraris.double_neg_satisfies` law with the conjunction reading; this is
original satisfaction, not permission to erase double negation in a frozen
reduct. `bounds_of_asserted_conjunction` composes the lower and upper threshold
laws for a conjunction containing both positive and negated thresholds. The
Rust recognizers and independent source checkers must authenticate these exact
contexts against the original root, visit and charge every source occurrence,
preserve the existing distinct-atom and guard requirements, and finish checking
all eligible children before publishing their family. These source views leave
the original theory and its reduct unchanged; traversal, complete provenance,
atomic publication and resource accounting remain implementation obligations.

The library also provides `entailed_fact_step` for an ordered sequence of proved
positive facts and `entailed_by_residual_patterns` for recognizing a source
under those facts. The latter explicitly requires correspondence to the original
roots; removing fact atoms does not make the original formula mention fewer
atoms. `arithmetic::root_facts` prepares ordered witnesses through the same
activation kernel as positive evaluation, then independently replays each
original rule against earlier checked facts. Only replay constructs the packed
fact mask used by clause recognition and its provenance checker. Negative
premises and non-atomic heads are not fact sources. Source A checks its raw
eight-atom limit and distinctness before conditioning; a known positive leaf
disappears from the residual condition, while a known atom's negative leaf makes
that condition false. Source B and cardinality guards keep their existing rules.
The laws do not verify the Rust occurrence transpose, queue schedule, witness
order, owner checks or metering. The retained facts never enter frozen-reduct
membership search.

Original-candidate preparation in `zetesis-ferraris` now recognizes bounded
parity and counting structure. `arithmetic::rows` checks local truth patterns
against their original constraint roots (`entailed_by_patterns`,
`entailed_by_part`); `arithmetic::completion` reads the complete normal,
sole-head producer span (`entailed_by_completion`). Independent source rereads
check each proposed equation before `arithmetic::parity` eliminates rows.
`equation_of_agreement`, `parity_measure_odd` and `parity_equation_of_xor`
connect the packed XOR representation to `equation_add`. Root units and
contradictions use the consequences stated by `parity_forces`,
`equation_excludes_given` and `excludes_region`.

Completion tables compose packed intersections of signed atom columns, unions
over all producers, and XOR with the head column. `AxisFolds.all_fold_true_iff`,
`any_fold_true_iff` and `AlgebraicRows.completion_fold_false_iff` state the
pointwise meaning of these reductions, including empty and repeated lists.
The concrete periodic columns, padding and metering remain Rust obligations.
Both occurrence traversals and every independent source check are retained;
within each traversal, the walker may borrow its caller's already-read entry
view from the same immutable owner.

The local affine-hull reduction stops inserting differences once its span has
full rank, but still counts all remaining allowed patterns to decide exactness.
`AxisFolds.pair_foldFrom_left_absorbing` states this separation: an absorbing
coordinate stays fixed while the other coordinate continues its reduction.
The echelon invariant establishing that the machine rank test recognizes full
span remains an implementation obligation. Full span alone does not establish
that the allowed set equals its affine hull.

`arithmetic::cardinality_source` checks threshold and clause provenance
(`lower_of_threshold`, `upper_of_negated_threshold`, `lower_of_clause`), retaining
each guard. Its uniform-weight normalization corresponds to
`ThresholdDivision.rounded_bound_le_iff`; handling signed machine bounds and
executing the division remain Rust obligations. `arithmetic::cardinality`
proposes families and independently checks that each side is a disjoint
partition of the same atoms before comparing its lower and upper totals
(`double_count_excludes`). A guarded bound does not become unconditional:
`guard_releases` requires its guard to hold.

The original-candidate routes consume a checked root refutation or a root
seeded by parity units; the frozen-reduct search receives neither these rows nor
that root state. The region-inspection library extends these checked systems
without recognizing new source rows during search.

`zetesis_ferraris::ParitySystem::inspect_reserved` reads touched basis rows
against one input region, records that input in `ParityKnowledge`, then applies
collected consequences in basis order. The snapshot precedes application, so a
new decision remains a trigger for the next inspection. `AlgebraicRows.parity_forces`,
`equation_excludes_given` and `excludes_region` supply the value-level argument;
`LearnedRegions.consequence_row_forces` carries a forced row into the region.
The shared `zetesis_algebra::region::SubjectStep::Changed` denotes progress,
not local or joint closure.

After exact touched-row selection, parity uses the existing metered
`words::first` to detect an empty mask. `PackedWordIterator.first_set_none_iff`
states that completed absence means every stored word is zero; interrupted
scans establish no such result. With no selected bit, the old row loop has no
row consumer and the already-cleared consequence vector stays empty. The
runtime can omit rebuilding its open mask and the vacuous row/application
work, while still publishing the inspected input before returning Unchanged.
This is not a permanent absorption flag: unrelated decisions are published,
and later incident decisions select rows normally. Validation, system identity
preconditions and the completed incidence-selection correspondence remain
necessary. Executing fewer operations changes bounded receipts and quota
observations; the packed value law does not establish those effects.

`zetesis_ferraris::CardinalitySystem::inspect_reserved` retains a complete input
snapshot and two sums per checked family pair in `CardinalityKnowledge`. It
reconstructs each affected occurrence's old and current bounds, replacing its
contribution before testing for exclusion. The general laws separate three
obligations:

| Lean laws | Mathematical premise and conclusion | Rust counterpart |
| --- | --- | --- |
| `Zetesis.Algebra.guardedLower_sound`, `guardedUpper_sound` | The original guarded bound holds for the class. A decided guard permits its bound to strengthen the held lower count or cap the possible upper count. | `arithmetic::cardinality_runtime::values::effective` |
| `Zetesis.Algebra.guardedLower_mono`, `guardedUpper_antitone` | Under `Inside current prior`, the effective natural lower bound grows and the effective upper bound shrinks. | Reconstructing bounds from the retained and current region masks |
| `Zetesis.AlgebraicRows.counting_replace_occurrence` | Subtracting the selected old summand and adding its replacement gives the exact natural fold; other occurrences remain unchanged. | `arithmetic::cardinality_runtime::values::replace` |
| `Zetesis.AlgebraicRows.partial_double_count_excludes_region` | Both complete families cover the same terms, old bounds hold in an enclosing region and new bounds hold in the current region. Any mixture whose lower fold exceeds its upper fold excludes the current region. | The first contradictory pair during occurrence refresh |

The partial-refresh exclusion law needs validity of both choices of bound,
not numerical monotonicity. Its complete-family premise is not permission to
omit unrefreshed summands. The runtime count uses finite machine values; the
natural-number laws do not prove its subtraction, addition or refusal paths.

Both kernels use the existing `Words`, `Meter` and `NarrowingScratch` primitives.
Their `ArithmeticAttempt` retains performed work on success or failure. Knowledge
reuse requires the same prepared system and an ancestor input; width and descent
checks do not authenticate system identity. The `copy_reserved` operations
preserve the source and make a failed destination unavailable. Ordinary `Clone`
does not provide metered copying.

Rows prepared from the original theory are consequences for its answer sets.
The frozen countermodel walk uses rows proved for its own candidate and reduct.
The default candidate schedules omit per-region parity and cardinality
inspections after root preparation. When composing these inspections, the
common Subject driver orders parity, cardinality and streamed constraints after
the eager subjects. A changed late subject restarts eager propagation. The source
adapter retains its own first-call marker so its first visit starts a source pass.

`CandidateKnowledge` keeps the arithmetic snapshots separate from restriction
and objective generations. The charged copy operation preserves parent state
and makes a refused child unavailable. Each returned attempt publishes its work
and applied decisions before its outcome is handled. Completed inspections and
semantic refutations have separate activity fields; an interrupted attempt
claims neither. The frozen countermodel walk records its own arithmetic in
`ReductStatistics::inner_preparation`.

The general laws do not establish the adapters' exact permit ownership, refund,
publication or concurrent lifetime behavior. Those remain executable refinement
and qualification obligations, including refusal during a bounded arithmetic
operation and differences in scratch or child-storage reuse across schedules.

Source ownership, complete producer inventories, packing and padded incidence
strides, touched-row coverage, snapshot publication, sibling isolation, retained
and transient storage, typed refusal and cancellation remain Rust refinement
obligations. The laws prove neither that refinement nor host/device equivalence.
Flow witnesses remain separate work.

Retained rows use either the scalar witness lists or sparse word groups, chosen
by actual buffer capacity including the common row header. A packed row keeps
premise words and output words in separate ordered spans of one allocation.
The scalar collector remains temporary; conversion peak accounting includes both
buffers. Packed matching must preserve the same complete, partial and opposite
condition classifications, and decoding must preserve canonical atom identity
and ascending output events. Publication filters each packed output word before
decoding, required absences by `held | !cut` and required presences by
`cut | !held`: settled outputs need no event, while contradicted outputs retain
precedence. The cursor is local to one
traversal, and publication may cut its returned atom but cannot drain other
propagation between visits. Word reads and emitted outputs remain charged;
interruption supplies no completed traversal. These representation obligations are checked
against exhaustive scalar readings and bounded interruption tests; the generic
conditional-absence laws do not prove that Rust refinement.

These theorems do not prove Rust extraction, signed atom identity, Boolean witness
traversal, complete producer coverage, exact-owner matching or resource accounting.
For the additional deduction, Rust must distinguish an unknown condition from
one already known opposite, establish every remaining premise and a held output,
and propagate the opposite signed value without changing original-only scope.
Those representation and runtime correspondences remain open. Exhaustive small
original theories and partial-region tests check semantic preservation, and
scope/interruption tests exercise the executable boundaries; they do not close
that formal gap or replace the reduct membership check.

The CPU positive candidate cursor uses `positive_stable_iff` as a coverage law:
only the completed least interpretation can belong to the answer-set family.
It proposes that interpretation once if the original constraints and candidate
conditions permit it, retaining ordinary membership checking. A failed original
constraint instead completes the empty family. Rust must preserve the exact
certificate owner, every additional restriction and streamed constraint, and
successful completion before reporting exhaustion. Cancellation or a
work/allocation refusal cannot be substituted for the empty-family case. The
mathematical law does not verify that cursor, its packed copy, batch publication
or worker accounting.

[`TerminalDefinitions`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/TerminalDefinitions.lean)
separates arbitrary base atoms from new derived atoms using a disjoint sum. Each
definition has one derived head and a positive conjunction of base atoms; the
base theory never reads derived atoms. `stable_iff` characterizes every full
answer set as a base answer set plus exactly its definition consequences.
`stable_extend_iff` and `unique_stable_extension` give membership preservation
and a unique full extension of each base answer set. Several producers of one
head contribute by existential union; empty bodies and duplicate occurrences
are allowed. The frozen-reduct characterization requires a subset of a candidate
satisfying the definitions and imposes consequence containment. Minimality,
rather than classical satisfaction alone, excludes extra derived atoms.

Applying this law to source programs still requires a complete semantic
partition: every producer occurrence, all body scopes and implicit coherence
constraints must respect the base/derived separation. Ground-instance coverage,
preserved source diagnostics, canonical owner correspondence, reconstruction
before publishing a full answer, and failure and coverage accounting remain
unproved implementation obligations. This one-layer theorem does not establish
a recursive definition schedule or certify a source or runtime optimization.

The executable consumer is
[`formula_terminal`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_terminal.rs).
Its partition matches the exact normalized source to every selected IR producer
and scans remaining semantic reads. Reconstruction joins true base-model rows
and publishes their union with derived heads through one canonical descendant
store. [`TerminalSession`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/terminal_session.rs)
retains the original input, counts only completed extensions as original
answers, and records an unfinished extension separately. Over a hybrid base its
base session yields only core answers the streamed constraints accepted, so
each answer is checked, then extended, then published: the composition rests on
`StreamedConstraints.stable_iff_completed_partition` for the base and
`TerminalDefinitions.stable_iff` for the extension, and no streamed constraint
reads a deferred head, so deferral changes no constraint's truth. These Rust checks
implement the stated obligations; the existing theorem does not prove their
source-to-proposition or machine-execution correspondence.

[`StreamedConstraints`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/StreamedConstraints.lean)
adds an executable finite scan beneath that filtering law. A violation names an
original occurrence, completion certifies that every checked body is false, and
interruption retains the suffix after a checked false prefix. A partition's
flattened occurrences must be a permutation of the complete source family.
`completed_partition` and `partition_invariance` preserve the completed
satisfaction verdict across chunk sizes and orders; they do not preserve the
first violation or authorize acceptance before every required part completes.
`stable_iff_completed_partition` combines pointwise Boolean-to-formula original
truth with the append law for an arbitrary retained theory. Concrete source
lowering, local and aggregate-family completeness, arithmetic admission, prepared
owner identity, cursor coverage and bounded execution remain separate obligations.
The occurrence-count fuel bound does not bound Rust join or expression work;
less retained materialization need not mean less replay work. See the
[constraint-stream guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/streamed-constraints.md).

Hybrid retention grounds some eligible templates in the retained theory instead
of streaming them. The law needs no change: `stable_iff_completed_partition`
already takes an arbitrary retained theory, here the producer core with the
grounded templates' instances. `ConstrainedPositive.stable_append_constraints`,
applied once to the grounded instances and once to the streamed ones, shows
that how the eligible constraints are split between the two leaves exactly the
answer sets of the theory that grounds every instance. The Rust obligations are
that a grounded template's instances are emitted as eager grounding emits a
constraint, over the same completed join that capture counted; that the
retired templates are exactly the grounded ones; and that retention creates no
atom, so completion's guards still cover every atom.

The hybrid executor composes this law with either CPU or GPU core membership.
Host source consequences narrow only original candidate regions. The device
checks the retained core; complete streamed acceptance still precedes scoring,
bound feedback and publication. Device batches may contain core answers awaiting
that acceptance, so core membership and original-answer counts remain distinct.
The law is independent of the execution backend. Core identity, complete source
coverage, queue ownership and interruption accounting are still concrete Rust
and shader correspondence obligations; selecting a GPU does not discharge them.

[`StreamedOptimization.completed_ties_exact`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/StreamedOptimization.lean)
filters a sound, complete finite list of retained-theory answer sets by those
completed partition scans before selecting the minimum integer cost and its
ties. Pointwise original evaluation and partition coverage make the filtered
list sound and complete for the full theory, so the existing optimization law
identifies its selected membership with full-theory optimality. This is a
composition over one unbounded integer cost. Rust acceptance and incumbent
ordering, priority-vector scoring, checked arithmetic, candidate-bound feedback
and interruption handling remain separate implementation obligations.

[`StreamedRegions`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/StreamedRegions.lean)
adds a sufficient original-candidate region test using the existing `Region` and
`FormulaBounds.Sure` readings. Positive and double-negated atoms held in the
lower bound, and default-negated atoms absent from the upper bound, make a
normalized body true in every interpretation between those bounds. One
authenticated original constraint occurrence with that body excludes every
answer set of the retained theory plus the constraints from the region.
`necessary_selection_preserves_witness` preserves existence of a sure witness
when every sure occurrence passes the supplied selection. The sign law supplies
the necessary held/cut conditions for positive-row and signed-predicate filters;
the predicate domain must include each admitted atom, including unsupported
negative occurrences. The lemma does not prove Rust's row-to-dense-ID mapping,
join traversal, or equivalence of resource-limited prefixes.
The Rust join authenticates each joined positive occurrence's exact source map
once per cursor and reuses it across backtracking, while reading current region
truth for every row. This preserves the abstract row authority and selection
premises. The prepared source slot and settlement of shared receipts remain
Rust implementation obligations. The abstract laws do not certify runtime
ownership, synchronization or failure handling.
Hybrid join-plan reuse adds a representation obligation: the cached order and
comparison schedule must equal fresh planning for the same rule, empty outer
binding and completed support owner. Candidate truth, bindings and arithmetic
failure state must not enter the cached plan. Rust owner checks and repeated-scan
tests exercise this boundary; the semantic selection lemma does not prove the
Rust cache or its storage accounting.
`scan_refutes` supplies the soundness premise of `CoverageTree.refuted`; it does
not prove that Rust constructs a coverage tree. A completed sufficient scan
without a witness is only `NotRefuted`, not original satisfaction; an interrupted
scan is not refutation. The final exact per-candidate checks retain their full
family-coverage obligation. Concrete lowering, discharged guards, complete body
readings, catalog-coordinate identity and sound candidate bounds remain premises
to establish. This law does not authorize applying the same original-constraint
hook inside a proper-subset search of the frozen reduct.

[`StreamedConsequences`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/StreamedConsequences.lean)
extends this semantic boundary to an original constraint with one open literal
occurrence and every other occurrence sure, after successful scalar filtering.
`unit_literal_false` forces that occurrence false in every original model of
the region: a default-negated pivot holds its atom; a positive or double-negated
pivot cuts it. `unit_preserves_models` and `unit_preserves_answers` retain the
same original theory and its full satisfaction and reduct obligations. A second
open occurrence of an aliased atom blocks this conservative test, independent of
sign. This does not prove completeness of Rust's selected positive-pivot queries.

[`StreamedSubjects`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/StreamedSubjects.lean)
connects these updates to the named algebra. `occurrence_row` authenticates each
prefix/pivot/suffix row without merging repeated occurrences; `body_excludes`
supplies the full-body exclusion. `fresh_step_reaches` identifies a validated
unit update as one elementary step. It does not claim that the first consequence
selected by an ordered scan is itself a monotone closure.

`complete_no_consequence_closed` requires a fixed finite family of authenticated
instances, successful scalar results for every instance, complete scan coverage
and no consequence throughout that scan. A failed or unvisited scalar, an empty
pending queue or a partial scan cannot establish closure. `grouped_joint_eq`
shows that packaging the completed family as a Subject preserves the joint
region; it does not equate intermediate states, costs or failure prefixes.
These are original-model laws, inherited by sought answer sets through class
inclusion, never transported to a frozen reduct by relabeling. Rust still must
establish instance coverage, cursor lineage, witness identity, publication
freshness and the separate source/search receipt boundaries.

`batch_preserves_answers` permits a finite list of consequences proved against
one immutable region to be applied at any narrower region. The prefix law does
not claim completion of the remaining decisions. `completed_template_unchanged`
permits reuse of a completed negative scan when both truth bounds agree on every
atom read by every instance. Its premises include complete instance coverage and
the same successful scalar results. Rust must separately establish that coverage,
authenticate region lineage, justify reuse after read changes and preserve fault
handling; equal decided masks alone cannot detect a held/cut reversal.
Successful `NoConsequence` may preserve this completed evidence across a new
`First` call while closing the previous allowance. Rust must authenticate the
same core, compare both masks, reassess affected reads, discard evidence for
incompatible regions, and start the new allowance independently of reuse. Errors,
refutations, ordinary checks and abandoned productive scans invalidate the
saved evidence. The existing law does not prove that lifecycle, the completion
marker, cancellation ordering or receipt settlement, and reusable negative scans
do not establish original satisfaction or reduct minimality.

A changed atom may be excluded from a template's reads by necessary
constant or constructor mismatches at every occurrence of its signed predicate.
Rust must establish that every full match satisfies these necessary tests, with
all default-negation polarities retained. Variables, repeated-variable equality
and lowered temporary slots are conservatively unconstrained. The implementation
checks a bounded set of changed atoms and restores a full scan on overflow.
This refines the dependency used
by `completed_template_unchanged`; it adds no semantic law. Canonical atom and
component ownership, occurrence alignment, charged reads and failure invalidation
remain implementation obligations. The Lean law does not verify the matcher.

`consequence_reads_change` proves that a newly available consequence must read a
changed atom after a completed scan found none. `positive_changes_cover_consequences`
specializes coverage to positive occurrences when every changed read has that
sign. `positive_cut_blocks_consequence` excludes a cut positive occurrence from
both sure bodies and fresh-pivot unit witnesses in a consistent region.
`completed_template_positive_cuts` preserves a completed unproductive scan under
positive cuts. `positive_holds_cover_consequences` permits mixed positive
changes but covers new consequences through held occurrences alone. These laws
require fixed complete instances and scalar results, consistent current bounds
and unchanged bounds outside the changed set; affected signs are checked over
every grounded occurrence. These laws need no nesting premise; Rust additionally
authenticates monotone lineage before retaining scan state. They concern newly
available witnesses, not the validity of previously emitted deductions.

`never_literal_blocks_consequence` extends the blocking argument to all three
signs. `completed_template_disabling_changes` preserves a completed negative
scan when every changed occurrence is now never true. Holding an atom disables
its default-negated occurrence; cutting it disables a positive or double-negated
occurrence. This original-truth argument does not identify their reducts.
`positive_holds_cover_signed_changes` covers every new witness through a
changed positive held occurrence when other changed occurrences are disabled.
It combines change coverage with the signed blocking law, rather than assuming
that every changed read is positive.
`completed_template_discharged` additionally permits a completed productive
family to become unproductive after every emitted occurrence is falsified.
Previously unproductive instances remain so, and emitted occurrences block
their represented instances. The earlier batch laws separately justify the
deductions' soundness.

For a productive scan restricted to positive anchors, Rust owes both stages of
coverage. The earlier complete unproductive scan and the signed coverage law
cover every current witness by a newly held positive occurrence. Exhaustive
anchor traversal must then either refute the region or record a pivot for every
witness, preserving scalar evaluation and occurrence identity. This establishes
the complete-family premise at the scan snapshot, even though the cursor visited
only anchors. Subsequent disabling changes and verified application of every
recorded decision supply the remaining premises of
`completed_template_discharged`. Coalescing identical atom decisions must preserve
coverage of every represented pivot occurrence. Cursor exhaustion alone is
insufficient.

Rust retains one optional rule identity for its existing pending batch. Before
publishing completed negative evidence, it checks all emitted decisions against
the region, including the delivered prefix, and authenticates every intervening
change. Any enabling or untracked read discards this receipt. Ignored
decisions, partial scans, stops and errors cannot establish discharge. The Lean
laws do not prove those concrete queue and failure boundaries.

The changed set may be arbitrary. Rust must project all mask changes onto each
rule's possible reads conservatively; excluded atoms must occur in no covered
instance. A positive cut cannot conceal a later negative or double-negative
occurrence of that atom. Rust can restrict
a later scan to newly held atoms only if it retains all affected occurrences,
accumulates changes since the completed scan, and preserves scalar evaluation
and the unit test. Overlapping occurrences must remain covered. The current
implementation retains up to four distinct atoms per predicate and per pending
rule. Overflow restores a full scan; this is an implementation bound on reuse,
not a program resource limit. Enabling default-negated readers, relevant
generated bindings or uncertain completion also retain a full scan. These laws do not verify the
cursor's row coordinates, bounded change storage or query schedule.

Sharing the immutable rule-to-predicate and atom-to-predicate mappings adds no
semantic premise. Rust must establish that the published mapping belongs to
the exact core, covers every dependency and retains no candidate decisions.
Concurrent publication, per-checker byte admission and independent mutable
progress remain implementation obligations; the semantic reuse laws do not
verify their ownership or accounting.

`steps_preserve` composes such consequences with any separately justified
monotone formula closure. `steps_bounded` and `no_unbounded_steps` bound completed
fresh decisions by a finite carrier's initial open count. They neither count
source work nor renew an allowance: all source passes for one candidate closure
share one allowance. Correct runtime ownership and settlement of that allowance,
occurrence identities, canonical atom mapping, scalar diagnostics, polling and
one-decision commit remain implementation obligations. The explicit no-unit and
pending-after-decision counterexamples retain the distinction between no current
consequence, interruption and completed original satisfaction. See the
[consequence guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/streamed-consequences.md).

For membership checking, a positive plan restricts candidates to the least
interpretation, which the stream proposes alone. The positive checker separately
authenticates original
satisfaction and compares the candidate with the least consequences. A distinct
original model is refuted: the least consequences are a proper-subset model of
its frozen producer reduct, and `constraints_frozen` supplies satisfaction of
the constraint reducts even when the least set fails the original constraints.
The checker accepts exactly the least original model. This application still
depends on the complete root partition and retained-owner correspondence above.

The head-element laws assume a correctly identified activity family. Explicit
aggregate elements use complete tuple keys; ordinary atomic choices distinguish
default-negation sign and complete atom. Ordinary Boolean choices instead use
pool-expanded occurrence keys. Each alternative of one written element has its
own key, including repeated equal alternatives. Grounding witnesses of one
expanded occurrence share that key. Thus `{ #true : p(1;1) } = 2. p(1).` has
answer set `{p(1)}`; the two pool occurrences count separately.

The Rust compiler reads themelios's counted entries and public `Identity`
classification. For a by-occurrence entry, `HeadElementKey::Occurrence` is
assigned separately to each literal/condition pool product alternative, before
local grounding; `HeadKey::Occurrence` retains it. `LocalFamily` separately
retains the original entry for arithmetic definedness and warning handling.
Keys are local to one choice group, and provenance locates evidence without
determining counting identity.

[`FinitePools.expanded_occurrence_activity`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FinitePools.lean)
authenticates each key by its position in the expanded list and characterizes
its activity by the existence of a grounding witness.
[`SignedHeadElements.boolean_witness_key` and `boolean_occurrence_keys_distinct`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SignedHeadElements.lean)
then describe the required key equality and separation. The original and frozen
activity laws apply to these rows under their stated coverage premises.
`FinitePools.local_occurrences_complete` and `local_activity_complete` remain
valid membership laws, but do not establish the multiplicity of counted pool
alternatives or justify keeping one Boolean key for a whole written element.
The Rust source-product enumeration, key assignment, complete witness coverage
and enclosing group scopes remain correspondence obligations.

The shared Rust
[`HeadLiteral`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ir.rs)
retains the sign and operand. Its `positive_atom` operation controls producer
eligibility, while
[`Builder::head_literal`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground.rs)
constructs signed truth. This is the implementation distinction corresponding to
the signed activity and permission laws. The finite `M/J` tests inspect the whole
admitted theory, including necessary-support guards and candidate-frozen bounds;
they do not establish pointwise equivalence between unguarded source formulas and
every internal activity node.

[`Builder::initialize`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground.rs)
establishes the named `FALSUM` and `VERUM` nodes used by formula construction.
Their mathematical meanings agree with `GroundGuards.constant`; its
[`constant_original` and `constant_frozen`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/GroundGuards.lean)
laws state that these constants retain their truth in every interpretation and
frozen reduct. Rust tests check the emitted constants through original and
frozen satisfaction, and check the node/work limits and counter restoration on
initialization failure. The named indices and those operational properties
remain implementation obligations, not consequences established by the Lean
laws alone.

An ordinary singleton data-comparison head is an interpretation-independent
Boolean after its source operands have been evaluated.
[`GroundGuards.constant_head_constraint`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/GroundGuards.lean)
states that a rule with body `B` and constant head `c` is equivalent to the
constraint with body `B` and the complementary constant. The equivalence covers
original truth and every frozen `M/J` pair, for an arbitrary body. The existing
`evaluated_guard_equivalent` and formula-context laws transport fully evaluated
signed guards to these constants; a negated chain complements the whole
conjunction of adjacent comparisons.

This law applies after total evaluation. It does not justify treating an
undefined comparison as false, binding a source variable from its head,
suppressing an original body diagnostic, or changing finite-row coverage. Those
remain separate Rust obligations. Comparison heads contribute no positive atom
producer; the existing `BooleanHeads` producer laws account for Boolean head
operands without adding atom support.

[`OrderedHeadActivity`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/OrderedHeadActivity.lean)
relates selected signed tuple activity to an ordered value reduction. In Rust,
[`contribution`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_head_aggregate.rs)
borrows the complete first tuple value, and `HeadContributions` retains it for
the existing value-extremum lowering. The laws require complete-key coverage,
comparison properties and a logical empty value. They do not prove that the
Rust comparator, source join or charged value copy realizes those premises.

[`OrderedBounds`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/OrderedBounds.lean)
separates a numeric measure from a logical bound whose order against every
integer is the same. `constant_aggregate` requires a complete tuple-mask carrier
and constant comparison on every mask; it then preserves both original truth
and arbitrary frozen `M/J` queries. `measured_head_in_context` preserves the
separate positive permissions when the bound is replaced. In Rust,
[`numeric_comparison`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground.rs)
uses the existing logical comparator after source and tuple validation. The
concrete comparator, binding coverage and resource accounting remain executable
correspondence obligations. Excluding nonnumeric bounds from the numeric count-plan
certificate is a separate runtime admission rule, not a consequence granted by
the theorem.

[`HeadContributions`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/HeadContributions.lean)
keeps numeric normalization independent of head permission. Missing/nonnumeric
sum values and nonpositive positive-only weights are neutral contributions;
`neutral_permission_is_not_truth` proves that their permission still matters in
the frozen reduct. Removing an absent bound preserves the permission formula in
arbitrary context. These laws apply after complete active tuple keys have been
coalesced. The [head contribution contract](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/head-contributions.md)
separates formal source translation, executable evidence and reference differences.
Its declared missing-extremum extension projects present first values before
the existing ordered reduction. `complete_extremum_conservative` preserves the
prior complete-value domain; `selected_extremum_values` requires complete key
coverage. `extremum_head_in_context` lifts an implementation's exact original
measure truth through the candidate-only bound with unchanged permissions.
This does not prove source coverage or Rust refinement.

[`ObjectivePriorities`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ObjectivePriorities.lean)
keeps weight, priority, tuple and eligibility in one resolved row.
`completed_presence` assumes exact eligible binding coverage; `partition_vector`
and `partition_optima` preserve the cost vector and all optimal ties under a fixed
priority layout. The Rust
[`Preparation::specialize`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground/objectives.rs)
resolves each template from the same binding while retaining its original-model
condition. These laws do not prove that possible aggregate support establishes
priority presence, that the source join is complete, or that checked arithmetic
and bounded execution implement mathematical evaluation. Objective laws also do
not establish search completion or alter answer-set acceptance.

[`ObjectiveConditions`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ObjectiveConditions.lean)
evaluates conditions in the supplied original interpretation.
Its `formula_query_truth` law permits reading a formula through a closed Boolean
query, with an explicit map from source atom identities to queried atoms.
Implication becomes `not antecedent or consequent` only for this original-truth
operation. `formula_query_changes_reduct` gives a counterexample to using that
conversion as a program rewrite: at `{a}`, the empty interpretation satisfies
the reduct of `a → a`, but not that of `not a ∨ a`.
[`ObjectiveEligibility`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ObjectiveEligibility.lean)
separately proves a three-valued truth cover: absent rows cannot contribute,
required rows are true, and optional rows retain both possibilities. Optional
does not assert that a row can be realized in an answer set. Complete source
bindings and a sound activity classification are premises, not consequences of
the truth algebra. The source analysis still needs its own refinement.

[`SourceActivityReuse`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SourceActivityReuse.lean)
connects that activity cover to a native weighted row. `activity_masks_compatible`
supplies the existing threshold algebra's lower and upper truth readings;
`threshold_covers` proves that their two sums classify every signed cut soundly.
`bounds_equal_of_occurrence_agreement` permits reuse when every occurrence in the
same immutable row has unchanged activity. Repeated conditions and correlated
truths are allowed; optional activity still asserts no realizable answer.

The Rust producer-scoped
[`SourceBodies`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ground/scoped_body/source.rs)
shares lowering through the existing aggregate identity and complete family-input
key, while retaining each binding's remaining literals. Its activity reader
memoizes bounds by the owned numeric-row identity and continues an initialized
formula prefix. `EvaluationPrefix.evaluate_append` and `storage_preservation`
provide the corresponding abstract prefix laws. Rust must establish immutable
completed support, complete keys, stable atom/node/row identities, and unchanged
activity throughout each retained prefix. The owner ends before staged activity
is published or refined; an exact mutable snapshot discards its evaluation cache.
A failed node must not advance the initialized prefix. Source tuple coalescing,
signed-weight lowering, finite-width sums, allocation, work charges, cancellation
and typed refusal order remain executable obligations. These laws neither change
reduct membership nor establish GPU execution or performance.

[`SourceSupport`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SourceSupport.lean)
connects completed producer closure to answer-set coverage under its explicit
`ProjectionCompatible` premise. Restricting an original model to the closed
carrier must preserve that model's frozen reduct; minimality then excludes
atoms outside the carrier. `completed_activity_covers` derives optional/absent
truth coverage for objective queries. Proposal monotonicity is a separate
hypothesis of the finite-stage containment law, not a termination theorem.

[`NormalSupport`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/NormalSupport.lean)
discharges that projection premise for possible heads of mathematical normalized
rules, and for any producer containing their proposals. Original satisfaction
supplies each required head's membership in the model; producer closure supplies
its membership in the carrier. Constraints survive restriction under unchanged
gates. The normal/Ferraris bridge then supplies the same frozen-reduct verdict.
This advances the mathematical producer argument without certifying Rust's
source bindings, rich heads or aggregate assignment machinery.

`NormalSupport.propose_gate_independent` states that changing only normalized
candidate gates leaves possible heads unchanged at every carrier. The eager
support scheduler admits flat ordinary negative and double-negative non-inputs
under its existing first-new-positive-occurrence partition. Rust must preserve
the positive bindings, checked scalar operations, source occurrence IDs and
old-head publication history; the emitted original formulas retain the gates.
The law does not equate answer sets of programs with different gates.

[`ProducerScheduling`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ProducerScheduling.lean)
retains original producer identities beside their ground rules. Complete
positive-input registration ensures every newly enabled body wakes its producer.
If every old head proposal is already published, selected proposals plus the
current carrier equal the full inflationary step; an empty selected set then
establishes possible-head closure. Zero-input producers require complete
bootstrap. The Rust positive-flat source plan preserves original IR slots,
uses signed-predicate reverse postings and a packed wake set, and retains the
existing first-new binding partitions. Complete source instantiation, exact
postings, checked work/storage and all-success round publication are executable
correspondences. Final constraints and answer-set membership remain separate.
The general laws, `selective_step_exact_of_stable` and
`empty_selection_closed_of_stable`, hold for any producer family whose unaffected
producers are stable; `stable_of_reads_only` gives stability to a producer whose
proposals depend only on the carrier's atoms with registered predicates, when the
carrier grows. The rule-level laws above are their corollaries. Without a
whole-program plan, the Rust registration supplies the read set of each rich
producer: every atom occurrence in its body and head element conditions, at
every depth. The proposal function intended to instantiate `ReadsOnly` is
`Q(p, C)`, the possible heads of producer `p` over the fixed round snapshot `C`
before pruning heads already supported. Equal registered relations must give
equal `Q` values. The Rust traversal additionally reads published or pending
head membership. If `H` is that accumulated head set before the traversal,
containing `C`, and `A(p, C, H)` its retained proposals on successful completion,
the separate correspondence is `H ∪ A(p, C, H) = H ∪ Q(p, C)`. It requires that
the traversal introduce no head outside `Q` and omit only heads already in `H`
or retained during the traversal. Composing that equality over selected
traversals gives the inflationary step to which the scheduling law applies with
`Q`. Neither the membership-pruned proposals nor their union with the carrier
is asserted to satisfy `ReadsOnly`.

[`ComponentClosure`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ComponentClosure.lean)
states Bekić's decomposition of two monotone least closures and its
dependency-ordered case. `completed_producers_closed` connects the final
carrier to each producer's last completed snapshot, assuming complete read
registration, settled inputs and publication of the unpruned proposals. The
component scheduler's `resolve` and `classify` must establish the dependency
premise; `mark_ready` must establish that those inputs cannot change again.
`take_ready` marks a producer before traversal, so only successful traversal
and publication supply the completed-history premise. Monotonicity of rich
source proposals, these imperative invariants and schedule termination remain
executable obligations. The laws establish possible-head closure, not answer-set
membership or equality of intermediate work receipts.

Read-set completeness for `Q`, the pruning equality, complete bootstrap and
old-head history, and append-only publication remain executable obligations.
The registration's
[differential tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/producers/registration/tests.rs)
compare per-round atom publication, emitted theory, objectives and warnings
with a reference that traverses every rich producer in every round, and assert
that visits were skipped. This checks the selected fixtures, not a Rust
refinement proof. The [scheduling guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/producer-scheduling.md)
states the proposal and pruning obligations together.

The Rust [support builder](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support.rs)
creates `CompletedCatalog` only after an admitted round establishes no new atom.
For selective traversal, completed old-head history and complete wake coverage
justify that conclusion even when no producer needs another snapshot. Its immutable
`CompletedSupport` view supplies the same typed rows to source activity,
projection-domain preparation and final grounding. Intermediate snapshots and resource-stopped rounds cannot
supply this capability. The types enforce the completion handoff; they do not
prove that Rust's source generation establishes `ProjectionCompatible`.
The [support proof guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/source-support.md)
identifies the remaining binding, key/value coverage, arithmetic and resource
correspondences. Recursive producer syntax alone neither prevents finite
completion nor establishes it.

[`ObjectiveConditionTable`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ObjectiveConditionTable.lean)
connects the finite node-table representation to those unfolded queries. Each
operand names an earlier node. The invariant equates each stored Boolean with
the original truth of its unfolded query. One node preserves that invariant;
induction over the remaining nodes gives the complete table. Backward admission
separately proves that all lookups succeed. The result is the last node, or true
when the table is empty.

[`ObjectiveProjection`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ObjectiveProjection.lean)
justifies preparing an objective from the atoms it reads. `query_read_locality`
requires agreement on every atom occurrence, including those under negation and
both Boolean branches. `query_predicate_projection` preserves the query when
all those occurrences belong to requested predicates. `requested_rows_preserved`
retains each requested predicate's complete ordered row family; it does not make
an incomplete source catalog complete. `compact_prefix_read` connects a present
compact prefix entry to truth at its stored original atom coordinate. It does
not identify that coordinate with the compact formula-node position.

The Rust
[`ObjectivePlan::new_projected`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/objective_bound/projected.rs)
receives an authenticated `ObjectiveSource` from one admitted source owner. Its
selector must collect all positive-pattern and closed-condition signed predicates
and arities, scan that owner's complete catalog, and retain increasing original
positions. The shared selected `AtomIndex` keeps those original coordinates;
`AtomPrefix::node` must translate them through the immutable compact atom-node
prefix before either a positive join or a closed condition uses a formula node.
The existing condition-table and cost-transport laws then apply to unchanged
keys, priorities and scores once pointwise condition correspondence is established.

Authentication, typed predicate equality, selection completeness, executable
binary lookup, graph append correspondence and finite-width operations remain
Rust obligations. The projection's explicit allowance covers its bookkeeping
and simultaneously live index scratch; it is not a total objective-memory or
process-memory ceiling. Actual capacities, work receipts, cancellation and typed
fallback on refusal require executable checks. A later refusal of optional choice
strengthening must retain a successful exact objective plan. These laws neither
identify displayed answer projections with complete interpretations nor change
the original theory, reduct membership or complete enumeration requirements.

The Rust
[`ObjectivePlan::score`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/objective_bound/score.rs)
reuses the compiler's already coalesced eligibility DAG to read a score without
reconstructing contribution tuples. `ObjectiveDirections.normalization_preserves_vector`
connects normalized complete-key groups with OR eligibility to the reference
fixed-priority vector; `ObjectiveConditions.condition_vector` transports exact
original-model condition truth into those scores. The table laws supply the
mathematical backward-reference invariant. The retained Rust DAG also contains
implication, whose original truth has the `formula_query_truth` interpretation;
it is not the table datatype verbatim. Complete catalog joins, global key
identity, translation of DAG operations, exact original-theory ownership,
finite-width reduction and admitted work prefixes remain Rust obligations.
This score reuse is distinct from source priority specialization and from
constructing a nonnegative candidate bound. It neither substitutes a new
original theory nor establishes search completion.

The candidate bounds that
[`ObjectivePlan::bound` and `ObjectivePlan::strict_bound`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/objective_bound.rs)
compile share one lexicographic fold (`objective_bound/bound.rs`), whose initial
suffix selects the comparison: true keeps cost at most the incumbent's, false
keeps cost strictly below it. The optional choice strengthening
(`objective_bound/choices.rs`) compares certified lower costs through the same
fold and comparison.
[`RequiredChoiceCosts.lexicographic_lower_bound`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/RequiredChoiceCosts.lean)
and `lower_bound_preserves_answer_sets` cover the inclusive comparison;
`lexicographic_lower_bound_strict`, `lower_bound_preserves_candidates_strict`
and `lower_bound_preserves_answer_sets_strict` cover the strict one. Under
either, a componentwise lower cost vector meets the comparison whenever the
exact vector does, so conjoining the strengthening removes no candidate the exact
bound keeps and leaves the original theory's stability unchanged. The laws
compare vectors of one fixed priority order. The fold's node construction, the
zero for a priority absent from either side, the certified lower costs
themselves and finite-width arithmetic remain implementation obligations.

The second choice cost is compiled by
[`objective_bound/choices/layers.rs`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/objective_bound/choices/layers.rs).
The existing checked source implications allocate disjoint, coalesced objective
keys. `layers::order` orders each retained allocation by weight; `append_group`
emits its activated minimum and nonnegative differences between distinct weight
levels. A difference is enabled only when the shared OR of all cheaper key
eligibilities is false. Eligibility keeps its exact-plan coordinate, while both
lower costs use the same copied activation coordinates. `layers::terms` adds
exact unallocated contributions and borrows unchanged priorities.

[`ChoiceMinimumLayers.increments_le` and `group_lower_le`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ChoiceMinimumLayers.lean)
prove the pointwise value bound. Their premises require ordered levels,
nonnegative allocated weights, an eligible witness in each active group, and
sound exclusion of cheaper eligible keys. They do not assume that a smaller
value is useful to a particular narrowing implementation. Extending each group
inequality to a priority requires the disjoint allocation and exact remaining-key
partition; applying the existing `RequiredChoiceCosts` lexicographic laws then
preserves the strict or inclusive comparison. `ChoiceMinimumLayers.conjoin_necessary`
expresses the final composition: two necessary restrictions are conjoined,
never added as scores. The original theory and membership reduct stay unchanged.

Rust correspondence must establish the source witness, complete key partition,
ordered cumulative-OR construction, activation/eligibility owner mapping and
checked integer differences. The canonical layer tests exercise the shared
`Minimum` value, exact total-model comparisons and tied costs. Actual partial
narrowing, complete solver families and all optimal ties remain distinct
qualification obligations. The value laws do not verify allocation, logical
work tariffs, cancellation, refused prefixes or atomic publication. Those follow
the existing `Work` and preparation `Storage` owners: replacement retains the
previous complete choice owner on refusal, and incumbent strengthening retains
the completed exact bound with its optional failure and performed-work receipt.

For example, `atom a; neg 0; disj 0 1` shares the first node and computes the
original truth of `a or not a`. This is a query-evaluation law, not permission to
replace that formula inside the original program: a program transformation must
also preserve frozen-reduct truth. The [proof reading](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/objective-condition-table.md)
gives the named steps and explicit premises. Rust atom conversion, validation,
indexing, allocation and work/cancellation behavior remain unproved executable
correspondences.

`ObjectivePriorities.zero_slot_comparison` and `zero_slot_optima` show that an
always-zero slot at a fixed priority position preserves ordering and all optimum
ties. They do not establish an identical raw priority layout in clingo. Proving
an extra slot always zero requires the source-coverage and actual-condition
arguments above; a zero observed in one answer is insufficient.

[`Observations`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Observations.lean)
connects complete query bindings to the distinct enabled-value channel, under
explicit binding and value-expansion coverage. The same row supplies its
condition truth and constructed values. Its family laws attach a display to
each original interpretation and recover the exact original list, preserving
order and multiplicity even when displays coincide. The world-view corollary
requires an already established `WorldViews.Represents` premise; output does
not prove membership or enumeration coverage.

`Observations.necessary_filter_preserves_terms` permits discarding rows that
cannot enable a completed query. Predicate equality supplies such a necessary
condition for an atom pattern. Rust locates that predicate's contiguous range in
the canonically ordered model, then matches complete tuples. Correct binary
bounds, cursor restoration, source-alternative identity and error ordering remain
implementation obligations; the theorem does not verify that Rust code.

The [observation reading](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/observations.md)
relates these laws to scoped queries, complete tuple keys and the Rust compiler
and evaluator. Source safety, anonymous projection, checked arithmetic,
construction, ownership and resource/cancellation completion remain executable
refinement obligations. The laws apply after completed evaluation, not to an
error prefix or an unfinished display.

`Observations.shared_equality_choice_exact` states the finite two-edge equality
law with one middle value and an arbitrary surrounding guard. The Rust planner
retains each original comparison while sharing that value through a bound value slot.
Structural capture reads the complete retained value and its typed components;
aggregate capture reads the actual supplied-model extremum. Source scheduling,
matching, widened numeric comparisons, copied-value charges and cleanup remain
concrete correspondence obligations.

[`AggregateInvariants`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/AggregateInvariants.lean)
states when optional tuple keys cannot change a required sum or extremum.
The source [measure carrier](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_objective_dependencies/presence/flat/carrier.rs)
coalesces complete keys and classifies required and possible activity. It
reduces its tuples to contributions and takes its values from the one carrier
operation the assignment proposal sites share (`formula_assignment::carrier`),
so one operation carries one meaning.
An invariant result is the singleton case.
[`SourceMeasures`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SourceMeasures.lean)
proves finite carrier membership and coverage of actual active selections.
Optional keys need not be independent in an answer. The concrete source
classification, carrier reduction, checked arithmetic and unary transport remain
implementation obligations. The mathematical list enumerator is a reference
definition; it does not verify the Rust subset-sum or extremum operations.

[`SourceMeasures.count_carrier_membership`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SourceMeasures.lean)
specializes carrier membership to no required keys and the measure `List.length`:
the possible values are exactly the naturals up to the list length.
[`SourceMeasures.required_count_carrier_membership`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SourceMeasures.lean)
generalizes it to required keys: over distinct possible keys with the required
keys among them, the values are exactly the naturals from the number of
required keys to the number of possible keys. These laws count distinct keys;
the aggregate callers must first coalesce complete tuple keys and supply one
unit contribution per key. They justify membership in the count proposal range,
not Rust's ordered buffer, numeric conversion or resource behavior. Checked `i32`
increments, distinct full-key coverage, the classification of required keys and
unchanged later aggregate equality remain implementation obligations.

[`SourceMeasures.sum_carrier_nil` and `sum_carrier_cons`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SourceMeasures.lean)
specialize the same carrier to optional integer sums: the empty input has only
zero, and one more occurrence keeps the old values together with their translation
by its weight. Equal weights remain separate occurrences.
[`SourceMeasures.required_sum_carrier_membership`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SourceMeasures.lean)
states the seeded form over distinct keys with a key-to-weight map: a value is
the required keys' total plus a subset sum of the optional keys' weights. Over
weights used as keys, membership in the required keys could not require each
occurrence, so the law is stated over keys. This is a common denotation for the
seeded sorted merging and packed reachability, not a proof of Rust shifts, word
boundaries, sorted output, checked `i32` arithmetic or resource behavior. Callers
still coalesce complete ASP tuple keys before supplying contributions; correlated
conditions can leave carrier values unrealizable, and the original aggregate
equality retains those conditions.
[`SourceMeasures.required_extremum_carrier_membership`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/SourceMeasures.lean)
states the ordered-extremum form for required keys: the carrier consists of
the required extremum and optional values strictly preceding it. Reversing
the supplied order covers both `#min` and `#max`; explicit order laws and
an empty endpoint are premises. The law does not require distinct measured
values, since extremum reduction is idempotent. Concrete ASP term comparison,
required-key classification, Rust construction and bounded failure behavior
remain separate obligations. The aggregate comparison campaign checks both
the required result alone and strictly preceding optional values.

[`IntegerEnvelopes`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/IntegerEnvelopes.lean)
proves directed bound coverage and integer floor/ceiling laws.
`filtered_bindings_exact` states that a covering envelope followed by the original
guard recovers exactly the satisfying bindings. The Rust
[envelope analysis](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_binding_guard/envelope.rs)
uses these mathematical obligations. Normalizing expressions, scheduling
endpoint inference, checking finite-width arithmetic and enumerating the
resulting intervals remain concrete refinement obligations.

The [affine reader](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_binding_guard/envelope/affine.rs)
requires source-normalized expression plans.
[`prepare`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ir.rs)
applies checked bottom-up normalization before compiling those plans: closed
arithmetic subtrees have become constants. Supported pool selection and interval
lowering preserve this property; intervals become variable slots. The reader
does not substitute a binding's values. An expression such as `X-X` therefore
remains syntactically open even when its affine coefficients cancel, and cannot
supply a closed multiplication factor. This compiler invariant is a premise of
the implementation review, not a theorem of `IntegerEnvelopes`.

Affine coefficients describe mathematical integers. The retained
[guard evaluator](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/evaluation.rs)
still checks the original expression's scalar arithmetic. Coefficient and bound
capacity refusals remain distinct from source arithmetic errors; neither is an
arithmetic value or evidence of completed enumeration.

[`EvaluationPrefix.root_preservation`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/EvaluationPrefix.lean)
equates returning a final operation's result with appending it to the completed
prefix and observing the last value. Both strict schedules preserve the first error.
The Rust [expression evaluator](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/evaluation.rs)
returns its root without appending another result slot. Single constants and
variables return authenticated existing term keys without numeric reconstruction.
The law assumes pure partial operations and stopping at the first fault. It does
not cover the source mode's missing-operand mask or continued independent branches after a
zero divisor. Scope and prefix authentication for reused keys, source-plan
validity, operand indices, checked scalar arithmetic, resource charges,
allocation and cleanup remain implementation correspondences. A smaller scratch
prefix does not establish a timing or
process-memory result.

`EvaluationPrefix.reset_preservation` states that an empty live prefix hides all
earlier workspace values. A Rust join lends its existing workspace to partial
comparisons, binding generators and final filters. Each evaluation returns a
scoped term key and clears its prefix before the next borrow. Preserving expression
order, complete-filter error precedence, work admission and cleanup is a concrete
caller obligation. The reset law neither establishes source-family exclusions
nor permits an earlier final filter to hide an independently required check.
An integer comparison form borrows no workspace. Its obligations are that its
verdict equals the evaluator's on every row it decides, with every other row
left to the evaluator, and that an attempt is checked against the work ceiling
for its constant, one unit per step and per variable step of both operands,
before it evaluates, and counts after it the work it performed, never more than
that constant; no Lean law states them.

`EvaluationPrefix.remember_success_sound` states the invariant for retaining
successful whole-expression results. It requires a pure partial evaluator and
keys whose equality implies equal evaluation results. `reuse_success_exact`
preserves values and errors; `reuse_complete_filter` transports that equality
through the same filter over the same complete ordered rows. These laws do not
permit row pruning, cached failures, or changing error precedence. The Rust
implementation must separately establish expression identity, exact input-key
coverage, vocabulary and reader-prefix authentication, successful publication,
zero-divisor state, and charged lookup/storage work. The laws are mathematical
preservation results, not a Lean-to-Rust refinement.

`EvaluationPrefix.success_preimage_exact` supplies the inverse-domain step:
filtering a finite input carrier by a sound, completely populated success map
selects exactly carrier inputs evaluating to a fixed wanted value. Cache coverage
of that carrier is distinct from the source proof that it covers every complete
binding. The Rust projection's covered prefix records the former only after the
whole eligible body passes totality preparation; its completed positive column
supplies the latter. A missing known-side key declines the optional restriction.

The derived input domains use `FiniteTables.indexed_survival_exact` through the
shared table selector, meeting the ordinary constant and incoming-binding
domains and retaining original row identities. Necessary selection preserves
complete contributing bindings by `DomainBindings.guarded_continuations_exact`;
the ordered raw positive-match family may shrink. Every residual comparison and
ordinary tuple match remains in place. Canonical input/result alignment, exact
reader ownership, prefix coverage publication and charged temporary domain
storage remain Rust obligations, with no refinement claim.

The shared numeric analyzer lowers prepared producers and compact fact columns
into one conservative fixed-point computation. Its source wrapper uses that same
engine. Argument unions must include every producer; local choice and conditional
frames must remain separate; mixed logical values do not become integers through
comparison alone. Finite-set intersections use identities from one analysis,
never a presumed ASP ordering. Outward widening, a complete synchronous narrowing
pass and opaque-expression treatment must preserve every possible produced value.
These are currently Rust obligations, exercised by source and prepared-IR tests;
the existing `DomainBindings` laws do not prove this concrete analyzer sound.
Stopped analysis publishes no usable domains. The separate positive-flat guard
certificate remains required for finite-value guards.

The neutral analyzer can also certify requested integer expressions over a
rule's complete-binding domains. It checks every intermediate, including both
children of correlated expressions; produced-only bounds and later guards do
not establish totality. The prepared-IR observer reports these certificates and
separated comparisons for the exact borrowed rule occurrence. Its census is
diagnostic: it does not remove literals or bypass arithmetic-family admission.
[`RemainderIntervals`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/RemainderIntervals.lean)
proves the signed variable-divisor envelope and the corresponding scalar fault
conditions. Analyzer soundness, occurrence mapping and the certificate walk
remain Rust correspondence obligations.

The same certificate walk uses the checked `Range` transfers for variable
products and quotients, absolute value and powers with a singleton nonnegative
exponent. Both children must already be certified, even when a zero factor or
exponent makes the final mathematical value constant. Divisor domains exclude
zero and possible `MIN / -1`; absolute value excludes `MIN`, and every
intermediate must fit i32. Finite-domain tests compare successful certificates
with checked scalar results. Those tests do not establish a Lean refinement of
these additional transfers; their implementation correspondence remains open.

Static totality also enables ordinary prefix filtering during support completion
for eligible flat normal producers, independently of diagnostic requests. Its
compact certificate borrows the exact rule; all producers contribute to the
finished analysis, while complete source-family traversal remains separate. No
computed-key coverage or complete-binding witness follows from this certificate.
The preservation argument is that a defined false plan-decided prefix excludes
every extension, and static totality rules out hidden arithmetic failures on
complete bindings. Earlier prefix failures retain the checked evaluator's
deferred treatment. The all-producer domain correspondence, certificate issuance,
owner authentication and preservation of admission remain Rust obligations;
existing Lean laws do not certify this new execution consumer.

The optional Rust
[finite totality certificate](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_support/projections/totality.rs)
adds a separate justification for ordinary row selection. A full positive
column of the immutable snapshot covers every full binding's value at that
variable, even when it contains extra values excluded by another argument or
relation. Completed support is required for post-admission selection; an
unfinished support round permits only selection justified for its own immutable
snapshot. An
expression reading one variable is evaluated on that whole domain. For sums,
differences, negations, products and division or remainder with variable operands,
Rust checks the numeric hull of every intermediate. This applies to eligible flat normal-rule
bodies, including constraints, without certifying their heads.
`IntegerEnvelopes` supplies
addition, negation, constant-product and signed truncating-quotient bounds, plus
the remainder's magnitude bound. Subtraction uses addition of a negation;
`ScalarArithmetic.checked_success` requires the resulting representability.
Every intermediate must satisfy that requirement, even if the final value fits.
No scalar guard or relation correlation supplies a premise to this interval
check. Unknown or mixed domains, zero divisors and possible overflow decline the certificate,
while owner and resource refusals remain failures. These laws do not prove the
Rust interval transfers, including the variable-product and variable-divisor
cases, or source-column coverage. The argument establishes
definedness on full bindings, not on every earlier join prefix.
One Rust totality attempt reuses successful numeric-column hulls for its fixed
rule and immutable support snapshot. Every intermediate remains checked against
the current hulls. Outside support completion no hull survives the attempt.
Support completion keeps each column hull it reads from one round to the next
and extends it with the appended rows' values; it keeps its integer key
certificates the same way.
[`KeptCertificates`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/KeptCertificates.lean)
reads both as folds of a commutative monoid over the rows: a hull joins the
rows' values and declines on a nonnumeric one, and a key certificate requires
every row's key to be defined. Joining a kept certificate with the appended
rows' certificate gives the whole relation's (`hull_extend`, `defined_extend`),
a declined certificate stays declined (`hull_declined_stays`,
`defined_declined_stays`), and the kept rows are the first rows of every later
snapshot (`kept_rows_prefix`). That support rows are append-only with stable
positions and values, that the Rust hull and key certificate compute these
folds, and that merging the appended keys' sorted entries keeps the index
ordered by key and row remain implementation obligations. Within a growing support round, Rust can retain a
successful expression map for one exact rule and snapshot, then borrow it across
that rule's delta variants. The body-totality premise is independent of the
pivot; each variant separately authenticates its selected multiples against its
own join order. This grants neither completed-support authority nor generic
table selection. Evidence lifetime, storage accounting and error-preserving
reuse remain implementation obligations.

Deterministic scalar assignments now carry checked hulls through their original
dependency order, including unused assignments. A separate comparison DAG
exposes those dependencies to join planning; interval masks reject only prefixes
whose enclosing bounds cannot satisfy a comparison. The original assignments,
head phase and diagnostics remain. Shared expression nodes, full-column
coverage, preservation of signed arithmetic and snapshot ownership are Rust
correspondence obligations. The interval laws above do not establish the new
planner or cursor implementation. Full-family scan omission additionally checks
every generated key and admits only leaves or constructors in the head suffix.

For an eligible body with one closed positive integer range, Rust can supply
the full original interval to the same numeric-hull check. After original-family
admission, a checked divisibility equality selects ascending multiples in the
existing range cursor. The original range width is still admitted; other
comparisons and tuple matching still run. This grants no support-round,
incremental or streamed-constraint certificate. `BoundedMultiples` and
`IntegerEnvelopes` justify the mathematical selection and numeric bounds;
cursor correspondence, canonical identities and diagnostic preservation remain
Rust obligations.

Eager emission, hybrid capture and hybrid model/region checks use the same Rust
preparation and necessary-domain selector. Hybrid capture runs after original
family admission, with an append-capable term owner over completed support.
It must admit every successful speculative value from the covering columns,
including values excluded by other source arguments and successful prefixes of
a declined certificate attempt. The prepared runtime rule checks totality once
against that immutable rule and completed carrier. Later cursors borrow the
completed map read-only, retaining the same covered inputs and canonical results.
Preserving capture's input traversal and admitted canonical identities remains a
Rust obligation; a missing identity is a typed failure. No new Boolean completion
authority or filtered support owner establishes this premise. An input outside
the covered map takes ordinary checked evaluation and cannot extend the retained
map. A declined attempt retains the ordinary cursor behavior, with no certificate.

Candidate row filters neither narrow the certificate's covering domains nor
supply source-family evidence. Their necessary conditions compose with computed
domains while preserving each complete violation witness, as required by
`StreamedRegions.necessary_selection_preserves_witness`. For a fixed final
candidate, `StreamedConstraints.clear_selection` gives the corresponding
complete-satisfaction law. Rust authenticates source rows and the candidate's
catalog, then excludes mapped positive rows absent from that candidate. Unmapped
rows keep the ordinary fallback. Every completed body still queries the model;
this filter lends no region truth or positive-row proof. Runtime preparation and
scratch remain charged to the checker, independently of the original admission
receipt. Retained preparation is charged once; borrowed headers do not claim a
second allocation. Existing laws do not verify this capture/frozen correspondence, typed
failure propagation, or the concrete Rust storage accounting.

Within a selected constraint cursor, an all-held positive prefix can justify
omitting repeated positive atom lookup after complete scalar success. The Rust
obligation is that the prefix covers every original positive occurrence, every
row has the exact mapped identity, and undo truncates evidence with the binding.
An alternative loan identifies one matched open positive occurrence and proves
the others held. Its original occurrence index and dense atom identity must
refer to that same completed binding; negative literals retain their ordinary
truth checks. This is the positive part of `StreamedConsequences.UnitBody`,
not a claim that every remaining literal is sure.
The borrowed body evidence must be consumed with that same immutable region.
It grants no source-family or model-membership authority; final model checks
ignore it. This is a representation refinement of the body check, not an
additional formal certification of the cursor.

The total nongenerated route shares the held-only and designated-positive-pivot
queries in one traversal allowing at most one mapped open positive occurrence.
Its correspondence must cover the union of those queries, including the
all-held case, without lending truth across an unmapped row or stale binding.
The first matched open occurrence still limits traversal beyond an unmapped
gap, and backtracking must remove that restriction at its recorded depth.
The retained rule's complete totality certificate may admit checked scalar
arithmetic to that route. Every rule in the streamed partition must be eligible; one
unproved arithmetic rule retains the sequential first-result schedule for the
whole partition. A sound later unit alone would not authorize bypassing an
earlier rule whose diagnostics could change after propagation. Partial
arithmetic and generated frames retain their existing query schedule. These
coverage and diagnostic obligations are not discharged by `UnitBody` or the
batch laws.

After that obligation is established, `GroundGuards.guard_original` and
`guard_frozen` identify each evaluated comparison with the same Boolean in both
worlds. Together with `RuleFactorization.reduct_conj` and `reduct_imp`, a false
body comparison makes that ground rule implication true in both the original
theory and every frozen reduct. Omitting such instances preserves the semantic theory.
This is a direct application of existing guard and reduct laws; those laws do
not establish the Rust column coverage, arithmetic totality or prefix schedule.
Speculative prefix failures retain the ordinary fallback, typed owner/resource
refusals remain failures, and arithmetic-family evidence still traverses complete
rows, including defined false witnesses. The success-cache laws above justify
reusing successful preparation results without changing their values.

For rejected flat rows, Rust shares atom-key validation between ordinary bodies
and streamed constraints instead of constructing discarded formulas. This relies
on the caller having completed the join and authoritative source-family checks;
generators and nested scopes retain complete scoped validation. Preservation of
binding errors, source locations, independent arithmetic faults and false-body
head suppression remains a Rust obligation, separate from the omission law.

[`ProjectedConditionals`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/ProjectedConditionals.lean)
separates anonymous witness disjunctions, signed source alternatives and
universal condition rows. The Rust
[`Projection`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/formula_ir.rs)
has flat-argument and structural-witness representations. `Builder::project`
completes their witness formula before the conditional compiler applies its
sign. Structural matching uses a private binding frame whose input prefix
excludes later outer bindings. This implements the intended quantifier order;
source support completeness, frame construction and matcher correspondence
remain separate proof obligations. A resource stop cannot establish an empty
completed carrier.

[`TernaryWatch.remaining_unique`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/TernaryWatch.lean)
identifies the one position outside two distinct watches in a three-position
clause. `replacement_exact` equates a covering generic scan with one availability
test at that position. Its Rust correspondent, the clause search's watch
replacement, was removed with that search; the law remains as a statement.

## Read hypotheses as caller obligations

The CPU and device tight checkers pack producer support into 32-bit words.
`TightEvaluation.head_support_append` proves that splitting the producer list
joins each head's support with Boolean OR; `head_support_true` connects that
support to an enabled original producer. The shader represents this OR by
atomic updates to head bits by default. The grouped support primitive gives each
word one owner that reduces its complete producer group.
`TightEvaluation.head_support_group` proves that exact group membership preserves
support for every atom owned by the group. This is a Boolean witness law;
preserving original occurrence counts and charged work requires a separate
argument. Rust grouping, word addressing, atomic execution, barriers and
readback remain unproved implementation correspondences. The packed membership
refinement below concerns a different, 64-bit Rust representation and does not
certify either shader schedule.

Candidate export splits each stored 64-bit word into ordered low and high
32-bit words. Only words covering the declared atom universe are exported;
unused final bits remain zero. Atom enumeration visits set bits in increasing
order. Both operations retain the original interpretation's theory identity.
On their complete routes both tight checkers find unsupported atoms from each
candidate word intersected with the complement of its support word. The CPU
visits words in ascending order and stops at the first nonempty difference; the
shader reduces the first set bit of every nonempty difference. The CPU update
route reports the least atom of its maintained unsupported set instead. All
yield the least unsupported atom;
an original-root failure still takes precedence. `support_true_iff` states the
corresponding per-atom support obligation. The tight shader's
`accumulate_zero_counts` shares the selected-bit filter used to construct its
retained unsupported set and update its touched portion. It preserves an initial
mask and adds exactly the selected atoms whose final packed support count is
zero. `PackedWordIterator.SetBits.least_clear_suffix` and
`lowest_bit_arithmetic` describe each least-bit selection and decrement-clear
step; `List.mem_filter` supplies the pointwise selection law. Relating the
shader's 32-bit operations and packed counters to that law, including completed
producer coverage and barriers before count reads, remains an implementation
obligation. The helper retains the actual count-read order and its callers own
all publication and work receipts. The concrete word conversion,
set-bit iterator, witness reduction and their resource bounds require separate
executable refinement; the retained proof of `Interpretation::contains` does
not cover these operations.

CPU and device membership can consume the same immutable original-theory
`TightPlan`; preparation does not itself classify a candidate. A completed
original-root and ranked-support check uses
`TightEvaluation.computed_verdict_sound`. Failed support under complete producer
coverage uses `TightEvaluation.unsupported_refutes`: every stable interpretation
of that grammar must support each present atom. This rejection needs no rank
assumption and is distinct from failure to model an original root. Accordingly,
the device adapter maps a stable verdict to `NoProperSubset`, original-root
failure to `NotModel`, and authenticated failed support to `Refuted`. The
primitive's failed-support variant is named `Residual` because it does not
return a deletion witness; that name alone supplies no refutation premise.

This shared semantic plan does not identify host scheduling with device work.
Automatic device execution selects the tight checker only after successful
certificate preparation. A theory admitted only by `PositivePlan` still uses
the general device checker. Exact indexed producers, full candidate coverage,
original-root truth, completed scans and decoded verdict identity must satisfy
the respective laws; missing certificates and interrupted or malformed device
results establish none of their conclusions. Rust certificate compilation and
WGSL execution remain outside the proved correspondence.

Tight device truth reuse follows the local principle in
`TightEvaluation.unchanged_inputs_keep_truth`; `updated_values_exact` establishes
the complete update rule for that module's binary Boolean DAG. The shader uses
its existing dependency order, a value bit and a change bit. Boolean gates
recompute their value in one pass and publish whether it changed. An absorbing
value can end a wide Boolean reduction after all children have been evaluated;
this does not skip a child transfer. Native row owners compare the same complete condition
values and preserve every contribution occurrence when retaining their guards.
`DependencySchedule.ReadsOnly` states the local dependency obligation, but neither
module verifies these native row transfers or the two-bit device representation.
Full root and producer scans remain mandatory. Correct truth masking, exact
subject/count ownership, invalidation before mutation, final-success publication,
barriers and conservative work accounting remain Rust/WGSL obligations. Existing
host and physical checks supply evidence at those boundaries; the reuse law does
not establish full executable refinement or a performance improvement.

The device coordinator uses `StableModels::next_batch_with_joint_completion`
for optimizing searches, and its two-phase form (`propose_slate`, `slate`,
`commit_slate`) for enumerations, which runs the same certification and commit:
its checker establishes original satisfaction and the reduct verdict together.
An opaque `CheckedBatch` retains the exact theory and ordered candidate slice
until the coordinator consumes it; on the two-phase form a `CheckedSlate` binds
each submission's results to the theory, the slate's identity and the
submission's offset and count, and the commit refuses any slate but the oldest
pending one. A slate proposed ahead of its predecessor's commit is what
`BatchAccounting.Ledger` already permits: `propose` appends a slate to the
pending ledger before `commit` takes the older prefix. `BatchAccounting` still requires exact
classification before commit; moving original evaluation to the device does
not weaken that premise. The generic callback API retains independent host
validation, including a retry after a failed joint check. Receipt construction,
subject checks, complete decoding and device execution are Rust/WGSL obligations.

Within one native threshold row, GPU frozen propagation also transfers required
lower and forbidden upper bounds between guards true in the original candidate.
`ThresholdPrefixes.frozen_implies` and `frozen_contrapositive`, with an empty
suffix, justify these same-row implications. False original guards are frozen
constants and supply no numeric requirement. The physical owner may itself be
false while serving other live guards; ownership grants no semantic premise.
Atomic intersections, dirty notifications, finite-width comparisons and charged
sweep work remain implementation obligations checked by host and device tests.

The removed static closure transport validated a nonzero submission epoch,
input-world ordinal and completion marker before publishing any batch. Its
marker was written after the workgroup's closure writes synchronized. Exact
record count, verdict bits, closure padding and the original seed's gate
projection were also checked. Historical malformed-record tests exercised
refusal at that host boundary; they did not establish that a particular device
ran the shader. Framing did not independently prove least closure or constraint
satisfaction, and the semantic laws do not certify that retired Rust/WGSL
protocol or arbitrary driver behavior. Its physical tests used independent
complete-closure comparisons across resident submissions; they are historical
evidence, not qualification of a current transport.

For a frozen mask, correctness means agreement with the fixed candidate's
classical truth, not merely matching dimensions. For lazy inference, final
source coverage means coverage at the final positive snapshot, not a successful
earlier scan. For an aggregate, a finite possible carrier is not proof that
every element is realized in a model. For a certificate, original producers and
their rank must satisfy the actual theorem's premises.

The library includes counterexamples to tempting shortcuts: positive
disjunction without a least model, cross-world joins without a common witness,
stale membership omitting newly enabled truth, and empty delivery hiding a valid
answer. These identify the exact invariants an implementation must preserve.

## What the checks establish

[`FactRejection`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/FactRejection.lean)
proves that a rule with `not p` in its body may be omitted from a theory that
retains the fact `p`. Original satisfaction, frozen-reduct satisfaction for
arbitrary outer and tested interpretations, and answer sets are unchanged.
This law does not justify skipping source admission: removed support may have
supplied a later arithmetic or constructor check. No runtime fact-pruning route
is certified or implemented by this result.

`lake build` checks the declared Lean package. Its
[`Audit.lean`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Audit.lean)
and maintained theorem index provide additional declaration and axiom-accounting
checks. An axiom report describes transitive logical dependencies; it is not an
audit of memory safety, allocation, integer overflow or device execution.

Rust unit tests, generated semantic comparisons, resource-boundary tests,
external clingo comparisons and physical GPU checks supply executable evidence
at their respective scopes. Agreement on finite tests is useful but not a proof
of every admitted program. A physical test must establish actual submitted work
on the requested adapter; CPU-authored readback tests alone cannot do that.

The current result is a checked mathematical library together with a tested
native solver. Describing the whole Rust/GPU solver as formally verified would
exceed the established correspondence. Progress consists of closing particular
arrows in this table while preserving explicit limits, not of replacing those
limits with a proof count.

The optional [packed-membership refinement](membership.md) checks the extracted
Rust `Interpretation::contains` body against packed-bit membership under a
storage invariant. It has a separate toolchain and explicit translation and
library-model assumptions; it does not close the whole evaluation layer above.

The [neuromorphic appendix](../appendices/neuromorphic.md) applies the same
discipline to proposed event backends: it separates existing mask and inference
laws from the unproved transport, epoch and completion correspondences.


### Original generator dependency preference

`zetesis_ferraris::GeneratorOrder` orders open candidate atoms; it changes no
formula, held/cut consequence, root-satisfaction test or frozen-reduct query.
Complete `Producers` supply generator marks and body-to-head dependencies. Both
signs of every formula operand participate in the temporary graph; its component
order is a preference, not a foundedness or normal-program certificate. Every
selected atom is checked against the region's open mask. The existing
`Search.CoverageTree.split` and `Region.split_partition`/`Region.split_disjoint` laws
therefore supply the semantic coverage obligation. Rust refinement separately
owes exact theory ownership, valid graph incidences, correct component ordering,
open-only selection and preservation of both children on every execution route.
No Lean claim here certifies the component implementation or measured behavior.
