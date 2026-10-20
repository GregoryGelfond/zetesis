# Composing exact execution

The semantic layer asks about satisfaction, a frozen reduct and minimality. The
execution layer represents the required work as joins, masks, Boolean evaluations,
reductions and independent candidate queries. These primitives are exact
operations; they are not learned approximations or neural attention layers.

The [alignment chapter](alignment.md) connects the two vocabularies with a
diagram, primitive map and pseudocode for closure, frozen satisfaction and
batched membership.
The [ownership chapter](ownership.md) describes prepared data, shared device
contexts, invocation lifetimes and the dependencies that permit parallel work.

## The composed solver boundary

`zetesis-solve` owns the ordinary computation from a coherent `PreparedInput`
to checked `AnswerSet` values and a `SemanticOutcome`. Its configuration contains
execution policy and resource limits. It contains no command-line options,
source-file loading or output writers. `zetesis-cli` admits input and consumes
the same public session interface as an embedding application.

```text
source or native program
    → admission and preparation
    → PreparedInput + SolveConfig + Cancellation
    → SessionBuilder + optional resources and measurements
    → Session: candidate generation → exact membership → checked AnswerSet
    → consumer retention or publication
```

The membership step follows one of the reduct paths below. `SessionBuilder`
composes the effects before execution; it does not add a second semantic engine.
Streaming permits partial consumption. `SessionBuilder::collect` instead owns
unrestricted enumeration and complete retention, producing a `WorldView` only
when both succeed. Output formatting consumes checked values after membership;
an output failure cannot retract checked membership or prove search exhaustion.

This boundary is implemented by
[`Session`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/session.rs),
[`WorldView`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-solve/src/world_view.rs)
and the CLI's
[publication adapter](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cli/src/publication.rs).
The [library manual](../rust/sessions.md) gives checked examples and operation
contracts; the [proof correspondence](../lean/correspondence.md) distinguishes
the semantic laws from the remaining executable refinement obligations.

## Two exact membership paths

For the normal profile, a seed fixes the reduct's gates. Positive inference
starts empty, derives consequences, checks constraints, then compares the
result's gate projection with the seed. The dense graph and source-driven
oracles implement this same acceptance question.

Ordinary sessions retain shared atom selections across each candidate batch.
CPU and GPU checkers borrow their true atoms through `SeedView`; an owned `Seed`
uses the same checking implementation. Eager device packing writes into the
admitted batch buffer without constructing a temporary word vector for each
seed. These ownership and transport choices do not change the frozen gates or
the least-consequence check. Returned answer sets still own their interpretations.

For finite general formulas, the candidate must first satisfy the original
theory. A frozen truth mask then permits repeated evaluations of its reduct
without constructing a separate formula tree. Finding a proper-subset model
rejects the candidate. Proving that none exists accepts it. The exhaustive
`zetesis-ferraris` checker is an exact small-instance reference; ordinary formula
search uses the native candidate and countermodel machinery in `zetesis-sat`.
Its Boolean search representation does not redefine ASP as classical
satisfiability: auxiliary encoding variables do not participate in answer-set
identity or minimality.

Candidates on the formula route can be proposed by regions
(`--search regions`), the same coverage tree the closure route walks
(`Search.lean`), over the theory's atoms. The root leaves every atom open. Under a region every node of the formula DAG
has two readings, decided by one pass over the DAG: sure, when every
candidate of the region satisfies it, and never, when no candidate does; a
held atom is sure, a cut atom never, and the connectives combine the
readings as the closure route's definite and possible gates combine a
rule's. The narrowing closes what every candidate must make of each node
in both directions: every root holds, a node learns from its operands, and
a node teaches its operands what its knowledge leaves them, a conjunction
that holds both operands, a disjunction that holds with one operand
failing the other, and so on through the connectives, until nothing
changes; an atom known is decided and a node known both ways refutes,
which is what unit propagation over a clause form decides. An atom none
of whose producers can support it, each having a failing body or another
head held, is cut, a held one refuting the region, and an atom held with
one producer left demands that producer's body. The `zetesis-ferraris` crate narrows a region by
these rules to a fixed point over the theory's own DAG, without a clause
form (`FormulaBounds.read_sound`, `never_root_refutes`, `known_sound`,
`unsupported_cut`). A region no reading refutes is split
on the open atom the split ranking below prefers, held branch first;
a region with every atom decided is a leaf, and a leaf is a classical model, since at a full decision every
root is sure or never (`FormulaBounds.decided_leaf_models`). The leaf
is the candidate the reduct decides.

Support rechecks carry two producer-row positions with the region's inherited
knowledge. A failed body or competing held head stays unavailable in every
descendant, so a later recheck skips that proved prefix and rechecks its remaining
witnesses. An open atom needs one witness; a held atom needs two before the scan
can stop without forcing a body. Zero or sole support requires complete coverage,
including the inherited failures. Atomic choices keep their existing exception
to competing-head failure. Interrupted scans publish no support conclusion.

Rows with at most two producers keep an allocation-free scan. Reaching a longer
row prepares one owned vector of two compact positions per atom, normally four
bytes per atom. Initialization is charged and fallible; the vector becomes
inherited state only after initialization completes. Repeated scans of one row
use work linear in its producer count plus its number of rechecks. Child clones
copy the owned positions, and restoring a parent replaces a previous child's
progress. Retained-byte receipts include actual vector capacity; cloning retains
the existing infallible allocation contract. This trades bounded per-child
storage for fewer repeated producer reads, without changing the original theory
or the reduct check.

When the complete producer inventory has positive cycles, narrowing also
restricts candidates by possible founded support. The defining whole relaxation
starts empty, propagates possible bodies through positive conjunctions and
disjunctions, and adds every permitted head of each enabled producer. Other body
formulas remain possible unless already known false. Held atoms do not seed
their own cycles. Atoms outside a completed upper closure can be cut; a held
atom outside it refutes the region. This is a necessary restriction on
candidates, not a replacement for reduct membership. Disjunctive heads are
retained together, without shifting.

The pass reuses the original incidence tables and worker scratch. Each worker
prepares the positive Boolean dependencies of producer bodies once for the exact
original theory. Exact SCCs use the immutable relaxed incidences, independently
of region false masks. Only heads in cyclic components need explicit derivation;
other uncut atoms are conceptual seeds. Their occurrences in the selected complete
producer bodies seed directly, avoiding unrelated atom postings. Full replay resets
those cyclic-head bodies, and publication inspects only missing uncut cyclic
heads. Ordinary support and foundedness still alternate to saturation: acyclic paths
can carry a cut to a downstream cycle, so one restricted pass may be weaker than a
full pass even though the fixed point is unchanged. SCC condensation order supplies
the preservation argument; the indexed executable refinement remains separate.
Preparation reuses the existing state cells, atom cursors and queue, retaining one
packed cyclic-atom mask. The body-ID vector temporarily holds up to A+B finishing
entries (atoms plus the full body cone), and its actual retained capacity is charged.
No second incidence graph or component map is retained. Formulas that cannot
contribute to a cyclic-head body remain outside the selected projection. A completed
acyclicity check skips it where ordinary support propagation suffices; interrupted
checks publish no classification. A successfully closed region carries its
completion to its children: if no projected body input becomes false, the upper
closure still covers every uncut atom and needs no repeat pass. The completed
projection for the exact original distinguishes outside nodes from included but blocked nodes;
without that projection every false node conservatively invalidates completion.
A false atom with no positive occurrence in the projection can only remove itself
from that upper closure; every included occurrence still invalidates, even when
already blocked. A held or cut decision that indirectly falsifies an opaque body
also invalidates. Without authenticated projection every false atom invalidates.
This authority belongs to inherited knowledge for the same original subject.
Worker scratch can identify an irrelevant node but cannot
establish completion. Candidate-only restrictions and frozen-reduct
queries do not use this producer closure. The semantic law
`PossibleFoundedness.stable_sub_possible` explains why the restriction preserves
answer sets; correspondence to the indexed Rust implementation remains separate.

Preparation on reuse partitions the same body IDs and immutable roles together so
mutable counters precede seeds. Full replay resets those counters before reading
current masks for boundary and opaque seeds. A true seed immediately visits its
parent and head rows; it need not be queued and read again. Atom events and
newly enabled Boolean nodes remain queued until every seed is initialized.
Projected parents cannot target these seed roles, so no replay observes stale
seed state. The first pass retains the ordinary queue schedule. This changes
event order, not the completed closure, producer coverage or support explanations.
Preparation adds a charged in-place partition and one split index; it stores no
additional region truth and retains the existing allocation and failure rules.

The first invalidated pass below inherited completed knowledge can instead use
[selective replay](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-ferraris/src/regions/founded/local.rs).
The exact original owner and completed projection are checked at entry and again
before replay. Newly false selected inputs identify the affected live cyclic
heads `H`. Forward discovery stops at currently false destinations. Newly false
inputs retain their independently queued notifications; an older false barrier
was already inactive in the inherited completed closure. A cut head cannot
activate, but every affected occurrence of its atom still supplies its own input.
The discovered heads are grouped at the front of the existing queue, preserving
their order, so head-only consumers need not scan formula nodes again.
The pass selects every producer body entering an uncut head in `H`,
including unchanged producers, and their Boolean operands, `B(H)`. A body already
known false remains selected but needs no operand traversal; a shared operand is
still selected through any other live path. Cut heads need no producer evaluation.
The pass freshly initializes `B(H)` under
current masks and saturates it through the shared event engine, filtering local
destinations. Unaffected uncut cyclic atoms are seeds; the completed atom mask
contains those seeds together with the derived heads. Publication and explanation
capture therefore still receive a complete result.

Worker scratch retains one disposable membership byte per original atom/node,
touched IDs and the reused queue; it carries no ancestor body truth or additional
per-region knowledge.
Missing authority, unfinished history or all live cyclic heads being affected requires
full replay, as do later passes in the same narrowing. Allocation, work exhaustion,
cancellation and deadlines remain typed failures; a partial local attempt supplies
no completion and retry uses full replay. Membership and incidence preparation
publish together only after success. Actual capacities count as retained storage;
replacement construction temporarily overlaps old membership with its fresh
replacement and incidence buffers. Retained-byte receipts do not measure that
transient peak. Discovery, selection and cleanup add charged work alongside the
reduced replay.

Candidate-region statistics distinguish producer rechecks (`support_work`) from
possible-foundedness preparation, closure and publication (`founded_work`). Both
are included in `reading_work`, and both retain work spent before interruption.
An omitted closure pass contributes no foundedness work.

A worker may retain a bounded ledger of conditional unsupported sets: if all
signed atom conditions hold, every output atom must be absent. Identical
conditions can share their output union, and a stronger newly certified row can
replace older rows requiring more conditions and excluding fewer atoms. Unions
retain the existing limit of 64 distinct outputs. Merely overlapping conditions
are not combined. Comparisons, packing and bounded compaction are admitted before
any old row is removed; an interrupted attempt preserves the old certificates.
Unrelated rows retain their order. Capture still stops when the ledger is full.
Matching and capture are counted in `support_explanations.work`, separately from
`founded_work`, and both contribute to total reads. Consolidation changes the
physical representation of established consequences, not their reduct meaning.
After a complete premise match, packed output words are filtered against current
held and cut masks before decoding atom IDs. Already-cut outputs need no event;
a held output must still refute the region, even if also cut. Ascending event
order is preserved. The cursor lasts only for that traversal and stores no
knowledge for another region. Work includes loaded words and emitted outputs.

Native nonnegative threshold rows also share consequences during
original-candidate narrowing. Preparation links exact prefixes of grouped
`(condition, weight)` terms. A required lower bound passes from a prefix to its
extensions; an exclusive upper bound passes from an extension to its prefixes.
Equal rows pass both requirements. Transfer continues through an intermediate
row even when it has no guard at that bound. Links stay in the prepared index
and pending rows in worker scratch; existing per-region threshold state holds
the requirements, adding no state to child copies. This transfer is disabled
for frozen-reduct narrowing. Recognition is limited to exact prefixes in
prepared order; other subset relations can remain undiscovered.

For a required finite sum interval, original-candidate narrowing also checks
divisibility of the remaining coefficients. If `B` is already known true and
`d` divides every undecided coefficient, every completion has sum `B + d*k`.
An interval containing none of those values refutes the region, even when its
ordinary lower and upper readings overlap. Coefficients sharing one condition
are grouped first. Unit rows cannot gain from this test and skip it. The
coalesced check reads coefficients from the small end. If the first undecided
coefficient fits within the required interval’s width, every divisor of that
coefficient has a progression point in the interval, and the check ends before
computing a remainder. Deciding conditions or tightening the interval reconsiders
the row; this witness is not retained as a permanent property.

These original-only checks read processed sums after node, bound-transfer,
witness and support queues reach a fixed point. A separate coalesced arithmetic
queue preserves bound-transfer order. For a nonunit weighted row constrained at
both ends, complementary gcds can also exclude one value of an open condition
and force the other. A suffix reduction and forward prefix share one drained
snapshot; new decisions enter ordinary propagation before arithmetic resumes.
Each new wave fixes an open condition, so the process terminates. Scratch retains
reduction capacity, not prior truth, and adds no state to child copies.

Complement preparation is skipped when one open coefficient fits the interval's
width and a positive partial gcd of the other open coefficients also fits.
Every complementary lattice then intersects the interval; clipped branch
endpoints are witnesses under the already-drained ordinary bounds. The test
reads small coefficients first and may stop without reading the rest. It avoids
fruitless complement tests but does not establish an attainable sum or model.
Unit rows and one-sided intervals acquire no complement reduction. All visits,
reductions and actual scratch capacities remain accounted.

Work includes visited coefficients, Euclidean remainder steps and interval tests.
The scan visits smaller coefficients first and stops if the current progression
intersects the interval: further gcd reductions preserve that intersection, so
they cannot refute the region. Deferring the scan can spend more mandatory work
before an arithmetic refutation, but cannot restore an impossible completion:
new decisions and stricter bounds only restrict the earlier possibilities.
Frozen-reduct narrowing is unchanged. The
[residual-sum laws](../lean/correspondence.md#nonnegative-aggregate-thresholds)
state the necessary condition and the remaining implementation obligations.

If lattice reasoning leaves a nonunit row unchanged, a bounded possible-sum
check can refute gaps that divisibility does not detect. It treats each open
grouped condition as independently optional and reduces its nonnegative weights
with packed shifts and unions. The queried offsets end below the required
exclusive upper bound. Missing the required interval refutes the region; finding
a sum merely ends this check, since correlated conditions may prevent that
selection. No condition is forced and no model is certified by the reduction.

The requested mask and optional visit allowance are bounded by the existing
row's payload. An unsuitable span or exhausted local allowance retains ordinary
propagation; allocation failure and global work, cancellation or deadline stops
remain errors. Used words are reinitialized for every attempt, and retained
capacity is accounted as worker scratch. Grounding's complete assignment sums
and this CPU check share one packed shift-and-union kernel. The propagation
operation `zetesis_core::packed::union_shifted_until_hit` checks translated bits
while inserting them, avoiding a separate interval scan. Earlier iterations
found no witness, so only the new translated bits can supply one. A hit may
leave a partial union, which the caller discards; absence requires completing
all admitted reductions. Frozen-reduct narrowing is unchanged.

Once a step proves a region contradictory, narrowing discards it without
visiting the remaining operands, parents or support wakeups. Work accounting
covers the visited prefix; cleanup still settles worker receipts. Partial
knowledge from a refuted or interrupted closure is not reused as a completed
closure. This stopping rule applies to the region decision: operations returning
a complete truth vector or least closure must still construct that result.

The knowledge of a region holds in every region inside it
(`FormulaBounds.known_mono`), so a split hands each child a copy of its
parent's knowledge and the child learns only what the split decided; the
regions still share nothing. The readings are arrays over the DAG, and the
regions partition the space exactly, so generation has the closure route's
shape: data-parallel work inside a region and share-nothing regions beside
one another. The node-sure, node-never, atom-sure, atom-never, seen and chain-witness masks
occupy disjoint spans of one owned word block. Narrowing splits that block once
into borrowed slices; child copies remain independent and include every span,
its boundaries, both counter arrays and the seeded state. A chain counts its
processed neutral operands exactly until a witness settles it: false for a
disjunction, true for a conjunction. Its witness bit records a processed operand
at the opposite, absorbing polarity. Later operand events retain their parent
visit but omit counter updates and witness notices that cannot teach anything.
Other parents, node truth, support wakeups and split ranking remain active.
Queued events contribute neither count nor witness; a pending truth bit alone
does not justify retiring the counter.
The counter arrays choose one width for the lifetime of a knowledge: `u16`,
`u32` or native width, according to a bound on each cell. That bound includes
all parent occurrences of all nodes carrying an atom, rather than the total
size of the theory. Narrow cells reduce the bytes copied with each child.
The witness replaces one counter per chain with one packed bit, subject to
final-word padding. Allocation and retained-header costs remain part of memory
accounting. The arrays retain their existing infallible allocation boundary. A candidate-only
restriction narrows the regions still to visit
without a restart, and no exclusion index is kept, because a leaf is visited
once. Propagation under the original theory, permanent restrictions and current
bound repeats to a shared fixed point before one split ranking. That ranking
uses the active bound when present, otherwise the latest permanent restriction,
otherwise the original theory. With no active restriction, complete original
producer coverage can prepare a generator preference once. It prefers atomic
choice heads and ordinary disjunctive heads having no ordinary singleton
producer, then chooses the earliest ground dependency component and lowest atom
identity within that component. Once no open generator remains, it uses the same
component order over all open atoms. Both signs of formula operands contribute:
atom-to-occurrence, operand-to-parent and producer-body-to-every-head edges.
Headless constraints add no producer edge. This is a scheduling dependency graph,
not positive-program certification or disjunctive shifting. Unused original
formulas can change ties between incomparable components, but cannot reverse a
required dependency precedence.

The reusable `GeneratorOrder` keeps the exact original theory identity, packed
head marks and one rank per atom. Its two temporary compact incidence tables and
iterative component walks take O(A + N + E + H) work and temporary space; retained
space is O(A), and a split reads each eligible open atom once. The work quota is
streamed and refunded like narrowing, preserving every refused prefix. The
existing reduct-preparation byte allowance separately bounds named simultaneous
generator preparation storage. A byte refusal retains the existing preference;
work exhaustion, cancellation, deadlines and allocation failure remain explicit
incompletion. `generator_order` receipts report applicability, preparation work,
peak named bytes and retained bytes. Preparation work is already included in
reading work. One owner is shared by scalar, parallel and joined device candidate
producers; frozen proper-subset searches never use it. Source preparation and
grounding may give eager and hybrid executions different dependency graphs, so
coverage and complete answer families, rather than identical traversal order,
are the preserved contract.

With an active restriction or without a prepared generator order, the existing
preference applies. Open atoms that head an atomic choice root of the
original theory rank first, whether or not the theory lies in the producer
fragment: deciding the choices leaves the atoms they derive to propagation and,
where it applies, the support cut. Among atoms of the same kind, the one with the
most unknown parents in the ranking subject is preferred. When that subject is
a restriction, the original theory's unknown-parent count breaks ties; the
lowest atom breaks any remaining tie. Both counts come from the knowledge
already closed for the region, without preparing another index. Any open
atom is a sound split (the `CoverageTree.split` node of `Search.lean`), so this
order changes the regions visited, never the answers. The packed region masks enumerate
open atoms in ascending order, excluding decided atoms and unused final bits;
each inspected parent count spends one work permit. The primary count is read
once per eligible atom; secondary counts are read only to resolve primary ties.
The traversal itself is one operation in `zetesis-cpu`, shared by both
routes when one worker walks; with several workers the formula route walks
the same tree in `zetesis-sat`'s parallel regions: the walk begins on the
calling thread, and the workers it then starts each own a deque of regions,
from which idle workers steal. The closure
route narrows by its two closures, the formula route by
the readings.

Before a CPU region query searches subsets, an optional checked fragment can
decide membership by least consequences of the frozen reduct. Candidate-false
subformulas become false; true default negations become true. Every active root
must then have a positive conjunction/disjunction body and one distinct live
atomic head. This can cover a candidate of a disjunctive original program without
classifying that whole program as normal. Closure starts empty. Equality with
the candidate proves stability; a smaller closure supplies an independently
checked countermodel. Unsupported active roots retain the general subset query.
Cancellation or exhausted work or storage returns an incomplete result with its
charged work preserved; it does not trigger fallback or establish nonmembership.
Original satisfaction remains mandatory. The CPU workspace reuses source
incidences, never another candidate's consequences; GPU residual completion
currently retains its general subset query.

For the general case, the reduct's proper-subset query is a second region
tree: its root cuts every atom outside the candidate and leaves the
candidate's atoms open, its regions are narrowed by the knowledge of the
frozen reduct, read as the original DAG under the candidate's truth mask,
without the support cut, since a model of the reduct need not be supported;
a leaf other than the candidate is a proper-subset model, the countermodel,
and a covered tree with no such leaf is the proof of minimality
(`ReductRegions.stable_iff_no_countermodel`). Generation and membership
are then the same operation over the same index of the theory, and no
clause form is built anywhere on the route.

Because the regions share nothing, several workers can walk the tree at
once (`--threads`, the host's available parallelism by default), started once
the walk has run on one thread, in wall time since it began, for longer than
their start costs, each deciding the leaves it reaches. The family is exact at
any worker count, each answer arriving once, by the partition law; the order in
which answers arrive is the scalar walk's until the workers start and the
schedule's after, is not promised to repeat between runs, and is not a property
of the result. Verification rests on the laws and on the
oracle comparison of answer sets as sets, not on order or determinism.

Candidate production and membership need not run on the same executor.
`StableModels::with_region_producers` uses the same original-theory readings
and disjoint splits in bounded Rayon rounds. A worker returns a classical leaf;
the coordinator retains it until original satisfaction and reduct membership
have been established. Device execution checks both in one operation. Native
completion finishes any general-reduct residuals. Producers never complete
membership merely to ask the device to repeat it.

```text
while candidate_frontier is unfinished:
    candidates := bounded_parallel_leaves(frontier, original_readings)
    join_producers_and_settle_work()
    verdicts := check_original_and_selected_membership_plan(candidates)
    answers := complete_residuals_and_commit(verdicts)
    yield answers
```

One output slot is reserved for every active producer region. Refutation or
splitting returns the reservation; an emitted leaf consumes it. The batch is
bounded even when workers finish together. A stopped validation retains its
admitted prefix and reports the unfinished suffix explicitly. Every producer
joins and settles its work before membership begins. Original validation has
its own per-candidate work allowance. This schedule does not overlap production
with device execution. Pending-byte limits cover pending/result storage and
original-validation scratch, not the entire region frontier, thread stacks or
process memory.

The generic batch-checker API independently validates original satisfaction
using one `EvaluationWorkspace` per owner. Each candidate receives a full
original-formula evaluation; only allocated capacity survives between candidates. The pending-byte allowance
includes the workspace header and actual capacity. A smaller allowance can drop
optional scratch without invalidating previously checked pending candidates.
Retrying their external checker does not repeat original validation. Separate
receipts report validation work and retained bytes.

Device execution uses the explicit joint-check protocol. A successful
`CheckedBatch` binds decoded results to the exact theory and candidate slice;
all non-`NotModel` results establish original satisfaction, including residuals.
The coordinator consumes these receipts in order and commits only after the
entire pending batch succeeds. A failed device call retains pending candidates;
retrying through the generic API first performs independent original validation.
`NotModel` is a producer invariant failure and cannot become exhaustion or a
partially published batch. The trusted callback contract and actual device
execution remain separate from mechanically proved correspondence.

The executor and its producer slots persist between batches. Each slot keeps
its narrowing worklists and, when needed, one checker for the immutable source.
Slots are used exclusively, may move between executor threads and number at
most the requested workers. The retained checker borrows one stable owner;
stopping or completing production drops the checkers before that owner.
Candidate truth and support knowledge still travel with each region, so reuse
does not transfer deductions between unrelated candidates. Source-filter
callbacks retain their own checker lifetime, separate from formula evaluation.

The original CPU `with_region_workers` operation remains useful when each
worker should perform both operations. The two schedules share the original
formula readings, support conditions, candidate partitions and reduct semantics.
The [batch API](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-sat/src/ferraris/batch.rs)
owns proposal validation and publication; the
[producer](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-sat/src/ferraris/region_proposals.rs)
owns the bounded parallel frontier. `Frontier` and `BatchAccounting` state the
coverage and completion laws; concrete synchronization remains a refinement
obligation.

The regions method is the default, chosen by
[measurement](https://github.com/GregoryGelfond/zetesis/blob/main/docs/book/reference/observations/README.md#the-regions-default-and-one-worker-count)
beside the classical search over a clause form on the same cells, with the
host's workers on the tree; the classical search remains reachable for
comparison. Read as regions the classical search is the same tree:
a search node holds some atoms in and some out, propagation adds the atoms
every classical model agrees on under those decisions, a conflict closes the
node, and a complete assignment is a leaf the reduct decides
(`FormulaRegions.classical_consequence_forces`, `no_model_refutes`). Under
the clauses method the reduct's proper-subset query is the Boolean search
too. Neither method decides membership by anything but the reduct, and
neither keeps learned clauses.

Both paths can restrict candidate generation by necessary conditions. A normal
source constraint supplies a forbidden positive gate conjunction when its
remaining antecedents are witnessed by actual unconditional facts. The binary
seed cursor can skip a whole interval while those gate bits remain true.
Possible support alone cannot supply that witness. Ineligible constraints stay
with the full closure check.

Before its first seed, the closure route narrows the region of seeds it
must enumerate. The region starts with nothing decided. Each pass computes
the region's two closures: the lower one, in which a rule fires only if its
gates hold under every seed of the region, and the upper one, in which each
gate is tested for possibility separately. The upper reading is conservative:
it can enable a rule even when no single seed satisfies all its gates. For
example, an open atom can pass both a required-true and a required-false gate.
A gate atom the lower closure
derives belongs to every answer set and is held in every seed; a gate atom
the upper closure does not derive belongs to no answer set and is cut,
never offered. The next pass reads those decisions, and the passes stop
when one changes nothing. The statistics report the passes and the atoms
held and cut; the rest are open. A constraint that fires in a
lower closure holds under every seed of the region, so no seed is offered
and the program has no answer set.

A completed upper bound supplies the root's potentially present gate atoms
directly. The root takes those atoms outside the lower bound, without scanning
the excluded Cartesian tuples. `GateIndex` computes each retained atom's rank
in the full original gate carrier using signature offsets and domain ranks.
It does not assign new positions to the compact subset. The original program,
gate universe and frozen interpretation therefore retain their identity.
Possible support is not truth: it supplies a covering candidate domain, and
the reduct still decides each candidate. An incomplete upper computation cannot
justify an exclusion. Existing resource-stopped narrowing uses its conservative
incremental fallback; full-carrier position overflow remains an explicit limit.

The narrowed root is then visited region by region rather than counted. A
region holds some of the root's open gate atoms in, cuts some out and
leaves the rest open; it is narrowed to its fixed point by the same two
closures, and the passes decide more of its atoms. A region a definite
constraint refutes, or in which a held atom is not derivable, offers no seed.
A region with no atom left open offers its one seed. Otherwise the
region is split on its highest open atom, into the region where that
atom is out and the region where it is in, visited in that order, which is
the order the flat counter would offer the same seeds in; the root is always
split. A region whose narrowing decided nothing beyond the split that formed
it is counted as a flat interval instead, with the counter's restrictions
inside it, so a program whose gates do not propagate pays two closures per
counted region and no more. The statistics report the regions visited,
refuted, reached as leaves and counted, and the narrowing passes below the root. Each
offered seed is still checked in full against the whole gate carrier. The
laws are `Bounds.narrowed_contains_accepted`, the iterated narrowing of the
open cube (`Bounds.undecided` in the Lean), `Bounds.lower_constraint_refutes` and
`Bounds.conflicting_atom_refutes` for the refutations, and the coverage
tree of `Search.lean`, whose `split` node with `Cube.split_partition` and
`Cube.split_disjoint` makes the regions a partition of their parent and
whose `CoverageTree.mem_outputs_iff` makes the leaves exactly the accepted
seeds of the root.
The program is prepared once for the narrowing and charged as one
preparation; each closure then runs on that preparation in one retained
workspace and is charged as one candidate check. A resource stop inside a
region keeps the completed passes' decisions and counts the region, a stop
in the preparation or the root keeps the bounds of the completed passes, and
a program without gate predicates computes no closure.

For a theory whose complete asserted-head grammar combines ordinary disjunction
and exact atomic choices, every true atom in an answer set needs original support:
an ordinary producer whose body is true and whose other head atoms are false,
or an atomic choice for that atom with a true body. Otherwise removing that atom
leaves a proper-subset model of the reduct. The resulting support condition
restricts only outer proposals; independent producers can still support several
atoms in one disjunctive head. Richer or cross-atom choice forms decline the
certificate, and choices alone do not trigger its construction: at least one
ordinary head must contain a syntactic disjunction. The original theory and
frozen reduct are unchanged.

The [gate restriction laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/GateRestrictions.lean)
and [disjunctive support laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DisjunctiveSupport.lean)
state these necessary conditions. Source binding coverage and the executable
certificate constructors retain separate refinement obligations.

For a complete positive atomic-head theory, `PositivePlan` computes the least
consequences once and checks every original constraint. CPU region modes then
propose only that interpretation, or none when a constraint fails. Accumulated
candidate restrictions and the region filter still apply to the singleton;
the ordinary membership and publication checks retain the original theory owner.
No worker needs to explore candidate regions or allocate a reduct query for this
case. Larger classical models need not be individually refuted: the certificate
already rules them out as answer sets. An unsupported or resource-refused plan
keeps general solving available; an interrupted attempt never establishes
exhaustion. This CPU policy does not change explicit device checking.

Under the clauses method, exact projection exclusions have one owner across
candidate restrictions: the outer cursor retains an index of previously
proposed semantic interpretations, and strengthening the candidate query
rebuilds its traversal while preserving that index; the regions method keeps
no such index, since it visits a leaf once. Original and restriction clauses alone enter the watch lists. A completed
assignment is independently checked against those clauses and looked up in the
exclusion index before becoming another candidate. The original theory and its
reduct remain separate from both operations. The
[projection contract](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-sat/docs/candidate-pruning.md)
states its transactional insertion and logical admission units.

Admitted classical queries use a single packed literal arena and clause-end
offsets. A literal occupies one machine word; repeated offsets preserve empty
clauses. Public `Literal` values remain lossless inputs until range admission.
`Cnf::clauses()` yields ordered borrowed `Clause` views, and `Cnf::clause(index)`
provides checked access. Decoding allocates nothing and preserves variable,
polarity and canonical order. This is a query representation change, with
[separate arithmetic laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/PackedQueryLiterals.lean);
it does not replace the formula reduct or establish a process-memory bound.

Local search operations can specialize while preserving the candidate sequence
of completed search. In a three-literal clause, two distinct watched positions
leave one possible replacement position: `3 - first - second`, with indices from zero to
two. The [replacement operation](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-sat/src/search.rs)
tests that occurrence once, after polling control and charging work. It returns
the same replacement as the general scan. Omitting watched positions can reduce
charged work, so a fixed work limit can permit more progress. The
[ternary law](../lean/correspondence.md) states the position invariant; original
satisfaction and reduct membership retain their existing obligations.

A sufficient class certificate can avoid a full countermodel query. For example,
ranked support under a complete original-producer representation justifies a
tight-program specialization. Refused certificate construction leaves general
reduct checking available. A completed check under a complete tight certificate
can refute an unsupported candidate by the support theorem. These are different procedures
for the same reduct-based membership contract.

The device [ranked-support checker](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/src/tight/check.wgsl)
stores support as one bit per semantic atom in each candidate's row. By default it clears
the row, then each enabled original producer atomically sets its head bit.
After the storage barrier, a bit is set exactly when that head has an enabled
producer. Atomic OR preserves updates from different heads sharing a word and
from repeated producers. The final scan compares candidate and support words:
`candidate & ~support` contains precisely that word's unsupported candidate bits.
Each nonzero word contributes its least set-bit position; the workgroup minimum
retains the least unsupported atom across all words. Candidate tail bits are
zero, and a failed original root still takes precedence over this witness. This representation reduces the support buffer to
`4 * max(worlds * ceil(atoms / 32), 1)` bytes; it does not describe total device
memory or establish a speedup. Shared-word contention remains a measurement
question. Ordinary formula sessions with automatic membership policy select this
primitive on an explicit device backend when complete tight preparation succeeds.

The complete device-work allowance is
`nodes + edges + roots + producers + 2 * ceil(atoms / 32) + levels + numeric`,
where `edges` counts every child occurrence, including repetitions, inline pairs
and implication. `numeric` reserves `48*n+32` per variable threshold and `24*n+16`
per variable extremum cut, for `n` condition occurrences. Original truth reserves
a complete operand scan; Boolean reductions can stop at an absorbing value.
One word scan constructs or initializes support and one
checks missing support. `levels` is zero for serial truth evaluation.
Cold preparation uses a level schedule when its extra storage
and work fit; otherwise it retains serial evaluation. A tighter work allowance
can also select serial evaluation over an already retained graph. Retained
schedule storage remains accounted even then, so reducing a later byte allowance
can still produce a typed capacity refusal. Work receipts and
pre-dispatch admission use this same allowance: an exact allowance is admitted
and one less is refused. Historical work counts need their source identity
when compared. These counts do not measure latency or change membership.

A later tight batch can reuse original truth from a successful batch with the
same theory and candidate count. Each existing truth word holds its value and a
change bit. In dependency order, Boolean gates compute their value in one pass
and compare it with the retained value. Wide conjunctions and disjunctions stop
at the first false or true operand respectively. Their children have already
been evaluated, so this skips only operand reads. Shared numeric row owners
retain unchanged rows and update all their guards together when needed.
Every root and producer is still checked, and support
is rebuilt. Reuse adds no buffers or bindings. Its optional allowance is
`4 + 2*nodes + edges` beyond the complete scan; insufficient work selects complete
evaluation. Change detection can add work when many values change, so reuse does
not itself establish a speedup. `original_truth_reused` records the selected route.

Only complete decoding, healthy context completion and the final cancellation
check authorize reuse. Dispatch consumes that authority before queue writes;
any failure, changed theory/count or explicit clear invalidates it. Without
valid authority, the same encoder clears the prior result hints before computing.
Epoch and slot headers alone never authorize retained truth. All consumers mask
the value bit, including roots and producer bodies.

Preparation shares the CPU certificate constructor and cumulative search budget,
but does not activate CPU certificate checking. The executor is selected before
pipeline creation, so these batches do not first run the general propagator.
Stable support results establish membership; failed original truth is rejected;
an unsupported present atom refutes membership by complete producer coverage.
Theories without a complete tight certificate retain general device propagation
with exact CPU residual completion. Positive programs can also be tight; their
checked certificate determines eligibility. Explicit countermodel policy retains
the general route. There is no device implementation of `PositivePlan` in this
selection.

General device propagation retains decoded residual reasons separately: a
fixed point that leaves the query unresolved, the full-sweep ceiling, or the
device work ceiling. These counters precede exact CPU completion and batch
commit. A later completion failure does not erase them; a submitted batch with
no validated result supplies no decoded reason. Tight support and CPU execution
leave this record absent. Aggregate sweep totals alone cannot identify the
reason for each residual.

With several requested workers, the region proposer produces a bounded batch
in parallel and joins before its device checker starts. CPU production and
device checking do not overlap. Membership, scoring and objective feedback keep
the same original-theory and completed-batch boundaries. Failed device execution
never triggers a hidden CPU replacement.

GPU formula sessions use fixed candidate batches by default. Library callers can
select `GpuBatching::Adaptive` in `SolveConfig`; `batch_size` then supplies the
initial target. A filled, checked and committed batch can grow the next target
within the pending-memory allowance. A smaller admitted device prefix bounds
further growth. Failed checks retain the same pending candidates and target.
Larger batches can reduce submissions but delay answer delivery and objective
feedback; adaptive sizing is not a performance or allocation guarantee.

The library also exposes `TightSupport::Grouped` through
`GpuTightOracle::new_with_support`. Packing places each original producer in the
group for its head's support word, retaining duplicate occurrences. One
invocation reduces that complete group into a local bit mask and overwrites the
word once. Empty groups write zero. This replaces contended producer ORs with
word ownership; original truth, root precedence, barriers and the least-atom
witness scan retain the same contract. The graph adds `ceil(atoms / 32) + 1`
32-bit offsets; fresh packing also needs one temporary 32-bit cursor per word.
Both payloads are charged before allocation. Few or uneven groups can limit
parallelism, so grouped construction is an explicit alternative, not the default.
The result marker identifies the branch that constructed support. A mismatched
policy marker is a readback failure even if the reported verdict agrees; this
validates the protocol, without proving the shader or device implementation.

The maintained physical
[tight-oracle tests](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/tests/integration/hardware_tight.rs)
exercise both policies on the selected Metal or Vulkan backend. They cover
native conjunction/disjunction rows, original-root precedence, least unsupported
atoms, duplicate and skewed producers, packed-word boundaries, batch isolation,
resource refusals and retained-theory identity. The
[performance evidence](../reference/measurement-protocols.md#performance-evidence) separates
these correctness checks from latency and occupancy measurements.

For 256 atoms and 128 candidates, the packed support buffer occupies 4,096 bytes
instead of the earlier per-atom buffer's 131,072 bytes. This saves 126,976 bytes
of logical device buffer storage. Bit packing alone leaves uploaded and
downloaded payloads unchanged; grouped construction additionally uploads its
immutable offsets on a fresh theory.

## Formula evaluation on the device

Both formula kernels borrow the admitted `Theory` through complete `NodeView`
rows. Their shared
[transport packer](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/src/formula_graph.rs)
keeps one four-word header per original node and appends wide operand rows to
the same storage buffer. Binary pairs remain inline; a wider conjunction or
disjunction remains one node with its ordered, repeated child occurrences.
No binary expansion or per-candidate graph copy is made. This immutable view
is retained for one exact theory identity. Unused raw arena cells are omitted;
overlapping raw Boolean spans are copied once per logical row. Numeric guards
retain their original node IDs and share condition and value columns by the
admitted row's identity. Each variable guard has a five-word
descriptor. Constant thresholds and extremum endpoints or impossible cuts need
only a constant header. No host eligibility masks are substituted for formulas.

Each live numeric row also records its owner, guard count and certified
arithmetic width. The complete nonnegative weight total bounds every partial
sum; an ordinal row's greatest rank bounds its selected and empty extrema.
After constant bounds are removed, those bounds justify exact 32- or 64-bit
operations where possible, with the full 128-bit path retained otherwise.
Original evaluation and frozen propagation use the same certificate. It also
omits the high-word column for 32-bit rows; wider rows retain both value words.
This reduces their contribution payload from three words to two without changing
condition identity, guard bounds or ASP order.

For `N` nodes and `W` appended operand/descriptor words, packing checks `4*N+W` against
the 32-bit word-address range. The padded byte size `4*max(4,4*N+W)` is checked
independently against both device buffer limits and included in resident and
cold-upload accounting. Formula cold admission also checks actual retained
staging capacity. Binding counts remain unchanged.

Both formula kernels assign one workgroup to each candidate. One shared
preparation operation groups original DAG nodes by dependency depth, taking the maximum depth
over every child in a native row. Nodes in one level read
only earlier levels, so the lanes can compute them independently:

```text
for level in dependency_order:
    frozen[level] := map(evaluate_original_node, level, candidate, frozen)
    make_level_writes_visible
```

This constructs the original truth used by the Ferraris reduct. Node and root
identities remain unchanged. The order is reused for the same retained theory.
General checking recomputes original truth for each candidate; tight checking may
reuse unchanged values under the successful-batch contract above. A chain with
one node per level keeps
the serial fold because it contains no independent node work. Other narrow
levels may still pay more in barriers than they gain in parallel evaluation.

Dense frozen-reduct propagation reuses those levels, starting downward and
alternating direction each sweep. Asserted roots can constrain operands across
several depths in a downward sweep; an upward sweep carries child consequences
toward parents.
Every node still receives one transfer per complete sweep; only the order changes.
Nodes in a level may share operands, so their transfers retain atomic domain
intersections and the existing protection against stale reads. Every lane reaches
the barrier before the next level. Pure chains run the selected fold on one lane.
Consequences flowing against the current direction can require another sweep; this schedule
does not promise fewer rounds for every graph or make a fixed point a proof of
membership.

The optional `ChangedDependencies` schedule instead visits active bits in the
existing slot-to-gate incidence. Two generations retain notifications arriving
after a gate's turn; each gate runs at most once per sweep. `IncrementalGroups`
adds two permission counts per wide AND/OR row, initialized after frozen domains
and roots. Each removed domain bit updates every affected operand occurrence
before waking the gate. Delayed updates only relax the possible outputs; a
required unique witness still scans distinct physical slots. Both routes retain
the same frozen gates and subset test. If optional storage or its conservative
work reservation cannot fit, preparation reports the scanned or dense route it
actually selected. These policies remain independently selectable for measurement;
Dense is the default.

During propagation, only semantic atoms can witness a proper subset. Each lane
scans its atom positions and the workgroup combines exact summaries:

```text
eligible := candidate_true_atoms_whose_domain_allows_false
(count, last) := reduce(sum_counts, maximum_index, lane_summaries(eligible))
if count = 0: refute_the_proper_subset_query
if count = 1: intersect_domain(last, {false})
```

`last` identifies an atom only when the count is one; zero is a valid atom index.
Auxiliary formula nodes never participate in this subset test. An original-false
subformula remains disabled by the frozen mask throughout propagation. Native
numeric projections make at most two linear scans per guard. Their conservative
bounds preserve completions but may infer less than expanded Boolean formulas.
A residual reports no narrowed atom region; exact host completion retains the
original proper-subset query.

Mandatory device setup charges `2*nodes+edges+atoms+roots+levels`; pure chains
and empty graphs use zero levels. Each complete sweep reserves
`9*nodes+2*edges+atoms+65+levels`, covering binary relation rows, both possible native
operand scans, the dependency-level walk and the strict-subset reduction. Setup and each sweep also reserve
`48*n+32` per variable threshold and `24*n+16` per variable extremum cut, where
`n` counts its condition occurrences. These conservative units include portable
limb arithmetic; they are not raw instruction counts. A limit below setup is refused before dispatch;
a limit preventing another sweep returns an explicit residual for completion.
Cold host preparation takes `O(nodes+source_edges+roots+levels+g*log(g))` work outside
that per-candidate device budget, where `source_edges` includes numeric occurrences
examined before constant folding, and `g` counts variable numeric guards. The
numeric row directory is bounded before reservation, charged by actual capacity,
and released after node packing. The sorted directory shares row transport;
it does not cache a second semantic representation.

[Preparation](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/src/formula/preparation.rs)
owns temporary depth and schedule storage and releases staging after upload.
Only a finalized graph enters residency. Actual staging capacity participates
in cold admission. A later preparation failure can discard old residency while
leaving a healthy context; it cannot publish a result or advance the oracle’s epoch.
The [kernel](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-wgpu/src/formula.wgsl)
implements the level map and subset reduction. Their
[Lean laws and remaining obligations](../lean/correspondence.md) are distinct
from device qualification and timing evidence.

## CPU and GPU responsibilities

| Capability | Execution boundary |
| --- | --- |
| Source loading, parsing, admission and output | Host libraries |
| Independent relational membership | Scalar CPU or owned Rayon pool |
| Shared lazy relational rounds | Host source stream with CPU or GPU chunk evaluation |
| Ordinary GPU formula membership | Complete tight support when certified; otherwise device propagation plus exact native CPU completion of residuals |
| Native aggregate reductions | Bounded reusable CPU/device primitive; availability alone does not imply use by every solve |

wgpu supplies portable device access, including Metal on supported macOS
adapters. With default resources, device availability is discovered at runtime.
Requesting a backend is
not evidence that it ran; adapter identity, submitted work, completed work and
residual work are separate observations. GPU support does not mean the complete
solver is device-resident.

The static reduct oracle reads one framed closure record per candidate. A checked
nonzero epoch identifies the submission, an ordinal identifies the input seed,
and a nonzero marker follows synchronized closure writes. The host validates
these fields, the exact record population, verdict bits and unused closure bits.
It also checks the gate-carrier projection against the original seed. A missing
or inconsistent record fails the whole batch instead of becoming acceptance.
The 32-byte parameter buffer and four-word per-record header count toward the
ordinary authored payload ceiling. Empty batches validate context health and
graph admission without consuming dispatch resources or an epoch. The epoch
sequence survives residency clearing and refuses overflow.

These checks establish transport identity and representation consistency, not
an independent proof of every rule consequence or constraint verdict. Actual
shader execution still relies on the device API contract and requires physical
qualification for each claimed backend; an observed driver fault is not presumed.

For admitted relational programs, automatic materialization selects lazy source
grounding on both CPU and GPU. Hardware changes do not require a complete ground
rule store. Explicit eager grounding retains its compiled graph. Automatic
formula admission can defer eligible terminal positive definitions and reconstruct
them after base membership; other formulas are materialized eagerly. Explicit lazy
CPU execution can stream eligible constraints against answers of its retained
producer core. Formula device membership consumes the materialized theory; terminal
reconstruction runs on the host.

CPU is the default backend throughout the solve. Existing measurements do not
establish a device crossover for an automatic hardware policy; batch size alone
is insufficient evidence. Explicit shared CPU source batching also
remains a CPU policy. An explicit GPU backend prepares its executor during
session setup. Default resources discover a device at that boundary;
caller-supplied [ExecutionResources](../rust/sessions.md#share-execution-resources)
reuse their exact context after policy and capability checks, without discovering
a replacement. Supplying a context does not override a CPU request.

An explicit device failure is returned without a hidden CPU retry. A source
limit or cancellation remains an incomplete result. Device statistics count
actual device attempts and never include seeds checked only by the CPU. This
policy leaves device scheduling separate from the reduct membership contract.

### One install, two executables

Linking the GPU stack starts the platform's graphics frameworks with every
process, so an executable that can use a GPU pays that start on every run,
including the CPU runs that are nearly all of them. The default install
therefore holds two builds of the same source. The `zetesis` a user runs is
built without the GPU stack (the CLI's `front` feature) and runs every CPU
invocation itself. An invocation that opens a GPU device, a solve on a GPU
backend or `zetesis devices`, it hands to the full build installed beside it in
`zetesis-libexec/<identity>/zetesis`, before any input is read or output
written, by replacing its own process; the arguments, standard streams,
environment, process ID and exit status carry over, and the name the operating
system reports stays `zetesis`. Test commands are not handed off: their GPU
work runs in child processes, and each decides for itself.

The identity is the full build's content, which the installer compiles into the
front, so a front can find only the build installed with it; a front built
without one does not compile. The command-line grammar is the same in every
build profile, which a test checks against one recorded rendering of the
command tree, so the two executables read an invocation identically. A front
that cannot run its full build reports `gpu_executable_unavailable`, and never
serves the request on the CPU instead. The JSON statistics and the record view
name the executable that did the work. A CPU-only install has no full build and refuses GPU requests with
`backend_unavailable`; a single build with GPU support, as Cargo installs it,
never hands off.

## Immutable rounds and commit boundaries

Each lazy batch has independent frozen seeds and positive snapshots. All chunks
in one round read those same snapshots. Deltas become the next snapshots only
after the source scan and every required evaluation complete. Catalog growth
preserves atom identity and each world's stride; it cannot turn another world's
truth into a local fact.

An incomplete scan or failed evaluation returns progress without completed
checks from that batch. A formula batch similarly separates pending proposals,
completed local checks and committed results. Cancellation or a device error
must not turn an unfinished operation into acceptance, rejection or exhaustion.

Performance work measures the actual boundary: admission, source work,
candidate generation, oracle calls, residual completion, scoring and output.
Host elapsed time around a GPU call includes transport and waiting; it is not
kernel time. When statistics are requested, a newly created device context also
requests optional wgpu timestamp queries. Formula and tight batches then measure
their compute pass through the existing submission and readback. Unsupported,
invalid or equal counter readings remain explicitly unavailable: equal readings
do not establish zero compute cost. Caller-supplied contexts retain
their granted features; statistics never replace them. Timing storage is accounted
separately and absent when disabled. Logical storage budgets are not process RSS or total device memory.
The [outcome contracts](../rust/outcomes.md) preserve these distinctions for
library consumers.

The [neuromorphic appendix](../appendices/neuromorphic.md) describes a proposed
mapping of the same semantic operations to Loihi 2 and SpiNNaker2. Its event
protocol and qualification steps are design obligations; these targets are not
implemented backends.
